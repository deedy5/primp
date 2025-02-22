use std::sync::Arc;

use anyhow::Result;
use encoding_rs::{Encoding, UTF_8};
use html2text::{
    from_read, from_read_with_decorator,
    render::{RichDecorator, TrivialDecorator},
};
use http_body_util::BodyExt;
use mime::Mime;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    IntoPyObjectExt,
};
use pythonize::pythonize;
use serde_json::from_slice;
use tokio::sync::Mutex as TMutex;

use crate::RUNTIME;

/// A struct representing an HTTP response.
///
/// This struct provides methods to access various parts of an HTTP response, such as headers, cookies, status code, and the response body.
/// It also supports decoding the response body as text or JSON, with the ability to specify the character encoding.
#[pyclass]
pub struct Response {
    pub resp: http::Response<rquest::Body>,
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

        let bytes = py.allow_threads(|| {
            RUNTIME.block_on(async {
                BodyExt::collect(self.resp.body_mut())
                    .await
                    .map(|buf| buf.to_bytes())
            })
        })?;

        let content = PyBytes::new(py, &bytes);
        self._content = Some(content.clone().unbind());
        Ok(content)
    }

    #[getter]
    fn get_encoding<'rs>(&mut self, py: Python<'rs>) -> Result<String> {
        if let Some(encoding) = self._encoding.as_ref() {
            return Ok(encoding.clone());
        }
        let encoding = py.allow_threads(|| {
            self.resp
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<Mime>().ok())
                .and_then(|mime| mime.get_param("charset").map(|charset| charset.to_string()))
                .unwrap_or("utf-8".to_string())
        });
        self._encoding = Some(encoding.clone());
        Ok(encoding)
    }

    #[setter]
    fn set_encoding<'rs>(&mut self, encoding: Option<String>) -> Result<()> {
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
        let text = py.allow_threads(|| {
            let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(raw_bytes);
            text
        });
        Ok(text.into_pyobject_or_pyerr(py)?)
    }

    fn json<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyAny>> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let json_value: serde_json::Value = from_slice(raw_bytes)?;
        let result = pythonize(py, &json_value)?;
        Ok(result)
    }

    #[getter]
    fn get_headers<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyDict>> {
        if let Some(headers) = &self._headers {
            return Ok(headers.clone_ref(py).into_bound(py));
        }

        let new_cookies = PyDict::new(py);
        for (key, value) in self.resp.headers() {
            new_cookies.set_item(key.as_str(), value.to_str()?)?;
        }
        self._headers = Some(new_cookies.clone().unbind());
        Ok(new_cookies)
    }

    #[getter]
    fn get_cookies<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyDict>> {
        if let Some(cookies) = &self._cookies {
            return Ok(cookies.clone_ref(py).into_bound(py));
        }

        let new_cookies = PyDict::new(py);
        let set_cookie_header = self.resp.headers().get_all(http::header::SET_COOKIE);
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
        let text = py.allow_threads(|| from_read(raw_bytes, 100))?;
        Ok(text)
    }

    #[getter]
    fn text_plain(&mut self, py: Python) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text =
            py.allow_threads(|| from_read_with_decorator(raw_bytes, 100, TrivialDecorator::new()))?;
        Ok(text)
    }

    #[getter]
    fn text_rich(&mut self, py: Python) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text =
            py.allow_threads(|| from_read_with_decorator(raw_bytes, 100, RichDecorator::new()))?;
        Ok(text)
    }
}

#[pyclass]
struct ResponseStream {
    resp: Arc<TMutex<http::Response<rquest::Body>>>,
}

#[pymethods]
impl ResponseStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        let chunk = py.allow_threads(|| {
            RUNTIME.block_on(async { self.resp.lock().await.body_mut().frame().await.transpose() })
        });
        match chunk {
            Ok(Some(frame)) => match frame.into_data() {
                Ok(data) => Ok(Some(PyBytes::new(py, data.as_ref()))),
                Err(e) => {
                    let error_msg = format!("Failed to convert frame to data: {:?}", e);
                    Err(pyo3::exceptions::PyRuntimeError::new_err(error_msg))
                }
            },
            Ok(None) => Err(pyo3::exceptions::PyStopIteration::new_err(
                "The iterator is exhausted",
            )),
            Err(e) => {
                let error_msg = format!("Failed to read chunk: {:?}", e);
                Err(pyo3::exceptions::PyRuntimeError::new_err(error_msg))
            }
        }
    }
}

#[pymethods]
impl Response {
    fn stream(&mut self, py: Python<'_>) -> PyResult<ResponseStream> {
        self.get_cookies(py)?;
        self.get_headers(py)?;
        self.get_encoding(py)?;
        let mut temp = http::Response::default();
        std::mem::swap(&mut self.resp, &mut temp);
        let resp = Arc::new(TMutex::new(temp));
        Ok(ResponseStream { resp })
    }
}
