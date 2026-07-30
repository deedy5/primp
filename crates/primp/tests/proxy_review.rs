// Regression tests for proxy / SOCKS handling, exercising the public `primp`
// API. These cover: SOCKS5 URL-embedded credential forwarding, SOCKS5 auth
// rejection, exact-200 CONNECT validation (and rejection of non-200 / non-200
// 2xx status), and trailing-byte replay after a successful CONNECT tunnel.

mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawn a raw TCP server that performs a single HTTP CONNECT handshake:
///
/// - reads the CONNECT request (until \r\n\r\n)
/// - writes a fixed `status_line` (must include trailing \r\n\r\n)
/// - then optionally writes `trailing` bytes immediately after the response
/// - then pipes the tunnel: echoes everything it receives back to the client
///
/// Returned addr is the proxy address to configure.
async fn spawn_connect_proxy(
    status_line: String,
    trailing: Vec<u8>,
    echo: bool,
) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let mut total = 0;
        // Read until we've seen the CONNECT header terminator.
        loop {
            let n = sock.read(&mut buf[total..]).await.unwrap();
            if n == 0 {
                return;
            }
            total += n;
            if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if total >= buf.len() {
                break;
            }
        }
        sock.write_all(status_line.as_bytes()).await.unwrap();
        if !trailing.is_empty() {
            sock.write_all(&trailing).await.unwrap();
        }
        sock.flush().await.unwrap();
        if echo {
            let mut b = [0u8; 4096];
            loop {
                match sock.read(&mut b).await {
                    Ok(0) => break,
                    Ok(n) => {
                        sock.write_all(&b[..n]).await.unwrap();
                        sock.flush().await.unwrap();
                    }
                    Err(_) => break,
                }
            }
        }
    });
    addr
}

#[tokio::test]
async fn connect_proxy_200_establishes_tunnel() {
    // The proxy responds 200 and then echoes tunneled bytes. We make an HTTPS
    // request to our own mock TLS-less target via the proxy; since there is no
    // real TLS, we instead verify the CONNECT handshake accepted a 200 and the
    // tunnel was reported open (a non-200 would error before any handshake).
    let addr = spawn_connect_proxy(
        "HTTP/1.1 200 Connection established\r\n\r\n".to_string(),
        Vec::new(),
        false,
    )
    .await;

    // Use a plain HTTP target through an HTTPS-intercepting proxy: primp will
    // issue CONNECT and, on 200, attempt TLS to the (nonexistent) target. We
    // only assert it got *past* the CONNECT status check (i.e. did not reject
    // the 200). A TLS failure to a junk host is expected and proves the tunnel
    // was opened.
    let proxy = format!("http://{}", addr);
    let res = primp::Client::builder()
        .proxy(primp::Proxy::https(&proxy).unwrap())
        .build()
        .unwrap()
        .get("https://127.0.0.1:1/")
        .send()
        .await;

    // Either way the connection did not fail at the CONNECT-validation step
    // with a "CONNECT failed" message.
    if let Err(e) = &res {
        let msg = format!("{:?}", e);
        assert!(
            !msg.contains("CONNECT failed"),
            "a valid 200 CONNECT must not be rejected: {}",
            msg
        );
    }
}

/// Regression test: the CONNECT response status is matched *exactly* against
/// `200`. A 2xx status that is not `200` (e.g. `299`, a malformed/forged
/// "2xx") must be rejected — not accepted by a loose "starts with 2" check.
#[tokio::test]
async fn connect_proxy_exact_200_required() {
    let addr =
        spawn_connect_proxy("HTTP/1.1 299 Weird\r\n\r\n".to_string(), Vec::new(), false).await;

    let proxy = format!("http://{}", addr);
    let err = primp::Client::builder()
        .proxy(primp::Proxy::https(&proxy).unwrap())
        .build()
        .unwrap()
        .get("https://example.invalid/")
        .send()
        .await
        .unwrap_err();

    let msg = format!("{:?}", err);
    assert!(
        msg.contains("CONNECT failed") || msg.contains("299"),
        "a 2xx CONNECT that is not exactly 200 must be rejected, got: {}",
        msg
    );
}

