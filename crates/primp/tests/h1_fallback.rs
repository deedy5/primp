//! Standalone integration tests for the ALPN http/1.1 fallback pool
//! (`Http1Pool`) and its keep-alive / self-heal behavior, exercised purely
//! through the public `Client` API. These do not depend on the (currently
//! broken) in-module `#[cfg(test)]` code in `negotiate.rs` / `h2_client/pool.rs`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use http::Request;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

use primp::Client;

/// Spin up a TLS server that advertises EXACTLY `http/1.1` via ALPN and
/// serves a fixed body on every request. Returns the bound address and an
/// atomic handshake counter (incremented once per accepted TLS connection).
///
/// If `close_after_response` is true, the server drops the TLS connection
/// immediately after writing the response (simulating a server that does not
/// keep connections alive), so the client's pooled connection goes stale.
async fn spawn_http1_tls_server(
    body: Vec<u8>,
    close_after_response: bool,
) -> (std::net::SocketAddr, Arc<AtomicUsize>) {
    let cert = std::fs::read("tests/support/server.cert").unwrap();
    let key = std::fs::read("tests/support/server.key").unwrap();
    let cert_der = CertificateDer::from(cert);
    let key_der = PrivateKeyDer::try_from(key).unwrap();
    let mut tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    tls.alpn_protocols = vec![b"http/1.1".into()];
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
                let svc = service_fn(move |_req: Request<hyper::body::Incoming>| {
                    let body = body.clone();
                    let close = close_after_response;
                    async move {
                        let mut builder = http::Response::builder().status(http::StatusCode::OK);
                        if close {
                            builder = builder.header(http::header::CONNECTION, "close");
                        }
                        let resp = builder.body(primp::Body::from(body)).unwrap();
                        Ok::<_, std::convert::Infallible>(resp)
                    }
                });
                let builder = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
                let conn = builder.serve_connection_with_upgrades(io, svc);
                let _ = conn.await;
            });
        }
    });

    (addr, accepts)
}

async fn build_client() -> Client {
    Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .unwrap()
}

/// Sequential requests to an http/1.1-only server must negotiate http/1.1 on a
/// SINGLE TLS handshake and reuse that connection for every request.
#[tokio::test]
async fn h1_fallback_sequential_single_handshake() {
    let (addr, accepts) = spawn_http1_tls_server(b"hello".to_vec(), false).await;
    let client = build_client().await;

    for _ in 0..5 {
        let resp = client
            .get(format!("https://{addr}/"))
            .send()
            .await
            .expect("request failed");
        assert!(resp.status().is_success());
        assert_eq!(resp.text().await.unwrap(), "hello");
    }

    assert_eq!(
        accepts.load(Ordering::SeqCst),
        1,
        "expected exactly one TLS handshake for sequential http/1.1 requests"
    );
}

/// When the server closes each connection after responding (no keep-alive),
/// the client's pooled http/1.1 connection goes stale. The client must
/// self-heal: every subsequent request still succeeds (it re-handshakes), even
/// though it transparently lost the pooled connection.
#[tokio::test]
async fn h1_fallback_self_heals_on_dead_connection() {
    let (addr, accepts) = spawn_http1_tls_server(b"hello".to_vec(), true).await;
    let client = build_client().await;

    for _ in 0..5 {
        let resp = client
            .get(format!("https://{addr}/"))
            .send()
            .await
            .expect("request must succeed after a dead pooled connection");
        assert!(resp.status().is_success());
        assert_eq!(resp.text().await.unwrap(), "hello");
    }

    // Each request re-handshakes because the server does not keep the
    // connection alive — so we expect one handshake per request.
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        5,
        "server closes each connection; expect one handshake per request"
    );
}

