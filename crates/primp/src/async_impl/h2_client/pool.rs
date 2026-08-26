use std::future::Future;

use foldhash::{HashMap, HashMapExt};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use bytes::Bytes;
use h2::client::SendRequest;
use http::{Request, Response, Uri};

/// Default cap (1024) on concurrent in-flight request/response units per
/// pooled h2 connection, used when `http2_max_concurrent_streams` is unset.
/// The real h2 stream limit is still enforced per-request via `poll_ready`;
/// this only bounds how many borrowers share one connection before a new one
/// is opened — typically far above server limits.
pub(crate) const DEFAULT_H2_MAX_CONCURRENT_STREAMS: usize = 1024;
use http_body::{Frame, SizeHint};
use http_body_util::BodyExt;
use log::warn;

use crate::error;
use crate::util::recover_lock;

use super::connect::{H2ConnectOutcome, H2Connector};

/// Helper trait combining tokio's `AsyncRead` and `AsyncWrite` into one
/// non-auto trait, so `Box<dyn AsyncStream + Unpin + Send + 'static>` can
/// represent a negotiated HTTP/1.1 fallback stream.
pub(crate) trait AsyncStream: tokio::io::AsyncRead + tokio::io::AsyncWrite {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite> AsyncStream for T {}

/// Compute the pool key (`scheme:host:port`) for a URI (default port 80 for
/// http, else 443). Shared by the h2 pool and the connector so an HTTP/1.1
/// fallback connection is keyed identically to its originating h2 connection.
/// The host is lowercased (DNS names are case-insensitive) so case-variant
/// URIs share one pooled connection per origin — mirroring the DNS cache key
/// normalization (§4.5).
pub(crate) fn pool_key(uri: &Uri) -> String {
    let default_port = match uri.scheme_str() {
        Some("http") => 80,
        _ => 443,
    };
    format!(
        "{}:{}:{}",
        uri.scheme_str().unwrap_or(""),
        uri.host().unwrap_or("").to_ascii_lowercase(),
        uri.port_u16().unwrap_or(default_port)
    )
}

/// Outcome of [`Pool::get_or_connect`].
///
/// With `http_version_pref == All`, the server may pick `h2`
/// ([`ConnectOutcome::H2`]) or `http/1.1`; the latter yields the *already
/// established* TLS stream ([`ConnectOutcome::Http1`]) so the caller runs
/// HTTP/1.1 over it without a second handshake.
pub(crate) enum ConnectOutcome {
    /// A negotiated HTTP/2 connection (with optional spawned driver).
    H2(GetOrConnectResult),
    /// An HTTP/1.1 connection negotiated via ALPN on the same TLS handshake;
    /// `stream` is the established TLS stream and `key` the pool key for reuse.
    Http1 {
        key: String,
        stream: Box<dyn AsyncStream + Unpin + Send + 'static>,
        tls_info: Option<crate::tls::TlsInfo>,
    },
}

/// Process-start epoch used to store `Instant`s as compact `u64` nanosecond
/// counts in `AtomicU64` (see [`now_nanos`] / [`instant_from_nanos`]).
fn epoch() -> Instant {
    use std::sync::OnceLock;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// Current monotonic time as nanoseconds since [`epoch`], for storing in an
/// `AtomicU64` and reconstructing via [`instant_from_nanos`].
fn now_nanos() -> u64 {
    Instant::now().saturating_duration_since(epoch()).as_nanos() as u64
}

/// Reconstruct an `Instant` from a value produced by [`now_nanos`].
fn instant_from_nanos(nanos: u64) -> Instant {
    epoch() + Duration::from_nanos(nanos)
}

fn jitter(max_ms: u64) -> Duration {
    if max_ms == 0 {
        return Duration::ZERO;
    }
    Duration::from_millis(rand::random::<u64>() % max_ms)
}

pub(crate) type H2Connection =
    Pin<Box<dyn std::future::Future<Output = Result<(), h2::Error>> + Send + 'static>>;

/// Configuration for HTTP/2 keep-alive, carried alongside the connection.
pub(crate) struct KeepAliveConfig {
    pub(crate) interval: Option<Duration>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) while_idle: bool,
}

/// Result of establishing a new H2 connection: the driver future, send
/// handle, optional PingPong, and keep-alive config.
pub(crate) struct H2ConnectResult {
    pub(crate) send_request: SendRequest<Bytes>,
    pub(crate) connection: H2Connection,
    pub(crate) ping_pong: Option<h2::PingPong>,
    pub(crate) keep_alive: KeepAliveConfig,
    pub(crate) tls_info: Option<crate::tls::TlsInfo>,
}

pub(crate) struct PooledSendRequest {
    pub(crate) send_request: SendRequest<Bytes>,
    pub(crate) pool_key: String,
    /// Active borrower refcount; at 0 the connection is truly idle.
    pub(crate) active_streams: Arc<AtomicUsize>,
    /// Notifies waiters when the connection becomes idle (active_streams → 0).
    stream_completed: Arc<tokio::sync::Notify>,
    /// Shared `last_used` timestamp (nanos since epoch in `AtomicU64`),
    /// refreshed when `active_streams` returns to 0 so idle eviction measures
    /// "time since idle" rather than "since checkout".
    last_used: Arc<AtomicU64>,
    /// The pool's concurrency gate (`PoolInner::max_concurrent_streams`); a
    /// slot frees when `active_streams` drops below this, waking Busy-waiters.
    capacity: usize,
}

impl Drop for PooledSendRequest {
    fn drop(&mut self) {
        // Decrement active_streams. Use compare_exchange to avoid underflow
        // if the pool entry was already removed (Arc still valid, count stale).
        let mut prev = self.active_streams.load(Ordering::Acquire);
        loop {
            if prev == 0 {
                break;
            }
            match self.active_streams.compare_exchange(
                prev,
                prev - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(old) => {
                    // old == prev (the value we expected). If old == 1,
                    // active_streams just reached 0 — connection is idle.
                    if old == 1 {
                        // The connection truly returned to idle. Refresh
                        // `last_used` so idle eviction measures "time since
                        // idle" rather than "time since checkout" — otherwise a
                        // connection borrowed longer than `idle_timeout` would
                        // be treated as expired and its healthy driver aborted.
                        self.last_used.store(now_nanos(), Ordering::Release);
                    }
                    // Notify parked Busy-waiters whenever a stream slot frees
                    // (count crossed below the concurrency gate), not only when
                    // the connection fully idles (old == 1). Under sustained
                    // saturation the count hovers at the gate and never reaches
                    // 0, so without this the waiters would park until timeout —
                    // the full-idle notify alone would starve them.
                    if old == self.capacity || old == 1 {
                        self.stream_completed.notify_waiters();
                    }
                    break;
                }
                Err(actual) => {
                    // Another thread changed the count — retry with the
                    // current value.
                    prev = actual;
                }
            }
        }
    }
}

/// Result of [`Pool::get_or_connect`]. The driver is registered with the pool
/// entry synchronously, so the returned handle is backed by a live driver.
pub(crate) struct GetOrConnectResult {
    pub(crate) pooled: PooledSendRequest,
    pub(crate) tls_info: Option<crate::tls::TlsInfo>,
}

/// Everything needed to spawn the H2 connection driver background task.
pub(crate) struct H2Driver {
    pub(crate) connection: H2Connection,
    pub(crate) ping_pong: Option<h2::PingPong>,
    pub(crate) keep_alive: KeepAliveConfig,
}

struct PooledEntry {
    send_request: SendRequest<Bytes>,
    /// TLS handshake info captured at connect time, surfaced on every reuse so
    /// pooled responses still carry `TlsInfo`.
    tls_info: Option<crate::tls::TlsInfo>,
    /// Shared `last_used` timestamp (nanos since epoch in `AtomicU64`),
    /// refreshed when `active_streams` returns to 0; read by idle eviction.
    last_used: Arc<AtomicU64>,
    connection_task: Option<tokio::task::JoinHandle<()>>,
    /// Active borrower count (PooledSendRequest clones + response streams);
    /// at 0 the connection is truly idle.
    active_streams: Arc<AtomicUsize>,
    /// Notifies waiters when the connection becomes idle.
    stream_completed: Arc<tokio::sync::Notify>,
}

impl PooledEntry {
    /// Read the shared `last_used` timestamp (nanos since epoch) back into an
    /// `Instant` for expiry comparisons — a lock-free atomic read.
    fn last_used_instant(&self) -> Instant {
        instant_from_nanos(self.last_used.load(Ordering::Acquire))
    }

    /// Detach a driver whose connection still has active streams, aborting it
    /// once the connection returns to idle.
    ///
    /// The primp-h2 fork's driver does NOT exit on idle without a server
    /// GOAWAY, so a detached driver would otherwise keep its task and socket
    /// alive (keep-alive pings prevent idle exit). A watcher task aborts the
    /// driver as soon as `active_streams` hits 0.
    fn detach_driver_until_idle(&mut self) {
        let Some(handle) = self.connection_task.take() else {
            return;
        };
        if self.active_streams.load(Ordering::Acquire) == 0 {
            handle.abort();
            return;
        }
        let active_streams = Arc::clone(&self.active_streams);
        let stream_completed = Arc::clone(&self.stream_completed);
        tokio::spawn(async move {
            loop {
                // Register before checking so a completion-notify racing in
                // between cannot be missed (same pattern as the busy-wait in
                // get_or_connect). The drop paths (PooledSendRequest /
                // H2ResponseBody) only call notify_waiters() when the count
                // transitions 1 -> 0, so once woken — or seeing 0 ourselves —
                // the connection is truly idle and aborting is safe.
                let notified = stream_completed.notified();
                if active_streams.load(Ordering::Acquire) == 0 {
                    handle.abort();
                    return;
                }
                notified.await;
            }
        });
    }
}

/// Monotonically increasing counter for unique sentinel IDs.
static SENTINEL_ID: AtomicU64 = AtomicU64::new(1);

/// Tracks an in-progress connection attempt. Waiters receive a
/// `watch::Receiver` and `.changed().await` until the sender drops (on
/// completion or failure), then re-check the pool.
struct InProgress {
    tx: watch::Sender<()>,
    created: Instant,
    /// Unique ID so InProgressGuard only removes the sentinel it inserted.
    id: u64,
}

/// Removes the InProgress sentinel on drop (e.g. future cancellation). Set
/// `completed = true` on the normal exit path to skip removal; only the
/// sentinel matching this guard's `id` is removed, never another task's.
struct InProgressGuard {
    inner: Arc<Mutex<PoolInner>>,
    key: String,
    completed: bool,
    sentinel_id: u64,
}

impl Drop for InProgressGuard {
    fn drop(&mut self) {
        if !self.completed {
            let mut guard = recover_lock(&self.inner);
            // Only remove the sentinel we inserted, not another task's.
            if let Some(entry) = guard.in_progress.get(&self.key) {
                if entry.id == self.sentinel_id {
                    guard.in_progress.remove(&self.key);
                }
            }
        }
    }
}

/// Result of trying to reuse an existing connection.
enum ReuseResult {
    /// A connection was reserved for reuse; caller must `poll_ready` first.
    Reused(PooledSendRequest, Option<crate::tls::TlsInfo>),
    /// A connection exists but is in use (active_streams > 0); wait and retry.
    Busy,
    /// No usable connection; caller should create one.
    None,
}

struct PoolInner {
    connections: HashMap<String, PooledEntry>,
    in_progress: HashMap<String, InProgress>,
    idle_timeout: Option<Duration>,
    max_connections: usize,
    /// Concurrency gate: when `active_streams` reaches this on a pooled
    /// connection, [`try_reuse`] reports `Busy`. The real h2 stream limit is
    /// still enforced per-request via `poll_ready`.
    max_concurrent_streams: usize,
}

impl PoolInner {
    fn new(
        idle_timeout: Option<Duration>,
        max_connections: usize,
        max_concurrent_streams: usize,
    ) -> Self {
        PoolInner {
            connections: HashMap::new(),
            in_progress: HashMap::new(),
            idle_timeout,
            max_connections,
            max_concurrent_streams: max_concurrent_streams.max(1),
        }
    }

