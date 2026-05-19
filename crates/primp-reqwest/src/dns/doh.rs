//! DNS-over-HTTPS (DoH) resolution via hickory-resolver

use hickory_resolver::config::{LookupIpStrategy, NameServerConfig, ResolverConfig};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::TokioResolver;

use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Addrs, Name, Resolve, Resolving, SocketAddrs};
use super::gai::GaiResolver;
use crate::error::BoxError;

/// A DNS-over-HTTPS resolver backed by hickory-resolver.
pub struct DohResolver {
    state: Arc<Mutex<Option<Arc<TokioResolver>>>>,
    bootstrap: Arc<dyn Resolve>,
    doh_host: String,
    doh_path: String,
    doh_port: u16,
}

impl std::fmt::Debug for DohResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DohResolver")
            .field("doh_host", &self.doh_host)
            .field("doh_path", &self.doh_path)
            .field("doh_port", &self.doh_port)
            .finish()
    }
}

impl Clone for DohResolver {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            bootstrap: self.bootstrap.clone(),
            doh_host: self.doh_host.clone(),
            doh_path: self.doh_path.clone(),
            doh_port: self.doh_port,
        }
    }
}

impl DohResolver {
    /// Create a new DoH resolver from a URL like `https://cloudflare-dns.com/dns-query`.
    ///
    /// The host is resolved via the system resolver (GaiResolver) on first lookup.
    pub fn new(url: &str) -> Result<Self, BoxError> {
        let parsed = url::Url::parse(url)?;
        let host = parsed.host_str().ok_or("DoH URL must have a host")?.to_string();
        // Strip IPv6 brackets; url::host_str() includes them
        let host = host.trim_start_matches('[').trim_end_matches(']').to_string();
        let port = parsed.port().unwrap_or(443);
        let path = parsed.path().to_string();
        let bootstrap: Arc<dyn Resolve> = Arc::new(GaiResolver::new());
        Ok(Self {
            state: Arc::new(Mutex::new(None)),
            bootstrap,
            doh_host: host,
            doh_path: path,
            doh_port: port,
        })
    }

    async fn get_resolver(&self) -> Result<Arc<TokioResolver>, BoxError> {
        if let Some(ref resolver) = *self.state.lock().unwrap() {
            return Ok(resolver.clone());
        }

        let addrs = self
            .bootstrap
            .resolve(Name::from_str(&self.doh_host)?)
            .await?;
        let ips: Vec<IpAddr> = addrs.map(|a| a.ip()).collect();

        let name_servers: Vec<NameServerConfig> = ips
            .iter()
            .map(|&ip| {
                NameServerConfig::https(
                    ip,
                    self.doh_host.clone().into(),
                    Some(self.doh_path.clone().into()),
                )
            })
            .collect();
        let config = ResolverConfig::from_parts(None, vec![], name_servers);

        let mut builder =
            TokioResolver::builder_with_config(config, TokioRuntimeProvider::default());
        let opts = builder.options_mut();
        opts.timeout = Duration::from_secs(5);
        opts.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
        let resolver = Arc::new(builder.build().expect("failed to build DoH resolver"));

        let mut guard = self.state.lock().unwrap();
        if guard.is_none() {
            *guard = Some(resolver.clone());
        }
        Ok(guard.as_ref().unwrap().clone())
    }
}

impl Resolve for DohResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let this = self.clone();
        Box::pin(async move {
            let resolver = this.get_resolver().await?;
            let lookup = tokio::time::timeout(Duration::from_secs(5), resolver.lookup_ip(name.as_str()))
                .await
                .map_err(|_| BoxError::from("DoH lookup timed out"))?
                .map_err(BoxError::from)?;
            let ips: Vec<IpAddr> = lookup.iter().collect();
            let addrs: Addrs = Box::new(SocketAddrs {
                iter: ips.into_iter(),
            });
            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;

    #[test]
    fn new_cloudflare() {
        let resolver = DohResolver::new("https://cloudflare-dns.com/dns-query").unwrap();
        assert_eq!(resolver.doh_host, "cloudflare-dns.com");
        assert_eq!(resolver.doh_port, 443);
        assert_eq!(resolver.doh_path, "/dns-query");
    }

    #[test]
    fn new_custom_port() {
        let resolver = DohResolver::new("https://dns.google:8443/dns-query").unwrap();
        assert_eq!(resolver.doh_host, "dns.google");
        assert_eq!(resolver.doh_port, 8443);
        assert_eq!(resolver.doh_path, "/dns-query");
    }

    #[test]
    fn new_ipv6_literal() {
        let resolver = DohResolver::new("https://[2606:4700:4700::1111]/dns-query").unwrap();
        assert_eq!(resolver.doh_host, "2606:4700:4700::1111");
        assert_eq!(resolver.doh_port, 443);
        assert_eq!(resolver.doh_path, "/dns-query");
    }

    #[test]
    fn new_rejects_invalid_url() {
        let err = DohResolver::new("not a url").unwrap_err();
        assert!(err.to_string().contains("relative URL"), "{err}");
    }

    #[test]
    fn builder_creates_with_doh_resolver() {
        let resolver = DohResolver::new("https://cloudflare-dns.com/dns-query").unwrap();
        let client = Client::builder()
            .dns_resolver(resolver)
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn builder_creates_with_dot_resolver() {
        use crate::dns::dot::DotResolver;
        let resolver = DotResolver::new("1.1.1.1");
        let client = Client::builder()
            .dns_resolver(resolver)
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn builder_creates_with_multi_resolver() {
        let r1: Arc<dyn Resolve> = Arc::new(
            DohResolver::new("https://cloudflare-dns.com/dns-query").unwrap(),
        );
        let r2: Arc<dyn Resolve> = Arc::new(crate::dns::gai::GaiResolver::new());
        let client = Client::builder()
            .dns_resolver(vec![r1, r2])
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn debug_output() {
        let resolver = DohResolver::new("https://cloudflare-dns.com:8443/custom-path").unwrap();
        let debug = format!("{:?}", resolver);
        assert!(debug.contains("cloudflare-dns.com"), "{debug}");
        assert!(debug.contains("/custom-path"), "{debug}");
        assert!(debug.contains("8443"), "{debug}");
    }
}
