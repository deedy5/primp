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

use crate::error::{convert_reqwest_error, PrimpErrorEnum};
use crate::RUNTIME;

/// Extract encoding from Content-Type header.
///
/// Returns the encoding specified in the charset parameter, or UTF-8 as fallback.
fn extract_encoding(headers: &::primp::header::HeaderMap) -> &'static Encoding {
    headers
        .get(::primp::header::CONTENT_TYPE)
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
    pub resp: Option<::primp::Response>,
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
        let bytes: Bytes = py.detach(|| {
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
                        Err(e) => {
                            return Err(convert_reqwest_error(e));
                        }
                    }
                }
                Ok(Bytes::from(all_chunks))
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
        let encoding = py.detach(|| {
            let resp = self
                .resp
                .as_ref()
                .expect("Response was moved for streaming");
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
        let json_value: serde_json::Value = match from_slice(raw_bytes) {
            Ok(v) => v,
            Err(e) => {
                let json_module = py.import("json")?;
                let error_type = json_module.getattr("JSONDecodeError")?;
                let doc = String::from_utf8_lossy(raw_bytes).to_string();
                let msg = e.to_string();
                let pos = e.line().saturating_sub(1);
                return Err(PyErr::from_value(error_type.call1((&msg, &doc, pos))?));
            }
        };
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
            .get_all(::primp::header::SET_COOKIE);
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

            return Err(PyErr::from(PrimpErrorEnum::HttpStatus(
                self.status_code,
                reason.to_string(),
                self.url.clone(),
            )));
        }
        Ok(())
    }
}

/// A streaming HTTP response that supports iteration over response chunks.
///
/// This is returned by `primp.stream()` or when using `stream=True` parameter.
/// It supports context manager protocol for automatic resource cleanup.
#[pyclass]
pub struct StreamResponse {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    encoding: String,
    #[pyo3(get)]
    url: String,
    #[pyo3(get)]
    status_code: u16,
    headers: Py<PyDict>,
    cookies: Py<PyDict>,
}

impl StreamResponse {
    pub fn new(
        resp: ::primp::Response,
        url: String,
        status_code: u16,
        encoding: String,
        headers: Py<PyDict>,
        cookies: Py<PyDict>,
    ) -> Self {
        StreamResponse {
            resp: Arc::new(TMutex::new(Some(resp))),
            encoding,
            url,
            status_code,
            headers,
            cookies,
        }
    }
}

