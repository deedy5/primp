use std::sync::Arc;

use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    IntoPyObjectExt,
};
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::error::{primp_body_error_to_pyerr, BodyError, DecodeError, PrimpErrorEnum};
use crate::traits::HeadersTraits;
use crate::utils::extract_encoding;

/// Collect body bytes from a response using a pre-allocated buffer.
pub async fn collect_body_bytes(resp: &mut ::primp::Response) -> Result<Bytes, PyErr> {
    let mut buf = Vec::with_capacity(8 * 1024);
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => buf.extend_from_slice(&chunk),
            Ok(None) => break Ok(Bytes::from(buf)),
            Err(e) => return Err(primp_body_error_to_pyerr(e)),
        }
    }
}

/// Byte index just past the n-th char of `s`.
///
/// Used by text iterators to split buffered text at exact char boundaries,
/// so multi-byte UTF-8 is never split mid-sequence.
pub fn chars_to_byte_pos(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len())
}

/// Upper bound for a user-supplied `chunk_size`.
///
/// Iterators grow their buffer lazily with the received body; the cap keeps a
/// single absurd chunk (e.g. `2**60`) from aborting on alloc failure under
/// `panic = "abort"`.
pub(crate) const MAX_CHUNK_SIZE: usize = 1 << 30; // 1 GiB

/// Validate a user-supplied `chunk_size` (bytes for `iter_bytes`, chars for
/// `iter_text`). Returns a `String` message so callers can build a PyErr
/// without the GIL; the crate never panics on numeric input.
pub(crate) fn parse_chunk_size(chunk_size: Option<usize>) -> Result<usize, String> {
    let size = chunk_size.unwrap_or(8192);
    if size == 0 {
        return Err("chunk_size must be greater than 0".to_string());
    }
    if size > MAX_CHUNK_SIZE {
        return Err(format!("chunk_size must be at most {MAX_CHUNK_SIZE}"));
    }
    Ok(size)
}

/// Raise HTTPError for 4xx/5xx status codes.
pub fn raise_for_status(status_code: u16, url: &str) -> PyResult<()> {
    if status_code >= 400 {
        let reason = if status_code < 600 {
            match status_code {
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
            status_code,
            reason.to_string(),
            url.to_string(),
        )));
    }
    Ok(())
}

/// Read response body bytes, blocking on the Tokio runtime.
pub fn read_body_bytes<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    py: Python<'py>,
) -> PyResult<Bytes> {
    let r = Arc::clone(resp);
    let runtime = crate::get_runtime(py)?;
    py.detach(|| {
        runtime.block_on(async {
            let mut guard = r.lock().await;
            match guard.as_mut() {
                Some(r) => collect_body_bytes(r).await,
                None => Err(BodyError::new_err(
                    "Response body already consumed or moved",
                )),
            }
        })
    })
}

/// Get response content as bytes, using cache for non-streaming.
pub fn get_content<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    content_cache: &mut Option<Py<PyBytes>>,
    streaming: bool,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyBytes>> {
    if !streaming {
        if let Some(content) = content_cache {
            return Ok(content.clone_ref(py).into_bound(py));
        }
    }

    let bytes: Bytes = read_body_bytes(resp, py)?;
    let content = PyBytes::new(py, &bytes);

    if !streaming {
        *content_cache = Some(content.clone().unbind());
    }
    Ok(content)
}

/// Get character encoding from response headers or cache.
pub fn get_encoding<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    encoding_cache: &mut Option<String>,
    py: Python<'py>,
) -> PyResult<String> {
    if let Some(encoding) = encoding_cache.as_ref() {
        return Ok(encoding.clone());
    }

    let r = Arc::clone(resp);
    let runtime = crate::get_runtime(py)?;
    let encoding: String = py.detach(|| {
        runtime.block_on(async {
            let guard = r.lock().await;
            match guard.as_ref() {
                Some(r) => Ok(extract_encoding(r.headers()).name().to_string()),
                None => Err(BodyError::new_err(
                    "Response body already consumed or moved",
                )),
            }
        })
    })?;

    *encoding_cache = Some(encoding.clone());
    Ok(encoding)
}

