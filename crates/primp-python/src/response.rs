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
    IntoPyObjectExt,
};
use pythonize::pythonize;
use serde_json::from_slice;
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::error::{body_collection_error, convert_reqwest_error, BodyError, PrimpErrorEnum};
use crate::traits::HeadersTraits;
use crate::utils::extract_encoding;

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

/// A struct representing an HTTP response.
///
/// Supports both buffered (non-streaming) and streaming modes.
/// In buffered mode, body content is cached after first read.
/// In streaming mode, body is consumed on each read and supports iteration.
#[pyclass]
pub struct Response {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    _content: Option<Py<PyBytes>>,
    _encoding: Option<String>,
    _headers: Option<IndexMapSSR>,
    _cookies: Option<IndexMapSSR>,
    #[pyo3(get)]
    url: String,
    #[pyo3(get)]
    status_code: u16,
    streaming: bool,
}

impl Response {
    pub fn new(
        resp: ::primp::Response,
        url: String,
        status_code: u16,
        headers: IndexMapSSR,
        cookies: IndexMapSSR,
        encoding: String,
    ) -> Self {
        Response {
            resp: Arc::new(TMutex::new(Some(resp))),
            _content: None,
            _encoding: Some(encoding),
            _headers: Some(headers),
            _cookies: Some(cookies),
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
        Response {
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

    async fn read_bytes(&self) -> Result<Bytes, PyErr> {
        let resp = Arc::clone(&self.resp);
        let mut resp_guard = resp.lock().await;
        match resp_guard.as_mut() {
            Some(r) => collect_body_bytes(r).await,
            None => Err(BodyError::new_err(
                "Response body already consumed or moved",
            )),
        }
    }

    async fn next_chunk(&self) -> Result<Option<Vec<u8>>, PyErr> {
        let resp = Arc::clone(&self.resp);
        let mut resp_guard = resp.lock().await;
        match resp_guard.as_mut() {
            Some(r) => match r.chunk().await {
                Ok(Some(data)) => Ok(Some(data.to_vec())),
                Ok(None) => Ok(None),
                Err(e) => Err(convert_reqwest_error(e)),
            },
            None => Ok(None),
        }
    }
}

#[pymethods]
impl Response {
    #[getter]
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyBytes>> {
        if !self.streaming {
            if let Some(content) = &self._content {
                let cloned = content.clone_ref(py);
                return Ok(cloned.into_bound(py));
            }
        }

        let runtime = crate::get_runtime(py);
        let bytes: Bytes = py.detach(|| runtime.block_on(self.read_bytes()))?;
        let content = PyBytes::new(py, &bytes);

        if !self.streaming {
            self._content = Some(content.clone().unbind());
        }
        Ok(content)
    }

    #[getter]
    fn get_encoding(&mut self, py: Python<'_>) -> Result<String> {
        if let Some(encoding) = self._encoding.as_ref() {
            return Ok(encoding.clone());
        }

        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py);
        let encoding = py.detach(|| {
            runtime.block_on(async {
                let resp_guard = resp.lock().await;
                match resp_guard.as_ref() {
                    Some(r) => Ok(extract_encoding(r.headers()).name().to_string()),
                    None => Err(BodyError::new_err(
                        "Response body already consumed or moved",
                    )),
                }
            })
        })?;

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
        let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
        let (text, _, _) = encoding.decode(raw_bytes);
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
            return Ok(headers.clone().into_pyobject(py)?);
        }

        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py);
        let new_headers = py.detach(|| {
            runtime.block_on(async {
                let resp_guard = resp.lock().await;
                match resp_guard.as_ref() {
                    Some(r) => Ok(r.headers().to_indexmap()),
                    None => Err(BodyError::new_err(
                        "Response body already consumed or moved",
                    )),
                }
            })
        })?;

        let py_dict = new_headers.clone().into_pyobject(py)?;
        self._headers = Some(new_headers);
        Ok(py_dict)
    }

    #[getter]
    fn get_cookies<'rs>(&mut self, py: Python<'rs>) -> Result<Bound<'rs, PyDict>> {
        if let Some(cookies) = &self._cookies {
            return Ok(cookies.clone().into_pyobject(py)?);
        }

        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py);
        let cookie_map = py.detach(|| {
            runtime.block_on(async {
                let resp_guard = resp.lock().await;
                match resp_guard.as_ref() {
                    Some(r) => Ok(crate::extract_cookies_to_indexmap(r.headers())),
                    None => Err(BodyError::new_err(
                        "Response body already consumed or moved",
                    )),
                }
            })
        })?;

        let py_dict = cookie_map.clone().into_pyobject(py)?;
        self._cookies = Some(cookie_map);
        Ok(py_dict)
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

    fn read<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        Ok(self.get_content(py)?)
    }

    #[pyo3(signature = (chunk_size=None))]
    fn iter_bytes(&self, chunk_size: Option<usize>) -> PyResult<BytesIterator> {
        Ok(BytesIterator::new(
            Arc::clone(&self.resp),
            chunk_size.unwrap_or(8192),
        ))
    }

    #[pyo3(signature = (chunk_size=None))]
    fn iter_text(&self, chunk_size: Option<usize>) -> PyResult<TextIterator> {
        let encoding = self
            ._encoding
            .clone()
            .unwrap_or_else(|| "utf-8".to_string());
        Ok(TextIterator::new(
            Arc::clone(&self.resp),
            encoding,
            chunk_size.unwrap_or(8192),
        ))
    }

    fn iter_lines(&self) -> PyResult<LinesIterator> {
        Ok(LinesIterator::new(Arc::clone(&self.resp)))
    }

    fn next<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        let runtime = crate::get_runtime(py);
        let chunk = py.detach(|| runtime.block_on(self.next_chunk()))?;
        match chunk {
            Some(data) => Ok(Some(PyBytes::new(py, &data))),
            None => Ok(None),
        }
    }

    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py);
        py.detach(|| {
            runtime.block_on(async {
                let mut resp_guard = resp.lock().await;
                resp_guard.take();
            })
        });
        Ok(())
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

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
        if self.buffer.len() >= self.chunk_size {
            let chunk: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
            return Ok(Some(PyBytes::new(py, &chunk)));
        }

        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py);
        let chunk = py.detach(|| {
            runtime.block_on(async {
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
        if self.buffer.len() >= self.chunk_size {
            let chunk: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
            let encoding = Encoding::for_label(self.encoding.as_bytes()).unwrap_or(UTF_8);
            let (text, _, _) = encoding.decode(&chunk);
            return Ok(Some(text.into_pyobject_or_pyerr(py)?));
        }

        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py);
        let chunk = py.detach(|| {
            runtime.block_on(async {
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

            let resp = Arc::clone(&self.resp);
            let runtime = crate::get_runtime(py);
            let chunk = py.detach(|| {
                runtime.block_on(async {
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
