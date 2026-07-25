//! HTTP/1.1 client implementations.
//!
//! Holds the legacy `hyper_util` HTTP/1.1 client (plain HTTP, HTTP-over-proxy,
//! and the `Http1`/`Http2` prefs) and the [`Http1Pool`] ALPN `http/1.1` fallback
//! used when `http_version_pref == All`.

pub(crate) mod connect;
pub(crate) mod pool;

pub(crate) use connect::{build_legacy_http1_client, LegacyClientSettings};
pub(crate) use pool::Http1Pool;
