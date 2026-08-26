use std::error::Error as StdError;
use std::fmt;
use std::io;

use crate::util::Escape;
use crate::{StatusCode, Url};

/// A `Result` alias where the `Err` case is `primp::Error`.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that may occur while processing a `Request`. The stored URL may
/// contain sensitive info (e.g. an API key in the query); strip it with
/// [`without_url`](Error::without_url).
pub struct Error {
    inner: Box<Inner>,
}

pub(crate) type BoxError = Box<dyn StdError + Send + Sync>;

struct Inner {
    kind: Kind,
    source: Option<BoxError>,
    url: Option<Url>,
}

impl Error {
    pub(crate) fn new<E>(kind: Kind, source: Option<E>) -> Error
    where
        E: Into<BoxError>,
    {
        Error {
            inner: Box::new(Inner {
                kind,
                source: source.map(Into::into),
                url: None,
            }),
        }
    }

    /// Returns the URL related to this error, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn run() {
    /// // displays last stop of a redirect loop
    /// let response = primp::get("http://site.with.redirect.loop").await;
    /// if let Err(e) = response {
    ///     if e.is_redirect() {
    ///         if let Some(final_stop) = e.url() {
    ///             println!("redirect loop at {final_stop}");
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub fn url(&self) -> Option<&Url> {
        self.inner.url.as_ref()
    }

    /// Returns a mutable reference to the related URL, so sensitive info
    /// (e.g. an API key in the query) can be removed without dropping the URL.
    pub fn url_mut(&mut self) -> Option<&mut Url> {
        self.inner.url.as_mut()
    }

    /// Attach a URL to this error, replacing any existing one (userinfo is redacted).
    pub fn with_url(mut self, url: Url) -> Self {
        self.inner.url = Some(redact_url(url));
        self
    }

    pub(crate) fn if_no_url(mut self, f: impl FnOnce() -> Url) -> Self {
        if self.inner.url.is_none() {
            self.inner.url = Some(redact_url(f()));
        }
        self
    }

    /// Strip the related URL from this error (e.g. if it contains sensitive information).
    pub fn without_url(mut self) -> Self {
        self.inner.url = None;
        self
    }

    /// Returns true if this error originated from a `Builder`.
    pub fn is_builder(&self) -> bool {
        matches!(self.inner.kind, Kind::Builder)
    }

    /// Returns true if this error came from a `RedirectPolicy`.
    pub fn is_redirect(&self) -> bool {
        matches!(self.inner.kind, Kind::Redirect)
    }

    /// Returns true if this error came from `Response::error_for_status`.
    pub fn is_status(&self) -> bool {
        matches!(self.inner.kind, Kind::Status(_, _))
    }

