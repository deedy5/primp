use std::sync::Arc;

use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
    IntoPyObjectExt,
};
use pythonize::pythonize;
use serde_json::from_slice;
use tokio::sync::Mutex as TMutex;

use crate::client_builder::IndexMapSSR;
use crate::error::{body_collection_error, BodyError, PrimpErrorEnum};
use crate::traits::HeadersTraits;
use crate::utils::extract_encoding;

/// Collect body bytes from a response using a pre-allocated buffer.
pub async fn collect_body_bytes(resp: &mut ::primp::Response) -> Result<Bytes, PyErr> {
    let mut buf = Vec::with_capacity(8 * 1024);
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => buf.extend_from_slice(&chunk),
            Ok(None) => break Ok(Bytes::from(buf)),
            Err(e) => return Err(body_collection_error(&e.to_string())),
        }
    }
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
    let runtime = crate::get_runtime(py);
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
    let runtime = crate::get_runtime(py);
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
    let content = get_content(resp, content_cache, streaming, py)?.unbind();
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
    let runtime = crate::get_runtime(py);
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
    let runtime = crate::get_runtime(py);
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