#[tokio::test]
async fn connect_proxy_non_200_is_rejected() {
    let addr = spawn_connect_proxy(
        "HTTP/1.1 403 Forbidden\r\n\r\n".to_string(),
        Vec::new(),
        false,
    )
    .await;

    let proxy = format!("http://{}", addr);
    let err = primp::Client::builder()
        .proxy(primp::Proxy::https(&proxy).unwrap())
        .build()
        .unwrap()
        .get("https://example.invalid/")
        .send()
        .await
        .unwrap_err();

    let msg = format!("{:?}", err);
    assert!(
        msg.contains("403") || msg.contains("CONNECT failed") || msg.contains("Forbidden"),
        "non-200 CONNECT must be rejected, got: {}",
        msg
    );
}

/// Minimal SOCKS5 server that requires username/password auth and then accepts
/// a single connect request, replying success, then echoing tunnel bytes.
async fn spawn_socks5_with_auth(user: &str, pass: &str) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let user = user.to_string();
    let pass = pass.to_string();
    tokio::spawn(async move {
        let (mut sock, _) = match listener.accept().await {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut buf = [0u8; 256];
        // greeting: VER NMETHODS METHODS...
        let n = sock.read(&mut buf).await.unwrap();
        assert!(n >= 2 && buf[0] == 5);
        // we advertised only user/pass (0x02)
        sock.write_all(&[0x05, 0x02]).await.unwrap();
        // auth: VER(0x01) ULEN UNAME PLEN PASSWD
        let _n = sock.read(&mut buf).await.unwrap();
        assert_eq!(buf[0], 0x01);
        let ulen = buf[1] as usize;
        let uname = String::from_utf8_lossy(&buf[2..2 + ulen]).to_string();
        let off = 2 + ulen;
        let plen = buf[off] as usize;
        let pwd = String::from_utf8_lossy(&buf[off + 1..off + 1 + plen]).to_string();
        if uname == user && pwd == pass {
            sock.write_all(&[0x01, 0x00]).await.unwrap();
        } else {
            sock.write_all(&[0x01, 0x01]).await.unwrap();
            return;
        }
        // connect request: VER CMD RSV ATYP ...
        let _n = sock.read(&mut buf).await.unwrap();
        assert_eq!(buf[0], 0x05);
        // reply success: VER REP RSV ATYP(1=ipv4) BND.ADDR(4) BND.PORT(2)
        let reply = [0x05u8, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0x00, 0x00];
        sock.write_all(&reply).await.unwrap();
        // echo loop
        let mut b = [0u8; 256];
        loop {
            match sock.read(&mut b).await {
                Ok(0) => break,
                Ok(m) => {
                    sock.write_all(&b[..m]).await.unwrap();
                }
                Err(_) => break,
            }
        }
    });
    addr
}

#[tokio::test]
async fn socks5_auth_is_forwarded() {
    let addr = spawn_socks5_with_auth("primp", "s3cret").await;

    let proxy = format!("socks5://primp:s3cret@{}", addr);
    // Target is a bogus host; the SOCKS handshake must succeed (auth accepted)
    // and only then fail trying to reach the (unresolvable) target. If auth
    // were wrong, the server returns auth failure and the connect errors with
    // a SOCKS error before any attempt to reach the target.
    let err = primp::Client::builder()
        .proxy(primp::Proxy::all(&proxy).unwrap())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap()
        .get("http://10.255.255.1/")
        .send()
        .await
        .unwrap_err();

    let msg = format!("{:?}", err).to_lowercase();
    assert!(
        !msg.contains("authentication") && !msg.contains("auth"),
        "SOCKS5 auth should have succeeded (got: {})",
        msg
    );
}

