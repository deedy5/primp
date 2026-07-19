//! Redirect handling. A `Client` automatically follows redirects, up to a
//! maximum chain of 10 hops, configurable via a `redirect::Policy`.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::{error::Error as StdError, sync::Arc};

use crate::header::{AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION, REFERER};
use crate::proxy::Matcher as ProxyMatcher;
use http::{uri::Scheme, HeaderMap, HeaderValue};
use hyper::StatusCode;

use crate::{async_impl, Url};
use tower_http::follow_redirect::policy::{
    Action as TowerAction, Attempt as TowerAttempt, Policy as TowerPolicy,
};

/// Controls how the `Client` follows redirects. The default catches redirect
/// loops and follows at most 10 hops before erroring. `limited` adjusts the
/// max, `none` disables following, and `custom` supplies a custom policy.
pub struct Policy {
    pub(crate) inner: PolicyKind,
}

/// Holds info on the next request and the previous requests in a redirect chain.
#[derive(Debug)]
pub struct Attempt<'a> {
    status: StatusCode,
    next: &'a Url,
    previous: &'a [Url],
}

/// An action to perform when a redirect status is encountered.
#[derive(Debug)]
pub struct Action {
    inner: ActionKind,
}

impl Policy {
    /// Create a `Policy` with a maximum number of redirects. An `Error` is
    /// returned once the max is reached.
    pub fn limited(max: usize) -> Self {
        Self {
            inner: PolicyKind::Limit(max),
        }
    }

    /// Create a `Policy` that does not follow any redirect.
    pub fn none() -> Self {
        Self {
            inner: PolicyKind::None,
        }
    }

    /// Create a custom `Policy` from a closure. The default `Policy` caps the
    /// redirect loop, but a custom policy must handle loops itself. See
    /// [`Attempt`] for the info and actions available.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use primp::{Error, redirect};
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let custom = redirect::Policy::custom(|attempt| {
    ///     if attempt.previous().len() > 5 {
    ///         attempt.error("too many redirects")
    ///     } else if attempt.url().host_str() == Some("example.domain") {
    ///         // prevent redirects to 'example.domain'
    ///         attempt.stop()
    ///     } else {
    ///         attempt.follow()
    ///     }
    /// });
    /// let client = primp::Client::builder()
    ///     .redirect(custom)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Attempt`]: struct.Attempt.html
    pub fn custom<T>(policy: T) -> Self
    where
        T: Fn(Attempt) -> Action + Send + Sync + 'static,
    {
        Self {
            inner: PolicyKind::Custom(Box::new(policy)),
        }
    }

    /// Apply this policy to an `Attempt` to produce an `Action`. Useful with
    /// `Policy::custom()` to wrap another policy.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use primp::{Error, redirect};
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let custom = redirect::Policy::custom(|attempt| {
    ///     eprintln!("{}, Location: {:?}", attempt.status(), attempt.url());
    ///     redirect::Policy::default().redirect(attempt)
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub fn redirect(&self, attempt: Attempt) -> Action {
        match self.inner {
            PolicyKind::Custom(ref custom) => custom(attempt),
            PolicyKind::Limit(max) => {
                // The first URL in the previous is the initial URL and not a redirection. It needs to be excluded.
                if attempt.previous.len() > max {
                    attempt.error(TooManyRedirects)
                } else {
                    attempt.follow()
                }
            }
            PolicyKind::None => attempt.stop(),
        }
    }

    pub(crate) fn check(&self, status: StatusCode, next: &Url, previous: &[Url]) -> ActionKind {
        self.redirect(Attempt {
            status,
            next,
            previous,
        })
        .inner
    }

    pub(crate) fn is_default(&self) -> bool {
        matches!(self.inner, PolicyKind::Limit(10))
    }
}

impl Default for Policy {
    fn default() -> Policy {
        // Keep `is_default` in sync
        Policy::limited(10)
    }
}

impl Attempt<'_> {
    /// Get the redirect status code.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Get the next URL to redirect to.
    pub fn url(&self) -> &Url {
        self.next
    }

    /// Get the list of previous URLs that have already been requested in this chain.
    pub fn previous(&self) -> &[Url] {
        self.previous
    }
    /// Returns an action meaning primp should follow the next URL.
    pub fn follow(self) -> Action {
        Action {
            inner: ActionKind::Follow,
        }
    }

    /// Action meaning primp should not follow the next URL. The 30x response is
    /// returned as the `Ok` result.
    pub fn stop(self) -> Action {
        Action {
            inner: ActionKind::Stop,
        }
    }

    /// Action failing the redirect with an error, returned as the request result.
    pub fn error<E: Into<Box<dyn StdError + Send + Sync>>>(self, error: E) -> Action {
        Action {
            inner: ActionKind::Error(error.into()),
        }
    }
}

pub(crate) enum PolicyKind {
    Custom(Box<dyn Fn(Attempt) -> Action + Send + Sync + 'static>),
    Limit(usize),
    None,
}

impl fmt::Debug for Policy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Policy").field(&self.inner).finish()
    }
}

impl fmt::Debug for PolicyKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PolicyKind::Custom(..) => f.pad("Custom"),
            PolicyKind::Limit(max) => f.debug_tuple("Limit").field(&max).finish(),
            PolicyKind::None => f.pad("None"),
        }
    }
}