    /// Search the source chain (descending through `io::Error` and boxed
    /// `io::Error` wrappers the h1/h2/h3 connectors use) for an `io::Error`.
    fn find_io_error<'a>(mut err: &'a (dyn StdError + 'static)) -> Option<&'a io::Error> {
        // Guard against pathological cycles in the source chain.
        for _ in 0..64 {
            if let Some(io_err) = err.downcast_ref::<io::Error>() {
                return Some(io_err);
            }
            // The h2/h3 connectors wrap raw `io::Error`s as `Box<io::Error>`
            // before handing them to `error::request`, so the concrete type
            // at this node may be `Box<io::Error>` rather than `io::Error`.
            if let Some(io_err) = err.downcast_ref::<Box<io::Error>>() {
                return Some(io_err);
            }
            err = err.source()?;
        }
        None
    }

    /// Returns true if this error was caused by a timeout.
    pub fn is_timeout(&self) -> bool {
        let mut source = self.source();
        // Guard against pathological cycles in the source chain.
        for _ in 0..64 {
            let Some(err) = source else { break };

            if err.is::<TimedOut>() {
                return true;
            }
            if let Some(err) = err.downcast_ref::<Error>() {
                if err.is_timeout() {
                    return true;
                }
            }
            if let Some(hyper_err) = err.downcast_ref::<hyper::Error>() {
                if hyper_err.is_timeout() {
                    return true;
                }
            }
            if let Some(io) = Self::find_io_error(err) {
                if io.kind() == io::ErrorKind::TimedOut {
                    return true;
                }
            }
            source = err.source();
        }

        false
    }

    /// Returns true if the error is related to the request
    pub fn is_request(&self) -> bool {
        matches!(self.inner.kind, Kind::Request)
    }

    /// Returns true if this error occurred while *establishing* a connection.
    ///
    /// Mid-request transport failures (`ConnectionReset`/`ConnectionAborted`/
    /// `BrokenPipe`) are intentionally NOT treated as connect errors — they mean
    /// the connection dropped mid-request. For those also check [`is_request`](Error::is_request).
    /// Body-read failures (mid-download drops) surface as `is_decode()` or
    /// `is_body()` instead (reqwest heritage: body frames and decompression
    /// both classify as decode), so they are NOT covered by `is_request()`.
    pub fn is_connect(&self) -> bool {
        // A name-resolution failure is never a connection failure. A DNS
        // *timeout* is wrapped as `dns(io::Error(TimedOut))`, so its inner
        // `io::Error(TimedOut)` would otherwise be misclassified as a connect
        // error by the walk below. Short-circuit on the DNS tag (which also
        // covers the `Arc`/`ArcErr`-wrapped variants the cache produces).
        // `is_timeout()` still reports `true` for a DNS timeout via
        // `find_io_error`, so only the connect classification is suppressed.
        if self.is_dns() {
            return false;
        }

        let mut source = self.source();
        // Guard against pathological cycles in the source chain.
        for _ in 0..64 {
            let Some(err) = source else { break };

            if let Some(hyper_err) = err.downcast_ref::<hyper_util::client::legacy::Error>() {
                if hyper_err.is_connect() {
                    return true;
                }
            }

            if let Some(io_err) = Self::find_io_error(err) {
                if matches!(
                    io_err.kind(),
                    io::ErrorKind::ConnectionRefused
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::AddrNotAvailable
                        | io::ErrorKind::AddrInUse
                        | io::ErrorKind::HostUnreachable
                        | io::ErrorKind::NetworkUnreachable
                ) {
                    return true;
                }
            }

            source = err.source();
        }

        false
    }

    /// Returns true if this error was caused by a DNS resolution failure.
    pub fn is_dns(&self) -> bool {
        let mut source = self.source();
        // Guard against pathological cycles in the source chain.
        for _ in 0..64 {
            let Some(err) = source else { break };

            if err.is::<DnsError>() {
                return true;
            }

            // Recurse into any nested `Error` so a `DnsError` wrapped deeper
            // in the source chain (e.g. by a `DynResolver`) is still detected.
            if let Some(err) = err.downcast_ref::<Error>() {
                if err.is_dns() {
                    return true;
                }
            }

            source = err.source();
        }

        false
    }

    /// Returns true if this error is related to the request or response body.
    pub fn is_body(&self) -> bool {
        matches!(self.inner.kind, Kind::Body)
    }

    /// Returns true if this error is from decoding the response body.
    pub fn is_decode(&self) -> bool {
        matches!(self.inner.kind, Kind::Decode)
    }

    /// Returns true if this error is from an invalid JSON request body.
    #[cfg(feature = "json")]
    pub fn is_json(&self) -> bool {
        matches!(self.inner.kind, Kind::Json)
    }

    /// Returns true if this error came from reading a response body stream
    /// after it had already been exhausted.
    pub fn is_stream_exhausted(&self) -> bool {
        matches!(self.inner.kind, Kind::StreamExhausted)
    }

    /// Returns the HTTP status code, if this error came from a response.
    pub fn status(&self) -> Option<StatusCode> {
        match self.inner.kind {
            Kind::Status(code, _) => Some(code),
            _ => None,
        }
    }

    /// Returns true if this error is from a protocol upgrade request.
    pub fn is_upgrade(&self) -> bool {
        matches!(self.inner.kind, Kind::Upgrade)
    }

    // private

    #[cfg(test)]
    pub(crate) fn into_io(self) -> io::Error {
        io::Error::other(self)
    }
}

/// Converts an external error into primp's internal representation. Currently
/// only `tower::timeout::error::Elapsed` needs conversion.
pub(crate) fn cast_to_internal_error(error: BoxError) -> BoxError {
    if error.is::<tower::timeout::error::Elapsed>() {
        // Surface connect timeouts as an `io::Error` with `TimedOut` kind so
        // that `Error::is_connect()` (which only inspects `io::Error` kinds
        // and `hyper_util` errors) classifies them as connection failures.
        Box::new(io::Error::new(io::ErrorKind::TimedOut, "connect timeout")) as BoxError
    } else {
        error
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("primp::Error");

        builder.field("kind", &self.inner.kind);

        if let Some(ref url) = self.inner.url {
            builder.field("url", &url.as_str());
        }
        if let Some(ref source) = self.inner.source {
            builder.field("source", source);
        }

        builder.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.inner.kind {
            Kind::Builder => f.write_str("builder error")?,
            Kind::Request => f.write_str("error sending request")?,
            Kind::Body => f.write_str("request or response body error")?,
            Kind::Decode => f.write_str("error decoding response body")?,
            Kind::Redirect => f.write_str("error following redirect")?,
            Kind::Upgrade => f.write_str("error upgrading connection")?,
            #[cfg(feature = "json")]
            Kind::Json => f.write_str("error serializing JSON request body")?,
            Kind::StreamExhausted => f.write_str("response body stream has been exhausted")?,
            Kind::Status(ref code, ref reason) => {
                let prefix = if code.is_client_error() {
                    "HTTP status client error"
                } else {
                    debug_assert!(code.is_server_error());
                    "HTTP status server error"
                };
                if let Some(reason) = reason {
                    write!(
                        f,
                        "{prefix} ({} {})",
                        code.as_str(),
                        Escape::new(reason.as_bytes())
                    )?;
                } else {
                    write!(f, "{prefix} ({code})")?;
                }
            }
        };

        if let Some(url) = &self.inner.url {
            write!(f, " for url ({url})")?;
        }

        Ok(())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source.as_ref().map(|e| &**e as _)
    }
}

#[derive(Debug)]
pub(crate) enum Kind {
    Builder,
    Request,
    Redirect,
    Status(StatusCode, Option<hyper::ext::ReasonPhrase>),
    Body,
    Decode,
    Upgrade,
    #[cfg(feature = "json")]
    Json,
    StreamExhausted,
}

// constructors

pub(crate) fn builder<E: Into<BoxError>>(e: E) -> Error {
    Error::new(Kind::Builder, Some(e))
}

pub(crate) fn body<E: Into<BoxError>>(e: E) -> Error {
    Error::new(Kind::Body, Some(e))
}

pub(crate) fn decode<E: Into<BoxError>>(e: E) -> Error {
    Error::new(Kind::Decode, Some(e))
}

#[cfg(feature = "json")]
pub(crate) fn json<E: Into<BoxError>>(e: E) -> Error {
    Error::new(Kind::Json, Some(e))
}

pub(crate) fn stream_exhausted() -> Error {
    Error::new(Kind::StreamExhausted, None::<Error>)
}

pub(crate) fn request<E: Into<BoxError>>(e: E) -> Error {
    Error::new(Kind::Request, Some(e))
}

/// Build a `Request`-kind error for a fallback attempt that ALSO failed,
/// keeping the original failure in the chain. The latest (fallback) error stays
/// the primary `source`, so `is_connect`/`is_dns`/`is_timeout` classify the
/// final failure, while the original remains visible in `Display`/`Debug`.
pub(crate) fn request_with_previous<E, S>(fallback: E, original: S) -> Error
where
    E: Into<BoxError>,
    S: Into<BoxError>,
{
    Error::new(
        Kind::Request,
        Some(WithPrevious {
            current: fallback.into(),
            previous: original.into(),
        }),
    )
}

/// Renders the latest failure plus a hint that an earlier attempt also failed;
/// the latest failure is the `source()`.
#[derive(Debug)]
struct WithPrevious {
    current: BoxError,
    previous: BoxError,
}

/// Render an error and its (bounded) source chain joined by `": "` so the leaf
/// cause is visible in `Display`.
fn chain_to_string(err: &(dyn StdError + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut cur = err.source();
    for _ in 0..64 {
        let Some(src) = cur else { break };
        parts.push(src.to_string());
        cur = src.source();
    }
    parts.join(": ")
}

impl fmt::Display for WithPrevious {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} (after an earlier failure: {})",
            chain_to_string(&*self.current),
            chain_to_string(&*self.previous)
        )
    }
}

