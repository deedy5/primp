//! HTTP Cookies

use crate::header::{HeaderValue, SET_COOKIE};
use bytes::Bytes;
use std::fmt;
use std::sync::RwLock;
use std::time::SystemTime;

/// A persistent cookie store providing session support.
pub trait CookieStore: Send + Sync {
    /// Store `Set-Cookie` header values received from `url`.
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &url::Url);
    /// Get the `Cookie` header value in the store for `url`, if any.
    fn cookies(&self, url: &url::Url) -> Option<HeaderValue>;
}

/// A single HTTP cookie.
pub struct Cookie<'a>(cookie_crate::Cookie<'a>);

/// The default `CookieStore` implementation, used by `cookie_store(true)`.
/// Exposed so you can pre-fill it with cookies before building a `Client`.
#[derive(Debug, Default)]
pub struct Jar(RwLock<cookie_store::CookieStore>);

// ===== impl Cookie =====

impl<'a> Cookie<'a> {
    fn parse(value: &'a HeaderValue) -> Result<Cookie<'a>, CookieParseError> {
        std::str::from_utf8(value.as_bytes())
            .map_err(cookie_crate::ParseError::from)
            .and_then(cookie_crate::Cookie::parse)
            .map_err(CookieParseError)
            .map(Cookie)
    }

    /// The name of the cookie.
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// The value of the cookie.
    pub fn value(&self) -> &str {
        self.0.value()
    }

    /// Returns true if the 'HttpOnly' directive is enabled.
    pub fn http_only(&self) -> bool {
        self.0.http_only().unwrap_or(false)
    }

    /// Returns true if the 'Secure' directive is enabled.
    pub fn secure(&self) -> bool {
        self.0.secure().unwrap_or(false)
    }

    /// Returns true if  'SameSite' directive is 'Lax'.
    pub fn same_site_lax(&self) -> bool {
        self.0.same_site() == Some(cookie_crate::SameSite::Lax)
    }

    /// Returns true if  'SameSite' directive is 'Strict'.
    pub fn same_site_strict(&self) -> bool {
        self.0.same_site() == Some(cookie_crate::SameSite::Strict)
    }

    /// Returns the path directive of the cookie, if set.
    pub fn path(&self) -> Option<&str> {
        self.0.path()
    }

    /// Returns the domain directive of the cookie, if set.
    pub fn domain(&self) -> Option<&str> {
        self.0.domain()
    }

    /// Get the Max-Age information.
    pub fn max_age(&self) -> Option<std::time::Duration> {
        // Defensive `.ok()`: the cookie crate normalizes negative
        // `Max-Age` to `Duration::ZERO` per RFC 6265 5.2.2, so this is
        // normally unreachable, but we return `Some(Duration::ZERO)`
        // (expire immediately) rather than panic if a negative value
        // sneaks through.
        self.0
            .max_age()
            .map(|d| d.try_into().unwrap_or(std::time::Duration::ZERO))
    }

    /// The cookie expiration time.
    pub fn expires(&self) -> Option<SystemTime> {
        match self.0.expires() {
            Some(cookie_crate::Expiration::DateTime(offset)) => Some(SystemTime::from(offset)),
            None | Some(cookie_crate::Expiration::Session) => None,
        }
    }
}

impl fmt::Debug for Cookie<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub(crate) fn extract_response_cookie_headers(
    headers: &hyper::HeaderMap,
) -> impl Iterator<Item = &HeaderValue> + '_ {
    headers.get_all(SET_COOKIE).iter()
}

pub(crate) fn extract_response_cookies(
    headers: &hyper::HeaderMap,
) -> impl Iterator<Item = Result<Cookie<'_>, CookieParseError>> + '_ {
    headers.get_all(SET_COOKIE).iter().map(Cookie::parse)
}

/// Error from failing to parse a `Set-Cookie` header.
pub(crate) struct CookieParseError(cookie_crate::ParseError);

impl fmt::Debug for CookieParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for CookieParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for CookieParseError {}

// ===== impl Jar =====

