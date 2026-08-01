use rcgen::{CertificateParams, IsCa, KeyPair};
use rustls::pki_types::PrivateKeyDer;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::rustls::ServerConfig as RustlsServerConfig;
use tokio_rustls::TlsAcceptor;

#[tokio::test]
async fn tls_connection_teardown_sends_close_notify() {
    let _ = env_logger::try_init();

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

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(stream).await.unwrap();

        // Read the request headers.
        let mut buf = vec![0u8; 4096];
        let mut saw_headers = false;
        loop {
            match tls.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                        saw_headers = true;
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        if !saw_headers {
            return Ok::<_, Box<dyn std::error::Error + Send + Sync>>(false);
        }

        tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        tls.flush().await?;

        // After the client drops the connection, a TLS close_notify must
        // arrive as a clean EOF (Ok(0)); a bare TCP close surfaces as
        // UnexpectedEof instead.
        let mut tail = Vec::new();
        match tls.read_buf(&mut tail).await {
            Ok(0) => Ok::<_, Box<dyn std::error::Error + Send + Sync>>(true),
            Ok(_) => Ok(false),
            Err(_) => Ok(false),
        }
    });

    let client = primp::Client::builder()
        .no_proxy()
        .connect_timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .unwrap();

    let url = format!("https://{addr}/");
    let res = client.get(&url).send().await.unwrap();
    assert_eq!(res.text().await.unwrap(), "ok");
    drop(client);

    let clean_eof = server.await.unwrap().unwrap();
    assert!(
        clean_eof,
        "client closed the TLS connection without a close_notify"
    );
}