impl StdError for WithPrevious {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.current.as_ref())
    }
}

pub(crate) fn dns<E: Into<BoxError>>(e: E) -> BoxError {
    Box::new(DnsError { inner: e.into() })
}

/// Strip any userinfo (`user:password@`) from a URL before storing it on an
/// error, so credentials can never leak via `Display`/`Debug` or `url()`.
/// URLs whose scheme cannot carry userinfo are returned unchanged.
pub(crate) fn redact_url(mut url: Url) -> Url {
    if url.username().is_empty() && url.password().is_none() {
        return url;
    }
    // These setters only fail for schemes that cannot have authority (e.g.
    // "mailto:"); such URLs cannot carry userinfo, so ignore the error.
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url
}

pub(crate) fn redirect<E: Into<BoxError>>(e: E, url: Url) -> Error {
    Error::new(Kind::Redirect, Some(e)).with_url(url)
}

pub(crate) fn status_code(
    url: Url,
    status: StatusCode,
    reason: Option<hyper::ext::ReasonPhrase>,
) -> Error {
    Error::new(Kind::Status(status, reason), None::<Error>).with_url(url)
}

pub(crate) fn url_bad_scheme(url: Url) -> Error {
    Error::new(Kind::Builder, Some(BadScheme)).with_url(url)
}

