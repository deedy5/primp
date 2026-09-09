use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[cfg(test)]
use http::uri::Scheme;
use http::{header::HeaderValue, HeaderMap, Uri};
use hyper_util::client::proxy::matcher;

use crate::into_url::{IntoUrl, IntoUrlSealed};
use crate::Url;

// # Internals
//
// This module is a couple pieces:
//
// - The public builder API
// - The internal built types that our Connector knows how to use.
//
// The user creates a builder (`crate::Proxy`), and configures any extras.
// Once that type is passed to the `ClientBuilder`, we convert it into the
// built matcher types, making use of `hyper-util`'s matchers.

/// Configuration of a proxy that a `Client` should route requests through.
///
/// A `Proxy` pairs a target URL with rules on which `Client` requests to
/// intercept. The `Client` checks each `Proxy` in the order added, so an eager
/// rule like `Proxy::all` added first can block later proxies. SOCKS is supported.
///
/// ```rust
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let proxy = primp::Proxy::http("https://secure.example")?;
/// # Ok(())
/// # }
/// ```
///
/// This proxy intercepts all HTTP requests but not HTTPS ones.
///
/// ```rust
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let proxy = primp::Proxy::all("socks5://192.168.1.1:9000")?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Proxy {
    extra: Extra,
    intercept: Intercept,
    no_proxy: Option<NoProxy>,
}

/// Filtering config for requests that should NOT be proxied.
#[derive(Clone, Debug, Default)]
pub struct NoProxy {
    inner: String,
}

#[derive(Clone)]
struct Extra {
    auth: Option<HeaderValue>,
    misc: Option<HeaderMap>,
}

// ===== Internal =====

pub(crate) struct Matcher {
    inner: Matcher_,
    extra: Extra,
    maybe_has_http_auth: bool,
    maybe_has_http_custom_headers: bool,
}

enum Matcher_ {
    Util(Box<matcher::Matcher>),
    Custom(Custom),
}

/// Wraps an `Intercept` plus any extra proxy configuration (auth/headers) added
/// by `primp::Proxy`.
#[derive(Clone)]
pub(crate) struct Intercepted {
    inner: matcher::Intercept,
    /// This is because of `primp::Proxy`'s design which allows configuring
    /// an explicit auth, besides what might have been in the URL (or Custom).
    extra: Extra,
}

/// Convert a value into a proxy URL. Parses URL-like types and factory-built
/// proxy schemes.
pub trait IntoProxy {
    fn into_proxy(self) -> crate::Result<Url>;
}

impl<S: IntoUrl> IntoProxy for S {
    fn into_proxy(self) -> crate::Result<Url> {
        // Proxy URLs may be socks, so parse directly with `Url::parse`
        // instead of `IntoUrl` (which only allows http/https for client
        // requests). We still require a host.
        match Url::parse(self.as_str()) {
            Ok(mut url) => {
                if url.host_str().is_none() {
                    return Err(crate::error::builder("proxy URL must have a host"));
                }
                // Reject non-http(s)/socks schemes: `url::set_username` fails
                // for them, which would panic `Proxy::basic_auth`.
                if !matches!(
                    url.scheme(),
                    "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h"
                ) {
                    return Err(crate::error::builder(format!(
                        "unsupported proxy scheme '{}'",
                        url.scheme()
                    )));
                }
                // If the scheme is a SOCKS protocol and no port is specified, set the default
                if url.port().is_none()
                    && matches!(url.scheme(), "socks4" | "socks4a" | "socks5" | "socks5h")
                {
                    let _ = url.set_port(Some(1080));
                }
                Ok(url)
            }
            Err(e) => {
                let mut presumed_to_have_scheme = true;
                let mut source = e.source();
                while let Some(err) = source {
                    if let Some(parse_error) = err.downcast_ref::<url::ParseError>() {
                        if *parse_error == url::ParseError::RelativeUrlWithoutBase {
                            presumed_to_have_scheme = false;
                            break;
                        }
                    }
                    source = err.source();
                }
                if presumed_to_have_scheme {
                    return Err(crate::error::builder(e));
                }
                // the issue could have been caused by a missing scheme, so we try adding http://
                let try_this = format!("http://{}", self.as_str());
                try_this.into_url().map_err(|_| {
                    // return the original error
                    crate::error::builder(e)
                })
            }
        }
    }
}

// These bounds are accidentally leaked by the blanket impl of IntoProxy
// for all types that implement IntoUrl. So, this function exists to detect
// if we were to break those bounds for a user.
fn _implied_bounds() {
    fn prox<T: IntoProxy>(_t: T) {}

    fn url<T: IntoUrl>(t: T) {
        prox(t);
    }
}

