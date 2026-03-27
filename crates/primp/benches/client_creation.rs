use criterion::{criterion_group, criterion_main, Criterion};
use primp::{Client, Impersonate};

fn bench_client_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("client_creation");

    // Benchmark basic client creation (no impersonation)
    group.bench_function("basic", |b| {
        b.iter(|| Client::builder().build().unwrap());
    });

    // Benchmark Chrome impersonation
    group.bench_function("chrome_146", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::ChromeV146)
                .build()
                .unwrap()
        });
    });

    // Benchmark Firefox impersonation
    group.bench_function("firefox_148", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::FirefoxV148)
                .build()
                .unwrap()
        });
    });

    // Benchmark Safari impersonation
    group.bench_function("safari_26.3", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::SafariV26_3)
                .build()
                .unwrap()
        });
    });

    // Benchmark Edge impersonation
    group.bench_function("edge_146", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::EdgeV146)
                .build()
                .unwrap()
        });
    });

    // Benchmark Opera impersonation
    group.bench_function("opera_129", |b| {
        b.iter(|| {
            Client::builder()
                .impersonate(Impersonate::OperaV129)
                .build()
                .unwrap()
        });
    });

    group.finish();
}

criterion_group!(benches, bench_client_creation);
criterion_main!(benches);
