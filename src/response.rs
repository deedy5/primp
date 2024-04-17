use std::collections::HashMap;

use encoding_rs::*;
use pyo3::exceptions;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pythonize::pythonize;
use serde_json::{Error as SerdeError, Value};

/// A struct representing an HTTP response.
///
/// This struct provides methods to access various parts of an HTTP response, such as headers, cookies, status code, and the response body.
/// It also supports decoding the response body as text or JSON, with the ability to specify the character encoding.
#[pyclass]
pub struct Response {
    #[pyo3(get)]
    pub cookies: HashMap<String, String>,
    #[pyo3(get)]
    pub encoding: String,
    #[pyo3(get)]
    pub headers: HashMap<String, String>,
    #[pyo3(get)]
    pub raw: Vec<u8>,
    #[pyo3(get)]
    pub status_code: u16,
    #[pyo3(get)]
    pub url: String,
}

#[pymethods]
impl Response {
    #[getter]
    fn content(&mut self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            // Convert the raw response body to PyBytes
            let py_bytes = PyBytes::new_bound(py, &self.raw);

            // Convert the PyBytes to a Python object
            Ok(py_bytes.into())
        })
    }

    #[getter]
    fn text(&mut self) -> PyResult<String> {
        Python::with_gil(|py| {
            // Release the GIL here because decoding can be CPU-intensive.
            py.allow_threads(|| {
                let encoding = Encoding::for_label(&self.encoding.as_bytes()).ok_or_else(|| {
                    PyErr::new::<exceptions::PyValueError, _>(format!(
                        "Unsupported charset: {}",
                        self.encoding
                    ))
                })?;
                let (decoded_str, detected_encoding, _) = encoding.decode(&self.raw);
                // Redefine resp.encoding if detected_encoding != charset
                if detected_encoding != encoding {
                    self.encoding = detected_encoding.name().to_string();
                }
                Ok(decoded_str.to_string())
            })
        })
    }

    fn json(&mut self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            // Directly parse the raw response body as JSON
            // We release the GIL here because JSON parsing can be CPU-intensive.
            let value: Result<Value, SerdeError> =
                py.allow_threads(|| serde_json::from_slice(&self.raw));

            // Manually convert the serde_json::Error into a pyo3::PyErr
            let json_value = value.map_err(|e| {
                PyErr::new::<exceptions::PyValueError, _>(format!("Failed to parse JSON: {}", e))
            })?;

            // Convert the parsed JSON into a Python object using pythonize
            pythonize(py, &json_value).map_err(|e| {
                PyErr::new::<exceptions::PyValueError, _>(format!(
                    "Failed to convert JSON to Python object: {}",
                    e
                ))
            })
        })
    }
}
