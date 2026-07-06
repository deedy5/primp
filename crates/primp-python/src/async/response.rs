use std::sync::Arc;

use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    PyErr,
};
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::r#async::bridge::BridgeTaskError;
use crate::response_shared::{self, chars_to_byte_pos};
use crate::traits::HeadersTraits;

/// An async HTTP response, supporting buffered and streaming modes.
#[pyclass]
pub struct AsyncResponse {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    _content: Option<Py<PyBytes>>,
    /// Bytes drained by `aread()` for non-streaming bodies, served by the
    /// sync-style getters (mirrors sync `read()`'s cache population).
    /// `Arc<Mutex<..>>` because the drain runs inside the bridge task,
    /// which cannot touch Python objects.
    _drained: Arc<std::sync::Mutex<Option<Vec<u8>>>>,
    _encoding: Option<String>,
    _headers: Option<IndexMapSSR>,
    _cookies: Option<IndexMapSSR>,
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub status_code: u16,
    streaming: bool,
    /// Set once the body is fully consumed, so further calls return `None`
    /// instead of re-polling the exhausted stream.
    exhausted: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AsyncResponse {
    pub fn new(resp: ::primp::Response, url: String, status_code: u16) -> Self {
        // Eagerly extract headers, cookies, and encoding from the response
        // (same as the sync path) so property accesses don't need to re-lock.
        let headers = resp.headers().to_indexmap();
        let cookies = crate::extract_cookies_to_indexmap(resp.headers());
        let encoding = crate::utils::extract_encoding(resp.headers())
            .name()
            .to_string();
        AsyncResponse {
            resp: Arc::new(TMutex::new(Some(resp))),
            _content: None,
            _drained: Arc::new(std::sync::Mutex::new(None)),
            _encoding: Some(encoding),
            _headers: Some(headers),
            _cookies: Some(cookies),
            url,
            status_code,
            streaming: false,
            exhausted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
            _drained: Arc::new(std::sync::Mutex::new(None)),
            _encoding: Some(encoding),
            _headers: Some(headers),
            _cookies: Some(cookies),
            url,
            status_code,
            streaming: true,
            exhausted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[pymethods]
impl AsyncResponse {
    /// Serve bytes already drained by `aread()` through the `_content`
    /// cache, mirroring sync `read()` for non-streaming bodies.
    fn promote_drained<'rs>(&mut self, py: Python<'rs>) {
        if !self.streaming && self._content.is_none() {
            let bytes = self
                ._drained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(bytes) = bytes {
                self._content = Some(PyBytes::new(py, &bytes).unbind());
            }
        }
    }

    /// Get response content as bytes (sync - blocks until content is read)
    #[getter]
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        self.promote_drained(py);
        let content =
            response_shared::get_content(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(content)
    }

    /// Get character encoding (sync)
    #[getter]
    fn get_encoding(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::get_encoding(&self.resp, &mut self._encoding, py)
    }

    /// Set character encoding
    #[setter]
    fn set_encoding(&mut self, encoding: Option<String>) -> PyResult<()> {
        self._encoding = encoding;
        Ok(())
    }

    /// Get response text (sync - blocks until content is read)
    #[getter]
    fn text<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyString>> {
        self.promote_drained(py);
        let text = response_shared::text(
            &self.resp,
            &mut self._content,
            &mut self._encoding,
            self.streaming,
            py,
        )?;
        self.exhausted
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(text)
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
        self.promote_drained(py);
        let text =
            response_shared::text_markdown(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(text)
    }

    /// Get HTML converted to plain text (sync)
    #[getter]
    fn text_plain(&mut self, py: Python<'_>) -> PyResult<String> {
        self.promote_drained(py);
        let text = response_shared::text_plain(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(text)
    }

    /// Get HTML converted to rich text (sync)
    #[getter]
    fn text_rich(&mut self, py: Python<'_>) -> PyResult<String> {
        self.promote_drained(py);
        let text = response_shared::text_rich(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(text)
    }

    /// Parse response body as JSON (sync)
    fn json<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyAny>> {
        self.promote_drained(py);
        // The body is fully drained by get_content before parsing, so mark
        // exhausted even when the JSON parse fails — a later anext() must not
        // re-poll the exhausted core (which would raise stream_exhausted).
        self.exhausted
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let json = response_shared::json(&self.resp, &mut self._content, self.streaming, py)?;
        Ok(json)
    }

    /// Raise HTTPError for 4xx/5xx status codes (sync)
    fn raise_for_status(&self) -> PyResult<()> {
        response_shared::raise_for_status(self.status_code, &self.url)
    }

    /// Read remaining content into memory (async)
    fn aread<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;
        // Mirror sync `read()`: a non-streaming body already buffered by
        // `content`/`text`/`json` or a prior `aread()` is returned from the
        // cache instead of re-awaiting the exhausted stream (which would raise
        // a BodyError). `aread()` buffers into `_drained` (bridge task cannot
        // touch `PyBytes`), sync getters buffer into `_content`; either cache
        // suffices.
        if !self.streaming {
            if let Some(content) = &self._content {
                let bytes: Vec<u8> = content.bind(py).as_bytes().to_vec();
                return future_into_coroutine(py, async move { Ok::<Vec<u8>, PyErr>(bytes) });
            }
            if let Some(bytes) = self
                ._drained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
            {
                return future_into_coroutine(py, async move { Ok::<Vec<u8>, PyErr>(bytes) });
            }
        }
        // Streaming bodies are single-use: second aread must be BodyError.
        // Non-streaming is idempotent via the caches above; streaming has no
        // cache, so check exhausted explicitly rather than relying on core
        // returning Err(stream_exhausted) (would be fragile if core returned Ok(None)).
        if self.streaming && self.exhausted.load(std::sync::atomic::Ordering::Relaxed) {
            return future_into_coroutine(py, async move {
                Err::<Vec<u8>, PyErr>(crate::error::BodyError::new_err(
                    "Response body already consumed or moved",
                ))
            });
        }
        let resp = Arc::clone(&self.resp);
        let exhausted = Arc::clone(&self.exhausted);
        let drained = Arc::clone(&self._drained);
        let streaming = self.streaming;
        let future = async move {
            let mut resp_guard = resp.lock().await;
            let bytes = match resp_guard.as_mut() {
                Some(r) => match response_shared::collect_body_bytes(r).await {
                    Ok(buf) => buf,
                    Err(e) => return Err(e),
                },
                // Mirrors sync `read()` after `close()`: the body is gone,
                // so error instead of pretending it was an empty success.
                None => {
                    return Err(crate::error::BodyError::new_err(
                        "Response body already consumed or moved",
                    ))
                }
            };
            // Mark the response exhausted so a subsequent `anext()` returns
            // `None` instead of re-polling the drained core (which would
            // surface `stream_exhausted` -> `PrimpError`).
            exhausted.store(true, std::sync::atomic::Ordering::Relaxed);
            if !streaming {
                // Mirror sync `read()`: buffer the drained body so the
                // sync-style getters (`content`/`text`/`json`) serve it
                // instead of raising BodyError.
                *drained
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(bytes.to_vec());
            }
            Ok::<Vec<u8>, PyErr>(bytes.to_vec())
        };
        future_into_coroutine(py, future)
    }

    #[pyo3(signature = (chunk_size=None))]
    fn aiter_bytes(&self, chunk_size: Option<usize>) -> PyResult<AsyncBytesIterator> {
        let chunk_size = response_shared::parse_chunk_size(chunk_size)
            .map_err(pyo3::exceptions::PyValueError::new_err)?;
        let resp = Arc::clone(&self.resp);
        Ok(AsyncBytesIterator::new(
            resp,
            chunk_size,
            Arc::clone(&self.exhausted),
        ))
    }

    #[pyo3(signature = (chunk_size=None))]
    fn aiter_text(&self, py: Python<'_>, chunk_size: Option<usize>) -> PyResult<AsyncTextIterator> {
        let chunk_size = response_shared::parse_chunk_size(chunk_size)
            .map_err(pyo3::exceptions::PyValueError::new_err)?;
        let resp = Arc::clone(&self.resp);
        let encoding = match &self._encoding {
            Some(enc) => enc.clone(),
            None => {
                let mut enc_cache = self._encoding.clone();
                response_shared::get_encoding(&self.resp, &mut enc_cache, py)?
            }
        };
        Ok(AsyncTextIterator::new(
            resp,
            encoding,
            chunk_size,
            Arc::clone(&self.exhausted),
        ))
    }

    fn aiter_lines(&self, py: Python<'_>) -> PyResult<AsyncLinesIterator> {
        let resp = Arc::clone(&self.resp);
        let encoding = match &self._encoding {
            Some(enc) => enc.clone(),
            None => {
                let mut enc_cache = self._encoding.clone();
                response_shared::get_encoding(&self.resp, &mut enc_cache, py)?
            }
        };
        Ok(AsyncLinesIterator::new(
            resp,
            encoding,
            Arc::clone(&self.exhausted),
        ))
    }

    fn anext<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;
        if self.exhausted.load(std::sync::atomic::Ordering::Relaxed) {
            return future_into_coroutine(py, async { Ok::<Option<Vec<u8>>, PyErr>(None) });
        }
        let resp = Arc::clone(&self.resp);
        let exhausted = Arc::clone(&self.exhausted);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            // `Response::chunk()` already skips empty HTTP/2 DATA frames.
            match resp_guard.as_mut() {
                Some(r) => match r.chunk().await {
                    Ok(Some(data)) => Ok::<Option<Vec<u8>>, BridgeTaskError>(Some(data.to_vec())),
                    Ok(None) => Ok(None),
                    Err(e) if e.is_stream_exhausted() => Ok(None),
                    Err(e) => Err(BridgeTaskError::Deferred(e.into())),
                },
                None => Ok(None),
            }
        };
        future_into_coroutine(py, async move {
            let result = future.await;
            if matches!(result, Ok(None)) {
                exhausted.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            result
        })
    }

    fn aclose<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            resp_guard.take();
            Ok::<(), PyErr>(())
        };

        future_into_coroutine(py, future)
    }

    /// Async context manager entry
    fn __aenter__<'py>(slf: PyRef<'_, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;

        let slf_py: Py<AsyncResponse> = Py::from(slf);
        let future = async move { Ok::<Py<AsyncResponse>, PyErr>(slf_py) };
        future_into_coroutine(py, future)
    }

    /// Async context manager exit
    fn __aexit__<'py>(
        &mut self,
        py: Python<'py>,
        _exc_type: Option<Bound<'py, PyAny>>,
        _exc_value: Option<Bound<'py, PyAny>>,
        _traceback: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;

        let resp = Arc::clone(&self.resp);
        let future = async move {
            let mut resp_guard = resp.lock().await;
            resp_guard.take();
            Ok::<bool, PyErr>(false)
        };

        future_into_coroutine(py, future)
    }
}