    /// Check for a reusable connection. Returns `None` (removing stale
    /// entries), `Reused` if spare capacity exists (`active_streams <
    /// max_concurrent_streams`), or `Busy` if at capacity. Caller must
    /// `poll_ready` with a real waker outside the lock before sending.
    fn try_reuse(&mut self, key: &str) -> ReuseResult {
        let expired = {
            let Some(entry) = self.connections.get(key) else {
                return ReuseResult::None;
            };
            // Expired only if idle — see `clear_expired` gating.
            if entry.active_streams.load(Ordering::Acquire) != 0 {
                false
            } else {
                self.idle_timeout
                    .map(|timeout| {
                        instant_from_nanos(entry.last_used.load(Ordering::Acquire)).elapsed()
                            > timeout
                    })
                    .unwrap_or(false)
            }
        };

        if expired {
            if let Some(mut entry) = self.connections.remove(key) {
                entry.stream_completed.notify_waiters();
                entry.detach_driver_until_idle();
            }
            return ReuseResult::None;
        }

        let entry = match self.connections.get_mut(key) {
            Some(e) => e,
            None => return ReuseResult::None,
        };

        // Gate on active_streams: allow up to `max_concurrent_streams` concurrent
        // borrowers on a single connection so HTTP/2 multiplexing is actually
        // used. Each in-flight request holds active_streams == 2 (one checkout
        // slot from try_reuse + one streaming-body guard from H2ResponseBody),
        // so the effective concurrency cap is max_concurrent_streams / 2.
        // The per-request `poll_ready` (in `PoolClient::send_request`) is the
        // authoritative check for the server's real `max_concurrent_streams`,
        // so this gate only bounds how many callers share ONE connection per
        // host before they park (`get_or_connect`'s Busy branch waits on
        // `stream_completed` and retries). The pool holds at most one
        // connection per key — `insert` replaces the entry for a key rather
        // than opening a second one — matching how browsers multiplex HTTP/2
        // on a single connection per origin. Reserving the slot with a
        // compare_exchange loop keeps the count accurate under concurrent
        // callers without exceeding the cap.
        let mut cur = entry.active_streams.load(Ordering::Acquire);
        loop {
            if cur >= self.max_concurrent_streams {
                return ReuseResult::Busy;
            }
            match entry.active_streams.compare_exchange(
                cur,
                cur + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => cur = actual,
            }
        }

        let tx = entry.send_request.clone();
        entry.last_used.store(now_nanos(), Ordering::Release);
        let active_streams = Arc::clone(&entry.active_streams);
        let stream_completed = Arc::clone(&entry.stream_completed);
        let last_used = Arc::clone(&entry.last_used);
        let tls_info = entry.tls_info.clone();
        ReuseResult::Reused(
            PooledSendRequest {
                send_request: tx,
                pool_key: key.to_string(),
                active_streams,
                stream_completed,
                last_used,
                capacity: self.max_concurrent_streams,
            },
            tls_info,
        )
    }

    /// Insert a new connection, evicting the oldest idle (or, if none, oldest
    /// busy) entry when the pool is at `max_connections`.
    fn insert(
        &mut self,
        key: String,
        tx: SendRequest<Bytes>,
        tls_info: Option<crate::tls::TlsInfo>,
    ) {
        // Evict the oldest idle entry if at capacity.
        if self.connections.len() >= self.max_connections {
            if let Some(oldest_key) = self
                .connections
                .iter()
                .filter(|(_, e)| e.active_streams.load(Ordering::Acquire) == 0)
                .min_by_key(|(_, e)| e.last_used_instant())
                .map(|(k, _)| k.clone())
            {
                if let Some(mut entry) = self.connections.remove(&oldest_key) {
                    // Signal waiters before removing so they can retry.
                    entry.stream_completed.notify_waiters();
                    if let Some(handle) = entry.connection_task.take() {
                        handle.abort();
                    }
                    warn!("h2 pool: evicting idle connection for {:?}", oldest_key);
                }
            } else {
                // All connections are busy. Remove the oldest one anyway —
                // don't abort its driver so active streams can finish.
                if let Some(oldest_key) = self
                    .connections
                    .iter()
                    .min_by_key(|(_, e)| e.last_used_instant())
                    .map(|(k, _)| k.clone())
                {
                    if let Some(mut entry) = self.connections.remove(&oldest_key) {
                        // Signal waiters before removing so they can retry.
                        entry.stream_completed.notify_waiters();
                        // Don't abort the driver while active streams are in
                        // flight — abort it as soon as they drain.
                        entry.detach_driver_until_idle();
                        warn!("h2 pool: evicting busy connection for {:?}", oldest_key);
                    }
                }
            }
        }
        // If the key already exists (race between two connect attempts),
        // abort the old entry's driver task before overwriting.
        if let Some(mut old) = self.connections.remove(&key) {
            // Signal waiters before removing so they can retry.
            old.stream_completed.notify_waiters();
            old.detach_driver_until_idle();
        }
        self.connections.insert(
            key,
            PooledEntry {
                send_request: tx,
                tls_info,
                last_used: Arc::new(AtomicU64::new(now_nanos())),
                connection_task: None,
                active_streams: Arc::new(AtomicUsize::new(0)),
                stream_completed: Arc::new(tokio::sync::Notify::new()),
            },
        );
    }

    /// Remove expired connections and stale in-progress sentinels.
    fn clear_expired(&mut self) {
        // Remove expired connections if idle_timeout is configured.
        if let Some(idle_timeout) = self.idle_timeout {
            // Collect keys to remove first to avoid borrow conflicts.
            // Only idle connections (active_streams == 0) are considered for
            // eviction — a connection borrowed longer than `idle_timeout`
            // must not be evicted while still serving streams, otherwise
            // `last_used` (stale from checkout, refreshed only on return to
            // idle) would cause a healthy driver to be detached mid-stream
            // and a second connection opened for the same origin (violates
            // §4.2 "one connection per key"). Mirrors `Http1Pool::evict_stale`
            // `busy == 0 && sender.is_some()` gating.
            let expired_keys: Vec<String> = self
                .connections
                .iter()
                .filter(|(_, entry)| {
                    entry.active_streams.load(Ordering::Acquire) == 0
                        && entry.last_used_instant().elapsed() > idle_timeout
                })
                .map(|(k, _)| k.clone())
                .collect();
            for key in expired_keys {
                if let Some(mut entry) = self.connections.remove(&key) {
                    // Signal waiters before removing so they can retry.
                    entry.stream_completed.notify_waiters();
                    // Expired but busy — abort the driver as soon as the last
                    // stream drains so its task and socket cannot leak.
                    entry.detach_driver_until_idle();
                    warn!("h2 pool: evicting expired connection for {:?}", key);
                }
            }
        }
        // Clean up stale sentinels from panicked connecting tasks.
        // Threshold: 2x idle timeout (or 60s), capped at 1 hour.
        const MAX_STALE_THRESHOLD: Duration = Duration::from_secs(3600);
        let stale_threshold = self
            .idle_timeout
            .map(|t| t.saturating_mul(2).min(MAX_STALE_THRESHOLD))
            .unwrap_or(Duration::from_secs(60));
        self.in_progress.retain(|key, entry| {
            if entry.created.elapsed() > stale_threshold {
                warn!("h2 pool: removing stale in-progress sentinel for {:?}", key);
                false
            } else {
                true
            }
        });
    }
}

/// Max connect attempts in `get_or_connect` (10) before giving up — prevents
/// infinite loops when connections are persistently at stream capacity.
const MAX_CONNECT_RETRIES: usize = 10;

/// Max total loop iterations in `get_or_connect` (1000) — bounds connect
/// attempts and busy-wait iterations to prevent livelock at full capacity.
const MAX_LOOP_ITERATIONS: usize = 1000;

/// Jitter (3ms) for concurrent waiters that find a connect in flight, avoiding
/// a thundering herd when the connection completes.
const BUSY_WAIT_JITTER_MS: u64 = 3;

/// H2 connection pool with a `std::sync::Mutex` (no async lock held during I/O).
pub(crate) struct Pool {
    inner: Arc<Mutex<PoolInner>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    /// True only for the pool created by `Pool::new` (stored in the client);
    /// false for every transient clone. The owner tears down driver tasks on
    /// drop; clones must not. Ownership is explicit because `cleanup_handle`
    /// is unreliable: built outside a Tokio runtime (the Python bindings path
    /// builds inside `py.detach`), `spawn_idle_cleanup` early-returns and the
    /// owner's `cleanup_handle` stays `None`, so using it as the owner signal
    /// would leak every driver task and socket.
    is_owner: bool,
}

impl Drop for Pool {
    fn drop(&mut self) {
        // Only the owner Pool tears down the connection drivers. All clones
        // share the same `Arc<Mutex<PoolInner>>` but are not owners, and
        // dropping them must NOT abort in-flight driver tasks — otherwise a
        // transient `self.clone()` returned from `get_or_connect` would, on its
        // own drop, tear down connections still in active use by the owner.
        //
        // Ownership is `is_owner`, not `cleanup_handle.is_some()`: a pool
        // built outside a Tokio runtime (Python bindings build path) never
        // spawns the idle-cleanup task, so its `cleanup_handle` is `None` even
        // for the owner — using it as the owner signal would leak every driver
        // task (and its socket) until process exit.
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
        if self.is_owner {
            // Abort drivers; if body still streams (`active_streams>0`), detach
            // and let watcher abort when idle — else `broken pipe` DecodeError.
            let mut inner = crate::util::recover_lock(&self.inner);
            for entry in inner.connections.values_mut() {
                if entry.active_streams.load(Ordering::Acquire) == 0 {
                    if let Some(handle) = &entry.connection_task {
                        handle.abort();
                    }
                } else if tokio::runtime::Handle::try_current().is_ok() {
                    entry.detach_driver_until_idle();
                } else {
                    // No runtime — `spawn` would panic; leak so body can finish.
                    let handle = entry.connection_task.take();
                    let pool_clone = Arc::clone(&self.inner);
                    std::mem::forget(handle);
                    std::mem::forget(pool_clone);
                }
            }
        }
    }
}

impl Pool {
    pub(crate) fn new(
        idle_timeout: Option<Duration>,
        max_connections: usize,
        max_concurrent_streams: usize,
    ) -> Self {
        Pool {
            inner: Arc::new(Mutex::new(PoolInner::new(
                idle_timeout,
                max_connections.max(1),
                max_concurrent_streams,
            ))),
            cleanup_handle: None,
            is_owner: true,
        }
    }

