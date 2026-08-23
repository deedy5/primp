//! Client configuration shared by the sync and async `Client` types.

use std::sync::{Arc, RwLock};

use foldhash::fast::RandomState;
use indexmap::IndexMap;
use primp::{
    dns::Resolve,
    header::{HeaderMap, HeaderValue},
    redirect::Policy,
    Client as PrimpClient, ClientBuilder, Proxy, Url,
};
use pyo3::prelude::*;
use pyo3::types::PyList;

use crate::error::{PrimpErrorEnum, PrimpResult};
use crate::impersonate::{
    get_random_element, parse_impersonate_os_with_fallback, parse_impersonate_with_fallback,
    IMPERSONATEOS_LIST,
};
use crate::traits::{HeaderMapExt, HeadersTraits};
use crate::utils::load_ca_certs;

/// Type alias for IndexMap with String keys and values.
pub type IndexMapSSR = IndexMap<String, String, RandomState>;

/// Parse a resolver string into an `Arc<dyn Resolve>`.
///
/// `doh://host/path` → DoH, `dot://host` → DoT, `dns://host` or bare host
/// → plain DNS on port 53, `system` → system resolver.
fn parse_single_resolver(s: &str) -> PrimpResult<Arc<dyn Resolve>> {
    if let Some(url) = s.strip_prefix("doh://") {
        let doh_url = format!("https://{url}");
        let resolver = primp::dns::doh::DohResolver::new(&doh_url)
            .map_err(|e| PrimpErrorEnum::Custom(format!("invalid DoH URL: {e}")))?;
        Ok(Arc::new(resolver))
    } else if let Some(host) = s.strip_prefix("dot://") {
        Ok(Arc::new(primp::dns::dot::DotResolver::new(host)))
    } else {
        let host = s.strip_prefix("dns://").unwrap_or(s);
        if host.is_empty() {
            return Err(PrimpErrorEnum::Custom("dns:// URL must have a host".into()));
        }
        if host == "system" {
            return Ok(Arc::new(primp::dns::gai::GaiResolver::new()));
        }
        Ok(Arc::new(primp::dns::plain::PlainDnsResolver::new(host)))
    }
}

/// Parse the `dns_resolver` Python argument into a `Vec<Arc<dyn Resolve>>`.
///
/// `None` → system default; `str` → single resolver; `list[str]` → fallback
/// chain (first success wins).
pub fn parse_dns_resolver(
    obj: Option<pyo3::Bound<'_, pyo3::types::PyAny>>,
) -> PrimpResult<Vec<Arc<dyn Resolve>>> {
    let Some(obj) = obj else {
        return Ok(Vec::new());
    };
    if let Ok(s) = obj.cast::<pyo3::types::PyString>() {
        return Ok(vec![parse_single_resolver(
            &s.to_cow()
                .map_err(|e| PrimpErrorEnum::Custom(e.to_string()))?,
        )?]);
    }
    if let Ok(list) = obj.cast::<PyList>() {
        let mut resolvers = Vec::with_capacity(list.len());
        for item in list.iter() {
            let s = item.cast::<pyo3::types::PyString>().map_err(|_| {
                PrimpErrorEnum::Custom("each item in dns_resolver list must be a string".into())
            })?;
            resolvers.push(parse_single_resolver(
                &s.to_cow()
                    .map_err(|e| PrimpErrorEnum::Custom(e.to_string()))?,
            )?);
        }
        return Ok(resolvers);
    }
    Err(PrimpErrorEnum::Custom(
        "dns_resolver must be a string, list of strings, or None".into(),
    ))
}

