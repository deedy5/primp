//! Error types for primp, providing a requests-like exception hierarchy.
//!
//! This module defines Python exceptions that mirror the requests library,
//! allowing for specific error handling based on the type of failure.
//!
//! # Exception Hierarchy
//!
//! ```text
//! RequestException (base class)
//! ├── HTTPError                    # HTTP 4xx/5xx status codes
//! ├── ConnectionError              # All connection-related failures
//! │   ├── ConnectTimeout          # Connection establishment timeout
//! │   ├── SSLError                # SSL/TLS certificate errors
//! │   └── ProxyError              # Proxy connection errors
//! ├── Timeout                      # All timeout-related failures
//! │   └── ReadTimeout             # Data read timeout
//! ├── RequestError                 # Request building/sending errors
//! │   ├── InvalidURL              # URL format errors
//! │   └── InvalidHeader           # Header format errors
//! ├── BodyError                    # Body-related errors
//! │   ├── StreamConsumedError     # Stream already consumed
//! │   └── ChunkedEncodingError    # Chunked encoding errors
//! ├── DecodeError                  # Decoding errors
//! │   └── ContentDecodingError    # Content decode errors
//! ├── JSONError                    # JSON-related errors
//! │   ├── InvalidJSONError        # Invalid JSON in request
//! │   └── JSONDecodeError         # JSON decode in response
//! └── TooManyRedirects            # Redirect limit exceeded
//! ```

use pyo3::create_exception;
use pyo3::prelude::*;
use pyo3::PyErr;

use std::error::Error;
use std::fmt;
use std::io;

/// Context information for error enrichment.
///
/// Captures request metadata that can be used to provide more informative
/// error messages when requests fail.
#[derive(Default, Clone, Debug)]
pub struct ErrorContext {
    /// Request timeout duration in milliseconds.
    pub timeout: Option<u64>,
    /// Maximum redirects allowed.
    pub max_redirects: Option<usize>,
    /// HTTP method used.
    pub method: Option<String>,
    /// Request URL.
    pub url: Option<String>,
}

impl ErrorContext {
    /// Create a new error context with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout = Some(timeout_ms);
        self
    }

    /// Set the max redirects.
    pub fn with_max_redirects(mut self, max: usize) -> Self {
        self.max_redirects = Some(max);
        self
    }

    /// Set the HTTP method.
    pub fn with_method(mut self, method: &str) -> Self {
        self.method = Some(method.to_string());
        self
    }

    /// Set the URL.
    pub fn with_url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }
}

/// Custom error enum for primp that wraps various error types.
#[derive(Debug)]
pub enum PrimpError {
    /// Reqwest error with optional context for error enrichment.
    Reqwest {
        error: reqwest::Error,
        context: Option<ErrorContext>,
    },
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

impl fmt::Display for PrimpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimpError::Reqwest { error, context } => {
                if let Some(ctx) = context {
                    write!(
                        f,
                        "{} (context: timeout={:?}ms, max_redirects={:?})",
                        error, ctx.timeout, ctx.max_redirects
                    )
                } else {
                    write!(f, "{}", error)
                }
            }
            PrimpError::Io(e) => write!(f, "IO error: {}", e),
            PrimpError::HttpHeaderInvalid(e) => write!(f, "Invalid HTTP header: {}", e),
            PrimpError::HttpMethodInvalid(e) => write!(f, "Invalid HTTP method: {}", e),
            PrimpError::HttpHeaderToStrError(e) => write!(f, "HTTP header to string error: {}", e),
            PrimpError::Anyhow(e) => write!(f, "{}", e),
            PrimpError::Custom(e) => write!(f, "{}", e),
            PrimpError::HttpStatus(status, reason, url) => {
                write!(f, "HTTP {} {} for URL: {}", status, reason, url)
            }
            PrimpError::JsonError(e) => write!(f, "JSON error: {}", e),
            PrimpError::InvalidHeaderValue(e) => write!(f, "Invalid header value: {}", e),
            PrimpError::StreamExhausted => write!(f, "Stream exhausted"),
            PrimpError::InvalidURL(e) => write!(f, "Invalid URL: {}", e),
        }
    }
}

impl std::error::Error for PrimpError {}

impl PrimpError {
    /// Create a PrimpError from a reqwest error with context.
    pub fn from_reqwest(error: reqwest::Error, context: Option<ErrorContext>) -> Self {
        PrimpError::Reqwest { error, context }
    }

    /// Create a PrimpError from a reqwest error without context.
    pub fn from_reqwest_simple(error: reqwest::Error) -> Self {
        PrimpError::Reqwest {
            error,
            context: None,
        }
    }
}

