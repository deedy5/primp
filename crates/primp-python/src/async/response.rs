use std::sync::Arc;

use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    PyErr,
};
use pythonize::pythonize;
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::error::convert_reqwest_error;
use crate::response_shared;

/// A struct representing an async HTTP response.
///
/// Supports both buffered (non-streaming) and streaming modes.
/// In buffered mode, body content is cached after first read.
/// In streaming mode, body is consumed on each read and supports iteration.
#[pyclass]
pub struct AsyncResponse {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    _content: Option<Py<PyBytes>>,
    _encoding: Option<String>,
    _headers: Option<IndexMapSSR>,
    _cookies: Option<IndexMapSSR>,
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub status_code: u16,
    streaming: bool,
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
            streaming: false,
        }
    }

    pub fn new_streaming(
        resp: ::primp::Response,
        url: String,
        status_code: u16,
        encoding: String,
        headers: IndexMapSSR,
        cookies: IndexMapSSR,
    ) -> Self {
        AsyncResponse {
            resp: Arc::new(TMutex::new(Some(resp))),
            _content: None,
            _encoding: Some(encoding),
            _headers: Some(headers),
            _cookies: Some(cookies),
            url,
            status_code,
            streaming: true,
        }
    }
}

#[pymethods]
impl AsyncResponse {
    /// Get response content as bytes (sync - blocks until content is read)
    #[getter]
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        response_shared::get_content(&self.resp, &mut self._content, self.streaming, py)
    }

    /// Get character encoding (sync)
    #[getter]
    fn get_encoding(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::get_encoding(&self.resp, &mut self._encoding, py)
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
        response_shared::text(
            &self.resp,
            &mut self._content,
            &mut self._encoding,
            self.streaming,
            py,
        )
    }

    /// Get response headers (sync)
    #[getter]
    fn get_headers<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        response_shared::get_headers(&self.resp, &mut self._headers, py)
    }

    /// Get response cookies (sync)
    #[getter]
    fn get_cookies<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        response_shared::get_cookies(&self.resp, &mut self._cookies, py)
    }

    /// Get HTML converted to Markdown (sync)
    #[getter]
    fn text_markdown(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::text_markdown(&self.resp, &mut self._content, self.streaming, py)
    }

    /// Get HTML converted to plain text (sync)
    #[getter]
    fn text_plain(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::text_plain(&self.resp, &mut self._content, self.streaming, py)
    }

    /// Get HTML converted to rich text (sync)
    #[getter]
    fn text_rich(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::text_rich(&self.resp, &mut self._content, self.streaming, py)
    }

    /// Parse response body as JSON (sync)
    fn json<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyAny>> {
        response_shared::json(&self.resp, &mut self._content, self.streaming, py)
    }

    /// Raise HTTPError for 4xx/5xx status codes (sync)
    fn raise_for_status(&self) -> PyResult<()> {
        response_shared::raise_for_status(self.status_code, &self.url)
    }

    /// Read remaining content into memory (async)
    fn aread<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;
        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            let bytes = match resp_guard.as_mut() {
                Some(r) => match response_shared::collect_body_bytes(r).await {
                    Ok(buf) => buf,
                    Err(e) => return Err(e),
                },
                None => Bytes::new(),
            };
            Ok::<Vec<u8>, PyErr>(bytes.to_vec())
        };
        future_into_py(py, future)
    }

    /// Get response content as bytes (async - reads all remaining content)
    #[getter]
    fn get_content_async<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.aread(py)
    }

    /// Get response text (async - reads all remaining content)
    #[getter]
    fn text_async<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let encoding = self
            ._encoding
            .clone()
            .unwrap_or_else(|| "utf-8".to_string());
        let future = async move {
            let mut resp_guard = resp.lock().await;
            let bytes = match resp_guard.as_mut() {
                Some(r) => match response_shared::collect_body_bytes(r).await {
                    Ok(buf) => buf,
                    Err(e) => return Err(e),
                },
                None => Bytes::new(),
            };
            let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(&bytes);
            Ok::<String, PyErr>(text.to_string())
        };

        future_into_py(py, future)
    }

    /// Parse response content as JSON (async - reads all remaining content)
    fn json_async<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3::exceptions::PyValueError;
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let future = future_into_py(py, async move {
            let mut resp_guard = resp.lock().await;
            let bytes = match resp_guard.as_mut() {
                Some(r) => match response_shared::collect_body_bytes(r).await {
                    Ok(buf) => buf,
                    Err(e) => return Err(e),
                },
                None => Bytes::new(),
            };
            let json_value: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    let doc = String::from_utf8_lossy(&bytes).to_string();
                    let msg = e.to_string();
                    let pos = e.line().saturating_sub(1);
                    return Err(Python::attach(|py| {
                        let json_module = py.import("json")?;
                        let error_type = json_module.getattr("JSONDecodeError")?;
                        let py_err = error_type.call1((&msg, &doc, pos))?;
                        Ok::<_, PyErr>(PyErr::from_value(py_err))
                    })?);
                }
            };
            let py_obj: Py<PyAny> = Python::attach(|py| {
                let bound = pythonize(py, &json_value)
                    .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
                Ok::<_, PyErr>(bound.unbind())
            })?;
            Ok(py_obj)
        })?;

        Ok(future)
    }

    #[pyo3(signature = (chunk_size=None))]
    fn aiter_bytes(&self, chunk_size: Option<usize>) -> PyResult<AsyncBytesIterator> {
        let resp = Arc::clone(&self.resp);
        Ok(AsyncBytesIterator::new(resp, chunk_size.unwrap_or(8192)))
    }

    #[pyo3(signature = (chunk_size=None))]
    fn aiter_text(&self, chunk_size: Option<usize>) -> PyResult<AsyncTextIterator> {
        let resp = Arc::clone(&self.resp);
        let encoding = self
            ._encoding
            .clone()
            .unwrap_or_else(|| "utf-8".to_string());
        Ok(AsyncTextIterator::new(
            resp,
            encoding,
            chunk_size.unwrap_or(8192),
        ))
    }

    fn aiter_lines(&self) -> PyResult<AsyncLinesIterator> {
        let resp = Arc::clone(&self.resp);
        Ok(AsyncLinesIterator::new(resp))
    }

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

    /// Async context manager entry
    fn __aenter__<'py>(slf: PyRef<'_, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

        let slf_py: Py<AsyncResponse> = Py::from(slf);
        let future = async move { Ok::<Py<AsyncResponse>, PyErr>(slf_py) };
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
    buffer: Arc<TMutex<Vec<u8>>>,
}