/// Async iterator over byte chunks from a streaming response.
#[pyclass]
pub struct AsyncBytesIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    chunk_size: usize,
    buffer: Arc<TMutex<Vec<u8>>>,
    /// Shared with the Response: set when the stream ends, so a new
    /// iterator created after this one finished also stops quietly.
    exhausted: Arc<std::sync::atomic::AtomicBool>,
    /// Set once the stream ends and the buffer is flushed, so repeated
    /// `__anext__` calls after the end raise `StopAsyncIteration` idempotently
    /// without re-polling the exhausted `Response`.
    done: Arc<TMutex<bool>>,
}

impl AsyncBytesIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        chunk_size: usize,
        exhausted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        AsyncBytesIterator {
            resp,
            chunk_size,
            // Grow lazily — never `with_capacity(chunk_size * 2)`: `chunk_size`
            // is validated only up to 1 GiB, so a 2 GiB eager reserve could
            // abort under `panic = "abort"` on a constrained host. Memory then
            // scales with the body actually received.
            buffer: Arc::new(TMutex::new(Vec::new())),
            // An iterator created after a full-body drain must raise
            // StopAsyncIteration on the first `__anext__`, not re-poll the
            // exhausted core.
            done: Arc::new(TMutex::new(
                exhausted.load(std::sync::atomic::Ordering::Relaxed),
            )),
            exhausted,
        }
    }
}

