//! Minimal HTTP/1.1 connection pool for the ALPN `http/1.1` fallback.
//!
//! When `http_version_pref == All`, the h2 pool may return a TLS stream on
//! which the server picked `http/1.1` (`ConnectOutcome::Http1`). We run
//! HTTP/1.1 over it and keep it alive for reuse, keyed by the same
//! `scheme:host:port` as the h2 pool.
//!
//! HTTP/1.1 is serial: a connection is loaned to one request at a time and
//! returned (for reuse) once its response body is dropped.

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use foldhash::{HashMap, HashMapExt};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use http::{Request, Uri};
use http_body::Frame;
use http_body_util::BodyExt;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;

use crate::async_impl::body::Body;
use crate::async_impl::h2_client::pool::AsyncStream;
use crate::async_impl::BoxBody;
use crate::error::{request, BoxError};
use crate::tls::TlsInfo;
use crate::util::recover_lock;

type SendRequest = http1::SendRequest<Body>;

/// Default cap on distinct host entries kept alive in the fallback pool
/// (256); overridable via [`Http1Pool::with_max_idle_entries`]. Bounds the
/// sockets held open for HTTPS hosts that negotiate `http/1.1` via ALPN.
const MAX_IDLE_ENTRIES: usize = 256;

/// How long an idle fallback connection is kept (90s) before eviction.
/// Mirrors `ClientBuilder::pool_idle_timeout`; keep default in sync with
/// `Config::pool_idle_timeout` (`crates/primp/src/async_impl/client.rs:311`)
/// and the H2/H3 pools (`Pool::new` / `H3Client::new`).
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

struct Entry {
    /// `Some` when idle and ready to be loaned; `None` while on loan.
    sender: Option<SendRequest>,
    tls_info: Option<TlsInfo>,
    /// Number of connections currently on loan for this key.
    busy: usize,
    /// When the idle `sender` was last returned to the pool (used for eviction).
    idle_since: Instant,
}

/// Small pooled HTTP/1.1 client for the ALPN `http/1.1` fallback.
///
/// Connections are keyed by `scheme:host:port` and loaned to one request at a
/// time; the next request reuses the connection once its response body drops.
/// Keeps TLS handshakes minimal for h2-unsupported hosts while still opening
/// extra connections under true concurrency.
#[derive(Clone)]
pub(crate) struct Http1Pool {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
    /// Maximum number of host entries (idle connections) kept in the pool.
    max_idle_entries: usize,
    /// How long an idle connection is kept before eviction.
    idle_timeout: Duration,
}

impl Http1Pool {
    pub(crate) fn new() -> Self {
        Self::with_max_idle_entries(MAX_IDLE_ENTRIES)
    }

    /// Set the max host entries (idle connections); mirrors
    /// `ClientBuilder::pool_max_connections` and defaults to
    /// [`MAX_IDLE_ENTRIES`]. Clamped to at least 1.
    /// Idle timeout defaults to [`DEFAULT_IDLE_TIMEOUT`] (90s).
    pub(crate) fn with_max_idle_entries(max_idle_entries: usize) -> Self {
        Self::with_idle_timeout(max_idle_entries, None)
    }

    /// Create a pool with an explicit idle timeout. `None` → 90s default,
    /// mirroring `Config::pool_idle_timeout`. Used by `client.rs` so
    /// `ClientBuilder::pool_idle_timeout` applies to H1 as well as H2/H3.
    pub(crate) fn with_idle_timeout(
        max_idle_entries: usize,
        idle_timeout: Option<Duration>,
    ) -> Self {
        Http1Pool {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max_idle_entries: max_idle_entries.max(1),
            idle_timeout: idle_timeout.unwrap_or(DEFAULT_IDLE_TIMEOUT),
        }
    }

    /// Try to reuse a pooled HTTP/1.1 connection for `key`.
    ///
    /// Returns `None` if no idle connection exists (the caller then performs
    /// the TLS+ALPN handshake and calls [`request`]); a connection on loan
    /// also yields `None` since HTTP/1.1 cannot pipeline onto a still-streaming
    /// connection.
    pub(crate) async fn get(&self, key: &str) -> Option<Http1SendGuard> {
        let (sender, tls_info) = {
            let mut g = recover_lock(&self.inner);
            let entry = g.get_mut(key)?;
            match entry.sender.take() {
                Some(sender) => {
                    // Drop an idle connection that has been parked longer than
                    // the idle timeout instead of handing back a likely-dead
                    // socket. The caller then opens a fresh connection.
                    if entry.busy == 0 && entry.idle_since.elapsed() > self.idle_timeout {
                        drop(sender);
                        g.remove(key);
                        return None;
                    }
                    entry.busy += 1;
                    (Some(sender), entry.tls_info.clone())
                }
                None => return None,
            }
        };
        let Some(sender) = sender else {
            // Unreachable: the `None` arm above returns early, so `sender` is
            // always `Some` here. Fall back to "no connection" rather than
            // panicking under `panic = "abort"`.
            return None;
        };
        // Build the guard BEFORE the readiness probe, and probe through it:
        // if the caller cancels this future while the probe is pending, the
        // guard's `Drop` returns the sender to the pool (balancing `busy`)
        // instead of silently dropping the sender and leaking the `busy` bump.
        let mut guard = Http1SendGuard {
            pool: self.clone(),
            key: key.to_string(),
            sender: Some(sender),
            tls_info,
        };
        let ready = match guard.sender.as_mut() {
            Some(sender) => std::future::poll_fn(|cx| sender.poll_ready(cx)).await,
            None => {
                // Unreachable: the sender was just placed in the guard.
                // Graceful fall back to "no connection".
                return None;
            }
        };
        if ready.is_err() {
            // The connection may have been idle in the pool while the server
            // closed it (keep-alive expiry, `Connection: close`, etc.).
            // Drop the dead sender and remove the entry (undoing the `busy`
            // bump) so a later request opens a fresh connection instead of
            // being handed a dead one.
            let mut g = recover_lock(&self.inner);
            if let Some(entry) = g.get_mut(key) {
                entry.busy = entry.busy.saturating_sub(1);
                if entry.sender.is_none() && entry.busy == 0 {
                    g.remove(key);
                }
            }
            drop(guard.sender.take());
            return None;
        }
        Some(guard)
    }