/// Get response text content.
pub fn text<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    content_cache: &mut Option<Py<PyBytes>>,
    encoding_cache: &mut Option<String>,
    streaming: bool,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyString>> {
    let content = get_content(resp, content_cache, streaming, py)?;
    let enc = get_encoding(resp, encoding_cache, py)?;
    let raw_bytes = content.as_bytes();
    let encoding = Encoding::for_label(enc.as_bytes()).unwrap_or(UTF_8);
    let (text, _, _) = encoding.decode(raw_bytes);
    text.into_pyobject_or_pyerr(py)
}

/// Parse response body as JSON.
pub fn json<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    content_cache: &mut Option<Py<PyBytes>>,
    streaming: bool,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let content = get_content(resp, content_cache, streaming, py)?;
    let raw_bytes = content.as_bytes();
    jiter::PythonParse::default()
        .python_parse(py, raw_bytes)
        .map_err(|e| {
            // Raise a combined JSONDecodeError (subclass of both DecodeError
            // and json.JSONDecodeError) so JSON parse failures are catchable
            // via both `except PrimpError` and `except json.JSONDecodeError`,
            // matching the `requests` library pattern. Falls back to plain
            // DecodeError if the combined type cannot be retrieved.
            let build = || -> PyResult<PyErr> {
                let primp_mod = py.import("primp")?;
                let err_type = primp_mod.getattr("JSONDecodeError")?;
                let msg = e.to_string();
                let doc = String::from_utf8_lossy(raw_bytes).to_string();
                let char_pos =
                    String::from_utf8_lossy(raw_bytes.get(..e.index).unwrap_or(raw_bytes))
                        .chars()
                        .count();
                let instance = err_type.call1((msg, doc, char_pos))?;
                Ok(PyErr::from_value(instance))
            };
            build().unwrap_or_else(|_| {
                DecodeError::new_err(format!("Invalid JSON: {e} (near byte {})", e.index))
            })
        })
}

/// Get response headers from cache or response.
pub fn get_headers<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    headers_cache: &mut Option<IndexMapSSR>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    if let Some(headers) = headers_cache {
        return headers.clone().into_pyobject(py);
    }

    let r = Arc::clone(resp);
    let runtime = crate::get_runtime(py)?;
    let headers: IndexMapSSR = py.detach(|| {
        runtime.block_on(async {
            let guard = r.lock().await;
            match guard.as_ref() {
                Some(r) => Ok(r.headers().to_indexmap()),
                None => Err(BodyError::new_err(
                    "Response body already consumed or moved",
                )),
            }
        })
    })?;

    let py_dict = headers.clone().into_pyobject(py)?;
    *headers_cache = Some(headers);
    Ok(py_dict)
}

/// Get response cookies from cache or response.
pub fn get_cookies<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    cookies_cache: &mut Option<IndexMapSSR>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    if let Some(cookies) = cookies_cache {
        return cookies.clone().into_pyobject(py);
    }

    let r = Arc::clone(resp);
    let runtime = crate::get_runtime(py)?;
    let cookies: IndexMapSSR = py.detach(|| {
        runtime.block_on(async {
            let guard = r.lock().await;
            match guard.as_ref() {
                Some(r) => Ok(crate::extract_cookies_to_indexmap(r.headers())),
                None => Err(BodyError::new_err(
                    "Response body already consumed or moved",
                )),
            }
        })
    })?;

    let py_dict = cookies.clone().into_pyobject(py)?;
    *cookies_cache = Some(cookies);
    Ok(py_dict)
}

/// Get HTML converted to Markdown.
pub fn text_markdown<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    content_cache: &mut Option<Py<PyBytes>>,
    streaming: bool,
    py: Python<'py>,
) -> PyResult<String> {
    let content = get_content(resp, content_cache, streaming, py)?;
    let raw_bytes = content.as_bytes();
    let text = py.detach(|| {
        html2text::from_read(raw_bytes, 100)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    })?;
    Ok(text)
}

