use std::cmp::min;
use std::net::SocketAddr;

use foldhash::{HashMap, HashMapExt};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::lru::LruCache;
use super::{Addrs, Name, Resolve, Resolving};

use crate::error::BoxError;

use std::error::Error as StdError;
use std::fmt;

use tokio::sync::watch;

/// Wraps an `Arc<BoxError>` so a shared in-flight result can be returned as a
/// `BoxError`. Its `source()` forwards to the inner error, so
/// `is_dns`/`is_connect`/`is_timeout` walks still observe the original error.
/// Yields cached `SocketAddr`s by copy from a cheaply-cloned `Arc<[SocketAddr]>`,
/// so serving a cache hit needs no `Vec` allocation.
struct ArcAddrs {
    arc: Arc<[SocketAddr]>,
    idx: usize,
}

impl Iterator for ArcAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.arc.get(self.idx).copied();
        if item.is_some() {
            self.idx += 1;
        }
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.arc.len().saturating_sub(self.idx);
        (remaining, Some(remaining))
    }
}

/// Tags a returned error with `DnsError` so `is_dns()` stays accurate for
/// direct/fixture `DnsCacheResolver` consumers and the negative-cache path.
/// Leaves already-tagged errors untouched to avoid nesting tags.
fn ensure_dns_tagged(e: BoxError) -> BoxError {
    if e.is::<crate::error::DnsError>() {
        return e;
    }
    crate::error::dns(e)
}

struct ArcErr(Arc<BoxError>);

impl fmt::Display for ArcErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.0, f)
    }
}

impl fmt::Debug for ArcErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl StdError for ArcErr {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&**self.0 as _)
    }
}

pub(crate) const DNS_CACHE_TTL: Duration = Duration::from_secs(30);
/// Failed resolutions (NXDOMAIN, SERVFAIL, timeout, unreachable) are cached for
/// 5s so fan-out to a dead host isn't re-resolved every request, while keeping
/// a recovered host unmasked.
const DNS_CACHE_NEGATIVE_TTL: Duration = Duration::from_secs(5);
pub(crate) const DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(30);
/// Per-client cap on cached hostnames (1024), leaving headroom for fan-out
/// workloads while bounding memory in long-lived clients.
const DNS_CACHE_MAX_SIZE: usize = 1024;

/// A cache entry is either a resolved address set or a previously observed
/// failure (negative caching); both record insertion time for TTL expiry.
enum CacheEntry {
    Resolved {
        addrs: Arc<[SocketAddr]>,
        inserted_at: Instant,
    },
    Failed {
        err: Arc<BoxError>,
        inserted_at: Instant,
    },
}

impl CacheEntry {
    fn resolved_addrs(&mut self) -> Option<&Arc<[SocketAddr]>> {
        match self {
            CacheEntry::Resolved { addrs, .. } => Some(addrs),
            _ => None,
        }
    }

    fn failed_err(&mut self) -> Option<&Arc<BoxError>> {
        match self {
            CacheEntry::Failed { err, .. } => Some(err),
            _ => None,
        }
    }
}

/// Returns a cache entry's age without promoting it, for TTL comparison.
fn entry_age(cache: &LruCache<String, CacheEntry>, key: &str) -> Duration {
    cache
        .peek(key)
        .map(|e| e.inserted_at().elapsed())
        .unwrap_or(Duration::MAX)
}

impl CacheEntry {
    fn inserted_at(&self) -> Instant {
        match self {
            CacheEntry::Resolved { inserted_at, .. } => *inserted_at,
            CacheEntry::Failed { inserted_at, .. } => *inserted_at,
        }
    }
}

/// The result of an in-flight resolution, shared with concurrent waiters.
type SharedResult = Result<Vec<SocketAddr>, Arc<BoxError>>;

/// Tracks resolutions currently in flight, keyed by (lowercased) host.
type InflightMap = Arc<Mutex<HashMap<String, Arc<watch::Sender<Option<Arc<SharedResult>>>>>>>;

pub(crate) struct DnsCacheResolver {
    inner: Arc<dyn Resolve>,
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    /// How long a resolved entry is served before re-resolution. The [`Resolve`]
    /// trait yields no per-record TTL, so a single client-wide TTL is applied;
    /// `Duration::ZERO` disables caching (each lookup re-resolves).
    ttl: Duration,
    /// Upper bound for a single resolution attempt (from construction or the
    /// per-request connect deadline via [`DnsCacheResolver::with_timeout`]).
    timeout: Duration,
    /// In-flight resolutions keyed by lowercased host; concurrent misses for the
    /// same host share one `watch` channel to avoid a resolution stampede.
    inflight: InflightMap,
}

impl DnsCacheResolver {
    #[cfg(test)]
    pub(crate) fn new(inner: Arc<dyn Resolve>) -> Self {
        Self::with_ttl(inner, DNS_CACHE_TTL)
    }