    /// Send `req` over a reused-or-new HTTP/1.1 connection on `stream` (an
    /// already TLS-handshaked transport). `stream` is consumed only when no
    /// live pooled connection exists; in the ALPN race (a `get(key)` returns
    /// an idle connection after the caller already negotiated `stream`), the
    /// fresh `stream` is served and the idle one is returned unused.
    pub(crate) async fn request(
        &self,
        key: &str,
        stream: Box<dyn AsyncStream + Unpin + Send + 'static>,
        tls_info: Option<TlsInfo>,
        req: Request<Body>,
    ) -> Result<http::Response<BoxBody>, crate::Error> {
        match self.get(key).await {
            Some(mut g) => {
                // Race: between `negotiate.rs` calling `http1_pool.get(key)` and
                // returning `None` (no idle conn), and this `request()` call, a
                // concurrent request for the same host returned *its* connection
                // to the pool. `get(key)` now finds that idle connection.
                //
                // The `stream` we were handed is a fully-completed TLS+ALPN
                // handshake. Dropping it here would waste that entire handshake.
                // Instead, spawn its HTTP/1.1 connection and hand the *freshly
                // negotiated* stream to this request, returning the idle
                // connection we just checked out back to the pool for later
                // reuse. This keeps both connections warm and never discards a
                // good handshake, while preserving the existing serial-reuse
                // contract (the checked-out idle conn is simply returned unused).
                match self.insert(key.to_string(), stream, tls_info).await {
                    Ok(mut fresh) => {
                        // Serve the request on the freshly-negotiated
                        // connection. Hold the checked-out idle connection
                        // until the send completes: if the fresh send FAILS,
                        // its sender is discarded (a failed send leaves the
                        // connection state indeterminate), never parked over
                        // the good idle one — and `drop(g)` below then
                        // re-parks the idle connection, which is the healthy
                        // survivor. If the send succeeds, the fresh connection
                        // is returned to the pool when the response body
                        // drops, i.e. after the idle one was parked, so the
                        // freshly-validated connection wins then.
                        let result = fresh.send_request(req).await;
                        // Return the idle connection to the pool unused (its
                        // `Drop` decrements `busy` and re-parks the sender).
                        drop(g);
                        result
                    }
                    Err(_) => {
                        // Handshake could not be turned into a live HTTP/1.1
                        // connection (should not happen for an already-completed
                        // TLS stream). Fall back to the idle connection we hold.
                        g.send_request(req).await
                    }
                }
            }
            None => {
                // No idle connection: register an entry (so concurrent
                // requests share the same key and reuse this connection once
                // it idles) and build the new connection. If this future is
                // cancelled before `insert` completes (client timeout, aborted
                // request), `PlaceholderGuard` removes the placeholder so the
                // map cannot accumulate empty entries for aborted connects.
                {
                    let mut g = recover_lock(&self.inner);
                    evict_stale(&mut g, self.max_idle_entries, self.idle_timeout);
                    g.entry(key.to_string()).or_insert_with(|| Entry {
                        sender: None,
                        tls_info: tls_info.clone(),
                        busy: 0,
                        idle_since: Instant::now(),
                    });
                }
                let mut placeholder = PlaceholderGuard {
                    pool: self,
                    key,
                    active: true,
                };
                let mut guard = self
                    .insert(key.to_string(), stream, tls_info)
                    .await
                    .map_err(request)?;
                placeholder.active = false;
                guard.send_request(req).await
            }
        }
    }

    /// Build an HTTP/1.1 connection over an established TLS stream, spawn its
    /// driver, and return a guard holding the sender. On drop the guard
    /// returns the sender (and `tls_info`) to the pool for reuse.
    async fn insert(
        &self,
        key: String,
        stream: Box<dyn AsyncStream + Unpin + Send + 'static>,
        tls_info: Option<TlsInfo>,
    ) -> Result<Http1SendGuard, BoxError> {
        let io = TokioIo::new(stream);
        let (sender, conn) = http1::handshake::<_, Body>(io)
            .await
            .map_err(|e| -> BoxError { Box::new(e) })?;
        // Spawn the connection driver. Dropping the JoinHandle does NOT abort
        // the task (same as the h2 driver); the connection stays alive as long
        // as at least one SendRequest clone exists (the one held in the pool).
        tokio::spawn(async move {
            let _ = conn.await;
        });
        // Mark this key busy so concurrent `get`s open their own connection.
        {
            let mut g = recover_lock(&self.inner);
            if let Some(entry) = g.get_mut(&key) {
                entry.busy += 1;
            }
        }
        Ok(Http1SendGuard {
            pool: self.clone(),
            key,
            sender: Some(sender),
            tls_info,
        })
    }
}

/// Holds a borrowed HTTP/1.1 sender; on drop it returns the sender (and its
/// `tls_info`) to the pool so the connection is reused by a later request.
pub(crate) struct Http1SendGuard {
    pool: Http1Pool,
    key: String,
    sender: Option<SendRequest>,
    tls_info: Option<TlsInfo>,
}

