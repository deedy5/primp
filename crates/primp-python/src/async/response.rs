use std::sync::Arc;

use anyhow::Result;
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

/// A struct representing an async HTTP response.
#[pyclass]
pub struct AsyncResponse {
    resp: Arc<TMutex<Option<::primp::Response>>>,
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
    pub fn new(resp: ::primp::Response, url: String, status_code: u16) -> Self {
        AsyncResponse {
            resp: Arc::new(TMutex::new(Some(resp))),
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
    /// Get response content as bytes (sync - blocks until content is read)
    #[getter]
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        if let Some(content) = &self._content {
            let cloned = content.clone_ref(py);
            return Ok(cloned.into_bound(py));
        }

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

        let content = PyBytes::new(py, &bytes);
        self._content = Some(content.clone().unbind());
        Ok(content)
    }

    /// Get character encoding (sync)
    #[getter]
    fn get_encoding<'rs>(&mut self, py: Python<'rs>) -> PyResult<String> {
        if let Some(encoding) = self._encoding.as_ref() {
            return Ok(encoding.clone());
        }

        let resp = Arc::clone(&self.resp);
        let encoding = py.detach(|| {
            RUNTIME.block_on(async {
                let resp_guard = resp.lock().await;
                match resp_guard.as_ref() {
                    Some(r) => {
                        Ok::<String, PyErr>(extract_encoding(r.headers()).name().to_string())
                    }
                    None => Ok("utf-8".to_string()),
                }
            })
        })?;

        self._encoding = Some(encoding.clone());
        Ok(encoding)
    }

    /// Set character encoding
    #[setter]
    fn set_encoding(&mut self, encoding: Option<String>) -> PyResult<()> {
        if let Some(encoding) = encoding {
            self._encoding = Some(encoding);
        }
        Ok(())
    }

    /// Get response text (sync - blocks until content is read)
    #[getter]
    fn text<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyString>> {
        let content = self.get_content(py)?.unbind();
        let encoding = self.get_encoding(py)?;
        let raw_bytes = content.as_bytes(py);
        let text = py.detach(|| {
            let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(raw_bytes);
            text
        });
        text.into_pyobject_or_pyerr(py)
    }

    /// Get response headers (sync)
    #[getter]
    fn get_headers<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        if let Some(headers) = &self._headers {
            return Ok(headers.clone_ref(py).into_bound(py));
        }

        let resp = Arc::clone(&self.resp);
        let headers_map = py.detach(|| {
            RUNTIME.block_on(async {
                let resp_guard = resp.lock().await;
                match resp_guard.as_ref() {
                    Some(r) => {
                        let mut headers = std::collections::HashMap::new();
                        for (key, value) in r.headers() {
                            headers
                                .insert(key.to_string(), value.to_str().unwrap_or("").to_string());
                        }
                        Ok::<Option<std::collections::HashMap<String, String>>, PyErr>(Some(
                            headers,
                        ))
                    }
                    None => Ok(None),
                }
            })
        })?;

