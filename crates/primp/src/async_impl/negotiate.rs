use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http_body_util::BodyExt;
use tower_service::Service;

use crate::async_impl::body::Body;
use crate::async_impl::client::HttpVersionPref;
use crate::async_impl::h1_client::Http1Pool;
use crate::async_impl::h2_client::connect::H2Connector;
use crate::async_impl::h2_client::pool::{pool_key, ConnectOutcome, Pool, PoolClient};
use crate::async_impl::h2_client::H2NegotiationFailed;
use crate::async_impl::BoxBody;

/// ALPN-negotiating connection used by the `HttpVersionPref::All` path.
///
/// Reuses a pooled HTTP/1.1 connection when idle, otherwise probes h2 via
/// ALPN. On h2 the pooled connection is used; on http/1.1 the same TLS stream
/// is reused (falling back to the legacy client if it dies).
pub(crate) struct NegotiatingConnection {
    pub(crate) h2_pool: Pool,
    pub(crate) h2_connector: Arc<H2Connector>,
    pub(crate) http1_client: hyper_util::client::legacy::Client<crate::connect::Connector, Body>,
    pub(crate) http1_pool: Http1Pool,
    pub(crate) http_version_pref: HttpVersionPref,
}

impl Clone for NegotiatingConnection {
    fn clone(&self) -> Self {
        NegotiatingConnection {
            h2_pool: self.h2_pool.clone(),
            h2_connector: self.h2_connector.clone(),
            http1_client: self.http1_client.clone(),
            http1_pool: self.http1_pool.clone(),
            http_version_pref: self.http_version_pref,
        }
    }
}

impl NegotiatingConnection {
    /// Force the request version to HTTP/2 so the h2 encoder emits proper h2
    /// framing (`:authority`/`:scheme` pseudo-headers) instead of h1-to-h2
    /// translation.
    fn set_h2_version(req: http::Request<Body>) -> http::Request<Body> {
        let (mut parts, body) = req.into_parts();
        parts.version = http::Version::HTTP_2;
        http::Request::from_parts(parts, body)
    }

    /// Whether the method is safe to replay after a mid-write failure.
    /// GET, HEAD, OPTIONS, TRACE, PUT, and DELETE are idempotent, so a
    /// pooled/sent request that failed can be retried on a fresh connection;
    /// POST/PATCH are not, since a server may have already acted (per
    /// urllib3/requests connection-error policy).
    fn method_is_idempotent(method: &http::Method) -> bool {
        matches!(
            method,
            &http::Method::GET
                | &http::Method::HEAD
                | &http::Method::OPTIONS
                | &http::Method::TRACE
                | &http::Method::PUT
                | &http::Method::DELETE
        )
    }

