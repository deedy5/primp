use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use encoding_rs::{Decoder, Encoding, UTF_8};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    IntoPyObjectExt,
};
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::error::primp_error_to_pyerr;
use crate::response_shared::{self, chars_to_byte_pos};

/// An HTTP response, supporting buffered (body cached after first read) and
/// streaming (consumed per read, iterable) modes.
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
    /// Set once the body is fully consumed (by any drain), so further calls
    /// return `None` instead of re-polling the exhausted stream. Shared with
    /// the iterators so new iterators also end quietly.
    exhausted: Arc<AtomicBool>,
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
            exhausted: Arc::new(AtomicBool::new(false)),
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
            exhausted: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn next_chunk(&self) -> Result<Option<Vec<u8>>, PyErr> {
        let resp = Arc::clone(&self.resp);
        let mut resp_guard = resp.lock().await;
        // `Response::chunk()` already skips empty HTTP/2 DATA frames.
        match resp_guard.as_mut() {
            Some(r) => match r.chunk().await {
                Ok(Some(data)) => Ok(Some(data.to_vec())),
                Ok(None) => Ok(None),
                Err(e) if e.is_stream_exhausted() => Ok(None),
                Err(e) => Err(primp_error_to_pyerr(e)),
            },
            None => Ok(None),
        }
    }
}

#[pymethods]
impl Response {
    #[getter]
    fn get_content<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        let content =
            response_shared::get_content(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted.store(true, Ordering::Relaxed);
        Ok(content)
    }

    #[getter]
    fn get_encoding(&mut self, py: Python<'_>) -> PyResult<String> {
        response_shared::get_encoding(&self.resp, &mut self._encoding, py)
    }

    #[setter]
    fn set_encoding(&mut self, encoding: Option<String>) -> PyResult<()> {
        // Raw bytes are cached; decoded text is not — `text()` re-decodes
        // on every call using the current `_encoding`. Setting this
        // between two `text` calls re-decodes the same bytes with the new
        // encoding, matching the Python `requests` library. `None` falls
        // back to the `Content-Type` encoding on the next read.
        self._encoding = encoding;
        Ok(())
    }

    #[getter]
    fn text<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyString>> {
        let text = response_shared::text(
            &self.resp,
            &mut self._content,
            &mut self._encoding,
            self.streaming,
            py,
        )?;
        self.exhausted.store(true, Ordering::Relaxed);
        Ok(text)
    }

    fn json<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyAny>> {
        // The body is fully drained by get_content before parsing, so mark
        // exhausted even when the JSON parse fails — a later next() must not
        // re-poll the exhausted core (which would raise stream_exhausted).
        self.exhausted.store(true, Ordering::Relaxed);
        let json = response_shared::json(&self.resp, &mut self._content, self.streaming, py)?;
        Ok(json)
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
        let text =
            response_shared::text_markdown(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted.store(true, Ordering::Relaxed);
        Ok(text)
    }

    #[getter]
    fn text_plain(&mut self, py: Python<'_>) -> PyResult<String> {
        let text = response_shared::text_plain(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted.store(true, Ordering::Relaxed);
        Ok(text)
    }

    #[getter]
    fn text_rich(&mut self, py: Python<'_>) -> PyResult<String> {
        let text = response_shared::text_rich(&self.resp, &mut self._content, self.streaming, py)?;
        self.exhausted.store(true, Ordering::Relaxed);
        Ok(text)
    }

    fn raise_for_status(&self) -> PyResult<()> {
        response_shared::raise_for_status(self.status_code, &self.url)
    }

    fn read<'rs>(&mut self, py: Python<'rs>) -> PyResult<Bound<'rs, PyBytes>> {
        let content =
            response_shared::get_content(&self.resp, &mut self._content, self.streaming, py)?;
        // `get_content` drains the underlying stream; mark the response
        // exhausted so a subsequent `next()` returns `None` instead of
        // re-polling the exhausted core (which would surface
        // `stream_exhausted` -> `PrimpError`).
        self.exhausted.store(true, Ordering::Relaxed);
        Ok(content)
    }

