#![cfg(feature = "http3")]

mod support;

use http::header::CONTENT_LENGTH;
use std::error::Error;
use support::server;

fn assert_send<T: Send>(_: &T) {}

#[tokio::test]
async fn http3_request_full() {
    use http_body_util::BodyExt;

    let server = server::Http3::new().build(move |req| async move {
        assert_eq!(req.headers()[CONTENT_LENGTH], "5");
        let reqb = req.collect().await.unwrap().to_bytes();
        assert_eq!(reqb, "hello");
        http::Response::default()
    });

    let url = format!("https://{}/content-length", server.addr());
    let res_fut = primp::Client::builder()
        .http3_prior_knowledge()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("client builder")
        .post(url)
        .version(http::Version::HTTP_3)
        .body("hello")
        .send();

    assert_send(&res_fut);
    let res = res_fut.await.expect("request");

    assert_eq!(res.version(), http::Version::HTTP_3);
    assert_eq!(res.status(), primp::StatusCode::OK);
}

async fn find_free_tcp_addr() -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap()
}

#[cfg(feature = "http3")]
#[tokio::test]
async fn http3_test_failed_connection() {
    let addr = find_free_tcp_addr().await;
    let port = addr.port();

    let url = format!("https://127.0.0.1:{port}/");
    let client = primp::Client::builder()
        .http3_prior_knowledge()
        .danger_accept_invalid_certs(true)
        .http3_max_idle_timeout(std::time::Duration::from_millis(20))
        .build()
        .expect("client builder");

    // Dead UDP endpoint: the QUIC handshake fails (quinn's 10s handshake
    // timeout, or the user's `connect_timeout` when set). The failure must
    // classify as a timeout AND a connect error (quinn errors are converted
    // to `io::Error` at the connector so the walks in `error.rs` see them).
    let err = client
        .get(&url)
        .version(http::Version::HTTP_3)
        .send()
        .await
        .unwrap_err();
    assert!(err.is_connect(), "h3 connect failure must be is_connect()");
    assert!(
        err.is_timeout(),
        "h3 dead-peer connect must be is_timeout()"
    );

    // The pool must not be poisoned: a second attempt fails the same way...
    let err = client
        .get(&url)
        .version(http::Version::HTTP_3)
        .send()
        .await
        .unwrap_err();
    assert!(err.is_connect(), "h3 connect failure must be is_connect()");
    assert!(
        err.is_timeout(),
        "h3 dead-peer connect must be is_timeout()"
    );

    // ...and once the server is up at that address, the same client succeeds.
    let server = server::Http3::new()
        .with_addr(addr)
        .build(|_| async { http::Response::default() });

    let res = client
        .post(&url)
        .version(http::Version::HTTP_3)
        .body("hello")
        .send()
        .await
        .expect("request");

    assert_eq!(res.version(), http::Version::HTTP_3);
    assert_eq!(res.status(), primp::StatusCode::OK);
    drop(server);
}

#[cfg(feature = "http3")]
#[tokio::test]
async fn http3_connect_honors_connect_timeout() {
    // A dead UDP port: the QUIC handshake gets no reply. Without the fix the
    // handshake hangs for quinn's 10s default handshake timeout regardless of
    // `connect_timeout`; with the fix the error surfaces at ~connect_timeout
    // and classifies as a timeout + connect error.
    let addr = find_free_tcp_addr().await;
    let url = format!("https://127.0.0.1:{}/", addr.port());

    let client = primp::Client::builder()
        .http3_prior_knowledge()
        .danger_accept_invalid_certs(true)
        .connect_timeout(std::time::Duration::from_millis(200))
        .build()
        .expect("client builder");

    let start = std::time::Instant::now();
    let err = client
        .get(&url)
        .version(http::Version::HTTP_3)
        .send()
        .await
        .unwrap_err();

    assert!(
        start.elapsed() < std::time::Duration::from_secs(5),
        "connect_timeout must bound the h3 handshake (quinn's default is 10s); took {:?}",
        start.elapsed()
    );
    assert!(
        err.is_timeout(),
        "timed-out h3 handshake must be is_timeout()"
    );
    assert!(
        err.is_connect(),
        "timed-out h3 handshake must be is_connect()"
    );
}

#[cfg(feature = "http3")]
#[tokio::test]
async fn http3_test_concurrent_request() {
    let server = server::Http3::new().build(|req| async move {
        let mut res = http::Response::default();
        *res.body_mut() = primp::Body::from(format!("hello {}", req.uri().path()));
        res
    });
    let addr = server.addr();

    let client = primp::Client::builder()
        .http3_prior_knowledge()
        .danger_accept_invalid_certs(true)
        .http3_max_idle_timeout(std::time::Duration::from_millis(20))
        .build()
        .expect("client builder");

    let mut tasks = vec![];
    for i in 0..10 {
        let client = client.clone();
        tasks.push(async move {
            let url = format!("https://{}/{}", addr, i);

            client
                .post(&url)
                .version(http::Version::HTTP_3)
                .send()
                .await
                .expect("request")
        });
    }

    let handlers = tasks.into_iter().map(tokio::spawn).collect::<Vec<_>>();

    for (i, handler) in handlers.into_iter().enumerate() {
        let result = handler.await.unwrap();

        assert_eq!(result.version(), http::Version::HTTP_3);
        assert_eq!(result.status(), primp::StatusCode::OK);

        let body = result.text().await.unwrap();
        assert_eq!(body, format!("hello /{}", i));
    }

    drop(server);
}