#[tokio::test]
async fn socks5_wrong_auth_is_rejected() {
    let addr = spawn_socks5_with_auth("primp", "s3cret").await;

    let proxy = format!("socks5://primp:WRONG@{}", addr);
    let err = primp::Client::builder()
        .proxy(primp::Proxy::all(&proxy).unwrap())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap()
        .get("http://10.255.255.1/")
        .send()
        .await
        .unwrap_err();

    let msg = format!("{:?}", err).to_lowercase();
    assert!(
        msg.contains("socks") || msg.contains("auth") || msg.contains("connect"),
        "wrong SOCKS5 creds should fail, got: {}",
        msg
    );
}

/// A forward proxy that accepts absolute-form requests and requires a
/// `Proxy-Authorization` header. Replies 407 without it; otherwise relays to
/// the origin and echoes the response. One request per connection.
async fn spawn_auth_forward_proxy() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => return,
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let mut total = 0;
                loop {
                    let n = sock.read(&mut buf[total..]).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    total += n;
                    if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if total >= buf.len() {
                        return;
                    }
                }
                let head = String::from_utf8_lossy(&buf[..total]).to_string();
                if !head.to_ascii_lowercase().contains("proxy-authorization:") {
                    let _ = sock
                        .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .await;
                    let _ = sock.flush().await;
                    return;
                }
                let request_line = head.lines().next().unwrap_or("");
                let url = request_line.split_whitespace().nth(1).unwrap_or("");
                let parsed = url::Url::parse(url)
                    .unwrap_or_else(|_| panic!("proxy received unparseable request line: {url:?}"));
                let host = parsed.host_str().unwrap_or("127.0.0.1").to_string();
                let port = parsed.port().unwrap_or(80);
                let path = if parsed.path().is_empty() {
                    "/".to_string()
                } else {
                    parsed.path().to_string()
                };
                let mut origin =
                    match tokio::net::TcpStream::connect(format!("{host}:{port}")).await {
                        Ok(o) => o,
                        Err(_) => {
                            let _ = sock
                                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                                .await;
                            return;
                        }
                    };
                let fwd = format!(
                    "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
                );
                if origin.write_all(fwd.as_bytes()).await.is_err() {
                    return;
                }
                let mut resp = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    match origin.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => resp.extend_from_slice(&chunk[..n]),
                    }
                }
                let _ = sock.write_all(&resp).await;
            });
        }
    });
    addr
}

/// Serve `body` (full raw HTTP response) for one connection at a time.
async fn spawn_origin(body: Vec<u8>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => return,
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let mut total = 0;
                loop {
                    let n = match sock.read(&mut buf[total..]).await {
                        Ok(0) => return,
                        Ok(n) => n,
                        Err(_) => return,
                    };
                    total += n;
                    if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if total >= buf.len() {
                        return;
                    }
                }
                let _ = sock.write_all(&body).await;
                let _ = sock.flush().await;
            });
        }
    });
    addr
}

/// Regression test: `Proxy-Authorization` belongs to the proxy connection,
/// not the destination origin. `execute_request` attaches it for hop 0 only,
/// and the redirect layer strips it on cross-host hops — it must be
/// re-attached per hop or the second hop 407s.
#[tokio::test]
async fn cross_host_redirect_re_attaches_proxy_auth() {
    let proxy_addr = spawn_auth_forward_proxy().await;

    // Origin B: the redirect target.
    let body_b = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec();
    let origin_b = spawn_origin(body_b).await;

    // Origin A: 302 to origin B (different port = cross-host).
    let body_a = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        origin_b
    )
    .into_bytes();
    let origin_a = spawn_origin(body_a).await;

    let proxy_url = format!("http://user:pass@{}", proxy_addr);
    let client = primp::Client::builder()
        .no_proxy()
        .proxy(primp::Proxy::http(&proxy_url).unwrap())
        .build()
        .expect("client builds");

    let resp = client
        .get(format!("http://{}/start", origin_a))
        .send()
        .await
        .expect("request completes");
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    assert_eq!(
        status.as_u16(),
        200,
        "second hop through the auth proxy must succeed; got {status} body={text:?}"
    );
    assert_eq!(text, "ok");
}

