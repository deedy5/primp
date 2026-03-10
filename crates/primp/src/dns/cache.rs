//! DNS caching layer to avoid repeated DNS lookups for the same hostnames.
//!
//! This module provides a DNS cache that stores resolved IP addresses with TTL
//! (time-to-live) to avoid repeated lookups for the same hostname.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::{Name, Resolve, Resolving};

/// A cached DNS entry with expiration time.
struct CachedEntry {
    /// The resolved socket addresses.
    addrs: Vec<SocketAddr>,
    /// When this entry expires.
    expires_at: Instant,
}

impl CachedEntry {
    /// Creates a new cached entry with the given TTL.
    fn new(addrs: Vec<SocketAddr>, ttl: Duration) -> Self {
        Self {
            addrs,
            expires_at: Instant::now() + ttl,
        }
    }

    /// Returns true if this entry has expired.
    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Inner cache data shared between clones.
#[derive(Default)]
struct CacheInner {
    /// The cached entries, keyed by hostname.
    entries: RwLock<HashMap<String, CachedEntry>>,
}

impl CacheInner {
    fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Gets cached addresses for a hostname, if available and not expired.
    fn get(&self, name: &str) -> Option<Vec<SocketAddr>> {
        let entries = self.entries.read().ok()?;
        let entry = entries.get(name)?;
        if entry.is_expired() {
            return None;
        }
        Some(entry.addrs.clone())
    }

    /// Inserts addresses into the cache with the given TTL.
    fn insert(&self, name: String, addrs: Vec<SocketAddr>, ttl: Duration) {
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(name, CachedEntry::new(addrs, ttl));
        }
    }

    /// Clears all cached entries.
    fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    /// Removes expired entries from the cache.
    fn purge_expired(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.retain(|_, entry| !entry.is_expired());
        }
    }

    /// Returns the number of cached entries (including expired ones).
    fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }
}

/// A DNS cache that wraps an inner resolver and caches results.
///
/// This cache is thread-safe and can be shared across multiple connections.
/// Entries are stored with a TTL and will be refreshed when they expire.
pub struct DnsCache {
    /// The inner resolver to use for cache misses.
    inner: Arc<dyn Resolve>,
    /// The shared cache data.
    cache: Arc<CacheInner>,
    /// The default TTL for cached entries.
    default_ttl: Duration,
}

impl DnsCache {
    /// Creates a new DNS cache with the given inner resolver and default TTL.
    pub fn new(inner: Arc<dyn Resolve>, default_ttl: Duration) -> Self {
        Self {
            inner,
            cache: Arc::new(CacheInner::new()),
            default_ttl,
        }
    }

    /// Creates a new DNS cache with a default TTL of 60 seconds.
    pub fn with_default_ttl(inner: Arc<dyn Resolve>) -> Self {
        Self::new(inner, Duration::from_secs(60))
    }

    /// Gets cached addresses for a hostname, if available and not expired.
    fn get_cached(&self, name: &str) -> Option<Vec<SocketAddr>> {
        self.cache.get(name)
    }

    /// Clears all cached entries.
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Removes expired entries from the cache.
    pub fn purge_expired(&self) {
        self.cache.purge_expired();
    }

    /// Returns the number of cached entries (including expired ones).
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Clone for DnsCache {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            cache: self.cache.clone(),
            default_ttl: self.default_ttl,
        }
    }
}

impl Resolve for DnsCache {
    fn resolve(&self, name: Name) -> Resolving {
        let name_str = name.as_str().to_string();

        // Check cache first
        if let Some(addrs) = self.get_cached(&name_str) {
            log::trace!("DNS cache hit for {}", name_str);
            return Box::pin(std::future::ready(Ok(Box::new(addrs.into_iter())
                as Box<dyn Iterator<Item = SocketAddr> + Send>)));
        }

        log::trace!("DNS cache miss for {}", name_str);

        // Cache miss - resolve using inner resolver
        let inner = self.inner.clone();
        let cache = self.cache.clone();
        let default_ttl = self.default_ttl;

        Box::pin(async move {
            let addrs: Vec<SocketAddr> = inner.resolve(name).await?.collect();

            // Insert into cache
            cache.insert(name_str, addrs.clone(), default_ttl);

            Ok(Box::new(addrs.into_iter()) as Box<dyn Iterator<Item = SocketAddr> + Send>)
        })
    }
}

impl std::fmt::Debug for DnsCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DnsCache")
            .field("default_ttl", &self.default_ttl)
            .field("cache_size", &self.len())
            .finish()
    }
}

/// A thread-safe reference to a DNS cache that can be shared.
/// This is an alias for backwards compatibility.
pub type SharedDnsCache = DnsCache;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    struct MockResolver {
        addrs: Vec<SocketAddr>,
        call_count: Arc<AtomicUsize>,
    }

    impl MockResolver {
        fn new(addrs: Vec<SocketAddr>) -> Self {
            Self {
                addrs,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl Resolve for MockResolver {
        fn resolve(&self, _name: Name) -> Resolving {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let addrs = self.addrs.clone();
            Box::pin(std::future::ready(Ok(Box::new(addrs.into_iter())
                as Box<dyn Iterator<Item = SocketAddr> + Send>)))
        }
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let mock = MockResolver::new(vec![test_addr(8080)]);
        let call_count = mock.call_count.clone();
        let cache = DnsCache::new(Arc::new(mock), Duration::from_secs(60));

        let name = Name::from_str("example.com").unwrap();

        // First call should hit the resolver
        let result: Vec<_> = cache.resolve(name.clone()).await.unwrap().collect();
        assert_eq!(result.len(), 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second call should hit the cache
        let result: Vec<_> = cache.resolve(name.clone()).await.unwrap().collect();
        assert_eq!(result.len(), 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Still 1
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let mock = MockResolver::new(vec![test_addr(8080)]);
        let call_count = mock.call_count.clone();
        let cache = DnsCache::new(Arc::new(mock), Duration::from_millis(10));

        let name = Name::from_str("example.com").unwrap();

        // First call
        let _ = cache.resolve(name.clone()).await.unwrap();
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should call resolver again after expiry
        let _ = cache.resolve(name.clone()).await.unwrap();
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_purge_expired() {
        let mock = MockResolver::new(vec![test_addr(8080)]);
        let cache = DnsCache::new(Arc::new(mock), Duration::from_millis(10));

        // Insert an entry
        cache.cache.insert("test.com".to_string(), vec![test_addr(8080)], Duration::from_millis(10));
        assert_eq!(cache.len(), 1);

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(20));

        // Purge should remove the expired entry
        cache.purge_expired();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_clone_shares_cache() {
        let mock = MockResolver::new(vec![test_addr(8080)]);
        let cache1 = DnsCache::new(Arc::new(mock), Duration::from_secs(60));
        let cache2 = cache1.clone();

        // Insert via cache1
        cache1.cache.insert("test.com".to_string(), vec![test_addr(8080)], Duration::from_secs(60));

        // Should be visible in cache2
        assert!(cache2.get_cached("test.com").is_some());
    }
}
