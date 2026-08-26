//! Python exception hierarchy for primp, mapping native error types via
//! `is_*()` methods rather than parsing error messages.
//!
//! ```text
//! PrimpError (base exception)
//! ├── BuilderError
//! ├── RequestError
//! │   ├── ConnectError
//! │   ├── TimeoutError
//! │   └── DNSError
//! │       └── DNSTimeoutError  (multiple inheritance: DNSError + TimeoutError)
//! ├── StatusError
//! ├── RedirectError
//! ├── BodyError
//! ├── DecodeError
//! └── UpgradeError
//! ```
//!
//! `DNSTimeoutError` is built once at module init here
//! (`init_dnstimeout_error`, via `type()` for multiple inheritance), and
//! cached as a `PyOnceLock<Py<PyType>>` for GIL-free per-error conversion.

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyAny, PyAnyMethods, PyDict, PyModule, PyModuleMethods, PyType};
use pyo3::{Bound, Py, PyErr, PyResult, Python};

use std::error::Error;
use std::fmt;
use std::io;

// =============================================================================
// Exception Hierarchy - Mapping to primp native error types
// =============================================================================

create_exception!(primp, PrimpError, PyException);
create_exception!(primp, BuilderError, PrimpError);
create_exception!(primp, RequestError, PrimpError);
create_exception!(primp, ConnectError, RequestError);
create_exception!(primp, TimeoutError, RequestError);
create_exception!(primp, DNSError, RequestError);
create_exception!(primp, StatusError, PrimpError);
create_exception!(primp, RedirectError, PrimpError);
create_exception!(primp, BodyError, PrimpError);
create_exception!(primp, DecodeError, PrimpError);
create_exception!(primp, UpgradeError, PrimpError);

// =============================================================================
// PrimpErrorEnum (for internal error handling)
// =============================================================================

/// Custom error enum for primp that wraps various error types.
#[derive(Debug)]
pub enum PrimpErrorEnum {
    /// primp error.
    PrimpError(::primp::Error),
    /// IO error (e.g., file operations).
    Io(io::Error),
    /// HTTP header invalid error.
    HttpHeaderInvalid(http::header::InvalidHeaderValue),
    /// HTTP method invalid error.
    HttpMethodInvalid(http::method::InvalidMethod),
    /// HTTP header to str error.
    HttpHeaderToStrError(http::header::ToStrError),
    /// Anyhow error (from helper modules).
    Anyhow(anyhow::Error),
    /// Custom error message.
    Custom(String),
    /// HTTP status error (4xx/5xx).
    HttpStatus(u16, String, String),
    /// Invalid header value.
    InvalidHeaderValue(String),
    /// Invalid URL.
    InvalidURL(String),
}

impl fmt::Display for PrimpErrorEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimpErrorEnum::PrimpError(e) => write!(f, "{}", e),
            PrimpErrorEnum::Io(e) => write!(f, "IO error: {}", e),
            PrimpErrorEnum::HttpHeaderInvalid(e) => write!(f, "Invalid HTTP header: {}", e),
            PrimpErrorEnum::HttpMethodInvalid(e) => write!(f, "Invalid HTTP method: {}", e),
            PrimpErrorEnum::HttpHeaderToStrError(e) => {
                write!(f, "HTTP header to string error: {}", e)
            }
            PrimpErrorEnum::Anyhow(e) => write!(f, "{}", e),
            PrimpErrorEnum::Custom(e) => write!(f, "{}", e),
            PrimpErrorEnum::HttpStatus(status, reason, url) => {
                write!(f, "HTTP {} {} for URL: {}", status, reason, url)
            }
            PrimpErrorEnum::InvalidHeaderValue(e) => write!(f, "Invalid header value: {}", e),
            PrimpErrorEnum::InvalidURL(e) => write!(f, "Invalid URL: {}", e),
        }
    }
}

impl std::error::Error for PrimpErrorEnum {}

impl From<::primp::Error> for PrimpErrorEnum {
    fn from(e: ::primp::Error) -> Self {
        PrimpErrorEnum::PrimpError(e)
    }
}

impl From<io::Error> for PrimpErrorEnum {
    fn from(e: io::Error) -> Self {
        PrimpErrorEnum::Io(e)
    }
}

impl From<http::header::InvalidHeaderValue> for PrimpErrorEnum {
    fn from(e: http::header::InvalidHeaderValue) -> Self {
        PrimpErrorEnum::HttpHeaderInvalid(e)
    }
}

impl From<http::method::InvalidMethod> for PrimpErrorEnum {
    fn from(e: http::method::InvalidMethod) -> Self {
        PrimpErrorEnum::HttpMethodInvalid(e)
    }
}

impl From<http::header::ToStrError> for PrimpErrorEnum {
    fn from(e: http::header::ToStrError) -> Self {
        PrimpErrorEnum::HttpHeaderToStrError(e)
    }
}

impl From<anyhow::Error> for PrimpErrorEnum {
    fn from(e: anyhow::Error) -> Self {
        PrimpErrorEnum::Anyhow(e)
    }
}