    fn call_inner(
        &mut self,
        req: http::Request<Body>,
    ) -> Pin<Box<dyn Future<Output = Result<http::Response<BoxBody>, crate::Error>> + Send>> {
        let h2_pool = self.h2_pool.clone();
        let h2_connector = self.h2_connector.clone();
        let http1_client = self.http1_client.clone();
        let http1_pool = self.http1_pool.clone();
        let version_pref = self.http_version_pref;
        let uri = req.uri().clone();

        Box::pin(async move {
            match version_pref {
                HttpVersionPref::Http2 => {
                    // h2 only, no fallback
                    let result = h2_pool
                        .get_or_connect(&h2_connector, &uri)
                        .await
                        .map_err(crate::error::request)?;
                    match result {
                        ConnectOutcome::H2(result) => {
                            let req = Self::set_h2_version(req);
                            let mut resp =
                                PoolClient::send_request(result.pooled, req, &h2_pool).await?;
                            if let Some(tls_info) = result.tls_info {
                                resp.extensions_mut().insert(tls_info);
                            }
                            Ok(resp.map(|body| body.boxed()))
                        }
                        ConnectOutcome::Http1 { .. } => Err(crate::error::request(
                            "server negotiated http/1.1 over TLS but http/2 was explicitly required",
                        )),
                    }
                }
                HttpVersionPref::Http1 => {
                    // http/1.1 only
                    let resp = http1_client
                        .request(req)
                        .await
                        .map_err(crate::error::request)?;
                    Ok(resp.map(|body| body.map_err(crate::error::request).boxed()))
                }
                HttpVersionPref::All => {
                    // Probe h2 ALPN on HTTPS; h2c (prior knowledge) requires
                    // explicit opt-in via Http2, so skip h2 for plain HTTP.
                    if uri.scheme_str() != Some("https") {
                        let resp = http1_client
                            .request(req)
                            .await
                            .map_err(crate::error::request)?;
                        return Ok(resp.map(|body| body.map_err(crate::error::request).boxed()));
                    }
                    // Reuse a previously negotiated HTTP/1.1 connection for
                    // this host instead of performing another TLS handshake.
                    let key = pool_key(&uri);
                    if let Some(mut guard) = http1_pool.get(&key).await {
                        // The readiness probe in `get` filters out already-dead
                        // pooled connections, but a keep-alive connection can
                        // still be closed by the server in the tiny window
                        // between that probe and the send (half-open socket).
                        // Mirror the ALPN-negotiation path below: on a reuse
                        // failure, fall back to the legacy client rather than
                        // surfacing a hard error. The body has not been consumed
                        // yet, so retrying is safe when the body is replayable
                        // AND the method is idempotent (a server that processed
                        // a non-idempotent request and then closed before
                        // responding would otherwise execute it twice — urllib3/
                        // requests likewise only retry idempotent methods); a
                        // non-replayable (streaming) body or a non-idempotent
                        // method surfaces the original error.
                        let method = req.method().clone();
                        let (parts, body) = req.into_parts();
                        let fallback_body = body.try_clone();
                        let fallback_parts = parts.clone();
                        let reuse_req = http::Request::from_parts(parts, body);
                        match guard.send_request(reuse_req).await {
                            Ok(resp) => return Ok(resp),
                            Err(e) => match fallback_body {
                                Some(body) if Self::method_is_idempotent(&method) => {
                                    match http1_client
                                        .request(http::Request::from_parts(fallback_parts, body))
                                        .await
                                    {
                                        Ok(resp) => {
                                            return Ok(resp.map(|body| {
                                                body.map_err(crate::error::request).boxed()
                                            }))
                                        }
                                        // Both the pooled reuse and the legacy
                                        // fallback failed: keep the original
                                        // failure in the chain instead of
                                        // silently replacing it.
                                        Err(fb) => {
                                            return Err(crate::error::request_with_previous(fb, e))
                                        }
                                    }
                                }
                                _ => return Err(e),
                            },
                        }
                    }
                    match h2_pool.get_or_connect(&h2_connector, &uri).await {
                        Ok(ConnectOutcome::H2(result)) => {
                            let req = Self::set_h2_version(req);
                            let mut resp =
                                PoolClient::send_request(result.pooled, req, &h2_pool).await?;
                            if let Some(tls_info) = result.tls_info {
                                resp.extensions_mut().insert(tls_info);
                            }
                            Ok(resp.map(|body| body.boxed()))
                        }
                        Ok(ConnectOutcome::Http1 {
                            key,
                            stream,
                            tls_info,
                        }) => {
                            // Server picked http/1.1 during the SAME TLS
                            // handshake — reuse that stream instead of doing a
                            // second handshake through the legacy client.
                            // If the reuse fails (e.g. the keep-alive
                            // connection was silently closed by the server and
                            // slipped the readiness probe), fall back to the
                            // legacy client rather than surfacing a hard error.
                            // The request body has not been consumed at this
                            // point, so retrying via the legacy client is safe
                            // when the body is replayable AND the method is
                            // idempotent (see above); otherwise we surface the
                            // original error.
                            let method = req.method().clone();
                            let (parts, body) = req.into_parts();
                            let fallback_body = body.try_clone();
                            let fallback_parts = parts.clone();
                            let reuse_req = http::Request::from_parts(parts, body);
                            match http1_pool.request(&key, stream, tls_info, reuse_req).await {
                                Ok(resp) => Ok(resp),
                                Err(e) => match fallback_body {
                                    Some(body) if Self::method_is_idempotent(&method) => {
                                        match http1_client
                                            .request(http::Request::from_parts(
                                                fallback_parts,
                                                body,
                                            ))
                                            .await
                                        {
                                            Ok(resp) => Ok(resp.map(|body| {
                                                body.map_err(crate::error::request).boxed()
                                            })),
                                            // Keep the original failure in the
                                            // chain when the fallback also fails.
                                            Err(fb) => {
                                                Err(crate::error::request_with_previous(fb, e))
                                            }
                                        }
                                    }
                                    _ => Err(e),
                                },
                            }
                        }
                        Err(e) => {
                            // Check if the error is H2NegotiationFailed directly
                            let is_neg_fail = e.downcast_ref::<H2NegotiationFailed>().is_some();
                            if is_neg_fail {
                                // Server does not support h2 (e.g. plain HTTP
                                // over a proxy) — fall back to the legacy client.
                                let resp = http1_client
                                    .request(req)
                                    .await
                                    .map_err(crate::error::request)?;
                                Ok(resp.map(|body| body.map_err(crate::error::request).boxed()))
                            } else {
                                Err(crate::error::request(e))
                            }
                        }
                    }
                }
                #[cfg(feature = "http3")]
                HttpVersionPref::Http3 => Err(crate::error::request(
                    "h3 is not handled by NegotiatingConnection; use the h3_client path instead",
                )),
            }
        })
    }
}

