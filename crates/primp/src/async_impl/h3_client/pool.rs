use bytes::Bytes;
use std::future;

use foldhash::{HashMap, HashMapExt};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::sync::{oneshot, watch, Notify};
use tokio::time::Instant;

use crate::async_impl::body::ResponseBody;
use crate::error::{BoxError, Error, Kind};
use crate::Body;
use bytes::Buf;
use h3::client::SendRequest;
use h3_quinn::{Connection, OpenStreams};
use http::uri::{Authority, Scheme};
use http::{Request, Response, Uri};
use log::{error, trace};

pub(super) type Key = (Scheme, Authority);

pub struct Pool {
    inner: Arc<Mutex<PoolInner>>,
    /// Owner Pool (from `Pool::new`) aborts driver tasks on drop; transient
    /// clones must not, or a per-request `H3Client::clone()` would kill
    /// connections still in active use.
    is_owner: bool,
}

impl Drop for Pool {
    fn drop(&mut self) {
        // Owner tears down drivers; if body still streams, detach until idle.
        if self.is_owner {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            for conn in inner.idle_conns.values_mut() {
                if conn.active_streams.load(Ordering::Acquire) == 0 {
                    if let Some(handle) = &conn.connection_task {
                        handle.abort();
                    }
                } else if tokio::runtime::Handle::try_current().is_ok() {
                    conn.detach_driver_until_idle();
                } else {
                    // No runtime — `spawn` would panic; leak so body can finish.
                    let handle = conn.connection_task.take();
                    let pool_clone = Arc::clone(&self.inner);
                    std::mem::forget(handle);
                    std::mem::forget(pool_clone);
                }
            }
        }
    }
}

struct ConnectingLockInner {
    key: Key,
    pool: Arc<Mutex<PoolInner>>,
}

/// Ensures only one HTTP/3 connection is established per host at a time;
/// released automatically on drop.
pub struct ConnectingLock(Option<ConnectingLockInner>);

/// A waiter that receives updates when a connection is established or its
/// attempt fails (e.g. when the connecting lock is dropped on error).
pub struct ConnectingWaiter {
    receiver: watch::Receiver<Option<PoolClient>>,
}

pub enum Connecting {
    /// A connection attempt is already in progress; subscribe for updates.
    InProgress(ConnectingWaiter),
    /// The connection lock is acquired; you may initiate a new connection.
    Acquired(ConnectingLock),
}

impl ConnectingLock {
    fn new(key: Key, pool: Arc<Mutex<PoolInner>>) -> Self {
        Self(Some(ConnectingLockInner { key, pool }))
    }

    /// Forget the lock and return corresponding Key
    fn forget(mut self) -> Key {
        // Unwrap is safe because the Option can be None only after dropping the
        // lock
        self.0.take().unwrap().key
    }
}

impl Drop for ConnectingLock {
    fn drop(&mut self) {
        if let Some(ConnectingLockInner { key, pool }) = self.0.take() {
            let mut pool = pool.lock().unwrap();
            pool.connecting.remove(&key);
            trace!("HTTP/3 connecting lock for {:?} is dropped", key);
        }
    }
}

impl ConnectingWaiter {
    pub async fn receive(mut self) -> Option<PoolClient> {
        self.receiver.wait_for(Option::is_some).await.ok()?;
        let guard = self.receiver.borrow_and_update();
        let client = guard.as_ref()?;
        // clone counts as borrower; the watch-held original balances on drop
        Some(client.clone())
    }
}

impl Clone for Pool {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            is_owner: false,
        }
    }
}