impl From<String> for PrimpErrorEnum {
    fn from(e: String) -> Self {
        PrimpErrorEnum::Custom(e)
    }
}

impl From<&str> for PrimpErrorEnum {
    fn from(e: &str) -> Self {
        PrimpErrorEnum::Custom(e.to_string())
    }
}

impl From<pythonize::PythonizeError> for PrimpErrorEnum {
    fn from(e: pythonize::PythonizeError) -> Self {
        PrimpErrorEnum::Anyhow(anyhow::anyhow!("{}", e))
    }
}

impl From<url::ParseError> for PrimpErrorEnum {
    fn from(e: url::ParseError) -> Self {
        PrimpErrorEnum::InvalidURL(e.to_string())
    }
}

impl From<PrimpErrorEnum> for PyErr {
    fn from(e: PrimpErrorEnum) -> Self {
        convert_primp_error_attached(e)
    }
}

/// Result type for primp functions that may fail.
pub type PrimpResult<T> = std::result::Result<T, PrimpErrorEnum>;

// =============================================================================
// Error Conversion Functions
// =============================================================================

/// Format an error with its full source chain for better debugging.
/// Bounded to 64 steps to match the Rust core's cycle-safe pattern.
fn format_with_source(err: &::primp::Error) -> String {
    let mut msg = err.to_string();
    let mut source: Option<&(dyn Error + 'static)> = err.source();
    for _ in 0..64 {
        match source {
            Some(s) => {
                let s_msg: String = s.to_string();
                if !msg.contains(&s_msg) {
                    msg.push_str(" > ");
                    msg.push_str(&s_msg);
                }
                source = s.source();
            }
            None => break,
        }
    }
    msg
}

/// The combined `DNSTimeoutError` type, built once at module init and cached
/// here so per-error conversion never needs `py.import("primp")`/`Python::attach`
/// (the crash class on a tokio worker after interpreter finalize).
static DNSTIMEOUT_ERROR_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();

/// Build and register `DNSTimeoutError` (DNSError + TimeoutError) at module
/// init, caching the type for [`converted_errors_with_py`]-style paths.
pub fn init_dnstimeout_error(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let locals = PyDict::new(py);
    locals.set_item("dns_err", py.get_type::<DNSError>().clone())?;
    locals.set_item("timeout_err", py.get_type::<TimeoutError>().clone())?;
    let combined: Bound<'_, PyAny> = py.eval(
        c"type('DNSTimeoutError', (dns_err, timeout_err), {'__module__': 'primp'})",
        None,
        Some(&locals),
    )?;
    let _ = DNSTIMEOUT_ERROR_TYPE.set(py, combined.clone().cast_into::<PyType>()?.unbind());
    m.add("DNSTimeoutError", combined)
}

/// Convert a `::primp::Error` to the matching Python exception via native
/// `is_*()` methods (no message parsing). Falls back to `PrimpError`.
///
/// A `StreamExhausted` *error* (polled past EOF) maps to `PrimpError`, not
/// `StopIteration` — clean end-of-stream is handled by the iterators.
pub(crate) fn primp_error_to_pyerr_with_py(py: Python<'_>, err: ::primp::Error) -> PyErr {
    let url = err.url().map(|u| u.to_string());

    // Use native primp error API - NO message parsing!

    // Builder errors (includes URL and header errors)
    if err.is_builder() {
        let message = err.to_string();
        return BuilderError::new_err((message, url));
    }

    // Status errors (HTTP 4xx/5xx)
    if err.is_status() {
        let message = err.to_string();
        let status_code = err.status().map(|s| s.as_u16()).unwrap_or(0);
        return StatusError::new_err((status_code, message, url));
    }

    // Redirect errors
    if err.is_redirect() {
        let message = err.to_string();
        return RedirectError::new_err((message, url));
    }

    // Include source chain for request-level errors (connect, timeout, generic)
    let message = format_with_source(&err);

    // DNS *timeouts* are both: raise the combined DNSTimeoutError (built in
    // lib.rs) before plain is_timeout() so the DNSError parent isn't lost;
    // fall back to TimeoutError if the combined type can't be retrieved.
    if err.is_dns() && err.is_timeout() {
        return match DNSTIMEOUT_ERROR_TYPE.get(py) {
            Some(ty) => match ty.bind(py).call1((message.clone(),)) {
                Ok(instance) => PyErr::from_value(instance),
                Err(_) => TimeoutError::new_err(message),
            },
            None => TimeoutError::new_err(message),
        };
    }

    // Timeout errors (child of RequestError)
    if err.is_timeout() {
        return TimeoutError::new_err(message);
    }

    // DNS errors (child of RequestError) — plain resolution failures. The core
    // `is_connect()` short-circuits on `is_dns()`, so these never reach the
    // ConnectError branch below.
    if err.is_dns() {
        return DNSError::new_err(message);
    }

    // Connect errors (child of RequestError)
    if err.is_connect() {
        return ConnectError::new_err(message);
    }

    // Request errors (generic)
    if err.is_request() {
        return RequestError::new_err(message);
    }

    // Decode errors
    if err.is_decode() {
        return DecodeError::new_err((message, url));
    }

    // Body errors
    if err.is_body() {
        return BodyError::new_err((message, url));
    }

    // JSON request-body serialization errors (request .json() failures)
    if err.is_json() {
        let message = err.to_string();
        return BuilderError::new_err((message, url));
    }

    // Note: clean end-of-stream is signalled by the streaming iterators
    // themselves (they translate `Ok(None)` from `chunk()` into
    // `StopIteration`/`StopAsyncIteration`). A `StreamExhausted` *error*
    // reaching here means `chunk()` was polled after the body already ended —
    // a genuine misuse — so it falls through to `PrimpError` below rather than
    // being masked as a clean stop (which also mis-fired `StopAsyncIteration`
    // in synchronous iterators).

    // Upgrade errors
    if err.is_upgrade() {
        return UpgradeError::new_err((message, url));
    }

    // Default fallback
    PrimpError::new_err((message, url))
}