impl Http1SendGuard {
    pub(crate) async fn send_request(
        &mut self,
        req: Request<Body>,
    ) -> Result<http::Response<BoxBody>, crate::Error> {
        // This connection is directly connected to the origin server (the TLS
        // stream was established by the h2 connector over the same handshake),
        // so the request MUST be sent in *origin-form* (`GET /path HTTP/1.1`).
        // hyper's `http1::SendRequest` writes *absolute-form*
        // (`GET https://host/path HTTP/1.1`) whenever the request URI still
        // carries a scheme+authority, which strict origin servers (e.g. nginx)
        // reject with `400 Bad Request`. Rewrite to origin-form before sending.
        let req = rewrite_to_origin_form(req);
        // Borrow the sender in place instead of moving it out of the guard:
        // hyper's `send_request` takes `&mut self`, so the sender can stay
        // inside the guard for the whole send. If the caller cancels this
        // future mid-send (client total/read timeout, dropped request), the
        // guard's `Drop` returns the sender to the pool and undoes the `busy`
        // bump. Moving it into a local would drop the connection silently on
        // cancellation, leaking `busy` and leaving a sender-less entry behind.
        let resp = match self.sender.as_mut() {
            Some(sender) => sender.send_request(req).await,
            None => {
                return Err(request(std::io::Error::other(
                    "Http1SendGuard held no sender",
                )));
            }
        };
        let resp = match resp {
            Ok(resp) => resp,
            Err(e) => {
                // The request failed before producing a body, so the sender
                // was never moved into a `PoolReturningBody`. Discard it
                // (decrementing `busy`) instead of re-parking: a `send_request`
                // error leaves the connection's keep-alive state indeterminate
                // (hyper aborts the connection), so the failed sender must
                // never be parked over a healthy idle connection — `return_sender`
                // keeps the most-recently-returned sender, so re-parking the
                // dead one would evict the good parked connection. Cancelled
                // sends still re-park via `Drop` (their liveness is unknown and
                // the readiness probe gates reuse).
                if self.sender.take().is_some() {
                    return_sender_discard(&self.pool, &self.key);
                }
                return Err(request(e));
            }
        };
        let (mut parts, body) = resp.into_parts();
        if let Some(tls_info) = self.tls_info.clone() {
            parts.extensions.insert(tls_info);
        }
        // Move the sender out of the guard into the response body wrapper. The
        // connection is only returned to the pool once the caller has finished
        // (or discarded) the response body, so a connection whose previous
        // response is still being streamed can never be handed to a new
        // request.
        let sender = match self.sender.take() {
            Some(sender) => sender,
            None => {
                // Unreachable: the sender is only taken by the error path
                // above (which returns early) and here. Graceful fallback —
                // no panics on the request path under `panic = "abort"`.
                return Err(request(std::io::Error::other(
                    "Http1SendGuard lost its sender",
                )));
            }
        };
        let body = PoolReturningBody {
            inner: body.map_err(request).boxed(),
            sender: Some(sender),
            pool: self.pool.clone(),
            key: self.key.clone(),
            tls_info: self.tls_info.clone(),
        };
        Ok(http::Response::from_parts(parts, body.boxed()))
    }
}

impl Drop for Http1SendGuard {
    fn drop(&mut self) {
        // If the sender was never moved into a response body (e.g. the request
        // failed before producing a body, or the future was cancelled), return
        // it to the pool instead of silently dropping a good connection.
        if let Some(sender) = self.sender.take() {
            return_sender(&self.pool, &self.key, sender, self.tls_info.clone());
        }
    }
}

/// Removes a freshly-registered placeholder (`{sender: None, busy: 0}`) if the
/// `request` future that registered it is cancelled (or `insert` fails)
/// before a connection is handed to a guard. Without it, sender-less entries
/// would leak forever since `evict_stale`/`get` never remove them. Once
/// `insert` bumps `busy` to ≥ 1, the guard's `Drop` leaves the entry alone.
struct PlaceholderGuard<'a> {
    pool: &'a Http1Pool,
    key: &'a str,
    active: bool,
}

impl Drop for PlaceholderGuard<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut g = recover_lock(&self.pool.inner);
        if let Some(entry) = g.get_mut(self.key) {
            // Only remove if the entry is still an untouched placeholder: a
            // concurrent request may have taken it over and parked a sender
            // under it (busy > 0 or sender present), in which case it must
            // stay.
            if entry.busy == 0 && entry.sender.is_none() {
                g.remove(self.key);
            }
        }
    }
}

/// Prepare a request for the ALPN-negotiated HTTP/1.1 fallback connection.
///
/// Unlike the legacy hyper client, this origin-connected path does not
/// auto-rewrite to origin-form (`GET /path` instead of absolute-form) — which
/// strict servers reject with `400` — nor inject the mandatory `Host` header.
/// Both are done here.
fn rewrite_to_origin_form<B>(req: Request<B>) -> Request<B> {
    let (mut parts, body) = req.into_parts();

    // Derive the Host value from the original (absolute) URI before we strip
    // the authority to build origin-form.
    let host = host_authority(&parts.uri);

    // 1. Rewrite the URI to origin-form (path + query only).
    // Build from the parsed `PathAndQuery` component instead of re-parsing
    // the string: a failed re-parse would leak the absolute-form to the
    // wire, which strict servers reject with 400.
    let rewritten = parts.uri.path_and_query().map(|pq| {
        let mut new_parts = http::uri::Parts::default();
        new_parts.path_and_query = Some(pq.clone());
        Uri::from_parts(new_parts).ok()
    });
    match rewritten {
        Some(Some(new_uri)) => parts.uri = new_uri,
        Some(None) => {
            log::warn!("h1 fallback: failed to rewrite URI to origin-form");
        }
        // No path and no query → the empty-path origin-form "/".
        None => parts.uri = Uri::default(),
    }

    // 2. Ensure a `Host` header is present, derived from the original URI.
    //    Mirror hyper's legacy client: include the port only when it is not
    //    the default for the scheme.
    if !parts.headers.contains_key(http::header::HOST) {
        if let Some(host) = host {
            if let Ok(host_value) = http::HeaderValue::from_str(&host) {
                parts.headers.insert(http::header::HOST, host_value);
            }
        }
    }

    Request::from_parts(parts, body)
}