impl Pool {
    pub fn new(timeout: Option<Duration>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PoolInner {
                connecting: HashMap::new(),
                idle_conns: HashMap::new(),
                timeout,
            })),
            is_owner: true,
        }
    }

    /// Acquire a connecting lock, ensuring only one HTTP/3 connection per host.
    pub fn connecting(&self, key: &Key) -> Connecting {
        let mut inner = self.inner.lock().unwrap();

        if let Some(sender) = inner.connecting.get(key) {
            Connecting::InProgress(ConnectingWaiter {
                receiver: sender.subscribe(),
            })
        } else {
            let (tx, _) = watch::channel(None);
            inner.connecting.insert(key.clone(), tx);
            Connecting::Acquired(ConnectingLock::new(key.clone(), Arc::clone(&self.inner)))
        }
    }

    pub fn try_pool(&self, key: &Key) -> Option<PoolClient> {
        let mut inner = self.inner.lock().unwrap();
        let timeout = inner.timeout;
        let unusable = inner.idle_conns.get(key).is_some_and(|conn| {
            // remove the connection from the pool if invalid or expired
            if conn.is_invalid() {
                return true;
            }

            if let Some(duration) = timeout {
                // Only idle connections (no active streams) are considered for
                // expiry — mirrors H1/H2 `busy == 0` / `active_streams == 0`
                // gating so a long-held borrow isn't evicted mid-stream.
                if conn.active_streams.load(Ordering::Acquire) == 0 {
                    let idle = *conn.idle_timeout.lock().unwrap_or_else(|p| p.into_inner());
                    if Instant::now().saturating_duration_since(idle) > duration {
                        return true;
                    }
                }
            }

            false
        });

        if unusable {
            trace!("removing unusable pooled HTTP/3 connection...");
            if let Some(mut conn) = inner.idle_conns.remove(key) {
                conn.detach_driver_until_idle();
            }
            return None;
        }

        inner.idle_conns.get_mut(key).map(|conn| conn.pool())
    }

    pub fn new_connection(
        &mut self,
        lock: ConnectingLock,
        mut driver: h3::client::Connection<Connection, Bytes>,
        tx: SendRequest<OpenStreams, Bytes>,
    ) -> PoolClient {
        let (close_tx, close_rx) = std::sync::mpsc::channel();
        let connection_task = tokio::spawn(async move {
            let e = future::poll_fn(|cx| driver.poll_close(cx)).await;
            trace!("poll_close returned error {e:?}");
            close_tx.send(e).ok();
        });

        let active_streams = Arc::new(AtomicUsize::new(0));
        let stream_completed = Arc::new(Notify::new());
        let idle_timeout = Arc::new(Mutex::new(Instant::now()));

        let mut inner = self.inner.lock().unwrap();

        // We clean up "connecting" here so we don't have to acquire the lock again.
        let key = lock.forget();
        let Some(notifier) = inner.connecting.remove(&key) else {
            unreachable!("there should be one connecting lock at a time");
        };
        let template = PoolClient::new(tx);

        // Send a borrower to awaiters; if there are none, the unsent
        // borrower is dropped and its increment undone.
        let _ = notifier.send(Some(template.checked_out(
            &active_streams,
            &stream_completed,
            &idle_timeout,
        )));

        let conn = PoolConnection::new(
            template,
            close_rx,
            connection_task,
            active_streams,
            stream_completed,
            idle_timeout,
        );
        inner.insert(key.clone(), conn);
        drop(inner);

        // The caller is a borrower too: it holds the response body as it
        // streams on this connection.
        self.inner
            .lock()
            .unwrap()
            .idle_conns
            .get_mut(&key)
            .map(PoolConnection::pool)
            .expect("just inserted")
    }
}

struct PoolInner {
    connecting: HashMap<Key, watch::Sender<Option<PoolClient>>>,
    idle_conns: HashMap<Key, PoolConnection>,
    timeout: Option<Duration>,
}

impl PoolInner {
    fn insert(&mut self, key: Key, conn: PoolConnection) {
        if let Some(mut old) = self.idle_conns.remove(&key) {
            trace!("h3 pool: replacing existing connection for {key:?}");
            old.detach_driver_until_idle();
        }
        self.idle_conns.insert(key, conn);
    }
}

pub struct PoolClient {
    inner: SendRequest<OpenStreams, Bytes>,
    /// None for the pool-stored template client; Some for borrowers.
    /// Borrower Drop decrements `active_streams`.
    active_streams: Option<Arc<AtomicUsize>>,
    stream_completed: Option<Arc<Notify>>,
    idle_timeout: Option<Arc<Mutex<Instant>>>,
}