/// Get HTML converted to plain text.
pub fn text_plain<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    content_cache: &mut Option<Py<PyBytes>>,
    streaming: bool,
    py: Python<'py>,
) -> PyResult<String> {
    use html2text::{from_read_with_decorator, render::TrivialDecorator};
    let content = get_content(resp, content_cache, streaming, py)?;
    let raw_bytes = content.as_bytes();
    let text = py.detach(|| {
        from_read_with_decorator(raw_bytes, 100, TrivialDecorator::new())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    })?;
    Ok(text)
}

/// Get HTML converted to rich text.
pub fn text_rich<'py>(
    resp: &Arc<TMutex<Option<::primp::Response>>>,
    content_cache: &mut Option<Py<PyBytes>>,
    streaming: bool,
    py: Python<'py>,
) -> PyResult<String> {
    use html2text::{from_read_with_decorator, render::RichDecorator};
    let content = get_content(resp, content_cache, streaming, py)?;
    let raw_bytes = content.as_bytes();
    let text = py.detach(|| {
        from_read_with_decorator(raw_bytes, 100, RichDecorator::new())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    })?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::{chars_to_byte_pos, parse_chunk_size, MAX_CHUNK_SIZE};

    #[test]
    fn n_zero_is_zero() {
        assert_eq!(chars_to_byte_pos("hello", 0), 0);
    }

    #[test]
    fn ascii_byte_index_matches_char_index() {
        assert_eq!(chars_to_byte_pos("hello", 1), 1);
        assert_eq!(chars_to_byte_pos("hello", 3), 3);
        assert_eq!(chars_to_byte_pos("hello", 5), 5);
    }

    #[test]
    fn n_beyond_len_returns_str_len() {
        assert_eq!(chars_to_byte_pos("hi", 10), 2);
    }

    #[test]
    fn n_exactly_len_returns_str_len() {
        assert_eq!(chars_to_byte_pos("hi", 2), 2);
    }

    #[test]
    fn multibyte_chars_skip_extra_bytes() {
        // "héllo" — 'é' is 2 bytes in UTF-8, total 6 bytes for 5 chars.
        let s = "héllo";
        assert_eq!(s.len(), 6);
        assert_eq!(chars_to_byte_pos(s, 1), 1);
        assert_eq!(chars_to_byte_pos(s, 2), 3);
        assert_eq!(chars_to_byte_pos(s, 5), 6);
    }

    #[test]
    fn four_byte_chars_skip_three_extra_bytes() {
        // "a😀b" — '😀' is 4 bytes in UTF-8, total 6 bytes for 3 chars.
        let s = "a😀b";
        assert_eq!(s.len(), 6);
        assert_eq!(chars_to_byte_pos(s, 1), 1);
        assert_eq!(chars_to_byte_pos(s, 2), 5);
        assert_eq!(chars_to_byte_pos(s, 3), 6);
    }

    #[test]
    fn empty_string() {
        assert_eq!(chars_to_byte_pos("", 0), 0);
        assert_eq!(chars_to_byte_pos("", 5), 0);
    }

    #[test]
    fn chunk_size_defaults_to_8192() {
        assert_eq!(parse_chunk_size(None).unwrap(), 8192);
    }

    #[test]
    fn chunk_size_zero_rejected() {
        let err = parse_chunk_size(Some(0)).unwrap_err();
        assert!(err.contains("greater than 0"), "unexpected error: {err}");
    }

    #[test]
    fn chunk_size_above_cap_rejected() {
        let err = parse_chunk_size(Some(1 << 40)).unwrap_err();
        assert!(err.contains("at most"), "unexpected error: {err}");
    }

    #[test]
    fn chunk_size_at_cap_accepted() {
        assert_eq!(
            parse_chunk_size(Some(MAX_CHUNK_SIZE)).unwrap(),
            MAX_CHUNK_SIZE
        );
    }

    #[test]
    fn chunk_size_just_below_cap_accepted() {
        assert_eq!(
            parse_chunk_size(Some(MAX_CHUNK_SIZE - 1)).unwrap(),
            MAX_CHUNK_SIZE - 1
        );
    }
}