    /// Builds a resolver caching entries for `ttl`. `Duration::ZERO` disables
    /// caching but still deduplicates concurrent in-flight resolutions.
    #[cfg(test)]
    pub(crate) fn with_ttl(inner: Arc<dyn Resolve>, ttl: Duration) -> Self {
        Self::with_ttl_and_timeout(inner, ttl, DNS_RESOLUTION_TIMEOUT)
    }

    /// Like [`DnsCacheResolver::with_ttl`] but with an explicit per-resolution
    /// timeout — keeps a hanging lookup reported as DNS, not connect, error.
    pub(crate) fn with_ttl_and_timeout(
        inner: Arc<dyn Resolve>,
        ttl: Duration,
        timeout: Duration,
    ) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(DNS_CACHE_MAX_SIZE).expect("DNS_CACHE_MAX_SIZE must be non-zero"),
            ))),
            ttl,
            timeout,
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Resolve for DnsCacheResolver {
    fn resolve(&self, name: Name) -> Resolving {
        // DNS hostnames are case-insensitive, so normalize the cache key to
        // lowercase. The original `name` (preserving case) is still passed to
        // the inner resolver below.
        let host_key = name.as_str().to_ascii_lowercase();

        // Blocking lock is fine: the critical section is one O(1) op
        // and the returned iterator is owned, so no cache borrow survives.
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        // `peek` checks presence + staleness without promoting; we only
        // promote on a confirmed hit, and only `pop` (no promote-then-evict)
        // on a confirmed stale entry. A zero TTL disables cache hits entirely
        // (`elapsed()` is always >= 0), so entries are treated as stale.
        match cache.peek(&host_key) {
            Some(CacheEntry::Resolved { .. })
                if !self.ttl.is_zero() && entry_age(&cache, &host_key) < self.ttl =>
            {
                // Clone only the `Arc` (cheap atomic bump) — no per-request
                // `Vec` copy. Move it into the future so the yielded iterator
                // can borrow it for `'static` without re-allocating.
                if let Some(entry) = cache.get_mut(&host_key) {
                    if let Some(addrs) = entry.resolved_addrs() {
                        let addrs = addrs.clone();
                        return Box::pin(async move {
                            Ok(Box::new(ArcAddrs { arc: addrs, idx: 0 }) as Addrs)
                        });
                    }
                }
                // `peek` confirmed presence under the held lock, so the entry
                // should be `Resolved`. If it is not (e.g. a concurrent mutation
                // raced us), fall through and re-resolve instead of panicking
                // (`panic = "abort"`).
            }
            Some(CacheEntry::Failed { .. })
                if !self.ttl.is_zero()
                    && entry_age(&cache, &host_key) < min(DNS_CACHE_NEGATIVE_TTL, self.ttl) =>
            {
                // Negative cache hit: return the previously observed error
                // (preserving its classification tags) instead of re-resolving.
                // This absorbs fan-out load to a host that is failing.
                if let Some(entry) = cache.get_mut(&host_key) {
                    if let Some(err) = entry.failed_err() {
                        let err = err.clone();
                        return Box::pin(async move { Err(Box::new(ArcErr(err)) as BoxError) });
                    }
                }
                // Unreachable: `peek` confirmed a `Failed` entry (see above).
                // Fall through to re-resolve.
            }
            Some(_) => {
                cache.pop(&host_key);
            }
            None => {}
        }
        drop(cache);

        // Another concurrent miss for the same host may already be resolving
        // it. Subscribe to that in-flight channel and await its result
        // instead of starting a second resolution. The check and the insert
        // below are done under a single lock acquisition so two concurrent
        // misses cannot both slip past the check before either inserts its
        // sender (which would start redundant underlying resolutions).
        let tx: Arc<watch::Sender<Option<Arc<SharedResult>>>>;
        // Declared in the outer scope so the returned future can capture it and
        // its `Drop` runs on *every* exit path — including when the future is
        // dropped before it is first polled (request timeout / select!
        // cancellation). Constructed synchronously below, before the future is
        // even returned, so it cannot be orphaned.
        let guard: InflightGuard;
        {
            let mut inflight = self.inflight.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(existing) = inflight.get(&host_key) {
                let mut rx = existing.subscribe();
                let inner = Arc::clone(&self.inner);
                let name = name.clone();
                let timeout = self.timeout;
                return Box::pin(async move {
                    await_inflight(&mut rx, inner, name, timeout)
                        .await
                        .map(|a| Box::new(a.into_iter()) as Addrs)
                        .map_err(|e| Box::new(ArcErr(e)) as BoxError)
                });
            }

            // We are the first/only resolver for this host right now. Create
            // the channel and register it *before* dropping the lock so no
            // concurrent miss can race in behind us.
            let (tx_inner, _rx) = watch::channel::<Option<Arc<SharedResult>>>(None);
            tx = Arc::new(tx_inner);
            inflight.insert(host_key.clone(), tx.clone());
            // Construct the guard here, synchronously, before dropping the lock
            // and before the future is even returned. This guarantees removal of
            // the in-flight entry on *every* exit path — including when the
            // returned future is dropped before it is ever polled (e.g. request
            // timeouts, select! races, connection-pool cancellation). If the
            // guard were only built inside the async block, a drop-before-poll
            // would orphan the sender and hang all future waiters for this host.
            guard = InflightGuard {
                inflight: Arc::clone(&self.inflight),
                host_key: host_key.clone(),
            };
        }

        let inner = Arc::clone(&self.inner);
        let cache = Arc::clone(&self.cache);
        let host_key_clone = host_key.clone();
        let ttl = self.ttl;
        let timeout = self.timeout;

        Box::pin(async move {
            // Remove the in-flight entry on any exit path, including a
            // cancelled/aborted future. `_guard` is moved into the async block
            // so its Drop runs when the future completes, errors, or is dropped.
            let _guard = guard;

            let res: SharedResult = resolve_with_timeout(&inner, name, timeout).await;

            // Publish the resolved result to concurrent waiters (cheap `Arc`
            // clone of `res`; the original is still used below).
            let _ = tx.send(Some(Arc::new(res.clone())));

            // Build the leader's own return value from a borrow of `res`.
            let return_value: Result<Addrs, BoxError> = match &res {
                Ok(addrs) => {
                    // Skip caching when TTL is zero (caching disabled); still
                    // publish to concurrent waiters so in-flight dedup holds.
                    if !ttl.is_zero() {
                        let cached: Arc<[SocketAddr]> = Arc::from(addrs.as_slice());
                        let mut cache = cache.lock().unwrap_or_else(|p| p.into_inner());
                        cache.put(
                            host_key_clone.clone(),
                            CacheEntry::Resolved {
                                addrs: cached,
                                inserted_at: Instant::now(),
                            },
                        );
                    }
                    // Build an owned iterator. The leader path is taken once
                    // per cache miss; the far more common cache-hit path avoids
                    // any `Vec` allocation by cloning the cheap `Arc<[SocketAddr]>`.
                    Ok(Box::new(ArcAddrs {
                        arc: Arc::from(addrs.as_slice()),
                        idx: 0,
                    }) as Addrs)
                }
                Err(e) => {
                    // Cache the failure briefly (negative caching) so repeated
                    // requests to a dead host do not each trigger a full
                    // (time-boxed) upstream resolution. Skip when TTL is zero.
                    if !ttl.is_zero() {
                        let mut cache = cache.lock().unwrap_or_else(|p| p.into_inner());
                        cache.put(
                            host_key_clone.clone(),
                            CacheEntry::Failed {
                                err: Arc::clone(e),
                                inserted_at: Instant::now(),
                            },
                        );
                    }
                    // Preserve the original error (and its `is_dns()` tag,
                    // if any) instead of re-wrapping it into a plain
                    // `io::Error`, which would erase the classification.
                    Err(Box::new(ArcErr(Arc::clone(e))) as BoxError)
                }
            };

            return_value
        })
    }
}