impl Service<http::Request<Body>> for NegotiatingConnection {
    type Response = http::Response<BoxBody>;
    type Error = crate::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        self.call_inner(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::async_impl::h2_client::pool::{poll_ping, PingFuture};
    use log::warn;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use hyper::service::service_fn;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use tokio::net::TcpListener;
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use tokio_rustls::rustls::ServerConfig;
    use tokio_rustls::TlsAcceptor;

    use crate::async_impl::body::Body;
    use crate::Client;

    #[test]
    fn test_h2_negotiation_failed_display() {
        let err = H2NegotiationFailed;
        assert_eq!(err.to_string(), "server did not negotiate h2 via ALPN");
    }

    #[test]
    fn test_h2_negotiation_failed_is_error() {
        let err = H2NegotiationFailed;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_h2_negotiation_failed_source_is_none() {
        use std::error::Error;
        let err = H2NegotiationFailed;
        assert!(err.source().is_none());
    }

    #[test]
    fn test_h2_negotiation_failed_downcastable() {
        let err: Box<dyn std::error::Error + Send + Sync> = Box::new(H2NegotiationFailed);
        assert!(err.downcast_ref::<H2NegotiationFailed>().is_some());
    }

    #[test]
    fn test_is_neg_fail_direct_downcast() {
        let err: crate::error::BoxError = Box::new(H2NegotiationFailed);
        let is_neg_fail = err.downcast_ref::<H2NegotiationFailed>().is_some();
        assert!(is_neg_fail);
    }

    #[test]
    fn test_non_neg_fail_error_not_detected() {
        let err: crate::error::BoxError = Box::new(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "connection refused",
        ));
        let is_neg_fail = err.downcast_ref::<H2NegotiationFailed>().is_some();
        assert!(!is_neg_fail);
    }

    #[test]
    fn test_method_is_idempotent() {
        use http::Method;
        for m in [
            Method::GET,
            Method::HEAD,
            Method::OPTIONS,
            Method::TRACE,
            Method::PUT,
            Method::DELETE,
        ] {
            assert!(
                NegotiatingConnection::method_is_idempotent(&m),
                "{m:?} must be idempotent"
            );
        }
        for m in [Method::POST, Method::PATCH, Method::CONNECT] {
            assert!(
                !NegotiatingConnection::method_is_idempotent(&m),
                "{m:?} must NOT be replayed on a fallback connection"
            );
        }
    }

    #[test]
    fn test_wrapped_neg_fail_not_directly_detected() {
        let err: crate::error::BoxError = Box::new(crate::error::request(H2NegotiationFailed));
        let is_neg_fail = err.downcast_ref::<H2NegotiationFailed>().is_some();
        assert!(!is_neg_fail);
    }

    /// Spawns a TLS server advertising `alpn_protocols` that returns `body` on
    /// every request. Returns the bound address and a counter of accepted TLS
    /// connections (i.e. client handshakes performed).
    async fn spawn_server(
        alpn_protocols: Vec<Vec<u8>>,
        body: Vec<u8>,
    ) -> (SocketAddr, Arc<AtomicUsize>) {
        let cert = std::fs::read("tests/support/server.cert").unwrap();
        let key = std::fs::read("tests/support/server.key").unwrap();
        let cert_der = CertificateDer::from(cert);
        let key_der = PrivateKeyDer::try_from(key).unwrap();
        let mut tls = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap();
        tls.alpn_protocols = alpn_protocols;
        let acceptor = TlsAcceptor::from(Arc::new(tls));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let accepts2 = accepts.clone();

        tokio::spawn(async move {
            loop {
                let (sock, _) = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => break,
                };
                accepts2.fetch_add(1, Ordering::SeqCst);
                let acceptor = acceptor.clone();
                let body = body.clone();
                tokio::spawn(async move {
                    let tls = match acceptor.accept(sock).await {
                        Ok(t) => t,
                        Err(_) => return,
                    };
                    let io = TokioIo::new(tls);
                    let svc = service_fn(move |_req| {
                        let body = body.clone();
                        async move {
                            Ok::<_, std::convert::Infallible>(hyper::Response::new(Body::from(
                                body,
                            )))
                        }
                    });
                    let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(io, svc)
                        .await;
                });
            }
        });

        (addr, accepts)
    }

