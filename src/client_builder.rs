//! Common client configuration shared between sync and async clients.
//!
//! This module eliminates code duplication by providing shared configuration
//! structures and functions used by both `RClient` and `RAsyncClient`.

use std::time::Duration;

use foldhash::fast::RandomState;
use indexmap::IndexMap;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    redirect::Policy,
    ClientBuilder,
};

use crate::error::PrimpResult;
use crate::impersonate::{
    get_random_element, parse_impersonate_os_with_fallback, parse_impersonate_with_fallback,
    IMPERSONATEOS_LIST,
};
use crate::traits::HeadersTraits;
use crate::utils::load_ca_certs;

/// Type alias for IndexMap with String keys and values.
pub type IndexMapSSR = IndexMap<String, String, RandomState>;

/// Applies common configuration to a client builder.
///
/// This function handles all configuration that is shared between
/// sync and async clients, including:
/// - Impersonation settings
/// - Headers
/// - Cookie store
/// - Referer
/// - Proxy
/// - Timeout
/// - Redirects
/// - SSL verification
/// - HTTPS-only mode
/// - HTTP2-only mode
///
/// # Arguments
///
/// * `builder` - The client builder to configure
/// * `headers` - Optional default headers
/// * `cookie_store` - Whether to enable cookie storage
/// * `referer` - Whether to automatically set Referer header
/// * `proxy` - Optional proxy URL
/// * `timeout` - Optional timeout in seconds
/// * `impersonate` - Optional browser impersonation target
/// * `impersonate_os` - Optional OS impersonation target
/// * `follow_redirects` - Whether to follow redirects
/// * `max_redirects` - Maximum number of redirects
/// * `verify` - Whether to verify SSL certificates
/// * `ca_cert_file` - Optional path to CA certificate file
/// * `https_only` - Whether to restrict to HTTPS only
/// * `http2_only` - Whether to use HTTP/2 only
///
/// # Returns
///
/// A tuple containing the configured builder and the resolved proxy URL.
pub fn configure_client_builder(
    mut builder: ClientBuilder,
    headers: Option<IndexMapSSR>,
    cookie_store: Option<bool>,
    referer: Option<bool>,
    proxy: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    https_only: Option<bool>,
    http2_only: Option<bool>,
) -> PrimpResult<(ClientBuilder, Option<String>)> {
    // Impersonate
    if let Some(imp) = &impersonate {
        let imp_val = parse_impersonate_with_fallback(imp.as_str());
        let imp_os = if let Some(os) = &impersonate_os {
            parse_impersonate_os_with_fallback(os.as_str())
        } else {
            *get_random_element(IMPERSONATEOS_LIST)
        };
        // IMPORTANT: Call impersonate_os BEFORE impersonate, because impersonate reads os_type from config
        builder = builder.impersonate_os(imp_os);
        builder = builder.impersonate(imp_val);
    } else if let Some(os) = &impersonate_os {
        let imp_os = parse_impersonate_os_with_fallback(os.as_str());
        builder = builder.impersonate_os(imp_os);
    }

    // Headers
    if let Some(headers) = headers {
        builder = builder.default_headers(headers.to_headermap());
    }

    // Cookie store
    if cookie_store.unwrap_or(true) {
        builder = builder.cookie_store(true);
    }

    // Referer
    if referer.unwrap_or(true) {
        builder = builder.referer(true);
    }

    // Proxy - check environment variable as fallback
    let proxy = proxy.or_else(|| std::env::var("PRIMP_PROXY").ok());
    if let Some(ref proxy_url) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
    }

    // Timeout
    if let Some(seconds) = timeout {
        builder = builder.timeout(Duration::from_secs_f64(seconds));
    }

    // Redirects
    if follow_redirects.unwrap_or(true) {
        builder = builder.redirect(Policy::limited(max_redirects.unwrap_or(20)));
    } else {
        builder = builder.redirect(Policy::none());
    }

    // Verify and ca_cert_file
    if verify.unwrap_or(true) {
        if let Some(ca_certs) = load_ca_certs(&ca_cert_file) {
            for cert in ca_certs {
                builder = builder.add_root_certificate(cert);
            }
        }
    } else {
        builder = builder.danger_accept_invalid_certs(true);
    }

    // HTTPS only
    if https_only == Some(true) {
        builder = builder.https_only(true);
    }

    // HTTP2 only
    if http2_only == Some(true) {
        builder = builder.http2_prior_knowledge();
    }

    Ok((builder, proxy))
}

/// Extracts cookies from a cookie header string into an IndexMap.
pub fn parse_cookies_from_header(cookie_str: &str) -> IndexMapSSR {
    let mut cookie_map = IndexMap::with_capacity_and_hasher(10, RandomState::default());
    for cookie in cookie_str.split(';') {
        let mut parts = cookie.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            cookie_map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    cookie_map
}

/// Converts an IndexMap of cookies to HeaderValue for setting cookies.
pub fn cookies_to_header_values(cookies: &IndexMapSSR) -> Vec<HeaderValue> {
    // Pre-allocate with known capacity
    let mut result = Vec::with_capacity(cookies.len());
    for (key, value) in cookies {
        if let Ok(header) = HeaderValue::from_str(&format!("{}={}", key, value)) {
            result.push(header);
        }
    }
    result
}

/// Removes the COOKIE header from a HeaderMap and returns the remaining headers as IndexMap.
pub fn headers_without_cookie(headers: &HeaderMap) -> IndexMapSSR {
    let mut headers_map = headers.to_indexmap();
    headers_map.swap_remove("cookie");
    headers_map
}
