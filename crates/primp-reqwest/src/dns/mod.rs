//! DNS resolution

pub use resolve::{Addrs, Name, Resolve, Resolving};
pub(crate) use resolve::{DnsResolverWithOverrides, DynResolver};

#[cfg(docsrs)]
pub use resolve::IntoResolve;

mod cache;
pub use cache::{DnsCache, SharedDnsCache};

pub(crate) mod gai;
#[cfg(feature = "hickory-dns")]
pub(crate) mod hickory;
pub(crate) mod resolve;