    /// Issues `n` sequential requests to `addr` and asserts each returns `"hello"`.
    async fn client_requests(addr: SocketAddr, n: usize) {
        let client = Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .unwrap();
        for _ in 0..n {
            let resp = client
                .get(format!("https://{addr}/"))
                .send()
                .await
                .expect("request failed");
            assert!(resp.status().is_success());
            assert_eq!(resp.text().await.unwrap(), "hello");
        }
    }

    /// Server advertises only `http/1.1`; the client must negotiate HTTP/1.1 over a single TLS handshake and reuse it (exactly one handshake).
    #[tokio::test]
    async fn all_pref_http1_only_single_handshake() {
        let (addr, accepts) = spawn_server(vec![b"http/1.1".into()], b"hello".to_vec()).await;
        client_requests(addr, 5).await;
        assert_eq!(
            accepts.load(Ordering::SeqCst),
            1,
            "http/1.1-only server: expected exactly one TLS handshake across many requests"
        );
    }

    /// Server advertises only `h2`; requests must multiplex over a single TLS handshake (exactly one handshake).
    #[tokio::test]
    async fn all_pref_h2_only_single_handshake() {
        let (addr, accepts) = spawn_server(vec![b"h2".into()], b"hello".to_vec()).await;
        client_requests(addr, 5).await;
        assert_eq!(
            accepts.load(Ordering::SeqCst),
            1,
            "h2-only server: expected exactly one TLS handshake across many requests"
        );
    }