#[pymethods]
impl StreamResponse {
    /// Get response headers
    #[getter]
    fn get_headers<'rs>(&self, py: Python<'rs>) -> Bound<'rs, PyDict> {
        self.headers.clone_ref(py).into_bound(py)
    }

    /// Get response cookies
    #[getter]
    fn get_cookies<'rs>(&self, py: Python<'rs>) -> Bound<'rs, PyDict> {
        self.cookies.clone_ref(py).into_bound(py)
    }

    /// Get character encoding
    #[getter]
    fn get_encoding(&self) -> String {
        self.encoding.clone()
    }

    /// Set character encoding
    #[setter]
    fn set_encoding(&mut self, encoding: String) {
        self.encoding = encoding;
    }

    /// Read remaining content into memory and return as bytes
    fn read<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        let resp = Arc::clone(&self.resp);
        let bytes = py.detach(|| {
            RUNTIME.block_on(async {
                let mut all_chunks = Vec::with_capacity(8 * 1024);
                loop {
                    let mut resp_guard = resp.lock().await;
                    match resp_guard.as_mut() {
                        Some(r) => match r.chunk().await {
                            Ok(Some(chunk)) => all_chunks.extend_from_slice(&chunk),
                            Ok(None) => break,
                            Err(e) => return Err(convert_reqwest_error(e)),
                        },
                        None => break,
                    }
                }
                Ok::<Vec<u8>, PyErr>(all_chunks)
            })
        })?;

        Ok(PyBytes::new(py, &bytes))
    }

    /// Get response content as bytes (reads all remaining content)
    #[getter]
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        self.read(py)
    }

    /// Get response text (reads all remaining content)
    #[getter]
    fn text<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyString>> {
        let content = self.read(py)?;
        let raw_bytes = content.as_bytes();
        let encoding = Encoding::for_label(self.encoding.as_bytes()).unwrap_or(UTF_8);
        let (text, _, _) = encoding.decode(raw_bytes);
        text.into_pyobject_or_pyerr(py)
    }

    /// Return an iterator over byte chunks
    #[pyo3(signature = (chunk_size=None))]
    fn iter_bytes(&self, chunk_size: Option<usize>) -> PyResult<BytesIterator> {
        Ok(BytesIterator::new(
            Arc::clone(&self.resp),
            chunk_size.unwrap_or(8192),
        ))
    }

    /// Return an iterator over text chunks
    #[pyo3(signature = (chunk_size=None))]
    fn iter_text(&self, chunk_size: Option<usize>) -> PyResult<TextIterator> {
        Ok(TextIterator::new(
            Arc::clone(&self.resp),
            self.encoding.clone(),
            chunk_size.unwrap_or(8192),
        ))
    }

    /// Return an iterator over lines
    fn iter_lines(&self) -> PyResult<LinesIterator> {
        Ok(LinesIterator::new(Arc::clone(&self.resp)))
    }

    /// Get next chunk explicitly (returns bytes)
    fn next<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        let resp = Arc::clone(&self.resp);
        let chunk = py.detach(|| {
            RUNTIME.block_on(async {
                let mut resp_guard = resp.lock().await;
                match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                        Ok(None) => Ok(None),
                        Err(e) => Err(convert_reqwest_error(e)),
                    },
                    None => Ok(None),
                }
            })
        })?;

        match chunk {
            Some(data) => Ok(Some(PyBytes::new(py, &data))),
            None => Ok(None),
        }
    }

    /// Close response and release resources
    fn close<'rs>(&mut self, py: Python<'rs>) -> PyResult<()> {
        let resp = Arc::clone(&self.resp);
        py.detach(|| {
            RUNTIME.block_on(async {
                let mut resp_guard = resp.lock().await;
                resp_guard.take();
            })
        });
        Ok(())
    }

    /// Raise HTTPError for 4xx/5xx status codes
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

            return Err(PyErr::from(PrimpErrorEnum::HttpStatus(
                self.status_code,
                reason.to_string(),
                self.url.clone(),
            )));
        }
        Ok(())
    }

    /// Context manager entry
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager exit
    fn __exit__<'rs>(
        &mut self,
        _exc_type: Option<Bound<'rs, PyAny>>,
        _exc_value: Option<Bound<'rs, PyAny>>,
        _traceback: Option<Bound<'rs, PyAny>>,
        py: Python<'rs>,
    ) -> PyResult<bool> {
        self.close(py)?;
        Ok(false)
    }
}

/// Iterator over byte chunks from a streaming response.
#[pyclass]
pub struct BytesIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    chunk_size: usize,
    buffer: Vec<u8>,
}

impl BytesIterator {
    fn new(resp: Arc<TMutex<Option<::primp::Response>>>, chunk_size: usize) -> Self {
        BytesIterator {
            resp,
            chunk_size,
            buffer: Vec::new(),
        }
    }
}

#[pymethods]
impl BytesIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        // If we have buffered data, return it
        if self.buffer.len() >= self.chunk_size {
            let chunk: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
            return Ok(Some(PyBytes::new(py, &chunk)));
        }

        // Need to fetch more data
        let resp = Arc::clone(&self.resp);
        let chunk = py.detach(|| {
            RUNTIME.block_on(async {
                let mut resp_guard = resp.lock().await;
                match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                        Ok(None) => Ok(None),
                        Err(e) => Err(convert_reqwest_error(e)),
                    },
                    None => Ok(None),
                }
            })
        })?;

        match chunk {
            Some(data) => {
                self.buffer.extend_from_slice(&data);
                if self.buffer.len() >= self.chunk_size {
                    let result: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
                    Ok(Some(PyBytes::new(py, &result)))
                } else if !self.buffer.is_empty() {
                    let result: Vec<u8> = std::mem::take(&mut self.buffer);
                    Ok(Some(PyBytes::new(py, &result)))
                } else {
                    Ok(None)
                }
            }
            None => {
                if !self.buffer.is_empty() {
                    let result: Vec<u8> = std::mem::take(&mut self.buffer);
                    Ok(Some(PyBytes::new(py, &result)))
                } else {
                    Err(pyo3::exceptions::PyStopIteration::new_err(
                        "Stream exhausted",
                    ))
                }
            }
        }
    }
}

