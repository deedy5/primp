use http::header::{ACCEPT_ENCODING, RANGE};
use http::{HeaderValue, Request, Response};
use std::task::{Context, Poll};
use tower::Service;

/// Forces `Accept-Encoding: identity` on requests carrying a `Range` header.
/// Overrides any explicit `Accept-Encoding`.
#[derive(Clone, Debug)]
pub(crate) struct RangeGuard<S> {
    inner: S,
}

impl<S> RangeGuard<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<ReqBody, ResBody, S> Service<Request<ReqBody>> for RangeGuard<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if req.headers().contains_key(RANGE) {
            req.headers_mut()
                .insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        }
        self.inner.call(req)
    }
}