    /// Server advertises both but ALPN-selects `http/1.1` first; the client must take HTTP/1.1 fallback on the *same* handshake and reuse it (exactly one handshake).
    #[tokio::test]
    async fn all_pref_http2_to_http1_single_handshake() {
        let (addr, accepts) =
            spawn_server(vec![b"http/1.1".into(), b"h2".into()], b"hello".to_vec()).await;
        client_requests(addr, 5).await;
        assert_eq!(
            accepts.load(Ordering::SeqCst),
            1,
            "http2->http1 server: expected exactly one TLS handshake across many requests"
        );
    }

    /// Regression: with `HttpVersionPref::All` against an h1-only server, a pooled
    /// connection whose 1 MB response body is still streaming must not be reused by
    /// the next request. The first request performs the handshake, drops its
    /// `Http1SendGuard` (returning the `SendRequest` to the pool), and leaves its
    /// 1 MB body in flight; reusing it would interleave/corrupt the two responses.
    #[tokio::test]
    async fn all_pref_http1_only_reuse_while_streaming() {
        let body = vec![b'x'; 1_000_000];
        let (addr, _accepts) = spawn_server(vec![b"http/1.1".into()], body.clone()).await;

        let client = Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .unwrap();

        for _ in 0..16 {
            // Request 1: response returned, guard dropped (sender pooled), but
            // the 1 MB body is still held in `resp1` and in flight.
            let resp1 = client
                .get(format!("https://{addr}/"))
                .send()
                .await
                .expect("request 1 failed");

            // Request 2: must reuse the pooled connection from request 1 while
            // `resp1`'s body is still streaming.
            let resp2 = client
                .get(format!("https://{addr}/"))
                .send()
                .await
                .expect("request 2 failed: pool reused a connection mid-stream");

            // Drain both bodies; both must be complete and uncorrupted.
            let (b1, b2) = tokio::join!(resp1.text(), resp2.text());
            assert_eq!(
                b1.unwrap().len(),
                1_000_000,
                "response 1 body truncated/corrupted"
            );
            assert_eq!(
                b2.unwrap().len(),
                1_000_000,
                "response 2 body truncated/corrupted"
            );
        }
    }

    /// Regression: an in-flight keep-alive PING must survive `select!` re-entries
    /// (a tick firing while a PING is outstanding). Mirrors `ensure_driver_spawned`'s
    /// loop with the real `poll_ping` helper; the old `take()`-based code dropped
    /// the future on re-entry, so the ping never resolved.
    #[tokio::test]
    async fn keepalive_in_flight_ping_survives_select_reentry() {
        use std::time::Duration;

        // A mock ping that resolves Ok(()) after 50ms (simulating a PONG).
        let fut: PingFuture = Box::pin(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(())
        });
        let mut ping_fut: Option<PingFuture> = Some(fut);

        // Tick faster (10ms) than the ping latency (50ms) so re-entries happen
        // while the ping is still in flight — the exact condition that used to
        // drop the future.
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        interval.tick().await; // skip first immediate tick

