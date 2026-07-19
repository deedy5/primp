//! Retry requests by resending them when a response is considered retryable.
//!
//! The default policy only retries errors / low-level protocol NACKs known to
//! be safe to retry (and has no budget). Built-in scoped policies (e.g.
//! `for_host`) add a retry budget (default 20% extra requests) keyed to the
//! scope, so e.g. retries for one host are capped by that host's own traffic.
//! A retry classifier decides what to retry; requests should not be retried if
//! the server cannot safely handle them twice.

use std::sync::Arc;
use std::time::Duration;

use tower::retry::budget::{Budget as _, TpsBudget as Budget};

#[cfg(docsrs)]
pub use classify::ReqRep;

/// Builder to configure retries. Construct with [`for_host()`].
#[derive(Debug)]
pub struct Builder {
    //backoff: Backoff,
    budget: Option<f32>,
    classifier: classify::Classifier,
    max_retries_per_request: u32,
    scope: scope::Scoped,
}

/// The internal type the builder converts into, which privately implements
/// tower::retry::Policy.
#[derive(Clone, Debug)]
pub(crate) struct Policy {
    budget: Option<Arc<Budget>>,
    classifier: classify::Classifier,
    max_retries_per_request: u32,
    retry_cnt: u32,
    scope: scope::Scoped,
}

//#[derive(Debug)]
//struct Backoff;

/// Create a retry builder scoped to a specific host. For a non-closure scope,
/// use [`Builder::scoped()`].
pub fn for_host<S>(host: S) -> Builder
where
    S: for<'a> PartialEq<&'a str> + Send + Sync + 'static,
{
    scoped(move |req| host == req.uri().host().unwrap_or(""))
}

/// Create a retry policy that never retries. Useful to disable the `Client`'s
/// default of retrying protocol nacks.
pub fn never() -> Builder {
    scoped(|_| false).no_budget()
}

fn scoped<F>(func: F) -> Builder
where
    F: Fn(&Req) -> bool + Send + Sync + 'static,
{
    Builder::scoped(scope::ScopeFn(func))
}

// ===== impl Builder =====

impl Builder {
    /// Create a scoped retry policy. See [`for_host()`] for a convenience ctor.
    pub fn scoped(scope: impl scope::Scope) -> Self {
        Self {
            budget: Some(0.2),
            classifier: classify::Classifier::Never,
            max_retries_per_request: 2, // on top of the original
            scope: scope::Scoped::Dyn(Arc::new(scope)),
        }
    }

    /// Disable the retry budget (treated as infinite). NOT recommended: this
    /// makes the system more susceptible to retry storms.
    pub fn no_budget(mut self) -> Self {
        self.budget = None;
        self
    }

    /// Set the max extra load (as a fraction of request rate) the budget allows.
    ///
    /// For example, 1000 req/s with `0.3` allows 300 more req/s in retries;
    /// `2.5` allows 2,500 more.
    ///
    /// # Panics
    ///
    /// `extra_percent` must be within `[0.0, 1000.0]`.
    pub fn max_extra_load(mut self, extra_percent: f32) -> Self {
        assert!(extra_percent >= 0.0);
        assert!(extra_percent <= 1000.0);
        self.budget = Some(extra_percent);
        self
    }

    // pub fn max_replay_body

    /// Set the max retries allowed per request (on top of the original).
    /// Combined with the token budget, which may independently cap retries.
    /// Default is 2.
    pub fn max_retries_per_request(mut self, max: u32) -> Self {
        self.max_retries_per_request = max;
        self
    }