/// Apply the shared client configuration: impersonation, default headers,
/// cookie store, referer, proxy (env `PRIMP_PROXY` fallback), timeouts,
/// redirects (default limit 20), TLS verification/`ca_cert_file`, `https_only`,
/// `http2_only`, and the DNS resolver chain. Returns the configured builder and
/// the resolved proxy URL.
pub fn configure_client_builder(
    mut builder: ClientBuilder,
    headers: Option<IndexMapSSR>,
    cookie_store: Option<bool>,
    referer: Option<bool>,
    proxy: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<&str>,
    impersonate_os: Option<&str>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    https_only: Option<bool>,
    http2_only: Option<bool>,
    dns_resolvers: Vec<Arc<dyn Resolve>>,
) -> PrimpResult<(ClientBuilder, Option<String>)> {
    // Impersonate
    if let Some(imp) = impersonate {
        let imp_val = parse_impersonate_with_fallback(imp);
        let imp_os = if let Some(os) = impersonate_os {
            parse_impersonate_os_with_fallback(os)
        } else {
            *get_random_element(IMPERSONATEOS_LIST)
        };
        builder = builder.impersonate_os(imp_os);
        builder = builder.impersonate(imp_val);
    } else if let Some(os) = impersonate_os {
        let imp_os = parse_impersonate_os_with_fallback(os);
        builder = builder.impersonate_os(imp_os);
    }

    // Headers
    if let Some(headers) = headers {
        builder = builder.default_headers(headers.to_headermap()?);
    }

    // Cookie store
    if cookie_store.unwrap_or(true) {
        builder = builder.cookie_store(true);
    }

    // Referer
    builder = builder.referer(referer.unwrap_or(true));

    // Proxy - check environment variable as fallback
    let proxy = proxy.or_else(|| std::env::var("PRIMP_PROXY").ok());
    if let Some(ref proxy_url) = proxy {
        builder = builder.proxy(Proxy::all(proxy_url)?);
    }

    // Timeout
    if let Some(seconds) = timeout {
        builder = builder.timeout(crate::utils::timeout_duration(seconds)?);
    }

    // Connect timeout
    if let Some(seconds) = connect_timeout {
        builder = builder.connect_timeout(crate::utils::timeout_duration(seconds)?);
    }

    // Read timeout
    if let Some(seconds) = read_timeout {
        builder = builder.read_timeout(crate::utils::timeout_duration(seconds)?);
    }

    // DNS resolution timeout
    if let Some(seconds) = dns_timeout {
        builder = builder.dns_timeout(crate::utils::timeout_duration(seconds)?);
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

    // DNS resolver (fallback chain)
    if !dns_resolvers.is_empty() {
        builder = builder.dns_resolver(dns_resolvers);
    }

    Ok((builder, proxy))
}

/// Extracts cookies from a cookie header string into an IndexMap.
pub fn parse_cookies_from_header(cookie_str: &str) -> IndexMapSSR {
    // Estimate capacity by counting semicolons (usually n cookies = n-1 semicolons)
    // This avoids reallocations during cookie parsing
    let estimated_count = cookie_str.bytes().filter(|&b| b == b';').count() + 1;
    let mut cookie_map =
        IndexMap::with_capacity_and_hasher(estimated_count.max(2), RandomState::default());

    for cookie in cookie_str.split(';') {
        let mut parts = cookie.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            let key = key.trim();
            let value = value.trim();
            cookie_map.insert(key.to_string(), value.to_string());
        }
    }
    cookie_map
}

/// Converts an IndexMap of cookies to HeaderValue for setting cookies.
pub fn cookies_to_header_values(cookies: &IndexMapSSR) -> Vec<HeaderValue> {
    let mut result = Vec::with_capacity(cookies.len());
    for (key, value) in cookies {
        let mut s = String::with_capacity(key.len() + 1 + value.len());
        s.push_str(key);
        s.push('=');
        s.push_str(value);
        if let Ok(header) = HeaderValue::from_str(&s) {
            result.push(header);
        }
    }
    result
}

/// Build the one-shot `Cookie` header for a request (Python `cookies=`
/// semantics: `client < request`, one-shot, never stored).
///
/// Client-level cookies are first persisted to the store so they last across
/// requests; the JAR itself is merged by the core cookie service on every
/// request hop (fresh per redirect), with these one-shots taking precedence
/// over same-named jar cookies. An invalid `k=v` pair is skipped rather than
/// dropping the whole header.
pub fn build_request_cookie_header(
    client: &PrimpClient,
    url: &Url,
    client_cookies: Option<&IndexMapSSR>,
    request_cookies: Option<&IndexMapSSR>,
) -> Option<String> {
    // 1. Push client-level cookies to the persistent store so they persist
    //    across requests (matching old behavior and `requests`/`httpx`).
    if let Some(cookies) = client_cookies {
        if !cookies.is_empty() {
            let header_values = cookies_to_header_values(cookies);
            client.set_cookies(url, header_values);
        }
    }

    // 2. Merge: client cookies are the base, request cookies override them
    //    (both one-shot — never stored into the jar).
    let mut merged = IndexMapSSR::default();
    if let Some(cookies) = client_cookies {
        for (k, v) in cookies {
            merged.insert(k.clone(), v.clone());
        }
    }
    if let Some(cookies) = request_cookies {
        for (k, v) in cookies {
            merged.insert(k.clone(), v.clone());
        }
    }

    if merged.is_empty() {
        return None;
    }

    let mut header = String::new();
    for (k, v) in &merged {
        let mut pair = String::with_capacity(k.len() + 1 + v.len());
        pair.push_str(k);
        pair.push('=');
        pair.push_str(v);
        // Skip only the invalid pair instead of dropping the whole header
        // (a control character in one cookie value must not kill the rest).
        if HeaderValue::from_str(&pair).is_err() {
            continue;
        }
        if !header.is_empty() {
            header.push_str("; ");
        }
        header.push_str(&pair);
    }

    if header.is_empty() {
        None
    } else {
        Some(header)
    }
}

