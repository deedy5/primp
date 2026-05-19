use std::sync::Arc;

use encoding_rs::{Encoding, UTF_8};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    IntoPyObjectExt,
};
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::error::convert_reqwest_error;
use crate::response_shared;

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
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        response_shared::get_content(&self.resp, &mut self._content, self.streaming, py)
    }

    #[getter]
    fn get_encoding(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::get_encoding(&self.resp, &mut self._encoding, py)
    }

    #[setter]
    fn set_encoding(&mut self, encoding: Option<String>) -> PyResult<()> {
        if let Some(encoding) = encoding {
            self._encoding = Some(encoding);
        }
        Ok(())
    }

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

    fn json<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyAny>> {
        response_shared::json(&self.resp, &mut self._content, self.streaming, py)
    }

    #[getter]
    fn get_headers<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        response_shared::get_headers(&self.resp, &mut self._headers, py)
    }

    #[getter]
    fn get_cookies<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyDict>> {
        response_shared::get_cookies(&self.resp, &mut self._cookies, py)
    }

    #[getter]
    fn text_markdown(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::text_markdown(&self.resp, &mut self._content, self.streaming, py)
    }

    #[getter]
    fn text_plain(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::text_plain(&self.resp, &mut self._content, self.streaming, py)
    }

    #[getter]
    fn text_rich(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::text_rich(&self.resp, &mut self._content, self.streaming, py)
    }

    fn raise_for_status(&self) -> PyResult<()> {
        response_shared::raise_for_status(self.status_code, &self.url)
    }

    fn read<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        response_shared::get_content(&self.resp, &mut self._content, self.streaming, py)
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
            buffer: Vec::with_capacity(chunk_size * 2),
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
    encoding: &'static Encoding,
    chunk_size: usize,
    buffer: Vec<u8>,
}

impl TextIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        encoding: String,
        chunk_size: usize,
    ) -> Self {
        let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
        TextIterator {
            resp,
            encoding,
            chunk_size,
            buffer: Vec::with_capacity(chunk_size * 2),
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
            let (text, _, _) = self.encoding.decode(&chunk);
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
                    let (text, _, _) = self.encoding.decode(&result);
                    Ok(Some(text.into_pyobject_or_pyerr(py)?))
                } else if !self.buffer.is_empty() {
                    let result: Vec<u8> = std::mem::take(&mut self.buffer);
                    let (text, _, _) = self.encoding.decode(&result);
                    Ok(Some(text.into_pyobject_or_pyerr(py)?))
                } else {
                    Ok(None)
                }
            }
            None => {
                if !self.buffer.is_empty() {
                    let result: Vec<u8> = std::mem::take(&mut self.buffer);
                    let (text, _, _) = self.encoding.decode(&result);
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
    buffer: Vec<u8>,
    done: bool,
}

impl LinesIterator {
    fn new(resp: Arc<TMutex<Option<::primp::Response>>>) -> Self {
        LinesIterator {
            resp,
            buffer: Vec::with_capacity(8192),
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
            if let Some(newline_pos) = self.buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.buffer.drain(..=newline_pos).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim_end_matches('\r').trim_end_matches('\n');
                return Ok(Some(line.to_owned().into_pyobject_or_pyerr(py)?));
            }

            if self.done {
                if !self.buffer.is_empty() {
                    let remaining = std::mem::take(&mut self.buffer);
                    let line = String::from_utf8_lossy(&remaining);
                    return Ok(Some(line.into_owned().into_pyobject_or_pyerr(py)?));
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
                    self.buffer.extend_from_slice(&data);
                }
                None => {
                    self.done = true;
                }
            }
        }
    }
}
