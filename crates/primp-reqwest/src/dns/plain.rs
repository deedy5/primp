//! Plain DNS (UDP/TCP) resolution via hickory-resolver

use hickory_resolver::{
    config::{LookupIpStrategy, NameServerConfig, NameServerConfigGroup, ResolverConfig},
    name_server::TokioConnectionProvider,
    proto::xfer::Protocol,
    TokioResolver,
};

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Addrs, Name, Resolve, Resolving, SocketAddrs};
use super::gai::GaiResolver;
use crate::error::BoxError;

/// A plain DNS (UDP/TCP) resolver backed by hickory-resolver.
///
/// Queries the specified DNS server directly using standard UDP/TCP on port 53.
pub struct PlainDnsResolver {
    state: Arc<Mutex<Option<Arc<TokioResolver>>>>,
    bootstrap: Arc<dyn Resolve>,
    dns_host: String,
    dns_port: u16,
}

impl std::fmt::Debug for PlainDnsResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlainDnsResolver")
            .field("dns_host", &self.dns_host)
            .field("dns_port", &self.dns_port)
            .finish()
    }
}

impl Clone for PlainDnsResolver {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            bootstrap: self.bootstrap.clone(),
            dns_host: self.dns_host.clone(),
            dns_port: self.dns_port,
        }
    }
}

impl PlainDnsResolver {
    /// Create a new plain DNS resolver for a given host.
    ///
    /// The host can be an IP address (`"1.1.1.1"`) or a hostname (`"resolver.example.com"`).
    /// The default port is 53.
    pub fn new(host: &str) -> Self {
        Self::new_with_port(host, 53)
    }

    /// Create a new plain DNS resolver with a custom port.
    pub fn new_with_port(host: &str, port: u16) -> Self {
        let bootstrap: Arc<dyn Resolve> = Arc::new(GaiResolver::new());
        Self {
            state: Arc::new(Mutex::new(None)),
            bootstrap,
            dns_host: host.to_string(),
            dns_port: port,
        }
    }

    async fn get_resolver(&self) -> Result<Arc<TokioResolver>, BoxError> {
        if let Some(ref resolver) = *self.state.lock().unwrap() {
            return Ok(resolver.clone());
        }

        let addrs = self
            .bootstrap
            .resolve(Name::from_str(&self.dns_host)?)
            .await?;
        let ips: Vec<_> = addrs.map(|a| a.ip()).collect();

        let mut group = NameServerConfigGroup::with_capacity(ips.len() * 2);
        for &ip in &ips {
            group.push(NameServerConfig {
                socket_addr: SocketAddr::new(ip, self.dns_port),
                protocol: Protocol::Udp,
                tls_dns_name: None,
                http_endpoint: None,
                trust_negative_responses: true,
                bind_addr: None,
            });
            group.push(NameServerConfig {
                socket_addr: SocketAddr::new(ip, self.dns_port),
                protocol: Protocol::Tcp,
                tls_dns_name: None,
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

impl Resolve for PlainDnsResolver {
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
        let resolver = PlainDnsResolver::new("1.1.1.1");
        assert_eq!(resolver.dns_host, "1.1.1.1");
        assert_eq!(resolver.dns_port, 53);
    }

    #[test]
    fn new_custom_port() {
        let resolver = PlainDnsResolver::new_with_port("8.8.8.8", 5353);
        assert_eq!(resolver.dns_host, "8.8.8.8");
        assert_eq!(resolver.dns_port, 5353);
    }

    #[test]
    fn debug_output() {
        let resolver = PlainDnsResolver::new_with_port("1.1.1.1", 53);
        let debug = format!("{:?}", resolver);
        assert!(debug.contains("1.1.1.1"), "{debug}");
        assert!(debug.contains("53"), "{debug}");
    }
}
