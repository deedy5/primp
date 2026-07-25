//! Dedicated HTTP/2 client with connection pooling and ALPN negotiation.
//!
//! Owns the h2 connection engine: the connector that performs the TLS + h2
//! handshake (and reports when the server picks `http/1.1`), plus the h2
//! connection pool. HTTP/1.1 fallback paths live in [`super::h1_client`].

pub(crate) mod connect;
pub(crate) mod pool;

/// Sentinel error: the server did not negotiate HTTP/2 via ALPN, triggering
/// fallback to HTTP/1.1 when `http_version_pref == All`.
#[derive(Debug)]
pub(crate) struct H2NegotiationFailed;

impl std::fmt::Display for H2NegotiationFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("server did not negotiate h2 via ALPN")
    }
}

impl std::error::Error for H2NegotiationFailed {}

pub(crate) use pool::Pool;
