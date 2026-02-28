//! Error types for primp, providing a native primp-reqwest exception hierarchy.
//!
//! This module defines Python exceptions that directly map to primp-reqwest's
//! native error types using `is_*()` methods, without parsing error messages.
//!
//! # Exception Hierarchy
//!
//! ```text
//! PrimpError (base exception)
//! ├── BuilderError
//! ├── RequestError
//! │   ├── ConnectError
//! │   └── TimeoutError
//! ├── StatusError
//! ├── RedirectError
//! ├── BodyError
//! ├── DecodeError
//! └── UpgradeError
//! ```

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::PyErr;

use std::fmt;
use std::io;

// =============================================================================
// Exception Hierarchy - Mapping to primp-reqwest native error types
// =============================================================================

create_exception!(primp, PrimpError, PyException);
create_exception!(primp, BuilderError, PrimpError);
create_exception!(primp, RequestError, PrimpError);
create_exception!(primp, ConnectError, RequestError);
create_exception!(primp, TimeoutError, RequestError);
create_exception!(primp, StatusError, PrimpError);
create_exception!(primp, RedirectError, PrimpError);
create_exception!(primp, BodyError, PrimpError);
create_exception!(primp, DecodeError, PrimpError);
create_exception!(primp, UpgradeError, PrimpError);

// =============================================================================
// PrimpPyError - Simple wrapper for ::primp::Error conversion
// =============================================================================

/// Simple wrapper struct for converting ::primp::Error to Python exceptions.
///
/// This follows the impit pattern for error propagation.
pub(crate) struct PrimpPyError(pub ::primp::Error);

impl From<::primp::Error> for PrimpPyError {
    fn from(err: ::primp::Error) -> Self {
        PrimpPyError(err)
    }
}

impl From<PrimpPyError> for PyErr {
    fn from(err: PrimpPyError) -> PyErr {
        convert_reqwest_error(err.0)
    }
}

// =============================================================================
// PrimpErrorEnum (for internal error handling)
// =============================================================================

/// Custom error enum for primp that wraps various error types.
#[derive(Debug)]
pub enum PrimpErrorEnum {
    /// Reqwest error.
    Reqwest(::primp::Error),
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
    /// JSON parsing error.
    JsonError(String),
    /// Invalid header value.
    InvalidHeaderValue(String),
    /// Stream exhausted.
    StreamExhausted,
    /// Invalid URL.
    InvalidURL(String),
}

impl fmt::Display for PrimpErrorEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimpErrorEnum::Reqwest(e) => write!(f, "{}", e),
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
            PrimpErrorEnum::JsonError(e) => write!(f, "JSON error: {}", e),
            PrimpErrorEnum::InvalidHeaderValue(e) => write!(f, "Invalid header value: {}", e),
            PrimpErrorEnum::StreamExhausted => write!(f, "Stream exhausted"),
            PrimpErrorEnum::InvalidURL(e) => write!(f, "Invalid URL: {}", e),
        }
    }
}

impl std::error::Error for PrimpErrorEnum {}

impl From<::primp::Error> for PrimpErrorEnum {
    fn from(e: ::primp::Error) -> Self {
        PrimpErrorEnum::Reqwest(e)
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
        PrimpErrorEnum::Anyhow(anyhow::anyhow!("{}", e))
    }
}

impl From<serde_json::Error> for PrimpErrorEnum {
    fn from(e: serde_json::Error) -> Self {
        PrimpErrorEnum::JsonError(e.to_string())
    }
}

impl From<PrimpErrorEnum> for PyErr {
    fn from(e: PrimpErrorEnum) -> Self {
        convert_primp_error(e)
    }
}

/// Result type for primp functions that may fail.
pub type PrimpResult<T> = std::result::Result<T, PrimpErrorEnum>;

// =============================================================================
// Error Conversion Functions
// =============================================================================

/// Convert a ::primp::Error to the appropriate Python exception.
///
/// Uses native type-based detection via `is_*()` methods from primp-reqwest.
/// Does NOT parse error messages - uses the native primp-reqwest error API.
///
/// # Error Detection Order
///
/// 1. Builder errors (`is_builder()`) → BuilderError (includes URL and header errors)
/// 2. Status errors (`is_status()`) → StatusError
/// 3. Redirect errors (`is_redirect()`) → RedirectError
/// 4. Timeout errors (`is_timeout()`) → TimeoutError
/// 5. Connect errors (`is_connect()`) → ConnectError
/// 6. Request errors (`is_request()`) → RequestError
/// 7. Body errors (`is_body()`) → BodyError
/// 8. Decode errors (`is_decode()`) → DecodeError
/// 9. Upgrade errors (`is_upgrade()`) → UpgradeError
/// 10. Fallback → PrimpError
pub(crate) fn convert_reqwest_error(err: ::primp::Error) -> PyErr {
    let url = err.url().map(|u| u.to_string());
    let message = err.to_string();

    // Use native primp-reqwest error API - NO message parsing!

    // Builder errors (includes URL and header errors)
    if err.is_builder() {
        return BuilderError::new_err((message, url));
    }

    // Status errors (HTTP 4xx/5xx)
    if err.is_status() {
        let status_code = err.status().map(|s| s.as_u16()).unwrap_or(0);
        return StatusError::new_err((status_code, message, url));
    }

    // Redirect errors
    if err.is_redirect() {
        return RedirectError::new_err((message, url));
    }

    // Timeout errors (child of RequestError)
    if err.is_timeout() {
        return TimeoutError::new_err((message, url));
    }

    // Connect errors (child of RequestError)
    if err.is_connect() {
        return ConnectError::new_err((message, url));
    }

    // Request errors (generic)
    if err.is_request() {
        return RequestError::new_err((message, url));
    }

    // Decode errors
    if err.is_decode() {
        return DecodeError::new_err((message, url));
    }

    // Body errors
    if err.is_body() {
        return BodyError::new_err((message, url));
    }

    // Upgrade errors
    if err.is_upgrade() {
        return UpgradeError::new_err((message, url));
    }

    // Default fallback
    PrimpError::new_err((message, url))
}

/// Convert a PrimpErrorEnum to the appropriate Python exception.
pub fn convert_primp_error(err: PrimpErrorEnum) -> PyErr {
    match err {
        PrimpErrorEnum::Reqwest(error) => convert_reqwest_error(error),
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
        PrimpErrorEnum::JsonError(msg) => PrimpError::new_err(msg),
        PrimpErrorEnum::InvalidHeaderValue(msg) => {
            BuilderError::new_err(format!("Header error: {}", msg))
        }
        PrimpErrorEnum::StreamExhausted => {
            // Stream exhausted - raise StopAsyncIteration in Python
            pyo3::exceptions::PyStopAsyncIteration::new_err("The iterator is exhausted")
        }
        PrimpErrorEnum::InvalidURL(msg) => BuilderError::new_err(format!("URL error: {}", msg)),
    }
}

/// Check if an error message indicates a decode/decompression error.
///
/// This is used for body collection errors where we only have the error message
/// string and not a proper reqwest::Error with `is_decode()` method.
/// Decompression errors contain keywords like "gzip", "deflate", "decompression", etc.
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

/// Create the appropriate Python exception for a body collection error.
///
/// This checks if the error message indicates a decompression error and
/// returns DecodeError for those cases, otherwise returns BodyError.
pub fn body_collection_error(error_msg: &str) -> PyErr {
    if is_decode_error_message(error_msg) {
        DecodeError::new_err(format!("Body collection error: {}", error_msg))
    } else {
        BodyError::new_err(format!("Body collection error: {}", error_msg))
    }
}