    /// Provide a classifier (closure) deciding if a request should be retried.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn with_builder(builder: primp::retry::Builder) -> primp::retry::Builder {
    /// builder.classify_fn(|req_rep| {
    ///     match (req_rep.method(), req_rep.status()) {
    ///         (&http::Method::GET, Some(http::StatusCode::SERVICE_UNAVAILABLE)) => {
    ///             req_rep.retryable()
    ///         },
    ///         _ => req_rep.success()
    ///     }
    /// })
    /// # }
    /// ```
    pub fn classify_fn<F>(self, func: F) -> Self
    where
        F: Fn(classify::ReqRep<'_>) -> classify::Action + Send + Sync + 'static,
    {
        self.classify(classify::ClassifyFn(func))
    }

    /// Provide a classifier to determine if a request should be retried.
    pub fn classify(mut self, classifier: impl classify::Classify) -> Self {
        self.classifier = classify::Classifier::Dyn(Arc::new(classifier));
        self
    }

    pub(crate) fn default() -> Builder {
        Self {
            // unscoped protocols nacks doesn't need a budget
            budget: None,
            classifier: classify::Classifier::ProtocolNacks,
            max_retries_per_request: 2, // on top of the original
            scope: scope::Scoped::Unscoped,
        }
    }

    pub(crate) fn into_policy(self) -> Policy {
        let budget = self
            .budget
            .map(|p| Arc::new(Budget::new(Duration::from_secs(10), 10, p)));
        Policy {
            budget,
            classifier: self.classifier,
            max_retries_per_request: self.max_retries_per_request,
            retry_cnt: 0,
            scope: self.scope,
        }
    }
}

// ===== internal ======

type Req = http::Request<crate::async_impl::body::Body>;

impl<B> tower::retry::Policy<Req, http::Response<B>, crate::Error> for Policy {
    // TODO? backoff futures...
    type Future = std::future::Ready<()>;

    fn retry(
        &mut self,
        req: &mut Req,
        result: &mut crate::Result<http::Response<B>>,
    ) -> Option<Self::Future> {
        match self.classifier.classify(req, result) {
            classify::Action::Success => {
                log::trace!("shouldn't retry!");
                // Out-of-scope requests never withdraw (see the `Retryable`
                // branch), so they must not deposit either: a scoped policy
                // shares one `Arc<Budget>` across all hosts/protocols, and
                // unrelated traffic must neither drain nor fill the pool
                // available to in-scope retries.
                if !self.scope.applies_to(req) {
                    return None;
                }
                if let Some(ref budget) = self.budget {
                    budget.deposit();
                }
                None
            }
            classify::Action::Retryable => {
                log::trace!("could retry!");
                // Out-of-scope requests are never actually retried (`clone_request`
                // returns `None` for them), so they must not withdraw from the
                // shared budget nor bump `retry_cnt`. A scoped policy shares one
                // `Arc<Budget>` across all hosts/protocols; letting unrelated
                // hosts drain it would starve in-scope retries.
                if !self.scope.applies_to(req) {
                    return None;
                }
                if self.budget.as_ref().map(|b| b.withdraw()).unwrap_or(true) {
                    self.retry_cnt += 1;
                    Some(std::future::ready(()))
                } else {
                    log::debug!("retryable but could not withdraw from budget");
                    None
                }
            }
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        if self.retry_cnt > 0 && !self.scope.applies_to(req) {
            return None;
        }
        if self.retry_cnt >= self.max_retries_per_request {
            log::trace!("max_retries_per_request hit");
            return None;
        }
        let body = req.body().try_clone()?;
        let mut new = http::Request::new(body);
        *new.method_mut() = req.method().clone();
        *new.uri_mut() = req.uri().clone();
        *new.version_mut() = req.version();
        *new.headers_mut() = req.headers().clone();
        *new.extensions_mut() = req.extensions().clone();

        Some(new)
    }
}

fn is_retryable_error(err: &crate::Error) -> bool {
    use std::error::Error as _;

    // The connector layer was refactored away from hyper-util's legacy
    // client, so protocol nacks (h2/h3 errors) are now wrapped *directly*
    // as the `crate::Error` source rather than nested behind a
    // `hyper_util::client::legacy::Error`. Walk the entire source chain
    // with a recursive downcast search instead of assuming a fixed nesting
    // depth, so we find the protocol error wherever it sits.
    let mut current: Option<&(dyn std::error::Error + 'static)> = err.source();

    // Guard against pathological cycles in the source chain.
    for _ in 0..64 {
        let Some(node) = current else {
            break;
        };

        if let Some(err) = node.downcast_ref::<h2::Error>() {
            // They sent us a graceful shutdown, try with a new connection!
            if err.is_go_away() && err.is_remote() && err.reason() == Some(h2::Reason::NO_ERROR) {
                return true;
            }

            // REFUSED_STREAM was sent from the server, which is safe to retry.
            // https://www.rfc-editor.org/rfc/rfc9113.html#section-8.7-3.2
            if err.is_reset() && err.is_remote() && err.reason() == Some(h2::Reason::REFUSED_STREAM)
            {
                return true;
            }
        }

        #[cfg(feature = "http3")]
        if let Some(err) = node.downcast_ref::<h3::error::ConnectionError>() {
            log::trace!("determining if HTTP/3 error {err} can be retried");
            // h3 0.0.8 marks `ConnectionError::Timeout` as `#[non_exhaustive]`
            // with a private variant, so it cannot be matched or constructed
            // from this crate. The only public signal is the `Display` string,
            // which currently yields "timeout". If a future h3 version exposes
            // a typed classifier, prefer that over this string compare.
            return err.to_string().as_str() == "timeout";
        }

        current = node.source();
    }

    false
}

// sealed types and traits on purpose while exploring design space
mod scope {
    pub trait Scope: Send + Sync + 'static {
        fn applies_to(&self, req: &super::Req) -> bool;
    }

    // I think scopes likely make the most sense being to hosts.
    // If that's the case, then it should probably be easiest to check for
    // the host. Perhaps also considering the ability to add more things
    // to scope off in the future...

    // For Future Whoever: making a blanket impl for any closure sounds nice,
    // but it causes inference issues at the call site. Every closure would
    // need to include `: ReqRep` in the arguments.
    //
    // An alternative is to make things like `ScopeFn`. Slightly more annoying,
    // but also more forwards-compatible. :shrug:

    pub struct ScopeFn<F>(pub(super) F);

    impl<F> Scope for ScopeFn<F>
    where
        F: Fn(&super::Req) -> bool + Send + Sync + 'static,
    {
        fn applies_to(&self, req: &super::Req) -> bool {
            (self.0)(req)
        }
    }

    #[derive(Clone)]
    pub(super) enum Scoped {
        Unscoped,
        Dyn(std::sync::Arc<dyn Scope>),
    }

    impl Scoped {
        pub(super) fn applies_to(&self, req: &super::Req) -> bool {
            let ret = match self {
                Self::Unscoped => true,
                Self::Dyn(s) => s.applies_to(req),
            };
            log::trace!("retry in scope: {ret}");
            ret
        }
    }

    impl std::fmt::Debug for Scoped {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Unscoped => f.write_str("Unscoped"),
                Self::Dyn(_) => f.write_str("Scoped"),
            }
        }
    }
}

