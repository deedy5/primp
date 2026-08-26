//! Pool `Drop` race — aborting driver while body streams → `broken pipe`.
//! Needs many concurrent temp clients (100) to hit — guards on `active_streams`.

use primp::Body;
use std::time::Duration;

/// H2 TLS server streaming body in two frames with 50 ms delay.
async fn spawn_h2_server(body: Vec<u8>) -> std::net::SocketAddr {
    use hyper::service::service_fn;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use tokio_rustls::rustls::ServerConfig;
    use tokio_rustls::TlsAcceptor;

    let cert = std::fs::read("tests/support/server.cert").unwrap();
    let key = std::fs::read("tests/support/server.key").unwrap();
    let cert_der = CertificateDer::from(cert);
    let key_der = PrivateKeyDer::try_from(key).unwrap();
    let mut tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    tls.alpn_protocols = vec![b"h2".into()];
    let acceptor = TlsAcceptor::from(Arc::new(tls));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (sock, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let body = body.clone();
            tokio::spawn(async move {
                let tls = acceptor.accept(sock).await.unwrap();
                let io = TokioIo::new(tls);
                let svc = service_fn(move |_req| {
                    let body = body.clone();
                    async move {
                        let stream = futures_util::stream::unfold(0, move |state| {
                            let body = body.clone();
                            async move {
                                match state {
                                    0 => Some((
                                        Ok::<_, std::convert::Infallible>(bytes::Bytes::from(
                                            body[..body.len() / 2].to_vec(),
                                        )),
                                        1,
                                    )),
                                    1 => {
                                        tokio::time::sleep(Duration::from_millis(50)).await;
                                        Some((
                                            Ok::<_, std::convert::Infallible>(bytes::Bytes::from(
                                                body[body.len() / 2..].to_vec(),
                                            )),
                                            2,
                                        ))
                                    }
                                    _ => None,
                                }
                            }
                        });
                        let body = Body::wrap_stream(stream);
                        Ok::<_, std::convert::Infallible>(
                            http::Response::builder().status(200).body(body).unwrap(),
                        )
                    }
                });
                let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection_with_upgrades(io, svc)
                    .await;
            });
        }
    });
    addr
}

/// Many concurrent temp H2 clients dropped after headers must not `broken pipe`.
#[tokio::test]
async fn many_concurrent_temp_clients_do_not_cause_broken_pipe() {
    let addr = spawn_h2_server(b"hello world hello world hello world".to_vec()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let n = 100;
    let mut handles = vec![];
    for _ in 0..n {
        let addr = addr.clone();
        handles.push(tokio::spawn(async move {
            let client = primp::Client::builder()
                .no_proxy()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .build()
                .unwrap();
            let resp = client
                .get(format!("https://{}/", addr))
                .send()
                .await
                .unwrap();
            drop(client);
            resp.text().await
        }));
    }
    let mut ok = 0;
    let mut decode_errs = 0;
    for h in handles {
        match h.await.unwrap() {
            Ok(txt) => {
                assert_eq!(txt, "hello world hello world hello world");
                ok += 1;
            }
            Err(e) => {
                if e.is_decode() {
                    decode_errs += 1;
                }
            }
        }
    }
    assert_eq!(
        ok, n,
        "all requests should succeed, got {} decode errors",
        decode_errs
    );
}
