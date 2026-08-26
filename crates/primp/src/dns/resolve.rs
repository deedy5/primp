use hyper_util::client::legacy::connect::dns::Name as HyperName;
use tower_service::Service;

use std::future::Future;
use std::net::SocketAddr;

use foldhash::HashMap;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::error::BoxError;

/// Alias for an `Iterator` trait object over `SocketAddr`.
pub type Addrs = Box<dyn Iterator<Item = SocketAddr> + Send>;

/// Alias for the `Future` type returned by a DNS resolver.
pub type Resolving = Pin<Box<dyn Future<Output = Result<Addrs, BoxError>> + Send>>;

/// Trait for customizing DNS resolution in primp.
pub trait Resolve: Send + Sync {
    /// Resolves a `Name` to an iterator of `SocketAddr`s.
    ///
    /// Unlike `tower_service::Service<Name>`, `resolve` is always ready to
    /// poll, needs no `&mut self`, and boxes its `Future`/`Iterator` to avoid
    /// associated types. An explicit URL port overrides the resolved port; a
    /// resolved port `0` becomes the scheme's conventional port (e.g. 80).
    fn resolve(&self, name: Name) -> Resolving;
}

/// A name that must be resolved to addresses.
#[derive(Debug, Clone)]
pub struct Name(pub(super) HyperName);

/// A more general trait implemented for types implementing `Resolve`.
///
/// Unnameable, only exported to aid seeing what implements this.
pub trait IntoResolve {
    #[doc(hidden)]
    fn into_resolve(self) -> Arc<dyn Resolve>;
}

impl Name {
    /// View the name as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for Name {
    type Err = sealed::InvalidNameError;

    fn from_str(host: &str) -> Result<Self, Self::Err> {
        HyperName::from_str(host)
            .map(Name)
            .map_err(|_| sealed::InvalidNameError { _ext: () })
    }
}

#[derive(Clone)]
pub(crate) struct DynResolver {
    resolver: Arc<dyn Resolve>,
}

impl DynResolver {
    pub(crate) fn new(resolver: Arc<dyn Resolve>) -> Self {
        Self { resolver }
    }

    pub(crate) fn gai() -> Self {
        Self::new(Arc::new(super::gai::GaiResolver::new()))
    }

    /// Resolve an HTTP host and port (like hyper-util's `HttpConnector` before it
    /// delegates to the underlying resolver).
    pub(crate) async fn http_resolve(
        &self,
        target: &http::Uri,
    ) -> Result<impl Iterator<Item = std::net::SocketAddr>, BoxError> {
        let host = target.host().ok_or("missing host")?;
        // `Uri::host()` keeps IPv6 brackets (`[::1]`), but getaddrinfo and
        // the overrides table expect the bare literal — otherwise resolution
        // fails with a DNS error.
        let host = crate::strip_ipv6_brackets(host);
        let port = target
            .port_u16()
            .unwrap_or_else(|| match target.scheme_str() {
                Some("https") => 443,
                Some("socks4") | Some("socks4a") | Some("socks5") | Some("socks5h") => 1080,
                _ => 80,
            });

        let explicit_port = target.port().is_some();

        let addrs = self
            .resolver
            .resolve(host.parse()?)
            .await
            .map_err(crate::error::dns)?;

        Ok(addrs.map(move |mut addr| {
            if explicit_port || addr.port() == 0 {
                addr.set_port(port);
            }
            addr
        }))
    }
}

impl Service<HyperName> for DynResolver {
    type Response = Addrs;
    type Error = BoxError;
    type Future = Resolving;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: HyperName) -> Self::Future {
        let resolving = self.resolver.resolve(Name(name));
        // Tag resolution failures so `Error::is_dns` can recognize them once
        Box::pin(async move { resolving.await.map_err(crate::error::dns) })
    }
}

pub(crate) struct DnsResolverWithOverrides {
    dns_resolver: Arc<dyn Resolve>,
    overrides: Arc<HashMap<String, Vec<SocketAddr>>>,
}

impl DnsResolverWithOverrides {
    pub(crate) fn new(
        dns_resolver: Arc<dyn Resolve>,
        overrides: HashMap<String, Vec<SocketAddr>>,
    ) -> Self {
        // DNS hostnames are case-insensitive, so normalize override keys to
        // lowercase so that an override registered as `Example.com` still
        // matches a query for `example.com`.
        let overrides = overrides
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect();
        DnsResolverWithOverrides {
            dns_resolver,
            overrides: Arc::new(overrides),
        }
    }
}

impl Resolve for DnsResolverWithOverrides {
    fn resolve(&self, name: Name) -> Resolving {
        match self.overrides.get(&name.as_str().to_ascii_lowercase()) {
            Some(dest) => {
                let addrs: Addrs = Box::new(dest.clone().into_iter());
                Box::pin(std::future::ready(Ok(addrs)))
            }
            None => self.dns_resolver.resolve(name),
        }
    }
}

impl IntoResolve for Arc<dyn Resolve> {
    fn into_resolve(self) -> Arc<dyn Resolve> {
        self
    }
}

impl<R> IntoResolve for Arc<R>
where
    R: Resolve + 'static,
{
    fn into_resolve(self) -> Arc<dyn Resolve> {
        self
    }
}

impl<R> IntoResolve for R
where
    R: Resolve + 'static,
{
    fn into_resolve(self) -> Arc<dyn Resolve> {
        Arc::new(self)
    }
}

/// Chains multiple resolvers: tries each in order, returning the first success.
struct ChainedResolver {
    resolvers: Vec<Arc<dyn Resolve>>,
}