/// Iterator over text chunks from a streaming response.
#[pyclass]
pub struct TextIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    encoding: String,
    chunk_size: usize,
    buffer: Vec<u8>,
}

impl TextIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        encoding: String,
        chunk_size: usize,
    ) -> Self {
        TextIterator {
            resp,
            encoding,
            chunk_size,
            buffer: Vec::new(),
        }
    }
}

#[pymethods]
impl TextIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyString>>> {
        // If we have buffered data, return it
        if self.buffer.len() >= self.chunk_size {
            let chunk: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
            let encoding = Encoding::for_label(self.encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(&chunk);
            return Ok(Some(text.into_pyobject_or_pyerr(py)?));
        }

        // Need to fetch more data
        let resp = Arc::clone(&self.resp);
        let chunk = py.detach(|| {
            RUNTIME.block_on(async {
                let mut resp_guard = resp.lock().await;
                match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                        Ok(None) => Ok(None),
                        Err(e) => Err(convert_reqwest_error(e)),
                    },
                    None => Ok(None),
                }
            })
        })?;

        match chunk {
            Some(data) => {
                self.buffer.extend_from_slice(&data);
                if self.buffer.len() >= self.chunk_size {
                    let result: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
                    let encoding = Encoding::for_label(self.encoding.as_bytes()).unwrap_or(UTF_8);
                    let (text, _, _) = encoding.decode(&result);
                    Ok(Some(text.into_pyobject_or_pyerr(py)?))
                } else if !self.buffer.is_empty() {
                    let result: Vec<u8> = std::mem::take(&mut self.buffer);
                    let encoding = Encoding::for_label(self.encoding.as_bytes()).unwrap_or(UTF_8);
                    let (text, _, _) = encoding.decode(&result);
                    Ok(Some(text.into_pyobject_or_pyerr(py)?))
                } else {
                    Ok(None)
                }
            }
            None => {
                if !self.buffer.is_empty() {
                    let result: Vec<u8> = std::mem::take(&mut self.buffer);
                    let encoding = Encoding::for_label(self.encoding.as_bytes()).unwrap_or(UTF_8);
                    let (text, _, _) = encoding.decode(&result);
                    Ok(Some(text.into_pyobject_or_pyerr(py)?))
                } else {
                    Err(pyo3::exceptions::PyStopIteration::new_err(
                        "Stream exhausted",
                    ))
                }
            }
        }
    }
}

/// Iterator over lines from a streaming response.
#[pyclass]
pub struct LinesIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    buffer: String,
    done: bool,
}

impl LinesIterator {
    fn new(resp: Arc<TMutex<Option<::primp::Response>>>) -> Self {
        LinesIterator {
            resp,
            buffer: String::new(),
            done: false,
        }
    }
}

#[pymethods]
impl LinesIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyString>>> {
        loop {
            // Check if we have a complete line in buffer
            if let Some(newline_pos) = self.buffer.find('\n') {
                let line: String = self.buffer.drain(..=newline_pos).collect();
                let line = line.trim_end_matches('\r').trim_end_matches('\n');
                return Ok(Some(line.to_string().into_pyobject_or_pyerr(py)?));
            }

            if self.done {
                if !self.buffer.is_empty() {
                    let line = std::mem::take(&mut self.buffer);
                    return Ok(Some(line.into_pyobject_or_pyerr(py)?));
                }
                return Err(pyo3::exceptions::PyStopIteration::new_err(
                    "Stream exhausted",
                ));
            }

            // Fetch more data
            let resp = Arc::clone(&self.resp);
            let chunk = py.detach(|| {
                RUNTIME.block_on(async {
                    let mut resp_guard = resp.lock().await;
                    match resp_guard.as_mut() {
                        Some(r) => match r.chunk().await {
                            Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                            Ok(None) => Ok(None),
                            Err(e) => Err(convert_reqwest_error(e)),
                        },
                        None => Ok(None),
                    }
                })
            })?;

            match chunk {
                Some(data) => {
                    let text = String::from_utf8_lossy(&data);
                    self.buffer.push_str(&text);
                }
                None => {
                    self.done = true;
                }
            }
        }
    }
}