pub(crate) fn url_invalid_uri(url: Url) -> Error {
    Error::new(Kind::Builder, Some("Parsed Url is not a valid Uri")).with_url(url)
}

pub(crate) fn upgrade<E: Into<BoxError>>(e: E) -> Error {
    Error::new(Kind::Upgrade, Some(e))
}

// io::Error helpers

#[cfg(test)]
pub(crate) fn decode_io(e: io::Error) -> Error {
    if e.get_ref().map(|r| r.is::<Error>()).unwrap_or(false) {
        *e.into_inner()
            .expect("io::Error::get_ref was Some(_)")
            .downcast::<Error>()
            .expect("StdError::is() was true")
    } else {
        decode(e)
    }
}

// internal Error "sources"

#[derive(Debug)]
pub(crate) struct TimedOut;

impl fmt::Display for TimedOut {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("operation timed out")
    }
}

impl StdError for TimedOut {}

#[derive(Debug)]
pub(crate) struct BadScheme;

impl fmt::Display for BadScheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("URL scheme is not allowed")
    }
}

impl StdError for BadScheme {}

#[derive(Debug)]
pub(crate) struct DnsError {
    pub(crate) inner: BoxError,
}

impl fmt::Display for DnsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("error resolving DNS")
    }
}

impl StdError for DnsError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&*self.inner as _)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_source_chain() {
        let root = Error::new(Kind::Request, None::<Error>);
        assert!(root.source().is_none());