impl Proxy {
    /// Proxy all HTTP traffic to the passed URL.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate primp;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = primp::Client::builder()
    ///     .proxy(primp::Proxy::http("https://my.prox")?)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn http<U: IntoProxy>(proxy_scheme: U) -> crate::Result<Proxy> {
        Ok(Proxy::new(Intercept::Http(proxy_scheme.into_proxy()?)))
    }

    /// Proxy all HTTPS traffic to the passed URL.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate primp;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = primp::Client::builder()
    ///     .proxy(primp::Proxy::https("https://example.prox:4545")?)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn https<U: IntoProxy>(proxy_scheme: U) -> crate::Result<Proxy> {
        Ok(Proxy::new(Intercept::Https(proxy_scheme.into_proxy()?)))
    }

    /// Proxy **all** traffic to the passed URL.
    ///
    /// "All" refers to `https` and `http` URLs. Other schemes are not
    /// recognized by primp.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate primp;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = primp::Client::builder()
    ///     .proxy(primp::Proxy::all("http://pro.xy")?)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn all<U: IntoProxy>(proxy_scheme: U) -> crate::Result<Proxy> {
        Ok(Proxy::new(Intercept::All(proxy_scheme.into_proxy()?)))
    }

    /// Provide a custom function to determine what traffic to proxy to where.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate primp;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let target = primp::Url::parse("https://my.prox")?;
    /// let client = primp::Client::builder()
    ///     .proxy(primp::Proxy::custom(move |url| {
    ///         if url.host_str() == Some("hyper.rs") {
    ///             Some(target.clone())
    ///         } else {
    ///             None
    ///         }
    ///     }))
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn custom<F, U: IntoProxy>(fun: F) -> Proxy
    where
        F: Fn(&Url) -> Option<U> + Send + Sync + 'static,
    {
        Proxy::new(Intercept::Custom(Custom {
            func: Arc::new(move |url| fun(url).map(IntoProxy::into_proxy)),
            no_proxy: None,
        }))
    }

    fn new(intercept: Intercept) -> Proxy {
        Proxy {
            extra: Extra {
                auth: None,
                misc: None,
            },
            intercept,
            no_proxy: None,
        }
    }

    /// Set the `Proxy-Authorization` header using Basic auth.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate primp;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let proxy = primp::Proxy::https("http://localhost:1234")?
    ///     .basic_auth("Aladdin", "open sesame");
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn basic_auth(mut self, username: &str, password: &str) -> Proxy {
        match self.intercept {
            Intercept::All(ref mut s)
            | Intercept::Http(ref mut s)
            | Intercept::Https(ref mut s) => {
                // URL can't carry userinfo? Fall back to the header store,
                // like `Intercept::Custom` — never panic.
                if !url_auth(s, username, password) {
                    self.extra.auth = Some(encode_basic_auth(username, password));
                }
            }
            Intercept::Custom(_) => {
                let header = encode_basic_auth(username, password);
                self.extra.auth = Some(header);
            }
        }

        self
    }

    /// Set the `Proxy-Authorization` header to a specified value.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate primp;
    /// # use primp::header::*;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let proxy = primp::Proxy::https("http://localhost:1234")?
    ///     .custom_http_auth(HeaderValue::from_static("justletmeinalreadyplease"));
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn custom_http_auth(mut self, header_value: HeaderValue) -> Proxy {
        self.extra.auth = Some(header_value);
        self
    }

    /// Attach custom headers to this `Proxy`.
    ///
    /// # Example
    /// ```
    /// # extern crate primp;
    /// # use primp::header::*;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut headers = HeaderMap::new();
    /// headers.insert(USER_AGENT, "primp".parse().unwrap());
    /// let proxy = primp::Proxy::https("http://localhost:1234")?
    ///     .headers(headers);
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn headers(mut self, headers: HeaderMap) -> Proxy {
        match self.intercept {
            Intercept::All(_) | Intercept::Http(_) | Intercept::Https(_) | Intercept::Custom(_) => {
                self.extra.misc = Some(headers);
            }
        }

        self
    }

    /// Adds a `No Proxy` exclusion list to this Proxy
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate primp;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let proxy = primp::Proxy::https("http://localhost:1234")?
    ///     .no_proxy(primp::NoProxy::from_string("direct.tld, sub.direct2.tld"));
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn no_proxy(mut self, no_proxy: Option<NoProxy>) -> Proxy {
        self.no_proxy = no_proxy;
        self
    }

    pub(crate) fn into_matcher(self) -> Matcher {
        let Proxy {
            intercept,
            extra,
            no_proxy,
        } = self;

        let maybe_has_http_auth;
        let maybe_has_http_custom_headers;

        let inner = match intercept {
            Intercept::All(url) => {
                maybe_has_http_auth = cache_maybe_has_http_auth(&url, &extra.auth);
                maybe_has_http_custom_headers =
                    cache_maybe_has_http_custom_headers(&url, &extra.misc);
                Matcher_::Util(Box::new(
                    matcher::Matcher::builder()
                        .all(String::from(url))
                        .no(no_proxy.as_ref().map(|n| n.inner.as_ref()).unwrap_or(""))
                        .build(),
                ))
            }
            Intercept::Http(url) => {
                maybe_has_http_auth = cache_maybe_has_http_auth(&url, &extra.auth);
                maybe_has_http_custom_headers =
                    cache_maybe_has_http_custom_headers(&url, &extra.misc);
                Matcher_::Util(Box::new(
                    matcher::Matcher::builder()
                        .http(String::from(url))
                        .no(no_proxy.as_ref().map(|n| n.inner.as_ref()).unwrap_or(""))
                        .build(),
                ))
            }
            Intercept::Https(url) => {
                maybe_has_http_auth = cache_maybe_has_http_auth(&url, &extra.auth);
                maybe_has_http_custom_headers =
                    cache_maybe_has_http_custom_headers(&url, &extra.misc);
                Matcher_::Util(Box::new(
                    matcher::Matcher::builder()
                        .https(String::from(url))
                        .no(no_proxy.as_ref().map(|n| n.inner.as_ref()).unwrap_or(""))
                        .build(),
                ))
            }
            Intercept::Custom(mut custom) => {
                maybe_has_http_auth = true; // never know
                maybe_has_http_custom_headers = true;
                custom.no_proxy = no_proxy;
                Matcher_::Custom(custom)
            }
        };

        Matcher {
            inner,
            extra,
            maybe_has_http_auth,
            maybe_has_http_custom_headers,
        }
    }
}