impl AsyncBytesIterator {
    fn new(resp: Arc<TMutex<Option<::primp::Response>>>, chunk_size: usize) -> Self {
        AsyncBytesIterator {
            resp,
            chunk_size,
            buffer: Arc::new(TMutex::new(Vec::new())),
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
        let buffer = Arc::clone(&self.buffer);

        let future = async move {
            {
                let mut buf = buffer.lock().await;
                if buf.len() >= chunk_size {
                    let chunk: Vec<u8> = buf.drain(..chunk_size).collect();
                    return Ok::<Vec<u8>, PyErr>(chunk);
                }
            }

            let mut resp_guard = resp.lock().await;
            match resp_guard.as_mut() {
                Some(r) => match r.chunk().await {
                    Ok(Some(data)) => {
                        let mut buf = buffer.lock().await;
                        buf.extend_from_slice(&data);
                        if buf.len() >= chunk_size {
                            let result: Vec<u8> = buf.drain(..chunk_size).collect();
                            Ok(result)
                        } else if !buf.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut *buf);
                            Ok(result)
                        } else {
                            Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                                "Stream exhausted",
                            ))
                        }
                    }
                    Ok(None) => {
                        let mut buf = buffer.lock().await;
                        if !buf.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut *buf);
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
                    let mut buf = buffer.lock().await;
                    if !buf.is_empty() {
                        let result: Vec<u8> = std::mem::take(&mut *buf);
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
    buffer: Arc<TMutex<Vec<u8>>>,
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
            buffer: Arc::new(TMutex::new(Vec::new())),
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
        let buffer = Arc::clone(&self.buffer);

        let future = async move {
            {
                let mut buf = buffer.lock().await;
                if buf.len() >= chunk_size {
                    let chunk: Vec<u8> = buf.drain(..chunk_size).collect();
                    let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                    let (text, _, _) = enc.decode(&chunk);
                    return Ok::<String, PyErr>(text.to_string());
                }
            }

            let mut resp_guard = resp.lock().await;
            match resp_guard.as_mut() {
                Some(r) => match r.chunk().await {
                    Ok(Some(data)) => {
                        let mut buf = buffer.lock().await;
                        buf.extend_from_slice(&data);
                        if buf.len() >= chunk_size {
                            let result: Vec<u8> = buf.drain(..chunk_size).collect();
                            let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                            let (text, _, _) = enc.decode(&result);
                            Ok(text.to_string())
                        } else if !buf.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut *buf);
                            let enc = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
                            let (text, _, _) = enc.decode(&result);
                            Ok(text.to_string())
                        } else {
                            Ok(String::new())
                        }
                    }
                    Ok(None) => {
                        let mut buf = buffer.lock().await;
                        if !buf.is_empty() {
                            let result: Vec<u8> = std::mem::take(&mut *buf);
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
                    let mut buf = buffer.lock().await;
                    if !buf.is_empty() {
                        let result: Vec<u8> = std::mem::take(&mut *buf);
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
    buffer: Arc<TMutex<String>>,
    done: Arc<TMutex<bool>>,
}

impl AsyncLinesIterator {
    fn new(resp: Arc<TMutex<Option<::primp::Response>>>) -> Self {
        AsyncLinesIterator {
            resp,
            buffer: Arc::new(TMutex::new(String::new())),
            done: Arc::new(TMutex::new(false)),
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
        let buffer = Arc::clone(&self.buffer);
        let done = Arc::clone(&self.done);

        let future = async move {
            loop {
                {
                    let mut buf = buffer.lock().await;
                    if let Some(newline_pos) = buf.find('\n') {
                        let line: String = buf.drain(..=newline_pos).collect();
                        let line = line.trim_end_matches('\r').trim_end_matches('\n');
                        return Ok::<String, PyErr>(line.to_string());
                    }
                }

                {
                    let is_done = *done.lock().await;
                    if is_done {
                        let mut buf = buffer.lock().await;
                        if !buf.is_empty() {
                            let line = std::mem::take(&mut *buf);
                            return Ok(line);
                        }
                        return Err(PyErr::new::<pyo3::exceptions::PyStopAsyncIteration, _>(
                            "Stream exhausted",
                        ));
                    }
                }

                let mut resp_guard = resp.lock().await;
                match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => {
                            let mut buf = buffer.lock().await;
                            let text = String::from_utf8_lossy(&data);
                            buf.push_str(&text);
                        }
                        Ok(None) => {
                            let mut is_done = done.lock().await;
                            *is_done = true;
                        }
                        Err(e) => return Err(convert_reqwest_error(e)),
                    },
                    None => {
                        let mut is_done = done.lock().await;
                        *is_done = true;
                    }
                }
            }
        };

        future_into_py(py, future)
    }
}