#[pymethods]
impl AsyncBytesIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;

        let resp = Arc::clone(&self.resp);
        let chunk_size = self.chunk_size;
        let buffer = Arc::clone(&self.buffer);
        let done = Arc::clone(&self.done);
        let exhausted = Arc::clone(&self.exhausted);

        let future = async move {
            // A drain by another consumer since creation must stop us quietly, not re-poll.
            if exhausted.load(std::sync::atomic::Ordering::Relaxed) {
                *done.lock().await = true;
            }
            if *done.lock().await {
                // Never drop bytes buffered since the last poll (mirrors
                // the sync iterator and the EOF branch).
                let mut buf = buffer.lock().await;
                if !buf.is_empty() {
                    let result: Vec<u8> = std::mem::take(&mut *buf);
                    return Ok::<Vec<u8>, BridgeTaskError>(result);
                }
                return Err(BridgeTaskError::Ready(PyErr::new::<
                    pyo3::exceptions::PyStopAsyncIteration,
                    _,
                >("Stream exhausted")));
            }
            {
                let mut buf = buffer.lock().await;
                if buf.len() >= chunk_size {
                    let chunk: Vec<u8> = buf.drain(..chunk_size).collect();
                    return Ok::<Vec<u8>, BridgeTaskError>(chunk);
                }
            }

            let mut resp_guard = resp.lock().await;
            // `Response::chunk()` already skips empty HTTP/2 DATA frames.
            let data: Option<Vec<u8>> = match resp_guard.as_mut() {
                Some(r) => match r.chunk().await {
                    Ok(Some(data)) => Some(data.to_vec()),
                    Ok(None) => None,
                    Err(e) if e.is_stream_exhausted() => None,
                    Err(e) => return Err(BridgeTaskError::Deferred(e.into())),
                },
                None => None,
            };

            match data {
                Some(data) => {
                    let mut buf = buffer.lock().await;
                    buf.extend_from_slice(&data);
                    if buf.len() >= chunk_size {
                        let result: Vec<u8> = buf.drain(..chunk_size).collect();
                        Ok(result)
                    } else if !buf.is_empty() {
                        let result: Vec<u8> = std::mem::take(&mut *buf);
                        Ok(result)
                    } else {
                        // Empty DATA-like chunk: treat as end of stream. Use
                        // the same guards as the EOF branch so a fresh
                        // iterator also stops quietly instead of re-polling
                        // the stream.
                        *done.lock().await = true;
                        exhausted.store(true, std::sync::atomic::Ordering::Relaxed);
                        Err(BridgeTaskError::Ready(PyErr::new::<
                            pyo3::exceptions::PyStopAsyncIteration,
                            _,
                        >(
                            "Stream exhausted"
                        )))
                    }
                }
                None => {
                    // EOF: flush the partial buffer, then set the
                    // per-iterator guard AND the shared Response flag so any
                    // later iterator also stops quietly.
                    *done.lock().await = true;
                    exhausted.store(true, std::sync::atomic::Ordering::Relaxed);
                    let mut buf = buffer.lock().await;
                    if !buf.is_empty() {
                        let result: Vec<u8> = std::mem::take(&mut *buf);
                        Ok(result)
                    } else {
                        Err(BridgeTaskError::Ready(PyErr::new::<
                            pyo3::exceptions::PyStopAsyncIteration,
                            _,
                        >(
                            "Stream exhausted"
                        )))
                    }
                }
            }
        };

        future_into_coroutine(py, future)
    }
}

