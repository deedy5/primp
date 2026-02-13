use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use html2text::{
    from_read, from_read_with_decorator,
    render::{RichDecorator, TrivialDecorator},
};
use mime::Mime;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    IntoPyObjectExt,
};
use pythonize::pythonize;
use serde_json::from_slice;
use tokio::sync::Mutex as TMutex;

use crate::error::{convert_reqwest_error, create_json_decode_error};
use crate::RUNTIME;

/// Extract encoding from Content-Type header.
///
/// Returns the encoding specified in the charset parameter, or UTF-8 as fallback.
fn extract_encoding(headers: &reqwest::header::HeaderMap) -> &'static Encoding {
    headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<Mime>().ok())
        .and_then(|mime| mime.get_param("charset").map(|c| c.as_str().to_string()))
        .and_then(|name| Encoding::for_label(name.as_bytes()))
        .unwrap_or(UTF_8)
}

/// A struct representing an HTTP response.
///
/// This struct provides methods to access various parts of an HTTP response, such as headers, cookies, status code, and the response body.
/// It also supports decoding the response body as text or JSON, with the ability to specify the character encoding.
#[pyclass]
pub struct Response {
    pub resp: Option<reqwest::Response>,
    pub _content: Option<Py<PyBytes>>,
    pub _encoding: Option<String>,
    pub _headers: Option<Py<PyDict>>,
    pub _cookies: Option<Py<PyDict>>,
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub status_code: u16,
}

#[pymethods]
impl Response {
    #[getter]
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyBytes>> {
        if let Some(content) = &self._content {
            let cloned = content.clone_ref(py);
            return Ok(cloned.into_bound(py));
        }
        let bytes = py.detach(|| {
            RUNTIME.block_on(async {
                // Collect all chunks using chunk() which takes &mut self
                // Pre-allocate with 8KB initial capacity to reduce reallocations
                let mut all_chunks = Vec::with_capacity(8 * 1024);
                loop {
                    match self
                        .resp
                        .as_mut()
                        .expect("Response was moved for streaming")
                        .chunk()
                        .await
                    {
                        Ok(Some(chunk)) => all_chunks.extend_from_slice(&chunk),
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                Bytes::from(all_chunks)
            })
        });

        let content = PyBytes::new(py, &bytes);
        self._content = Some(content.clone().unbind());
        Ok(content)
    }

    #[getter]
    fn get_encoding<'rs>(&mut self, py: Python<'rs>) -> Result<String> {
        if let Some(encoding) = self._encoding.as_ref() {
            return Ok(encoding.clone());
        }
        let encoding = py.detach(|| {
            let resp = self.resp.as_ref().expect("Response was moved for streaming");
            extract_encoding(resp.headers()).name().to_string()
        });
        self._encoding = Some(encoding.clone());
        Ok(encoding)
    }

    #[setter]
    fn set_encoding(&mut self, encoding: Option<String>) -> Result<()> {
        if let Some(encoding) = encoding {
            self._encoding = Some(encoding);
        }
        Ok(())
    }

    #[getter]
    fn text<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyString>> {
        let content = self.get_content(py)?.unbind();
        let encoding = self.get_encoding(py)?;
        let raw_bytes = content.as_bytes(py);
        let text = py.detach(|| {
            let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(raw_bytes);
            text
        });
        Ok(text.into_pyobject_or_pyerr(py)?)
    }

    fn json<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyAny>> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let json_value: serde_json::Value = from_slice(raw_bytes).map_err(|e| {
            let doc = String::from_utf8_lossy(raw_bytes).to_string();
            let msg = e.to_string();
            let pos = e.line().saturating_sub(1);
            let lineno = e.line();
            let colno = e.column();
            create_json_decode_error(msg, doc, pos, lineno, colno)
        })?;
        let result = pythonize(py, &json_value)?;
        Ok(result)
    }

    #[getter]
    fn get_headers<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyDict>> {
        if let Some(headers) = &self._headers {
            return Ok(headers.clone_ref(py).into_bound(py));
        }

        let new_headers = PyDict::new(py);
        for (key, value) in self
            .resp
            .as_ref()
            .expect("Response was moved for streaming")
            .headers()
        {
            new_headers.set_item(key.as_str(), value.to_str()?)?;
        }
        self._headers = Some(new_headers.clone().unbind());
        Ok(new_headers)
    }

