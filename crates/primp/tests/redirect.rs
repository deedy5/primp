mod support;
use http_body_util::BodyExt;
use primp::Body;
use support::server;

/// One-shot cookies (`RequestBuilder::one_shot_cookies`) must be re-merged
/// with the FRESH jar on every same-origin redirect hop. Regression: a stale
/// pre-merged `Cookie` header (or none at all) suppressed jar injection, so a
/// `Set-Cookie` from an intermediate hop never reached the final hop, and
/// one-shot cookies vanished entirely.
#[cfg(feature = "cookies")]
#[tokio::test]
async fn test_same_origin_redirect_re_merges_one_shot_cookies_with_fresh_jar() {
    use std::sync::{Arc, Mutex};

    let final_cookie: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let final_cookie2 = Arc::clone(&final_cookie);

    let server = server::http(move |req| {
        let final_cookie2 = Arc::clone(&final_cookie2);
        async move {
            if req.uri() == "/start" {
                http::Response::builder()
                    .status(302)
                    .header("location", "/mid")
                    .header("set-cookie", "fresh=1; Path=/")
                    .body(Body::default())
                    .unwrap()
            } else if req.uri() == "/mid" {
                http::Response::builder()
                    .status(302)
                    .header("location", "/final")
                    .header("set-cookie", "more=2; Path=/")
                    .body(Body::default())
                    .unwrap()
            } else {
                *final_cookie2.lock().unwrap() = req
                    .headers()
                    .get(primp::header::COOKIE)
                    .map(|v| v.to_str().unwrap().to_string());
                http::Response::builder()
                    .status(200)
                    .body(Body::default())
                    .unwrap()
            }
        }
    });

    let client = primp::ClientBuilder::new()
        .no_proxy()
        .cookie_store(true)
        .build()
        .unwrap();
    let res = client
        .request(
            primp::Method::GET,
            format!("http://{}/start", server.addr()),
        )
        .one_shot_cookies(http::header::HeaderValue::from_static("oneshot=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), primp::StatusCode::OK);

    let captured = final_cookie.lock().unwrap().clone();
    let mut pairs: Vec<&str> = captured
        .as_deref()
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .collect();
    pairs.sort_unstable();
    assert_eq!(
        pairs.join("; "),
        "fresh=1; more=2; oneshot=1",
        "every hop must contribute: intermediate Set-Cookies from the fresh jar \
         plus the one-shot cookies (jar order is store-defined)"
    );
}

/// One-shot cookies must NOT follow the redirect across hosts: the redirect
/// policy strips them (like the `Cookie` header itself) on cross-host hops.
#[cfg(feature = "cookies")]
#[tokio::test]
async fn test_redirect_cross_host_drops_one_shot_cookies() {
    use std::sync::{Arc, Mutex};

    let target_cookie: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let target_cookie2 = Arc::clone(&target_cookie);

    let dst = server::http(move |req| {
        let target_cookie2 = Arc::clone(&target_cookie2);
        async move {
            if req.uri() == "/dst" {
                *target_cookie2.lock().unwrap() = req
                    .headers()
                    .get(primp::header::COOKIE)
                    .map(|v| v.to_str().unwrap().to_string());
            }
            http::Response::builder()
                .status(200)
                .body(Body::default())
                .unwrap()
        }
    });

    let dst_url = format!("http://localhost:{}/dst", dst.addr().port());
    let src = server::http(move |req| {
        let dst_url = dst_url.clone();
        async move {
            assert_eq!(req.uri(), "/start");
            http::Response::builder()
                .status(302)
                .header("location", dst_url)
                .body(Body::default())
                .unwrap()
        }
    });

    let client = primp::ClientBuilder::new()
        .no_proxy()
        .cookie_store(true)
        .build()
        .unwrap();
    let res = client
        .request(primp::Method::GET, format!("http://{}/start", src.addr()))
        .one_shot_cookies(http::header::HeaderValue::from_static("oneshot=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), primp::StatusCode::OK);

    assert_eq!(
        *target_cookie.lock().unwrap(),
        None,
        "one-shot cookies must not leak to the redirect target"
    );
}

fn test_client() -> primp::Client {
    primp::Client::builder().no_proxy().build().unwrap()
}

#[tokio::test]
async fn test_redirect_301_and_302_and_303_changes_post_to_get() {
    let client = test_client();
    let codes = [301u16, 302, 303];

    for &code in &codes {
        let redirect = server::http(move |req| async move {
            if req.method() == "POST" {
                assert_eq!(req.uri(), &*format!("/{code}"));
                http::Response::builder()
                    .status(code)
                    .header("location", "/dst")
                    .header("server", "test-redirect")
                    .body(Body::default())
                    .unwrap()
            } else {
                assert_eq!(req.method(), "GET");

                http::Response::builder()
                    .header("server", "test-dst")
                    .body(Body::default())
                    .unwrap()
            }
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let dst = format!("http://{}/{}", redirect.addr(), "dst");
        let res = client.post(&url).send().await.unwrap();
        assert_eq!(res.url().as_str(), dst);
        assert_eq!(res.status(), primp::StatusCode::OK);
        assert_eq!(
            res.headers().get(primp::header::SERVER).unwrap(),
            &"test-dst"
        );
    }
}

#[tokio::test]
async fn test_redirect_307_and_308_tries_to_get_again() {
    let client = test_client();
    let codes = [307u16, 308];
    for &code in &codes {
        let redirect = server::http(move |req| async move {
            assert_eq!(req.method(), "GET");
            if req.uri() == &*format!("/{code}") {
                http::Response::builder()
                    .status(code)
                    .header("location", "/dst")
                    .header("server", "test-redirect")
                    .body(Body::default())
                    .unwrap()
            } else {
                assert_eq!(req.uri(), "/dst");

                http::Response::builder()
                    .header("server", "test-dst")
                    .body(Body::default())
                    .unwrap()
            }
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let dst = format!("http://{}/{}", redirect.addr(), "dst");
        let res = client.get(&url).send().await.unwrap();
        assert_eq!(res.url().as_str(), dst);
        assert_eq!(res.status(), primp::StatusCode::OK);
        assert_eq!(
            res.headers().get(primp::header::SERVER).unwrap(),
            &"test-dst"
        );
    }
}

#[tokio::test]
async fn test_redirect_307_and_308_tries_to_post_again() {
    let _ = env_logger::try_init();
    let client = test_client();
    let codes = [307u16, 308];
    for &code in &codes {
        let redirect = server::http(move |mut req| async move {
            assert_eq!(req.method(), "POST");
            assert_eq!(req.headers()["content-length"], "5");

            let data = req
                .body_mut()
                .frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap();
            assert_eq!(&*data, b"Hello");

            if req.uri() == &*format!("/{code}") {
                http::Response::builder()
                    .status(code)
                    .header("location", "/dst")
                    .header("server", "test-redirect")
                    .body(Body::default())
                    .unwrap()
            } else {
                assert_eq!(req.uri(), "/dst");

                http::Response::builder()
                    .header("server", "test-dst")
                    .body(Body::default())
                    .unwrap()
            }
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let dst = format!("http://{}/{}", redirect.addr(), "dst");
        let res = client.post(&url).body("Hello").send().await.unwrap();
        assert_eq!(res.url().as_str(), dst);
        assert_eq!(res.status(), primp::StatusCode::OK);
        assert_eq!(
            res.headers().get(primp::header::SERVER).unwrap(),
            &"test-dst"
        );
    }
}

#[tokio::test]
async fn test_redirect_removes_sensitive_headers() {
    use tokio::sync::watch;

    let (tx, rx) = watch::channel::<Option<std::net::SocketAddr>>(None);

    let end_server = server::http(move |req| {
        let mut rx = rx.clone();
        async move {
            assert_eq!(req.headers().get("cookie"), None);

            rx.changed().await.unwrap();
            let mid_addr = rx.borrow().unwrap();
            assert_eq!(
                req.headers()["referer"],
                format!("http://{mid_addr}/sensitive")
            );
            http::Response::default()
        }
    });

    let end_addr = end_server.addr();

    let mid_server = server::http(move |req| async move {
        assert_eq!(req.headers()["cookie"], "foo=bar");
        http::Response::builder()
            .status(302)
            .header("location", format!("http://{end_addr}/end"))
            .body(Body::default())
            .unwrap()
    });

    tx.send(Some(mid_server.addr())).unwrap();

    primp::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://{}/sensitive", mid_server.addr()))
        .header(
            primp::header::COOKIE,
            primp::header::HeaderValue::from_static("foo=bar"),
        )
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redirect_strips_auth_for_whole_chain_after_cross_host_hop() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // B records whether it saw Authorization on each of its two endpoints.
    let mid_saw_auth = Arc::new(AtomicBool::new(false));
    let end_saw_auth = Arc::new(AtomicBool::new(false));
    let mid_flag = mid_saw_auth.clone();
    let end_flag = end_saw_auth.clone();

    let b_server = server::http(move |req| {
        let mid_flag = mid_flag.clone();
        let end_flag = end_flag.clone();
        async move {
            let has_auth = req.headers().contains_key(primp::header::AUTHORIZATION);
            if req.uri() == "/mid" {
                mid_flag.store(has_auth, Ordering::SeqCst);
                // Same-origin redirect: the hop that used to resurrect the
                // Authorization stripped on the A -> B cross-host hop.
                http::Response::builder()
                    .status(302)
                    .header("location", "/end")
                    .body(Body::default())
                    .unwrap()
            } else {
                end_flag.store(has_auth, Ordering::SeqCst);
                http::Response::default()
            }
        }
    });
    let b_addr = b_server.addr();

    // A: redirects to B (cross-host hop).
    let a_server = server::http(move |_req| async move {
        http::Response::builder()
            .status(302)
            .header("location", format!("http://{b_addr}/mid"))
            .body(Body::default())
            .unwrap()
    });

    primp::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://{}/start", a_server.addr()))
        .header(
            primp::header::AUTHORIZATION,
            primp::header::HeaderValue::from_static("Bearer leakme"),
        )
        .send()
        .await
        .unwrap();

    assert!(
        !mid_saw_auth.load(Ordering::SeqCst),
        "cross-host hop must strip Authorization at B/mid"
    );
    assert!(
        !end_saw_auth.load(Ordering::SeqCst),
        "LEAK: Authorization for origin A was resent to origin B on the same-origin B/mid -> B/end hop"
    );
}

#[tokio::test]
async fn test_redirect_policy_can_return_errors() {
    let server = server::http(move |req| async move {
        assert_eq!(req.uri(), "/loop");
        http::Response::builder()
            .status(302)
            .header("location", "/loop")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("http://{}/loop", server.addr());
    let err = test_client().get(&url).send().await.unwrap_err();
    assert!(err.is_redirect());
}

#[tokio::test]
async fn test_redirect_policy_can_stop_redirects_without_an_error() {
    let server = server::http(move |req| async move {
        assert_eq!(req.uri(), "/no-redirect");
        http::Response::builder()
            .status(302)
            .header("location", "/dont")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("http://{}/no-redirect", server.addr());

    let res = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::none())
        .build()
        .unwrap()
        .get(&url)
        .send()
        .await
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), primp::StatusCode::FOUND);
}

#[tokio::test]
async fn test_referer_is_not_set_if_disabled() {
    let server = server::http(move |req| async move {
        if req.uri() == "/no-refer" {
            http::Response::builder()
                .status(302)
                .header("location", "/dst")
                .body(Body::default())
                .unwrap()
        } else {
            assert_eq!(req.uri(), "/dst");
            assert_eq!(req.headers().get("referer"), None);

            http::Response::default()
        }
    });

    primp::Client::builder()
        .no_proxy()
        .referer(false)
        .build()
        .unwrap()
        .get(format!("http://{}/no-refer", server.addr()))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_invalid_location_stops_redirect_gh484() {
    let server = server::http(move |_req| async move {
        http::Response::builder()
            .status(302)
            .header("location", "http://www.yikes{KABOOM}")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("http://{}/yikes", server.addr());

    let res = test_client().get(&url).send().await.unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), primp::StatusCode::FOUND);
}

#[tokio::test]
async fn test_invalid_scheme_is_rejected() {
    let server = server::http(move |_req| async move {
        http::Response::builder()
            .status(302)
            .header("location", "htt://www.yikes.com/")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("http://{}/yikes", server.addr());

    let err = test_client().get(&url).send().await.unwrap_err();
    // A redirect hop to an unsupported scheme is a redirect failure.
    assert!(err.is_redirect(), "expected redirect error, got: {err:?}");
}

/// Cross-host redirect: cookies set by the ORIGINAL host must NOT be sent to
/// the redirect target (per-hop jar scoping). Regression: the `Url` request
/// extension stashed by `execute_request` was replayed onto every rebuilt hop
/// (tower-http 0.7 preserves extensions), so the cookie service keyed to the
/// original host — leaking its cookies to an unrelated origin.
#[cfg(feature = "cookies")]
#[tokio::test]
async fn test_redirect_cross_host_does_not_send_original_host_cookies() {
    use std::sync::{Arc, Mutex};

    let target_cookie: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let target_cookie2 = Arc::clone(&target_cookie);

    let dst = server::http(move |req| {
        let target_cookie2 = Arc::clone(&target_cookie2);
        async move {
            if req.uri() == "/dst" {
                *target_cookie2.lock().unwrap() = req
                    .headers()
                    .get(primp::header::COOKIE)
                    .map(|v| v.to_str().unwrap().to_string());
            }
            http::Response::builder()
                .status(200)
                .header("set-cookie", "b=2; Path=/")
                .body(Body::default())
                .unwrap()
        }
    });

    let dst_url = format!("http://localhost:{}/dst", dst.addr().port());
    let src = server::http(move |req| {
        let dst_url = dst_url.clone();
        async move {
            assert_eq!(req.uri(), "/start");
            http::Response::builder()
                .status(302)
                .header("location", dst_url)
                .header("set-cookie", "a=1; Path=/")
                .body(Body::default())
                .unwrap()
        }
    });

    let client = primp::ClientBuilder::new()
        .no_proxy()
        .cookie_store(true)
        .build()
        .unwrap();
    let res = client
        .get(format!("http://{}/start", src.addr()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), primp::StatusCode::OK);

    assert_eq!(
        *target_cookie.lock().unwrap(),
        None,
        "redirect target received the original host's cookie"
    );
}

/// The Set-Cookie from the redirect target must be stored under the target's
/// host, not the original request's host — so later requests to the target
/// send it, and the original host never sees it.
#[cfg(feature = "cookies")]
#[tokio::test]
async fn test_redirect_cross_host_stores_target_cookie_under_target() {
    use std::sync::{Arc, Mutex};

    let dst = server::http(move |req| async move {
        assert_eq!(req.uri(), "/dst");
        http::Response::builder()
            .status(200)
            .header("set-cookie", "b=2; Path=/")
            .body(Body::default())
            .unwrap()
    });

    let dst_url = format!("http://localhost:{}/dst", dst.addr().port());
    let src = server::http(move |req| {
        let dst_url = dst_url.clone();
        async move {
            assert_eq!(req.uri(), "/start");
            http::Response::builder()
                .status(302)
                .header("location", dst_url)
                .body(Body::default())
                .unwrap()
        }
    });

    let client = primp::ClientBuilder::new()
        .no_proxy()
        .cookie_store(true)
        .build()
        .unwrap();
    let res = client
        .get(format!("http://{}/start", src.addr()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), primp::StatusCode::OK);

    let dst_cookie: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let dst_cookie2 = Arc::clone(&dst_cookie);
    let dst2 = server::http(move |req| {
        let dst_cookie2 = Arc::clone(&dst_cookie2);
        async move {
            *dst_cookie2.lock().unwrap() = req
                .headers()
                .get(primp::header::COOKIE)
                .map(|v| v.to_str().unwrap().to_string());
            http::Response::builder()
                .status(200)
                .body(Body::default())
                .unwrap()
        }
    });
    client
        .get(format!("http://localhost:{}/direct", dst2.addr().port()))
        .send()
        .await
        .unwrap();
    assert_eq!(
        *dst_cookie.lock().unwrap(),
        Some("b=2".to_string()),
        "target cookie was not stored under the target host"
    );

    let src_cookie: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let src_cookie2 = Arc::clone(&src_cookie);
    let src2 = server::http(move |req| {
        let src_cookie2 = Arc::clone(&src_cookie2);
        async move {
            *src_cookie2.lock().unwrap() = req
                .headers()
                .get(primp::header::COOKIE)
                .map(|v| v.to_str().unwrap().to_string());
            http::Response::builder()
                .status(200)
                .body(Body::default())
                .unwrap()
        }
    });
    client
        .get(format!("http://{}/again", src2.addr()))
        .send()
        .await
        .unwrap();
    let saw = src_cookie.lock().unwrap().clone();
    assert!(
        saw.as_deref().map_or(true, |c| !c.contains("b=2")),
        "original host was sent the redirect target's cookie: {saw:?}"
    );
}

#[cfg(feature = "cookies")]
#[tokio::test]
async fn test_redirect_302_with_set_cookies() {
    let code = 302;
    let server = server::http(move |req| async move {
        if req.uri() == "/302" {
            http::Response::builder()
                .status(302)
                .header("location", "/dst")
                .header("set-cookie", "key=value")
                .body(Body::default())
                .unwrap()
        } else {
            assert_eq!(req.uri(), "/dst");
            assert_eq!(req.headers()["cookie"], "key=value");
            http::Response::default()
        }
    });

    let url = format!("http://{}/{}", server.addr(), code);
    let dst = format!("http://{}/{}", server.addr(), "dst");

    let client = primp::ClientBuilder::new()
        .no_proxy()
        .cookie_store(true)
        .build()
        .unwrap();
    let res = client.get(&url).send().await.unwrap();

    assert_eq!(res.url().as_str(), dst);
    assert_eq!(res.status(), primp::StatusCode::OK);
}

#[tokio::test]
#[ignore = "Needs TLS support in the test server"]
async fn test_redirect_https_only_enforced_gh1312() {
    let server = server::http(move |_req| async move {
        http::Response::builder()
            .status(302)
            .header("location", "http://insecure")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("https://{}/yikes", server.addr());

    let res = primp::Client::builder()
        .no_proxy()
        .tls_danger_accept_invalid_certs(true)
        .tls_backend_rustls()
        .https_only(true)
        .build()
        .expect("client builder")
        .get(&url)
        .send()
        .await;

    let err = res.unwrap_err();
    assert!(err.is_redirect());
}

#[tokio::test]
async fn test_redirect_to_unsupported_scheme_is_redirect_error() {
    let server = server::http(move |_req| async move {
        http::Response::builder()
            .status(302)
            .header("location", "ftp://example.com/file")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("http://{}/yikes", server.addr());

    let res = primp::Client::builder()
        .no_proxy()
        .build()
        .expect("client builder")
        .get(&url)
        .send()
        .await;

    let err = res.unwrap_err();
    // Redirect to an unsupported scheme must classify as a redirect error.
    assert!(err.is_redirect(), "expected redirect error, got: {err:?}");
}

#[tokio::test]
async fn test_redirect_limit_to_1() {
    let server = server::http(move |req| async move {
        let i: i32 = req
            .uri()
            .path()
            .rsplit('/')
            .next()
            .unwrap()
            .parse::<i32>()
            .unwrap();
        assert!(req.uri().path().ends_with(&format!("/redirect/{i}")));
        http::Response::builder()
            .status(302)
            .header("location", format!("/redirect/{}", i + 1))
            .body(Body::default())
            .unwrap()
    });
    // The number at the end of the uri indicates the total number of redirections
    let url = format!("http://{}/redirect/0", server.addr());

    let client = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::limited(1))
        .build()
        .unwrap();
    let res = client.get(&url).send().await.unwrap_err();
    // If the maximum limit is 1, then the final uri should be /redirect/1
    assert_eq!(
        res.url().unwrap().as_str(),
        format!("http://{}/redirect/1", server.addr()).as_str()
    );
    assert!(res.is_redirect());
}

#[tokio::test]
async fn test_redirect_custom() {
    let server = server::http(move |req| async move {
        assert!(req.uri().path().ends_with("/foo"));
        http::Response::builder()
            .status(302)
            .header("location", "/should_not_be_called")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("http://{}/foo", server.addr());

    let res = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::custom(|attempt| {
            if attempt.url().path().ends_with("/should_not_be_called") {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .unwrap()
        .get(&url)
        .send()
        .await
        .unwrap();

    assert_eq!(res.url().as_str(), url);
    assert_eq!(res.status(), primp::StatusCode::FOUND);
}

#[tokio::test]
async fn test_scheme_only_check_after_policy_return_follow() {
    let server = server::http(move |_| async move {
        http::Response::builder()
            .status(302)
            .header("location", "htt://www.yikes.com/")
            .body(Body::default())
            .unwrap()
    });

    let url = format!("http://{}/yikes", server.addr());
    let res = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::custom(|attempt| attempt.stop()))
        .build()
        .unwrap()
        .get(&url)
        .send()
        .await;

    assert!(res.is_ok());
    assert_eq!(res.unwrap().status(), primp::StatusCode::FOUND);

    let res = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::custom(|attempt| attempt.follow()))
        .build()
        .unwrap()
        .get(&url)
        .send()
        .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().is_redirect());
}

#[tokio::test]
async fn test_redirect_301_302_303_empty_payload_headers() {
    let client = test_client();
    let codes = [301u16, 302, 303];
    for &code in &codes {
        let redirect = server::http(move |mut req| async move {
            if req.method() == "POST" {
                let data = req
                    .body_mut()
                    .frame()
                    .await
                    .unwrap()
                    .unwrap()
                    .into_data()
                    .unwrap();

                assert_eq!(&*data, b"Hello");
                if req.headers().get(primp::header::CONTENT_LENGTH).is_some() {
                    assert_eq!(req.headers()[primp::header::CONTENT_LENGTH], "5");
                }
                assert_eq!(req.uri(), &*format!("/{code}"));

                http::Response::builder()
                    .header("location", "/dst")
                    .header("server", "test-dst")
                    .status(code)
                    .body(Body::default())
                    .unwrap()
            } else {
                assert_eq!(req.method(), "GET");
                assert!(req.headers().get(primp::header::CONTENT_TYPE).is_none());
                assert!(req.headers().get(primp::header::CONTENT_LENGTH).is_none());
                assert!(req.headers().get(primp::header::CONTENT_ENCODING).is_none());
                http::Response::builder()
                    .header("server", "test-dst")
                    .body(Body::default())
                    .unwrap()
            }
        });

        let url = format!("http://{}/{}", redirect.addr(), code);
        let dst = format!("http://{}/{}", redirect.addr(), "dst");
        let res = client
            .post(&url)
            .body("Hello")
            .header(primp::header::CONTENT_TYPE, "text/plain")
            .header(primp::header::CONTENT_LENGTH, "5")
            .header(primp::header::CONTENT_ENCODING, "identity")
            .send()
            .await
            .unwrap();
        assert_eq!(res.url().as_str(), dst);
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers().get(primp::header::SERVER).unwrap(),
            &"test-dst"
        );
    }
}

/// Regression test: the redirect policy is shared across requests via an
/// `Arc`, but the per-request chain of URLs must not leak between requests.
/// If the chain leaked, the second (single-hop) request below would
/// incorrectly report `TooManyRedirects` because the accumulated `urls`
/// from the first request would already exceed the limit.
#[tokio::test]
async fn test_redirect_policy_state_not_polluted_across_requests() {
    let server = server::http(move |req| async move {
        let n: i32 = req
            .uri()
            .path()
            .rsplit('/')
            .next()
            .unwrap()
            .parse()
            .unwrap_or(-1);
        let next = if n < 0 { 1 } else { n + 1 };
        // Stop the chain at /2 so the first request follows exactly 2 hops.
        if n >= 2 {
            http::Response::builder().body(Body::default()).unwrap()
        } else {
            http::Response::builder()
                .status(302)
                .header("location", format!("/{}", next))
                .body(Body::default())
                .unwrap()
        }
    });

    let client = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::limited(2))
        .build()
        .unwrap();

    let first = format!("http://{}/start", server.addr());
    let res = client.get(&first).send().await.unwrap();
    assert_eq!(res.url().as_str(), format!("http://{}/2", server.addr()));

    // Second request: a single hop. If the policy state leaked, this would
    // error with TooManyRedirects since the prior chain would still be counted.
    let second = format!("http://{}/start", server.addr());
    let res = client.get(&second).send().await.unwrap();
    assert_eq!(res.url().as_str(), format!("http://{}/2", server.addr()));
}

/// A per-request `redirect_override` enables redirects on a
/// `Policy::none()` client without mutating the shared client.
#[tokio::test]
async fn test_redirect_override_enables_redirects_on_none_client() {
    let server = server::http(move |req| async move {
        if req.uri().path() == "/start" {
            http::Response::builder()
                .status(302)
                .header("location", "/dst")
                .header("server", "test-redirect")
                .body(Body::default())
                .unwrap()
        } else {
            assert_eq!(req.uri().path(), "/dst");
            http::Response::builder()
                .header("server", "test-dst")
                .body(Body::default())
                .unwrap()
        }
    });

    let client = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::none())
        .build()
        .unwrap();

    let url = format!("http://{}/start", server.addr());
    let dst = format!("http://{}/dst", server.addr());

    // Without an override: the 302 is returned as-is.
    let res = client.get(&url).send().await.unwrap();
    assert_eq!(res.status(), primp::StatusCode::FOUND);
    assert_eq!(res.url().as_str(), url);

    // With a per-request override: the redirect is followed.
    let res = client
        .get(&url)
        .redirect_override(primp::RedirectOverride::Follow(5))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), primp::StatusCode::OK);
    assert_eq!(res.url().as_str(), dst);
    assert_eq!(
        res.headers().get(primp::header::SERVER).unwrap(),
        "test-dst"
    );

    // The client's own policy is untouched: still `none`.
    let res = client.get(&url).send().await.unwrap();
    assert_eq!(res.status(), primp::StatusCode::FOUND);
}

/// `RedirectOverride::Follow` honors its OWN max: `Follow(2)` on a 10-hop
/// chain must raise a redirect error after 2 hops.
#[tokio::test]
async fn test_redirect_override_follow_honors_own_max() {
    let server = server::http(move |req| async move {
        let n: u32 = req
            .uri()
            .path()
            .trim_start_matches('/')
            .parse()
            .unwrap_or(0);
        if n > 1 {
            http::Response::builder()
                .status(302)
                .header("location", format!("/{}", n - 1))
                .body(Body::default())
                .unwrap()
        } else {
            http::Response::builder()
                .header("server", "test-dst")
                .body(Body::default())
                .unwrap()
        }
    });

    let client = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::none())
        .build()
        .unwrap();

    let url = format!("http://{}/10", server.addr());

    let err = client
        .get(&url)
        .redirect_override(primp::RedirectOverride::Follow(2))
        .send()
        .await
        .unwrap_err();
    assert!(err.is_redirect(), "expected a redirect error, got: {err}");
}

/// `RedirectOverride::Disabled` stops redirects on a follow client without
/// mutating the shared client.
#[tokio::test]
async fn test_redirect_override_disables_redirects_on_follow_client() {
    let server = server::http(move |req| async move {
        if req.uri().path() == "/start" {
            http::Response::builder()
                .status(302)
                .header("location", "/dst")
                .body(Body::default())
                .unwrap()
        } else {
            http::Response::builder().body(Body::default()).unwrap()
        }
    });

    let client = primp::Client::builder()
        .no_proxy()
        .redirect(primp::redirect::Policy::limited(5))
        .build()
        .unwrap();

    let url = format!("http://{}/start", server.addr());

    // Per-request disable wins over the client policy.
    let res = client
        .get(&url)
        .redirect_override(primp::RedirectOverride::Disabled)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), primp::StatusCode::FOUND);

    // The client's own policy is untouched: the next request still follows.
    let res = client.get(&url).send().await.unwrap();
    assert_eq!(res.status(), primp::StatusCode::OK);
}
