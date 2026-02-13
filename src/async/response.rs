use std::sync::Arc;

use encoding_rs::{Encoding, UTF_8};
use html2text::{
    from_read, from_read_with_decorator,
    render::{RichDecorator, TrivialDecorator},
};
use mime::Mime;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
};
use pythonize::pythonize;
use serde_json::from_slice;
use tokio::sync::Mutex as TMutex;

use crate::error::{convert_reqwest_error, PrimpError};

/// Collect all chunks from a response into a Vec<u8>.
///
/// This helper function eliminates the duplicated chunk collection logic
/// that was previously repeated across multiple methods.
async fn collect_chunks(resp: &Arc<TMutex<reqwest::Response>>) -> Result<Vec<u8>, PyErr> {
    let mut all_chunks = Vec::with_capacity(8 * 1024);
    loop {
        let mut resp_guard = resp.lock().await;
        match resp_guard.chunk().await {
            Ok(Some(chunk)) => all_chunks.extend_from_slice(&chunk),
            Ok(None) => break,
            Err(e) => return Err(convert_reqwest_error(e, None)),
        }
    }
    Ok(all_chunks)
}

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

/// A struct representing an async HTTP response.
#[pyclass]
pub struct AsyncResponse {
    resp: Arc<TMutex<reqwest::Response>>,
    _content: Option<Py<PyBytes>>,
    _encoding: Option<String>,
    _headers: Option<Py<PyDict>>,
    _cookies: Option<Py<PyDict>>,
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub status_code: u16,
}

impl AsyncResponse {
    pub fn new(resp: reqwest::Response, url: String, status_code: u16) -> Self {
        AsyncResponse {
            resp: Arc::new(TMutex::new(resp)),
            _content: None,
            _encoding: None,
            _headers: None,
            _cookies: None,
            url,
            status_code,
        }
    }
}

#[pymethods]
impl AsyncResponse {
    #[getter]
    fn get_content<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        if let Some(content) = &self._content {
            let cloned = content.clone_ref(py);
            return Ok(cloned.into_any().into_bound(py));
        }

        let resp = Arc::clone(&self.resp);
        let future = async move { collect_chunks(&resp).await };