    #[getter]
    fn get_cookies<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyDict>> {
        if let Some(cookies) = &self._cookies {
            return Ok(cookies.clone_ref(py).into_bound(py));
        }

        let new_cookies = PyDict::new(py);
        let set_cookie_header = self
            .resp
            .as_ref()
            .expect("Response was moved for streaming")
            .headers()
            .get_all(reqwest::header::SET_COOKIE);
        for cookie_header in set_cookie_header.iter() {
            if let Ok(cookie_str) = cookie_header.to_str() {
                if let Some((name, value)) = cookie_str.split_once('=') {
                    new_cookies
                        .set_item(name.trim(), value.split(';').next().unwrap_or("").trim())?;
                }
            }
        }
        self._cookies = Some(new_cookies.clone().unbind());
        Ok(new_cookies)
    }

    #[getter]
    fn text_markdown(&mut self, py: Python) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text = py.detach(|| from_read(raw_bytes, 100))?;
        Ok(text)
    }

    #[getter]
    fn text_plain(&mut self, py: Python) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text =
            py.detach(|| from_read_with_decorator(raw_bytes, 100, TrivialDecorator::new()))?;
        Ok(text)
    }

    #[getter]
    fn text_rich(&mut self, py: Python) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text = py.detach(|| from_read_with_decorator(raw_bytes, 100, RichDecorator::new()))?;
        Ok(text)
    }

    fn read<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyBytes>> {
        self.get_content(py)
    }

    fn iter_content<'rs>(
        &mut self,
        py: Python<'rs>,
        chunk_size: usize,
    ) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        let chunk = py.detach(|| {
            RUNTIME.block_on(async {
                match self
                    .resp
                    .as_mut()
                    .expect("Response was moved for streaming")
                    .chunk()
                    .await
                {
                    Ok(Some(data)) => {
                        let chunk: Vec<u8> = if chunk_size > 0 && data.len() > chunk_size {
                            data[..chunk_size].to_vec()
                        } else {
                            data.to_vec()
                        };
                        Ok(Some(chunk))
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(e),
                }
            })
        });

        match chunk {
            Ok(Some(data)) => Ok(Some(PyBytes::new(py, &data))),
            Ok(None) => Ok(None),
            Err(e) => Err(convert_reqwest_error(e, None)),
        }
    }

    fn iter_lines<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Vec<String>>> {
        let chunk = py.detach(|| {
            RUNTIME.block_on(async {
                match self
                    .resp
                    .as_mut()
                    .expect("Response was moved for streaming")
                    .chunk()
                    .await
                {
                    Ok(Some(data)) => {
                        let text = String::from_utf8_lossy(&data);
                        let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
                        Ok(if lines.is_empty() { None } else { Some(lines) })
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(e),
                }
            })
        });

        match chunk {
            Ok(lines) => Ok(lines),
            Err(e) => Err(convert_reqwest_error(e, None)),
        }
    }

    fn raise_for_status(&self) -> PyResult<()> {
        if self.status_code >= 400 {
            let reason = if self.status_code < 600 {
                match self.status_code {
                    400 => "Bad Request",
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    404 => "Not Found",
                    405 => "Method Not Allowed",
                    409 => "Conflict",
                    500 => "Internal Server Error",
                    502 => "Bad Gateway",
                    503 => "Service Unavailable",
                    _ => "Error",
                }
            } else {
                "Unknown Error"
            };

            return Err(pyo3::exceptions::PyException::new_err(format!(
                "{}: {}",
                self.status_code, reason
            )));
        }
        Ok(())
    }
}

#[pyclass]
pub struct ResponseStream {
    resp: Arc<TMutex<reqwest::Response>>,
}

#[pymethods]
impl ResponseStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        let chunk = py.detach(|| {
            RUNTIME.block_on(async { self.resp.lock().await.chunk().await })
        });
        match chunk {
            Ok(Some(data)) => Ok(Some(PyBytes::new(py, &data))),
            Ok(None) => Err(pyo3::exceptions::PyStopIteration::new_err(
                "The iterator is exhausted",
            )),
            Err(e) => Err(convert_reqwest_error(e, None)),
        }
    }
}

#[pymethods]
impl Response {
    fn stream(&mut self, py: Python<'_>) -> PyResult<ResponseStream> {
        self.get_cookies(py)?;
        self.get_headers(py)?;
        self.get_encoding(py)?;
        // Take ownership of resp for streaming
        let resp = self
            .resp
            .take()
            .expect("Response already used for streaming");
        let resp = Arc::new(TMutex::new(resp));
        Ok(ResponseStream { resp })
    }
}
