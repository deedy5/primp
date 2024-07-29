use encoding_rs::Encoding;
use html2text::{from_read, from_read_with_decorator, render::text_renderer::TrivialDecorator};
use pyo3::exceptions;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyString};

/// A struct representing an HTTP response.
///
/// This struct provides methods to access various parts of an HTTP response, such as headers, cookies, status code, and the response body.
/// It also supports decoding the response body as text or JSON, with the ability to specify the character encoding.
#[pyclass]
pub struct Response {
    #[pyo3(get)]
    pub content: Py<PyBytes>,
    #[pyo3(get)]
    pub cookies: Py<PyDict>,
    #[pyo3(get)]
    pub encoding: Py<PyString>,
    #[pyo3(get)]
    pub headers: Py<PyDict>,
    #[pyo3(get)]
    pub status_code: Py<PyAny>,
    #[pyo3(get)]
    pub url: Py<PyString>,
}

#[pymethods]
impl Response {
    #[getter]
    fn text(&mut self, py: Python) -> PyResult<String> {
        let encoding_name = &self.encoding.bind(py).to_string();

        // Convert Py<PyBytes> to &[u8]
        let raw_bytes = &self.content.bind(py).as_bytes();

        // Release the GIL here because decoding can be CPU-intensive
        let (decoded_str, detected_encoding_name) = py.allow_threads(|| {
            let encoding_name_bytes = &encoding_name.as_bytes().to_vec();
            let encoding = Encoding::for_label(encoding_name_bytes).ok_or_else(|| {
                PyErr::new::<exceptions::PyValueError, _>(format!(
                    "Unsupported charset: {}",
                    String::from_utf8_lossy(encoding_name_bytes)
                ))
            })?;
            let (decoded_str, detected_encoding, _) = encoding.decode(raw_bytes);
            // Return the decoded string and the name of the detected encoding
            Ok::<(String, String), PyErr>((
                decoded_str.to_string(),
                detected_encoding.name().to_string(),
            ))
        })?;

        // Update self.encoding based on the detected encoding
        if encoding_name != &detected_encoding_name {
            self.encoding = PyString::new_bound(py, &detected_encoding_name).into();
        }

        Ok(decoded_str)
    }

    fn json(&mut self, py: Python) -> PyResult<PyObject> {
        let json_module = PyModule::import_bound(py, "json")?;
        let loads = json_module.getattr("loads")?;
        let result = loads.call1((&self.content,))?.extract::<PyObject>()?;
        Ok(result)
    }

    #[getter]
    fn text_markdown(&mut self, py: Python) -> PyResult<String> {
        let raw_bytes = self.content.bind(py).as_bytes();
        let text = py.allow_threads(|| from_read(raw_bytes, 100));
        Ok(text)
    }

    #[getter]
    fn text_plain(&mut self, py: Python) -> PyResult<String> {
        let raw_bytes = self.content.bind(py).as_bytes();
        let text =
            py.allow_threads(|| from_read_with_decorator(raw_bytes, 100, TrivialDecorator::new()));
        Ok(text)
    }
}