        future_into_py(py, future)
    }

    #[getter]
    fn get_encoding<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        if let Some(encoding) = &self._encoding {
            return Ok(encoding.clone().into_pyobject(py)?.into_any());
        }

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let resp_guard = resp.lock().await;
            let encoding = extract_encoding(resp_guard.headers());
            Ok::<String, PyErr>(encoding.name().to_string())
        };

        future_into_py(py, future)
    }

    #[getter]
    fn get_text<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let raw_bytes = collect_chunks(&resp).await?;
            let resp_guard = resp.lock().await;
            let encoding = extract_encoding(resp_guard.headers());
            let (text, _, _) = encoding.decode(&raw_bytes);
            Ok::<String, PyErr>(text.to_string())
        };

        future_into_py(py, future)
    }

    #[getter]
    fn get_json<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let raw_bytes = collect_chunks(&resp).await?;
            let json_value: serde_json::Value = from_slice(&raw_bytes)
                .map_err(|e| PyErr::from(PrimpError::JsonError(e.to_string())))?;

            Python::attach(|py| {
                pythonize(py, &json_value)
                    .map(|obj| obj.unbind())
                    .map_err(PyErr::from)
            })
        };

        future_into_py(py, future)
    }

    #[getter]
    fn get_headers<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        if let Some(headers) = &self._headers {
            let cloned = headers.clone_ref(py);
            return Ok(cloned.into_any().into_bound(py));
        }

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let resp_guard = resp.lock().await;
            let headers = resp_guard.headers();

            Python::attach(|py| {
                let new_headers = PyDict::new(py);
                for (name, value) in headers.iter() {
                    new_headers.set_item(name.as_str(), value.to_str().unwrap_or(""))?;
                }
                Ok::<Py<PyDict>, PyErr>(new_headers.unbind())
            })
        };

        future_into_py(py, future)
    }

    #[getter]
    fn get_cookies<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        if let Some(cookies) = &self._cookies {
            let cloned = cookies.clone_ref(py);
            return Ok(cloned.into_any().into_bound(py));
        }

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let resp_guard = resp.lock().await;
            let headers = resp_guard.headers();

            Python::attach(|py| {
                let new_cookies = PyDict::new(py);
                if let Some(cookie_header) = headers.get(reqwest::header::COOKIE) {
                    if let Ok(cookie_str) = cookie_header.to_str() {
                        for cookie in cookie_str.split(';') {
                            if let Some((key, value)) = cookie.split_once('=') {
                                new_cookies.set_item(key.trim(), value.trim())?;
                            }
                        }
                    }
                }
                Ok::<Py<PyDict>, PyErr>(new_cookies.unbind())
            })
        };

        future_into_py(py, future)
    }

    fn read<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.get_content(py)
    }

    fn text<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.get_text(py)
    }

    fn json<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.get_json(py)
    }

    fn text_markdown<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let raw_bytes = collect_chunks(&resp).await?;
            let resp_guard = resp.lock().await;
            let encoding = extract_encoding(resp_guard.headers());
            let (text, _, _) = encoding.decode(&raw_bytes);
            Ok::<String, PyErr>(from_read(text.as_bytes(), 80).unwrap())
        };

        future_into_py(py, future)
    }

    fn text_plain<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let raw_bytes = collect_chunks(&resp).await?;
            let resp_guard = resp.lock().await;
            let encoding = extract_encoding(resp_guard.headers());
            let (text, _, _) = encoding.decode(&raw_bytes);
            Ok::<String, PyErr>(
                from_read_with_decorator(text.as_bytes(), 80, TrivialDecorator::new()).unwrap(),
            )
        };

        future_into_py(py, future)
    }

    fn text_rich<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let raw_bytes = collect_chunks(&resp).await?;
            let resp_guard = resp.lock().await;
            let encoding = extract_encoding(resp_guard.headers());
            let (text, _, _) = encoding.decode(&raw_bytes);
            Ok::<String, PyErr>(
                from_read_with_decorator(text.as_bytes(), 80, RichDecorator::new()).unwrap(),
            )
        };

        future_into_py(py, future)
    }

    fn iter_content<'py>(
        &mut self,
        py: Python<'py>,
        chunk_size: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            match resp_guard.chunk().await {
                Ok(Some(data)) => {
                    let chunk: Vec<u8> = if chunk_size > 0 && data.len() > chunk_size {
                        data[..chunk_size].to_vec()
                    } else {
                        data.to_vec()
                    };
                    Ok::<Option<Vec<u8>>, PyErr>(Some(chunk))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(convert_reqwest_error(e, None)),
            }
        };

        future_into_py(py, future)
    }

    fn iter_lines<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            match resp_guard.chunk().await {
                Ok(Some(data)) => {
                    let text = String::from_utf8_lossy(&data);
                    let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
                    Ok::<Option<Vec<String>>, PyErr>(if lines.is_empty() {
                        None
                    } else {
                        Some(lines)
                    })
                }
                Ok(None) => Ok(None),
                Err(e) => Err(convert_reqwest_error(e, None)),
            }
        };

        future_into_py(py, future)
    }

    fn raise_for_status<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let status_code = self.status_code;
        let url = self.url.clone();
        let future = async move {
            if status_code >= 400 {
                let reason = if status_code < 600 {
                    match status_code {
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

                return Err(PyErr::from(PrimpError::HttpStatus(
                    status_code,
                    reason.to_string(),
                    url,
                )));
            }
            Ok::<(), PyErr>(())
        };

        future_into_py(py, future)
    }

    fn __aiter__(&self) -> PyResult<AsyncResponseStream> {
        Ok(AsyncResponseStream {
            resp: Arc::clone(&self.resp),
        })
    }

    fn aread<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.read(py)
    }

    fn acontent<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.get_content(py)
    }

    fn atext<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.get_text(py)
    }

    fn ajson<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.get_json(py)
    }

    fn aiter_content<'py>(
        &mut self,
        py: Python<'py>,
        chunk_size: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.iter_content(py, chunk_size)
    }

    fn aiter_lines<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.iter_lines(py)
    }
}

/// Async iterator for streaming response content.
#[pyclass]
pub struct AsyncResponseStream {
    resp: Arc<TMutex<reqwest::Response>>,
}

#[pymethods]
impl AsyncResponseStream {
    fn __anext__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            match resp_guard.chunk().await {
                Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                Ok(None) => Ok(None),
                Err(e) => Err(convert_reqwest_error(e, None)),
            }
        };

        future_into_py(py, future)
    }
}