// pub(crate)

#[derive(Debug)]
pub(crate) enum ActionKind {
    Follow,
    Stop,
    Error(Box<dyn StdError + Send + Sync>),
}

/// Unconditionally remove the sensitive headers (used once a cross-host
/// hop has stripped them, since tower-http rebuilds later hops from the
/// original snapshot).
fn strip_sensitive_headers(headers: &mut HeaderMap) {
    headers.remove(AUTHORIZATION);
    headers.remove(COOKIE);
    headers.remove("cookie2");
    headers.remove(PROXY_AUTHORIZATION);
}

/// Strip sensitive headers when `next` is cross-host relative to the last
/// visited URL. Returns whether anything was stripped.
pub(crate) fn remove_sensitive_headers(
    headers: &mut HeaderMap,
    next: &Url,
    previous: &[Url],
) -> bool {
    if let Some(previous) = previous.last() {
        let cross_host = next.host_str() != previous.host_str()
            || next.port_or_known_default() != previous.port_or_known_default()
            || next.scheme() != previous.scheme();
        if cross_host {
            strip_sensitive_headers(headers);
            return true;
        }
    }
    false
}

#[derive(Debug)]
struct TooManyRedirects;

impl fmt::Display for TooManyRedirects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("too many redirects")
    }
}

impl StdError for TooManyRedirects {}

#[derive(Clone)]
pub(crate) struct TowerRedirectPolicy {
    policy: Arc<std::sync::RwLock<Policy>>,
    referer: bool,
    urls: Vec<Url>,
    https_only: bool,
    redirect_enabled: Arc<AtomicBool>,
    /// Per-request override from the first `on_request` call; lives on the
    /// per-request policy clone, never on the shared client.
    override_policy: Option<crate::config::RedirectOverride>,
    /// Set once a cross-host hop strips sensitive headers. tower-http
    /// rebuilds later hops from the original hop-0 snapshot (which still
    /// carries the creds), so they must be re-stripped unconditionally.
    sensitive_stripped: bool,
    /// Shared with `ClientInner`: per-hop `Proxy-Authorization` re-attach
    /// reads the same matchers the connector uses, so `set_proxies` stays in
    /// sync.
    proxies: Arc<RwLock<Vec<ProxyMatcher>>>,
}

