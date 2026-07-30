use primp::cookie::CookieStore;

#[test]
fn public_suffix_domain_cookie_is_rejected() {
    let jar = primp::cookie::Jar::default();
    let evil: primp::Url = "http://evil.com/".parse().unwrap();
    let _ = jar.add_cookie_str("x=1; Domain=.com", &evil);
    let _ = jar.add_cookie_str("y=2; Domain=co.uk", &evil);

    let bank: primp::Url = "http://bank.com/".parse().unwrap();
    let shop: primp::Url = "http://shop.co.uk/".parse().unwrap();
    assert!(jar.cookies(&bank).is_none(), "cookie leaked to bank.com");
    assert!(jar.cookies(&shop).is_none(), "cookie leaked to shop.co.uk");
}

#[test]
fn registrable_domain_cookie_is_stored() {
    let jar = primp::cookie::Jar::default();
    let url: primp::Url = "http://shop.example.com/".parse().unwrap();
    let _ = jar.add_cookie_str("x=1; Domain=example.com", &url);
    assert!(jar.cookies(&url).is_some());
}

#[test]
fn set_cookies_rejects_public_suffix_domain() {
    let jar = primp::cookie::Jar::default();
    let headers = vec![
        primp::header::HeaderValue::from_static("x=1; Domain=.com"),
        primp::header::HeaderValue::from_static("y=2"),
    ];
    let evil: primp::Url = "http://evil.com/".parse().unwrap();
    jar.set_cookies(&mut headers.iter(), &evil);

    let bank: primp::Url = "http://bank.com/".parse().unwrap();
    assert!(jar.cookies(&bank).is_none(), "cookie leaked to bank.com");
    assert!(
        jar.cookies(&evil).is_some(),
        "host-only cookie still stored"
    );
}

#[test]
fn host_only_cookie_is_stored() {
    let jar = primp::cookie::Jar::default();
    let url: primp::Url = "http://example.com/".parse().unwrap();
    let _ = jar.add_cookie_str("x=1", &url);
    assert!(jar.cookies(&url).is_some());
}