impl Resolve for ChainedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolvers = self.resolvers.clone();
        Box::pin(async move {
            let mut last_err: Option<BoxError> = None;
            for resolver in &resolvers {
                match resolver.resolve(name.clone()).await {
                    Ok(addrs) => return Ok(addrs),
                    Err(e) => last_err = Some(e),
                }
            }
            // Tag the final error as a DNS failure so `Error::is_dns()` is
            // accurate even when the inner resolvers return untagged errors and
            // this chain is consumed without the `DnsCacheResolver`/
            // `DynResolver` wrappers that would otherwise add the tag.
            // Avoid double-tagging an already-tagged `DnsError` (same pattern
            // as `dns::cache::ensure_dns_tagged`).
            let err = last_err.unwrap_or_else(|| "all DNS resolvers failed".into());
            if err.is::<crate::error::DnsError>() {
                Err(err)
            } else {
                Err(crate::error::dns(err))
            }
        })
    }
}

impl<R: Resolve + 'static> IntoResolve for Vec<R> {
    fn into_resolve(self) -> Arc<dyn Resolve> {
        if self.len() == 1 {
            return Arc::new(self.into_iter().next().expect("self.len() == 1 confirmed"));
        }
        Arc::new(ChainedResolver {
            resolvers: self
                .into_iter()
                .map(|r| Arc::new(r) as Arc<dyn Resolve>)
                .collect(),
        })
    }
}

impl IntoResolve for Vec<Arc<dyn Resolve>> {
    fn into_resolve(self) -> Arc<dyn Resolve> {
        if self.len() == 1 {
            return self.into_iter().next().expect("self.len() == 1 confirmed");
        }
        Arc::new(ChainedResolver { resolvers: self })
    }
}

mod sealed {
    use std::fmt;

    #[derive(Debug)]
    pub struct InvalidNameError {
        pub(super) _ext: (),
    }

    impl fmt::Display for InvalidNameError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("invalid DNS name")
        }
    }

    impl std::error::Error for InvalidNameError {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records the names it is asked to resolve.
    #[derive(Clone)]
    struct RecordingResolver {
        names: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Resolve for RecordingResolver {
        fn resolve(&self, name: Name) -> Resolving {
            self.names.lock().unwrap().push(name.as_str().to_owned());
            let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
            let addrs: Addrs = Box::new([addr].into_iter());
            Box::pin(std::future::ready(Ok(addrs)))
        }
    }

    #[tokio::test]
    async fn http_resolve_strips_ipv6_brackets_before_resolving() {
        // `Uri::host()` returns the bracketed form (`[::1]`); getaddrinfo
        // rejects it, so `https://[::1]/` broke with a DNS error on the h2
        // path while the h1 path worked.
        let names = Arc::new(std::sync::Mutex::new(Vec::new()));
        let resolver = DynResolver::new(Arc::new(RecordingResolver {
            names: names.clone(),
        }));

        let uri: http::Uri = "https://[::1]:8443/".parse().unwrap();
        let addrs = resolver
            .http_resolve(&uri)
            .await
            .expect("resolving an IPv6 literal must succeed");

        assert_eq!(
            *names.lock().unwrap(),
            vec!["::1".to_owned()],
            "the underlying resolver must receive the unbracketed host"
        );
        let collected: Vec<_> = addrs.collect();
        assert_eq!(collected[0].port(), 8443, "explicit port must be applied");
    }

    #[tokio::test]
    async fn http_resolve_keeps_hostname_unchanged() {
        let names = Arc::new(std::sync::Mutex::new(Vec::new()));
        let resolver = DynResolver::new(Arc::new(RecordingResolver {
            names: names.clone(),
        }));

        let uri: http::Uri = "https://example.com/".parse().unwrap();
        let addrs = resolver
            .http_resolve(&uri)
            .await
            .expect("resolving a hostname must succeed");
        let _: Vec<_> = addrs.collect();

        assert_eq!(
            *names.lock().unwrap(),
            vec!["example.com".to_owned()],
            "plain hostnames must not be modified"
        );
    }

    // Mirrors `h3_client/connect.rs:437` trio — `http_resolve` must keep the
    // same port semantics as the H3 helper (explicit wins, non-zero kept, zero
    // replaced). Without this the custom-resolver port would be clobbered.

    struct FixedPortResolver(u16);

    impl Resolve for FixedPortResolver {
        fn resolve(&self, _name: Name) -> Resolving {
            let addr: std::net::SocketAddr = format!("127.0.0.1:{}", self.0).parse().unwrap();
            let addrs: Addrs = Box::new([addr].into_iter());
            Box::pin(std::future::ready(Ok(addrs)))
        }
    }

    #[tokio::test]
    async fn http_resolve_respects_explicit_uri_ports() {
        let resolver = DynResolver::new(Arc::new(FixedPortResolver(6881)));
        let uri: http::Uri = "https://example.com:42/".parse().unwrap();
        let mut iter = resolver.http_resolve(&uri).await.unwrap();
        assert_eq!(iter.next().unwrap().port(), 42);
    }

    #[tokio::test]
    async fn http_resolve_keeps_non_zero_resolved_ports() {
        let resolver = DynResolver::new(Arc::new(FixedPortResolver(6881)));
        let uri: http::Uri = "https://example.com/".parse().unwrap();
        let mut iter = resolver.http_resolve(&uri).await.unwrap();
        assert_eq!(iter.next().unwrap().port(), 6881);
    }

    #[tokio::test]
    async fn http_resolve_uses_default_when_resolved_port_is_zero() {
        let resolver = DynResolver::new(Arc::new(FixedPortResolver(0)));
        let uri: http::Uri = "https://example.com/".parse().unwrap();
        let mut iter = resolver.http_resolve(&uri).await.unwrap();
        assert_eq!(iter.next().unwrap().port(), 443);
    }
}
