mod support;

use support::server;

use http::header::{CONTENT_LENGTH, CONTENT_TYPE, TRANSFER_ENCODING};
#[cfg(feature = "json")]
use std::collections::HashMap;

use primp::Client;
use tokio::io::AsyncWriteExt;

/// Spin up a local HTTPS server with a self-signed cert for `localhost`
/// (ALPN `h2` + `http/1.1`, auto-negotiated per connection). Returns the
/// bound address. Clients must disable cert/hostname verification
/// (`danger_accept_invalid_*`) to connect.
async fn spawn_https_server() -> std::net::SocketAddr {
    use http::Request;
    use hyper::service::service_fn;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use rcgen::{CertificateParams, IsCa, KeyPair};
    use rustls::pki_types::PrivateKeyDer;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio_rustls::rustls::ServerConfig as RustlsServerConfig;
    use tokio_rustls::TlsAcceptor;

    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    params.is_ca = IsCa::NoCa;
    let cert = params.self_signed(&key).unwrap();
    let cert_der = cert.der().clone();
    let key_der = PrivateKeyDer::try_from(key.serialize_der()).unwrap();

    let tls = RustlsServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(tls));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (sock, _) = match listener.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let tls = match acceptor.accept(sock).await {
                    Ok(t) => t,
                    Err(_) => return,
                };
                let io = TokioIo::new(tls);
                let svc = service_fn(move |_req: Request<hyper::body::Incoming>| async move {
                    Ok::<_, std::convert::Infallible>(
                        http::Response::builder()
                            .status(200)
                            .body(primp::Body::from("Hello"))
                            .unwrap(),
                    )
                });
                let builder = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
                let _ = builder.serve_connection(io, svc).await;
            });
        }
    });

    addr
}