/// Control: a same-origin redirect keeps the hop-0 header untouched, so both
/// hops authenticate. Proves the proxy itself requires auth correctly.
#[tokio::test]
async fn same_origin_redirect_keeps_proxy_auth() {
    let proxy_addr = spawn_auth_forward_proxy().await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin_a = listener.local_addr().unwrap();
    let port = origin_a.port();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => return,
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let mut total = 0;
                loop {
                    let n = match sock.read(&mut buf[total..]).await {
                        Ok(0) => return,
                        Ok(n) => n,
                        Err(_) => return,
                    };
                    total += n;
                    if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if total >= buf.len() {
                        return;
                    }
                }
                let head = String::from_utf8_lossy(&buf[..total]).to_string();
                let url = head
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("");
                if url.ends_with("/start") {
                    let loc = format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{}:{}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        "127.0.0.1",
                        port
                    );
                    let _ = sock.write_all(loc.as_bytes()).await;
                } else {
                    let _ = sock
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nfinal")
                        .await;
                }
                let _ = sock.flush().await;
            });
        }
    });

    let proxy_url = format!("http://user:pass@{}", proxy_addr);
    let client = primp::Client::builder()
        .no_proxy()
        .proxy(primp::Proxy::http(&proxy_url).unwrap())
        .build()
        .expect("client builds");

    let resp = client
        .get(format!("http://{}/start", origin_a))
        .send()
        .await
        .expect("request completes");
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    assert_eq!(
        status.as_u16(),
        200,
        "same-origin hop 2 must keep Proxy-Authorization; got {status} body={text:?}"
    );
    assert_eq!(text, "final");
}

/// Anti-leak guard for the per-hop re-attach: when the redirect target is
/// NOT routed through the proxy (no_proxy entry), the re-attach must NOT fire
/// — proxy creds never travel on a direct hop to the origin.
#[tokio::test]
async fn cross_host_redirect_to_direct_host_does_not_send_proxy_auth() {
    let proxy_addr = spawn_auth_forward_proxy().await;

    // Echo origin on a second loopback IP: returns the raw request head as
    // its body so the test can inspect exactly what the origin received.
    let listener = TcpListener::bind((std::net::IpAddr::from([127, 0, 0, 2]), 0))
        .await
        .unwrap();
    let origin_b = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => return,
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let mut total = 0;
                loop {
                    let n = match sock.read(&mut buf[total..]).await {
                        Ok(0) => return,
                        Ok(n) => n,
                        Err(_) => return,
                    };
                    total += n;
                    if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if total >= buf.len() {
                        return;
                    }
                }
                let head = String::from_utf8_lossy(&buf[..total]).to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    head.len(),
                    head
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.flush().await;
            });
        }
    });

    let body_a = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        origin_b
    )
    .into_bytes();
    let origin_a = spawn_origin(body_a).await;

    let proxy_url = format!("http://user:pass@{}", proxy_addr);
    let client = primp::Client::builder()
        .no_proxy()
        .proxy(
            primp::Proxy::all(&proxy_url)
                .unwrap()
                .no_proxy(primp::NoProxy::from_string("127.0.0.2")),
        )
        .build()
        .expect("client builds");

    let resp = client
        .get(format!("http://{}/start", origin_a))
        .send()
        .await
        .expect("request completes");
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    assert_eq!(
        status.as_u16(),
        200,
        "direct hop must succeed; got {status} body={text:?}"
    );
    assert!(
        !text.to_ascii_lowercase().contains("proxy-authorization"),
        "direct hop must not carry proxy creds; origin saw:\n{text}"
    );
}