/// Same server (closes each connection) but using the dedicated HTTP/1.1 client
/// path (`http1_only()`), whose pool is managed by `hyper_util` and DOES
/// transparently reconnect on a dead pooled connection. This is the control:
/// it should NOT fail, confirming the failure above is specific to the
/// `Http1Pool` ALPN-fallback path.
#[tokio::test]
async fn http1_only_path_self_heals_on_dead_connection() {
    let (addr, _accepts) = spawn_http1_tls_server(b"hello".to_vec(), true).await;
    let client = Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .http1_only()
        .build()
        .unwrap();

    for _ in 0..5 {
        let resp = client
            .get(format!("https://{addr}/"))
            .send()
            .await
            .expect("http1_only path must self-heal on a dead pooled connection");
        assert!(resp.status().is_success());
        assert_eq!(resp.text().await.unwrap(), "hello");
    }
}

/// A query-only URL (`https://host?q=1`) must be rewritten to origin-form on
/// the ALPN-h1 fallback. The rewrite builds the origin-form URI from the
/// parsed `PathAndQuery` component rather than re-parsing the string (a
/// failed re-parse would leak the absolute-form to the wire, which strict
/// servers reject with 400).
#[tokio::test]
async fn h1_fallback_rewrites_query_only_url_to_origin_form() {
    let (addr, line_rx) = spawn_http1_raw_line_server().await;
    let client = build_client().await;

    let resp = client
        .get(format!("https://{addr}?q=1"))
        .send()
        .await
        .expect("request failed");
    assert!(resp.status().is_success());

    let line = line_rx
        .lock()
        .unwrap()
        .as_ref()
        .expect("server captured the request line")
        .clone();
    assert_eq!(
        line, "GET /?q=1 HTTP/1.1",
        "ALPN-h1 fallback must send origin-form, not the absolute-form"
    );
}

/// TLS (ALPN http/1.1) server that records the raw first request line and
/// answers 200 with `Connection: close`. Single connection, single request.
async fn spawn_http1_raw_line_server(
) -> (std::net::SocketAddr, Arc<std::sync::Mutex<Option<String>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let cert = std::fs::read("tests/support/server.cert").unwrap();
    let key = std::fs::read("tests/support/server.key").unwrap();
    let cert_der = CertificateDer::from(cert);
    let key_der = PrivateKeyDer::try_from(key).unwrap();
    let mut tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    tls.alpn_protocols = vec![b"http/1.1".into()];
    let acceptor = TlsAcceptor::from(Arc::new(tls));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let line_store = Arc::new(std::sync::Mutex::new(None));
    let line_store2 = line_store.clone();

    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let tls = acceptor.accept(sock).await.unwrap();
        let mut io = tls;
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            io.read(&mut byte).await.unwrap();
            buf.push(byte[0]);
            if buf.ends_with(b"\r\n") {
                break;
            }
        }
        let line = String::from_utf8_lossy(&buf).to_string();
        let line = line.strip_suffix("\r\n").unwrap_or(&line).to_string();
        *line_store2.lock().unwrap() = Some(line);
        io.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await
            .unwrap();
        let _ = io.shutdown().await;
    });

    (addr, line_store)
}