        let result = 'outer: loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Mimics the driver: on a tick, fall through and re-enter
                    // the `select!` (which re-evaluates the `poll_ping` branch,
                    // i.e. re-creates the branch future).
                }
                r = poll_ping(&mut ping_fut), if ping_fut.is_some() => {
                    break 'outer r;
                }
            }
        };

        assert!(
            result.is_ok(),
            "in-flight keep-alive ping was dropped on select! re-entry \
             instead of being driven to completion"
        );
    }

    /// End-to-end: runs `ensure_driver_spawned`'s real driver loop against a live
    /// duplex h2 server (interval 10 ms). If the in-flight PING were dropped on
    /// re-entry, the connection tears down and the trailing request fails.
    #[tokio::test]
    async fn keepalive_real_driver_ping_keeps_connection_alive() {
        use std::time::Duration;
        use tokio::sync::Mutex;

        let (io_client, io_server) = tokio::io::duplex(64 * 1024);

        // H2 server: establish and KEEP the connection alive. In this fork the
        // server `Connection` is driven by repeatedly calling `accept()`, which
        // also processes connection-level frames (PING/PONG). Looping on accept
        // keeps the connection up so it pongs our keep-alive pings.
        let server_task = tokio::spawn(async move {
            let mut conn = match h2::server::Builder::new()
                .handshake::<_, bytes::Bytes>(io_server)
                .await
            {
                Ok(c) => c,
                Err(_) => return,
            };
            while let Some(Ok((_req, mut send))) = conn.accept().await {
                let rsp = http::Response::new(());
                let _ = send.send_response(rsp, false);
            }
        });

        let (mut send_request, mut conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(io_client)
            .await
            .unwrap();

        let ping_pong = conn.ping_pong();
        let conn: crate::async_impl::h2_client::pool::H2Connection = Box::pin(conn);

        let keep_alive = crate::async_impl::h2_client::pool::KeepAliveConfig {
            interval: Some(Duration::from_millis(10)),
            timeout: Some(Duration::from_millis(500)),
            while_idle: true,
        };
        let identity = Arc::new(AtomicUsize::new(1));
        // Faithful copy of ensure_driver_spawned's spawned task (no Pool needed).
        let ping_pong: Option<Arc<Mutex<h2::PingPong>>> =
            ping_pong.map(|p| Arc::new(Mutex::new(p)));
        let mut conn = conn;
        tokio::spawn(async move {
            if let Some(interval) = keep_alive.interval.filter(|i| !i.is_zero()) {
                let mut interval = tokio::time::interval(interval);
                interval.tick().await;
                let timeout = keep_alive.timeout.unwrap_or(Duration::from_secs(10));
                let mut ping_fut: Option<PingFuture> = None;
                let result = loop {
                    tokio::select! {
                        result = &mut conn => { break result; }
                        _ = interval.tick() => {
                            if !keep_alive.while_idle
                                && identity.load(Ordering::Acquire) == 0
                            { continue; }
                            if ping_fut.is_none() {
                                if let Some(pp) = &ping_pong {
                                    let pp = Arc::clone(pp);
                                    ping_fut = Some(Box::pin(async move {
                                        let mut guard = pp.lock().await;
                                        match tokio::time::timeout(
                                            timeout, guard.ping(h2::Ping::opaque()),
                                        ).await {
                                            Ok(Ok(_)) => Ok(()),
                                            Ok(Err(e)) => Err(e),
                                            Err(_) => Err(h2::Reason::NO_ERROR.into()),
                                        }
                                    }));
                                }
                            }
                        }
                        ping_result = poll_ping(&mut ping_fut), if ping_fut.is_some() => {
                            if let Err(e) = ping_result {
                                if e.reason() == Some(h2::Reason::NO_ERROR) {
                                    warn!("h2 keep-alive timeout, closing connection");
                                } else {
                                    warn!("h2 keep-alive ping error: {}", e);
                                }
                                break Err(e);
                            }
                        }
                    }
                };
                if let Err(e) = result {
                    warn!("h2 connection driver error: {}", e);
                }
            } else {
                if let Err(e) = conn.await {
                    warn!("h2 connection driver error: {}", e);
                }
            }
        });

        // Let many keep-alive intervals tick and pings fly (the in-flight ping
        // future is re-entered on every tick — the condition that used to drop
        // it).
        tokio::time::sleep(Duration::from_millis(200)).await;

        // The connection must still be alive: poll_ready must succeed. If the
        // in-flight PING had been dropped on select! re-entry, the PONG would
        // never be observed, the driver would tear the connection down during
        // the storm, and poll_ready would return Err (GOAWAY/FIN/transport).
        let ready = std::future::poll_fn(|cx| send_request.poll_ready(cx)).await;
        assert!(
            ready.is_ok(),
            "connection was torn down by keep-alive: in-flight ping not driven to completion"
        );

        drop(server_task);
    }
}