#[cfg(feature = "http3")]
#[tokio::test]
async fn http3_test_reconnection() {
    use std::error::Error;

    use h3::error::{ConnectionError, StreamError};

    let server = server::Http3::new().build(|_| async { http::Response::default() });
    let addr = server.addr();

    let url = format!("https://{}/", addr);
    let client = primp::Client::builder()
        .http3_prior_knowledge()
        .danger_accept_invalid_certs(true)
        .http3_max_idle_timeout(std::time::Duration::from_millis(20))
        .build()
        .expect("client builder");

    let res = client
        .post(&url)
        .version(http::Version::HTTP_3)
        .send()
        .await
        .expect("request");

    assert_eq!(res.version(), http::Version::HTTP_3);
    assert_eq!(res.status(), primp::StatusCode::OK);
    drop(server);

    let err = client
        .get(&url)
        .version(http::Version::HTTP_3)
        .send()
        .await
        .unwrap_err();

    let err = err.source().unwrap().downcast_ref::<StreamError>().unwrap();

    assert!(matches!(
        err,
        StreamError::ConnectionError {
            0: ConnectionError::Timeout { .. },
            ..
        }
    ));

    let server = server::Http3::new()
        .with_addr(addr)
        .build(|_| async { http::Response::default() });

    let res = client
        .post(&url)
        .version(http::Version::HTTP_3)
        .body("hello")
        .send()
        .await
        .expect("request");

    assert_eq!(res.version(), http::Version::HTTP_3);
    assert_eq!(res.status(), primp::StatusCode::OK);
    drop(server);
}

#[cfg(feature = "http3")]
#[tokio::test]
async fn http3_pooled_connection_closes_when_client_dropped() {
    let mut server = server::Http3::new().build(|_| async { http::Response::default() });
    let url = format!("https://{}/", server.addr());

    {
        let client = primp::Client::builder()
            .http3_prior_knowledge()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("client builder");

        let res = client
            .post(&url)
            .version(http::Version::HTTP_3)
            .send()
            .await
            .expect("request");
        assert_eq!(res.status(), primp::StatusCode::OK);
    }

    // Dropping the client must tear down the pooled HTTP/3 connection; quinn's
    // default idle timeout (30s) keeps a leaked one alive past this 10s deadline.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if server
            .events()
            .iter()
            .any(|e| matches!(e, server::Event::QuicConnectionClosed))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "pooled HTTP/3 connection leaked: server never saw the connection close after client drop"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[cfg(all(feature = "http3", feature = "stream"))]
#[tokio::test]
async fn http3_request_stream() {
    use http_body_util::BodyExt;

    let server = server::Http3::new().build(move |req| async move {
        let reqb = req.collect().await.unwrap().to_bytes();
        assert_eq!(reqb, "hello world");
        http::Response::default()
    });

    let url = format!("https://{}", server.addr());
    let body = primp::Body::wrap_stream(futures_util::stream::iter(vec![
        Ok::<_, std::convert::Infallible>("hello"),
        Ok::<_, std::convert::Infallible>(" "),
        Ok::<_, std::convert::Infallible>("world"),
    ]));

    let res = primp::Client::builder()
        .http3_prior_knowledge()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("client builder")
        .post(url)
        .version(http::Version::HTTP_3)
        .body(body)
        .send()
        .await
        .expect("request");

    assert_eq!(res.version(), http::Version::HTTP_3);
    assert_eq!(res.status(), primp::StatusCode::OK);
}

#[cfg(all(feature = "http3", feature = "stream"))]
#[tokio::test]
async fn http3_request_stream_error() {
    use http_body_util::BodyExt;

    let server = server::Http3::new().build(move |req| async move {
        // HTTP/3 response can start and finish before the entire request body has been received.
        // To avoid prematurely terminating the session, collect full request body before responding.
        let _ = req.collect().await;

        http::Response::default()
    });

    let url = format!("https://{}", server.addr());
    let body = primp::Body::wrap_stream(futures_util::stream::iter(vec![
        Ok::<_, std::io::Error>("first chunk"),
        Err::<_, std::io::Error>(std::io::Error::other("oh no!")),
    ]));

    let res = primp::Client::builder()
        .http3_prior_knowledge()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("client builder")
        .post(url)
        .version(http::Version::HTTP_3)
        .body(body)
        .send()
        .await;

    let err = res.unwrap_err();
    assert!(err.is_request());
    let err = err
        .source()
        .unwrap()
        .downcast_ref::<primp::Error>()
        .unwrap();
    assert!(err.is_body());
}
