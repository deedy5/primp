use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use criterion::{criterion_group, criterion_main, Criterion};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use primp::cookie::{CookieStore, Jar};
use primp::header::{HeaderMap, HeaderValue};
use primp::{Client, Impersonate};
use tokio::net::TcpListener;

async fn run_server(listener: TcpListener) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(v) => v,
            Err(_) => break,
        };
        let io = TokioIo::new(stream);
        tokio::spawn(async move {
            let service = service_fn(|_req: Request<hyper::body::Incoming>| async move {
                Ok::<_, Infallible>(Response::new(http_body_util::Empty::<Bytes>::new()))
            });
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, service)
                .await;
        });
    }
}

/// Spawns a live local server on the given runtime and returns its base URL.
///
/// The server runs as a task on the *same* runtime used by the client so that
/// a single-threaded runtime still drives both sides during `block_on`.
fn live_server(rt: &tokio::runtime::Runtime) -> String {
    let listener = rt.block_on(async { TcpListener::bind("127.0.0.1:0").await.unwrap() });
    let addr: SocketAddr = listener.local_addr().unwrap();
    rt.spawn(run_server(listener));
    format!("http://{addr}/")
}

fn bench_request_roundtrip(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let url = live_server(&rt);

    let mut group = c.benchmark_group("request");

    // Plain client, kept-alive connection pool: isolates request build +
    // hyper send/recv from TLS and per-request TCP connect after warmup.
    let client = Client::builder().build().unwrap();
    group.bench_function("plain", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = client.get(&url).send().await.unwrap();
            });
        });
    });

    // With default headers set, to exercise the per-request default-header merge.
    let mut default_headers = HeaderMap::new();
    default_headers.insert("X-Bench", HeaderValue::from_static("1"));
    let client = Client::builder()
        .default_headers(default_headers)
        .build()
        .unwrap();
    group.bench_function("with_default_header", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = client.get(&url).send().await.unwrap();
            });
        });
    });

    // Impersonation client build + request (Chrome fingerprint).
    // NOTE: the bench server is plain `http://`, so this measures the
    // impersonation header/settings path only — the TLS ClientHello
    // emulator is never exercised here (a TLS server would be needed).
    // The response is dropped without reading the body (Empty body, fine).
    let client = Client::builder()
        .impersonate(Impersonate::ChromeV146)
        .build()
        .unwrap();
    group.bench_function("impersonate_chrome", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = client.get(&url).send().await.unwrap();
            });
        });
    });

    group.finish();
}

fn bench_cookie_jar(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let url = url::Url::parse("http://example.com/").unwrap();

    let mut group = c.benchmark_group("cookie_jar");

    // Read path: build the outgoing `Cookie` header from a populated jar.
    group.bench_function("get", |b| {
        let jar = Jar::default();
        for i in 0..8 {
            jar.add_cookie_str(&format!("name{i}=value{i}; Domain=example.com"), &url);
        }
        b.iter(|| {
            let _ = jar.cookies(&url);
        });
    });

    // Write path: store Set-Cookie headers into the jar.
    group.bench_function("set", |b| {
        let jar = Jar::default();
        let hdrs: Vec<HeaderValue> = (0..8)
            .map(|i| HeaderValue::from_str(&format!("name{i}=value{i}")).unwrap())
            .collect();
        b.iter(|| {
            jar.set_cookies(&mut hdrs.iter(), &url);
        });
    });

    // Round-trip with cookies enabled through the full client.
    let srv_url = live_server(&rt);
    let client = Client::builder().cookie_store(true).build().unwrap();
    group.bench_function("request_with_cookies", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = client.get(&srv_url).send().await.unwrap();
            });
        });
    });

    group.finish();
}

criterion_group!(benches, bench_request_roundtrip, bench_cookie_jar);
criterion_main!(benches);