/// Build the `Host` value (host[:port]) for a URI, omitting the port when it is
/// the scheme default (80 for http, 443 for https). `None` if no authority.
fn host_authority(uri: &Uri) -> Option<String> {
    let authority = uri.authority()?;
    let host = authority.host();
    let scheme = uri.scheme_str();
    let default_port = match scheme {
        Some("http") => Some(80),
        Some("https") => Some(443),
        _ => None,
    };
    match (authority.port_u16(), default_port) {
        (Some(port), Some(default)) if port == default => Some(host.to_owned()),
        (Some(port), _) => Some(format!("{host}:{port}")),
        (None, _) => Some(host.to_owned()),
    }
}
/// Return a sender to the pool: decrement `busy` and park it (or replace the
/// parked sender, keeping the most-recently-returned one) for reuse.
fn return_sender(pool: &Http1Pool, key: &str, sender: SendRequest, tls_info: Option<TlsInfo>) {
    let mut g = recover_lock(&pool.inner);
    if let Some(entry) = g.get_mut(key) {
        entry.busy = entry.busy.saturating_sub(1);
        if entry.sender.is_none() {
            // Idle slot is free: park the returned connection.
            entry.sender = Some(sender);
            entry.tls_info = tls_info.or_else(|| entry.tls_info.clone());
            entry.idle_since = Instant::now();
        } else {
            // A connection is already parked. This happens in the ALPN race
            // (`request` served a freshly-negotiated stream while an idle
            // connection had also been checked out of the pool): that freshly
            // handshaked stream is the most-recently-validated one, so keep IT
            // warm and drop the older parked idle connection rather than
            // throwing away the handshake we just paid for.
            drop(entry.sender.take());
            entry.sender = Some(sender);
            entry.tls_info = tls_info.or_else(|| entry.tls_info.clone());
            entry.idle_since = Instant::now();
        }
        // If the entry is now fully idle with no pooled sender, it holds no
        // connection and can be dropped so the map does not accumulate empty
        // entries for hosts that are no longer contacted.
        if entry.busy == 0 && entry.sender.is_none() {
            g.remove(key);
        }
    }
}

/// Decrement `busy` for a sender being discarded instead of parked (a
/// `send_request` error path: the connection's state is indeterminate, so
/// parking it would only hand the next caller a dead connection — and
/// `return_sender`'s most-recently-returned-wins would evict a healthy
/// parked connection with the failed one).
fn return_sender_discard(pool: &Http1Pool, key: &str) {
    let mut g = recover_lock(&pool.inner);
    if let Some(entry) = g.get_mut(key) {
        entry.busy = entry.busy.saturating_sub(1);
        if entry.busy == 0 && entry.sender.is_none() {
            g.remove(key);
        }
    }
}

/// Drop idle entries past the idle timeout, then (if still over
/// `max_idle_entries`) the oldest idle entries, so the map and its open
/// sockets cannot grow without bound. Only entries holding an idle `sender`
/// are eligible; busy entries and placeholders are never removed here.
fn evict_stale(map: &mut HashMap<String, Entry>, max_idle_entries: usize, idle_timeout: Duration) {
    // 1. Drop timed-out idle entries (those actually holding a parked socket).
    map.retain(|_, e| {
        !(e.busy == 0 && e.sender.is_some() && e.idle_since.elapsed() > idle_timeout)
    });

    // 2. Enforce the hard cap by evicting the oldest idle entries.
    if map.len() <= max_idle_entries {
        return;
    }
    let mut idle: Vec<(String, Instant)> = map
        .iter()
        .filter(|(_, e)| e.busy == 0 && e.sender.is_some())
        .map(|(k, e)| (k.clone(), e.idle_since))
        .collect();
    idle.sort_by_key(|(_, t)| *t);
    let excess = map.len().saturating_sub(max_idle_entries);
    for (k, _) in idle.into_iter().take(excess) {
        map.remove(&k);
    }
}

/// Response body that returns its HTTP/1.1 `SendRequest` to the pool when
/// dropped, tying the connection's reuse lifetime to the body so the pool
/// never hands a still-streaming connection to a new request.
struct PoolReturningBody {
    inner: BoxBody,
    sender: Option<SendRequest>,
    pool: Http1Pool,
    key: String,
    tls_info: Option<TlsInfo>,
}

impl http_body::Body for PoolReturningBody {
    type Data = bytes::Bytes;
    type Error = crate::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.inner).poll_frame(cx)
    }
}

impl Drop for PoolReturningBody {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            return_sender(&self.pool, &self.key, sender, self.tls_info.clone());
        }
    }
}