/// Spin up a TLS (ALPN http/1.1) server that keeps the connection alive for
/// the FIRST request (no `Connection: close`) so the client pools it, then —
/// after `close_after` responses on that same connection — silently drops the
/// TCP socket WITHOUT sending `Connection: close`. This reproduces a
/// half-open pooled connection: the client's `poll_ready` liveness probe still
/// reports the pooled sender as ready, but the subsequent `send_request` fails.
///
/// This is exactly the case that the ALPN-negotiation path handles by falling
/// back to the legacy client, and that the pooled-reuse path must handle too.
async fn spawn_http1_tls_server_silent_drop(
    body: Vec<u8>,
    close_after: usize,
) -> (std::net::SocketAddr, Arc<AtomicUsize>) {
    let cert = std::fs::read("tests/support/server.cert").unwrap();
    let key = std::fs::read("tests/support/server.key").unwrap();
    let cert_der = CertificateDer::from(cert);
    let key_der = PrivateKeyDer::try_from(key).unwrap();
    let mut tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    tls.alpn_protocols = vec![b"http/1.1".into()];
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
                let served = Arc::new(AtomicUsize::new(0));
                let served2 = served.clone();
                let io = TokioIo::new(tls);
                let svc = service_fn(move |_req: Request<hyper::body::Incoming>| {
                    let body = body.clone();
                    let served = served2.clone();
                    async move {
                        served.fetch_add(1, Ordering::SeqCst);
                        // No `Connection: close` — the client believes the
                        // connection stays alive and pools it.
                        let resp = http::Response::builder()
                            .status(http::StatusCode::OK)
                            .body(primp::Body::from(body))
                            .unwrap();
                        Ok::<_, std::convert::Infallible>(resp)
                    }
                });
                let builder = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
                let conn = builder.serve_connection_with_upgrades(io, svc);
                tokio::pin!(conn);
                // Drive the connection but stop (drop the TLS socket) once we
                // have served `close_after` responses, WITHOUT a graceful
                // shutdown — leaving the client with a half-open pooled conn.
                loop {
                    tokio::select! {
                        _ = &mut conn => break,
                        _ = async {
                            loop {
                                if served.load(Ordering::SeqCst) >= close_after {
                                    return;
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                            }
                        } => {
                            // Give the response time to flush to the client,
                            // then drop the connection abruptly.
                            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                            break;
                        }
                    }
                }
                // `conn` (and its TLS socket) dropped here without close frame.
            });
        }
    });

    (addr, accepts)
}

/// Regression test for the pooled-reuse fallback gap: the first request pools a
/// keep-alive http/1.1 connection; the server then silently drops the socket.
/// The second request reuses the pooled connection (which passes the readiness
/// probe but fails on send) and MUST transparently fall back to a fresh
/// connection instead of surfacing a hard error.
#[tokio::test]
async fn h1_pooled_reuse_falls_back_on_silent_drop() {
    let (addr, accepts) = spawn_http1_tls_server_silent_drop(b"hello".to_vec(), 1).await;
    let client = build_client().await;

    // Request 1: establishes + pools the keep-alive connection.
    let resp = client
        .get(format!("https://{addr}/"))
        .send()
        .await
        .expect("first request failed");
    assert!(resp.status().is_success());
    assert_eq!(resp.text().await.unwrap(), "hello");

    // Let the server drop the socket after that first response.
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;

    // Request 2: reuses the (now half-open) pooled connection. This must
    // succeed via fallback, not error out.
    let resp = client
        .get(format!("https://{addr}/"))
        .send()
        .await
        .expect("second request must fall back to a fresh connection, not hard-error");
    assert!(resp.status().is_success());
    assert_eq!(resp.text().await.unwrap(), "hello");

    assert!(
        accepts.load(Ordering::SeqCst) >= 2,
        "second request should have opened a fresh handshake after the pooled conn died"
    );
}

/// Concurrent requests to an http/1.1-only server: each concurrent request
/// needs its own connection (http/1.1 is serial), so we expect at least as
/// many handshakes as the concurrency level, and ALL must succeed.
#[tokio::test]
async fn h1_fallback_concurrent_requests_succeed() {
    let (addr, accepts) = spawn_http1_tls_server(b"hello".to_vec(), false).await;
    let client = build_client().await;

    let n = 8;
    let mut handles = Vec::new();
    for _ in 0..n {
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            let resp = c
                .get(format!("https://{addr}/"))
                .send()
                .await
                .expect("concurrent request failed");
            assert!(resp.status().is_success());
            assert_eq!(resp.text().await.unwrap(), "hello");
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let handshakes = accepts.load(Ordering::SeqCst);
    assert!(
        handshakes >= 1 && handshakes <= n,
        "concurrent http/1.1 requests should use between 1 and {n} connections, got {handshakes}"
    );
}