impl From<reqwest::Error> for PrimpError {
    fn from(e: reqwest::Error) -> Self {
        PrimpError::from_reqwest_simple(e)
    }
}

impl From<io::Error> for PrimpError {
    fn from(e: io::Error) -> Self {
        PrimpError::Io(e)
    }
}

impl From<http::header::InvalidHeaderValue> for PrimpError {
    fn from(e: http::header::InvalidHeaderValue) -> Self {
        PrimpError::HttpHeaderInvalid(e)
    }
}

impl From<http::method::InvalidMethod> for PrimpError {
    fn from(e: http::method::InvalidMethod) -> Self {
        PrimpError::HttpMethodInvalid(e)
    }
}

impl From<http::header::ToStrError> for PrimpError {
    fn from(e: http::header::ToStrError) -> Self {
        PrimpError::HttpHeaderToStrError(e)
    }
}

impl From<anyhow::Error> for PrimpError {
    fn from(e: anyhow::Error) -> Self {
        PrimpError::Anyhow(e)
    }
}

impl From<String> for PrimpError {
    fn from(e: String) -> Self {
        PrimpError::Custom(e)
    }
}

impl From<&str> for PrimpError {
    fn from(e: &str) -> Self {
        PrimpError::Custom(e.to_string())
    }
}

impl From<pythonize::PythonizeError> for PrimpError {
    fn from(e: pythonize::PythonizeError) -> Self {
        PrimpError::Anyhow(anyhow::anyhow!("{}", e))
    }
}

impl From<url::ParseError> for PrimpError {
    fn from(e: url::ParseError) -> Self {
        PrimpError::Anyhow(anyhow::anyhow!("{}", e))
    }
}

impl From<serde_json::Error> for PrimpError {
    fn from(e: serde_json::Error) -> Self {
        PrimpError::JsonError(e.to_string())
    }
}

impl From<PrimpError> for PyErr {
    fn from(e: PrimpError) -> Self {
        convert_primp_error(e)
    }
}

/// Result type for primp functions that may fail.
/// This type alias replaces `anyhow::Result<T>` to enable proper error conversion
/// to Python exceptions using `convert_primp_error()`.
pub type PrimpResult<T> = std::result::Result<T, PrimpError>;

// RequestException is the base exception for all primp errors.
// PrimpError is exported as an alias for RequestException in lib.rs for Python users.
create_exception!(primp, RequestException, pyo3::exceptions::PyException);
create_exception!(primp, HTTPError, RequestException);
create_exception!(primp, ConnectionError, RequestException);
create_exception!(primp, Timeout, RequestException);
create_exception!(primp, TooManyRedirects, RequestException);
create_exception!(primp, RequestError, RequestException);

// New parent exceptions for better hierarchy
create_exception!(primp, BodyError, RequestException);
create_exception!(primp, DecodeError, RequestException);
create_exception!(primp, JSONError, RequestException);

// Body-related exceptions (now under BodyError)
create_exception!(primp, StreamConsumedError, BodyError);
create_exception!(primp, ChunkedEncodingError, BodyError);

// Decode-related exceptions (now under DecodeError)
create_exception!(primp, ContentDecodingError, DecodeError);

// JSON-related exceptions (now under JSONError)
create_exception!(primp, InvalidJSONError, JSONError);
create_exception!(primp, JSONDecodeError, JSONError);

// Connection-related exceptions
create_exception!(primp, ProxyError, ConnectionError);
create_exception!(primp, SSLError, ConnectionError);
create_exception!(primp, ConnectTimeout, ConnectionError);

// Timeout-related exceptions
create_exception!(primp, ReadTimeout, Timeout);

// URL and Header exceptions (under RequestError)
create_exception!(primp, InvalidURL, RequestError);
create_exception!(primp, InvalidHeader, RequestError);

/// Extract the URL from a reqwest error if available.
fn extract_url(err: &reqwest::Error) -> Option<String> {
    err.url().map(|u| u.to_string())
}

/// Extract the status code from a reqwest error if it's a status error.
fn extract_status_code(err: &reqwest::Error) -> Option<u16> {
    err.status().map(|s| s.as_u16())
}

/// Extract the reason phrase from a reqwest error if it's a status error.
fn extract_reason(err: &reqwest::Error) -> Option<String> {
    err.status().map(|s| {
        s.canonical_reason()
            .map(|r| r.to_string())
            .unwrap_or_else(|| s.as_u16().to_string())
    })
}

// =============================================================================
// Error Detection Helpers
// =============================================================================