        let link = super::body(root);
        assert!(link.source().is_some());
        assert_send::<Error>();
        assert_sync::<Error>();
    }

    #[test]
    fn request_with_previous_keeps_both_failures() {
        let original = super::request(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "pooled conn reset",
        ));
        let fallback =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "fallback refused");
        let chained = super::request_with_previous(fallback, original);

        // The latest (fallback) failure is the visible source; its Display
        // carries both the final and original failure messages.
        assert!(chained.is_request());
        assert!(
            chained.is_connect(),
            "fallback ConnectionRefused must classify as connect"
        );
        let src = chained.source().unwrap().to_string();
        assert!(
            src.contains("fallback refused"),
            "source displays fallback: {src}"
        );
        assert!(
            src.contains("pooled conn reset"),
            "source keeps original: {src}"
        );
        // Debug also exposes both failures for the diagnostics path.
        let dbg = format!("{chained:?}");
        assert!(
            dbg.contains("fallback refused"),
            "debug shows fallback: {dbg}"
        );
        assert!(
            dbg.contains("pooled conn reset"),
            "debug keeps original: {dbg}"
        );
    }

    #[test]
    fn request_with_previous_classifies_final_failure() {
        // The FINAL (fallback) failure drives classification — it is the last
        // attempt — while the original failure stays visible in Display/Debug.
        let original = super::request(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "pooled conn reset",
        ));
        let fallback = super::dns("fallback dns failure");
        let chained = super::request_with_previous(fallback, original);
        assert!(chained.is_request());
        assert!(chained.is_dns(), "final dns failure must classify as dns");
        assert!(
            !chained.is_connect(),
            "original ConnectionReset must not leak into classification"
        );
        let src = chained.source().unwrap().to_string();
        assert!(
            src.contains("fallback dns failure"),
            "source shows final failure: {src}"
        );
        assert!(
            src.contains("pooled conn reset"),
            "source keeps original failure: {src}"
        );
    }

    #[test]
    fn mem_size_of() {
        use std::mem::size_of;
        assert_eq!(size_of::<Error>(), size_of::<usize>());
    }

    #[test]
    fn with_url_redacts_userinfo() {
        let url = Url::parse("https://user:s3cret@example.com/path?token=abc").unwrap();
        let err = super::redirect("boom", url);
        let stored = err.url().unwrap();
        assert_eq!(stored.username(), "");
        assert!(stored.password().is_none());
        // Host, path and query are preserved.
        assert_eq!(stored.host_str(), Some("example.com"));
        assert_eq!(stored.path(), "/path");
        assert_eq!(stored.query(), Some("token=abc"));
        // Neither Display nor Debug may leak the credentials.
        assert!(!format!("{err}").contains("s3cret"));
        assert!(!format!("{err}").contains("user:"));
        assert!(!format!("{err:?}").contains("s3cret"));
    }

    #[test]
    fn redact_url_leaves_clean_url_untouched() {
        let url = Url::parse("https://example.com/x").unwrap();
        assert_eq!(super::redact_url(url.clone()), url);
    }

    #[test]
    fn roundtrip_io_error() {
        let orig = super::request("orig");
        // Convert crate::Error into an io::Error...
        let io = orig.into_io();
        // Convert that io::Error back into a crate::Error...
        let err = super::decode_io(io);
        // It should have pulled out the original, not nested it...
        match err.inner.kind {
            Kind::Request => (),
            _ => panic!("{err:?}"),
        }
    }

    #[test]
    fn from_unknown_io_error() {
        let orig = io::Error::other("orly");
        let err = super::decode_io(orig);
        match err.inner.kind {
            Kind::Decode => (),
            _ => panic!("{err:?}"),
        }
    }

    #[test]
    fn is_timeout() {
        let err = super::request(super::TimedOut);
        assert!(err.is_timeout());

        // todo: test `hyper::Error::is_timeout` when we can easily construct one

        let io = io::Error::from(io::ErrorKind::TimedOut);
        let nested = super::request(io);
        assert!(nested.is_timeout());
    }

    #[test]
    fn is_dns() {
        let err = super::request(DnsError { inner: "".into() });
        assert!(err.is_dns());
    }

    #[test]
    fn connect_timeout_is_classified_as_connect() {
        // The connect path now surfaces a connect timeout as an `io::Error`
        // with `TimedOut` kind (see `connect.rs` `with_timeout` and
        // `cast_to_internal_error`). It must read as both connect and timeout.
        let io_err = io::Error::new(io::ErrorKind::TimedOut, "connect timeout");
        let err = super::request(io_err);
        assert!(
            err.is_connect(),
            "io::Error(TimedOut) should be is_connect()"
        );
        assert!(
            err.is_timeout(),
            "io::Error(TimedOut) should be is_timeout()"
        );
    }

    #[test]
    fn request_timeout_bare_timed_out_is_not_connect() {
        // A request-level timeout uses a bare `TimedOut` and must NOT be
        // classified as a connect error.
        let err = super::request(super::TimedOut);
        assert!(!err.is_connect(), "bare TimedOut must not be is_connect()");
        assert!(
            err.is_timeout(),
            "bare TimedOut should still be is_timeout()"
        );
    }

    #[test]
    fn boxed_connect_io_error_is_classified_as_connect() {
        // The h2/h3 connectors wrap a raw `io::Error` as `Box<io::Error>`
        // before handing it to `error::request`. `is_connect` must still
        // detect the connect failure through that double-boxing.
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "connection refused");
        let err = super::request(Box::new(io_err));
        assert!(
            err.is_connect(),
            "Box<io::Error>(ConnectionRefused) should be is_connect()"
        );
        assert!(
            !err.is_dns(),
            "connection-refused error must not read as is_dns()"
        );
    }

    #[test]
    fn boxed_timed_out_io_error_is_connect_and_timeout() {
        let io_err = io::Error::new(io::ErrorKind::TimedOut, "connect timeout");
        let err = super::request(Box::new(io_err));
        assert!(
            err.is_connect(),
            "Box<io::Error>(TimedOut) should be is_connect()"
        );
        assert!(
            err.is_timeout(),
            "Box<io::Error>(TimedOut) should be is_timeout()"
        );
    }

    #[test]
    fn dns_error_is_not_connect() {
        // A DNS failure must not be misclassified as a connect error.
        let err = super::request(DnsError { inner: "".into() });
        assert!(err.is_dns());
        assert!(!err.is_connect(), "is_dns() error must not be is_connect()");
    }

    #[test]
    fn dns_timeout_is_not_connect() {
        // A DNS *timeout* wraps an `io::Error(TimedOut)` inside a `DnsError`.
        // `is_connect()` must not descend past the `DnsError` wrapper and
        // misclassify the inner `TimedOut` as a connection failure — it is a
        // DNS error (and a timeout) but never a connect error.
        let err = super::request(super::dns(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "DNS resolution timed out",
        )));
        assert!(err.is_dns());
        assert!(
            err.is_timeout(),
            "DNS timeout must still read as is_timeout()"
        );
        assert!(
            !err.is_connect(),
            "DnsError wrapping io::Error(TimedOut) must not be is_connect()"
        );
    }

    /// A pathological error whose `source()` returns itself, forming a cycle.
    /// The `is_*` source-chain walks must terminate (bounded) rather than
    /// spin forever under `panic="abort"`.
    #[derive(Debug)]
    struct CyclicError;

    impl std::fmt::Display for CyclicError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("cyclic")
        }
    }

    impl std::error::Error for CyclicError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self)
        }
    }

    #[test]
    fn cyclic_source_chain_terminates() {
        let err = super::request(CyclicError);
        // Must return (not loop forever); the cyclic error is neither dns nor
        // connect nor timeout, so all three return false.
        assert!(!err.is_dns());
        assert!(!err.is_connect());
        assert!(!err.is_timeout());
    }
}
