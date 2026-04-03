use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use html2text::{
    from_read, from_read_with_decorator,
    render::{RichDecorator, TrivialDecorator},
};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    IntoPyObjectExt, PyErr,
};
use pythonize::pythonize;
use serde_json::from_slice;
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::error::{body_collection_error, convert_reqwest_error, BodyError, PrimpErrorEnum};
use crate::traits::HeadersTraits;
use crate::utils::extract_encoding;
use crate::RUNTIME;

/// Collect body bytes from a response using a pre-allocated buffer.
/// Uses `Vec::with_capacity(8 * 1024)` for efficient buffer allocation.
async fn collect_body_bytes(resp: &mut ::primp::Response) -> Result<Bytes, PyErr> {
    let mut buf = Vec::with_capacity(8 * 1024);
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => buf.extend_from_slice(&chunk),
            Ok(None) => break Ok(Bytes::from(buf)),
            Err(e) => return Err(body_collection_error(&e.to_string())),
        }
    }
}

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
        if !self.streaming {
            if let Some(content) = &self._content {
                let cloned = content.clone_ref(py);
                return Ok(cloned.into_bound(py));
            }
        }

        let resp = Arc::clone(&self.resp);
        let bytes: Bytes = py.detach(|| {
            RUNTIME.block_on(async {
                let mut resp_guard = resp.lock().await;
                match resp_guard.as_mut() {
                    Some(r) => collect_body_bytes(r).await,
                    None => Err(BodyError::new_err(
                        "Response body already consumed or moved",
                    )),
                }
            })
        })?;

        let content = PyBytes::new(py, &bytes);

        if !self.streaming {
            self._content = Some(content.clone().unbind());
        }
        Ok(content)
    }

    /// Get character encoding (sync)
    #[getter]
    fn get_encoding(&mut self, _py: Python<'_>) -> PyResult<String> {
        if let Some(encoding) = self._encoding.as_ref() {
            return Ok(encoding.clone());
        }

        let resp = Arc::clone(&self.resp);
        let encoding = RUNTIME.block_on(async {
            let resp_guard = resp.lock().await;
            match resp_guard.as_ref() {
                Some(r) => Ok::<String, PyErr>(extract_encoding(r.headers()).name().to_string()),
                None => Err(BodyError::new_err(
                    "Response body already consumed or moved",
                )),
            }
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
        let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
        let (text, _, _) = encoding.decode(raw_bytes);
        text.into_pyobject_or_pyerr(py)
    }

    /// Get response headers (sync)
    #[getter]
    fn get_headers<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        if let Some(headers) = &self._headers {
            return headers.clone().into_pyobject(py);
        }

        let resp = Arc::clone(&self.resp);
        let headers: IndexMapSSR = RUNTIME.block_on(async {
            let resp_guard = resp.lock().await;
            match resp_guard.as_ref() {
                Some(r) => Ok(r.headers().to_indexmap()),
                None => Err(BodyError::new_err(
                    "Response body already consumed or moved",
                )),
            }
        })?;

        let py_dict = headers.clone().into_pyobject(py)?;
        self._headers = Some(headers);
        Ok(py_dict)
    }

    /// Get response cookies (sync)
    #[getter]
    fn get_cookies<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        if let Some(cookies) = &self._cookies {
            return cookies.clone().into_pyobject(py);
        }

        let resp = Arc::clone(&self.resp);
        let cookies: IndexMapSSR = RUNTIME.block_on(async {
            let resp_guard = resp.lock().await;
            match resp_guard.as_ref() {
                Some(r) => Ok(crate::extract_cookies_to_indexmap(r.headers())),
                None => Err(BodyError::new_err(
                    "Response body already consumed or moved",
                )),
            }
        })?;

        let py_dict = cookies.clone().into_pyobject(py)?;
        self._cookies = Some(cookies);
        Ok(py_dict)
    }

    /// Get HTML converted to Markdown (sync)
    #[getter]
    fn text_markdown(&mut self, py: Python<'_>) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text = py.detach(|| from_read(raw_bytes, 100))?;
        Ok(text)
    }

    /// Get HTML converted to plain text (sync)
    #[getter]
    fn text_plain(&mut self, py: Python<'_>) -> Result<String> {
        let content = self.get_content(py)?.unbind();
        let raw_bytes = content.as_bytes(py);
        let text =
            py.detach(|| from_read_with_decorator(raw_bytes, 100, TrivialDecorator::new()))?;
        Ok(text)
    }

    /// Get HTML converted to rich text (sync)
    #[getter]
    fn text_rich(&mut self, py: Python<'_>) -> Result<String> {
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

    /// Read remaining content into memory (async)
    fn aread<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;
        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            let bytes = match resp_guard.as_mut() {
                Some(r) => match collect_body_bytes(r).await {
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
                Some(r) => match collect_body_bytes(r).await {
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
        use pyo3_async_runtimes::tokio::future_into_py;

        let resp = Arc::clone(&self.resp);
        let bytes_future = future_into_py(py, async move {
            let mut resp_guard = resp.lock().await;
            let bytes = match resp_guard.as_mut() {
                Some(r) => match collect_body_bytes(r).await {
                    Ok(buf) => buf,
                    Err(e) => return Err(e),
                },
                None => Bytes::new(),
            };
            Ok::<Vec<u8>, PyErr>(bytes.to_vec())
        })?;

        let json_code = c"
import json as _json
async def _parse_json(coro):
    data = await coro
    return _json.loads(bytes(data))
_parse_json
";
        let parse_fn = py.eval(json_code, None, None)?.call0()?;
        parse_fn.call1((bytes_future,))
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