/// On drop, removes the in-flight entry so a cancelled/panicking resolver
/// future can't orphan a `watch` channel that waiters would block on forever.
struct InflightGuard {
    inflight: InflightMap,
    host_key: String,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.inflight
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.host_key);
    }
}

/// Resolve `name` with the per-lookup deadline, tagging failures so
/// `is_dns()` stays accurate. Shared by the leader path and the waiter
/// fallback so both produce identical error shapes.
async fn resolve_with_timeout(
    inner: &Arc<dyn Resolve>,
    name: Name,
    timeout: Duration,
) -> SharedResult {
    match tokio::time::timeout(timeout, inner.resolve(name)).await {
        Ok(Ok(it)) => Ok(it.collect()),
        Ok(Err(e)) => Err(Arc::new(ensure_dns_tagged(e))),
        Err(_) => Err(Arc::new(crate::error::dns(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("DNS resolution timed out after {timeout:?}"),
        )))),
    }
}

/// Await an in-flight resolution published via a `watch` channel and return
/// its (cloned) result. When the sender is dropped without publishing — the
/// leader was cancelled (`InflightGuard` removed the entry, so no new waiter
/// will join) — the fallback resolves the name itself instead of failing a
/// healthy request with a bogus cancellation error.
async fn await_inflight(
    rx: &mut watch::Receiver<Option<Arc<SharedResult>>>,
    inner: Arc<dyn Resolve>,
    name: Name,
    timeout: Duration,
) -> SharedResult {
    loop {
        if let Some(v) = rx.borrow().as_ref() {
            return v.as_ref().clone();
        }
        if rx.changed().await.is_err() {
            return resolve_with_timeout(&inner, name, timeout).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foldhash::{HashMap, HashMapExt};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;

    use crate::dns::resolve::DnsResolverWithOverrides;

    /// Test resolver that counts `resolve` calls and returns canned addrs.
    struct CountingResolver {
        addrs: Arc<[SocketAddr]>,
        calls: StdArc<AtomicUsize>,
    }

    impl Resolve for CountingResolver {
        fn resolve(&self, _name: Name) -> Resolving {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let addrs = self.addrs.clone();
            Box::pin(async move { Ok(Box::new(ArcAddrs { arc: addrs, idx: 0 }) as Addrs) })
        }
    }

    fn parse_addrs(s: &str) -> Vec<SocketAddr> {
        vec![SocketAddr::from_str(s).unwrap()]
    }

    /// Build a `CacheEntry`-compatible `Arc<[SocketAddr]>`.
    fn cache_addrs(s: &str) -> Arc<[SocketAddr]> {
        Arc::from(parse_addrs(s))
    }

    #[test]
    fn two_resolvers_have_independent_caches() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let r1 = DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("1.2.3.4:80"),
            calls: StdArc::clone(&calls),
        }));
        let r2 = DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("9.9.9.9:80"),
            calls: StdArc::clone(&calls),
        }));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("independent-caches-host.invalid").unwrap();
            let a = r1.resolve(n.clone()).await.unwrap().collect::<Vec<_>>();
            let b = r2.resolve(n).await.unwrap().collect::<Vec<_>>();

            assert_eq!(a, parse_addrs("1.2.3.4:80"));
            assert_eq!(b, parse_addrs("9.9.9.9:80"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                2,
                "per-client cache must not share entries across resolvers with different inners"
            );
        });
    }

    #[test]
    fn second_resolve_through_same_resolver_is_a_cache_hit() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("1.2.3.4:80"),
            calls: StdArc::clone(&calls),
        }));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("same-resolver-cache-hit.invalid").unwrap();
            let a = resolver
                .resolve(n.clone())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            let b = resolver.resolve(n).await.unwrap().collect::<Vec<_>>();

            assert_eq!(a, parse_addrs("1.2.3.4:80"));
            assert_eq!(b, parse_addrs("1.2.3.4:80"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "second resolve must be a cache hit"
            );
        });
    }

    #[test]
    fn expired_entry_triggers_re_resolve() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("5.6.7.8:443"),
            calls: StdArc::clone(&calls),
        }));

        // Seed a stale entry; hostname is unique to this test.
        {
            let mut cache = resolver.cache.lock().unwrap_or_else(|p| p.into_inner());
            cache.put(
                "stale-host.invalid".to_string(),
                CacheEntry::Resolved {
                    addrs: cache_addrs("0.0.0.0:0"),
                    inserted_at: Instant::now() - DNS_CACHE_TTL - Duration::from_secs(1),
                },
            );
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let addrs = resolver
                .resolve(Name::from_str("stale-host.invalid").unwrap())
                .await
                .unwrap()
                .collect::<Vec<_>>();

            assert_eq!(addrs, parse_addrs("5.6.7.8:443"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "stale entry must be re-resolved"
            );
        });
    }

    #[test]
    fn cache_evicts_least_recently_used_entry_when_full() {
        let resolver = DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("10.0.0.1:80"),
            calls: StdArc::new(AtomicUsize::new(0)),
        }));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            // Fill to capacity. LRU order after this: lru-fill-0 (LRU) ... lru-fill-N-1 (MRU).
            for i in 0..DNS_CACHE_MAX_SIZE {
                let host = format!("lru-fill-{i}.invalid");
                resolver
                    .resolve(Name::from_str(&host).unwrap())
                    .await
                    .unwrap()
                    .for_each(|_| {});
            }
            {
                let cache = resolver.cache.lock().unwrap_or_else(|p| p.into_inner());
                assert_eq!(
                    cache.len(),
                    DNS_CACHE_MAX_SIZE,
                    "cache should be at capacity"
                );
            }

            // Touch lru-fill-0 (LRU) → MRU; lru-fill-1 becomes LRU.
            resolver
                .resolve(Name::from_str("lru-fill-0.invalid").unwrap())
                .await
                .unwrap()
                .for_each(|_| {});

            // One more insert evicts lru-fill-1 (LRU), not lru-fill-0 (touched).
            resolver
                .resolve(Name::from_str("lru-overflow.invalid").unwrap())
                .await
                .unwrap()
                .for_each(|_| {});

            let cache = resolver.cache.lock().unwrap_or_else(|p| p.into_inner());
            assert_eq!(cache.len(), DNS_CACHE_MAX_SIZE, "cache must stay bounded");
            assert!(
                cache.peek("lru-fill-0.invalid").is_some(),
                "touched entry survives"
            );
            assert!(
                cache.peek("lru-fill-1.invalid").is_none(),
                "LRU entry is evicted"
            );
            assert!(
                cache.peek("lru-overflow.invalid").is_some(),
                "new entry is present"
            );
        });
    }

    /// Regression test: concurrent cache misses for the same host must
    /// dedupe to exactly ONE underlying resolve. The in-flight table must be
    /// checked and inserted under a single lock so two concurrent misses
    /// cannot both slip past the check before either registers its sender
    /// (which would start redundant underlying resolutions).
    #[test]
    fn concurrent_miss_dedupes_to_single_resolve() {
        use std::time::Duration as StdDuration;
        struct SlowResolver {
            addrs: Arc<[SocketAddr]>,
            calls: StdArc<AtomicUsize>,
        }
        impl Resolve for SlowResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let addrs = self.addrs.clone();
                let calls = StdArc::clone(&self.calls);
                Box::pin(async move {
                    tokio::time::sleep(StdDuration::from_millis(20)).await;
                    let _ = calls;
                    Ok(Box::new(ArcAddrs { arc: addrs, idx: 0 }) as Addrs)
                })
            }
        }

        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = StdArc::new(DnsCacheResolver::new(Arc::new(SlowResolver {
            addrs: cache_addrs("1.1.1.1:80"),
            calls: StdArc::clone(&calls),
        })));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("concurrent-host.invalid").unwrap();
            let mut tasks = Vec::new();
            for _ in 0..16 {
                let r = StdArc::clone(&resolver);
                let nn = n.clone();
                tasks.push(tokio::spawn(async move {
                    let _ = r.resolve(nn).await;
                }));
            }
            for t in tasks {
                let _ = t.await;
            }
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "concurrent misses for same host must dedupe to a single underlying resolve"
            );
        });
    }

    /// Regression test for the drop-before-first-poll deadlock: if the leader
    /// future for a host is dropped before it is ever polled (e.g. request
    /// timeout / select! cancellation), the in-flight entry must still be
    /// cleared so subsequent resolutions for the same host can proceed instead
    /// of blocking forever on an orphaned `watch` channel.
    #[test]
    fn drop_before_poll_does_not_hang_waiters() {
        struct NeverResolver {
            addrs: Arc<[SocketAddr]>,
            calls: StdArc<AtomicUsize>,
        }
        impl Resolve for NeverResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let addrs = self.addrs.clone();
                Box::pin(async move {
                    // Never completes, so the leader future stays pending; callers
                    // abort it before it is polled to exercise the drop window.
                    let _: Result<Addrs, BoxError> =
                        std::future::pending::<Result<Addrs, BoxError>>().await;
                    Ok(Box::new(ArcAddrs { arc: addrs, idx: 0 }) as Addrs)
                })
            }
        }

        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = StdArc::new(DnsCacheResolver::new(Arc::new(NeverResolver {
            addrs: cache_addrs("1.1.1.1:80"),
            calls: StdArc::clone(&calls),
        })));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("abort-host.invalid").unwrap();

            // Spawn the leader and abort it immediately, before it is polled.
            let leader = {
                let r = StdArc::clone(&resolver);
                let nn = n.clone();
                tokio::spawn(async move {
                    let _ = r.resolve(nn).await;
                })
            };
            leader.abort();

            // Give the runtime a chance to run/drop the leader future.
            tokio::task::yield_now().await;

            // A subsequent resolution for the same host must NOT hang. Wait with a
            // timeout: if the in-flight entry was orphaned, this would block
            // forever (the inner `NeverResolver` never publishes a result, so a
            // surviving entry would make the waiter await a channel that never
            // changes).
            let wait =
                tokio::time::timeout(std::time::Duration::from_secs(35), resolver.resolve(n)).await;
            assert!(
                wait.is_ok(),
                "resolution must not hang after a leader future was dropped before poll"
            );
        });
    }

    /// A leader cancelled mid-resolution must not fail already-subscribed
    /// waiters: the waiter falls back to resolving the name itself instead of
    /// surfacing a spurious "DNS resolution cancelled" error from the dead
    /// watch channel (the leader's `InflightGuard` removed the entry and the
    /// sender dropped without publishing).
    #[test]
    fn cancelled_leader_falls_back_to_self_resolution() {
        struct GatedResolver {
            release: StdArc<tokio::sync::Notify>,
            called: StdArc<AtomicUsize>,
            addrs: Arc<[SocketAddr]>,
        }
        impl Resolve for GatedResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                self.called.fetch_add(1, Ordering::SeqCst);
                let addrs = self.addrs.clone();
                let release = StdArc::clone(&self.release);
                Box::pin(async move {
                    release.notified().await;
                    Ok(Box::new(ArcAddrs { arc: addrs, idx: 0 }) as Addrs)
                })
            }
        }

        let release = StdArc::new(tokio::sync::Notify::new());
        let called = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::new(Arc::new(GatedResolver {
            release: StdArc::clone(&release),
            called: StdArc::clone(&called),
            addrs: cache_addrs("1.1.1.1:80"),
        }));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("waiter-host.invalid").unwrap();

            // Leader future dropped before first poll: the in-flight entry is
            // removed and the channel closes without a published value.
            let leader_fut = resolver.resolve(n.clone());
            let waiter_fut = resolver.resolve(n);
            drop(leader_fut);
            // The waiter's fallback resolution is the second `resolve` call.
            release.notify_one();

            let res = tokio::time::timeout(std::time::Duration::from_secs(5), waiter_fut).await;
            let res = res
                .expect("waiter must resolve by itself, not hang, after the leader was cancelled");
            assert!(
                res.is_ok(),
                "waiter must not fail with a spurious error after the leader was cancelled"
            );
            assert!(
                called.load(Ordering::SeqCst) >= 1,
                "the waiter's fallback must perform its own underlying resolution \
                 (pre-fix it returned Err without ever resolving)"
            );
        });
    }

    #[test]
    fn distinct_hosts_do_not_collide() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("1.1.1.1:80"),
            calls: StdArc::clone(&calls),
        }));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let a = resolver
                .resolve(Name::from_str("distinct-host-a.invalid").unwrap())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            let b = resolver
                .resolve(Name::from_str("distinct-host-b.invalid").unwrap())
                .await
                .unwrap()
                .collect::<Vec<_>>();

            assert_eq!(a, parse_addrs("1.1.1.1:80"));
            assert_eq!(b, parse_addrs("1.1.1.1:80"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                2,
                "distinct hosts must each trigger one underlying resolve"
            );
        });
    }

    /// Regression guard for the cache-below-overrides architecture:
    /// when an `DnsResolverWithOverrides` wraps the cache, the override
    /// must short-circuit *before* the cache is consulted, and the
    /// override value must not enter the cache (only base resolutions
    /// are cached).
    #[test]
    fn override_short_circuits_before_caching() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let cached = Arc::new(DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("1.2.3.4:80"),
            calls: StdArc::clone(&calls),
        })));
        let mut overrides = HashMap::new();
        overrides.insert(
            "override-host.invalid".to_string(),
            parse_addrs("5.6.7.8:443"),
        );
        let resolver = DnsResolverWithOverrides::new(cached.clone(), overrides);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            // Override path: returns the override, never calls inner,
            // and does not store the override value in the cache.
            let addrs = resolver
                .resolve(Name::from_str("override-host.invalid").unwrap())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            assert_eq!(addrs, parse_addrs("5.6.7.8:443"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                0,
                "override must short-circuit before the cache/inner"
            );
            {
                let cache = cached.cache.lock().unwrap_or_else(|p| p.into_inner());
                assert!(
                    cache.peek("override-host.invalid").is_none(),
                    "override value must never enter the cache"
                );
            }

            // Non-override path through the same resolver: falls through
            // to the cache → inner, and the result is cached for the
            // next miss.
            let addrs = resolver
                .resolve(Name::from_str("non-override-host.invalid").unwrap())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            assert_eq!(addrs, parse_addrs("1.2.3.4:80"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "non-override hits the inner"
            );
            {
                let cache = cached.cache.lock().unwrap_or_else(|p| p.into_inner());
                assert!(
                    cache.peek("non-override-host.invalid").is_some(),
                    "base resolution is cached"
                );
            }
        });
    }

    /// Regression test: DNS override keys are matched case-insensitively, so
    /// an override registered as `Example.com` is applied to a query for
    /// `example.com` (hostnames are case-insensitive per RFC 4343).
    #[test]
    fn override_match_is_case_insensitive() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let cached = Arc::new(DnsCacheResolver::new(Arc::new(CountingResolver {
            addrs: cache_addrs("1.2.3.4:80"),
            calls: StdArc::clone(&calls),
        })));
        let mut overrides = HashMap::new();
        overrides.insert("Example.com".to_string(), parse_addrs("5.6.7.8:443"));
        let resolver = DnsResolverWithOverrides::new(cached.clone(), overrides);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let addrs = resolver
                .resolve(Name::from_str("example.com").unwrap())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            assert_eq!(addrs, parse_addrs("5.6.7.8:443"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                0,
                "case-insensitive override must short-circuit before the cache/inner"
            );
        });
    }

    #[test]
    fn zero_ttl_disables_caching_but_keeps_inflight_dedup() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::with_ttl(
            Arc::new(CountingResolver {
                addrs: cache_addrs("1.2.3.4:80"),
                calls: StdArc::clone(&calls),
            }),
            Duration::ZERO,
        );

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("zero-ttl-host.invalid").unwrap();
            let _ = resolver
                .resolve(n.clone())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            let _ = resolver.resolve(n).await.unwrap().collect::<Vec<_>>();
            // With caching disabled, each sequential resolve hits the inner.
            assert_eq!(
                calls.load(Ordering::SeqCst),
                2,
                "zero TTL must re-resolve every request (no cache hit)"
            );
        });
    }

    #[test]
    fn custom_ttl_expiry_triggers_reresolution() {
        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::with_ttl(
            Arc::new(CountingResolver {
                addrs: cache_addrs("1.2.3.4:80"),
                calls: StdArc::clone(&calls),
            }),
            Duration::from_millis(30),
        );

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("custom-ttl-host.invalid").unwrap();
            let _ = resolver
                .resolve(n.clone())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            // Immediate second resolve is a cache hit.
            let _ = resolver
                .resolve(n.clone())
                .await
                .unwrap()
                .collect::<Vec<_>>();
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "second resolve must be cached"
            );

            // After the TTL elapses, the entry is stale and re-resolved.
            tokio::time::sleep(Duration::from_millis(60)).await;
            let _ = resolver.resolve(n).await.unwrap().collect::<Vec<_>>();
            assert_eq!(
                calls.load(Ordering::SeqCst),
                2,
                "expired entry must trigger re-resolution"
            );
        });
    }

    /// Regression test for DNS error tag erosion through the cache (issue 1):
    /// a resolution failure returned by the inner resolver must preserve its
    /// `is_dns()` classification after it has been cached / de-duplicated,
    /// rather than being re-wrapped into a plain `io::Error` that loses the
    /// tag. This exercises both the leader path and the cache-hit path.
    #[test]
    fn dns_error_tag_preserved_through_cache_leader() {
        struct FailingResolver;
        impl Resolve for FailingResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                let e: BoxError = crate::error::dns("simulated DNS failure");
                Box::pin(std::future::ready(Err(e)))
            }
        }

        let resolver = DnsCacheResolver::new(Arc::new(FailingResolver));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let resolved = resolver
                .resolve(Name::from_str("always-fails.invalid").unwrap())
                .await;
            let err = crate::error::request(resolved.err().expect("expected a DNS error"));
            assert!(err.is_dns(), "DNS error tag must survive the cache path");

            let resolved2 = resolver
                .resolve(Name::from_str("always-fails.invalid").unwrap())
                .await;
            let err2 = crate::error::request(resolved2.err().expect("expected a cached DNS error"));
            assert!(
                err2.is_dns(),
                "cached DNS error tag must survive the cache path"
            );
        });
    }

    /// Regression test for negative caching: a failed resolution must be
    /// served from the cache (without re-querying the inner resolver) for the
    /// duration of the negative TTL. Previously failures were never cached, so
    /// every request to a dead host re-triggered a full (time-boxed) upstream
    /// resolution — a load multiplier under fan-out.
    #[test]
    fn failed_resolution_is_negative_cached() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct FailingResolver {
            calls: StdArc<AtomicUsize>,
        }
        impl Resolve for FailingResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let e: BoxError = crate::error::dns("simulated DNS failure");
                Box::pin(std::future::ready(Err(e)))
            }
        }

        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::new(Arc::new(FailingResolver {
            calls: StdArc::clone(&calls),
        }));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("neg-cache-host.invalid").unwrap();
            let _ = resolver.resolve(n.clone()).await;
            // A second resolve within the negative TTL must be a cache hit,
            // not a re-resolution.
            let _ = resolver.resolve(n).await;
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "failed resolution must be negative-cached, not re-resolved"
            );
        });
    }

    /// The negative TTL must be clamped to the user's (possibly shorter)
    /// positive TTL: with `dns_cache_ttl` < 5s, a recovered host must not stay
    /// masked as failed for the full fixed 5s — failures must not outlive
    /// successes, defeating the explicit short-TTL intent.
    #[test]
    fn negative_ttl_is_clamped_to_custom_short_ttl() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct FailingResolver {
            calls: StdArc<AtomicUsize>,
        }
        impl Resolve for FailingResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let e: BoxError = crate::error::dns("simulated DNS failure");
                Box::pin(std::future::ready(Err(e)))
            }
        }

        let calls = StdArc::new(AtomicUsize::new(0));
        let resolver = DnsCacheResolver::with_ttl(
            Arc::new(FailingResolver {
                calls: StdArc::clone(&calls),
            }),
            Duration::from_millis(100),
        );

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("neg-clamp-host.invalid").unwrap();
            let _ = resolver.resolve(n.clone()).await;
            let _ = resolver.resolve(n.clone()).await;
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "first re-resolve within the clamped TTL must be a cache hit"
            );
            // Past the clamped negative TTL (= the 100ms positive TTL, not the
            // fixed 5s), the entry must be stale and re-resolved.
            tokio::time::sleep(Duration::from_millis(150)).await;
            let _ = resolver.resolve(n.clone()).await;
            assert_eq!(
                calls.load(Ordering::SeqCst),
                2,
                "a failed host must be re-probed after the clamped negative TTL"
            );
        });
    }

    /// Regression test for DNS error tag erosion through the in-flight
    /// de-duplication path: a concurrent waiter that subscribes to an
    /// already-running resolution must receive the original DNS error (with
    /// its `is_dns()` tag) rather than a stringified `io::Error`.
    #[test]
    fn timeout_is_classified_as_dns() {
        // A resolver that never resolves forces the cache's
        // `tokio::time::timeout` branch. The produced error must still read as
        // `is_dns()` so downstream logic (retry/observability) branches on DNS
        // failures consistently with inner-resolver failures.
        struct HangingResolver;
        impl Resolve for HangingResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                Box::pin(std::future::pending::<Result<Addrs, BoxError>>())
            }
        }

        let resolver = DnsCacheResolver::new(Arc::new(HangingResolver));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("hangs.invalid").unwrap();
            let res = resolver.resolve(n).await;
            let err = crate::error::request(res.err().expect("timeout error"));
            assert!(err.is_dns(), "DNS timeout must be classified as is_dns()");
            // A DNS timeout is a DNS error and a timeout, but it is NOT a
            // connection error: the `io::Error(TimedOut)` it wraps must not
            // leak into `is_connect()` (regression for the connect/dns overlap).
            assert!(
                !err.is_connect(),
                "DNS timeout must not be classified as is_connect()"
            );
        });
    }

    /// Regression test for DNS error tag erosion when the *inner* resolver
    /// returns an error that is NOT already `DnsError`-tagged (this is the
    /// case for `GaiResolver`, `HickoryDnsResolver`, `DohResolver`, `DotResolver`
    /// and `PlainDnsResolver`, which all surface plain `io::Error`s). The cache
    /// deliberately preserves the inner tag rather than re-wrapping, so an
    /// untagged inner error must be re-tagged by the cache to keep `is_dns()`
    /// accurate for direct/fixture consumers (the client path relies on
    /// `DynResolver::call` to do this, but the cache must not silently drop it).
    #[test]
    fn untagged_inner_error_is_classified_as_dns() {
        // A resolver that fails with a plain `io::Error` (no `DnsError` tag),
        // mirroring the production non-hickory resolvers.
        struct PlainFailingResolver;
        impl Resolve for PlainFailingResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                let e: BoxError = std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "simulated name resolution failure",
                )
                .into();
                Box::pin(std::future::ready(Err(e)))
            }
        }

        let resolver = DnsCacheResolver::new(Arc::new(PlainFailingResolver));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let n = Name::from_str("plain-fails.invalid").unwrap();
            let res = resolver.resolve(n).await;
            let err = crate::error::request(res.err().expect("DNS error"));
            assert!(
                err.is_dns(),
                "untagged inner resolver error must be re-classified as is_dns() by the cache"
            );
        });
    }

    #[test]
    fn dns_error_tag_preserved_through_inflight_waiter() {
        struct SlowFailingResolver;
        impl Resolve for SlowFailingResolver {
            fn resolve(&self, _name: Name) -> Resolving {
                let e: BoxError = crate::error::dns("slow DNS failure");
                Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                    Err(e)
                })
            }
        }

        let resolver = StdArc::new(DnsCacheResolver::new(Arc::new(SlowFailingResolver)));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let leader_n = Name::from_str("slow-fails.invalid").unwrap();
            let waiter_n = leader_n.clone();

            let leader = {
                let r = StdArc::clone(&resolver);
                let nn = leader_n;
                tokio::spawn(async move { r.resolve(nn).await })
            };
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            let waiter = {
                let r = StdArc::clone(&resolver);
                let nn = waiter_n;
                tokio::spawn(async move { r.resolve(nn).await })
            };

            let leader_res = leader.await.unwrap();
            let waiter_res = waiter.await.unwrap();

            let leader_err = crate::error::request(leader_res.err().expect("leader DNS error"));
            let waiter_err = crate::error::request(waiter_res.err().expect("waiter DNS error"));

            assert!(
                leader_err.is_dns(),
                "leader DNS error tag must survive the cache path"
            );
            assert!(
                waiter_err.is_dns(),
                "in-flight waiter DNS error tag must survive the cache path"
            );
        });
    }
}
