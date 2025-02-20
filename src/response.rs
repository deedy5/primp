use anyhow::Result;
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

use crate::RUNTIME;
use http_body_util::BodyExt;

/// A struct representing an HTTP response.
///
/// This struct provides methods to access various parts of an HTTP response, such as headers, cookies, status code, and the response body.
/// It also supports decoding the response body as text or JSON, with the ability to specify the character encoding.
#[pyclass]
pub struct Response {
    pub resp: http::Response<rquest::Body>,
    pub _content: Option<Py<PyBytes>>,
    pub _encoding: Option<String>,
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

    #[getter]
    fn text(&mut self, py: Python) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let encoding = self.get_encoding(py)?;
        let raw_bytes = content.as_bytes(py);
        let text = py.allow_threads(|| {
            let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(raw_bytes);
            text
        });
        Ok(text.to_string())
    }

    fn json<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyAny>> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let json_value: serde_json::Value = from_slice(raw_bytes)?;
        let result = pythonize(py, &json_value)?;
        Ok(result)
    }

    #[getter]
    fn headers<'rs>(&self, py: Python<'rs>) -> Result<Bound<'rs, PyDict>> {
        let res = PyDict::new(py);
        for (key, value) in self.resp.headers() {
            res.set_item(key.as_str(), value.to_str()?)?;
        }
        Ok(res)
    }

    #[getter]
    fn cookies<'rs>(&self, py: Python<'rs>) -> Result<Bound<'rs, PyDict>> {
        let cookie_dict = PyDict::new(py);
        let set_cookies = self.resp.headers().get_all(http::header::SET_COOKIE);
        for cookie_header in set_cookies.iter() {
            if let Ok(cookie_str) = cookie_header.to_str() {
                if let Some((name, value)) = cookie_str.split_once('=') {
                    cookie_dict
                        .set_item(name.trim(), value.split(';').next().unwrap_or("").trim())?;
                }
            }
        }
        Ok(cookie_dict)
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