fn cache_maybe_has_http_auth(url: &Url, extra: &Option<HeaderValue>) -> bool {
    (url.scheme() == "http" || url.scheme() == "https")
        && (!url.username().is_empty() || url.password().is_some() || extra.is_some())
}

fn cache_maybe_has_http_custom_headers(url: &Url, extra: &Option<HeaderMap>) -> bool {
    (url.scheme() == "http" || url.scheme() == "https") && extra.is_some()
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Proxy")
            .field(&self.intercept)
            .field(&self.no_proxy)
            .finish()
    }
}

impl NoProxy {
    /// Build a no-proxy config from `NO_PROXY`/`no_proxy` env vars (`None` if
    /// unset). See [`from_string`](Self::from_string) for the format.
    pub fn from_env() -> Option<NoProxy> {
        let raw = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .ok()?;

        // Per the docs, this returns `None` if no environment variable is set. We can only reach
        // here if an env var is set, so we return `Some(NoProxy::default)` if `from_string`
        // returns None, which occurs with an empty string.
        Some(Self::from_string(&raw).unwrap_or_default())
    }

    /// Build a no-proxy config from a `NO_PROXY`-style string. Entries are
    /// comma-separated; both IPv4/IPv6 (with optional `/size` subnet) are
    /// allowed; `*` matches all hosts; any other entry matches that domain and
    /// its subdomains. Stored as-is and parsed lazily by hyper-util when used.
    pub fn from_string(no_proxy_list: &str) -> Option<Self> {
        // lazy parsed, to not make the type public in hyper-util
        Some(NoProxy {
            inner: no_proxy_list.into(),
        })
    }
}

impl Matcher {
    pub(crate) fn system() -> Self {
        Self {
            inner: Matcher_::Util(Box::new(matcher::Matcher::from_system())),
            extra: Extra {
                auth: None,
                misc: None,
            },
            // maybe env vars have auth!
            maybe_has_http_auth: true,
            maybe_has_http_custom_headers: true,
        }
    }

    pub(crate) fn intercept(&self, dst: &Uri) -> crate::Result<Option<Intercepted>> {
        let inner = match self.inner {
            Matcher_::Util(ref m) => m.intercept(dst),
            Matcher_::Custom(ref c) => c.call(dst)?,
        };

        Ok(inner.map(|inner| Intercepted {
            inner,
            extra: self.extra.clone(),
        }))
    }

    /// Whether this matcher might provide HTTP (non-tunnel) auth — a hint to
    /// skip the more expensive `intercept()` when forwarding never needs auth.
    pub(crate) fn maybe_has_http_auth(&self) -> bool {
        self.maybe_has_http_auth
    }

