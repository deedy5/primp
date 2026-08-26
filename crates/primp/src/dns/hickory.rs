//! DNS resolution via the [hickory-resolver](https://github.com/hickory-dns/hickory-dns) crate

use hickory_resolver::{
    config::{LookupIpStrategy, ResolveHosts, ResolverConfig, GOOGLE},
    net::{runtime::TokioRuntimeProvider, NetError},
    TokioResolver,
};
use once_cell::sync::OnceCell;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use super::{Addrs, Name, Resolve, Resolving};

/// DNS resolver backed by hickory-resolver.
#[derive(Debug, Default, Clone)]
pub(crate) struct HickoryDnsResolver {
    /// Resolver construction is deferred via `OnceCell` because we may be
    /// initialized outside a Tokio runtime.
    state: Arc<OnceCell<TokioResolver>>,
}

pub(crate) struct SocketAddrs {
    pub(crate) iter: std::vec::IntoIter<IpAddr>,
}

impl Resolve for HickoryDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolver = self.clone();
        Box::pin(async move {
            let resolver = resolver.state.get_or_try_init(new_resolver)?;

            let lookup = resolver.lookup_ip(name.as_str()).await?;
            let ips: Vec<IpAddr> = lookup.iter().collect();
            let addrs: Addrs = Box::new(SocketAddrs {
                iter: ips.into_iter(),
            });
            Ok(addrs)
        })
    }
}

impl Iterator for SocketAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ip| SocketAddr::new(ip, 0))
    }
}

/// Builds a resolver from `/etc/resolv.conf` (falling back to Google DNS),
/// with `Ipv4AndIpv6` lookup for happy eyeballs.
fn new_resolver() -> Result<TokioResolver, NetError> {
    let mut builder = TokioResolver::builder_tokio().unwrap_or_else(|err| {
        log::debug!(
            "hickory-dns: failed to load system DNS configuration; falling back to Google DNS: {:?}",
            err
        );
        TokioResolver::builder_with_config(
            ResolverConfig::udp_and_tcp(&GOOGLE),
            TokioRuntimeProvider::default(),
        )
    });
    // Prefer IPv4 and fall back to IPv6. We deliberately avoid
    // `Ipv6AndIpv4` because some networks silently black-hole IPv6, which
    // makes happy eyeballs wait out its connection-attempt delay (~250ms)
    // before falling back to IPv4 on the first request.
    builder.options_mut().ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
    // Hosts file can be 400k+ entries; disable to avoid 400 MB rehash.
    builder.options_mut().use_hosts_file = ResolveHosts::Never;
    builder.build()
}