        let new_headers = PyDict::new(py);
        if let Some(headers) = headers_map {
            for (key, value) in headers {
                new_headers.set_item(key, value)?;
            }
        }
        self._headers = Some(new_headers.clone().unbind());
        Ok(new_headers)
    }

    /// Get response cookies (sync)
    #[getter]
    fn get_cookies<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        if let Some(cookies) = &self._cookies {
            return Ok(cookies.clone_ref(py).into_bound(py));
        }

        let resp = Arc::clone(&self.resp);
        let cookies_map = py.detach(|| {
            RUNTIME.block_on(async {
                let resp_guard = resp.lock().await;
                match resp_guard.as_ref() {
                    Some(r) => {
                        let mut cookies = std::collections::HashMap::new();
                        let set_cookie_header = r.headers().get_all(::primp::header::SET_COOKIE);
                        for cookie_header in set_cookie_header.iter() {
                            if let Ok(cookie_str) = cookie_header.to_str() {
                                if let Some((name, value)) = cookie_str.split_once('=') {
                                    cookies.insert(
                                        name.trim().to_string(),
                                        value.split(';').next().unwrap_or("").trim().to_string(),
                                    );
                                }
                            }
                        }
                        Ok::<Option<std::collections::HashMap<String, String>>, PyErr>(Some(
                            cookies,
                        ))
                    }
                    None => Ok(None),
                }
            })
        })?;

        let new_cookies = PyDict::new(py);
        if let Some(cookies) = cookies_map {
            for (key, value) in cookies {
                new_cookies.set_item(key, value)?;
            }
        }
        self._cookies = Some(new_cookies.clone().unbind());
        Ok(new_cookies)
    }

    /// Get HTML converted to Markdown (sync)
    #[getter]
    fn text_markdown<'rs>(&mut self, py: Python<'rs>) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text = py.detach(|| from_read(raw_bytes, 100))?;
        Ok(text)
    }

    /// Get HTML converted to plain text (sync)
    #[getter]
    fn text_plain<'rs>(&mut self, py: Python<'rs>) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text =
            py.detach(|| from_read_with_decorator(raw_bytes, 100, TrivialDecorator::new()))?;
        Ok(text)
    }

    /// Get HTML converted to rich text (sync)
    #[getter]
    fn text_rich<'rs>(&mut self, py: Python<'rs>) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text = py.detach(|| from_read_with_decorator(raw_bytes, 100, RichDecorator::new()))?;
        Ok(text)
    }

    /// Parse response body as JSON (sync)
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

    /// Raise HTTPError for 4xx/5xx status codes (sync)
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

/// A streaming async HTTP response that supports async iteration over response chunks.
///
/// This is returned by `primp.async_stream()` or when using `stream=True` parameter with async client.
/// It supports async context manager protocol for automatic resource cleanup.
#[pyclass]
pub struct AsyncStreamResponse {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    encoding: String,
    #[pyo3(get)]
    url: String,
    #[pyo3(get)]
    status_code: u16,
    headers: Py<PyDict>,
    cookies: Py<PyDict>,
}

