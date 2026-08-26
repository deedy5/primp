//! DNS resolution

pub use resolve::{Addrs, IntoResolve, Name, Resolve, Resolving};
pub(crate) use resolve::{DnsResolverWithOverrides, DynResolver};

pub(crate) mod cache;
pub(crate) mod hosts;
pub(crate) mod lru;

/// System DNS resolver backed by the OS's getaddrinfo.
pub mod gai;
#[cfg(feature = "hickory-dns")]
pub(crate) mod hickory;
/// DNS resolution traits.
pub mod resolve;
#[cfg(feature = "hickory-dns")]
pub(crate) use hickory::SocketAddrs;

#[cfg(feature = "hickory-dns")]
pub mod doh;
#[cfg(feature = "hickory-dns")]
pub mod dot;
#[cfg(feature = "hickory-dns")]
pub mod plain;