    #[cfg(test)]
    pub(crate) fn http_non_tunnel_basic_auth(
        &self,
        dst: &Uri,
    ) -> crate::Result<Option<HeaderValue>> {
        match self.intercept(dst) {
            Ok(Some(proxy)) => {
                let scheme = proxy.uri().scheme();
                if scheme == Some(&Scheme::HTTP) || scheme == Some(&Scheme::HTTPS) {
                    return Ok(proxy.basic_auth().cloned());
                }
                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn maybe_has_http_custom_headers(&self) -> bool {
        self.maybe_has_http_custom_headers
    }

    #[cfg(test)]
    pub(crate) fn http_non_tunnel_custom_headers(
        &self,
        dst: &Uri,
    ) -> crate::Result<Option<HeaderMap>> {
        match self.intercept(dst) {
            Ok(Some(proxy)) => {
                let scheme = proxy.uri().scheme();
                if scheme == Some(&Scheme::HTTP) || scheme == Some(&Scheme::HTTPS) {
                    return Ok(proxy.custom_headers().cloned());
                }
                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl fmt::Debug for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.inner {
            Matcher_::Util(_) => "Util",
            Matcher_::Custom(_) => "Custom",
        };
        f.debug_struct("Matcher")
            .field("kind", &kind)
            .field("has_auth", &self.maybe_has_http_auth)
            .field("has_custom_headers", &self.maybe_has_http_custom_headers)
            .finish_non_exhaustive()
    }
}

impl Intercepted {
    pub(crate) fn uri(&self) -> &http::Uri {
        self.inner.uri()
    }

    pub(crate) fn basic_auth(&self) -> Option<&HeaderValue> {
        if let Some(ref val) = self.extra.auth {
            return Some(val);
        }
        self.inner.basic_auth()
    }

    pub(crate) fn custom_headers(&self) -> Option<&HeaderMap> {
        if let Some(ref val) = self.extra.misc {
            return Some(val);
        }
        None
    }

    /// Return SOCKS (user, password) credentials for this proxy. SOCKS proxies
    /// store URL-embedded credentials as `Auth::Raw`, so [`basic_auth`] returns
    /// `None` for them and must not be used to source SOCKS auth. This accessor
    /// prefers the raw URL credentials and falls back to decoding an explicit
    /// `Proxy-Authorization` Basic header (from `Proxy::custom_http_auth`).
    pub(crate) fn socks_auth(&self) -> Option<(String, String)> {
        if let Some((user, pass)) = self.inner.raw_auth() {
            return Some((user.to_owned(), pass.to_owned()));
        }
        self.extra.auth.as_ref().and_then(decode_basic_auth)
    }
}

impl fmt::Debug for Intercepted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&redact_uri_userinfo(self.inner.uri()))
    }
}

/// Return `uri` as a string with any `userinfo` (`user:password@`) masked, so
/// proxy credentials never leak through `Debug` implementations or log lines.
/// `http://user:pass@host:8080/` becomes `http://***@host:8080/`.
pub(crate) fn redact_uri_userinfo(uri: &http::Uri) -> String {
    let s = uri.to_string();
    // Find the userinfo boundary: the first '@' that sits inside the authority
    // (after `scheme://` and before the first '/' or '?'). A literal IPv6 host
    // like `[::1]` has no '@'. An '@' in the path or query is NOT a userinfo
    // separator and must never be treated as one.
    if let Some(scheme_end) = s.find("://") {
        let rest = &s[scheme_end + 3..];
        let authority_end = rest
            .find('/')
            .or_else(|| rest.find('?'))
            .unwrap_or(rest.len());
        if let Some(at) = rest[..authority_end].find('@') {
            let at = scheme_end + 3 + at;
            return format!("{}***{}", &s[..scheme_end + 3], &s[at..]);
        }
    }
    s
}

/// Mask any userinfo (`user:pass@`) in a `url::Url` for Debug/log output.
/// `http://user:pass@host:8080/` becomes `http://***@host:8080/`.
fn redact_url_userinfo(url: &Url) -> String {
    let mut masked = url.clone();
    if !masked.username().is_empty() || masked.password().is_some() {
        let _ = masked.set_username("***");
        let _ = masked.set_password(None);
    }
    masked.to_string()
}

#[derive(Clone)]
enum Intercept {
    All(Url),
    Http(Url),
    Https(Url),
    Custom(Custom),
}

impl fmt::Debug for Intercept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // URLs carry proxy credentials (via `Proxy::basic_auth` /
        // URL-embedded userinfo); mask them so `Debug for Proxy` never leaks.
        match self {
            Intercept::All(u) => f.debug_tuple("All").field(&redact_url_userinfo(u)).finish(),
            Intercept::Http(u) => f
                .debug_tuple("Http")
                .field(&redact_url_userinfo(u))
                .finish(),
            Intercept::Https(u) => f
                .debug_tuple("Https")
                .field(&redact_url_userinfo(u))
                .finish(),
            Intercept::Custom(c) => f.debug_tuple("Custom").field(c).finish(),
        }
    }
}

fn url_auth(url: &mut Url, username: &str, password: &str) -> bool {
    url.set_username(username).is_ok() && url.set_password(Some(password)).is_ok()
}

#[derive(Clone)]
struct Custom {
    #[allow(clippy::type_complexity)]
    func: Arc<dyn Fn(&Url) -> Option<crate::Result<Url>> + Send + Sync + 'static>,
    no_proxy: Option<NoProxy>,
}