impl Drop for PoolClient {
    fn drop(&mut self) {
        // Only borrowers carry counters; the template never increments.
        let Some(count) = &self.active_streams else {
            return;
        };
        let mut prev = count.load(Ordering::Acquire);
        loop {
            if prev == 0 {
                break;
            }
            match count.compare_exchange(prev, prev - 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(old) => {
                    // last borrower gone: connection is idle again
                    if old == 1 {
                        if let Some(ref idle) = self.idle_timeout {
                            *idle.lock().unwrap_or_else(|p| p.into_inner()) = Instant::now();
                        }
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

impl Clone for PoolClient {
    fn clone(&self) -> Self {
        // Cloning a borrower counts as another borrower (e.g. an awaiter of
        // the connecting-watch channel); each clone's Drop decrements once.
        let client = Self {
            inner: self.inner.clone(),
            active_streams: self.active_streams.clone(),
            stream_completed: self.stream_completed.clone(),
            idle_timeout: self.idle_timeout.clone(),
        };
        if let Some(ref count) = client.active_streams {
            count.fetch_add(1, Ordering::AcqRel);
        }
        client
    }
}

impl PoolClient {
    pub fn new(tx: SendRequest<OpenStreams, Bytes>) -> Self {
        Self {
            inner: tx,
            active_streams: None,
            stream_completed: None,
            idle_timeout: None,
        }
    }

    /// Clone the template as a counted borrower: attach the connection's
    /// shared counters and increment `active_streams`.
    fn checked_out(
        &self,
        active_streams: &Arc<AtomicUsize>,
        stream_completed: &Arc<Notify>,
        idle_timeout: &Arc<Mutex<Instant>>,
    ) -> Self {
        let client = Self {
            inner: self.inner.clone(),
            active_streams: Some(Arc::clone(active_streams)),
            stream_completed: Some(Arc::clone(stream_completed)),
            idle_timeout: Some(Arc::clone(idle_timeout)),
        };
        active_streams.fetch_add(1, Ordering::AcqRel);
        client
    }

    pub async fn send_request(
        &mut self,
        req: Request<Body>,
    ) -> Result<Response<ResponseBody>, BoxError> {
        use hyper::body::Body as _;

        let (head, mut req_body) = req.into_parts();
        let mut req = Request::from_parts(head, ());

        if let Some(n) = req_body.size_hint().exact() {
            if n > 0 {
                req.headers_mut()
                    .insert(http::header::CONTENT_LENGTH, n.into());
            }
        }

        let (mut send, mut recv) = self.inner.send_request(req).await?.split();

        let (tx, mut rx) = oneshot::channel::<Result<(), BoxError>>();
        tokio::spawn(async move {
            let mut req_body = Pin::new(&mut req_body);
            loop {
                match std::future::poll_fn(|cx| req_body.as_mut().poll_frame(cx)).await {
                    Some(Ok(frame)) => {
                        if let Ok(b) = frame.into_data() {
                            if let Err(e) = send.send_data(Bytes::copy_from_slice(&b)).await {
                                if is_stop_sending(&e) {
                                    let _ = tx.send(Ok(()));
                                    return;
                                }
                                if let Err(e) = tx.send(Err(e.into())) {
                                    error!("Failed to communicate send.send_data() error: {e:?}");
                                }
                                return;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        if let Err(e) = tx.send(Err(e.into())) {
                            error!("Failed to communicate req_body read error: {e:?}");
                        }
                        return;
                    }

                    None => break,
                }
            }

            if let Err(e) = send.finish().await {
                if !is_stop_sending(&e) {
                    if let Err(e) = tx.send(Err(e.into())) {
                        error!("Failed to communicate send.finish read error: {e:?}");
                    }
                    return;
                }
            }

            let _ = tx.send(Ok(()));
        });

        tokio::select! {
            Ok(Err(e)) = &mut rx => Err(e),
            resp = recv.recv_response() => {
                let resp = resp?;
                let resp_body = crate::async_impl::body::boxed(Incoming::new(
                    recv,
                    resp.headers(),
                    rx,
                    self.active_streams.clone(),
                    self.stream_completed.clone(),
                    self.idle_timeout.clone(),
                ));
                Ok(resp.map(|_| resp_body))
            }
        }
    }
}

pub struct PoolConnection {
    // This receives errors from polling h3 driver.
    close_rx: Receiver<h3::error::ConnectionError>,
    /// Pool-stored template client (uncounted); borrowers are clones via
    /// [`PoolConnection::pool`].
    client: PoolClient,
    idle_timeout: Arc<Mutex<Instant>>,
    /// Driver task handle, aborted on eviction or pool drop (the driver
    /// holds the quinn connection — without aborting, it leaks).
    connection_task: Option<tokio::task::JoinHandle<()>>,
    /// Borrow count (checked-out `PoolClient`s + response bodies); at 0 the
    /// connection is truly idle and its driver can be aborted.
    active_streams: Arc<AtomicUsize>,
    /// Notifies waiters when the connection becomes idle (active_streams → 0).
    stream_completed: Arc<Notify>,
}

impl PoolConnection {
    pub fn new(
        client: PoolClient,
        close_rx: Receiver<h3::error::ConnectionError>,
        connection_task: tokio::task::JoinHandle<()>,
        active_streams: Arc<AtomicUsize>,
        stream_completed: Arc<Notify>,
        idle_timeout: Arc<Mutex<Instant>>,
    ) -> Self {
        Self {
            close_rx,
            client,
            idle_timeout,
            connection_task: Some(connection_task),
            active_streams,
            stream_completed,
        }
    }

    pub fn pool(&mut self) -> PoolClient {
        *self.idle_timeout.lock().unwrap_or_else(|p| p.into_inner()) = Instant::now();
        self.client.checked_out(
            &self.active_streams,
            &self.stream_completed,
            &self.idle_timeout,
        )
    }

    pub fn is_invalid(&self) -> bool {
        match self.close_rx.try_recv() {
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => true,
            Ok(_) => true,
        }
    }

    /// Abort the driver once the connection is truly idle; if streams are
    /// still active, a watcher task aborts when `active_streams` hits 0
    /// (aborting mid-stream would kill in-flight responses).
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

struct Incoming<S, B> {
    inner: h3::client::RequestStream<S, B>,
    content_length: Option<u64>,
    send_rx: oneshot::Receiver<Result<(), BoxError>>,
    /// Shared counters cloned from the borrower: keep the connection busy
    /// while the body streams, so the driver is not aborted mid-body.
    active_streams: Option<Arc<AtomicUsize>>,
    stream_completed: Option<Arc<Notify>>,
    idle_timeout: Option<Arc<Mutex<Instant>>>,
}

impl<S, B> Incoming<S, B> {
    fn new(
        stream: h3::client::RequestStream<S, B>,
        headers: &http::header::HeaderMap,
        send_rx: oneshot::Receiver<Result<(), BoxError>>,
        active_streams: Option<Arc<AtomicUsize>>,
        stream_completed: Option<Arc<Notify>>,
        idle_timeout: Option<Arc<Mutex<Instant>>>,
    ) -> Self {
        if let Some(ref count) = active_streams {
            count.fetch_add(1, Ordering::AcqRel);
        }
        Self {
            inner: stream,
            content_length: headers
                .get(http::header::CONTENT_LENGTH)
                .and_then(|h| h.to_str().ok())
                .and_then(|v| v.parse().ok()),
            send_rx,
            active_streams,
            stream_completed,
            idle_timeout,
        }
    }
}

impl<S, B> Drop for Incoming<S, B> {
    fn drop(&mut self) {
        let Some(count) = &self.active_streams else {
            return;
        };
        let mut prev = count.load(Ordering::Acquire);
        loop {
            if prev == 0 {
                break;
            }
            match count.compare_exchange(prev, prev - 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(old) => {
                    if old == 1 {
                        if let Some(ref idle) = self.idle_timeout {
                            *idle.lock().unwrap_or_else(|p| p.into_inner()) = Instant::now();
                        }
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

impl<S, B> http_body::Body for Incoming<S, B>
where
    S: h3::quic::RecvStream,
{
    type Data = Bytes;
    type Error = crate::error::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        if let Ok(Err(e)) = self.send_rx.try_recv() {
            return Poll::Ready(Some(Err(crate::error::body(e))));
        }

        match ready!(self.inner.poll_recv_data(cx)) {
            Ok(Some(mut b)) => Poll::Ready(Some(Ok(hyper::body::Frame::data(
                b.copy_to_bytes(b.remaining()),
            )))),
            Ok(None) => Poll::Ready(None),
            Err(e) => Poll::Ready(Some(Err(crate::error::body(e)))),
        }
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        if let Some(content_length) = self.content_length {
            hyper::body::SizeHint::with_exact(content_length)
        } else {
            hyper::body::SizeHint::default()
        }
    }
}

pub(crate) fn extract_domain(uri: &mut Uri) -> Result<Key, Error> {
    let uri_clone = uri.clone();
    match (uri_clone.scheme(), uri_clone.authority()) {
        (Some(scheme), Some(auth)) => {
            let scheme_str = scheme.as_str();
            if scheme_str != "https" && scheme_str != "h3" {
                return Err(Error::new(
                    Kind::Request,
                    Some(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "HTTP/3 only supports 'https' or 'h3' schemes, got: {}",
                            scheme_str
                        ),
                    ))),
                ));
            }
            Ok((scheme.clone(), auth.clone()))
        }
        _ => Err(Error::new(Kind::Request, None::<Error>)),
    }
}

pub(crate) fn domain_as_uri((scheme, auth): Key) -> Result<Uri, BoxError> {
    http::uri::Builder::new()
        .scheme(scheme)
        .authority(auth)
        .path_and_query("/")
        .build()
        .map_err(BoxError::from)
}

/// True if the remote requested the peer stop sending without error.
fn is_stop_sending(e: &h3::error::StreamError) -> bool {
    matches!(
        e,
        h3::error::StreamError::RemoteTerminate {
            code: h3::error::Code::H3_NO_ERROR,
            ..
        }
    )
}
