// Regression tests for SOCKS proxies with IPv6 destinations (F8).
//
// Background: socks4/socks5 LOCAL-DNS mode resolved IPv6 targets to a
// bracket-wrapped literal (`[::1]`) and passed that into hyper-util's socks
// services, which read `Uri::host()` (brackets kept) and fell into the DOMAIN
// path — the proxy received `"[::1]"` as a hostname and DNS-failed the
// tunnel. The handshake is now primp-owned: IPv6 goes out as SOCKS5 ATYP=4
// (native 16-byte address) and SOCKS4 (which cannot carry IPv6) fails with a
// clean error before any request bytes are sent.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Resolves every name to `::1:80`.
struct V6Resolver;

impl primp::dns::Resolve for V6Resolver {
    fn resolve(&self, _name: primp::dns::Name) -> primp::dns::Resolving {
        Box::pin(async move {
            let addrs: primp::dns::Addrs =
                Box::new([SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 80))].into_iter());
            Ok(addrs)
        })
    }
}

/// SOCKS5 no-auth server: replies to the greeting, records the CONNECT
/// request bytes, replies success, then closes.
async fn spawn_record_socks5() -> (SocketAddr, Arc<Mutex<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let recorded2 = recorded.clone();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 512];
        let n = sock.read(&mut buf).await.unwrap();
        assert!(
            buf[0] == 5 && n >= 2,
            "expected SOCKS5 greeting, got {buf:02x?}"
        );
        sock.write_all(&[0x05, 0x00]).await.unwrap();
        let n = sock.read(&mut buf).await.unwrap();
        recorded2.lock().unwrap().extend_from_slice(&buf[..n]);
        sock.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await
            .unwrap();
        let mut b = [0u8; 256];
        let _ = sock.read(&mut b).await;
    });
    (addr, recorded)
}

/// SOCKS4 server: records whatever arrives, closes without replying.
async fn spawn_record_socks4() -> (SocketAddr, Arc<Mutex<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let recorded2 = recorded.clone();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 512];
        match tokio::time::timeout(std::time::Duration::from_secs(3), sock.read(&mut buf)).await {
            Ok(Ok(n)) if n > 0 => recorded2.lock().unwrap().extend_from_slice(&buf[..n]),
            _ => {}
        }
    });
    (addr, recorded)
}

const V6_BYTES: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

/// socks5 (LOCAL-DNS): a resolved IPv6 target must reach the proxy as a
/// native IPv6 address (ATYP=0x04), never as a bracketed DOMAIN name.
#[tokio::test]
async fn socks5_local_dns_ipv6_sends_atyp4() {
    let (addr, recorded) = spawn_record_socks5().await;
    let client = primp::Client::builder()
        .proxy(primp::Proxy::all(format!("socks5://{addr}")).unwrap())
        .dns_resolver(V6Resolver)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let _ = client.get("http://ipv6-target.invalid/").send().await;

    let bytes = recorded.lock().unwrap().clone();
    // VER CMD RSV ATYP ADDR[16] PORT[2]
    assert_eq!(
        &bytes[..4],
        &[0x05, 0x01, 0x00, 0x04],
        "CONNECT request must be ATYP=IPv6, got {bytes:02x?}"
    );
    assert_eq!(
        &bytes[4..20],
        &V6_BYTES,
        "expected ::1 address, got {bytes:02x?}"
    );
}

/// socks5h (remote DNS) with an IPv6-literal request URL: the bracket-stripped
/// literal must go out as ATYP=0x04, not as a DOMAIN.
#[tokio::test]
async fn socks5h_ipv6_literal_sends_atyp4() {
    let (addr, recorded) = spawn_record_socks5().await;
    let client = primp::Client::builder()
        .proxy(primp::Proxy::all(format!("socks5h://{addr}")).unwrap())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let _ = client.get("http://[::1]:80/").send().await;

    let bytes = recorded.lock().unwrap().clone();
    assert_eq!(
        &bytes[..4],
        &[0x05, 0x01, 0x00, 0x04],
        "CONNECT request must be ATYP=IPv6, got {bytes:02x?}"
    );
    assert_eq!(
        &bytes[4..20],
        &V6_BYTES,
        "expected ::1 address, got {bytes:02x?}"
    );
}

/// socks4 with an IPv6 destination is a protocol limitation: it must fail
/// with a clean error before any request bytes reach the proxy.
#[tokio::test]
async fn socks4_ipv6_is_a_clean_error() {
    let (addr, recorded) = spawn_record_socks4().await;
    let client = primp::Client::builder()
        .proxy(primp::Proxy::all(format!("socks4://{addr}")).unwrap())
        .dns_resolver(V6Resolver)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let err = client
        .get("http://ipv6-target.invalid/")
        .send()
        .await
        .unwrap_err();

    let msg = format!("{:?}", err);
    assert!(
        msg.contains("IPv6"),
        "socks4 + IPv6 must fail with a clear IPv6 error, got: {msg}"
    );

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let bytes = recorded.lock().unwrap().clone();
    assert!(
        bytes.is_empty(),
        "no SOCKS4 request bytes may be sent for an IPv6 destination, got {bytes:02x?}"
    );
}
