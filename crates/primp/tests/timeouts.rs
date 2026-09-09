mod support;
use support::server;

use std::time::Duration;

#[tokio::test]
async fn client_timeout() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| {
        async {
            // delay returning the response
            tokio::time::sleep(Duration::from_millis(300)).await;
            http::Response::default()
        }
    });

    let client = primp::Client::builder()
        .timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();

    let url = format!("http://{}/slow", server.addr());

    let res = client.get(&url).send().await;

    let err = res.unwrap_err();

    assert!(err.is_timeout());
    assert_eq!(err.url().map(|u| u.as_str()), Some(url.as_str()));
}

#[tokio::test]
async fn request_timeout() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| {
        async {
            // delay returning the response
            tokio::time::sleep(Duration::from_millis(300)).await;
            http::Response::default()
        }
    });

    let client = primp::Client::builder().no_proxy().build().unwrap();

    let url = format!("http://{}/slow", server.addr());

    let res = client
        .get(&url)
        .timeout(Duration::from_millis(100))
        .send()
        .await;

    let err = res.unwrap_err();

    assert!(err.is_timeout() && !err.is_connect());
    assert_eq!(err.url().map(|u| u.as_str()), Some(url.as_str()));
}

#[tokio::test]
async fn connect_timeout() {
    let _ = env_logger::try_init();

    let client = primp::Client::builder()
        .connect_timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();

    let url = "http://192.0.2.1:81/slow";

    let res = client
        .get(url)
        .timeout(Duration::from_millis(1000))
        .send()
        .await;

    let err = res.unwrap_err();

    assert!(err.is_connect() && err.is_timeout());
}

#[tokio::test]
async fn connect_many_timeout_succeeds() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::default() });
    let port = server.addr().port();

    let client = primp::Client::builder()
        .resolve_to_addrs(
            "many_addrs",
            &["192.0.2.1:81".parse().unwrap(), server.addr()],
        )
        .connect_timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();

    let url = format!("http://many_addrs:{port}/eventual");

    let _res = client
        .get(url)
        .timeout(Duration::from_millis(1000))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn connect_many_timeout() {
    let _ = env_logger::try_init();

    let client = primp::Client::builder()
        .resolve_to_addrs(
            "many_addrs",
            &[
                "192.0.2.1:81".parse().unwrap(),
                "192.0.2.2:81".parse().unwrap(),
            ],
        )
        .connect_timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();

    let url = "http://many_addrs:81/slow".to_string();

    let res = client
        .get(url)
        .timeout(Duration::from_millis(1000))
        .send()
        .await;

    let err = res.unwrap_err();

    assert!(err.is_connect() && err.is_timeout());
}

#[cfg(feature = "stream")]
#[tokio::test]
async fn response_timeout() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| {
        async {
            // immediate response, but delayed body
            let body = primp::Body::wrap_stream(futures_util::stream::once(async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok::<_, std::convert::Infallible>("Hello")
            }));

            http::Response::new(body)
        }
    });

    let client = primp::Client::builder()
        .timeout(Duration::from_millis(500))
        .no_proxy()
        .build()
        .unwrap();

    let url = format!("http://{}/slow", server.addr());
    let res = client.get(&url).send().await.expect("Failed to get");
    let body = res.text().await;

    let err = body.unwrap_err();

    assert!(err.is_timeout());
}

#[tokio::test]
async fn read_timeout_applies_to_headers() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| {
        async {
            // delay returning the response
            tokio::time::sleep(Duration::from_millis(300)).await;
            http::Response::default()
        }
    });

    let client = primp::Client::builder()
        .read_timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();

    let url = format!("http://{}/slow", server.addr());

    let res = client.get(&url).send().await;

    let err = res.unwrap_err();

    assert!(err.is_timeout());
    assert_eq!(err.url().map(|u| u.as_str()), Some(url.as_str()));
}