// sealed types and traits on purpose while exploring design space
mod classify {
    pub trait Classify: Send + Sync + 'static {
        fn classify(&self, req_rep: ReqRep<'_>) -> Action;
    }

    // For Future Whoever: making a blanket impl for any closure sounds nice,
    // but it causes inference issues at the call site. Every closure would
    // need to include `: ReqRep` in the arguments.
    //
    // An alternative is to make things like `ClassifyFn`. Slightly more
    // annoying, but also more forwards-compatible. :shrug:
    pub struct ClassifyFn<F>(pub(super) F);

    impl<F> Classify for ClassifyFn<F>
    where
        F: Fn(ReqRep<'_>) -> Action + Send + Sync + 'static,
    {
        fn classify(&self, req_rep: ReqRep<'_>) -> Action {
            (self.0)(req_rep)
        }
    }

    /// A request/response result passed to a `classify` function.
    #[derive(Debug)]
    pub struct ReqRep<'a>(&'a super::Req, Result<http::StatusCode, &'a crate::Error>);

    impl ReqRep<'_> {
        /// Access the request method.
        pub fn method(&self) -> &http::Method {
            self.0.method()
        }

        /// Access the request URI.
        pub fn uri(&self) -> &http::Uri {
            self.0.uri()
        }

        /// Access the response status, if it did not error.
        pub fn status(&self) -> Option<http::StatusCode> {
            self.1.ok()
        }

        /// Access the error, if a response was not received.
        pub fn error(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.1.as_ref().err().map(|e| &**e as _)
        }

        /// Classify this attempt as retryable.
        pub fn retryable(self) -> Action {
            Action::Retryable
        }

        /// Classify this attempt as success (no retry), even on a domain error.
        pub fn success(self) -> Action {
            Action::Success
        }

        fn is_protocol_nack(&self) -> bool {
            self.1
                .as_ref()
                .err()
                .map(|&e| super::is_retryable_error(e))
                .unwrap_or(false)
        }
    }

    #[must_use]
    #[derive(Debug)]
    pub enum Action {
        Success,
        Retryable,
    }

    #[derive(Clone)]
    pub(super) enum Classifier {
        Never,
        ProtocolNacks,
        Dyn(std::sync::Arc<dyn Classify>),
    }