/// Parse a string as a URL, or treat a bare domain as `https://{domain}/`.
pub fn parse_url_or_domain(input: &str) -> Result<Url, url::ParseError> {
    if let Ok(url) = Url::parse(input) {
        if url.scheme() == "http" || url.scheme() == "https" {
            return Ok(url);
        }
    }
    // Treat as domain: prepend https://
    Url::parse(&format!("https://{}/", input.trim_end_matches('/')))
}

/// Removes the COOKIE header from a HeaderMap and returns the remaining headers as IndexMap.
pub fn headers_without_cookie(headers: &HeaderMap) -> IndexMapSSR {
    let mut headers_map = headers.to_indexmap();
    headers_map.swap_remove("cookie");
    headers_map
}

pub fn client_headers(client: &Arc<RwLock<PrimpClient>>) -> PrimpResult<IndexMapSSR> {
    let c = client.read().unwrap_or_else(|e| e.into_inner());
    Ok(headers_without_cookie(c.headers()))
}

pub fn client_set_headers(
    client: &Arc<RwLock<PrimpClient>>,
    new_headers: Option<IndexMapSSR>,
) -> PrimpResult<()> {
    let mut c = client.write().unwrap_or_else(|e| e.into_inner());
    let headers = c.headers_mut();
    headers.clear();
    if let Some(new_headers) = new_headers {
        for (k, v) in new_headers {
            headers.insert_key_value(k, v)?;
        }
    }
    Ok(())
}

pub fn client_headers_update(
    client: &Arc<RwLock<PrimpClient>>,
    new_headers: Option<IndexMapSSR>,
) -> PrimpResult<()> {
    let mut c = client.write().unwrap_or_else(|e| e.into_inner());
    let headers = c.headers_mut();
    if let Some(new_headers) = new_headers {
        for (k, v) in new_headers {
            headers.insert_key_value(k, v)?;
        }
    }
    Ok(())
}

pub fn client_set_proxy(
    client: &Arc<RwLock<PrimpClient>>,
    proxy: Option<String>,
) -> PrimpResult<Option<String>> {
    let mut c = client.write().unwrap_or_else(|e| e.into_inner());
    match proxy {
        Some(url) => {
            let rproxy = Proxy::all(url.clone())?;
            c.set_proxies(vec![rproxy]);
            Ok(Some(url))
        }
        None => {
            c.set_proxies(vec![]);
            Ok(None)
        }
    }
}

pub fn client_get_cookies(
    client: &Arc<RwLock<PrimpClient>>,
    url: &str,
) -> PrimpResult<IndexMapSSR> {
    let parsed = parse_url_or_domain(url).map_err(|e| PrimpErrorEnum::InvalidURL(e.to_string()))?;
    let c = client.read().unwrap_or_else(|e| e.into_inner());
    // An empty jar for the URL returns `None` from the core; surface an
    // empty dict (like `Response.cookies` and requests/httpx) rather than
    // raising.
    let Some(cookie) = c.get_cookies(&parsed) else {
        return Ok(IndexMapSSR::default());
    };
    let cookie_str = cookie.to_str()?;
    Ok(parse_cookies_from_header(cookie_str))
}

pub fn client_set_cookies(
    client: &Arc<RwLock<PrimpClient>>,
    url: &str,
    cookies: Option<IndexMapSSR>,
) -> PrimpResult<()> {
    let Some(cookies) = cookies else {
        return Ok(());
    };
    let parsed = parse_url_or_domain(url).map_err(|e| PrimpErrorEnum::InvalidURL(e.to_string()))?;
    let header_values = cookies_to_header_values(&cookies);
    let c = client.read().unwrap_or_else(|e| e.into_inner());
    c.set_cookies(&parsed, header_values);
    Ok(())
}