impl AsyncStreamResponse {
    pub fn new(
        resp: ::primp::Response,
        url: String,
        status_code: u16,
        encoding: String,
        headers: Py<PyDict>,
        cookies: Py<PyDict>,
    ) -> Self {
        AsyncStreamResponse {
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
impl AsyncStreamResponse {
    /// Get response headers (sync)
    #[getter]
    fn get_headers<'rs>(&self, py: Python<'rs>) -> Bound<'rs, PyDict> {
        self.headers.clone_ref(py).into_bound(py)
    }

    /// Get response cookies (sync)
    #[getter]
    fn get_cookies<'rs>(&self, py: Python<'rs>) -> Bound<'rs, PyDict> {
        self.cookies.clone_ref(py).into_bound(py)
    }

    /// Get character encoding (sync)
    #[getter]
    fn get_encoding(&self) -> String {
        self.encoding.clone()
    }

    /// Set character encoding (sync)
    #[setter]
    fn set_encoding(&mut self, encoding: String) {
        self.encoding = encoding;
    }

    /// Read remaining content into memory (async)
    fn aread<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
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
        };

        future_into_py(py, future)
    }

    /// Get response content as bytes (async - reads all remaining content)
    #[getter]
    fn get_content<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.aread(py)
    }

    /// Get response text (async - reads all remaining content)
    #[getter]
    fn text<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let encoding = self.encoding.clone();
        let future = async move {
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
            let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(&all_chunks);
            Ok::<String, PyErr>(text.to_string())
        };

        future_into_py(py, future)
    }

    /// Return an async iterator over byte chunks
    #[pyo3(signature = (chunk_size=None))]
    fn aiter_bytes(&self, chunk_size: Option<usize>) -> PyResult<AsyncBytesIterator> {
        Ok(AsyncBytesIterator::new(
            Arc::clone(&self.resp),
            chunk_size.unwrap_or(8192),
        ))
    }

    /// Return an async iterator over text chunks
    #[pyo3(signature = (chunk_size=None))]
    fn aiter_text(&self, chunk_size: Option<usize>) -> PyResult<AsyncTextIterator> {
        Ok(AsyncTextIterator::new(
            Arc::clone(&self.resp),
            self.encoding.clone(),
            chunk_size.unwrap_or(8192),
        ))
    }

    /// Return an async iterator over lines
    fn aiter_lines(&self) -> PyResult<AsyncLinesIterator> {
        Ok(AsyncLinesIterator::new(Arc::clone(&self.resp)))
    }

    /// Get next chunk explicitly (async)
    fn anext<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            match resp_guard.as_mut() {
                Some(r) => match r.chunk().await {
                    Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                    Ok(None) => Ok(None),
                    Err(e) => Err(convert_reqwest_error(e)),
                },
                None => Ok(None),
            }
        };

        future_into_py(py, future)
    }

    /// Close response and release resources (async)
    fn aclose<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            resp_guard.take();
            Ok::<(), PyErr>(())
        };

        future_into_py(py, future)
    }

    /// Raise HTTPError for 4xx/5xx status codes (sync)
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

    /// Async context manager entry
    fn __aenter__<'py>(slf: PyRef<'_, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        // Get the Py<Self> from the PyRef's cell
        let slf_py: Py<AsyncStreamResponse> = Py::from(slf);
        let future = async move { Ok::<Py<AsyncStreamResponse>, PyErr>(slf_py) };
        future_into_py(py, future)
    }

    /// Async context manager exit
    fn __aexit__<'py>(
        &mut self,
        py: Python<'py>,
        _exc_type: Option<Bound<'py, PyAny>>,
        _exc_value: Option<Bound<'py, PyAny>>,
        _traceback: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            resp_guard.take();
            Ok::<bool, PyErr>(false)
        };

        future_into_py(py, future)
    }
}

/// Async iterator over byte chunks from a streaming response.
#[pyclass]
pub struct AsyncBytesIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    chunk_size: usize,
    buffer: Vec<u8>,
}

impl AsyncBytesIterator {
    fn new(resp: Arc<TMutex<Option<::primp::Response>>>, chunk_size: usize) -> Self {
        AsyncBytesIterator {
            resp,
            chunk_size,
            buffer: Vec::new(),
        }
    }
}

#[pymethods]
impl AsyncBytesIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let chunk_size = self.chunk_size;
        let buffer = std::mem::take(&mut self.buffer);

        let future = async move {
            let mut buffer = buffer;

            // If we have buffered data, return it
            if buffer.len() >= chunk_size {
                let chunk: Vec<u8> = buffer.drain(..chunk_size).collect();
                return Ok::<Vec<u8>, PyErr>(chunk);
            }

            // Need to fetch more data
            let mut resp_guard = resp.lock().await;
            match resp_guard.as_mut() {
                Some(r) => match r.chunk().await {
                    Ok(Some(data)) => {
                        buffer.extend_from_slice(&data);
                        if buffer.len() >= chunk_size {
                            let result: Vec<u8> = buffer.drain(..chunk_size).collect();
                            Ok(result)
                        } else if !buffer.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut buffer);
                            Ok(result)
                        } else {
                            // Received empty chunk, stream is exhausted
                            Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                                "Stream exhausted",
                            ))
                        }
                    }
                    Ok(None) => {
                        if !buffer.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut buffer);
                            Ok(result)
                        } else {
                            Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                                "Stream exhausted",
                            ))
                        }
                    }
                    Err(e) => Err(convert_reqwest_error(e)),
                },
                None => {
                    if !buffer.is_empty() {
                        let result: Vec<u8> = std::mem::take(&mut buffer);
                        Ok(result)
                    } else {
                        Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                            "Stream exhausted",
                        ))
                    }
                }
            }
        };

        future_into_py(py, future)
    }
}