    #[pyo3(signature = (chunk_size=None))]
    fn iter_bytes(&self, chunk_size: Option<usize>) -> PyResult<BytesIterator> {
        let chunk_size = response_shared::parse_chunk_size(chunk_size)
            .map_err(pyo3::exceptions::PyValueError::new_err)?;
        Ok(BytesIterator::new(
            Arc::clone(&self.resp),
            chunk_size,
            Arc::clone(&self.exhausted),
        ))
    }

    #[pyo3(signature = (chunk_size=None))]
    fn iter_text(&self, py: Python<'_>, chunk_size: Option<usize>) -> PyResult<TextIterator> {
        let chunk_size = response_shared::parse_chunk_size(chunk_size)
            .map_err(pyo3::exceptions::PyValueError::new_err)?;
        let encoding = match &self._encoding {
            Some(enc) => enc.clone(),
            None => {
                let mut enc_cache = self._encoding.clone();
                response_shared::get_encoding(&self.resp, &mut enc_cache, py)?
            }
        };
        Ok(TextIterator::new(
            Arc::clone(&self.resp),
            encoding,
            chunk_size,
            Arc::clone(&self.exhausted),
        ))
    }

    fn iter_lines(&self, py: Python<'_>) -> PyResult<LinesIterator> {
        let encoding = match &self._encoding {
            Some(enc) => enc.clone(),
            None => {
                let mut enc_cache = self._encoding.clone();
                response_shared::get_encoding(&self.resp, &mut enc_cache, py)?
            }
        };
        Ok(LinesIterator::new(
            Arc::clone(&self.resp),
            encoding,
            Arc::clone(&self.exhausted),
        ))
    }

    fn next<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        if self.exhausted.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let runtime = crate::get_runtime(py)?;
        let chunk = py.detach(|| runtime.block_on(self.next_chunk()))?;
        match chunk {
            Some(data) => Ok(Some(PyBytes::new(py, &data))),
            None => {
                self.exhausted.store(true, Ordering::Relaxed);
                Ok(None)
            }
        }
    }

    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py)?;
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
    /// Shared with the Response: set when the stream ends, so a new
    /// iterator created after this one finished also stops quietly.
    exhausted: Arc<AtomicBool>,
    /// Set once the stream ends and the buffer is flushed, so repeated
    /// `__next__` calls after the end raise `StopIteration` idempotently
    /// without re-polling the exhausted `Response`.
    done: bool,
}

impl BytesIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        chunk_size: usize,
        exhausted: Arc<AtomicBool>,
    ) -> Self {
        BytesIterator {
            resp,
            chunk_size,
            // Grow lazily — never `with_capacity(chunk_size * 2)`: `chunk_size`
            // is validated only up to 1 GiB, so a 2 GiB eager reserve could
            // abort under `panic = "abort"` before any data exists. Memory then
            // scales with the body actually received.
            buffer: Vec::new(),
            // An iterator created after the response was fully drained
            // (content/read/text/json, or a previous iterator) must raise
            // StopIteration on the first `__next__`, not re-poll the
            // exhausted core (which would surface `stream_exhausted` ->
            // `PrimpError`).
            done: exhausted.load(Ordering::Relaxed),
            exhausted,
        }
    }
}

#[pymethods]
impl BytesIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyBytes>>> {
        // A drain by another consumer since creation must stop us quietly, not re-poll.
        if self.exhausted.load(Ordering::Relaxed) {
            self.done = true;
        }
        if self.done {
            if !self.buffer.is_empty() {
                let result: Vec<u8> = std::mem::take(&mut self.buffer);
                return Ok(Some(PyBytes::new(py, &result)));
            }
            return Err(pyo3::exceptions::PyStopIteration::new_err(
                "Stream exhausted",
            ));
        }
        if self.buffer.len() >= self.chunk_size {
            let chunk: Vec<u8> = self.buffer.drain(..self.chunk_size).collect();
            return Ok(Some(PyBytes::new(py, &chunk)));
        }