impl Jar {
    /// Add a cookie to this jar.
    ///
    /// # Example
    ///
    /// ```
    /// use primp::{cookie::Jar, Url};
    ///
    /// let cookie = "foo=bar; Domain=yolo.local";
    /// let url = "https://yolo.local".parse::<Url>().unwrap();
    ///
    /// let jar = Jar::default();
    /// jar.add_cookie_str(cookie, &url);
    ///
    /// // and now add to a `ClientBuilder`?
    /// ```
    pub fn add_cookie_str(&self, cookie: &str, url: &url::Url) {
        let cookies = cookie_crate::Cookie::parse(cookie)
            .ok()
            .map(|c| c.into_owned())
            .into_iter()
            .filter(|c| !public_suffix_domain_rejected(c));
        // Recover from a poisoned lock: a panic in one task must not kill
        // the entire client.
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .store_response_cookies(cookies, url);
    }
}

impl CookieStore for Jar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &url::Url) {
        let iter = cookie_headers.filter_map(|val| {
            Cookie::parse(val)
                .map(|c| c.0.into_owned())
                .ok()
                .filter(|c| !public_suffix_domain_rejected(c))
        });

        // Recover from a poisoned lock; see `add_cookie_str` above.
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .store_response_cookies(iter, url);
    }

    fn cookies(&self, url: &url::Url) -> Option<HeaderValue> {
        let mut s = String::new();
        for (name, value) in self
            .0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_request_values(url)
        {
            if !s.is_empty() {
                s.push_str("; ");
            }
            s.push_str(name);
            s.push('=');
            s.push_str(value);
        }

        if s.is_empty() {
            return None;
        }

        HeaderValue::from_maybe_shared(Bytes::from(s)).ok()
    }
}

/// RFC 6265 §5.3: reject `Domain` cookies that are public suffixes
/// (`com`, `co.uk`) — otherwise `evil.com` could set `Domain=.com`
/// and leak its cookie to every `*.com` host.
fn public_suffix_domain_rejected(cookie: &cookie_crate::Cookie<'_>) -> bool {
    cookie
        .domain()
        .map(|d| {
            let name = d.trim_start_matches('.').to_ascii_lowercase();
            // `domain_str` is `None` exactly when `name` is itself a public
            // suffix (or wildcard-public, e.g. `www.co.uk`).
            psl::domain_str(&name).is_none()
        })
        .unwrap_or(false)
}

pub(crate) mod service {
    use crate::cookie;
    use http::{Request, Response};
    use http_body::Body;
    use pin_project_lite::pin_project;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::ready;
    use std::task::Context;
    use std::task::Poll;
    use tower::Service;
    use url::Url;

    /// A [`Service`] adding cookie support (inject on send, store on response)
    /// to an inner [`Service`].
    #[derive(Clone)]
    pub struct CookieService<S> {
        inner: S,
        cookie_store: Option<Arc<dyn cookie::CookieStore>>,
    }

    impl<S> CookieService<S> {
        /// Create a new [`CookieService`].
        pub fn new(inner: S, cookie_store: Option<Arc<dyn cookie::CookieStore>>) -> Self {
            Self {
                inner,
                cookie_store,
            }
        }
    }