/// Async iterator over text chunks from a streaming response.
#[pyclass]
pub struct AsyncTextIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    encoding: String,
    chunk_size: usize,
    buffer: Vec<u8>,
}

impl AsyncTextIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        encoding: String,
        chunk_size: usize,
    ) -> Self {
        AsyncTextIterator {
            resp,
            encoding,
            chunk_size,
            buffer: Vec::new(),
        }
    }
}

#[pymethods]
impl AsyncTextIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let encoding = self.encoding.clone();
        let chunk_size = self.chunk_size;
        let buffer = std::mem::take(&mut self.buffer);

        let future = async move {
            let mut buffer = buffer;

            // If we have buffered data, return it
            if buffer.len() >= chunk_size {
                let chunk: Vec<u8> = buffer.drain(..chunk_size).collect();
                let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                let (text, _, _) = enc.decode(&chunk);
                return Ok::<String, PyErr>(text.to_string());
            }

            // Need to fetch more data
            let mut resp_guard = resp.lock().await;
            match resp_guard.as_mut() {
                Some(r) => match r.chunk().await {
                    Ok(Some(data)) => {
                        buffer.extend_from_slice(&data);
                        if buffer.len() >= chunk_size {
                            let result: Vec<u8> = buffer.drain(..chunk_size).collect();
                            let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                            let (text, _, _) = enc.decode(&result);
                            Ok(text.to_string())
                        } else if !buffer.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut buffer);
                            let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                            let (text, _, _) = enc.decode(&result);
                            Ok(text.to_string())
                        } else {
                            Ok(String::new())
                        }
                    }
                    Ok(None) => {
                        if !buffer.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut buffer);
                            let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                            let (text, _, _) = enc.decode(&result);
                            Ok(text.to_string())
                        } else {
                            Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                                "Stream exhausted",
                            ))
                        }
                    }
                    Err(e) => Err(convert_reqwest_error(e)),
                },
                None => {
                    if !buffer.is_empty() {
                        let result: Vec<u8> = std::mem::take(&mut buffer);
                        let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                        let (text, _, _) = enc.decode(&result);
                        Ok(text.to_string())
                    } else {
                        Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                            "Stream exhausted",
                        ))
                    }
                }
            }
        };

        future_into_py(py, future)
    }
}

/// Async iterator over lines from a streaming response.
#[pyclass]
pub struct AsyncLinesIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    buffer: String,
    done: bool,
}

impl AsyncLinesIterator {
    fn new(resp: Arc<TMutex<Option<::primp::Response>>>) -> Self {
        AsyncLinesIterator {
            resp,
            buffer: String::new(),
            done: false,
        }
    }
}

#[pymethods]
impl AsyncLinesIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let buffer = std::mem::take(&mut self.buffer);
        let done = self.done;

        let future = async move {
            let mut buffer = buffer;
            let mut done = done;

            loop {
                // Check if we have a complete line in buffer
                if let Some(newline_pos) = buffer.find('\n') {
                    let line: String = buffer.drain(..=newline_pos).collect();
                    let line = line.trim_end_matches('\r').trim_end_matches('\n');
                    return Ok::<String, PyErr>(line.to_string());
                }

                if done {
                    if !buffer.is_empty() {
                        let line = std::mem::take(&mut buffer);
                        return Ok(line);
                    }
                    return Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                        "Stream exhausted",
                    ));
                }

                // Fetch more data
                let mut resp_guard = resp.lock().await;
                match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => {
                            let text = String::from_utf8_lossy(&data);
                            buffer.push_str(&text);
                        }
                        Ok(None) => {
                            done = true;
                        }
                        Err(e) => return Err(convert_reqwest_error(e)),
                    },
                    None => {
                        done = true;
                    }
                }
            }
        };

        future_into_py(py, future)
    }
}
