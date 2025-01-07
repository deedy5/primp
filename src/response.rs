use crate::utils::{get_encoding_from_content, get_encoding_from_headers};
use anyhow::{anyhow, Result};
use encoding_rs::Encoding;
use foldhash::fast::RandomState;
use html2text::{
    from_read, from_read_with_decorator,
    render::{RichDecorator, TrivialDecorator},
};
use indexmap::IndexMap;
use pyo3::{prelude::*, types::PyBytes};
use pythonize::pythonize;
use serde_json::from_slice;

/// A struct representing an HTTP response.
///
/// This struct provides methods to access various parts of an HTTP response, such as headers, cookies, status code, and the response body.
/// It also supports decoding the response body as text or JSON, with the ability to specify the character encoding.
#[pyclass]
pub struct Response {
    #[pyo3(get)]
    pub content: Py<PyBytes>,
    #[pyo3(get)]
    pub cookies: IndexMap<String, String, RandomState>,
    #[pyo3(get, set)]
    pub encoding: String,
    #[pyo3(get)]
    pub headers: IndexMap<String, String, RandomState>,
    #[pyo3(get)]
    pub status_code: u16,
    #[pyo3(get)]
    pub url: String,
}

#[pymethods]
impl Response {
    #[getter]
    fn get_encoding(&mut self, py: Python) -> Result<&String> {
        if !self.encoding.is_empty() {
            return Ok(&self.encoding);
        }
        self.encoding = get_encoding_from_headers(&self.headers)
            .or_else(|| get_encoding_from_content(self.content.as_bytes(py)))
            .unwrap_or_else(|| "utf-8".to_string());
        Ok(&self.encoding)
    }

    #[getter]
    fn text(&mut self, py: Python) -> Result<String> {
        // If self.encoding is empty, call get_encoding to populate self.encoding
        if self.encoding.is_empty() {
            self.get_encoding(py)?;
        }

        // Convert Py<PyBytes> to &[u8]
        let raw_bytes = self.content.as_bytes(py);

        // Release the GIL here because decoding can be CPU-intensive
        py.allow_threads(|| {
            let encoding = Encoding::for_label(self.encoding.as_bytes())
                .ok_or_else(|| anyhow!("Unsupported charset: {}", self.encoding))?;
            let (decoded_str, detected_encoding, _) = encoding.decode(raw_bytes);

            // Update self.encoding based on the detected encoding
            if self.encoding != detected_encoding.name() {
                self.encoding = detected_encoding.name().to_string();
            }

            Ok(decoded_str.to_string())
        })
    }

    fn json(&mut self, py: Python) -> Result<PyObject> {
        let json_value: serde_json::Value = from_slice(self.content.as_bytes(py))?;
        let result = pythonize(py, &json_value).unwrap().unbind();
        Ok(result)
    }

    #[getter]
    fn text_markdown(&mut self, py: Python) -> Result<String> {
        let raw_bytes = self.content.bind(py).as_bytes();
        let text = py.allow_threads(|| from_read(raw_bytes, 100))?;
        Ok(text)
    }

    #[getter]
    fn text_plain(&mut self, py: Python) -> Result<String> {
        let raw_bytes = self.content.bind(py).as_bytes();
        let text =
            py.allow_threads(|| from_read_with_decorator(raw_bytes, 100, TrivialDecorator::new()))?;
        Ok(text)
    }

    #[getter]
    fn text_rich(&mut self, py: Python) -> Result<String> {
        let raw_bytes = self.content.bind(py).as_bytes();
        let text =
            py.allow_threads(|| from_read_with_decorator(raw_bytes, 100, RichDecorator::new()))?;
        Ok(text)
    }
}