    impl<ReqBody, ResBody, S> Service<Request<ReqBody>> for CookieService<S>
    where
        S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
        ReqBody: Body + Default,
    {
        type Response = Response<ResBody>;
        type Error = S::Error;
        type Future = ResponseFuture<S, ReqBody>;

        #[inline]
        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
            let clone = self.inner.clone();
            let mut inner = std::mem::replace(&mut self.inner, clone);
            // Cookies are injected for (and stored under) the URL of the
            // *current* request. `req.uri()` is always the effective per-request
            // URI: tower-http's redirect layer rewrites it on every rebuilt hop.
            // Do NOT read a `Url` request extension instead — FollowRedirect
            // preserves extensions across hops (tower-http 0.7 defaults
            // `preserve_extensions: true`), so an extension stashed at
            // request-build time still holds the ORIGINAL URL on hop 2+ and
            // would leak the original host's cookies to a cross-host target.
            // One-shot cookies (`RequestConfig<OneShotCookies>`) are re-merged
            // with the jar's CURRENT state on EVERY hop, so intermediate
            // `Set-Cookie`s reach the final request; the explicit header alone
            // (tower-http carries it across same-origin hops) would go stale.
            let url = if let Some(cookie_store) = self.cookie_store.as_ref() {
                let url = Url::parse(req.uri().to_string().as_str()).ok();
                if let Some(url) = url.as_ref() {
                    if let Some(one_shot) = crate::config::RequestConfig::<
                        crate::config::OneShotCookies,
                    >::get(req.extensions())
                    {
                        let merged = crate::util::merge_one_shot_cookie_header(
                            &**cookie_store,
                            url,
                            one_shot,
                        );
                        req.headers_mut().insert(crate::header::COOKIE, merged);
                    } else if req.headers().get(crate::header::COOKIE).is_none() {
                        let headers = req.headers_mut();
                        crate::util::add_cookie_header(headers, &**cookie_store, url);
                    }
                }
                url
            } else if let Some(one_shot) =
                crate::config::RequestConfig::<crate::config::OneShotCookies>::get(req.extensions())
            {
                // No store: there is no jar to merge with — emit the one-shots
                // verbatim. They are an explicit per-request header, not a jar
                // operation, so they must reach the wire regardless.
                let one_shot = one_shot.clone();
                req.headers_mut().insert(crate::header::COOKIE, one_shot);
                None
            } else {
                // No store configured: `url` is unused downstream, so avoid any
                // parsing on the hot path.
                None
            };

            let cookie_store = self.cookie_store.clone();
            ResponseFuture {
                future: inner.call(req),
                cookie_store,
                url,
            }
        }
    }

    pin_project! {
        #[allow(missing_debug_implementations)]
        #[derive(Clone)]
        /// A [`Future`] adding cookie support to an inner [`Future`].
        pub struct ResponseFuture<S, B>
        where
            S: Service<Request<B>>,
        {
            #[pin]
            future: S::Future,
            cookie_store: Option<Arc<dyn cookie::CookieStore>>,
            url: Option<Url>,
        }
    }

    impl<S, ReqBody, ResBody> Future for ResponseFuture<S, ReqBody>
    where
        S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
        ReqBody: Body + Default,
    {
        type Output = Result<Response<ResBody>, S::Error>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let cookie_store = self.cookie_store.clone();
            let url = self.url.clone();
            let res = ready!(self.project().future.as_mut().poll(cx)?);

            if let (Some(cookie_store), Some(url)) = (cookie_store.as_ref(), url.as_ref()) {
                let mut cookies = cookie::extract_response_cookie_headers(res.headers()).peekable();
                if cookies.peek().is_some() {
                    cookie_store.set_cookies(&mut cookies, url);
                }
            }
            Poll::Ready(Ok(res))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// After a panic poisons the store's lock, reads and writes must recover.
    #[test]
    fn cookie_store_recovers_from_poisoned_lock() {
        let jar = Arc::new(Jar::default());
        let url = url::Url::parse("http://example.com/").unwrap();

        jar.add_cookie_str("a=1", &url);

        // Poison the lock by panicking while holding the write guard.
        let jar_clone = Arc::clone(&jar);
        let handle = thread::spawn(move || {
            let _guard = jar_clone.0.write().expect("first lock");
            panic!("intentional panic to poison the cookie store lock");
        });
        let _ = handle.join();

        // Read after poison.
        let cookies = jar.cookies(&url);
        assert!(
            cookies.is_some(),
            "cookies() must recover from a poisoned lock"
        );

        // Write after poison.
        jar.set_cookies(&mut [HeaderValue::from_static("b=2")].iter(), &url);
        let cookies = jar.cookies(&url).unwrap();
        let s = std::str::from_utf8(cookies.as_bytes()).unwrap();
        assert!(s.contains("a=1"), "old cookie lost: {s}");
        assert!(s.contains("b=2"), "new cookie missing: {s}");

        // `add_cookie_str` must also recover.
        jar.add_cookie_str("c=3", &url);
    }
}
