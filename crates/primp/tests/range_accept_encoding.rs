mod support;
use support::server;

#[tokio::test]
async fn range_request_gets_identity_accept_encoding_h1() {
    let _ = env_logger::try_init();

    let server = server::http(move |req| async move {
        let ae = req
            .headers()
            .get("accept-encoding")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        http::Response::new(primp::Body::from(ae))
    });

    let url = format!("http://{}/range", server.addr());
    let client = primp::Client::new();

    let res = client.get(&url).send().await.unwrap();
    let without_range = res.text().await.unwrap();
    assert_ne!(without_range, "identity", "plain GET must keep compression");

    let res = client
        .get(&url)
        .header("range", "bytes=0-10")
        .send()
        .await
        .unwrap();
    let with_range = res.text().await.unwrap();
    assert_eq!(
        with_range, "identity",
        "Range request must disable compression"
    );
}

#[tokio::test]
async fn range_request_overrides_explicit_accept_encoding() {
    let _ = env_logger::try_init();

    let server = server::http(move |req| async move {
        let ae = req
            .headers()
            .get("accept-encoding")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        http::Response::new(primp::Body::from(ae))
    });

    let url = format!("http://{}/range", server.addr());
    let client = primp::Client::new();

    let res = client
        .get(&url)
        .header("range", "bytes=0-10")
        .header("accept-encoding", "gzip")
        .send()
        .await
        .unwrap();
    let with_explicit_ae = res.text().await.unwrap();
    assert_eq!(
        with_explicit_ae, "identity",
        "Range request must force identity even with an explicit Accept-Encoding"
    );
}

#[tokio::test]
async fn range_request_gets_identity_accept_encoding_h2() {
    let _ = env_logger::try_init();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (io, _) = listener.accept().await.unwrap();
        let mut conn = h2::server::Builder::new()
            .handshake::<_, bytes::Bytes>(io)
            .await
            .unwrap();
        while let Some(Ok((req, mut send))) = conn.accept().await {
            let ae = req
                .headers()
                .get("accept-encoding")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let mut stream = send.send_response(http::Response::new(()), false).unwrap();
            stream.send_data(bytes::Bytes::from(ae), true).unwrap();
        }
    });

    let url = format!("http://{addr}/range");
    let client = primp::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();

    let res = client.get(&url).send().await.unwrap();
    let without_range = res.text().await.unwrap();
    assert_ne!(without_range, "identity", "plain GET must keep compression");

    let res = client
        .get(&url)
        .header("range", "bytes=0-10")
        .send()
        .await
        .unwrap();
    let with_range = res.text().await.unwrap();
    assert_eq!(
        with_range, "identity",
        "Range request must disable compression"
    );

    drop(client);
    let _ = server.await;
}
