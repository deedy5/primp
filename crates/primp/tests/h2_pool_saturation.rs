//! Sustained-concurrency behavior of the h2 pool: many concurrent requests
//! against a single pooled connection must all eventually succeed. A waiter
//! parked on the Busy branch loses the wake-up race once per freed slot;
//! under heavy load that must not trip an artificial loop cap (regression:
//! the counter used to count Busy-waiter cycles, failing requests with a bare
//! string error after 1000 wake-up races).

mod support;
use support::server;

use std::time::Duration;

use primp::Client;

/// ~1100 concurrent requests against one slow h2 connection (pool gate 2 →
/// 1 in-flight at a time): the last winner accumulates ~1100 wake-up races,
/// which previously tripped the 1000-iteration cap.
#[tokio::test]
async fn many_concurrent_requests_all_succeed_on_one_saturated_connection() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (io, _) = listener.accept().await.unwrap();
        let mut conn = h2::server::Builder::new()
            .handshake::<_, bytes::Bytes>(io)
            .await
            .unwrap();
        while let Some(Ok((_req, mut send))) = conn.accept().await {
            let mut stream = send.send_response(http::Response::new(()), false).unwrap();
            tokio::time::sleep(Duration::from_millis(4)).await;
            stream.send_data(bytes::Bytes::from("ok"), true).unwrap();
        }
    });

    let url = format!("http://{addr}/");
    let client = Client::builder()
        .no_proxy()
        .http2_prior_knowledge()
        .http2_max_concurrent_streams(2)
        .build()
        .unwrap();

    let mut futs = Vec::new();
    for _ in 0..1100 {
        let client = client.clone();
        let url = url.clone();
        futs.push(async move {
            let res = client.get(&url).send().await.expect("request succeeded");
            assert_eq!(res.status(), primp::StatusCode::OK);
            assert_eq!(res.text().await.unwrap(), "ok");
        });
    }
    futures_util::future::join_all(futs).await;

    drop(client);
    let _ = server.await;
}