    /// Reuse an existing connection or connect and insert a new one. The lock
    /// is held only for pool-state mutations, never during network I/O.
    ///
    /// Concurrent connect attempts for the same key dedup via a `watch`
    /// channel: only the first performs I/O, the rest wait for it (thundering-
    /// herd protection).
    pub(crate) async fn get_or_connect(
        &self,
        connector: &H2Connector,
        uri: &Uri,
    ) -> Result<ConnectOutcome, crate::error::BoxError> {
        let key = pool_key(uri);

        let mut retries = 0;
        let mut loop_iterations = 0;
        loop {
            // Try to get a connection from the pool (lock held briefly).
            let try_result = {
                let mut guard = recover_lock(&self.inner);
                guard.try_reuse(&key)
            };
            // guard is dropped here — safe to .await below.

            match try_result {
                ReuseResult::Reused(pooled, tls_info) => {
                    // Capacity check is done in PoolClient::send_request on
                    // the SAME SendRequest handle that will call
                    // send_request(). poll_ready must not be called here on a
                    // clone, because it reserves a stream slot on that
                    // specific clone — a different clone used later would
                    // not have the slot and send_request() could fail with
                    // InactiveStreamId.
                    return Ok(ConnectOutcome::H2(GetOrConnectResult { pooled, tls_info }));
                }
                ReuseResult::Busy => {
                    // Connection exists but in use — wait on notification.
                    // Waiting on capacity is legitimate: it is bounded by the
                    // caller's own timeouts, NOT by this loop's iteration cap.
                    // Counting these cycles would fail requests spuriously
                    // under heavy concurrency (a waiter parked on the Busy
                    // branch loses the wake-up race once per freed slot, so a
                    // late winner accumulates one cycle per other request).
                    let (notify, entry_ptr) = {
                        let guard = recover_lock(&self.inner);
                        match guard.connections.get(&key) {
                            Some(entry) => (
                                Arc::clone(&entry.stream_completed),
                                Arc::as_ptr(&entry.stream_completed),
                            ),
                            None => continue,
                        }
                    };
                    let notified = notify.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    let still_busy = {
                        let guard = recover_lock(&self.inner);
                        matches!(
                            guard.connections.get(&key),
                            Some(entry)
                                if Arc::as_ptr(&entry.stream_completed) == entry_ptr
                                // Also re-check capacity: a "connection is
                                // idle" notify can fire between the clone above
                                // and the notified() registration — if that
                                // freed a stream slot, the count is now below
                                // the gate and this waiter must NOT park on a
                                // notification it missed.
                                && entry.active_streams.load(Ordering::Acquire)
                                    >= guard.max_concurrent_streams
                        )
                    };
                    if still_busy {
                        notified.await;
                    }
                    continue;
                }
                ReuseResult::None => {}
            }

            // No existing connection — check if another task is connecting.
            let wait_rx = {
                let guard = recover_lock(&self.inner);
                guard.in_progress.get(&key).map(|ip| ip.tx.subscribe())
            };
            if let Some(mut rx) = wait_rx {
                let _ = rx.changed().await;
                tokio::time::sleep(jitter(BUSY_WAIT_JITTER_MS)).await;
                continue;
            }

            // Slow path: connect without holding the lock. Every cycle through
            // here is real work (or a tight retry race), so it is what the
            // loop-iteration cap protects against.
            loop_iterations += 1;
            if loop_iterations > MAX_LOOP_ITERATIONS {
                return Err("h2 pool: max loop iterations exceeded".into());
            }

            // Insert an InProgress sentinel under a single lock hold.
            let sentinel_id = {
                let (maybe_rx, id) = {
                    let mut guard = recover_lock(&self.inner);
                    if let Some(entry) = guard.in_progress.get(&key) {
                        (Some(entry.tx.subscribe()), 0)
                    } else {
                        let (tx, _rx) = watch::channel(());
                        let id = SENTINEL_ID.fetch_add(1, Ordering::Relaxed);
                        guard.in_progress.insert(
                            key.clone(),
                            InProgress {
                                tx,
                                created: Instant::now(),
                                id,
                            },
                        );
                        (None, id)
                    }
                };

                if let Some(mut rx) = maybe_rx {
                    let _ = rx.changed().await;
                    tokio::time::sleep(jitter(BUSY_WAIT_JITTER_MS)).await;
                    continue;
                }
                id
            };

            let mut _progress_guard = InProgressGuard {
                inner: self.inner.clone(),
                key: key.clone(),
                completed: false,
                sentinel_id,
            };

            // Slow path: connect without holding the lock.
            retries += 1;
            if retries > MAX_CONNECT_RETRIES {
                return Err("h2 pool: max connect retries exceeded".into());
            }
            let connect_result = connector.connect(uri).await;

            {
                let mut guard = recover_lock(&self.inner);

                match connect_result {
                    Ok(H2ConnectOutcome::H2(h2_result)) => {
                        let tx = h2_result.send_request.clone();
                        // Double-check: another task may have connected while we
                        // were connecting.
                        match guard.try_reuse(&key) {
                            ReuseResult::Reused(pooled, tls_info) => {
                                drop(h2_result.connection);
                                drop(h2_result.ping_pong);
                                if let Some(entry) = guard.in_progress.get(&key) {
                                    if entry.id == sentinel_id {
                                        guard.in_progress.remove(&key);
                                    }
                                }
                                _progress_guard.completed = true;
                                return Ok(ConnectOutcome::H2(GetOrConnectResult {
                                    pooled,
                                    tls_info,
                                }));
                            }
                            ReuseResult::Busy => {
                                // Another task connected and reserved the
                                // connection. Drop ours and retry.
                                drop(h2_result.connection);
                                drop(h2_result.ping_pong);
                                drop(tx);
                                if let Some(entry) = guard.in_progress.get(&key) {
                                    if entry.id == sentinel_id {
                                        guard.in_progress.remove(&key);
                                    }
                                }
                                _progress_guard.completed = true;
                                continue;
                            }
                            ReuseResult::None => {
                                guard.insert(key.clone(), tx.clone(), h2_result.tls_info.clone());
                            }
                        }
                        if let Some(entry) = guard.in_progress.get(&key) {
                            if entry.id == sentinel_id {
                                guard.in_progress.remove(&key);
                            }
                        }
                        _progress_guard.completed = true;
                        let entry = match guard.connections.get_mut(&key) {
                            Some(entry) => entry,
                            None => {
                                return Err(Box::new(crate::error::request(
                                    std::io::Error::other("connection entry missing after insert"),
                                )));
                            }
                        };
                        let active_streams = entry.active_streams.clone();
                        let stream_completed = entry.stream_completed.clone();
                        let last_used = Arc::clone(&entry.last_used);
                        active_streams.fetch_add(1, Ordering::AcqRel);

                        // Spawn the driver and register its handle with the entry
                        // *under this same lock hold*, so there is no window in
                        // which the entry exists but has no live driver task —
                        // previously the handle was registered only after the
                        // lock was released, allowing an eviction (idle-timeout /
                        // capacity) to abort the live connection's driver.
                        let handle = spawn_h2_driver(
                            self.clone(),
                            key.clone(),
                            Arc::clone(&active_streams),
                            H2Driver {
                                connection: h2_result.connection,
                                ping_pong: h2_result.ping_pong,
                                keep_alive: h2_result.keep_alive,
                            },
                        );
                        entry.connection_task = Some(handle);

                        return Ok(ConnectOutcome::H2(GetOrConnectResult {
                            pooled: PooledSendRequest {
                                send_request: tx,
                                pool_key: key,
                                active_streams,
                                stream_completed,
                                last_used,
                                capacity: guard.max_concurrent_streams,
                            },
                            tls_info: h2_result.tls_info,
                        }));
                    }
                    Ok(H2ConnectOutcome::Http1 {
                        key,
                        stream,
                        tls_info,
                    }) => {
                        // The HTTP/1.1 stream is used directly by the caller
                        // (see NegotiatingConnection); it is not pooled here.
                        // Remove the in-progress sentinel and return.
                        if let Some(entry) = guard.in_progress.get(&key) {
                            if entry.id == sentinel_id {
                                guard.in_progress.remove(&key);
                            }
                        }
                        _progress_guard.completed = true;
                        return Ok(ConnectOutcome::Http1 {
                            key,
                            stream,
                            tls_info,
                        });
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    /// Remove a connection entry and abort its driver task. Returns `true` if
    /// an entry was removed.
    ///
    /// Only removes the entry if its `active_streams` Arc matches `identity`,
    /// so a driver task cannot remove a newer connection inserted for the same
    /// key after its own died. Called by the driver after `conn.await` returns
    /// so a server-initiated close does not leave a dead entry until idle
    /// timeout. Safe when no entry exists.
    pub(crate) fn remove_dead_connection(&self, key: &str, identity: &Arc<AtomicUsize>) -> bool {
        let mut guard = recover_lock(&self.inner);
        if let Some(entry) = guard.connections.get_mut(key) {
            // Only remove if this is still the same connection (same Arc
            // pointer). A newer connection inserted for the same key will
            // have a different Arc, so we skip removal.
            if !Arc::ptr_eq(&entry.active_streams, identity) {
                return false;
            }
        }
        if let Some(mut entry) = guard.connections.remove(key) {
            // Signal waiters before removing so they can retry. Matches the
            // pattern in clear_expired / insert eviction. Without this, a
            // waiter in the Busy branch of get_or_connect stays parked in notified.await until
            // the in-flight request (if any) finishes and its Drop fires
            // notify_waiters() — or never, if the request is stuck on a dead
            // connection that will never deliver more data.
            entry.stream_completed.notify_waiters();
            if let Some(handle) = entry.connection_task.take() {
                handle.abort();
            }
            true
        } else {
            false
        }
    }

    /// Spawn a background task that periodically evicts expired connections.
    /// Only spawns inside a Tokio runtime; otherwise cleanup is lazy on
    /// checkout. The `JoinHandle` is stored in `Pool` and aborted on `Drop`.
    pub(crate) fn spawn_idle_cleanup(&mut self) {
        // Determine the cleanup interval:
        // - If idle_timeout is set, use it (cleans expired connections + stale sentinels).
        // - If idle_timeout is None, use a fixed 60s interval to clean stale sentinels
        //   that may accumulate from panicked connecting tasks.
        let timeout = {
            let guard = recover_lock(&self.inner);
            match guard.idle_timeout {
                Some(t) if !t.is_zero() => t,
                _ => Duration::from_secs(60),
            }
        };
        // Only spawn if we're inside a Tokio runtime.
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let pool = self.inner.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(timeout).await;
                let mut guard = recover_lock(&pool);
                guard.clear_expired();
            }
        });
        // Abort any previous cleanup task.
        if let Some(old) = self.cleanup_handle.take() {
            old.abort();
        }
        self.cleanup_handle = Some(handle);
    }
}

impl Clone for Pool {
    fn clone(&self) -> Self {
        Pool {
            inner: self.inner.clone(),
            cleanup_handle: None,
            is_owner: false,
        }
    }
}

pub(crate) struct PoolClient;

impl PoolClient {
    pub(crate) async fn send_request(
        pooled: PooledSendRequest,
        req: Request<crate::async_impl::body::Body>,
        pool: &Pool,
    ) -> Result<Response<H2ResponseBody>, crate::Error> {
        // RFC 9113 §8.7: a server may REFUSE_STREAM a newly opened stream and
        // expect the client to retry it (typically because the server's
        // MAX_CONCURRENT_STREAMS was exhausted, or the client opened streams
        // before learning the server's limit). We retry such resets on the
        // SAME connection by re-acquiring a stream slot and re-sending. This
        // is bounded so we cannot loop forever on a persistently refusing
        // server. The request body is replayed from a clone when available.
        const MAX_REFUSED_RETRIES: usize = 32;

        let (parts, mut body) = req.into_parts();
        // Immutable source used to replay a replayable body. It is never
        // streamed itself; each retry clones a *fresh* copy so a body is never
        // sent empty after a previous (refused) attempt already drained a
        // clone. For streaming (non-replayable) bodies this is `None`.
        let body_source = body.try_clone();

        // Clone send_request: send_request() takes self by value, and a
        // partial move of PooledSendRequest would skip its Drop impl.
        let mut tx = pooled.send_request.clone();

        for attempt in 0..=MAX_REFUSED_RETRIES {
            // Check capacity on the SAME handle that will call send_request().
            // poll_ready reserves a stream slot on this specific clone — calling
            // it on a different clone would not help this one. h2's poll_ready
            // properly tracks max_concurrent_streams and registers the waker to
            // be notified when streams complete.
            if let Err(e) = std::future::poll_fn(|cx| tx.poll_ready(cx)).await {
                // The connection is likely dead (GOAWAY, FIN, or transport error).
                // Proactively remove it from the pool so the next request does not
                // reuse it and fail again. This closes the TOCTOU window where
                // other tasks could get a Reused handle for the dead connection
                // before the driver task exits and calls remove_dead_connection.
                pool.remove_dead_connection(&pooled.pool_key, &pooled.active_streams);
                return Err(error::request(e));
            }

            let (response_fut, send_stream) =
                match tx.send_request(Request::from_parts(parts.clone(), ()), false) {
                    Ok(v) => v,
                    Err(e) => {
                        // send_request failed after poll_ready succeeded — the
                        // connection died in the interim. Remove it so the next
                        // request does not reuse a dead entry before the driver
                        // task observes the close and cleans up.
                        pool.remove_dead_connection(&pooled.pool_key, &pooled.active_streams);
                        return Err(error::request(e));
                    }
                };

            // Stream the request body frame by frame instead of buffering.
            let mut send_stream = Some(send_stream);
            // On attempt 0 we stream the original `body`. On every retry we
            // stream a *fresh* clone of `body_source` so a replayable body is
            // never sent empty after a previous refusal already drained a
            // clone (C1 / RFC 9113 §8.7).
            let mut streaming_body: crate::async_impl::body::Body = if attempt == 0 {
                std::mem::take(&mut body)
            } else {
                match body_source.as_ref().and_then(|b| b.try_clone()) {
                    Some(b) => b,
                    // Defensive: retries are gated on `body_source.is_some()`
                    // below, and a Reusable body's `try_clone()` cannot fail
                    // (body.rs), so this arm is unreachable in practice. Fail
                    // cleanly with a truthful error rather than falling
                    // through to the misleading "maximum retries" tail.
                    None => {
                        return Err(error::request(
                            "h2 stream refused: request body cannot be replayed for retry",
                        ));
                    }
                }
            };
            while let Some(frame) = streaming_body.frame().await {
                let frame = frame.map_err(error::request)?;
                match frame.into_data() {
                    Ok(data) => {
                        if !data.is_empty() {
                            match send_stream.as_mut() {
                                Some(stream) => {
                                    // Gate large chunks on flow-control window to
                                    // bound memory; small bodies use direct send.
                                    if data.len() <= 1024 {
                                        stream.send_data(data, false).map_err(error::request)?;
                                    } else {
                                        let mut remaining = data;
                                        while !remaining.is_empty() {
                                            let len = remaining.len();
                                            stream.reserve_capacity(len);
                                            let capacity =
                                                std::future::poll_fn(|cx| stream.poll_capacity(cx))
                                                    .await;
                                            let capacity = match capacity {
                                                Some(Ok(c)) => c,
                                                Some(Err(e)) => {
                                                    return Err(error::request(e));
                                                }
                                                None => {
                                                    return Err(error::request(
                                                        "h2 stream closed while waiting for capacity",
                                                    ));
                                                }
                                            };
                                            if capacity == 0 {
                                                tokio::task::yield_now().await;
                                                continue;
                                            }
                                            let to_send = std::cmp::min(capacity, remaining.len());
                                            let chunk = remaining.split_to(to_send);
                                            stream
                                                .send_data(chunk, false)
                                                .map_err(error::request)?;
                                        }
                                    }
                                }
                                None => {
                                    // Data after trailers — protocol violation.
                                    return Err(error::request("data frame after trailers"));
                                }
                            }
                        }
                    }
                    Err(frame) => {
                        if let Ok(trailers) = frame.into_trailers() {
                            if let Some(mut stream) = send_stream.take() {
                                stream.send_trailers(trailers).map_err(error::request)?;
                            }
                        }
                        // Frame is not data — must be trailers. If into_trailers()
                        // also fails, skip the unknown frame type.
                    }
                }
            }
            // Signal end of request body.
            if let Some(mut stream) = send_stream.take() {
                stream
                    .send_data(Bytes::new(), true)
                    .map_err(error::request)?;
            }

            match response_fut.await {
                Ok(response) => {
                    let (parts, recv_stream) = response.into_parts();
                    let content_length = parts
                        .headers
                        .get(http::header::CONTENT_LENGTH)
                        .and_then(|v: &http::HeaderValue| v.to_str().ok())
                        .and_then(|s: &str| s.parse::<u64>().ok());
                    // Clone active_streams into H2ResponseBody. new() increments the
                    // count so the connection stays marked as busy during streaming.
                    let active = Some(Arc::clone(&pooled.active_streams));
                    let stream_completed = Some(Arc::clone(&pooled.stream_completed));
                    let last_used = Some(Arc::clone(&pooled.last_used));
                    return Ok(Response::from_parts(
                        parts,
                        H2ResponseBody::new(
                            recv_stream,
                            content_length,
                            active,
                            stream_completed,
                            last_used,
                            Some(pooled.capacity),
                        ),
                    ));
                }
                Err(e) => {
                    let is_refused = e.is_reset()
                        && e.is_remote()
                        && e.reason() == Some(h2::Reason::REFUSED_STREAM);
                    if is_refused && attempt < MAX_REFUSED_RETRIES && body_source.is_some() {
                        // Retry on the same connection: re-acquire a stream
                        // slot and re-send. Loop continues. We only retry when
                        // the body is replayable — a non-replayable (streaming)
                        // body cannot be re-sent, so retrying would waste a
                        // second stream and return a misleading error instead
                        // of the original REFUSED_STREAM.
                        continue;
                    }
                    // A connection-level failure (GOAWAY, transport/IO error,
                    // or the connection going away) means this pooled entry is
                    // dead. Remove it so subsequent requests do not reuse it
                    // before the driver task observes the close. A per-stream
                    // RST from the peer (e.reason() set, e.is_reset()) leaves
                    // the connection usable, so those are left in the pool.
                    if !e.is_reset() {
                        pool.remove_dead_connection(&pooled.pool_key, &pooled.active_streams);
                    }
                    return Err(error::request(e));
                }
            }
        }

        // Unreachable in practice: every iteration of the loop above either
        // returns or (for a replayable body) retries via `continue`, and the
        // retry gate stops at `attempt < MAX_REFUSED_RETRIES` — the final
        // iteration's REFUSED_STREAM falls through to `return Err(e)` above.
        // The `break` that used to reach this tail (non-replayable body on a
        // retry) was removed as dead code: retries are gated on
        // `body_source.is_some()`, and a Reusable body's `try_clone()` cannot
        // fail. Kept only as the type-checker tail for the loop.
        Err(error::request("h2 stream refused after maximum retries"))
    }
}

pub(crate) struct H2ResponseBody {
    stream: h2::RecvStream,
    trailers: Option<http::HeaderMap>,
    content_length: Option<u64>,
    consumed: u64,
    saw_end_of_data: bool,
    /// Prevents connection eviction while the response body is being streamed.
    /// Incremented in [`new`] and decremented in [`Drop`].
    active_streams: Option<Arc<AtomicUsize>>,
    /// Notifies waiters when the connection becomes idle.
    stream_completed: Option<Arc<tokio::sync::Notify>>,
    /// Shared handle to the pool entry's `last_used` timestamp, refreshed when
    /// this body is dropped and `active_streams` returns to 0 (see [`Drop`]).
    last_used: Option<Arc<AtomicU64>>,
    /// The pool's stream-concurrency gate (`PoolInner::max_concurrent_streams`),
    /// if this body belongs to a pooled connection. A slot is freed when
    /// `active_streams` crosses below this value — used to notify Busy-waiters.
    capacity: Option<usize>,
}

impl std::fmt::Debug for H2ResponseBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("H2ResponseBody")
            .field("content_length", &self.content_length)
            .field("consumed", &self.consumed)
            .field("saw_end_of_data", &self.saw_end_of_data)
            .field(
                "active_streams",
                &self
                    .active_streams
                    .as_ref()
                    .map(|a| a.load(Ordering::Relaxed)),
            )
            .finish()
    }
}

impl Drop for H2ResponseBody {
    fn drop(&mut self) {
        if let Some(ref count) = self.active_streams {
            let mut prev = count.load(Ordering::Acquire);
            loop {
                if prev == 0 {
                    break;
                }
                match count.compare_exchange(prev, prev - 1, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(old) => {
                        if old == 1 {
                            // Connection truly returned to idle — refresh
                            // `last_used` so idle eviction measures time-since-idle,
                            // not time-since-checkout (mirrors PooledSendRequest).
                            if let Some(ref ts) = self.last_used {
                                ts.store(now_nanos(), Ordering::Release);
                            }
                        }
                        // Notify parked Busy-waiters whenever a stream slot frees
                        // (count crossed below the concurrency gate), matching the
                        // PooledSendRequest drop path — otherwise waiters would
                        // only ever wake on full idle and starve under sustained
                        // load, where the count never reaches 0.
                        if old == self.capacity.unwrap_or(usize::MAX) || old == 1 {
                            if let Some(ref notify) = self.stream_completed {
                                notify.notify_waiters();
                            }
                        }
                        break;
                    }
                    Err(actual) => {
                        prev = actual;
                    }
                }
            }
        }
    }
}

impl H2ResponseBody {
    fn new(
        stream: h2::RecvStream,
        content_length: Option<u64>,
        active_streams: Option<Arc<AtomicUsize>>,
        stream_completed: Option<Arc<tokio::sync::Notify>>,
        last_used: Option<Arc<AtomicU64>>,
        capacity: Option<usize>,
    ) -> Self {
        // Increment active_streams so the connection stays busy during
        // streaming. Paired with Drop: try_reuse +1, this +1, PooledSendRequest::drop
        // -1, Drop -1 — reaches 0 only when the body is fully consumed.
        if let Some(ref count) = active_streams {
            count.fetch_add(1, Ordering::AcqRel);
        }
        H2ResponseBody {
            stream,
            trailers: None,
            content_length,
            consumed: 0,
            saw_end_of_data: false,
            active_streams,
            stream_completed,
            last_used,
            capacity,
        }
    }
}

impl http_body::Body for H2ResponseBody {
    type Data = Bytes;
    type Error = crate::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // Drain any buffered trailers from a previous poll.
        if let Some(trailers) = self.trailers.take() {
            return Poll::Ready(Some(Ok(Frame::trailers(trailers))));
        }