#[tokio::test]
async fn auto_headers() {
    let server = server::http(move |req| async move {
        assert_eq!(req.method(), "GET");

        assert_eq!(req.headers()["accept"], "*/*");
        assert_eq!(req.headers().get("user-agent"), None);
        if cfg!(feature = "gzip") {
            assert!(req.headers()["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("gzip"));
        }
        if cfg!(feature = "brotli") {
            assert!(req.headers()["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("br"));
        }
        if cfg!(feature = "zstd") {
            assert!(req.headers()["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("zstd"));
        }
        if cfg!(feature = "deflate") {
            assert!(req.headers()["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("deflate"));
        }

        http::Response::default()
    });

    let url = format!("http://{}/1", server.addr());
    let res = primp::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(&url)
        .send()
        .await
        .unwrap();

    assert_eq!(res.url().as_str(), &url);
    assert_eq!(res.status(), primp::StatusCode::OK);
    assert_eq!(res.remote_addr(), Some(server.addr()));
}

#[tokio::test]
async fn donot_set_content_length_0_if_have_no_body() {
    let server = server::http(move |req| async move {
        let headers = req.headers();
        assert_eq!(headers.get(CONTENT_LENGTH), None);
        assert!(headers.get(CONTENT_TYPE).is_none());
        assert!(headers.get(TRANSFER_ENCODING).is_none());
        dbg!(&headers);
        http::Response::default()
    });

    let url = format!("http://{}/content-length", server.addr());
    let res = primp::Client::builder()
        .no_proxy()
        .build()
        .expect("client builder")
        .get(&url)
        .send()
        .await
        .expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
}

#[tokio::test]
async fn user_agent() {
    let server = server::http(move |req| async move {
        assert_eq!(req.headers()["user-agent"], "primp-test-agent");
        http::Response::default()
    });

    let url = format!("http://{}/ua", server.addr());
    let res = primp::Client::builder()
        .user_agent("primp-test-agent")
        .build()
        .expect("client builder")
        .get(&url)
        .send()
        .await
        .expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
}

#[tokio::test]
async fn response_text() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let client = Client::new();

    let res = client
        .get(format!("http://{}/text", server.addr()))
        .send()
        .await
        .expect("Failed to get");
    assert_eq!(res.content_length(), Some(5));
    let text = res.text().await.expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[tokio::test]
async fn response_bytes() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let client = Client::new();

    let res = client
        .get(format!("http://{}/bytes", server.addr()))
        .send()
        .await
        .expect("Failed to get");
    assert_eq!(res.content_length(), Some(5));
    let bytes = res.bytes().await.expect("res.bytes()");
    assert_eq!("Hello", bytes);
}

#[tokio::test]
#[cfg(feature = "json")]
async fn response_json() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::new("\"Hello\"".into()) });

    let client = Client::new();

    let res = client
        .get(format!("http://{}/json", server.addr()))
        .send()
        .await
        .expect("Failed to get");
    let text = res.json::<String>().await.expect("Failed to get json");
    assert_eq!("Hello", text);
}

#[tokio::test]
async fn body_pipe_response() {
    use http_body_util::BodyExt;
    let _ = env_logger::try_init();

    let server = server::http(move |req| async move {
        if req.uri() == "/get" {
            http::Response::new("pipe me".into())
        } else {
            assert_eq!(req.uri(), "/pipe");
            assert_eq!(req.headers()["content-length"], "7");

            let full: Vec<u8> = req
                .into_body()
                .collect()
                .await
                .expect("must succeed")
                .to_bytes()
                .to_vec();

            assert_eq!(full, b"pipe me");

            http::Response::default()
        }
    });

    let client = Client::new();

    let res1 = client
        .get(format!("http://{}/get", server.addr()))
        .send()
        .await
        .expect("get1");

    assert_eq!(res1.status(), primp::StatusCode::OK);
    assert_eq!(res1.content_length(), Some(7));

    // and now ensure we can "pipe" the response to another request
    let res2 = client
        .post(format!("http://{}/pipe", server.addr()))
        .body(res1)
        .send()
        .await
        .expect("res2");

    assert_eq!(res2.status(), primp::StatusCode::OK);
}

#[tokio::test]
async fn overridden_dns_resolution_with_gai() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{overridden_domain}:{}/domain_override",
        server.addr().port()
    );
    let client = primp::Client::builder()
        .no_proxy()
        .resolve(overridden_domain, server.addr())
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().await.expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
    let text = res.text().await.expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[tokio::test]
async fn overridden_dns_resolution_with_gai_multiple() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{overridden_domain}:{}/domain_override",
        server.addr().port()
    );
    // the server runs on IPv4 localhost, so provide both IPv4 and IPv6 and let the happy eyeballs
    // algorithm decide which address to use.
    let client = primp::Client::builder()
        .no_proxy()
        .resolve_to_addrs(
            overridden_domain,
            &[
                std::net::SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    server.addr().port(),
                ),
                server.addr(),
            ],
        )
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().await.expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
    let text = res.text().await.expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[cfg(feature = "hickory-dns")]
#[tokio::test]
async fn overridden_dns_resolution_with_hickory_dns() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{overridden_domain}:{}/domain_override",
        server.addr().port()
    );
    let client = primp::Client::builder()
        .no_proxy()
        .resolve(overridden_domain, server.addr())
        .hickory_dns(true)
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().await.expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
    let text = res.text().await.expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[cfg(feature = "hickory-dns")]
#[tokio::test]
async fn overridden_dns_resolution_with_hickory_dns_multiple() {
    let _ = env_logger::builder().is_test(true).try_init();
    let server = server::http(move |_req| async { http::Response::new("Hello".into()) });

    let overridden_domain = "rust-lang.org";
    let url = format!(
        "http://{overridden_domain}:{}/domain_override",
        server.addr().port()
    );
    // the server runs on IPv4 localhost, so provide both IPv4 and IPv6 and let the happy eyeballs
    // algorithm decide which address to use.
    let client = primp::Client::builder()
        .no_proxy()
        .resolve_to_addrs(
            overridden_domain,
            &[
                std::net::SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    server.addr().port(),
                ),
                server.addr(),
            ],
        )
        .hickory_dns(true)
        .build()
        .expect("client builder");
    let req = client.get(&url);
    let res = req.send().await.expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
    let text = res.text().await.expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[test]
fn use_preconfigured_rustls_default() {
    extern crate rustls;

    let root_cert_store = rustls::RootCertStore::empty();
    let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(root_cert_store)
    .with_no_client_auth();

    primp::Client::builder()
        .use_preconfigured_tls(tls)
        .build()
        .expect("preconfigured rustls tls");
}

