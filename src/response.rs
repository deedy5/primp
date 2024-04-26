use encoding_rs::*;
use pyo3::exceptions;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyString};
use pythonize::pythonize;
use serde_json::{Error as SerdeError, Value};

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
    pub url: Py<PyAny>,
}

#[pymethods]
impl Response {
    #[getter]
    fn text(&mut self, py: Python) -> PyResult<String> {
        // Access the string data as bytes
        let encoding_bytes = &self.encoding.bind(py).to_string().as_bytes().to_vec();

        // Convert Py<PyBytes> to &[u8]
        let raw_bytes = &self.content.bind(py).as_bytes();

        // Release the GIL here because decoding can be CPU-intensive
        let (decoded_str, detected_encoding_name) = py.allow_threads(move || {
            let encoding = Encoding::for_label(encoding_bytes).ok_or_else(|| {
                PyErr::new::<exceptions::PyValueError, _>(format!(
                    "Unsupported charset: {}",
                    String::from_utf8_lossy(encoding_bytes)
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
        if self.encoding.bind(py).to_string() != detected_encoding_name {
            self.encoding = PyString::new_bound(py, &detected_encoding_name).into();
        }

        Ok(decoded_str)
    }

    fn json(&mut self, py: Python) -> PyResult<PyObject> {
        // Convert Py<PyBytes> to &[u8]
        let raw_bytes = &self.content.bind(py).as_bytes();

        // Release the GIL here because JSON parsing can be CPU-intensive
        let value: Result<Value, SerdeError> =
            py.allow_threads(|| serde_json::from_slice(raw_bytes));

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
    }
}