#[cfg(feature = "stream")]
#[tokio::test]
async fn read_timeout_applies_to_body() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| {
        async {
            // immediate response, but delayed body
            let body = primp::Body::wrap_stream(futures_util::stream::once(async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                Ok::<_, std::convert::Infallible>("Hello")
            }));

            http::Response::new(body)
        }
    });

    let client = primp::Client::builder()
        .read_timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();

    let url = format!("http://{}/slow", server.addr());
    let res = client.get(&url).send().await.expect("Failed to get");
    let body = res.text().await;

    let err = body.unwrap_err();

    assert!(err.is_timeout());
}

#[cfg(feature = "stream")]
#[tokio::test]
async fn read_timeout_allows_slow_response_body() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| {
        async {
            // immediate response, but body that has slow chunks

            let slow = futures_util::stream::unfold(0, |state| async move {
                if state < 3 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Some((
                        Ok::<_, std::convert::Infallible>(state.to_string()),
                        state + 1,
                    ))
                } else {
                    None
                }
            });
            let body = primp::Body::wrap_stream(slow);

            http::Response::new(body)
        }
    });

    let client = primp::Client::builder()
        .read_timeout(Duration::from_millis(200))
        //.timeout(Duration::from_millis(200))
        .no_proxy()
        .build()
        .unwrap();

    let url = format!("http://{}/slow", server.addr());
    let res = client.get(&url).send().await.expect("Failed to get");
    let body = res.text().await.expect("body text");

    assert_eq!(body, "012");
}

#[tokio::test]
async fn response_body_timeout_forwards_size_hint() {
    let _ = env_logger::try_init();

    let server = server::http(move |_req| async { http::Response::new(b"hello".to_vec().into()) });

    let client = primp::Client::builder().no_proxy().build().unwrap();

    let url = format!("http://{}/slow", server.addr());

    let res = client
        .get(&url)
        .timeout(Duration::from_secs(1))
        .send()
        .await
        .expect("response");

    assert_eq!(res.content_length(), Some(5));
}

/// A resolver whose lookups never complete.
#[derive(Clone)]
struct HangingResolver;

