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
    pub resp: reqwest_impersonate::blocking::Response,
    #[pyo3(get)]
    pub encoding: String,
    pub _content_as_vec: Option<Vec<u8>>,
    pub _text: Option<String>,
}

#[pymethods]
impl Response {
    /// Returns the cookies from the response as a `HashMap<String, String>`.
    #[getter]
    fn cookies(&self) -> PyResult<HashMap<String, String>> {
        let cookies: HashMap<String, String> = self
            .resp
            .cookies()
            .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
            .collect();
        Ok(cookies)
    }

    /// Returns the headers from the response as a `HashMap<String, String>`.
    #[getter]
    fn headers(&self) -> PyResult<HashMap<String, String>> {
        let headers = self
            .resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        Ok(headers)
    }

    /// Returns the status code of the response as a `u16`.
    #[getter]
    fn status_code(&self) -> PyResult<u16> {
        let status_code = self.resp.status().as_u16();
        Ok(status_code)
    }

    #[getter]
    fn url(&self) -> PyResult<String> {
        let url = self.resp.url().to_string();
        Ok(url)
    }

    /// Stores the content of the response as a `Vec<u8>`.
    ///
    /// This method is used internally to cache the response body for future use.
    fn content_as_vec(&mut self) -> PyResult<Vec<u8>> {
        // Check if content has already been read and stored
        if self._content_as_vec.is_none() {
            let mut buf: Vec<u8> = vec![];
            self.resp.copy_to(&mut buf).map_err(|e| {
                PyErr::new::<exceptions::PyIOError, _>(format!(
                    "Error copying response body: {}",
                    e
                ))
            })?;
            // Store the content for future use
            self._content_as_vec = Some(buf);
        }
        // Return the stored content
        Ok(self._content_as_vec.as_ref().unwrap().clone())
    }

    /// Returns the content of the response as Python bytes.
    #[getter]
    fn content(&mut self) -> PyResult<PyObject> {
        let content = self.content_as_vec()?;
        Python::with_gil(|py| {
            let bytes = PyBytes::new_bound(py, &content);
            Ok(bytes.to_object(py))
        })
    }

    /// Extracts the character encoding from the "Content-Type" header, or defaults to "UTF-8".
    #[getter]
    fn charset(&mut self) -> PyResult<String> {
        let encoding_from_headers = self
            .resp
            .headers()
            .get("Content-Type")
            .and_then(|ct| ct.to_str().ok())
            .and_then(|ct| {
                ct.split(';').find_map(|param| {
                    let mut kv = param.splitn(2, '=');
                    let key = kv.next()?.trim();
                    let value = kv.next()?.trim();
                    if key.eq_ignore_ascii_case("charset") {
                        Some(value.to_string())
                    } else {
                        None
                    }
                })
            });
        // If encoding is not found in headers, default to UTF-8
        Ok(encoding_from_headers.unwrap_or_else(|| "UTF-8".to_string()))
    }

    /// Decodes the response body as text.
    #[getter]
    fn text(&mut self) -> PyResult<String> {
        if let Some(ref text) = self._text {
            // If the text is already decoded, return it
            Ok(text.clone())
        } else {
            Python::with_gil(|py| {
                // Release the GIL here because decoding can be CPU-intensive.
                py.allow_threads(|| {
                    // Otherwise, decode the content and store it
                    let content = self.content_as_vec()?;
                    let charset = self.charset()?;
                    let encoding = Encoding::for_label(charset.as_bytes()).ok_or_else(|| {
                        PyErr::new::<exceptions::PyValueError, _>(format!(
                            "Unsupported charset: {}",
                            charset
                        ))
                    })?;
                    let (decoded_str, detected_encoding, _) = encoding.decode(&content);
                    // Redefine resp.encoding if detected_encoding != charset
                    if detected_encoding != encoding {
                        self.encoding = detected_encoding.name().to_string();
                    }
                    // Store the decoded text for future use
                    self._text = Some(decoded_str.into_owned());
                    Ok(self._text.as_ref().unwrap().clone())
                })
            })
        }
    }

    /// Parses the response body as JSON and returns it as a Python object.
    fn json(&mut self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            // This call to self.text() should already correctly manage the GIL.
            let text = self.text()?;

            // Parse the text as JSON
            // We release the GIL here because JSON parsing can be CPU-intensive.
            let value: Result<Value, SerdeError> = py.allow_threads(|| serde_json::from_str(&text));

            // Handle the result of parsing
            match value {
                Ok(json_value) => {
                    // Convert the parsed JSON into a Python object using pythonize
                    match pythonize(py, &json_value) {
                        Ok(py_obj) => Ok(py_obj),
                        Err(e) => Err(PyErr::new::<exceptions::PyValueError, _>(format!(
                            "Failed to convert JSON to Python object: {}",
                            e
                        ))),
                    }
                }
                Err(e) => Err(PyErr::new::<exceptions::PyValueError, _>(format!(
                    "Failed to parse JSON: {}",
                    e
                ))),
            }
        })
    }
}
