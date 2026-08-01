mod support;
use support::server;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn retries_apply_in_scope() {
    let _ = env_logger::try_init();
    let cnt = Arc::new(AtomicUsize::new(0));
    let server = server::http(move |_req| {
        let cnt = cnt.clone();
        async move {
            if cnt.fetch_add(1, Ordering::Relaxed) == 0 {
                // first req is bad
                http::Response::builder()
                    .status(http::StatusCode::SERVICE_UNAVAILABLE)
                    .body(Default::default())
                    .unwrap()
            } else {
                http::Response::default()
            }
        }
    });

    let scope = server.addr().ip().to_string();
    let retries = primp::retry::for_host(scope).classify_fn(|req_rep| {
        if req_rep.status() == Some(http::StatusCode::SERVICE_UNAVAILABLE) {
            req_rep.retryable()
        } else {
            req_rep.success()
        }
    });

    let url = format!("http://{}", server.addr());
    let resp = primp::Client::builder()
        .retry(retries)
        .build()
        .unwrap()
        .get(url)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn out_of_scope_retryables_do_not_drain_the_budget() {
    let _ = env_logger::try_init();

    // Always-retryable 503 servers. `in_scope` is the retry target;
    // `out_of_scope` merely hammers the shared retry budget.
    let in_scope_hits = Arc::new(AtomicUsize::new(0));
    let hits = in_scope_hits.clone();
    let in_scope = server::http(move |_req| {
        let hits = hits.clone();
        async move {
            hits.fetch_add(1, Ordering::Relaxed);
            http::Response::builder()
                .status(http::StatusCode::SERVICE_UNAVAILABLE)
                .body(Default::default())
                .unwrap()
        }
    });
    let out_of_scope_hits = Arc::new(AtomicUsize::new(0));
    let hits = out_of_scope_hits.clone();
    let bind = std::net::SocketAddr::from(([127, 0, 0, 2], 0));
    let out_of_scope = server::http_with_config_and_bind(
        move |_req| {
            let hits = hits.clone();
            async move {
                hits.fetch_add(1, Ordering::Relaxed);
                Ok::<_, std::convert::Infallible>(
                    http::Response::builder()
                        .status(http::StatusCode::SERVICE_UNAVAILABLE)
                        .body(Default::default())
                        .unwrap(),
                )
            }
        },
        |_| {},
        bind,
    );

    let scope = in_scope.addr().ip().to_string();
    let retries = primp::retry::for_host(scope).classify_fn(|req_rep| {
        if req_rep.status() == Some(http::StatusCode::SERVICE_UNAVAILABLE) {
            req_rep.retryable()
        } else {
            req_rep.success()
        }
    });

    let client = primp::Client::builder().retry(retries).build().unwrap();

    // Enough out-of-scope retryable responses to exhaust the shared budget
    // (TpsBudget withdrawal is 5 tokens each; reserve is 500). If these drained
    // the budget, the in-scope request below could not retry at all.
    let out_url = format!("http://{}", out_of_scope.addr());
    for _ in 0..110 {
        let resp = client.get(&out_url).send().await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::SERVICE_UNAVAILABLE);
    }
    assert_eq!(
        out_of_scope_hits.load(Ordering::Relaxed),
        110,
        "out-of-scope server must see exactly one hit per request (no retries)"
    );

    // The in-scope request must still get its full retry allowance: original
    // + `max_retries_per_request` (2) = 3 hits, 503 forever.
    let in_url = format!("http://{}", in_scope.addr());
    let resp = client.get(&in_url).send().await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        in_scope_hits.load(Ordering::Relaxed),
        3,
        "out-of-scope retryables must not exhaust the shared budget"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_retries_have_a_limit() {
    let _ = env_logger::try_init();

    // The default policy (no budget) retries protocol nacks exactly
    // `max_retries_per_request` (2) times on top of the original: 3 wire
    // hits in total. tower::retry always sends one more attempt than
    // `clone_request` grants (the taken clone), and the final attempt's
    // failure never reaches `retry()`, so the count is exact.
    //
    // A hyper server cannot produce a retryable error here: hyper's
    // `find_source::<h2::Error>` matches against the REGISTRY h2 type,
    // while the handler error in this test would be the FORK `h2::Error` —
    // a type mismatch sends INTERNAL_ERROR, which is not retryable. A raw
    // fork-h2 server instead sends genuine remote per-stream
    // REFUSED_STREAM resets.
    //
    // Layering: each tower attempt passes through the h2 pool, which itself
    // retries REFUSED_STREAM up to MAX_REFUSED_RETRIES (32) on the same
    // connection for replayable bodies. A persistently-refusing server
    // therefore sees 3 tower attempts × 33 pool attempts = 99 wire hits
    // before the final error surfaces.
    let (addr_tx, addr_rx) = tokio::sync::oneshot::channel();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let server_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let _ = addr_tx.send(listener.local_addr().unwrap());
        let (io, _) = listener.accept().await.unwrap();
        let mut conn = h2::server::Builder::new()
            .handshake::<_, bytes::Bytes>(io)
            .await
            .unwrap();
        while let Some(Ok((_req, mut send))) = conn.accept().await {
            hits2.fetch_add(1, Ordering::Relaxed);
            let _ = send.send_reset(h2::Reason::REFUSED_STREAM);
        }
    });

    let client = primp::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();

    let url = format!("http://{}", addr_rx.await.unwrap());

    let _err = client.get(url).send().await.unwrap_err();
    assert_eq!(
        hits.load(Ordering::Relaxed),
        99,
        "default policy must retry exactly max_retries_per_request (2) times on top of the original"
    );
    drop(server_task);
}

// NOTE: using the default "current_thread" runtime here would cause the test to
// fail, because the only thread would block until `panic_rx` receives a
// notification while the client needs to be driven to get the graceful shutdown
// done.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn highly_concurrent_requests_to_http2_server_with_low_max_concurrent_streams() {
    let client = primp::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();

    let server = server::http_with_config(
        move |req| async move {
            assert_eq!(req.version(), http::Version::HTTP_2);
            Ok::<_, std::convert::Infallible>(http::Response::default())
        },
        |builder| {
            builder.http2().max_concurrent_streams(1);
        },
    );

    let url = format!("http://{}", server.addr());

    let futs = (0..100).map(|_| {
        let client = client.clone();
        let url = url.clone();
        async move {
            let res = client.get(&url).send().await.unwrap();
            assert_eq!(res.status(), primp::StatusCode::OK);
        }
    });
    futures_util::future::join_all(futs).await;
}

#[tokio::test]
async fn highly_concurrent_requests_to_slow_http2_server_with_low_max_concurrent_streams() {
    use support::delay_server;

    let client = primp::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();

    let server = delay_server::Server::new(
        move |req| async move {
            assert_eq!(req.version(), http::Version::HTTP_2);
            http::Response::default()
        },
        |http| {
            http.http2().max_concurrent_streams(1);
        },
        std::time::Duration::from_secs(2),
    )
    .await;

    let url = format!("http://{}", server.addr());

    let futs = (0..100).map(|_| {
        let client = client.clone();
        let url = url.clone();
        async move {
            let res = client.get(&url).send().await.unwrap();
            assert_eq!(res.status(), primp::StatusCode::OK);
        }
    });
    futures_util::future::join_all(futs).await;

    server.shutdown().await;
}