/// Async iterator over text chunks from a streaming response.
#[pyclass]
pub struct AsyncTextIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    chunk_size: usize,
    /// Decoded text waiting to be returned; always complete chars (the
    /// decoder buffers any incomplete multi-byte sequence internally).
    decoded: Arc<TMutex<String>>,
    /// Cached char count of `decoded`; kept in sync incrementally so the
    /// length check in `__anext__` stays O(1) instead of O(decoded.len()).
    decoded_char_count: Arc<TMutex<usize>>,
    decoder: Arc<TMutex<encoding_rs::Decoder>>,
    eof: Arc<TMutex<bool>>,
    /// Response-level flag shared with the Response (see `AsyncBytesIterator`).
    exhausted: Arc<std::sync::atomic::AtomicBool>,
}

impl AsyncTextIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        encoding: String,
        chunk_size: usize,
        exhausted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let encoding =
            encoding_rs::Encoding::for_label(encoding.as_bytes()).unwrap_or(encoding_rs::UTF_8);
        AsyncTextIterator {
            resp,
            chunk_size,
            decoded: Arc::new(TMutex::new(String::new())),
            decoded_char_count: Arc::new(TMutex::new(0)),
            decoder: Arc::new(TMutex::new(encoding.new_decoder())),
            // An iterator created after a full-body drain must raise
            // StopAsyncIteration on the first `__anext__`, not re-poll the
            // exhausted core.
            eof: Arc::new(TMutex::new(
                exhausted.load(std::sync::atomic::Ordering::Relaxed),
            )),
            exhausted,
        }
    }
}

