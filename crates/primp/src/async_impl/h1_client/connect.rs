//! Legacy `hyper_util` HTTP/1.1 client.
//!
//! The `hyper_util::client::legacy::Client` for plain HTTP, HTTP-over-proxy,
//! and the `Http1`/`Http2` prefs. Its config is captured up-front into
//! [`LegacyClientSettings`] (read from [`crate::async_impl::client::Config`]
//! before `Config`'s fields are partially moved during construction).

use std::time::Duration;

use crate::async_impl::body::Body;
use crate::async_impl::client::Config;
use crate::connect::Connector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioTimer};

/// Subset of [`Config`] consumed by the legacy HTTP/1.1 client builder.
///
/// Captured as copyable values so the builder does not depend on the
/// (partially moved) `Config` during construction.
pub(crate) struct LegacyClientSettings {
    pub(crate) http2_initial_stream_window_size: Option<u32>,
    pub(crate) http2_initial_connection_window_size: Option<u32>,
    pub(crate) http2_adaptive_window: bool,
    pub(crate) http2_max_frame_size: Option<u32>,
    pub(crate) http2_max_header_list_size: Option<u32>,
    pub(crate) http2_keep_alive_interval: Option<Duration>,
    pub(crate) http2_keep_alive_timeout: Option<Duration>,
    pub(crate) http2_keep_alive_while_idle: bool,
    pub(crate) http09_responses: bool,
    pub(crate) http1_title_case_headers: bool,
    pub(crate) http1_allow_obsolete_multiline_headers_in_responses: bool,
    pub(crate) http1_ignore_invalid_headers_in_responses: bool,
    pub(crate) http1_allow_spaces_after_header_name_in_responses: bool,
    pub(crate) http1_max_headers: Option<usize>,
    pub(crate) pool_idle_timeout: Option<Duration>,
    pub(crate) pool_max_idle_per_host: usize,
}

impl LegacyClientSettings {
    /// Read the legacy-client settings out of `config` without moving anything.
    pub(crate) fn from_config(config: &Config) -> Self {
        LegacyClientSettings {
            http2_initial_stream_window_size: config.http2_initial_stream_window_size,
            http2_initial_connection_window_size: config.http2_initial_connection_window_size,
            http2_adaptive_window: config.http2_adaptive_window,
            http2_max_frame_size: config.http2_max_frame_size,
            http2_max_header_list_size: config.http2_max_header_list_size,
            http2_keep_alive_interval: config.http2_keep_alive_interval,
            http2_keep_alive_timeout: config.http2_keep_alive_timeout,
            http2_keep_alive_while_idle: config.http2_keep_alive_while_idle,
            http09_responses: config.http09_responses,
            http1_title_case_headers: config.http1_title_case_headers,
            http1_allow_obsolete_multiline_headers_in_responses: config
                .http1_allow_obsolete_multiline_headers_in_responses,
            http1_ignore_invalid_headers_in_responses: config
                .http1_ignore_invalid_headers_in_responses,
            http1_allow_spaces_after_header_name_in_responses: config
                .http1_allow_spaces_after_header_name_in_responses,
            http1_max_headers: config.http1_max_headers,
            pool_idle_timeout: config.pool_idle_timeout,
            pool_max_idle_per_host: config.pool_max_idle_per_host,
        }
    }
}

/// Build the legacy HTTP/1.1 client for non-h2 paths (plain HTTP,
/// HTTP-over-proxy, and the `Http1`/`Http2` prefs).
pub(crate) fn build_legacy_http1_client(
    settings: &LegacyClientSettings,
    connector: Connector,
) -> Client<Connector, Body> {
    let mut b = Client::builder(TokioExecutor::new());
    if let Some(http2_initial_stream_window_size) = settings.http2_initial_stream_window_size {
        b.http2_initial_stream_window_size(http2_initial_stream_window_size);
    }
    if let Some(http2_initial_connection_window_size) =
        settings.http2_initial_connection_window_size
    {
        b.http2_initial_connection_window_size(http2_initial_connection_window_size);
    }
    if settings.http2_adaptive_window {
        b.http2_adaptive_window(true);
    }
    if let Some(http2_max_frame_size) = settings.http2_max_frame_size {
        b.http2_max_frame_size(http2_max_frame_size);
    }
    if let Some(http2_max_header_list_size) = settings.http2_max_header_list_size {
        b.http2_max_header_list_size(http2_max_header_list_size);
    }
    if let Some(http2_keep_alive_interval) = settings.http2_keep_alive_interval {
        b.http2_keep_alive_interval(http2_keep_alive_interval);
    }
    if let Some(http2_keep_alive_timeout) = settings.http2_keep_alive_timeout {
        b.http2_keep_alive_timeout(http2_keep_alive_timeout);
    }
    if settings.http2_keep_alive_while_idle {
        b.http2_keep_alive_while_idle(true);
    }
    if settings.http09_responses {
        b.http09_responses(true);
    }
    if settings.http1_title_case_headers {
        b.http1_title_case_headers(true);
    }
    if settings.http1_allow_obsolete_multiline_headers_in_responses {
        b.http1_allow_obsolete_multiline_headers_in_responses(true);
    }
    if settings.http1_ignore_invalid_headers_in_responses {
        b.http1_ignore_invalid_headers_in_responses(true);
    }
    if settings.http1_allow_spaces_after_header_name_in_responses {
        b.http1_allow_spaces_after_header_name_in_responses(true);
    }
    if let Some(http1_max_headers) = settings.http1_max_headers {
        b.http1_max_headers(http1_max_headers);
    }
    b.timer(TokioTimer::new());
    b.pool_timer(TokioTimer::new());
    b.pool_idle_timeout(settings.pool_idle_timeout);
    b.pool_max_idle_per_host(settings.pool_max_idle_per_host);
    b.build(connector)
}