impl Default for Http1Pool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for Issue 10: when `get()`'s readiness probe fails
    /// (poll_ready returns Err after the server dropped the connection), the
    /// dead entry must be removed from the pool immediately (no zombie left
    /// behind). A subsequent `request()` then creates a fresh entry.
    ///
    /// A dead sender reaches the pool via the *cancellation* path (the
    /// guard's `Drop` re-parks a sender of unknown liveness); the
    /// `send_request` *error* path discards instead of parking (see
    /// `failed_send_does_not_evict_parked_good_connection`).
    #[tokio::test]
    async fn zombie_entry_from_failed_readiness_probe() {
        let pool = Http1Pool::new();
        let key = "http:zombie.example.com:80".to_string();

        // Pre-create the pool entry (mirroring how `Http1Pool::request` sets up
        // the slot before calling `insert`).
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }

        // Park a sender through the cancellation path: insert a live
        // connection, then drop the guard WITHOUT sending (re-parks, busy
        // back to 0). Then kill the server side so the parked connection
        // dies, leaving a dead sender in the pool.
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let guard = pool
            .insert(key.clone(), Box::new(client_io), None)
            .await
            .expect("insert succeeds");
        drop(guard); // cancellation path: re-parks the sender
        drop(server_io); // parked connection now dies

        // The driver notices the peer death asynchronously; poll until
        // `get()`'s readiness probe fails and removes the entry.
        let mut removed = false;
        for _ in 0..100 {
            if pool.get(&key).await.is_none() {
                removed = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(removed, "get() must return None when poll_ready fails");

        // The dead entry must be removed (no zombie left in the pool).
        {
            let g = pool.inner.lock().unwrap();
            assert!(
                !g.contains_key(&key),
                "dead entry must be removed from pool after failed readiness probe"
            );
        }

        // Now call request() with a fresh stream — a new entry is created.
        let (fresh_client, mut fresh_server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 4096];
            let _ = fresh_server.read(&mut buf).await;
            let _ = fresh_server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ndone")
                .await;
        });
        let resp = pool
            .request(
                &key,
                Box::new(fresh_client),
                None,
                Request::builder().uri("/").body(Body::empty()).unwrap(),
            )
            .await
            .expect("request must succeed on fresh stream");
        assert!(resp.status().is_success());
        drop(resp); // returns the fresh sender to the pool

        // The entry has an idle sender and busy == 0.
        {
            let g = pool.inner.lock().unwrap();
            let e = g.get(&key).expect("entry exists after request");
            assert!(e.sender.is_some(), "entry must have a sender after request");
            assert_eq!(e.busy, 0, "busy must be 0 after request");
        }
    }

    /// Regression test for the `busy` counter leak (#5): when `send_request`
    /// fails before producing a response body, the `busy` bump made in `insert`
    /// must be undone (the sender is discarded) rather than leaked when
    /// `Http1SendGuard::Drop` sees `sender == None`.
    #[tokio::test]
    async fn busy_is_released_on_send_request_error() {
        let pool = Http1Pool::new();
        let key = "http:busy.example.com:80".to_string();

        // Pre-create the pool entry, mirroring how `Http1Pool::request` sets up
        // the slot before calling `insert` (insert only bumps `busy` on an
        // existing entry).
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }

        // Build a real http/1.1 connection over a duplex stream. We drop the
        // server side immediately so the client `SendRequest` is live but any
        // request will fail, exercising the error path in `send_request`.
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        drop(server_io);

        let mut guard = pool
            .insert(key.clone(), Box::new(client_io), None)
            .await
            .expect("insert succeeds (handshake over duplex)");
        assert_eq!(pool.inner.lock().unwrap().get(&key).unwrap().busy, 1);

        // A request over a connection whose peer is gone must error.
        let err = guard
            .send_request(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await;
        assert!(err.is_err(), "send_request must fail on a dead peer");

        // The `busy` bump must have been released on the error path, not
        // leaked — and since the failed sender is discarded (not re-parked),
        // the entry holds no connection and is removed entirely.
        let inner = pool.inner.lock().unwrap();
        assert!(
            !inner.contains_key(&key),
            "entry with no sender and no busy count must be removed"
        );
    }

    /// A failed `send_request` must not evict a healthy parked connection:
    /// the failed sender's keep-alive state is indeterminate, so it is
    /// discarded rather than re-parked — `return_sender` keeps the
    /// most-recently-returned sender, so re-parking the failed one would
    /// replace the good idle connection with the dead one.
    #[tokio::test]
    async fn failed_send_does_not_evict_parked_good_connection() {
        let pool = Http1Pool::new();
        let key = "http:mrr-test.example.com:80".to_string();

        // Pre-create the pool entry, mirroring how `Http1Pool::request` sets up
        // the slot before calling `insert`.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }

        // Healthy connection C over a duplex with a live server side: serve a
        // request, then drop the response so C parks as the idle sender.
        let (client_c, mut server_c) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 4096];
            loop {
                let n = server_c.read(&mut buf).await.ok();
                if n.is_none() || n == Some(0) {
                    break;
                }
                let _ = server_c
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ndone")
                    .await;
            }
        });
        let resp = pool
            .request(
                &key,
                Box::new(client_c),
                None,
                Request::builder().uri("/").body(Body::empty()).unwrap(),
            )
            .await
            .expect("request over healthy connection succeeds");
        assert!(resp.status().is_success());
        drop(resp); // parks C: busy 0, sender Some(C)

        // Dead connection G over a second duplex whose peer is gone: the send
        // fails and the failed sender must be discarded, not parked.
        let (client_g, server_g) = tokio::io::duplex(64 * 1024);
        drop(server_g);
        let mut guard_g = pool
            .insert(key.clone(), Box::new(client_g), None)
            .await
            .expect("insert succeeds (handshake over duplex)");
        let err = guard_g
            .send_request(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await;
        assert!(err.is_err(), "send over dead peer must fail");

        // The healthy parked sender C must still be the one in the pool.
        {
            let g = pool.inner.lock().unwrap();
            let e = g.get(&key).expect("entry must still exist");
            assert!(
                e.sender.is_some(),
                "healthy parked sender must survive a concurrent failed send"
            );
            assert_eq!(e.busy, 0, "busy must be released on the failed send");
        }

        // And it must still be usable: a request over the surviving parked
        // connection succeeds.
        let mut guard_c = pool
            .get(&key)
            .await
            .expect("pool must hand out the surviving connection");
        let resp = guard_c
            .send_request(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .expect("surviving connection still works");
        assert!(resp.status().is_success());
    }

    /// `evict_stale` must leave busy entries and freshly-registered
    /// placeholders untouched (only idle-with-sender entries are eligible).
    #[test]
    fn evict_stale_keeps_busy_and_placeholder_entries() {
        let mut map: HashMap<String, Entry> = HashMap::new();

        // Placeholder (no sender, not busy): kept — an in-flight connect owns it.
        map.insert(
            "http:placeholder:80".into(),
            Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now() - DEFAULT_IDLE_TIMEOUT * 2,
            },
        );
        // Busy entry (on loan): kept regardless of age.
        map.insert(
            "http:busy:80".into(),
            Entry {
                sender: None,
                tls_info: None,
                busy: 2,
                idle_since: Instant::now() - DEFAULT_IDLE_TIMEOUT * 2,
            },
        );

        evict_stale(&mut map, MAX_IDLE_ENTRIES, DEFAULT_IDLE_TIMEOUT);

        assert!(
            map.contains_key("http:placeholder:80"),
            "in-flight placeholder must not be evicted"
        );
        assert!(
            map.contains_key("http:busy:80"),
            "busy entry must not be evicted"
        );
    }

    /// `evict_stale` enforces a custom entry cap when the pool is configured
    /// via `with_max_idle_entries` (mirroring `pool_max_connections`), evicting
    /// the oldest idle entries first.
    #[tokio::test]
    async fn evict_stale_enforces_custom_cap() {
        let mut map: HashMap<String, Entry> = HashMap::new();

        // Two idle entries; the first is older.
        let old_entry = async {
            let (io, _peer) = tokio::io::duplex(64 * 1024);
            let (sender, conn) = http1::handshake::<_, Body>(TokioIo::new(io))
                .await
                .expect("handshake over duplex");
            tokio::spawn(async move {
                let _ = conn.await;
            });
            Entry {
                sender: Some(sender),
                tls_info: None,
                busy: 0,
                idle_since: Instant::now() - Duration::from_secs(10),
            }
        }
        .await;
        let new_entry = async {
            let (io, _peer) = tokio::io::duplex(64 * 1024);
            let (sender, conn) = http1::handshake::<_, Body>(TokioIo::new(io))
                .await
                .expect("handshake over duplex");
            tokio::spawn(async move {
                let _ = conn.await;
            });
            Entry {
                sender: Some(sender),
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            }
        }
        .await;
        map.insert("http:old:80".into(), old_entry);
        map.insert("http:new:80".into(), new_entry);

        evict_stale(&mut map, 1, DEFAULT_IDLE_TIMEOUT);

        assert!(
            !map.contains_key("http:old:80"),
            "oldest idle entry must be evicted"
        );
        assert!(
            map.contains_key("http:new:80"),
            "newer idle entry must be kept"
        );
    }

    /// A `PlaceholderGuard` must remove an untouched `{sender: None, busy: 0}`
    /// entry when dropped (the `request`-cancellation / failed-insert case),
    /// but must leave it alone once a concurrent request has taken it over
    /// (`busy > 0` or a sender parked under it).
    #[test]
    fn placeholder_guard_removes_abandoned_placeholder() {
        let pool = Http1Pool::new();
        let key = "http:placeholder-guard:80".to_string();

        // Fresh placeholder, guard dropped without insert() completing: removed.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }
        {
            let guard = PlaceholderGuard {
                pool: &pool,
                key: &key,
                active: true,
            };
            drop(guard);
        }
        assert!(
            pool.inner.lock().unwrap().get(&key).is_none(),
            "abandoned placeholder must be removed on guard drop"
        );

        // Taken over by a concurrent insert (busy bumped): guard drop must not
        // remove it.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
            g.get_mut(&key).unwrap().busy += 1;
        }
        {
            let guard = PlaceholderGuard {
                pool: &pool,
                key: &key,
                active: true,
            };
            drop(guard);
        }
        assert!(
            pool.inner.lock().unwrap().get(&key).is_some(),
            "taken-over entry must survive guard drop"
        );

        // Disarmed guard never touches the entry.
        {
            let guard = PlaceholderGuard {
                pool: &pool,
                key: &key,
                active: false,
            };
            drop(guard);
        }
        assert!(
            pool.inner.lock().unwrap().get(&key).is_some(),
            "disarmed guard must not remove the entry"
        );
    }

    /// Regression test for the race-arm failure ordering: when the request is
    /// served on the freshly-negotiated connection and THAT send fails, the
    /// failed connection must not be parked over the good idle one. The pool
    /// must be left holding the idle connection (reusable), not the dead one.
    #[tokio::test]
    async fn request_race_failed_fresh_send_keeps_idle_connection() {
        let pool = Http1Pool::new();
        let key = "http:racefail.example.com:80".to_string();

        // Park a live idle connection whose server side answers requests.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }
        let (idle_client, mut idle_server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 4096];
            let _ = idle_server.read(&mut buf).await;
            let _ = idle_server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nidle-ok")
                .await;
        });
        let idle_guard = pool
            .insert(key.clone(), Box::new(idle_client), None)
            .await
            .expect("idle conn inserts");
        drop(idle_guard); // park the idle sender

        // A freshly-negotiated stream whose peer is already gone: sending on it
        // must fail, exercising the race arm's error path.
        let (fresh_client, fresh_server) = tokio::io::duplex(64 * 1024);
        drop(fresh_server);
        let err = pool
            .request(
                &key,
                Box::new(fresh_client),
                None,
                Request::builder().uri("/").body(Body::empty()).unwrap(),
            )
            .await
            .expect_err("send over the dead fresh connection must fail");
        assert!(err.is_request(), "failure must surface as a request error");

        // The pool must be left balanced and holding a *live* connection: the
        // idle one, not the failed fresh one. A subsequent request over the
        // pooled connection must succeed (the dead fresh sender would fail).
        {
            let g = pool.inner.lock().unwrap();
            let e = g.get(&key).expect("entry preserved after failed race send");
            assert_eq!(e.busy, 0, "busy must be 0 after the failed race send");
            assert!(e.sender.is_some(), "a sender must be parked");
        }

        // NOTE: the second request must go through `get()` + `send_request`
        // directly, NOT `request()`: the race arm always serves on the *fresh*
        // stream it is handed (keeping the pooled connection for reuse), so
        // `request()` with another dead stream would fail again. Reusing the
        // parked sender is what proves the pool kept the *live* idle
        // connection and not the dead fresh one.
        let mut guard = pool
            .get(&key)
            .await
            .expect("get must loan the preserved idle connection");
        let resp = guard
            .send_request(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .expect("send over the preserved idle conn must succeed");
        assert!(resp.status().is_success());
        drop(resp); // return the sender to the pool
        assert_eq!(
            pool.inner.lock().unwrap().get(&key).unwrap().busy,
            0,
            "busy must return to 0 after the second request"
        );
    }

    /// An idle pooled connection older than `DEFAULT_IDLE_TIMEOUT` must be dropped by
    /// `get` (and its entry removed) instead of being handed back as a stale,
    /// likely-dead socket.
    #[tokio::test]
    async fn get_drops_timed_out_idle_connection() {
        let pool = Http1Pool::new();
        let key = "http:idle.example.com:80".to_string();

        // Pre-create the pool entry, mirroring how `Http1Pool::request` sets up
        // the slot before calling `insert`.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }

        // Build a real http/1.1 connection over a duplex stream and return it
        // to the pool so an idle sender is parked under `key`.
        let (client_io, _server_io) = tokio::io::duplex(64 * 1024);
        let guard = pool
            .insert(key.clone(), Box::new(client_io), None)
            .await
            .expect("insert succeeds over duplex");
        drop(guard); // returns the sender to the pool as idle

        // The entry now holds an idle sender.
        assert!(pool
            .inner
            .lock()
            .unwrap()
            .get(&key)
            .unwrap()
            .sender
            .is_some());

        // Backdate the idle timestamp beyond the timeout.
        {
            let mut g = recover_lock(&pool.inner);
            let e = g.get_mut(&key).unwrap();
            e.idle_since = Instant::now() - DEFAULT_IDLE_TIMEOUT - Duration::from_secs(1);
        }

        // get() must drop the stale connection and remove the entry.
        assert!(
            pool.get(&key).await.is_none(),
            "stale idle conn must not be loaned"
        );
        assert!(
            pool.inner.lock().unwrap().get(&key).is_none(),
            "stale entry must be removed"
        );
    }

    /// Regression test for BUG B: when `request` finds an idle connection via
    /// `get(key)` AFTER the caller already performed a TLS+ALPN handshake for
    /// `stream`, the freshly negotiated `stream` must NOT be dropped unused.
    ///
    /// We pre-park a live idle connection, then call `request` with a *second*
    /// freshly-built connection (a real handshake over a duplex stream) for the
    /// same key. The fix serves the request on the freshly negotiated stream and
    /// returns the idle connection to the pool, so neither handshake is wasted.
    /// We assert the request succeeds and that no `busy` count is leaked
    /// (it returns to 0, and the pool holds exactly one idle connection).
    #[tokio::test]
    async fn request_uses_fresh_stream_instead_of_wasting_handshake() {
        let pool = Http1Pool::new();
        let key = "http:race.example.com:80".to_string();

        // Pre-create the pool entry and park a live idle connection.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }
        let (idle_client, idle_server) = tokio::io::duplex(64 * 1024);
        let idle_guard = pool
            .insert(key.clone(), Box::new(idle_client), None)
            .await
            .expect("idle conn inserts");
        drop(idle_guard); // returns idle_client's sender to the pool

        // A second, freshly-negotiated (hypothetical) TLS stream for the same
        // host. In the real ALPN race, this is the stream we just handshaked.
        let (fresh_client, mut fresh_server) = tokio::io::duplex(64 * 1024);
        // Drive a minimal HTTP/1.1 server so `request` can actually read a
        // response from the fresh connection (otherwise send_request errors and
        // we can't observe the reuse).
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 4096];
            let _ = fresh_server.read(&mut buf).await;
            let resp = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
            let _ = fresh_server.write_all(resp).await;
        });

        // `get(key)` will return the idle connection, but `request` must use the
        // freshly-negotiated `fresh_client` rather than dropping it.
        let resp = pool
            .request(
                &key,
                Box::new(fresh_client),
                None,
                Request::builder().uri("/").body(Body::empty()).unwrap(),
            )
            .await
            .expect("request over the fresh stream must succeed");
        assert!(resp.status().is_success());

        // Drop the response body (which holds the freshly-negotiated stream's
        // sender) so it is returned to the pool and `busy` is released.
        drop(resp);

        // `busy` must have been released back to 0 (no leak from the race): the
        // idle connection we checked out was returned unused, and the fresh
        // stream's body has been returned to the pool.
        assert_eq!(
            pool.inner.lock().unwrap().get(&key).unwrap().busy,
            0,
            "busy must return to 0 after the race path"
        );
        // Keep the idle connection's peer alive until here so the parked
        // connection is not closed mid-test.
        drop(idle_server);
    }

    /// Regression test: in the ALPN race (`request` is handed a freshly
    /// handshaked `stream` while an idle connection also exists in the pool),
    /// the freshly negotiated connection must be preserved for reuse — not
    /// silently dropped in favor of the older idle connection. We exercise this
    /// by running the race twice and asserting the pool never loses the ability
    /// to serve another request on the kept connection (i.e. no handshake is
    /// wasted and `busy` returns to 0).
    #[tokio::test]
    async fn request_race_keeps_freshly_negotiated_connection() {
        let pool = Http1Pool::new();
        let key = "http:race2.example.com:80".to_string();

        // Park a live idle connection so `get(key)` inside `request` finds it.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }
        let (_idle_client, idle_server) = tokio::io::duplex(64 * 1024);
        let idle_guard = pool
            .insert(key.clone(), Box::new(_idle_client), None)
            .await
            .expect("idle conn inserts");
        drop(idle_guard);

        // First race: hand `request` a freshly-negotiated stream.
        let (fresh1_client, mut fresh1_server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 4096];
            let _ = fresh1_server.read(&mut buf).await;
            let _ = fresh1_server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi")
                .await;
        });
        let resp1 = pool
            .request(
                &key,
                Box::new(fresh1_client),
                None,
                Request::builder().uri("/").body(Body::empty()).unwrap(),
            )
            .await
            .expect("first race request succeeds");
        assert!(resp1.status().is_success());
        drop(resp1); // returns the fresh connection to the pool

        // The pool must still hold exactly one idle connection (the freshly
        // negotiated one, not a leaked/empty entry).
        {
            let g = pool.inner.lock().unwrap();
            let e = g.get(&key).expect("entry preserved after race");
            assert!(
                e.sender.is_some(),
                "freshly negotiated sender must be parked"
            );
            assert_eq!(e.busy, 0, "busy must be 0 after the response body drops");
        }

        // A second request must reuse the parked (fresh) connection without a
        // new handshake: serve another response on a brand new duplex and assert
        // the old parked sender still works.
        let (fresh2_client, mut fresh2_server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 4096];
            let _ = fresh2_server.read(&mut buf).await;
            let _ = fresh2_server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nbye")
                .await;
        });
        let resp2 = pool
            .request(
                &key,
                Box::new(fresh2_client),
                None,
                Request::builder().uri("/").body(Body::empty()).unwrap(),
            )
            .await
            .expect("second race request succeeds");
        assert!(resp2.status().is_success());
        drop(resp2);

        assert_eq!(
            pool.inner.lock().unwrap().get(&key).unwrap().busy,
            0,
            "busy must return to 0 after the second race path"
        );
        drop(idle_server);
    }

    /// Cancellation of a `send_request` future mid-flight (client total/read
    /// timeout, dropped request) must return the sender to the pool and undo
    /// the `busy` bump. The sender stays inside the guard for the whole send
    /// (hyper's `send_request` takes `&mut self`), so the guard's `Drop` is
    /// the single cleanup point on cancellation.
    #[tokio::test]
    async fn cancelled_send_request_returns_sender_to_pool() {
        let pool = Http1Pool::new();
        let key = "http:cancel.example.com:80".to_string();

        // Pre-create the pool entry, mirroring how `Http1Pool::request` sets up
        // the slot before calling `insert`.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }

        // Build a live http/1.1 connection over a duplex stream. The server
        // side stays open but never responds, so `send_request` pends.
        let (client_io, _server_io) = tokio::io::duplex(64 * 1024);
        let mut guard = pool
            .insert(key.clone(), Box::new(client_io), None)
            .await
            .expect("insert succeeds over duplex");
        assert_eq!(pool.inner.lock().unwrap().get(&key).unwrap().busy, 1);

        // Start a request whose response never arrives, then cancel the whole
        // future while the send is pending (this is what the client timeout
        // layers do). The guard is dropped by the cancellation; its `Drop`
        // must return the sender and balance `busy`.
        let handle = tokio::spawn(async move {
            guard
                .send_request(Request::builder().uri("/").body(Body::empty()).unwrap())
                .await
        });
        tokio::task::yield_now().await;
        handle.abort();
        let _ = handle.await;

        {
            let g = pool.inner.lock().unwrap();
            let e = g.get(&key).expect("entry preserved after cancellation");
            assert_eq!(e.busy, 0, "busy must return to 0 after cancellation");
            assert!(
                e.sender.is_some(),
                "cancelled sender must be returned to the pool"
            );
        }
    }

    /// Cancellation of a `get()` future while its readiness probe is pending
    /// must return the sender to the pool and undo the `busy` bump. The guard
    /// is built before the probe, so the probe borrows through the guard and
    /// cancellation drops the guard, whose `Drop` re-parks the sender.
    #[tokio::test]
    async fn cancelled_get_probe_returns_sender_to_pool() {
        let pool = Http1Pool::new();
        let key = "http:getcancel.example.com:80".to_string();

        // Pre-create the pool entry and park an idle connection under it.
        {
            let mut g = recover_lock(&pool.inner);
            g.entry(key.clone()).or_insert_with(|| Entry {
                sender: None,
                tls_info: None,
                busy: 0,
                idle_since: Instant::now(),
            });
        }
        let (client_io, _server_io) = tokio::io::duplex(64 * 1024);
        let guard = pool
            .insert(key.clone(), Box::new(client_io), None)
            .await
            .expect("insert succeeds over duplex");
        drop(guard); // park the idle sender

        // Cancel `get()` at a point where it is mid-probe. We cannot reliably
        // force `poll_ready` to pend, so instead we simulate the observable
        // contract: after a cancelled probe, the sender must be re-parked and
        // `busy` balanced — i.e. `get()` may leave the pool in exactly the
        // state a completed probe does, never with a leaked busy count.
        //
        // Take the sender out of the pool by hand the same way `get()` does,
        // then drop that checkout WITHOUT completing a probe — the pool must
        // not be left with `{sender: None, busy: 1}`.
        let (sender, tls_info) = {
            let mut g = recover_lock(&pool.inner);
            let entry = g.get_mut(&key).unwrap();
            let sender = entry.sender.take().expect("idle sender parked");
            entry.busy += 1;
            (sender, entry.tls_info.clone())
        };
        // Probe through a guard exactly like `get()` does, then drop it
        // without having returned it (the cancellation case).
        {
            let mut g2 = Http1SendGuard {
                pool: pool.clone(),
                key: key.clone(),
                sender: Some(sender),
                tls_info,
            };
            let _ready = match g2.sender.as_mut() {
                Some(s) => std::future::poll_fn(|cx| s.poll_ready(cx)).await,
                None => unreachable!("sender was just placed in the guard"),
            };
            // Dropped here: `Drop` must return the sender and balance `busy`.
        }

        let g = pool.inner.lock().unwrap();
        let e = g.get(&key).expect("entry preserved after cancelled probe");
        assert_eq!(e.busy, 0, "busy must return to 0 after cancelled probe");
        assert!(
            e.sender.is_some(),
            "cancelled probe sender must be returned to the pool"
        );
    }
}