impl Custom {
    fn call(&self, uri: &http::Uri) -> crate::Result<Option<matcher::Intercept>> {
        let (Some(scheme), Some(host)) = (uri.scheme(), uri.host()) else {
            return Ok(None);
        };
        // Preserve path and query for Custom proxy closures that inspect Url::path/query
        // (e.g. "/connect/path?query=1"). The previous format discarded path/query.
        // Userinfo and fragments are intentionally NOT forwarded: credentials
        // are extracted into `Authorization` before routing, and fragments are
        // never sent on the wire — closures route on origin + path only.
        let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
        let host_stripped = crate::strip_ipv6_brackets(host);
        let host_str = if host_stripped.contains(':') {
            format!("[{host_stripped}]")
        } else {
            host_stripped.to_string()
        };
        let url: Url = match format!(
            "{}://{}{}{}",
            scheme.as_str(),
            host_str,
            uri.port().map_or(String::new(), |p| format!(":{p}")),
            path_and_query
        )
        .parse()
        {
            Ok(u) => u,
            Err(e) => {
                // Going direct would be a fail-open bypass; error instead.
                log::warn!("custom proxy target failed to parse as URL: {e}");
                return Err(crate::error::builder(format!(
                    "custom proxy target parse failure: {e}"
                )));
            }
        };

        // `func` returns `None` when no proxy is configured for this URI
        // (send direct), but `Some(Err(..))` when a proxy *was* configured yet
        // its URL is invalid. The latter must be surfaced as an error rather
        // than silently falling through to a direct connection.
        match (self.func)(&url) {
            None => Ok(None),
            Some(Err(e)) => Err(e),
            Some(Ok(target)) => {
                let mut builder = matcher::Matcher::builder().all(String::from(target));
                if let Some(no_proxy) = self.no_proxy.as_ref() {
                    builder = builder.no(no_proxy.inner.as_str());
                }
                let m = builder.build();
                Ok(m.intercept(uri))
            }
        }
        //.map(|scheme| scheme.if_no_auth(&self.auth))
    }
}

impl fmt::Debug for Custom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("_")
    }
}

pub(crate) fn encode_basic_auth(username: &str, password: &str) -> HeaderValue {
    crate::util::basic_auth(username, Some(password))
}

