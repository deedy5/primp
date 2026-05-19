//! DNS-over-TLS (DoT) resolution via hickory-resolver

use hickory_resolver::{
    config::{LookupIpStrategy, NameServerConfig, NameServerConfigGroup, ResolverConfig},
    name_server::TokioConnectionProvider,
    proto::xfer::Protocol,
    TokioResolver,
};

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Addrs, Name, Resolve, Resolving, SocketAddrs};
use super::gai::GaiResolver;
use crate::error::BoxError;

/// A DNS-over-TLS resolver backed by hickory-resolver.
pub struct DotResolver {
    state: Arc<Mutex<Option<Arc<TokioResolver>>>>,
    bootstrap: Arc<dyn Resolve>,
    tls_host: String,
    tls_port: u16,
}

impl std::fmt::Debug for DotResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DotResolver")
            .field("tls_host", &self.tls_host)
            .field("tls_port", &self.tls_port)
            .finish()
    }
}

impl Clone for DotResolver {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            bootstrap: self.bootstrap.clone(),
            tls_host: self.tls_host.clone(),
            tls_port: self.tls_port,
        }
    }
}

impl DotResolver {
    /// Create a new DoT resolver from a hostname like `"1.1.1.1"` or `"cloudflare-dns.com"`.
    ///
    /// The host is resolved via the system resolver (GaiResolver) on first lookup.
    /// The default port is 853.
    pub fn new(host: &str) -> Self {
        Self::new_with_port(host, 853)
    }

    /// Create a new DoT resolver with a custom port.
    pub fn new_with_port(host: &str, port: u16) -> Self {
        let bootstrap: Arc<dyn Resolve> = Arc::new(GaiResolver::new());
        Self {
            state: Arc::new(Mutex::new(None)),
            bootstrap,
            tls_host: host.to_string(),
            tls_port: port,
        }
    }

    async fn get_resolver(&self) -> Result<Arc<TokioResolver>, BoxError> {
        if let Some(ref resolver) = *self.state.lock().unwrap() {
            return Ok(resolver.clone());
        }

        let addrs = self
            .bootstrap
            .resolve(Name::from_str(&self.tls_host)?)
            .await?;
        let ips: Vec<IpAddr> = addrs.map(|a| a.ip()).collect();

        let mut group = NameServerConfigGroup::with_capacity(ips.len());
        for &ip in &ips {
            group.push(NameServerConfig {
                socket_addr: SocketAddr::new(ip, self.tls_port),
                protocol: Protocol::Tls,
                tls_dns_name: Some(self.tls_host.clone()),
                http_endpoint: None,
                trust_negative_responses: true,
                bind_addr: None,
            });
        }
        let config = ResolverConfig::from_parts(None, vec![], group);

        let mut builder =
            TokioResolver::builder_with_config(config, TokioConnectionProvider::default());
        let opts = builder.options_mut();
        opts.timeout = Duration::from_secs(5);
        opts.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
        let resolver = Arc::new(builder.build());

        let mut guard = self.state.lock().unwrap();
        if guard.is_none() {
            *guard = Some(resolver.clone());
        }
        Ok(guard.as_ref().unwrap().clone())
    }
}

impl Resolve for DotResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let this = self.clone();
        Box::pin(async move {
            let resolver = this.get_resolver().await?;
            let lookup = resolver.lookup_ip(name.as_str()).await?;
            let addrs: Addrs = Box::new(SocketAddrs {
                iter: lookup.into_iter(),
            });
            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_default_port() {
        let resolver = DotResolver::new("1.1.1.1");
        assert_eq!(resolver.tls_host, "1.1.1.1");
        assert_eq!(resolver.tls_port, 853);
    }

    #[test]
    fn new_custom_port() {
        let resolver = DotResolver::new_with_port("dns.google", 5353);
        assert_eq!(resolver.tls_host, "dns.google");
        assert_eq!(resolver.tls_port, 5353);
    }

    #[test]
    fn debug_output() {
        let resolver = DotResolver::new_with_port("cloudflare-dns.com", 853);
        let debug = format!("{:?}", resolver);
        assert!(debug.contains("cloudflare-dns.com"), "{debug}");
        assert!(debug.contains("853"), "{debug}");
    }
}