/// Walk the error source chain and check if any error matches the predicate.
fn error_chain_matches<F>(mut err: &dyn Error, predicate: F) -> bool
where
    F: Fn(&dyn Error) -> bool,
{
    loop {
        if predicate(err) {
            return true;
        }
        match err.source() {
            Some(source) => err = source,
            None => return false,
        }
    }
}

/// Check if any error in the source chain contains specific keywords (case-insensitive).
fn error_chain_contains_keywords(err: &dyn Error, keywords: &[&str]) -> bool {
    error_chain_matches(err, |e| {
        let err_str = e.to_string().to_lowercase();
        keywords.iter().any(|keyword| err_str.contains(&keyword.to_lowercase()))
    })
}

/// SSL/TLS error keywords.
const SSL_KEYWORDS: &[&str] = &["certificate", "ssl", "tls", "handshake", "x509", "pkix"];

/// Proxy error keywords.
const PROXY_KEYWORDS: &[&str] = &["proxy", "socks"];

/// Check if the error is a connect timeout error.
fn is_connect_timeout(err: &reqwest::Error) -> bool {
    err.is_timeout() && error_chain_contains_keywords(err, &["connect"])
}

/// Check if the error is an SSL/TLS error.
fn is_ssl_error(err: &reqwest::Error) -> bool {
    error_chain_contains_keywords(err, SSL_KEYWORDS)
}

/// Check if the error is a proxy error.
fn is_proxy_error(err: &reqwest::Error) -> bool {
    error_chain_contains_keywords(err, PROXY_KEYWORDS)
}

/// Check if a message contains any of the given patterns (case-insensitive).
fn message_matches_patterns(message: &str, patterns: &[&str]) -> bool {
    let lower = message.to_lowercase();
    patterns.iter().any(|p| lower.contains(&p.to_lowercase()))
}

/// URL-related error patterns.
const URL_ERROR_PATTERNS: &[&str] = &[
    "MissingSchema", "missing scheme", "no scheme",
    "InvalidSchema", "invalid scheme", "unsupported scheme",
    "InvalidURL", "invalid url"
];

/// Header-related error patterns.
const HEADER_ERROR_PATTERNS: &[&str] = &["InvalidHeader", "invalid header"];

/// Categorize error message into URL, header, or chunked encoding error types.
/// Returns the appropriate PyErr for builder/request errors.
fn categorize_builder_error(message: &str, url: Option<String>) -> PyErr {
    if message_matches_patterns(message, URL_ERROR_PATTERNS) {
        return InvalidURL::new_err((message.to_string(), url));
    }
    
    if message_matches_patterns(message, HEADER_ERROR_PATTERNS) {
        return InvalidHeader::new_err((message.to_string(), url));
    }
    
    RequestError::new_err((message.to_string(), url))
}