/// The preconfigured root store, not the builder defaults, drives
/// verification: trusted → 200, empty → UnknownIssuer.
#[tokio::test]
async fn use_preconfigured_tls_honors_custom_root_store() {
    extern crate rustls;

    use http::Request;
    use hyper::service::service_fn;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use rcgen::{CertificateParams, IsCa, KeyPair};
    use rustls::pki_types::PrivateKeyDer;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio_rustls::rustls::ServerConfig as RustlsServerConfig;
    use tokio_rustls::TlsAcceptor;

    fn preconfigured(roots: rustls::RootCertStore) -> rustls::ClientConfig {
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth()
    }

    // Self-signed leaf for "testserver.com", generated at test time so the
    // trust chain is fully under our control.
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(vec!["testserver.com".to_string()]).unwrap();
    params.is_ca = IsCa::NoCa;
    let cert = params.self_signed(&key).unwrap();
    let cert_der = cert.der().clone();
    let key_der = PrivateKeyDer::try_from(key.serialize_der()).unwrap();

    let mut tls = RustlsServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .unwrap();
    tls.alpn_protocols = vec![b"http/1.1".into()];
    let acceptor = TlsAcceptor::from(Arc::new(tls));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (sock, _) = match listener.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let tls = match acceptor.accept(sock).await {
                    Ok(t) => t,
                    Err(_) => return,
                };
                let io = TokioIo::new(tls);
                let svc = service_fn(move |_req: Request<hyper::body::Incoming>| async move {
                    Ok::<_, std::convert::Infallible>(
                        http::Response::builder()
                            .status(200)
                            .body(primp::Body::from("Hello"))
                            .unwrap(),
                    )
                });
                let builder = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
                let _ = builder.serve_connection(io, svc).await;
            });
        }
    });

    let url = format!("https://testserver.com:{}/", addr.port());

    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert_der).expect("add cert to root store");
    let client = primp::Client::builder()
        .no_proxy()
        .resolve("testserver.com", addr)
        .use_preconfigured_tls(preconfigured(roots))
        .build()
        .expect("preconfigured rustls tls");
    let res = client
        .get(&url)
        .send()
        .await
        .expect("trusted root store must succeed");
    assert_eq!(res.status(), primp::StatusCode::OK);

    let client = primp::Client::builder()
        .no_proxy()
        .resolve("testserver.com", addr)
        .use_preconfigured_tls(preconfigured(rustls::RootCertStore::empty()))
        .build()
        .expect("preconfigured rustls tls");
    let err = client
        .get(&url)
        .send()
        .await
        .expect_err("untrusted root store must fail");
    let mut chain = std::iter::successors(Some(&err as &dyn std::error::Error), |e| e.source());
    assert!(
        chain.any(|e| e.to_string().contains("UnknownIssuer")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn http1_only() {
    let addr = spawn_https_server().await;
    let res = primp::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .http1_only()
        .build()
        .expect("client builder")
        .get(format!("https://{addr}/"))
        .send()
        .await
        .expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
    assert_eq!(res.version(), primp::Version::HTTP_11);
}

#[tokio::test]
#[ignore = "Needs TLS support in the test server"]
async fn http2_upgrade() {
    let server = server::http(move |_| async move { http::Response::default() });

    let url = format!("https://localhost:{}", server.addr().port());
    let res = primp::Client::builder()
        .tls_danger_accept_invalid_certs(true)
        .tls_backend_rustls()
        .build()
        .expect("client builder")
        .get(&url)
        .send()
        .await
        .expect("request");

    assert_eq!(res.status(), primp::StatusCode::OK);
    assert_eq!(res.version(), primp::Version::HTTP_2);
}

#[test]
#[cfg(feature = "json")]
fn add_json_default_content_type_if_not_set_manually() {
    let mut map = HashMap::new();
    map.insert("body", "json");
    let content_type = http::HeaderValue::from_static("application/vnd.api+json");
    let req = Client::new()
        .post("https://www.google.com/")
        .header(CONTENT_TYPE, &content_type)
        .json(&map)
        .build()
        .expect("request is not valid");

    assert_eq!(content_type, req.headers().get(CONTENT_TYPE).unwrap());
}

#[test]
#[cfg(feature = "json")]
fn update_json_content_type_if_set_manually() {
    let mut map = HashMap::new();
    map.insert("body", "json");
    let req = Client::new()
        .post("https://www.google.com/")
        .json(&map)
        .build()
        .expect("request is not valid");

    assert_eq!("application/json", req.headers().get(CONTENT_TYPE).unwrap());
}

#[tokio::test]
async fn test_tls_info() {
    let addr = spawn_https_server().await;
    let resp = primp::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .tls_info(true)
        .build()
        .expect("client builder")
        .get(format!("https://{addr}/"))
        .send()
        .await
        .expect("response");
    let tls_info = resp.extensions().get::<primp::tls::TlsInfo>();
    assert!(tls_info.is_some());
    let tls_info = tls_info.unwrap();
    let peer_certificate = tls_info.peer_certificate();
    assert!(peer_certificate.is_some());
    let der = peer_certificate.unwrap();
    assert_eq!(der[0], 0x30); // ASN.1 SEQUENCE

    let resp = primp::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .expect("client builder")
        .get(format!("https://{addr}/"))
        .send()
        .await
        .expect("response");
    let tls_info = resp.extensions().get::<primp::tls::TlsInfo>();
    assert!(tls_info.is_none());
}

#[tokio::test]
async fn close_connection_after_idle_timeout() {
    let mut server = server::http(move |_| async move { http::Response::default() });

    let client = primp::Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(1))
        .build()
        .unwrap();

    let url = format!("http://{}", server.addr());

    client.get(&url).send().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    assert!(server
        .events()
        .iter()
        .any(|e| matches!(e, server::Event::ConnectionClosed)));
}

#[tokio::test]
async fn http1_reason_phrase() {
    let server = server::low_level_with_response(|_raw_request, client_socket| {
        Box::new(async move {
            client_socket
                .write_all(b"HTTP/1.1 418 I'm not a teapot\r\nContent-Length: 0\r\n\r\n")
                .await
                .expect("response write_all failed");
        })
    });

    let client = Client::new();

    let res = client
        .get(format!("http://{}", server.addr()))
        .send()
        .await
        .expect("Failed to get");

    assert_eq!(
        res.error_for_status().unwrap_err().to_string(),
        format!(
            "HTTP status client error (418 I'm not a teapot) for url (http://{}/)",
            server.addr()
        )
    );
}

#[tokio::test]
async fn error_has_url() {
    let u = "http://does.not.exist.local/ever";
    let client = primp::Client::builder().no_proxy().build().unwrap();
    let err = client.get(u).send().await.unwrap_err();
    assert_eq!(err.url().map(AsRef::as_ref), Some(u), "{err:?}");
}

/// `.version(HTTP_3)` on a client without the h3 dispatch must be clamped
/// to HTTP/1.1: hyper's h1 encoder panics on any other version and the
/// release profile (`panic = "abort"`) turns that panic into a process
/// abort. Gated off the http3 feature because with it the version is
/// legitimately dispatched to the QUIC client instead.
#[tokio::test]
#[cfg(not(feature = "http3"))]
async fn version_http3_is_clamped_to_http11_on_h1_paths() {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;

    let saw = Arc::new(AtomicU8::new(0));
    let saw2 = saw.clone();
    let server = server::http(move |req: http::Request<hyper::body::Incoming>| {
        let v = match req.version() {
            http::Version::HTTP_11 => 11,
            http::Version::HTTP_10 => 10,
            http::Version::HTTP_2 => 2,
            _ => 99,
        };
        saw2.store(v, Ordering::SeqCst);
        async move {
            http::Response::builder()
                .status(200)
                .body(primp::Body::from("ok"))
                .unwrap()
        }
    });

    let client = Client::new();
    for version in [http::Version::HTTP_3, http::Version::HTTP_09] {
        let res = client
            .get(format!("http://{}", server.addr()))
            .version(version)
            .send()
            .await
            .expect("request with unsupported version must not panic");
        assert_eq!(res.status(), primp::StatusCode::OK);
        assert_eq!(
            saw.load(Ordering::SeqCst),
            11,
            "server must see HTTP/1.1 for {version:?}"
        );
    }
}
