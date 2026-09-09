use primp::header::HeaderValue;
use primp::{Certificate, Proxy, Url};

#[test]
fn into_url_rejects_non_http_schemes() {
    // ftp:// should be rejected as proxy
    let proxy = Proxy::all("ftp://example.com");
    assert!(proxy.is_err(), "ftp proxy should be rejected");
    // http and https should pass
    assert!(Proxy::all("http://proxy.example:8080").is_ok());
    assert!(Proxy::all("socks5://proxy.example:1080").is_ok());
    // Url with ftp scheme as proxy should also fail
    let ftp_url = Url::parse("ftp://example.com").unwrap();
    let ftp_proxy_via_url = Proxy::all(ftp_url);
    assert!(ftp_proxy_via_url.is_err());
}

#[test]
fn proxy_custom_smoke() {
    // Public-API smoke only (not a regression guard): custom proxy closure
    // construction succeeds and Url parsing preserves path/query. The real
    // intercept path (closure receives path/query) is unit-tested at
    // `primp::proxy::tests::test_custom_closure_sees_path_and_query`;
    // cross-host wiring is covered by proxy_review integration tests.
    use primp::Proxy;
    let target = Url::parse("http://proxy.example:8080").unwrap();
    let test_url = Url::parse("http://example.com/api/v1?q=1").unwrap();
    assert_eq!(test_url.path(), "/api/v1");
    assert_eq!(test_url.query(), Some("q=1"));

    let proxy = Proxy::custom({
        let target = target.clone();
        move |url| {
            if url.path() == "/api/v1" && url.query() == Some("q=1") {
                Some(target.clone())
            } else {
                None::<Url>
            }
        }
    });
    // Construction via public API must succeed (intercept wiring verified
    // in proxy_review.rs cross_host tests).
    let _ = proxy;
    // IPv6 parsing: host() yields Ipv6 without double brackets.
    let ipv6 = Url::parse("http://[::1]:8080/").unwrap();
    assert!(ipv6.as_str().contains("[::1]"));
    assert_eq!(
        ipv6.host(),
        Some(url::Host::Ipv6(std::net::Ipv6Addr::LOCALHOST))
    );
}

#[test]
fn proxy_debug_redacts_credentials_but_shows_kind() {
    let proxy = Proxy::all("http://user:secret@proxy.example:8080/")
        .unwrap()
        .basic_auth("Aladdin", "open sesame");
    let debug = format!("{:?}", proxy);
    assert!(!debug.contains("secret"), "leaked password: {debug}");
    assert!(
        !debug.contains("open sesame"),
        "leaked password via basic_auth: {debug}"
    );
    assert!(!debug.contains("Aladdin"), "leaked username: {debug}");
    // Proxy Debug should be redacted
    assert!(debug.contains("***"), "should contain redacted marker");
}

#[test]
fn proxy_basic_auth_build_smoke() {
    // Smoke: Proxy with HTAB auth builds and doesn't panic. The real HTAB
    // decode guard lives in `proxy::tests::decode_basic_auth_tolerates_trailing_ows`;
    // this path stores via `custom_http_auth` and only asserts build.
    let proxy = Proxy::all("http://proxy.example:8080")
        .unwrap()
        .custom_http_auth(HeaderValue::from_str("Basic\t dXNlcjpwYXNz").unwrap());
    // Building client with this proxy should succeed (decode handles HTAB)
    let client = primp::Client::builder().proxy(proxy).build();
    assert!(client.is_ok());
}

#[test]
fn credentials_stripped_even_on_decode_failure() {
    // Via public RequestBuilder API: userinfo must be stripped even when percent-decode fails
    let vectors = [
        "http://user%FF:pass@example.com/",
        "http://%FF:pass@example.com/",
        "http://%FF%FE@example.com/",
        "http://u%FF:p@example.com/",
        "http://user:%FF@example.com/",
    ];
    for url in vectors {
        let req = primp::Client::new().get(url).build().expect("build");
        let parsed = req.url();
        assert!(
            parsed.username().is_empty() && parsed.password().is_none(),
            "leaked userinfo for {url}: {}",
            parsed.as_str()
        );
        assert!(
            !parsed.as_str().contains('@'),
            "URL still contains @ for {url}"
        );
    }
    // Valid credentials should be stripped and set as Authorization
    let req = primp::Client::new()
        .get("http://alice:secret@example.com/")
        .build()
        .unwrap();
    assert_eq!(req.url().as_str(), "http://example.com/");
    assert_eq!(
        req.headers().get("authorization").unwrap(),
        "Basic YWxpY2U6c2VjcmV0"
    );
    // Username decode failure should strip but not set auth (no valid username)
    let req = primp::Client::new()
        .get("http://user%FF:pass@example.com/")
        .build()
        .unwrap();
    assert!(req.headers().get("authorization").is_none());
    assert_eq!(req.url().as_str(), "http://example.com/");
}

#[test]
fn pool_idle_timeout_build_smoke() {
    // Build-level smoke (not eviction semantics): both None (disable
    // eviction) and Some build. Eviction semantics incl. the None path are
    // unit-tested at
    // `h1_client::pool::tests::evict_stale_none_disables_timeout_eviction`.
    let _client = primp::Client::builder()
        .pool_idle_timeout(None)
        .build()
        .expect("build with None");
    let with_timeout = primp::Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build();
    assert!(with_timeout.is_ok());
}

#[test]
fn tls_pem_validation_rejects_empty() {
    let empty = b"";
    let cert = Certificate::from_pem(empty);
    assert!(cert.is_err(), "empty PEM should be rejected");
    let bundle = Certificate::from_pem_bundle(empty);
    assert!(bundle.is_err(), "empty bundle should be rejected");
    // Whitespace-only and garbage must also be rejected, not silently accepted.
    for bad in [b"   \n\t  ".as_slice(), b"not a pem at all".as_slice()] {
        assert!(Certificate::from_pem(bad).is_err());
        assert!(Certificate::from_pem_bundle(bad).is_err());
    }
}

#[test]
fn h2_header_client_smoke() {
    // Client-level smoke only (not h2 grouping): duplicate headers preserve
    // order at the request-builder layer. Real h2 continuation grouping
    // under `header_order` is unit-tested at
    // `primp-h2 frame::headers::test::duplicate_headers_stay_grouped_under_header_order`.
    let client = primp::Client::new();
    let req = client
        .get("http://example.com/")
        .header(http::header::COOKIE, "a=1")
        .header(http::header::COOKIE, "b=2")
        .build()
        .unwrap();
    let cookies: Vec<_> = req.headers().get_all(http::header::COOKIE).iter().collect();
    assert_eq!(cookies.len(), 2);
    assert_eq!(cookies[0], "a=1");
    assert_eq!(cookies[1], "b=2");
}