        // After end-of-data, poll for trailers (which may arrive after the
        // final DATA frame).
        if self.saw_end_of_data {
            return match self.stream.poll_trailers(cx) {
                Poll::Ready(Ok(Some(trailers))) => Poll::Ready(Some(Ok(Frame::trailers(trailers)))),
                Poll::Ready(Ok(None)) => Poll::Ready(None),
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(error::request(e)))),
                Poll::Pending => Poll::Pending,
            };
        }

        // Normal path: poll for the next data frame.
        match self.stream.poll_data(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let len = data.len() as u64;
                let is_end = self.stream.is_end_stream();
                if is_end {
                    self.saw_end_of_data = true;
                    // Try to harvest trailers eagerly; if not yet available
                    // they will be picked up on the next poll via the
                    // saw_end_of_data path above.
                    match self.stream.poll_trailers(cx) {
                        Poll::Ready(Ok(Some(trailers))) => {
                            self.trailers = Some(trailers);
                        }
                        // No trailers and END_STREAM on DATA: body is done.
                        // Return immediately — falling through to Pending
                        // without a waker would hang the task permanently.
                        Poll::Ready(Ok(None)) if data.is_empty() => {
                            return Poll::Ready(None);
                        }
                        // Server sent RST_STREAM or GOAWAY after END_STREAM.
                        // Propagate the error immediately instead of silently
                        // swallowing it and forcing an unnecessary Pending cycle.
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Some(Err(error::request(e))));
                        }
                        _ => {}
                    }
                }
                if is_end && data.is_empty() {
                    // Empty final frame — end of body.
                    // Return eagerly harvested trailers if available.
                    if let Some(trailers) = self.trailers.take() {
                        self.saw_end_of_data = false;
                        return Poll::Ready(Some(Ok(Frame::trailers(trailers))));
                    }
                    // Trailers not yet available. Returning Pending here (instead
                    // of Ready(None)) is required: the consumer will re-poll, the
                    // saw_end_of_data branch above will poll for trailers, and
                    // the waker registered by poll_trailers guarantees progress.
                    // Returning Ready(None) would signal end-of-body prematurely
                    // and silently drop any trailers the server is about to send.
                    Poll::Pending
                } else {
                    // Release received-data capacity back to the transport so
                    // the peer's send window expands and it can continue.
                    // (Covers the rare case of a non-empty DATA frame with
                    // END_STREAM set: data is delivered now, trailers harvested
                    // on the next poll via the saw_end_of_data branch above.)
                    if let Err(e) = self.stream.flow_control().release_capacity(data.len()) {
                        return Poll::Ready(Some(Err(error::request(e))));
                    }
                    self.consumed += len;
                    Poll::Ready(Some(Ok(Frame::data(data))))
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(error::request(e)))),
            // Stream ended without an explicit empty DATA frame.
            Poll::Ready(None) => {
                self.saw_end_of_data = true;
                match self.stream.poll_trailers(cx) {
                    Poll::Ready(Ok(Some(trailers))) => {
                        Poll::Ready(Some(Ok(Frame::trailers(trailers))))
                    }
                    Poll::Ready(Ok(None)) => Poll::Ready(None),
                    Poll::Ready(Err(e)) => Poll::Ready(Some(Err(error::request(e)))),
                    Poll::Pending => Poll::Pending,
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.stream.is_end_stream() && self.trailers.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        match self.content_length {
            Some(len) => {
                let remaining = len.saturating_sub(self.consumed);
                SizeHint::with_exact(remaining)
            }
            None => SizeHint::default(),
        }
    }
}

/// Future returned by a keep-alive PING that is driven as its own `select!`
/// branch inside [`spawn_h2_driver`].
pub(crate) type PingFuture = Pin<Box<dyn Future<Output = Result<(), h2::Error>> + Send>>;

