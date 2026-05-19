//! Plain DNS (UDP/TCP) resolution via hickory-resolver

use hickory_resolver::{
    config::{LookupIpStrategy, NameServerConfig, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
    TokioResolver,
};

use std::net::IpAddr;
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

        let name_servers: Vec<NameServerConfig> = ips
            .iter()
            .map(|&ip| NameServerConfig::udp_and_tcp(ip))
            .collect();
        let config = ResolverConfig::from_parts(None, vec![], name_servers);

        let mut builder =
            TokioResolver::builder_with_config(config, TokioRuntimeProvider::default());
        let opts = builder.options_mut();
        opts.timeout = Duration::from_secs(5);
        opts.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
        let resolver = Arc::new(builder.build().expect("failed to build plain DNS resolver"));

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