impl primp::dns::Resolve for HangingResolver {
    fn resolve(&self, _name: primp::dns::Name) -> primp::dns::Resolving {
        Box::pin(std::future::pending())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn hanging_dns_with_short_connect_timeout_is_dns_error() {
    use std::future::Future;
    use std::task::Poll;

    let _ = env_logger::try_init();

    // Deterministic under a paused clock (cf. ready_response... below): the
    // DNS deadline is capped just below `connect_timeout` (connect - 1ms via
    // `effective_dns_timeout`), so stepping virtual time in sub-millisecond
    // increments observes the DNS timer fire strictly before the connect
    // timer can. The old real-time `await` asserted which of two timers 1ms
    // apart fired first — scheduling jitter flipped the classification under
    // parallel load. Here deadline ordering alone decides.
    let client = primp::Client::builder()
        .dns_resolver(HangingResolver)
        .connect_timeout(Duration::from_millis(200))
        .no_proxy()
        .build()
        .unwrap();

    // Pause before the first poll so both timers arm against the frozen clock.
    tokio::time::pause();
    let waker = futures_util::task::noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut fut = Box::pin(client.get("http://never-resolves.example/").send());

    // 100us steps can never skip over the 1ms gap between the DNS deadline
    // and the connect timeout: the first Ready observed is necessarily the
    // DNS outcome. Bound the loop so a regression fails loudly, not hangs.
    for _ in 0..100_000 {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => match res {
                Err(err) => {
                    assert!(
                        err.is_dns(),
                        "hanging DNS under a short connect_timeout must classify as DNS: {err}"
                    );
                    assert!(err.is_timeout(), "DNS deadline is a timeout: {err}");
                    assert!(
                        !err.is_connect(),
                        "hanging DNS must not classify as a connect error: {err}"
                    );
                    return;
                }
                Ok(_) => panic!("hanging DNS lookup unexpectedly succeeded"),
            },
            Poll::Pending => {}
        }
        tokio::time::advance(Duration::from_micros(100)).await;
    }
    panic!("hanging DNS did not produce an error within 10s of virtual time");
}

#[tokio::test]
async fn explicit_dns_timeout_is_dns_error() {
    let _ = env_logger::try_init();

    // An explicit `dns_timeout` shorter than `connect_timeout` bounds the
    // lookup: the tagged DNS timeout fires first.
    let client = primp::Client::builder()
        .dns_resolver(HangingResolver)
        .dns_timeout(Duration::from_millis(100))
        .connect_timeout(Duration::from_secs(1))
        .no_proxy()
        .build()
        .unwrap();

    let err = client
        .get("http://never-resolves.example/")
        .send()
        .await
        .unwrap_err();

    assert!(err.is_dns(), "must classify as DNS: {err}");
    assert!(err.is_timeout());
    assert!(!err.is_connect(), "must not classify as connect: {err}");
}

#[tokio::test(flavor = "current_thread")]
async fn ready_response_wins_over_simultaneous_timeout() {
    // Ready response must beat simultaneous timeout (polls in-flight first).
    // Manual polling, no timing flake; warmup reuses pooled connection.
    use std::future::Future;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::task::Poll;

    let _ = env_logger::try_init();
    let received = Arc::new(AtomicBool::new(false));
    let server = server::http({
        let received = received.clone();
        move |req| {
            let received = received.clone();
            async move {
                if req.uri().path() == "/held" {
                    received.store(true, Ordering::SeqCst);
                }
                http::Response::default()
            }
        }
    });

    let client = primp::Client::builder()
        .timeout(Duration::from_millis(50))
        .no_proxy()
        .build()
        .unwrap();

    // Warm up with the clock RUNNING: pooled connection for the race.
    client
        .get(format!("http://{}/warm", server.addr()))
        .send()
        .await
        .expect("warmup request must succeed");

    let url = format!("http://{}/held", server.addr());
    tokio::time::pause();
    let waker = futures_util::task::noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut fut = Box::pin(client.get(&url).send());

    // Drive the send with check-then-poll order: each iteration checks the
    // server flag FIRST and breaks without polling once set, so no poll can
    // observe a completed response (every poll happens strictly before the
    // server has the request, hence observes timeout Pending — the clock is
    // paused — and in_flight Pending). A poll that already observes Ready is
    // a valid fast-path pass. Bound the loop so a pool miss (a new handshake
    // stalls with a paused clock) fails loudly instead of hanging.
    for _ in 0..20000 {
        if received.load(Ordering::SeqCst) {
            break;
        }
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => {
                let res = res.expect("fast response must succeed");
                assert_eq!(res.status(), primp::StatusCode::OK);
                return;
            }
            Poll::Pending => {}
        }
        tokio::task::yield_now().await;
    }
    assert!(
        received.load(Ordering::SeqCst),
        "server never received the race request (pooled connection not reused?)"
    );

    // Phase B: poll nothing. The server already responded; kernel TCP
    // delivery needs no task polling. Real-time wait lets it complete while
    // the paused clock keeps the deadline Pending. (This test uses the
    // single-threaded runtime to minimize thread contention with the other
    // timing-sensitive tests in this file.)
    tokio::task::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_millis(100)))
        .await
        .expect("sleep");

    // Phase C: elapse the deadline WELL past the Sleep (advance must exceed
    // the deadline strictly: tokio's timer wheel does not fire a Sleep whose
    // deadline merely equals `now`), then take exactly ONE poll. in_flight
    // completes synchronously off the kernel buffer while the timeout is
    // already Ready: poll order alone decides.
    tokio::time::advance(Duration::from_millis(100)).await;
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(res) => {
            let res = res.expect("ready response must win over simultaneous timeout");
            assert_eq!(res.status(), primp::StatusCode::OK);
        }
        Poll::Pending => panic!("response bytes were not kernel-buffered before the deadline"),
    }
}