/// Convert a `::primp::Error` to the matching Python exception, attaching the
/// GIL. For SYNC (GIL-held) callers only — async bridge tasks must use
/// [`primp_error_to_pyerr_with_py`] (the delivery closure already holds the
/// GIL; attaching from a tokio worker is the documented crash class).
pub fn primp_error_to_pyerr(err: ::primp::Error) -> PyErr {
    Python::attach(|py| primp_error_to_pyerr_with_py(py, err))
}

/// Convert a PrimpErrorEnum to the appropriate Python exception.
pub fn convert_primp_error(py: Python<'_>, err: PrimpErrorEnum) -> PyErr {
    match err {
        PrimpErrorEnum::PrimpError(error) => primp_error_to_pyerr_with_py(py, error),
        PrimpErrorEnum::Io(_) => PrimpError::new_err(err.to_string()),
        PrimpErrorEnum::HttpHeaderInvalid(_) => {
            BuilderError::new_err(format!("Header error: {}", err))
        }
        PrimpErrorEnum::HttpMethodInvalid(_) => PrimpError::new_err(err.to_string()),
        PrimpErrorEnum::HttpHeaderToStrError(_) => {
            BuilderError::new_err(format!("Header error: {}", err))
        }
        PrimpErrorEnum::Anyhow(_) => PrimpError::new_err(err.to_string()),
        PrimpErrorEnum::Custom(msg) => PrimpError::new_err(msg),
        PrimpErrorEnum::HttpStatus(status, reason, url) => {
            let message = format!("HTTP {} {} for URL: {}", status, reason, url);
            StatusError::new_err((status, message, Some(url)))
        }
        PrimpErrorEnum::InvalidHeaderValue(msg) => {
            BuilderError::new_err(format!("Header error: {}", msg))
        }
        PrimpErrorEnum::InvalidURL(msg) => BuilderError::new_err(format!("URL error: {}", msg)),
    }
}

/// GIL-attaching variant of [`convert_primp_error`] for sync (GIL-held) paths
/// that convert via a bare `From` impl (e.g. `?` in pyfunctions).
pub fn convert_primp_error_attached(err: PrimpErrorEnum) -> PyErr {
    Python::attach(|py| convert_primp_error(py, err))
}

/// Heuristically detect decode/decompression errors from a message string
/// (used when only the message is available, not a `primp::Error`).
/// Matches keywords like "gzip", "deflate", "decompression", "corrupt", etc.
pub fn is_decode_error_message(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    lower.contains("gzip")
        || lower.contains("deflate")
        || lower.contains("decompression")
        || lower.contains("decoding")
        || lower.contains("invalid header")
        || lower.contains("corrupt")
        || lower.contains("truncated")
        || lower.contains("incorrect header check")
}

/// Map a body-collection error message to `DecodeError` (decompression) or
/// `BodyError`.
#[allow(dead_code)]
pub fn body_collection_error(error_msg: &str) -> PyErr {
    if is_decode_error_message(error_msg) {
        DecodeError::new_err(format!("Body collection error: {}", error_msg))
    } else {
        BodyError::new_err(format!("Body collection error: {}", error_msg))
    }
}

/// Body error → Python exception via `is_*()`, preserving chain.
/// Checks `is_timeout` before `is_decode` so mid-body timeouts become
/// `TimeoutError`, not `DecodeError`.
pub fn primp_body_error_to_pyerr(err: ::primp::Error) -> PyErr {
    // Keep the full chain for diagnostics, not just `err.to_string()` ("error
    // decoding response body").
    let msg = format_with_source(&err);
    let url = err.url().map(|u| u.to_string());
    // Timeout is wrapped as `Kind::Decode` — check first.
    if err.is_timeout() {
        // No GIL for `DNSTimeoutError`; fallback to `TimeoutError`.
        return TimeoutError::new_err(msg);
    }
    if err.is_decode() {
        return DecodeError::new_err((msg, url));
    }
    if err.is_body() {
        return BodyError::new_err((msg, url));
    }
    // Fallback: keyword heuristic.
    if is_decode_error_message(&msg) {
        DecodeError::new_err((msg, url))
    } else {
        BodyError::new_err((msg, url))
    }
}