impl TowerRedirectPolicy {
    pub(crate) fn new(policy: Policy) -> Self {
        let enabled = !matches!(policy.inner, PolicyKind::None);
        Self {
            policy: Arc::new(std::sync::RwLock::new(policy)),
            referer: false,
            urls: Vec::new(),
            https_only: false,
            redirect_enabled: Arc::new(AtomicBool::new(enabled)),
            override_policy: None,
            sensitive_stripped: false,
            proxies: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub(crate) fn with_referer(&mut self, referer: bool) -> &mut Self {
        self.referer = referer;
        self
    }

    pub(crate) fn with_https_only(&mut self, https_only: bool) -> &mut Self {
        self.https_only = https_only;
        self
    }

    pub(crate) fn with_proxies(&mut self, proxies: Arc<RwLock<Vec<ProxyMatcher>>>) -> &mut Self {
        self.proxies = proxies;
        self
    }

    /// Proxy auth belongs to the proxy CONNECTION, not the destination
    /// origin. `execute_request` attaches it for hop 0 only and the
    /// cross-host strip removes it, so re-attach per hop when this hop is
    /// also routed through an auth proxy (same matcher the connector uses).
    fn reattach_proxy_auth(&self, req: &mut http::Request<async_impl::body::Body>) {
        if req.uri().scheme() != Some(&Scheme::HTTP) {
            return;
        }
        if req.headers().contains_key(PROXY_AUTHORIZATION) {
            return;
        }
        let header = {
            let proxies = self.proxies.read().unwrap_or_else(|e| e.into_inner());
            let mut found = None;
            for proxy in proxies.iter() {
                match proxy.intercept(req.uri()) {
                    Ok(Some(intercepted)) => {
                        if let Some(scheme) = intercepted.uri().scheme() {
                            if scheme == &Scheme::HTTP || scheme == &Scheme::HTTPS {
                                found = intercepted.basic_auth().cloned();
                            }
                        }
                        break;
                    }
                    Ok(None) => continue,
                    Err(e) => {
                        log::warn!("proxy intercept error in reattach_proxy_auth: {e}");
                        break;
                    }
                }
            }
            found
        };
        if let Some(header) = header {
            req.headers_mut().insert(PROXY_AUTHORIZATION, header);
        }
    }

    /// Replace the active policy and its enabled flag so the new limits take
    /// effect immediately. Mutates through the existing `Arc`s so the tower
    /// `FollowRedirect` service (which shares them via the `clone()` at build
    /// time) sees the update; fresh `Arc`s would isolate the handle and make
    /// this a no-op.
    pub(crate) fn set_policy(&mut self, policy: Policy) {
        let enabled = !matches!(policy.inner, PolicyKind::None);
        *self.policy.write().unwrap_or_else(|e| e.into_inner()) = policy;
        self.redirect_enabled
            .store(enabled, std::sync::atomic::Ordering::Release);
    }
}

fn make_referer(next: &Url, previous: &Url) -> Option<HeaderValue> {
    if next.scheme() == "http" && previous.scheme() == "https" {
        return None;
    }

    let mut referer = previous.clone();
    let _ = referer.set_username("");
    let _ = referer.set_password(None);
    referer.set_fragment(None);
    referer.as_str().parse().ok()
}

impl TowerPolicy<async_impl::body::Body, crate::Error> for TowerRedirectPolicy {
    fn redirect(&mut self, attempt: &TowerAttempt<'_>) -> Result<TowerAction, crate::Error> {
        // A per-request override replaces the shared policy: it must not be
        // gated by the shared `redirect_enabled` flag.
        let override_enabled = match self.override_policy {
            Some(crate::config::RedirectOverride::Disabled) => return Ok(TowerAction::Stop),
            Some(crate::config::RedirectOverride::Follow(_)) => true,
            None => false,
        };

        // Check if redirects are enabled
        if !override_enabled && !self.redirect_enabled.load(Ordering::Acquire) {
            return Ok(TowerAction::Stop);
        }

        let previous_url = match Url::parse(&attempt.previous().to_string()) {
            Ok(url) => url,
            Err(e) => return Err(crate::error::builder(e)),
        };

        let next_url = match Url::parse(&attempt.location().to_string()) {
            Ok(url) => url,
            Err(e) => return Err(crate::error::builder(e)),
        };

        self.urls.push(previous_url.clone());

        // A per-request `Follow(n)` checks against its own limit; otherwise
        // use the shared policy (read-locked so `set_policy` can swap it,
        // poison-recovered so a panic elsewhere cannot disable redirects).
        let action = match self.override_policy {
            Some(crate::config::RedirectOverride::Follow(max)) => {
                let override_policy = Policy::limited(max);
                override_policy.check(attempt.status(), &next_url, &self.urls)
            }
            _ => self
                .policy
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .check(attempt.status(), &next_url, &self.urls),
        };
        match action {
            ActionKind::Follow => {
                if next_url.scheme() != "http" && next_url.scheme() != "https" {
                    return Err(crate::error::redirect(
                        crate::error::url_bad_scheme(next_url.clone()),
                        next_url,
                    ));
                }

                if self.https_only && next_url.scheme() != "https" {
                    return Err(crate::error::redirect(
                        crate::error::url_bad_scheme(next_url.clone()),
                        next_url,
                    ));
                }
                Ok(TowerAction::Follow)
            }
            ActionKind::Stop => Ok(TowerAction::Stop),
            ActionKind::Error(e) => Err(crate::error::redirect(e, previous_url)),
        }
    }

    fn on_request(&mut self, req: &mut http::Request<async_impl::body::Body>) {
        // Capture the per-request override from the request extensions; the
        // ORIGINAL request is the only one in the chain carrying them.
        // `or()` is REQUIRED: tower-http rebuilds follow-up requests WITHOUT
        // extensions and re-calls `on_request`, so a bare overwrite would
        // erase the override after hop 1.
        self.override_policy = self.override_policy.or(req
            .extensions()
            .get::<crate::config::RedirectOverride>()
            .copied());

        if let Ok(next_url) = Url::parse(&req.uri().to_string()) {
            let stripped = remove_sensitive_headers(req.headers_mut(), &next_url, &self.urls);
            if stripped {
                self.sensitive_stripped = true;
                // One-shot cookies must not follow across hosts either. Extensions
                // are replayed onto rebuilt hops, so removing it here keeps it
                // off every later hop (sticky).
                req.extensions_mut()
                    .remove::<crate::config::RequestConfig<crate::config::OneShotCookies>>();
            } else if self.sensitive_stripped {
                // Hop rebuilt from the hop-0 snapshot: re-strip so a later
                // same-origin hop can't resurrect the creds.
                strip_sensitive_headers(req.headers_mut());
            }
            if self.referer {
                if let Some(previous_url) = self.urls.last() {
                    if let Some(v) = make_referer(&next_url, previous_url) {
                        req.headers_mut().insert(REFERER, v);
                    }
                }
            }
        } else {
            // Be conservative: if next URL is unparseable by `Url` (e.g.
            // `http://example.com:abc/` where `http::Uri` is lenient but `Url`
            // rejects), treat it as cross-host and strip sensitive headers and
            // one-shot cookies. Otherwise `Authorization`/`Proxy-Authorization`
            // could leak to an attacker-controlled host via a malformed
            // `Location`.
            strip_sensitive_headers(req.headers_mut());
            self.sensitive_stripped = true;
            req.extensions_mut()
                .remove::<crate::config::RequestConfig<crate::config::OneShotCookies>>();
        };
        self.reattach_proxy_auth(req);
    }

    // This must be implemented to make 307 and 308 redirects work.
    //
    // A streaming (non-cloneable) body cannot be replayed on a 307/308
    // redirect. tower-http treats a `None` return as "no body to clone" and
    // silently sends an empty body — which would lose the upload. Instead we
    // return a body that fails loudly, so the caller gets a clear error rather
    // than a silently-truncated request. (clone_body is only ever consulted for
    // 307/308; 301/302/303 rebuild the request as GET without the body.)
    fn clone_body(&self, body: &async_impl::body::Body) -> Option<async_impl::body::Body> {
        match body.try_clone() {
            Some(cloned) => Some(cloned),
            None => Some(async_impl::body::Body::error(
                "cannot replay a streaming request body on a 307/308 redirect",
            )),
        }
    }
}

#[test]
fn test_redirect_policy_limit() {
    let policy = Policy::default();
    let next = Url::parse("http://x.y/z").unwrap();
    let mut previous = (0..=9)
        .map(|i| Url::parse(&format!("http://a.b/c/{i}")).unwrap())
        .collect::<Vec<_>>();

    match policy.check(StatusCode::FOUND, &next, &previous) {
        ActionKind::Follow => (),
        other => panic!("unexpected {other:?}"),
    }

    previous.push(Url::parse("http://a.b.d/e/33").unwrap());

    match policy.check(StatusCode::FOUND, &next, &previous) {
        ActionKind::Error(err) if err.is::<TooManyRedirects>() => (),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn test_redirect_policy_limit_to_0() {
    let policy = Policy::limited(0);
    let next = Url::parse("http://x.y/z").unwrap();
    let previous = vec![Url::parse("http://a.b/c").unwrap()];

    match policy.check(StatusCode::FOUND, &next, &previous) {
        ActionKind::Error(err) if err.is::<TooManyRedirects>() => (),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn test_tower_redirect_policy_set_policy_updates_limits() {
    // Start with a policy that allows 5 redirects, then swap to 0.
    // We can't easily construct a `tower_http::follow_redirect::Attempt`
    // outside of tower-http, so we verify the swap by reading the inner
    // policy back through the same RwLock the redirect path uses.
    let mut policy = TowerRedirectPolicy::new(Policy::limited(5));
    policy.set_policy(Policy::limited(0));

    let guard = policy.policy.read().expect("policy lock poisoned");
    match guard.check(
        StatusCode::FOUND,
        &Url::parse("http://x.y/z").unwrap(),
        &[Url::parse("http://a.b/c").unwrap()],
    ) {
        ActionKind::Error(err) if err.is::<TooManyRedirects>() => {}
        other => panic!("expected TooManyRedirects after set_policy(limited(0)), got {other:?}"),
    }
}

#[test]
fn test_tower_redirect_policy_clone_shares_state() {
    // `TowerRedirectPolicy` stores `policy` and `redirect_enabled` behind
    // `Arc`s so the `FollowRedirect` tower service (which receives a clone
    // of the same `TowerRedirectPolicy` at build time) always reads the
    // current policy.  `set_policy` mutates through those shared `Arc`s,
    // meaning a clone sees the same mutation as the original — this is by
    // design so that `Client::set_redirect_policy` actually changes the
    // behaviour of the live redirect service.
    let original = TowerRedirectPolicy::new(Policy::limited(10));
    let mut cloned = original.clone();

    cloned.set_policy(Policy::limited(0));

    // Both original and clone share the same Arc, so both see limited(0).
    for (label, p) in [("original", &original), ("clone", &cloned)] {
        let guard = p
            .policy
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match guard.check(
            StatusCode::FOUND,
            &Url::parse("http://x.y/z").unwrap(),
            &[Url::parse("http://a.b/c").unwrap()],
        ) {
            ActionKind::Error(err) if err.is::<TooManyRedirects>() => {}
            other => panic!("{label} should see limited(0) after set_policy, got {other:?}"),
        }
    }
}

#[test]
fn test_tower_redirect_policy_clone_shares_enabled_flag() {
    // Same sharing requirement for the `redirect_enabled` flag.
    let original = TowerRedirectPolicy::new(Policy::limited(5));
    let mut cloned = original.clone();

    cloned.set_policy(Policy::none());

    // Both original and clone share the same Arc, so both see false.
    assert!(
        !original
            .redirect_enabled
            .load(std::sync::atomic::Ordering::Relaxed),
        "original redirect_enabled should be false after set_policy(none) on clone"
    );
    assert!(
        !cloned
            .redirect_enabled
            .load(std::sync::atomic::Ordering::Relaxed),
        "clone redirect_enabled should be false after set_policy(none)"
    );
}

#[test]
fn test_tower_redirect_policy_recovers_from_poisoned_lock() {
    // Poison the write lock by panicking while holding it, then verify that
    // both `set_policy` and the redirect path's read still recover cleanly.
    // (Stable `std::sync::RwLock` does not expose a way to clear the poison,
    // but `PoisonError::into_inner` lets us still grab a guard and use it.)
    use std::sync::Arc;
    use std::thread;

    let mut policy = Arc::new(TowerRedirectPolicy::new(Policy::limited(5)));

    let poison_handle = {
        let policy = Arc::clone(&policy);
        thread::spawn(move || {
            let _guard = policy.policy.write().expect("first write should succeed");
            panic!("intentional panic to poison the policy lock");
        })
    };
    let _ = poison_handle.join();

    // Verify the lock is poisoned after the panic.
    assert!(
        policy.policy.read().is_err(),
        "policy lock should be poisoned"
    );

    // `set_policy` must still install the new policy even with a poisoned
    // lock. Use `Arc::get_mut` since the poison thread has joined and the
    // Arc is uniquely owned.
    Arc::get_mut(&mut policy)
        .expect("policy Arc should be uniquely owned after poison thread joined")
        .set_policy(Policy::limited(0));

    // The redirect path's read must also recover (not silently return
    // TowerAction::Stop) and observe the new `limited(0)` policy.
    let guard = policy
        .policy
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let next = Url::parse("http://x.y/z").unwrap();
    match guard.check(
        StatusCode::FOUND,
        &next,
        &[Url::parse("http://a.b/c").unwrap()],
    ) {
        ActionKind::Error(err) if err.is::<TooManyRedirects>() => {}
        other => panic!("expected TooManyRedirects after recovery, got {other:?}"),
    }
}

#[test]
fn test_redirect_policy_custom() {
    let policy = Policy::custom(|attempt| {
        if attempt.url().host_str() == Some("foo") {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });

    let next = Url::parse("http://bar/baz").unwrap();
    match policy.check(StatusCode::FOUND, &next, &[]) {
        ActionKind::Follow => (),
        other => panic!("unexpected {other:?}"),
    }

    let next = Url::parse("http://foo/baz").unwrap();
    match policy.check(StatusCode::FOUND, &next, &[]) {
        ActionKind::Stop => (),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn test_remove_sensitive_headers() {
    use hyper::header::{HeaderValue, ACCEPT, AUTHORIZATION, COOKIE};

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(AUTHORIZATION, HeaderValue::from_static("let me in"));
    headers.insert(COOKIE, HeaderValue::from_static("foo=bar"));

    let next = Url::parse("http://initial-domain.com/path").unwrap();
    let mut prev = vec![Url::parse("http://initial-domain.com/new_path").unwrap()];
    let mut filtered_headers = headers.clone();

    remove_sensitive_headers(&mut headers, &next, &prev);
    assert_eq!(headers, filtered_headers);

    prev.push(Url::parse("http://new-domain.com/path").unwrap());
    filtered_headers.remove(AUTHORIZATION);
    filtered_headers.remove(COOKIE);

    remove_sensitive_headers(&mut headers, &next, &prev);
    assert_eq!(headers, filtered_headers);
}

/// A same-host HTTPS -> HTTP redirect is a scheme downgrade and MUST strip
/// credential-bearing headers (they would otherwise be sent over plaintext).
/// `remove_sensitive_headers` treats a scheme change as `cross_host`.
#[test]
fn test_remove_sensitive_headers_strips_on_scheme_downgrade() {
    use hyper::header::{HeaderValue, ACCEPT, AUTHORIZATION, COOKIE};

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(AUTHORIZATION, HeaderValue::from_static("secret"));
    headers.insert(COOKIE, HeaderValue::from_static("sid=abc"));

    // Same host + port, only the scheme downgrades from https to http.
    let previous = vec![Url::parse("https://same-host.example/a").unwrap()];
    let next = Url::parse("http://same-host.example/b").unwrap();

    remove_sensitive_headers(&mut headers, &next, &previous);

    assert!(
        headers.get(AUTHORIZATION).is_none(),
        "Authorization must be stripped on an https->http downgrade"
    );
    assert!(
        headers.get(COOKIE).is_none(),
        "Cookie must be stripped on an https->http downgrade"
    );
    assert!(
        headers.get(ACCEPT).is_some(),
        "non-sensitive headers are kept"
    );
}

/// A same-origin redirect (identical scheme, host, and port) must KEEP
/// credential headers so ordinary within-site redirects still authenticate.
#[test]
fn test_remove_sensitive_headers_keeps_on_same_origin() {
    use hyper::header::{HeaderValue, AUTHORIZATION, COOKIE};

    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("secret"));
    headers.insert(COOKIE, HeaderValue::from_static("sid=abc"));

    let previous = vec![Url::parse("https://same-host.example/a").unwrap()];
    let next = Url::parse("https://same-host.example/b").unwrap();

    remove_sensitive_headers(&mut headers, &next, &previous);

    assert!(
        headers.get(AUTHORIZATION).is_some(),
        "same-origin keeps Authorization"
    );
    assert!(headers.get(COOKIE).is_some(), "same-origin keeps Cookie");
}

/// A replayable (in-memory) body must be cloned for a 307/308 redirect.
#[test]
fn clone_body_clones_reusable_body() {
    use crate::async_impl::body::Body;
    use tower_http::follow_redirect::policy::Policy as FollowPolicy;

    let policy = TowerRedirectPolicy::new(Policy::default());
    // `Body::empty()` is a `Reusable` variant, which `try_clone` can duplicate.
    let reusable = Body::empty();
    let cloned = FollowPolicy::<Body, crate::Error>::clone_body(&policy, &reusable);
    assert!(
        cloned.is_some(),
        "reusable body must be cloneable for 307/308"
    );
}

/// A streaming (non-cloneable) body must NOT be silently emptied on a 307/308
/// redirect. `clone_body` must return a body that errors, so the request fails
/// loudly instead of uploading an empty body.
#[tokio::test]
async fn clone_body_errors_for_streaming_body() {
    use crate::async_impl::body::Body;
    use bytes::Bytes;
    use http_body_util::BodyExt;
    use tower_http::follow_redirect::policy::Policy as FollowPolicy;

    let policy = TowerRedirectPolicy::new(Policy::default());
    // `Body::error` is a `Streaming` variant whose `try_clone` is `None`, so it
    // stands in for a non-replayable streaming upload. `clone_body` must return
    // a body that yields an error rather than succeeding with empty data.
    let streaming = Body::error("original streaming body marker");
    let returned = FollowPolicy::<Body, crate::Error>::clone_body(&policy, &streaming)
        .expect("clone_body returns a body for streaming input");
    // Drive the returned body and assert it errors (it must not yield Ok(None)).
    let result: Result<http_body_util::Collected<Bytes>, _> = returned.collect().await;
    assert!(
        result.is_err(),
        "streaming-body replay on 307/308 must error, not send empty body"
    );
}