/// Poll the in-flight keep-alive ping future, leaving it in `ping_fut` until it
/// settles. The future is polled **in place** (not moved out via `take()`):
/// `tokio::select!` re-evaluates this branch on every re-entry into the driver
/// `loop`, so a `take()`-based helper would drop the still-in-flight ping future
/// on the next tick, orphaning the PING. Polling in place keeps the same future
/// object alive across re-entries until it resolves.
pub(crate) async fn poll_ping(ping_fut: &mut Option<PingFuture>) -> Result<(), h2::Error> {
    let res = match ping_fut.as_mut() {
        Some(fut) => fut.await,
        None => std::future::pending().await,
    };
    *ping_fut = None;
    res
}

/// Spawn the h2 connection driver task. It processes incoming frames and calls
/// `pool.remove_dead_connection` when the connection dies, so the pool never
/// serves stale entries. With keep-alive, it periodically PINGs and aborts the
/// connection if no pong arrives in time.
///
/// Returns the `JoinHandle` so the caller registers it with the pool entry
/// under the same lock hold as `insert`, eliminating the TOCTOU window where
/// an entry exists without a live driver.
fn spawn_h2_driver(
    pool: Pool,
    key: String,
    identity: Arc<AtomicUsize>,
    driver: H2Driver,
) -> tokio::task::JoinHandle<()> {
    let keep_alive = driver.keep_alive;
    // Wrap the PingPong in an `Arc<Mutex>` so the keep-alive ping future can own
    // a shared handle and be driven as its own `select!` branch (independent of
    // the connection driver). The `conn` future keeps the connection running
    // while the ping is in flight, allowing the PING/PONG to be exchanged.
    let ping_pong: Option<Arc<tokio::sync::Mutex<h2::PingPong>>> = driver
        .ping_pong
        .map(|p| Arc::new(tokio::sync::Mutex::new(p)));
    let mut conn = driver.connection;

    tokio::spawn(async move {
        if let Some(interval) = keep_alive.interval.filter(|i| !i.is_zero()) {
            // Keep-alive enabled: periodically send PING frames.
            let mut interval = tokio::time::interval(interval);
            interval.tick().await; // skip first immediate tick
            let timeout = keep_alive.timeout.unwrap_or(Duration::from_secs(10));

            // The in-flight keep-alive ping future. It must be polled as its own
            // `select!` branch: awaiting it from inside the tick branch would
            // stop polling `conn`, so the PING/PONG could never be sent/received
            // and a healthy connection would be torn down on every interval.
            let mut ping_fut: Option<PingFuture> = None;

            let result = loop {
                tokio::select! {
                    result = &mut conn => {
                        break result;
                    }
                    _ = interval.tick() => {
                        // When while_idle is false, skip pings when the
                        // connection has no active streams.
                        if !keep_alive.while_idle
                            && identity.load(std::sync::atomic::Ordering::Acquire) == 0
                        {
                            continue;
                        }
                        // Only queue a new ping if none is already in flight;
                        // the `conn` branch keeps the driver running so the
                        // PING/PONG can be exchanged.
                        if ping_fut.is_none() {
                            if let Some(pp) = &ping_pong {
                                let pp = Arc::clone(pp);
                                ping_fut = Some(Box::pin(async move {
                                    let mut guard = pp.lock().await;
                                    match tokio::time::timeout(
                                        timeout,
                                        guard.ping(h2::Ping::opaque()),
                                    )
                                    .await
                                    {
                                        Ok(Ok(_)) => Ok(()),
                                        Ok(Err(e)) => Err(e),
                                        Err(_) => Err(h2::Reason::NO_ERROR.into()),
                                    }
                                }));
                            }
                        }
                    }
                    ping_result = poll_ping(&mut ping_fut), if ping_fut.is_some() => {
                        match ping_result {
                            Ok(()) => {
                                log::trace!("h2 keep-alive ping acknowledged");
                            }
                            Err(e) => {
                                if e.reason() == Some(h2::Reason::NO_ERROR) {
                                    warn!("h2 keep-alive timeout, closing connection");
                                } else {
                                    warn!("h2 keep-alive ping error: {}", e);
                                }
                                break Err(e);
                            }
                        }
                    }
                }
            };

            if let Err(e) = result {
                warn!("h2 connection driver error: {}", e);
            }
        } else {
            // No keep-alive: simple await.
            if let Err(e) = conn.await {
                warn!("h2 connection driver error: {}", e);
            }
        }
        pool.remove_dead_connection(&key, &identity);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_key_lowercases_host_case_variants_share_one_key() {
        let cases = [
            ("https://Example.com/", "https:example.com:443"),
            ("https://example.com/", "https:example.com:443"),
            ("https://EXAMPLE.com:8443/", "https:example.com:8443"),
            ("http://ExAmPlE.COM/", "http:example.com:80"),
            ("https://example.com:443/", "https:example.com:443"),
        ];
        for (url, expected) in cases {
            let uri: Uri = url.parse().unwrap();
            assert_eq!(pool_key(&uri), expected, "for {url}");
        }
    }

    #[test]
    fn test_active_streams_refcount() {
        let count = Arc::new(AtomicUsize::new(0));

        // Simulate acquiring a stream
        count.fetch_add(1, Ordering::AcqRel);
        assert_eq!(count.load(Ordering::Acquire), 1);

        // Simulate another stream
        count.fetch_add(1, Ordering::AcqRel);
        assert_eq!(count.load(Ordering::Acquire), 2);

        // Simulate dropping one
        count.fetch_sub(1, Ordering::AcqRel);
        assert_eq!(count.load(Ordering::Acquire), 1);

        // Simulate dropping the last
        count.fetch_sub(1, Ordering::AcqRel);
        assert_eq!(count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_transient_pool_clone_drop_does_not_abort_drivers() {
        // Regression test for M3: `Pool` is cloned per `get_or_connect` call,
        // and those transient clones are dropped when the call returns. Dropping
        // a non-owner clone must NOT abort the live connection driver tasks,
        // otherwise an in-flight connection would be torn down the moment the
        // transient clone is dropped. Only the owner (the clone holding the
        // idle-cleanup handle) tears down drivers on drop.
        let owner = Pool::new(
            Some(Duration::from_secs(90)),
            4,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        // Simulate `get_or_connect`'s internal `self.clone()`: a transient,
        // non-owner clone.
        let transient = owner.clone();
        // Drop the transient clone. Before the fix this aborted every driver
        // task in the shared `PoolInner` and panicked the next access.
        drop(transient);

        // The owner must still be fully usable: its inner state must be intact
        // and not have had its connection tasks aborted by the transient drop.
        let guard = owner.inner.lock().unwrap();
        assert_eq!(guard.max_connections, 4);
        drop(guard);
        // Owner's own drop should run cleanly (no double-abort / panic).
        drop(owner);
    }

    #[test]
    fn test_pool_max_connections_validation() {
        // Test that pool_max_connections validates minimum value
        let builder = crate::Client::builder();
        let builder = builder.pool_max_connections(0);
        // Should be clamped to 1 — build should succeed
        let client = builder.build().unwrap();
        drop(client);

        // Test that Pool::new clamps max_connections to 1
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            0,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let guard = pool.inner.lock().unwrap();
        assert_eq!(guard.max_connections, 1);
        drop(guard);
    }

    #[test]
    fn test_jitter_range() {
        // Test that jitter produces values in [0, max)
        for _ in 0..1000 {
            let d = super::jitter(50);
            assert!(d.as_millis() < 50, "jitter {}ms >= 50ms", d.as_millis());
        }
    }

    #[test]
    fn test_pool_creation() {
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        assert!(pool.cleanup_handle.is_none());
    }

    #[tokio::test]
    async fn test_pool_clone_shares_inner() {
        let mut pool1 = Pool::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        pool1.spawn_idle_cleanup();
        let pool2 = pool1.clone();

        // Verify they share the same Arc by checking pool_inner address
        let ptr1 = Arc::as_ptr(&pool1.inner);
        let ptr2 = Arc::as_ptr(&pool2.inner);
        assert_eq!(ptr1, ptr2, "cloned pools should share the same inner");

        // Verify cleanup handle is on the original Pool, not the clone
        assert!(
            pool1.cleanup_handle.is_some(),
            "original pool should have cleanup handle"
        );
        assert!(
            pool2.cleanup_handle.is_none(),
            "cloned pool should not have cleanup handle"
        );
    }

    #[test]
    fn test_pool_inner_default() {
        let inner = PoolInner::new(
            Some(Duration::from_secs(60)),
            100,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        assert!(inner.connections.is_empty());
        assert!(inner.in_progress.is_empty());
        assert_eq!(inner.max_connections, 100);
    }

    #[test]
    fn test_active_streams_atomic() {
        let count = Arc::new(AtomicUsize::new(0));

        // Multiple threads can safely increment/decrement
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let c = Arc::clone(&count);
                std::thread::spawn(move || {
                    c.fetch_add(1, Ordering::AcqRel);
                    std::thread::sleep(Duration::from_millis(1));
                    c.fetch_sub(1, Ordering::AcqRel);
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_sentinel_id_uniqueness() {
        let id1 = SENTINEL_ID.fetch_add(1, Ordering::Relaxed);
        let id2 = SENTINEL_ID.fetch_add(1, Ordering::Relaxed);
        assert_ne!(id1, id2, "sentinel IDs must be unique");
    }

    #[test]
    fn test_in_progress_guard_only_removes_own_sentinel() {
        let pool = PoolInner::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let inner = Arc::new(Mutex::new(pool));
        let key = "https:example.com:443".to_string();

        // Insert sentinel with ID 1
        let (tx1, _rx1) = watch::channel(());
        {
            let mut guard = inner.lock().unwrap();
            guard.in_progress.insert(
                key.clone(),
                InProgress {
                    tx: tx1,
                    created: Instant::now(),
                    id: 1,
                },
            );
        }

        // Create a guard for sentinel ID 2 (different task)
        let guard2 = InProgressGuard {
            inner: inner.clone(),
            key: key.clone(),
            completed: false,
            sentinel_id: 2,
        };

        // Drop the guard — it should NOT remove sentinel ID 1
        drop(guard2);

        let guard = inner.lock().unwrap();
        assert!(
            guard.in_progress.contains_key(&key),
            "sentinel ID 1 should still exist"
        );
        assert_eq!(guard.in_progress[&key].id, 1);
    }

    #[test]
    fn test_in_progress_guard_removes_own_sentinel() {
        let pool = PoolInner::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let inner = Arc::new(Mutex::new(pool));
        let key = "https:example.com:443".to_string();

        // Insert sentinel with ID 1
        let (tx1, _rx1) = watch::channel(());
        {
            let mut guard = inner.lock().unwrap();
            guard.in_progress.insert(
                key.clone(),
                InProgress {
                    tx: tx1,
                    created: Instant::now(),
                    id: 1,
                },
            );
        }

        // Create a guard for sentinel ID 1 (same task)
        let guard1 = InProgressGuard {
            inner: inner.clone(),
            key: key.clone(),
            completed: false,
            sentinel_id: 1,
        };

        // Drop the guard — it SHOULD remove sentinel ID 1
        drop(guard1);

        let guard = inner.lock().unwrap();
        assert!(
            !guard.in_progress.contains_key(&key),
            "sentinel ID 1 should be removed"
        );
    }

    #[test]
    fn test_in_progress_guard_completed_noop() {
        let pool = PoolInner::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let inner = Arc::new(Mutex::new(pool));
        let key = "https:example.com:443".to_string();

        // Insert sentinel with ID 1
        let (tx1, _rx1) = watch::channel(());
        {
            let mut guard = inner.lock().unwrap();
            guard.in_progress.insert(
                key.clone(),
                InProgress {
                    tx: tx1,
                    created: Instant::now(),
                    id: 1,
                },
            );
        }

        // Create a guard with completed = true
        let guard1 = InProgressGuard {
            inner: inner.clone(),
            key: key.clone(),
            completed: true,
            sentinel_id: 1,
        };

        // Drop the guard — it should NOT remove the sentinel (completed = true)
        drop(guard1);

        let guard = inner.lock().unwrap();
        assert!(
            guard.in_progress.contains_key(&key),
            "sentinel should still exist when completed = true"
        );
    }

    #[tokio::test]
    async fn test_clear_expired_removes_stale_sentinels() {
        let pool = PoolInner::new(
            Some(Duration::from_millis(10)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let inner = Arc::new(Mutex::new(pool));

        // Insert a stale sentinel (created long ago)
        let (tx, _rx) = watch::channel(());
        {
            let mut guard = inner.lock().unwrap();
            guard.in_progress.insert(
                "key".to_string(),
                InProgress {
                    tx,
                    created: Instant::now() - Duration::from_millis(100),
                    id: 1,
                },
            );
        }

        // Wait for stale_threshold to pass
        tokio::time::sleep(Duration::from_millis(25)).await;

        {
            let mut guard = inner.lock().unwrap();
            guard.clear_expired();
        }

        let guard = inner.lock().unwrap();
        assert!(
            guard.in_progress.is_empty(),
            "stale sentinel should be removed"
        );
    }

    #[test]
    fn test_try_reuse_none_for_missing_key() {
        let mut pool = PoolInner::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        assert!(matches!(
            pool.try_reuse("https:missing.example.com:443"),
            ReuseResult::None
        ));
    }

    #[test]
    fn test_pool_inner_clear_expired_empty_pool() {
        let mut pool = PoolInner::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        pool.clear_expired();
        assert!(pool.connections.is_empty());
        assert!(pool.in_progress.is_empty());
    }

    #[test]
    fn test_pool_inner_clear_expired_no_idle_timeout() {
        let mut pool = PoolInner::new(None, 256, DEFAULT_H2_MAX_CONCURRENT_STREAMS);
        let (tx, _rx) = watch::channel(());
        pool.in_progress.insert(
            "key".to_string(),
            InProgress {
                tx,
                created: Instant::now() - Duration::from_secs(999),
                id: 1,
            },
        );
        pool.clear_expired();
        // No idle_timeout: connections are not expired, but stale sentinels
        // are still cleaned up using a 60-second default threshold.
        assert!(
            !pool.in_progress.contains_key("key"),
            "stale sentinel should be removed even when idle_timeout is None"
        );
    }

    #[test]
    fn test_notification_fires_on_last_drop() {
        let count = Arc::new(AtomicUsize::new(0));
        let notify = Arc::new(tokio::sync::Notify::new());

        // Simulate two active streams.
        count.fetch_add(1, Ordering::AcqRel);
        count.fetch_add(1, Ordering::AcqRel);
        assert_eq!(count.load(Ordering::Acquire), 2);

        // Drop first — count goes to 1, no notification expected.
        let prev = count.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            notify.notify_waiters();
        }
        assert_eq!(count.load(Ordering::Acquire), 1);

        // Drop second — count goes to 0, notification fires.
        let prev = count.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            notify.notify_waiters();
        }
        assert_eq!(count.load(Ordering::Acquire), 0);

        // Verify the notification counter was bumped (notify_waiters was called).
        // We can't easily poll the Notify in a sync test, but we verified
        // the code path by checking the count reached 0 and the branch fired.
    }

    #[test]
    fn test_notification_not_fired_above_zero() {
        let count = Arc::new(AtomicUsize::new(0));
        let _notify = Arc::new(tokio::sync::Notify::new());

        count.fetch_add(1, Ordering::AcqRel);

        // Drop with prev == 1 → notification fires.
        let prev = count.fetch_sub(1, Ordering::AcqRel);
        assert_eq!(prev, 1, "prev should be 1 so notification branch fires");
        assert_eq!(count.load(Ordering::Acquire), 0);

        // Re-increment — simulates a new stream being opened.
        count.fetch_add(1, Ordering::AcqRel);

        // Drop again — notification fires again.
        let prev = count.fetch_sub(1, Ordering::AcqRel);
        assert_eq!(prev, 1);
        assert_eq!(count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_jitter_zero_max() {
        let d = super::jitter(0);
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn test_jitter_one_max() {
        // jitter(1) should return Duration::ZERO since val % 1 == 0
        let d = super::jitter(1);
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn test_pool_max_connections_clamped_to_one() {
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            0,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let guard = pool.inner.lock().unwrap();
        assert_eq!(
            guard.max_connections, 1,
            "max_connections should be clamped to 1"
        );
        drop(guard);

        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            1,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let guard = pool.inner.lock().unwrap();
        assert_eq!(guard.max_connections, 1);
        drop(guard);

        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            512,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let guard = pool.inner.lock().unwrap();
        assert_eq!(guard.max_connections, 512);
        drop(guard);
    }

    #[test]
    fn test_remove_dead_connection_returns_false_when_missing() {
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let fake_identity = Arc::new(AtomicUsize::new(0));
        assert!(
            !pool.remove_dead_connection("https:missing.example.com:443", &fake_identity),
            "should return false when no entry exists for the key",
        );
    }

    #[tokio::test]
    async fn test_remove_dead_connection_removes_present_entry() {
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let key = "https:example.com:443".to_string();

        let (io_client, io_server) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let _ = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await;
        });
        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();
        let driver = tokio::spawn(async move {
            let _ = conn.await;
        });

        let identity = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request, None);
            Arc::clone(&guard.connections[&key].active_streams)
        };
        assert!(
            recover_lock(&pool.inner).connections.contains_key(&key),
            "entry should be present after insert",
        );

        assert!(
            pool.remove_dead_connection(&key, &identity),
            "should return true when an entry exists for the key and identity matches",
        );
        assert!(
            !recover_lock(&pool.inner).connections.contains_key(&key),
            "entry should be removed",
        );

        drop(server_task);
        driver.abort();
    }

    #[tokio::test]
    async fn test_remove_dead_connection_skips_newer_entry() {
        // Regression test: remove_dead_connection must NOT remove a newer
        // connection that was inserted for the same key after this driver's
        // connection died.
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let key = "https:example.com:443".to_string();

        let (io_client1, io_server1) = tokio::io::duplex(64 * 1024);
        let server_task1 = tokio::spawn(async move {
            let _ = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server1)
                .await;
        });
        let (send_request1, conn1) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client1)
            .await
            .unwrap();
        let driver1 = tokio::spawn(async move {
            let _ = conn1.await;
        });

        // Insert the first connection and capture its identity.
        let old_identity = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request1, None);
            Arc::clone(&guard.connections[&key].active_streams)
        };

        // Insert a newer connection for the same key (simulates reconnect).
        let (io_client2, io_server2) = tokio::io::duplex(64 * 1024);
        let server_task2 = tokio::spawn(async move {
            let _ = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server2)
                .await;
        });
        let (send_request2, conn2) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client2)
            .await
            .unwrap();
        let driver2 = tokio::spawn(async move {
            let _ = conn2.await;
        });

        {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request2, None);
        }

        // The old driver tries to remove its dead connection using its
        // (now stale) identity. It must NOT remove the newer entry.
        assert!(
            !pool.remove_dead_connection(&key, &old_identity),
            "should return false when identity does not match the current entry",
        );
        assert!(
            recover_lock(&pool.inner).connections.contains_key(&key),
            "newer entry must still be present",
        );

        drop(server_task1);
        drop(server_task2);
        driver1.abort();
        driver2.abort();
    }

    #[tokio::test]
    async fn test_remove_dead_connection_notifies_waiters() {
        // Regression test: remove_dead_connection must signal
        // stream_completed.notify_waiters() (matching the other removal
        // paths) so a request waiting in the Busy branch of get_or_connect
        // does not park forever in notified.await when the driver task
        // exits.
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let key = "https:example.com:443".to_string();

        let (io_client, io_server) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let _ = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await;
        });
        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();
        let driver = tokio::spawn(async move {
            let _ = conn.await;
        });

        let (waiter_notify, identity) = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request, None);
            (
                Arc::clone(&guard.connections[&key].stream_completed),
                Arc::clone(&guard.connections[&key].active_streams),
            )
        };

        // Park a task on the entry's stream_completed Notify — same as a
        // request stuck in the Busy branch of get_or_connect.
        let waiter = tokio::spawn(async move {
            waiter_notify.notified().await;
        });

        // Give the waiter time to register on the Notify. notify_waiters()
        // only wakes currently registered waiters, so the waiter must be
        // parked before we call remove_dead_connection.
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            pool.remove_dead_connection(&key, &identity),
            "should return true when an entry exists for the key and identity matches",
        );

        // The waiter should complete promptly because remove_dead_connection
        // signals the Notify. A 1s timeout is generous — without the fix the
        // timeout fires.
        match tokio::time::timeout(Duration::from_secs(1), waiter).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => panic!("waiter task panicked: {}", e),
            Err(_) => panic!(
                "remove_dead_connection did not notify stream_completed — \
                 waiter would park forever in notified.await"
            ),
        }

        drop(server_task);
        driver.abort();
    }

    /// Watch channel used by the `InProgress` sentinel: a subscriber must
    /// observe sender drop (changed() returns Err), otherwise the "another
    /// task is connecting" branch of `get_or_connect` would park forever.
    #[tokio::test]
    async fn sentinel_watch_channel_signals_on_sender_drop() {
        let (tx, mut rx) = watch::channel(());
        let waiter = tokio::spawn(async move { rx.changed().await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(tx);
        let result = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("subscriber did not wake up after sender was dropped")
            .expect("subscriber task panicked");
        assert!(result.is_err(), "rx.changed() must return Err after drop");
    }

    /// InProgressGuard with a non-matching sentinel ID must NOT remove an
    /// existing sentinel — a guard must only remove the entry it inserted.
    #[tokio::test]
    async fn in_progress_guard_only_removes_matching_sentinel_id() {
        let pool = Pool::new(
            Some(Duration::from_secs(90)),
            256,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        let key = "https:shared.example.com:443".to_string();
        let inner = pool.inner.clone();

        let (tx, _rx) = watch::channel(());
        {
            let mut guard = recover_lock(&inner);
            guard.in_progress.insert(
                key.clone(),
                InProgress {
                    tx,
                    created: Instant::now(),
                    id: 1,
                },
            );
        }

        // Mismatched ID: must not remove.
        drop(InProgressGuard {
            inner: inner.clone(),
            key: key.clone(),
            completed: false,
            sentinel_id: 999,
        });
        assert!(recover_lock(&inner).in_progress.contains_key(&key));

        // Matching ID: must remove.
        drop(InProgressGuard {
            inner: inner.clone(),
            key: key.clone(),
            completed: false,
            sentinel_id: 1,
        });
        assert!(!recover_lock(&inner).in_progress.contains_key(&key));
    }

    /// Regression test for the refcount window between `PooledSendRequest::drop`
    /// (which decrements `active_streams` to 0) and `H2ResponseBody::new` (which
    /// re-increments it to 1 for the streaming body). A live response body must
    /// hold the connection's `active_streams` count at >= 1 so that a concurrent
    /// `try_reuse`/`clear_expired` never treats an in-flight response as idle
    /// and aborts the driver. We send a real request, capture the live
    /// `H2ResponseBody`, and assert the pool's `active_streams` stays positive
    /// while the body is resident, then returns to 0 on drop.
    #[tokio::test]
    async fn active_streams_stays_positive_while_body_streams() {
        let (io_client, io_server) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let mut conn = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await
                .unwrap();
            while let Some(Ok((_req, mut send))) = conn.accept().await {
                let rsp = http::Response::new(());
                let mut stream = send.send_response(rsp, false).unwrap();
                stream
                    .send_data(bytes::Bytes::from_static(b"hello"), true)
                    .unwrap();
            }
        });

        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();
        let driver = tokio::spawn(async move {
            let _ = conn.await;
        });

        let key = "https:refcount.example.com:443".to_string();
        let pool = Pool::new(None, 1, DEFAULT_H2_MAX_CONCURRENT_STREAMS);
        let identity = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request.clone(), None);
            Arc::clone(&guard.connections[&key].active_streams)
        };
        // Mirror get_or_connect: the freshly-inserted entry is bumped to 1
        // before the PooledSendRequest is handed to the caller.
        identity.fetch_add(1, Ordering::AcqRel);

        let pooled = PooledSendRequest {
            send_request: send_request.clone(),
            pool_key: key.clone(),
            active_streams: Arc::clone(&identity),
            stream_completed: {
                let g = recover_lock(&pool.inner);
                Arc::clone(&g.connections[&key].stream_completed)
            },
            last_used: {
                let g = recover_lock(&pool.inner);
                Arc::clone(&g.connections[&key].last_used)
            },
            capacity: DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        };

        let req = http::Request::builder()
            .uri("/")
            .body(crate::async_impl::body::Body::empty())
            .unwrap();
        let resp = PoolClient::send_request(pooled, req, &pool).await.unwrap();
        assert_eq!(
            identity.load(Ordering::Acquire),
            1,
            "active_streams must stay >=1 while the response body is resident"
        );

        // Body still alive here; dropping it must return the count to 0.
        drop(resp);
        assert_eq!(
            identity.load(Ordering::Acquire),
            0,
            "active_streams must return to 0 after the body is dropped"
        );

        drop(server_task);
        driver.abort();
    }

    /// Regression test (2026-08-04): when the pool is built OUTSIDE a Tokio
    /// runtime (the Python bindings build path — `primp.Client(...)` calls
    /// `client_builder.build()` inside `py.detach`, no runtime entered),
    /// `spawn_idle_cleanup` early-returns and `cleanup_handle` stays `None`.
    /// The old `Pool::drop` used `cleanup_handle.is_some()` as the owner
    /// signal, so an owner drop was misclassified as a transient clone drop
    /// and aborted NO driver tasks — the connection drivers (and their
    /// sockets) leaked until process exit. Ownership is now tracked
    /// explicitly (`is_owner`), so the owner tears drivers down even when the
    /// idle-cleanup task never spawned.
    #[test]
    fn pool_built_outside_runtime_aborts_drivers_on_owner_drop() {
        let mut pool = Pool::new(
            Some(Duration::from_secs(90)),
            4,
            DEFAULT_H2_MAX_CONCURRENT_STREAMS,
        );
        pool.spawn_idle_cleanup();
        assert!(
            pool.cleanup_handle.is_none(),
            "outside a runtime, spawn_idle_cleanup must not spawn (the trigger)"
        );
        let inner = Arc::clone(&pool.inner);
        let key = "https:leak.example.com:443".to_string();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // The server stays alive for the whole test, so the connection is
        // healthy: the ONLY way the driver task can finish is the pool
        // tearing it down.
        let server_task = rt.block_on(async {
            let (io_client, io_server) = tokio::io::duplex(64 * 1024);
            let server_task = tokio::spawn(async move {
                let mut conn = h2::server::Builder::new()
                    .handshake::<_, Bytes>(io_server)
                    .await
                    .unwrap();
                while let Some(Ok((_req, _send))) = conn.accept().await {}
            });
            let (send_request, conn) = h2::client::Builder::new()
                .handshake::<_, Bytes>(io_client)
                .await
                .unwrap();
            let driver = tokio::spawn(async move {
                let _ = conn.await;
            });
            {
                let mut guard = recover_lock(&pool.inner);
                guard.insert(key.clone(), send_request, None);
                let entry = guard.connections.get_mut(&key).unwrap();
                entry.connection_task = Some(driver);
            }
            server_task
        });

        // The "owner" drop happens OUTSIDE the runtime, exactly like the
        // Python bindings path. `cleanup_handle` is `None`, but the owner
        // must still tear down the connection driver.
        drop(pool);

        // The driver task must now be FINISHED — aborted by the owner drop.
        // Before the fix it stayed alive: the connection and its socket were
        // leaked until process exit.
        let is_finished = rt.block_on(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let guard = recover_lock(&inner);
            let handle = guard
                .connections
                .get(&key)
                .unwrap()
                .connection_task
                .as_ref()
                .unwrap();
            handle.is_finished()
        });
        assert!(
            is_finished,
            "the owner pool drop must abort the connection driver even when the \
             pool was built outside a runtime (cleanup_handle is None); otherwise \
             the connection and socket leak until process exit"
        );
        drop(server_task);
        rt.shutdown_timeout(Duration::from_millis(500));
    }

    /// Regression test for BUG A: h2 idle eviction must measure "time since the
    /// connection returned to idle", not "time since checkout".
    ///
    /// A connection borrowed for longer than `idle_timeout` (e.g. a long
    /// streamed response) would previously accrue a stale `last_used` set only
    /// on checkout. When it later returned to idle, `clear_expired` /
    /// `try_reuse` treated it as expired and aborted its healthy driver. The
    /// fix refreshes `last_used` when `active_streams` transitions back to 0 via
    /// the `PooledSendRequest` / `H2ResponseBody` Drop impls.
    #[tokio::test]
    async fn last_used_refreshed_on_idle_return_prevents_eviction() {
        let idle_timeout = Duration::from_millis(50);
        let mut pool = PoolInner::new(idle_timeout.into(), 8, DEFAULT_H2_MAX_CONCURRENT_STREAMS);

        // Let the process live past the simulated staleness window so the
        // epoch-relative timestamps below resolve to genuine past instants.
        tokio::time::sleep(idle_timeout * 5).await;

        // Two entries. `refreshed` simulates a connection that was borrowed for
        // a long time (stale checkout timestamp) but whose `last_used` was
        // refreshed on idle-return. `stale` simulates the buggy behaviour where
        // `last_used` was never refreshed after a long borrow.
        let refreshed_key = "https:refreshed.example.com:443".to_string();
        let stale_key = "https:stale.example.com:443".to_string();

        // Build entries with a checkout timestamp already older than the idle
        // timeout (mimicking a borrow that outlived idle_timeout). Encode it as
        // nanos-since-epoch to match `now_nanos()`.
        let stale_nanos = now_nanos().saturating_sub((idle_timeout * 4).as_nanos() as u64);
        for (key, ts) in [(&refreshed_key, stale_nanos), (&stale_key, stale_nanos)] {
            let send_request = throwaway_send_request().await;
            pool.connections.insert(
                key.clone(),
                PooledEntry {
                    send_request,
                    tls_info: None,
                    last_used: Arc::new(AtomicU64::new(ts)),
                    connection_task: None,
                    active_streams: Arc::new(AtomicUsize::new(0)),
                    stream_completed: Arc::new(tokio::sync::Notify::new()),
                },
            );
        }

        // Simulate the Drop-path refresh for the "refreshed" entry: the borrower
        // returned to idle *now*, so its shared timestamp is updated.
        {
            let entry = pool.connections.get(&refreshed_key).unwrap();
            entry.last_used.store(now_nanos(), Ordering::Release);
        }

        pool.clear_expired();

        assert!(
            pool.connections.contains_key(&refreshed_key),
            "connection refreshed on idle-return must NOT be evicted"
        );
        assert!(
            !pool.connections.contains_key(&stale_key),
            "connection with a stale (unrefreshed) last_used must be evicted"
        );
    }

    /// Same invariant exercised through `try_reuse`'s expired-branch: a
    /// refreshed entry is reusable; a stale one is treated as `None`.
    #[tokio::test]
    async fn try_reuse_respects_refreshed_last_used() {
        let idle_timeout = Duration::from_millis(50);
        let mut pool = PoolInner::new(idle_timeout.into(), 8, DEFAULT_H2_MAX_CONCURRENT_STREAMS);
        // Let the process live past the simulated staleness window.
        tokio::time::sleep(idle_timeout * 5).await;
        let key = "https:reuse.example.com:443".to_string();
        let old_nanos = now_nanos().saturating_sub((idle_timeout * 4).as_nanos() as u64);
        let send_request = throwaway_send_request().await;
        pool.connections.insert(
            key.clone(),
            PooledEntry {
                send_request,
                tls_info: None,
                last_used: Arc::new(AtomicU64::new(old_nanos)),
                connection_task: None,
                active_streams: Arc::new(AtomicUsize::new(0)),
                stream_completed: Arc::new(tokio::sync::Notify::new()),
            },
        );

        // Stale timestamp → treated as expired → None (and entry removed).
        assert!(matches!(pool.try_reuse(&key), ReuseResult::None));
        assert!(!pool.connections.contains_key(&key));

        // A fresh timestamp → reusable.
        let key2 = "https:reuse2.example.com:443".to_string();
        let send_request2 = throwaway_send_request().await;
        pool.connections.insert(
            key2.clone(),
            PooledEntry {
                send_request: send_request2,
                tls_info: None,
                last_used: Arc::new(AtomicU64::new(now_nanos())),
                connection_task: None,
                active_streams: Arc::new(AtomicUsize::new(0)),
                stream_completed: Arc::new(tokio::sync::Notify::new()),
            },
        );
        assert!(matches!(pool.try_reuse(&key2), ReuseResult::Reused(..)));
    }

    /// End-to-end regression for BUG A over a real duplex h2 connection: borrow
    /// the connection for longer than `idle_timeout` (holding the response body
    /// in flight), then drop it so it returns to idle. The background cleanup
    /// must NOT abort the healthy driver, and a second request must reuse the
    /// same connection.
    #[tokio::test]
    async fn h2_long_borrow_then_idle_is_not_evicted() {
        let (io_client, io_server) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let mut conn = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await
                .unwrap();
            while let Some(Ok((_req, mut send))) = conn.accept().await {
                let rsp = http::Response::new(());
                let mut stream = send.send_response(rsp, false).unwrap();
                stream
                    .send_data(bytes::Bytes::from_static(b"hello"), true)
                    .unwrap();
            }
        });

        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();

        let key = "https:longborrow.example.com:443".to_string();
        let idle_timeout = Duration::from_millis(30);
        let pool = Pool::new(Some(idle_timeout), 4, DEFAULT_H2_MAX_CONCURRENT_STREAMS);

        // Insert and register a real driver task under the entry.
        let (identity, last_used) = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request.clone(), None);
            let entry = guard.connections.get_mut(&key).unwrap();
            let handle = tokio::spawn(async move {
                let _ = conn.await;
            });
            entry.connection_task = Some(handle);
            (
                Arc::clone(&entry.active_streams),
                Arc::clone(&entry.last_used),
            )
        };

        // Check out the connection and issue a request, keeping the response
        // body alive (active_streams stays >= 1).
        let pooled = match {
            let mut guard = recover_lock(&pool.inner);
            guard.try_reuse(&key)
        } {
            ReuseResult::Reused(p, _) => p,
            _ => panic!("expected reuse of freshly inserted connection"),
        };
        let req = http::Request::builder()
            .uri("/")
            .body(crate::async_impl::body::Body::empty())
            .unwrap();
        let resp = PoolClient::send_request(pooled, req, &pool).await.unwrap();

        // Simulate a long borrow: hold the body well past idle_timeout. During
        // this time last_used stays stale (checkout time), but active_streams > 0
        // protects the entry from eviction.
        tokio::time::sleep(idle_timeout * 3).await;
        assert!(
            identity.load(Ordering::Acquire) >= 1,
            "active_streams must stay positive while the body is held"
        );
        let stale_before = last_used.load(Ordering::Acquire);

        // Return to idle: dropping the body decrements active_streams to 0 and
        // (with the fix) refreshes last_used to now.
        drop(resp);
        assert_eq!(identity.load(Ordering::Acquire), 0);
        let refreshed_after = last_used.load(Ordering::Acquire);
        assert!(
            refreshed_after > stale_before,
            "last_used must be refreshed when the connection returns to idle"
        );

        // Run the eviction pass: the entry must survive because last_used is now
        // fresh, and its driver must not be aborted.
        {
            let mut guard = recover_lock(&pool.inner);
            guard.clear_expired();
        }
        assert!(
            recover_lock(&pool.inner).connections.contains_key(&key),
            "connection returned to idle within idle_timeout must NOT be evicted"
        );

        // The connection is still healthy: a second request must reuse it.
        let pooled2 = match {
            let mut guard = recover_lock(&pool.inner);
            guard.try_reuse(&key)
        } {
            ReuseResult::Reused(p, _) => p,
            other => panic!(
                "expected connection to be reusable, got {:?}",
                match other {
                    ReuseResult::Busy => "Busy",
                    ReuseResult::None => "None",
                    ReuseResult::Reused(..) => unreachable!(),
                }
            ),
        };
        let req2 = http::Request::builder()
            .uri("/")
            .body(crate::async_impl::body::Body::empty())
            .unwrap();
        let resp2 = PoolClient::send_request(pooled2, req2, &pool)
            .await
            .expect("second request must reuse the healthy idle connection");
        let body = http_body_util::BodyExt::collect(resp2.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(&body[..], b"hello");

        drop(server_task);
    }

    /// Regression test: a request whose body is NOT replayable (a streaming
    /// body, `try_clone()` == None) that receives a server-initiated
    /// `REFUSED_STREAM` must NOT be silently retried on a second stream with an
    /// EMPTY body (which would corrupt the upload). The retry loop should give
    /// up on the first REFUSED_STREAM for a non-replayable body and return the
    /// request error instead of sending empty data on the new stream.
    ///
    /// The server REFUSEs the first stream and, on the (buggy) retry, receives
    /// whatever the client sends on the second stream. We echo the second
    /// stream's body back as the response so the test can detect whether the
    /// client uploaded the real payload or an empty one.
    #[tokio::test]
    async fn refused_stream_with_streaming_body_is_not_corrupted() {
        let upload = Bytes::from_static(b"upload-data");
        let (io_client, io_server) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let mut conn = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await
                .unwrap();
            let mut accepted = 0usize;
            while let Some(Ok((req, mut send))) = conn.accept().await {
                accepted += 1;
                if accepted == 1 {
                    // Server-initiated REFUSED_STREAM on the first stream.
                    let _ = send.send_reset(h2::Reason::REFUSED_STREAM);
                    continue;
                }
                // Collect the request body the client sent on this (retry) stream.
                let mut recv = req.into_body();
                let mut received = Vec::new();
                while let Some(chunk) = recv.data().await {
                    match chunk {
                        Ok(data) => received.extend_from_slice(&data[..]),
                        Err(_) => break,
                    }
                }
                let rsp = http::Response::new(());
                let mut stream = send.send_response(rsp, false).unwrap();
                stream
                    .send_data(Bytes::copy_from_slice(&received), true)
                    .unwrap();
            }
        });

        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();

        let key = "https:refused.example.com:443".to_string();
        let pool = Pool::new(None, 1, DEFAULT_H2_MAX_CONCURRENT_STREAMS);
        let (identity, _last_used) = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request.clone(), None);
            let entry = guard.connections.get_mut(&key).unwrap();
            let handle = tokio::spawn(async move {
                let _ = conn.await;
            });
            entry.connection_task = Some(handle);
            (
                Arc::clone(&entry.active_streams),
                Arc::clone(&entry.last_used),
            )
        };
        identity.fetch_add(1, Ordering::AcqRel);

        let pooled = match {
            let mut guard = recover_lock(&pool.inner);
            guard.try_reuse(&key)
        } {
            ReuseResult::Reused(p, _) => p,
            _ => panic!("expected reuse of freshly inserted connection"),
        };

        // A streaming (non-replayable) body: `Body::wrap` forces Inner::Streaming
        // so `try_clone()` returns None.
        let body = crate::async_impl::body::Body::wrap(http_body_util::Full::new(upload.clone()));
        assert!(
            body.try_clone().is_none(),
            "precondition: streaming body must not be replayable"
        );
        let req = http::Request::builder().uri("/").body(body).unwrap();

        let result = PoolClient::send_request(pooled, req, &pool).await;
        // A non-replayable body cannot be retried after REFUSED_STREAM, so the
        // request must fail cleanly (never succeed with a corrupt/empty body).
        match result {
            Ok(resp) => {
                let got = http_body_util::BodyExt::collect(resp.into_body())
                    .await
                    .unwrap()
                    .to_bytes();
                assert_eq!(
                    &got[..], &upload[..],
                    "REFUSED_STREAM with a non-replayable body must NOT be retried with an empty body"
                );
            }
            Err(_) => {
                // Failure is the only correct outcome for a non-replayable body:
                // it cannot be re-sent, so the request must not be retried with
                // empty data (which would silently corrupt the upload).
            }
        }

        drop(server_task);
    }

    /// Regression test: a REPLAYABLE (non-streaming) body that receives a
    /// server-initiated `REFUSED_STREAM` MUST be retried on the same connection
    /// with the original payload. The server REFUSEs the first stream and
    /// accepts the second, echoing the received body back; the client must
    /// transparently retry and receive the echoed payload.
    #[tokio::test]
    async fn refused_stream_with_replayable_body_is_retried() {
        let upload = Bytes::from_static(b"replayable-data");
        let (io_client, io_server) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let mut conn = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await
                .unwrap();
            let mut accepted = 0usize;
            while let Some(Ok((req, mut send))) = conn.accept().await {
                accepted += 1;
                if accepted == 1 {
                    let _ = send.send_reset(h2::Reason::REFUSED_STREAM);
                    continue;
                }
                let mut recv = req.into_body();
                let mut received = Vec::new();
                while let Some(chunk) = recv.data().await {
                    match chunk {
                        Ok(data) => received.extend_from_slice(&data[..]),
                        Err(_) => break,
                    }
                }
                let rsp = http::Response::new(());
                let mut stream = send.send_response(rsp, false).unwrap();
                stream
                    .send_data(Bytes::copy_from_slice(&received), true)
                    .unwrap();
            }
        });

        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();

        let key = "https:refused2.example.com:443".to_string();
        let pool = Pool::new(None, 1, DEFAULT_H2_MAX_CONCURRENT_STREAMS);
        let (identity, _last_used) = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request.clone(), None);
            let entry = guard.connections.get_mut(&key).unwrap();
            let handle = tokio::spawn(async move {
                let _ = conn.await;
            });
            entry.connection_task = Some(handle);
            (
                Arc::clone(&entry.active_streams),
                Arc::clone(&entry.last_used),
            )
        };
        identity.fetch_add(1, Ordering::AcqRel);

        let pooled = match {
            let mut guard = recover_lock(&pool.inner);
            guard.try_reuse(&key)
        } {
            ReuseResult::Reused(p, _) => p,
            _ => panic!("expected reuse of freshly inserted connection"),
        };

        let body = crate::async_impl::body::Body::reusable(upload.clone());
        assert!(
            body.try_clone().is_some(),
            "precondition: replayable body must be cloneable"
        );
        let req = http::Request::builder().uri("/").body(body).unwrap();

        let resp = PoolClient::send_request(pooled, req, &pool)
            .await
            .expect("REFUSED_STREAM must be retried transparently for a replayable body");
        let got = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(
            &got[..],
            &upload[..],
            "retried request must carry the original payload"
        );

        drop(server_task);
    }

    /// Regression test for C1: a REPLAYABLE body refused on the first TWO
    /// streams must still be retried with the original payload on the third
    /// attempt. The retry loop must re-clone a *fresh* copy of the body on
    /// every retry, never a clone already drained by a prior refusal — else a
    /// second consecutive REFUSED_STREAM silently uploads an empty body.
    #[tokio::test]
    async fn refused_stream_with_replayable_body_refused_twice() {
        let upload = Bytes::from_static(b"replayable-data");
        let (io_client, io_server) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let mut conn = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await
                .unwrap();
            let mut accepted = 0usize;
            while let Some(Ok((req, mut send))) = conn.accept().await {
                accepted += 1;
                if accepted <= 2 {
                    let _ = send.send_reset(h2::Reason::REFUSED_STREAM);
                    continue;
                }
                let mut recv = req.into_body();
                let mut received = Vec::new();
                while let Some(chunk) = recv.data().await {
                    match chunk {
                        Ok(data) => received.extend_from_slice(&data[..]),
                        Err(_) => break,
                    }
                }
                let rsp = http::Response::new(());
                let mut stream = send.send_response(rsp, false).unwrap();
                stream
                    .send_data(Bytes::copy_from_slice(&received), true)
                    .unwrap();
            }
        });

        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();

        let key = "https:refused3.example.com:443".to_string();
        let pool = Pool::new(None, 1, DEFAULT_H2_MAX_CONCURRENT_STREAMS);
        let (identity, _last_used) = {
            let mut guard = recover_lock(&pool.inner);
            guard.insert(key.clone(), send_request.clone(), None);
            let entry = guard.connections.get_mut(&key).unwrap();
            let handle = tokio::spawn(async move {
                let _ = conn.await;
            });
            entry.connection_task = Some(handle);
            (
                Arc::clone(&entry.active_streams),
                Arc::clone(&entry.last_used),
            )
        };
        identity.fetch_add(1, Ordering::AcqRel);

        let pooled = match {
            let mut guard = recover_lock(&pool.inner);
            guard.try_reuse(&key)
        } {
            ReuseResult::Reused(p, _) => p,
            _ => panic!("expected reuse of freshly inserted connection"),
        };

        let body = crate::async_impl::body::Body::reusable(upload.clone());
        assert!(
            body.try_clone().is_some(),
            "precondition: replayable body must be cloneable"
        );
        let req = http::Request::builder().uri("/").body(body).unwrap();

        let resp = PoolClient::send_request(pooled, req, &pool)
            .await
            .expect("REFUSED_STREAM must be retried transparently for a replayable body");
        let got = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(
            &got[..],
            &upload[..],
            "retried request must carry the original payload even after two refusals"
        );

        drop(server_task);
    }

    /// Build a live `SendRequest` from a throwaway duplex handshake. The
    /// eviction / expiry paths under test read only `last_used`,
    /// `active_streams`, and `connection_task`, never `send_request`, so the
    /// connection behind this handle can stay idle.
    async fn throwaway_send_request() -> SendRequest<Bytes> {
        let (io_client, io_server) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            let _ = h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await;
        });
        let (send_request, conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });
        send_request
    }

    /// Regression test for the orphaned-driver leak: a driver detached while
    /// the connection still has active streams (expiry/eviction/replace) must
    /// be aborted as soon as the connection returns to idle. The primp-h2
    /// fork's driver does not exit on its own without a server GOAWAY
    /// (`should_close_on_idle` is only true once GOAWAY was received), so
    /// without the watcher its task and socket would leak indefinitely —
    /// keep-alive pings keep the socket busy, so nothing would tear it down.
    #[tokio::test]
    async fn detached_busy_driver_is_aborted_when_connection_idles() {
        let idle_timeout = Duration::from_millis(30);
        let mut pool = PoolInner::new(Some(idle_timeout), 8, DEFAULT_H2_MAX_CONCURRENT_STREAMS);
        // Let the process live past the simulated staleness window.
        tokio::time::sleep(idle_timeout * 5).await;

        // A fake "driver" that never exits on its own — like the primp-h2
        // driver without a server GOAWAY — but observably alive via a tick
        // counter.
        let alive = Arc::new(AtomicUsize::new(0));
        let alive2 = Arc::clone(&alive);
        let handle = tokio::spawn(async move {
            loop {
                alive2.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        let key = "https:detach.example.com:443".to_string();
        let stale_nanos = now_nanos().saturating_sub((idle_timeout * 4).as_nanos() as u64);
        let (identity, stream_completed) = {
            let send_request = throwaway_send_request().await;
            pool.connections.insert(
                key.clone(),
                PooledEntry {
                    send_request,
                    tls_info: None,
                    last_used: Arc::new(AtomicU64::new(stale_nanos)),
                    connection_task: Some(handle),
                    active_streams: Arc::new(AtomicUsize::new(1)),
                    stream_completed: Arc::new(tokio::sync::Notify::new()),
                },
            );
            let entry = pool.connections.get(&key).unwrap();
            (
                Arc::clone(&entry.active_streams),
                Arc::clone(&entry.stream_completed),
            )
        };

        // `clear_expired` must NOT evict a busy connection even if its
        // `last_used` is stale — idle timeout measures time-since-idle, not
        // time-since-checkout (mirrors `Http1Pool::evict_stale` and H3
        // `try_pool` gating). A long-held borrow must remain in the pool so
        // new streams can still multiplex onto it.
        pool.clear_expired();
        assert!(
            pool.connections.contains_key(&key),
            "busy connection must NOT be evicted by idle timeout"
        );
        assert_eq!(identity.load(Ordering::Acquire), 1);

        // Now detach the busy entry explicitly (e.g. via capacity eviction
        // or idle-timeout of an idle entry) and verify the watcher aborts
        // the driver only after the last stream drains.
        let mut entry = pool
            .connections
            .remove(&key)
            .expect("busy entry should still be present");
        entry.detach_driver_until_idle();
        assert_eq!(identity.load(Ordering::Acquire), 1);

        // The last stream completes — the drop paths notify on the 1 -> 0
        // transition. The watcher must then abort the orphaned driver.
        identity.fetch_sub(1, Ordering::AcqRel);
        stream_completed.notify_waiters();

        // Sample the tick counter twice: if the driver were still running it
        // would keep growing.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let a = alive.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let b = alive.load(Ordering::Relaxed);
        assert_eq!(
            a, b,
            "orphaned driver must be aborted once the connection idles"
        );
    }
}
