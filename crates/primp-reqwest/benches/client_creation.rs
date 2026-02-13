use criterion::{criterion_group, criterion_main, Criterion};
use reqwest::{Client, Impersonate};

fn bench_client_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("client_creation");

    // Benchmark basic client creation (no impersonation)
    group.bench_function("basic", |b| {
        b.iter(|| Client::builder().build().unwrap());
    });

    // Benchmark Chrome impersonation
    group.bench_function("chrome_144", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::ChromeV144)
                .build()
                .unwrap()
        });
    });

    group.bench_function("chrome_145", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::ChromeV145)
                .build()
                .unwrap()
        });
    });

    // Benchmark Firefox impersonation
    group.bench_function("firefox_140", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::FirefoxV140)
                .build()
                .unwrap()
        });
    });

    group.bench_function("firefox_146", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::FirefoxV146)
                .build()
                .unwrap()
        });
    });

    // Benchmark Safari impersonation
    group.bench_function("safari_18_5", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::SafariV18_5)
                .build()
                .unwrap()
        });
    });

    group.bench_function("safari_26", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::SafariV26)
                .build()
                .unwrap()
        });
    });

    // Benchmark Edge impersonation
    group.bench_function("edge_144", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::EdgeV144)
                .build()
                .unwrap()
        });
    });

    group.bench_function("edge_145", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::EdgeV145)
                .build()
                .unwrap()
        });
    });

    // Benchmark Opera impersonation
    group.bench_function("opera_126", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::OperaV126)
                .build()
                .unwrap()
        });
    });

    group.bench_function("opera_127", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::OperaV127)
                .build()
                .unwrap()
        });
    });

    group.finish();
}

criterion_group!(benches, bench_client_creation);
criterion_main!(benches);