/// Convert a reqwest::Error to the appropriate Python exception.
///
/// Uses type-based detection via `is_*()` methods and source chain inspection
/// instead of string parsing where possible. This provides more reliable error
/// categorization that is resilient to error message format changes.
///
/// # Error Detection Order
///
/// 1. HTTP status errors (`is_status()`)
/// 2. Redirect errors (`is_redirect()`)
/// 3. Timeout errors (`is_timeout()`) - with connect/read distinction
/// 4. SSL errors (source chain inspection)
/// 5. Proxy errors (source chain inspection)
/// 6. Connection errors (`is_connect()`)
/// 7. Builder/Request errors (`is_builder()`, `is_request()`)
/// 8. Decode errors (`is_decode()`)
/// 9. Body errors (`is_body()`)
/// 10. Upgrade errors (`is_upgrade()`)
///
/// # Context Enrichment
///
/// When context is provided, error messages are enriched with request metadata:
/// - Timeout errors include the timeout value in milliseconds
/// - Redirect errors include the maximum redirects allowed
/// - Connection errors include the URL
pub fn convert_reqwest_error(err: reqwest::Error, context: Option<&ErrorContext>) -> PyErr {
    let message = err.to_string();
    // Use context URL if available, otherwise extract from error
    let url = context
        .and_then(|c| c.url.clone())
        .or_else(|| extract_url(&err));

    // 1. HTTP status errors - use is_status()
    if err.is_status() {
        let status_code = extract_status_code(&err).unwrap_or(0);
        let reason = extract_reason(&err);
        return HTTPError::new_err((message, status_code, url, reason));
    }

    // 2. Redirect errors - use is_redirect() with context enrichment
    if err.is_redirect() {
        let max = context.and_then(|c| c.max_redirects);
        let enriched_message = max
            .map(|m| format!("Too many redirects (max {})", m))
            .unwrap_or(message);
        return TooManyRedirects::new_err((enriched_message, url));
    }

    // 3. Timeout errors - use is_timeout() with source inspection and context enrichment
    if err.is_timeout() {
        // Check for connect timeout first (more specific)
        if is_connect_timeout(&err) {
            return ConnectTimeout::new_err((message, url));
        }

        // Enrich message with timeout value from context
        let timeout_msg = context
            .and_then(|c| c.timeout)
            .map(|t| format!("Request timeout ({} ms) exceeded", t))
            .unwrap_or_else(|| message.clone());

        // Check for read timeout pattern
        if message_matches_patterns(&message, &["read", "timeout"]) {
            return ReadTimeout::new_err((timeout_msg, url));
        }
        // Generic timeout
        return Timeout::new_err((timeout_msg, url));
    }

    // 4. SSL errors - inspect source chain (check before connection errors as more specific)
    if is_ssl_error(&err) {
        return SSLError::new_err((message, url));
    }

    // 5. Proxy errors - inspect source chain (check before connection errors as more specific)
    if is_proxy_error(&err) {
        return ProxyError::new_err((message, url));
    }

    // 6. Connection errors - use is_connect() with context enrichment
    if err.is_connect() {
        let enriched_message = url
            .as_ref()
            .map(|u| format!("Connection error for URL: {}", u))
            .unwrap_or(message);
        return ConnectionError::new_err((enriched_message, url));
    }

    // 7. Builder/Request errors - use is_builder()/is_request()
    if err.is_builder() || err.is_request() {
        return categorize_builder_error(&message, url);
    }

    // 8. Decode errors - use is_decode()
    if err.is_decode() {
        return ContentDecodingError::new_err((message, url));
    }

    // 9. Body errors - use is_body()
    if err.is_body() {
        // Check for chunked encoding errors
        if message_matches_patterns(&message, &["ChunkedEncodingError", "chunked encoding"]) {
            return ChunkedEncodingError::new_err((message, url));
        }
        return StreamConsumedError::new_err((message, url));
    }

    // 10. Upgrade errors - map to RequestError (simplified hierarchy)
    if err.is_upgrade() {
        return RequestError::new_err((message, url));
    }

    // Fallback to base RequestException
    RequestException::new_err((message, url))
}

/// Convert a PrimpError to the appropriate Python exception.
pub fn convert_primp_error(err: PrimpError) -> PyErr {
    match err {
        PrimpError::Reqwest { error, context } => convert_reqwest_error(error, context.as_ref()),
        PrimpError::Io(_) => RequestError::new_err(err.to_string()),
        PrimpError::HttpHeaderInvalid(_) => RequestError::new_err(err.to_string()),
        PrimpError::HttpMethodInvalid(_) => RequestError::new_err(err.to_string()),
        PrimpError::HttpHeaderToStrError(_) => RequestError::new_err(err.to_string()),
        PrimpError::Anyhow(_) => RequestError::new_err(err.to_string()),
        PrimpError::Custom(msg) => RequestException::new_err(msg),
        // Handle HTTP status errors (4xx/5xx)
        PrimpError::HttpStatus(status, reason, url) => {
            let message = format!("HTTP {} {} for URL: {}", status, reason, url);
            HTTPError::new_err((message, status, Some(url), Some(reason)))
        }
        PrimpError::JsonError(msg) => InvalidJSONError::new_err(msg),
        PrimpError::InvalidHeaderValue(msg) => InvalidHeader::new_err(msg),
        PrimpError::StreamExhausted => {
            // Stream exhausted - raise StopAsyncIteration in Python
            // This is handled in the async stream __anext__ method
            pyo3::exceptions::PyStopAsyncIteration::new_err("The iterator is exhausted")
        }
        PrimpError::InvalidURL(msg) => InvalidURL::new_err(msg),
    }
}

/// Create a JSONDecodeError with proper pickling support.
///
/// This function creates a Python json.JSONDecodeError with all the
/// necessary attributes for proper exception handling and pickling.
pub fn create_json_decode_error(
    msg: String,
    doc: String,
    pos: usize,
    lineno: usize,
    colno: usize,
) -> PyErr {
    Python::attach(|py| {
        let json_module = py.import("json").unwrap();
        let error_type = json_module.getattr("JSONDecodeError").unwrap();
        let error = error_type.call1((&msg, &doc, pos)).unwrap();

        // Set additional attributes for compatibility
        error.setattr("msg", msg).unwrap();
        error.setattr("doc", doc).unwrap();
        error.setattr("pos", pos).unwrap();
        error.setattr("lineno", lineno).unwrap();
        error.setattr("colno", colno).unwrap();

        PyErr::from_value(error)
    })
}