#[pymethods]
impl AsyncTextIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;

        let resp = Arc::clone(&self.resp);
        let chunk_size = self.chunk_size;
        let decoded = Arc::clone(&self.decoded);
        let decoded_char_count = Arc::clone(&self.decoded_char_count);
        let decoder = Arc::clone(&self.decoder);
        let eof = Arc::clone(&self.eof);
        let exhausted = Arc::clone(&self.exhausted);

        let future = async move {
            loop {
                // Lock ordering rules for this loop:
                //   * `eof` is always acquired in isolation — never held with
                //     any other lock — to keep it on a single-lock critical
                //     section and avoid cross-task deadlock.
                //   * `decoded` is always acquired before `decoder` and
                //     `decoded_char_count`, so the two ordering paths below
                //     are compatible:
                //       - count check: decoded → decoded_char_count
                //       - decode:      decoded → decoder → decoded_char_count

                // Check if we already have enough decoded chars.
                {
                    let mut buf = decoded.lock().await;
                    let mut count = decoded_char_count.lock().await;
                    if *count >= chunk_size {
                        let end = chars_to_byte_pos(&buf, chunk_size);
                        let chunk: String = buf.drain(..end).collect();
                        // `drain(..end)` removes exactly `chunk_size` chars;
                        // the O(1) update keeps the cached counter accurate.
                        *count -= chunk_size;
                        return Ok::<String, BridgeTaskError>(chunk);
                    }
                }

                // A drain by another consumer since creation must stop us quietly, not re-poll.
                if exhausted.load(std::sync::atomic::Ordering::Relaxed) {
                    *eof.lock().await = true;
                }
                // Check EOF without holding other locks.
                let is_eof = *eof.lock().await;
                if is_eof {
                    let mut buf = decoded.lock().await;
                    if !buf.is_empty() {
                        let chunk = std::mem::take(&mut *buf);
                        let mut count = decoded_char_count.lock().await;
                        *count = 0;
                        return Ok(chunk);
                    }
                    return Err(BridgeTaskError::Ready(PyErr::new::<
                        pyo3::exceptions::PyStopAsyncIteration,
                        _,
                    >(
                        "Stream exhausted"
                    )));
                }

                let mut resp_guard = resp.lock().await;
                // `Response::chunk()` already skips empty HTTP/2 DATA frames.
                let data: Option<Vec<u8>> = match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => Some(data.to_vec()),
                        Ok(None) => None,
                        Err(e) if e.is_stream_exhausted() => None,
                        Err(e) => return Err(BridgeTaskError::Deferred(e.into())),
                    },
                    None => None,
                };

                // Acquire locks in consistent order: decoded → decoder → decoded_char_count.
                let mut buf = decoded.lock().await;
                let mut dec = decoder.lock().await;
                let mut count = decoded_char_count.lock().await;
                match data {
                    Some(data) => {
                        // Stateful decode: incomplete sequences stay in `dec`.
                        let mut out = vec![0u8; data.len() * 4 + 16].into_boxed_slice();
                        let (_result, _read, written, _replaced) =
                            dec.decode_to_utf8(&data, &mut out, false);
                        // SAFETY: encoding_rs always emits valid UTF-8 (replaces
                        // malformed sequences with U+FFFD).
                        debug_assert!(
                            std::str::from_utf8(&out[..written]).is_ok(),
                            "encoding_rs produced invalid UTF-8"
                        );
                        let s = unsafe { std::str::from_utf8_unchecked(&out[..written]) };
                        *count += s.chars().count();
                        buf.push_str(s);
                    }
                    None => {
                        // EOF: flush the decoder's state (U+FFFD for truncated).
                        // 64 bytes is defensive headroom; encoding_rs bounds
                        // per-call output to one U+FFFD per truncated input
                        // sequence (≤4 bytes for any supported encoding).
                        let mut out = [0u8; 64];
                        let (_result, _read, written, _replaced) =
                            dec.decode_to_utf8(&[], &mut out, true);
                        debug_assert!(written <= out.len(), "decoder wrote past EOF buffer");
                        // SAFETY: same as above.
                        debug_assert!(
                            std::str::from_utf8(&out[..written]).is_ok(),
                            "encoding_rs produced invalid UTF-8 at EOF"
                        );
                        let s = unsafe { std::str::from_utf8_unchecked(&out[..written]) };
                        *count += s.chars().count();
                        buf.push_str(s);
                        // Release buf/dec/count before acquiring eof lock to
                        // maintain consistent lock ordering.
                        drop(count);
                        drop(dec);
                        drop(buf);
                        *eof.lock().await = true;
                        exhausted.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        };

        future_into_coroutine(py, future)
    }
}