    impl Classifier {
        pub(super) fn classify<B>(
            &self,
            req: &super::Req,
            res: &Result<http::Response<B>, crate::Error>,
        ) -> Action {
            let req_rep = ReqRep(req, res.as_ref().map(|r| r.status()));
            match self {
                Self::Never => Action::Success,
                Self::ProtocolNacks => {
                    if req_rep.is_protocol_nack() {
                        Action::Retryable
                    } else {
                        Action::Success
                    }
                }
                Self::Dyn(c) => c.classify(req_rep),
            }
        }
    }

    impl std::fmt::Debug for Classifier {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Never => f.write_str("Never"),
                Self::ProtocolNacks => f.write_str("ProtocolNacks"),
                Self::Dyn(_) => f.write_str("Classifier"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    /// Build a `crate::Error` whose source is an `h2::Error`, mirroring how
    /// the refactored h2 connector surfaces protocol nacks via
    /// `error::request(h2_err)` (no `hyper_util::client::legacy::Error`
    /// nesting).
    fn h2_error_as_primp_err(reason: h2::Reason) -> crate::Error {
        crate::error::request(h2::Error::from(reason))
    }

    #[test]
    fn h2_error_is_reached_in_source_chain() {
        // Regression for the fixed-depth `.source()` pop bug: the old code
        // popped crate::Error -> inner, then popped AGAIN expecting a
        // `hyper_util::client::legacy::Error`, and so skipped past the h2
        // error entirely (returning false for every h2 nack). The rewritten
        // `is_retryable_error` walks the whole chain, so an h2 error that is
        // directly the source is now inspected.
        //
        // `h2::Error::from(Reason)` yields a `Kind::Reason` error (neither a
        // RST_STREAM nor a GOAWAY frame), which the real classifier treats as
        // non-retryable — a genuinely retryable nack is a *received*
        // RST_STREAM/GOAWAY frame, which h2 only constructs internally and
        // exposes no public constructor for. The assertion here verifies the
        // h2 error is reached and classified (rather than skipped).
        let err = h2_error_as_primp_err(h2::Reason::REFUSED_STREAM);
        let inner = err
            .source()
            .and_then(|e| e.downcast_ref::<h2::Error>())
            .expect("h2::Error must be directly downcastable from the source");
        assert_eq!(inner.reason(), Some(h2::Reason::REFUSED_STREAM));
        assert!(
            !is_retryable_error(&err),
            "a bare Reason error is not a retryable RST_STREAM/GOAWAY frame"
        );
    }

    #[test]
    fn non_protocol_error_is_not_retryable() {
        let err = crate::error::request("some unrelated failure");
        assert!(
            !is_retryable_error(&err),
            "plain error must not be retryable"
        );
    }

    #[test]
    fn out_of_scope_successes_do_not_inflate_the_budget() {
        // Mirror of `out_of_scope_retryables_do_not_drain_the_budget` (the
        // integration test): the withdraw side is scope-gated, and the
        // deposit side must be too. Unrelated traffic must neither drain
        // nor fill the shared budget.
        use tower::retry::budget::Budget as _;
        use tower::retry::Policy as _;

        fn drain(policy: &mut Policy) -> u64 {
            let mut n = 0;
            while policy
                .budget
                .as_ref()
                .map(|b| b.withdraw())
                .unwrap_or(false)
            {
                n += 1;
            }
            n
        }

        fn feed_out_of_scope_successes(policy: &mut Policy, count: usize) {
            for _ in 0..count {
                let mut req = http::Request::builder()
                    .uri("http://out-of-scope/")
                    .body(crate::async_impl::body::Body::empty())
                    .unwrap();
                let mut result: crate::Result<http::Response<()>> = Ok(http::Response::default());
                assert!(
                    policy.retry(&mut req, &mut result).is_none(),
                    "out-of-scope success must not be retried"
                );
            }
        }

        // Control: a fresh budget, never touched by out-of-scope traffic.
        let mut control = for_host("in-scope")
            .classify_fn(|_| classify::Action::Success)
            .into_policy();
        let baseline = drain(&mut control);
        assert!(baseline > 0, "budget probe must be able to withdraw");

        // Same fresh budget, but 100 out-of-scope successes flow through it.
        let mut fed = for_host("in-scope")
            .classify_fn(|_| classify::Action::Success)
            .into_policy();
        feed_out_of_scope_successes(&mut fed, 100);
        assert_eq!(
            drain(&mut fed),
            baseline,
            "out-of-scope successes must not deposit into the shared budget"
        );
    }
}