        let resp = Arc::clone(&self.resp);
        let runtime = crate::get_runtime(py)?;
        // `Response::chunk()` already skips empty HTTP/2 DATA frames.
        let chunk: Option<Vec<u8>> = py.detach(|| {
            runtime.block_on(async {
                let mut resp_guard = resp.lock().await;
                match resp_guard.as_mut() {
                    Some(r) => match r.chunk().await {
                        Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                        Ok(None) => Ok(None),
                        Err(e) if e.is_stream_exhausted() => Ok(None),
                        Err(e) => Err(primp_error_to_pyerr(e)),
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
                    // Empty DATA-like chunk: treat as end of stream. Use the
                    // same guard as the EOF branch so a fresh iterator also
                    // stops quietly instead of re-polling the stream.
                    self.done = true;
                    self.exhausted.store(true, Ordering::Relaxed);
                    Err(pyo3::exceptions::PyStopIteration::new_err(
                        "Stream exhausted",
                    ))
                }
            }
            None => {
                // EOF: flush the partial buffer, then set the
                // per-iterator guard AND the shared Response flag so any
                // later iterator also stops quietly.
                self.done = true;
                self.exhausted.store(true, Ordering::Relaxed);
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
    chunk_size: usize,
    /// Decoded text waiting to be returned; always complete chars (the
    /// decoder buffers any incomplete multi-byte sequence internally).
    decoded: String,
    /// Cached char count of `decoded`; kept in sync incrementally so the
    /// length check in `__next__` stays O(1) instead of O(decoded.len()).
    decoded_char_count: usize,
    decoder: Decoder,
    eof: bool,
    /// Response-level flag shared with the Response (see `BytesIterator`).
    exhausted: Arc<AtomicBool>,
}

impl TextIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        encoding: String,
        chunk_size: usize,
        exhausted: Arc<AtomicBool>,
    ) -> Self {
        let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
        TextIterator {
            resp,
            chunk_size,
            decoded: String::new(),
            decoded_char_count: 0,
            decoder: encoding.new_decoder(),
            // An iterator created after a full-body drain must raise
            // StopIteration on the first `__next__`, not re-poll the
            // exhausted core.
            eof: exhausted.load(Ordering::Relaxed),
            exhausted,
        }
    }
}

#[pymethods]
impl TextIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'rs>(&mut self, py: Python<'rs>) -> PyResult<Option<Bound<'rs, PyString>>> {
        loop {
            // A drain by another consumer since creation must stop us quietly, not re-poll.
            if self.exhausted.load(Ordering::Relaxed) {
                self.eof = true;
            }
            if self.decoded_char_count >= self.chunk_size {
                let end = chars_to_byte_pos(&self.decoded, self.chunk_size);
                let chunk: String = self.decoded.drain(..end).collect();
                // `drain(..end)` removes exactly `chunk_size` chars; the
                // O(1) update keeps the cached counter accurate.
                self.decoded_char_count -= self.chunk_size;
                return Ok(Some(chunk.into_pyobject_or_pyerr(py)?));
            }

            if self.eof {
                if !self.decoded.is_empty() {
                    let chunk = std::mem::take(&mut self.decoded);
                    self.decoded_char_count = 0;
                    return Ok(Some(chunk.into_pyobject_or_pyerr(py)?));
                }
                return Err(pyo3::exceptions::PyStopIteration::new_err(
                    "Stream exhausted",
                ));
            }

            let resp = Arc::clone(&self.resp);
            let runtime = crate::get_runtime(py)?;
            // `Response::chunk()` already skips empty HTTP/2 DATA frames.
            let chunk: Option<Vec<u8>> = py.detach(|| {
                runtime.block_on(async {
                    let mut resp_guard = resp.lock().await;
                    match resp_guard.as_mut() {
                        Some(r) => match r.chunk().await {
                            Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                            Ok(None) => Ok(None),
                            Err(e) if e.is_stream_exhausted() => Ok(None),
                            Err(e) => Err(primp_error_to_pyerr(e)),
                        },
                        None => Ok(None),
                    }
                })
            })?;

            match chunk {
                Some(data) => {
                    // Stateful decode: incomplete sequences stay in `decoder`.
                    let mut buf = vec![0u8; data.len() * 4 + 16].into_boxed_slice();
                    let (_result, _read, written, _replaced) =
                        self.decoder.decode_to_utf8(&data, &mut buf, false);
                    // SAFETY: encoding_rs always emits valid UTF-8 (replaces
                    // malformed sequences with U+FFFD).
                    debug_assert!(
                        std::str::from_utf8(&buf[..written]).is_ok(),
                        "encoding_rs produced invalid UTF-8"
                    );
                    let s = unsafe { std::str::from_utf8_unchecked(&buf[..written]) };
                    self.decoded_char_count += s.chars().count();
                    self.decoded.push_str(s);
                }
                None => {
                    // EOF: flush the decoder's state (U+FFFD for truncated).
                    // 64 bytes is defensive headroom: encoding_rs bounds
                    // per-call output to one U+FFFD per truncated input
                    // sequence (≤4 bytes for any supported encoding).
                    let mut buf = [0u8; 64];
                    let (_result, _read, written, _replaced) =
                        self.decoder.decode_to_utf8(&[], &mut buf, true);
                    debug_assert!(written <= buf.len(), "decoder wrote past EOF buffer");
                    // SAFETY: same as above.
                    debug_assert!(
                        std::str::from_utf8(&buf[..written]).is_ok(),
                        "encoding_rs produced invalid UTF-8 at EOF"
                    );
                    let s = unsafe { std::str::from_utf8_unchecked(&buf[..written]) };
                    self.decoded_char_count += s.chars().count();
                    self.decoded.push_str(s);
                    self.eof = true;
                    self.exhausted.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Iterator over lines from a streaming response.
#[pyclass]
pub struct LinesIterator {
    resp: Arc<TMutex<Option<::primp::Response>>>,
    /// Raw bytes not yet decoded; may end mid-multibyte sequence.
    buffer: Vec<u8>,
    /// Stateful decoder that preserves incomplete multi-byte sequences
    /// across chunk boundaries (unlike `from_utf8_lossy` which emits
    /// U+FFFD for each incomplete tail).
    decoder: Decoder,
    /// Accumulated decoded text carried across `__next__` calls. Text is
    /// appended here when a chunk decode produces output without a newline,
    /// and cleared once a line is extracted or the stream is exhausted.
    decoded: String,
    done: bool,
    /// Response-level flag shared with the Response (see `BytesIterator`).
    exhausted: Arc<AtomicBool>,
}

impl LinesIterator {
    fn new(
        resp: Arc<TMutex<Option<::primp::Response>>>,
        encoding: String,
        exhausted: Arc<AtomicBool>,
    ) -> Self {
        let encoding = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
        LinesIterator {
            resp,
            buffer: Vec::with_capacity(8192),
            decoder: encoding.new_decoder(),
            decoded: String::new(),
            // An iterator created after a full-body drain must raise
            // StopIteration on the first `__next__`, not re-poll the
            // exhausted core.
            done: exhausted.load(Ordering::Relaxed),
            exhausted,
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
            // Check for a newline in already-decoded text from a previous
            // iteration before attempting to decode more from the buffer.
            if let Some(newline_pos) = self.decoded.find('\n') {
                let line_str = self.decoded[..=newline_pos]
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_owned();
                // Keep the rest in `decoded` (not the raw buffer) to avoid
                // round-tripping through UTF-8 bytes, which corrupts non-UTF-8
                // encodings and can orphan incomplete multi-byte decoder state.
                self.decoded.drain(..=newline_pos);
                return Ok(Some(line_str.into_pyobject_or_pyerr(py)?));
            }

            // Decode buffered bytes into self.decoded, preserving
            // incomplete multi-byte sequences across chunk boundaries.
            let mut input_pos = 0usize;
            loop {
                let remaining = &self.buffer[input_pos..];
                if remaining.is_empty() {
                    break;
                }
                let buf_size = remaining.len().max(64);
                let mut out = vec![0u8; buf_size];
                let (_result, read, written, _replaced) =
                    self.decoder.decode_to_utf8(remaining, &mut out, false);
                if written > 0 {
                    // SAFETY: encoding_rs always emits valid UTF-8.
                    debug_assert!(
                        std::str::from_utf8(&out[..written]).is_ok(),
                        "encoding_rs produced invalid UTF-8 in LinesIterator"
                    );
                    let s = unsafe { std::str::from_utf8_unchecked(&out[..written]) };
                    self.decoded.push_str(s);
                }
                input_pos += read;
                if read == 0 {
                    break;
                }
            }
            self.buffer.drain(..input_pos);

            // Check again after decoding new bytes.
            if let Some(newline_pos) = self.decoded.find('\n') {
                let line_str = self.decoded[..=newline_pos]
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_owned();
                self.decoded.drain(..=newline_pos);
                return Ok(Some(line_str.into_pyobject_or_pyerr(py)?));
            }

            // A drain by another consumer since creation must stop us quietly, not re-poll.
            if self.exhausted.load(Ordering::Relaxed) {
                self.done = true;
            }
            if self.done {
                if !self.decoded.is_empty() {
                    let remaining = std::mem::take(&mut self.decoded);
                    return Ok(Some(remaining.into_pyobject_or_pyerr(py)?));
                }
                return Err(pyo3::exceptions::PyStopIteration::new_err(
                    "Stream exhausted",
                ));
            }

            let resp = Arc::clone(&self.resp);
            let runtime = crate::get_runtime(py)?;
            let chunk = py.detach(|| {
                runtime.block_on(async {
                    let mut resp_guard = resp.lock().await;
                    match resp_guard.as_mut() {
                        Some(r) => match r.chunk().await {
                            Ok(Some(data)) => Ok::<Option<Vec<u8>>, PyErr>(Some(data.to_vec())),
                            Ok(None) => Ok(None),
                            Err(e) if e.is_stream_exhausted() => Ok(None),
                            Err(e) => Err(primp_error_to_pyerr(e)),
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
                    self.exhausted.store(true, Ordering::Relaxed);
                    // Flush the decoder's internal state so any incomplete
                    // multi-byte sequence at the end of the stream is emitted
                    // as U+FFFD rather than silently dropped.
                    let mut out = [0u8; 64];
                    let (_result, _read, written, _replaced) =
                        self.decoder.decode_to_utf8(&[], &mut out, true);
                    if written > 0 {
                        debug_assert!(
                            std::str::from_utf8(&out[..written]).is_ok(),
                            "encoding_rs produced invalid UTF-8 at EOF in LinesIterator"
                        );
                        let s = unsafe { std::str::from_utf8_unchecked(&out[..written]) };
                        self.decoded.push_str(s);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use encoding_rs::UTF_8;

    /// `iter_bytes(1 << 30)` is legal (`chunk_size` ≤ MAX_CHUNK_SIZE), so the
    /// buffer must not eagerly reserve 2 GiB — an abort under `panic = "abort"`
    /// on a memory-constrained host.
    #[test]
    fn bytes_iterator_reserves_no_memory_at_cap() {
        use super::BytesIterator;
        let resp = std::sync::Arc::new(tokio::sync::Mutex::new(None::<::primp::Response>));
        let exhausted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let it = BytesIterator::new(resp, 1 << 30, exhausted);
        assert_eq!(
            it.buffer.capacity(),
            0,
            "iterator must build with a lazy, zero-capacity buffer"
        );
    }

    /// Backs the 64-byte EOF-flush buffer in the streaming text iterators
    /// (`TextIterator`/`AsyncTextIterator`): the per-call output of
    /// `decode_to_utf8(&[], buf, true)` on a stuck decoder must fit in
    /// `buf.len()`. If this ever exceeds 4 bytes, widen the iterator buffers
    /// at `response.rs:403` and `async/response.rs:462`.
    #[test]
    fn eof_buffer_is_sufficient_for_stuck_state() {
        // Force a stuck state via a truncated UTF-8 sequence.
        let mut dec = UTF_8.new_decoder();
        let mut scratch = [0u8; 4];
        let _ = dec.decode_to_utf8(b"\xE2\x82", &mut scratch, false);

        let mut eof_buf = [0u8; 64];
        let (_result, _read, written, _replaced) = dec.decode_to_utf8(&[], &mut eof_buf, true);
        assert!(written <= eof_buf.len(), "EOF flush wrote {written} bytes");
        assert!(
            written <= 4,
            "EOF flush emitted more than expected: {written}"
        );
        let s = std::str::from_utf8(&eof_buf[..written]).expect("valid utf-8");
        assert!(
            s.chars().any(|c| c == '\u{FFFD}'),
            "expected a U+FFFD in EOF output"
        );
    }
}