/// Async iterator over lines from a streaming response.
#[pyclass]
pub struct AsyncLinesIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    buffer: Arc<TMutex<Vec<u8>>>,
    decoder: Arc<TMutex<encoding_rs::Decoder>>,
    /// Accumulated decoded text carried across `__anext__` calls.
    decoded: Arc<TMutex<String>>,
    done: Arc<TMutex<bool>>,
    /// Response-level flag shared with the Response (see `AsyncBytesIterator`).
    exhausted: Arc<std::sync::atomic::AtomicBool>,
}

impl AsyncLinesIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        encoding: String,
        exhausted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let encoding =
            encoding_rs::Encoding::for_label(encoding.as_bytes()).unwrap_or(encoding_rs::UTF_8);
        AsyncLinesIterator {
            resp,
            buffer: Arc::new(TMutex::new(Vec::with_capacity(8192))),
            decoder: Arc::new(TMutex::new(encoding.new_decoder())),
            decoded: Arc::new(TMutex::new(String::new())),
            // An iterator created after a full-body drain must raise
            // StopAsyncIteration on the first `__anext__`, not re-poll the
            // exhausted core.
            done: Arc::new(TMutex::new(
                exhausted.load(std::sync::atomic::Ordering::Relaxed),
            )),
            exhausted,
        }
    }
}