/// Decode a `Basic` `Proxy-Authorization` value into `(username, password)`.
/// Returns `None` if missing, not `Basic`, or not valid UTF-8/base64/`user:pass`.
/// The scheme token is case-insensitive (RFC 7617) and base64 padding may be
/// omitted.
pub(crate) fn decode_basic_auth(header: &HeaderValue) -> Option<(String, String)> {
    use base64::Engine;
    let value = header.to_str().ok()?;
    // Tolerate OWS *(SP / HTAB) around the scheme token (RFC 7230 §3.2.3)
    // and match the scheme case-insensitively. Split on first SP or HTAB.
    let trimmed = value.trim_start();
    let (scheme, encoded) = {
        let idx = trimmed.find([' ', '\t'])?;
        let (s, rest) = trimmed.split_at(idx);
        (s, rest.trim_start_matches([' ', '\t']))
    };
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    // Tolerate trailing OWS after the credentials (RFC 7230 §3.2.3).
    let encoded = encoded.trim_end_matches([' ', '\t']);
    // RFC 7617 §2.1: the base64 padding may be omitted — try the standard
    // (padded) decoder first, then the no-pad decoder for unpadded input.
    let decoded = base64::prelude::BASE64_STANDARD
        .decode(encoded)
        .or_else(|_| base64::prelude::BASE64_STANDARD_NO_PAD.decode(encoded))
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let (username, password) = text.split_once(':')?;
    Some((username.to_owned(), password.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> http::Uri {
        s.parse().unwrap()
    }

    fn intercepted_uri(p: &Matcher, s: &str) -> Uri {
        p.intercept(&s.parse().unwrap())
            .unwrap()
            .expect("expected intercept")
            .uri()
            .clone()
    }

    #[test]
    fn test_http() {
        let target = "http://example.domain/";
        let p = Proxy::http(target).unwrap().into_matcher();

        let http = "http://hyper.rs";
        let other = "https://hyper.rs";

        assert_eq!(intercepted_uri(&p, http), target);
        assert!(p.intercept(&url(other)).unwrap().is_none());
    }

    #[test]
    fn redact_uri_userinfo_masks_credentials() {
        let with_creds: Uri = "http://user:secret@proxy.example:8080/".parse().unwrap();
        let redacted = redact_uri_userinfo(&with_creds);
        assert!(!redacted.contains("secret"), "leaked password: {redacted}");
        assert!(
            redacted.contains("***@proxy.example:8080/"),
            "unexpected form: {redacted}"
        );
        // Userinfo is fully removed, not partially.
        assert!(!redacted.contains("user:"), "leaked username: {redacted}");

        let no_creds: Uri = "http://proxy.example:8080/".parse().unwrap();
        assert_eq!(redact_uri_userinfo(&no_creds), "http://proxy.example:8080/");

        let ipv6: Uri = "http://[::1]:8080/".parse().unwrap();
        assert_eq!(redact_uri_userinfo(&ipv6), "http://[::1]:8080/");
    }

    #[test]
    fn redact_uri_userinfo_does_not_mask_at_in_path_or_query() {
        // An '@' inside the path or query is NOT a userinfo separator; the
        // authority must stay intact and nothing may be masked.
        let path_at: Uri = "http://proxy.example:8080/api@v1".parse().unwrap();
        assert_eq!(
            redact_uri_userinfo(&path_at),
            "http://proxy.example:8080/api@v1"
        );

        let query_at: Uri = "http://proxy.example:8080/?a=b@c".parse().unwrap();
        assert_eq!(
            redact_uri_userinfo(&query_at),
            "http://proxy.example:8080/?a=b@c"
        );

        // Credentials must still be masked when userinfo IS present.
        let with_creds: Uri = "http://user:secret@proxy.example:8080/api@v1"
            .parse()
            .unwrap();
        let redacted = redact_uri_userinfo(&with_creds);
        assert!(!redacted.contains("secret"), "leaked password: {redacted}");
        assert_eq!(redacted, "http://***@proxy.example:8080/api@v1");
    }

    #[test]
    fn proxy_debug_redacts_credentials() {
        let proxy = Proxy::all("http://proxy.example:8080/")
            .unwrap()
            .basic_auth("Aladdin", "open sesame");
        let debug = format!("{:?}", proxy);
        assert!(
            !debug.contains("open sesame"),
            "Proxy Debug leaked password: {debug}"
        );
        assert!(
            !debug.contains("Aladdin"),
            "Proxy Debug leaked username: {debug}"
        );
        assert!(
            debug.contains("***@proxy.example:8080"),
            "unexpected form: {debug}"
        );
    }

    #[test]
    fn proxy_debug_redacts_url_embedded_credentials() {
        let proxy = Proxy::all("http://Aladdin:open%20sesame@proxy.example:8080/").unwrap();
        let debug = format!("{:?}", proxy);
        assert!(
            !debug.contains("open"),
            "Proxy Debug leaked password: {debug}"
        );
        assert!(
            !debug.contains("Aladdin"),
            "Proxy Debug leaked username: {debug}"
        );
        assert!(
            debug.contains("***@proxy.example:8080"),
            "unexpected form: {debug}"
        );
    }

    #[test]
    fn matcher_debug_shows_kind_but_redacts_credentials() {
        // Util matcher with auth: kind visible, secrets not.
        let m = Proxy::all("http://user:secret@proxy.example:8080")
            .unwrap()
            .basic_auth("Aladdin", "open sesame")
            .into_matcher();
        let debug = format!("{:?}", m);
        assert!(debug.contains("Util"), "must show kind: {debug}");
        assert!(
            debug.contains("has_auth"),
            "must show auth presence: {debug}"
        );
        assert!(
            debug.contains("...") || debug.contains(".."),
            "must be non-exhaustive: {debug}"
        );
        for leaked in [
            "secret",
            "open sesame",
            "Aladdin",
            "user:secret@",
            "proxy.example:8080",
        ] {
            assert!(
                !debug.contains(leaked),
                "Matcher Debug leaked {leaked:?}: {debug}"
            );
        }
        // Custom matcher shows Custom kind, same redaction.
        let c = Proxy::custom(|_| Some("http://proxy.example:8080/")).into_matcher();
        let cdebug = format!("{:?}", c);
        assert!(cdebug.contains("Custom"), "must show kind: {cdebug}");
        assert!(
            !cdebug.contains("proxy.example:8080"),
            "must not leak target: {cdebug}"
        );
    }
    #[test]
    fn test_https() {
        let target = "http://example.domain/";
        let p = Proxy::https(target).unwrap().into_matcher();

        let http = "http://hyper.rs";
        let other = "https://hyper.rs";

        assert!(p.intercept(&url(http)).unwrap().is_none());
        assert_eq!(intercepted_uri(&p, other), target);
    }

    #[test]
    fn test_all() {
        let target = "http://example.domain/";
        let p = Proxy::all(target).unwrap().into_matcher();

        let http = "http://hyper.rs";
        let https = "https://hyper.rs";
        // no longer supported
        //let other = "x-youve-never-heard-of-me-mr-proxy://hyper.rs";

        assert_eq!(intercepted_uri(&p, http), target);
        assert_eq!(intercepted_uri(&p, https), target);
        //assert_eq!(intercepted_uri(&p, other), target);
    }

    #[test]
    fn test_custom() {
        let target1 = "http://example.domain/";
        let target2 = "https://example.domain/";
        let p = Proxy::custom(move |url| {
            if url.host_str() == Some("hyper.rs") {
                target1.parse().ok()
            } else if url.scheme() == "http" {
                target2.parse().ok()
            } else {
                None::<Url>
            }
        })
        .into_matcher();

        let http = "http://seanmonstar.com";
        let https = "https://hyper.rs";
        let other = "x-youve-never-heard-of-me-mr-proxy://seanmonstar.com";

        assert_eq!(intercepted_uri(&p, http), target2);
        assert_eq!(intercepted_uri(&p, https), target1);
        assert!(p.intercept(&url(other)).unwrap().is_none());
    }

    #[test]
    fn test_custom_closure_sees_path_and_query() {
        use std::sync::{Arc, Mutex};

        // Regression guard for path/query preservation: the closure must see
        // the request path and query (previously only scheme://host:port was
        // forwarded, so path-based routing silently saw "/").
        let seen = Arc::new(Mutex::new((String::new(), None::<String>)));
        let seen_in_closure = seen.clone();
        let p = Proxy::custom(move |url| {
            *seen_in_closure.lock().unwrap() =
                (url.path().to_owned(), url.query().map(str::to_owned));
            Some("http://proxy.example:8080".parse::<Url>().unwrap())
        })
        .into_matcher();

        let got = p
            .intercept(&url("http://example.com/api/v1?q=1"))
            .unwrap()
            .expect("must route via custom proxy");
        assert_eq!(
            got.uri(),
            &"http://proxy.example:8080/".parse::<http::Uri>().unwrap()
        );
        assert_eq!(
            *seen.lock().unwrap(),
            ("/api/v1".to_owned(), Some("q=1".to_owned()))
        );
    }

    #[test]
    fn test_custom_honors_no_proxy() {
        let target = "http://example.domain/";
        let p = Proxy::custom(move |_| target.parse::<Url>().ok())
            .no_proxy(NoProxy::from_string("direct.tld"))
            .into_matcher();

        // A host covered by no_proxy is bypassed even though the custom
        // closure would have proxied it.
        assert!(p.intercept(&url("http://direct.tld/")).unwrap().is_none());
        // A host not covered by no_proxy still routes through the custom proxy.
        assert_eq!(intercepted_uri(&p, "http://other.tld"), target);
    }

    #[test]
    fn test_standard_with_custom_auth_header() {
        let target = "http://example.domain/";
        let p = Proxy::all(target)
            .unwrap()
            .custom_http_auth(http::HeaderValue::from_static("testme"))
            .into_matcher();

        let got = p.intercept(&url("http://anywhere.local")).unwrap().unwrap();
        let auth = got.basic_auth().unwrap();
        assert_eq!(auth, "testme");
    }

    #[test]
    fn test_custom_with_custom_auth_header() {
        let target = "http://example.domain/";
        let p = Proxy::custom(move |_| target.parse::<Url>().ok())
            .custom_http_auth(http::HeaderValue::from_static("testme"))
            .into_matcher();

        let got = p.intercept(&url("http://anywhere.local")).unwrap().unwrap();
        let auth = got.basic_auth().unwrap();
        assert_eq!(auth, "testme");
    }

    #[test]
    fn test_maybe_has_http_auth() {
        let m = Proxy::all("https://letme:in@yo.local")
            .unwrap()
            .into_matcher();
        assert!(m.maybe_has_http_auth(), "https forwards");

        let m = Proxy::all("http://letme:in@yo.local")
            .unwrap()
            .into_matcher();
        assert!(m.maybe_has_http_auth(), "http forwards");

        let m = Proxy::all("http://:in@yo.local").unwrap().into_matcher();
        assert!(m.maybe_has_http_auth(), "http forwards with empty username");

        let m = Proxy::all("http://letme:@yo.local").unwrap().into_matcher();
        assert!(m.maybe_has_http_auth(), "http forwards with empty password");
    }

    #[test]
    fn custom_target_parse_failure_is_builder_fail_closed() {
        // `Custom::call` Url-build at `:687` is unreachable via valid `Uri`,
        // but must stay fail-closed. Pin via the reachable closure-Err path.
        let p = Proxy::custom(move |_url| Some("http://")).into_matcher();
        let err = p.intercept(&url("http://example.com/")).unwrap_err();
        assert!(err.is_builder(), "must be Builder Err, got {err:?}");
    }

    #[test]
    fn test_custom_error_swallowed_by_non_tunnel_helpers() {
        // A custom proxy whose closure returns an invalid proxy URL that
        // fails IntoProxy conversion. This makes Custom::call / intercept()
        // return Err(...), which the connector path (connect.rs:608-621)
        // correctly propagates.
        let p = Proxy::custom(move |_url| Some("")).into_matcher();

        let uri = url("http://example.com");

        // The connector path correctly propagates the intercept error.
        let intercept_result = p.intercept(&uri);
        assert!(
            intercept_result.is_err(),
            "intercept() should return Err for an invalid proxy URL"
        );

        // Fixed: http_non_tunnel_* now propagates the error instead of swallowing.
        let auth = p.http_non_tunnel_basic_auth(&uri);
        assert!(
            auth.is_err(),
            "http_non_tunnel_basic_auth should propagate intercept error"
        );

        let headers = p.http_non_tunnel_custom_headers(&uri);
        assert!(
            headers.is_err(),
            "http_non_tunnel_custom_headers should propagate intercept error"
        );
    }

    #[test]
    fn custom_catch_all_never_bypasses_http_uris() {
        // Catch-all must route via proxy, never go direct (Ok(None)) and
        // never abort (Err) for valid URIs. Fail-closed Err paths are
        // covered by `test_custom_error_swallowed_by_non_tunnel_helpers`
        // (`Some("")` invalid target -> is_err).
        let p = Proxy::custom(|_| Some("http://proxy.example:8080/")).into_matcher();
        for uri in [
            "http://a.example/",
            "http://b.example:1234/p?q=1",
            "https://c.example/",
        ] {
            let got = p.intercept(&url(uri));
            assert!(
                matches!(got, Ok(Some(_))),
                "catch-all must intercept {uri}, got {got:?}"
            );
        }
    }

    #[test]
    fn test_rejects_non_proxy_schemes() {
        // Rejected at construction — otherwise `basic_auth` panics on
        // `url::set_username`.
        for s in [
            "file://example.com/path",
            "data://example.com/x",
            "gopher://example.com",
            "mailto://example.com",
        ] {
            assert!(Proxy::all(s).is_err(), "Proxy::all({}) must be rejected", s);
            assert!(
                Proxy::http(s).is_err(),
                "Proxy::http({}) must be rejected",
                s
            );
            assert!(
                Proxy::https(s).is_err(),
                "Proxy::https({}) must be rejected",
                s
            );
        }
    }

    #[test]
    fn test_basic_auth_does_not_panic_on_rejected_scheme() {
        // Regression: `basic_auth` used to panic on `file://` (set_username
        // fails on non-special schemes).
        assert!(Proxy::all("file://example.com/path").is_err());
    }

    #[test]
    fn missing_proxy_host_reports_host_not_scheme() {
        // Missing host must say so; scheme itself is allowed.
        let err = Proxy::all("socks5:foo").unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.to_lowercase().contains("host"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn test_socks_proxy_default_port() {
        {
            let m = Proxy::all("socks5://example.com").unwrap().into_matcher();

            let http = "http://hyper.rs";
            let https = "https://hyper.rs";

            assert_eq!(intercepted_uri(&m, http).port_u16(), Some(1080));
            assert_eq!(intercepted_uri(&m, https).port_u16(), Some(1080));

            // custom port
            let m = Proxy::all("socks5://example.com:1234")
                .unwrap()
                .into_matcher();

            assert_eq!(intercepted_uri(&m, http).port_u16(), Some(1234));
            assert_eq!(intercepted_uri(&m, https).port_u16(), Some(1234));
        }
    }

    fn basic_header(s: &str) -> HeaderValue {
        HeaderValue::from_str(s).unwrap()
    }

    #[test]
    fn decode_basic_auth_accepts_canonical_form() {
        // "user:pass" in padded base64.
        let h = basic_header("Basic dXNlcjpwYXNz");
        assert_eq!(decode_basic_auth(&h), Some(("user".into(), "pass".into())));
    }

    #[test]
    fn decode_basic_auth_scheme_is_case_insensitive() {
        // RFC 7617: the scheme token is case-insensitive.
        let h = basic_header("basic dXNlcjpwYXNz");
        assert_eq!(decode_basic_auth(&h), Some(("user".into(), "pass".into())));
        let h = basic_header("bAsIc dXNlcjpwYXNz");
        assert_eq!(decode_basic_auth(&h), Some(("user".into(), "pass".into())));
    }

    #[test]
    fn decode_basic_auth_accepts_unpadded_base64() {
        // RFC 7617 §2.1: base64 padding MAY be omitted. "user:password"
        // pads to "dXNlcjpwYXNzd29yZA=="; the unpadded form must decode too.
        let h = basic_header("Basic dXNlcjpwYXNzd29yZA==");
        assert_eq!(
            decode_basic_auth(&h),
            Some(("user".into(), "password".into()))
        );
        let h = basic_header("Basic dXNlcjpwYXNzd29yZA");
        assert_eq!(
            decode_basic_auth(&h),
            Some(("user".into(), "password".into()))
        );
    }

    #[test]
    fn decode_basic_auth_roundtrip_matches_own_encoder() {
        let encoded = encode_basic_auth("Aladdin", "open sesame");
        assert_eq!(
            decode_basic_auth(&encoded),
            Some(("Aladdin".into(), "open sesame".into()))
        );
    }

    #[test]
    fn decode_basic_auth_tolerates_trailing_ows() {
        // RFC 7230 §3.2.3: optional whitespace around the field value.
        let h = basic_header("Basic dXNlcjpwYXNz   ");
        assert_eq!(decode_basic_auth(&h), Some(("user".into(), "pass".into())));
        let h = basic_header("Basic\tdXNlcjpwYXNz\t");
        assert_eq!(decode_basic_auth(&h), Some(("user".into(), "pass".into())));
    }

    #[test]
    fn decode_basic_auth_rejects_invalid_input() {
        // Wrong scheme.
        assert_eq!(
            decode_basic_auth(&basic_header("Bearer dXNlcjpwYXNz")),
            None
        );
        // Scheme with no space / no credentials.
        assert_eq!(decode_basic_auth(&basic_header("Basic")), None);
        // Invalid base64.
        assert_eq!(decode_basic_auth(&basic_header("Basic !!!")), None);
        // Valid base64 but not "user:pass".
        assert_eq!(decode_basic_auth(&basic_header("Basic aGVsbG8=")), None);
        // Non-UTF-8 base64 payload.
        assert_eq!(decode_basic_auth(&basic_header("Basic /w==")), None);
    }
}