#[pymethods]
impl AsyncLinesIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::future_into_coroutine;

        // Lock acquisition: there are two paths below, but they run
        // sequentially within a single __anext__ call and their guards
        // do not overlap:
        //   Path A (newline check): decoded only
        //   Path B (decode buffer):  buffer → decoder → decoded
        // Path A returns (guard dropped) before Path B starts, so no
        // deadlock is possible under Python's sequential iteration.

        let resp = Arc::clone(&self.resp);
        let buffer = Arc::clone(&self.buffer);
        let decoder = Arc::clone(&self.decoder);
        let decoded = Arc::clone(&self.decoded);
        let done = Arc::clone(&self.done);
        let exhausted = Arc::clone(&self.exhausted);

        let future = async move {
            loop {
                // Check for a newline in already-decoded text from a
                // previous iteration before decoding more from the buffer.
                {
                    let mut dec_text = decoded.lock().await;
                    if let Some(newline_pos) = dec_text.find('\n') {
                        let line = &dec_text[..=newline_pos];
                        let line = line
                            .trim_end_matches('\n')
                            .trim_end_matches('\r')
                            .to_owned();
                        // Keep the rest in `decoded` (not the raw buffer) to
                        // avoid round-tripping through UTF-8 bytes, which
                        // corrupts non-UTF-8 encodings and can orphan
                        // incomplete multi-byte decoder state.
                        dec_text.drain(..=newline_pos);
                        return Ok::<String, BridgeTaskError>(line);
                    }
                }

                // Decode buffered bytes, preserving incomplete multi-byte
                // sequences across chunk boundaries.
                {
                    let mut buf = buffer.lock().await;
                    let mut dec = decoder.lock().await;
                    let mut input_pos = 0usize;
                    loop {
                        let remaining = &buf[input_pos..];
                        if remaining.is_empty() {
                            break;
                        }
                        let buf_size = remaining.len().max(64);
                        let mut out = vec![0u8; buf_size];
                        let (_result, read, written, _) =
                            dec.decode_to_utf8(remaining, &mut out, false);
                        if written > 0 {
                            // SAFETY: encoding_rs always emits valid UTF-8.
                            debug_assert!(
                                std::str::from_utf8(&out[..written]).is_ok(),
                                "encoding_rs produced invalid UTF-8 in AsyncLinesIterator"
                            );
                            let s = unsafe { std::str::from_utf8_unchecked(&out[..written]) };
                            let mut dec_text = decoded.lock().await;
                            dec_text.push_str(s);
                        }
                        input_pos += read;
                        if read == 0 {
                            break;
                        }
                    }
                    buf.drain(..input_pos);
                }

                // Check again after decoding new bytes.
                {
                    let mut dec_text = decoded.lock().await;
                    if let Some(newline_pos) = dec_text.find('\n') {
                        let line = &dec_text[..=newline_pos];
                        let line = line
                            .trim_end_matches('\n')
                            .trim_end_matches('\r')
                            .to_owned();
                        dec_text.drain(..=newline_pos);
                        return Ok::<String, BridgeTaskError>(line);
                    }
                }

                {
                    // A drain by another consumer since creation must stop us quietly, not re-poll.
                    if exhausted.load(std::sync::atomic::Ordering::Relaxed) {
                        *done.lock().await = true;
                    }
                    let is_done = *done.lock().await;
                    if is_done {
                        let mut dec_text = decoded.lock().await;
                        if !dec_text.is_empty() {
                            let remaining = std::mem::take(&mut *dec_text);
                            return Ok(remaining);
                        }
                        return Err(BridgeTaskError::Ready(PyErr::new::<
                            pyo3::exceptions::PyStopAsyncIteration,
                            _,
                        >(
                            "Stream exhausted"
                        )));
                    }
                }

                let mut resp_guard = resp.lock().await;
                let mut finished = false;
                match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => {
                            let mut buf = buffer.lock().await;
                            buf.extend_from_slice(&data);
                        }
                        Ok(None) => finished = true,
                        Err(e) if e.is_stream_exhausted() => finished = true,
                        Err(e) => return Err(BridgeTaskError::Deferred(e.into())),
                    },
                    None => finished = true,
                }
                if finished {
                    // Flush incomplete multi-byte tail as U+FFFD, not dropped.
                    let mut dec = decoder.lock().await;
                    let mut out = [0u8; 64];
                    let (_result, _read, written, _replaced) =
                        dec.decode_to_utf8(&[], &mut out, true);
                    if written > 0 {
                        debug_assert!(
                            std::str::from_utf8(&out[..written]).is_ok(),
                            "encoding_rs produced invalid UTF-8 at EOF in AsyncLinesIterator"
                        );
                        let s = unsafe { std::str::from_utf8_unchecked(&out[..written]) };
                        let mut dec_text = decoded.lock().await;
                        dec_text.push_str(s);
                    }
                    drop(dec);
                    let mut is_done = done.lock().await;
                    *is_done = true;
                    exhausted.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        };

        future_into_coroutine(py, future)
    }
}
