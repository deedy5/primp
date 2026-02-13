#![cfg(not(target_arch = "wasm32"))]
#![cfg(not(feature = "rustls-no-provider"))]

// Network-dependent tests - these tests require external network access
// They verify that each impersonate profile can successfully make requests to various URLs

#[cfg(feature = "impersonate")]
use reqwest::{Client, Impersonate};
#[cfg(feature = "impersonate")]
use std::time::Duration;
#[cfg(feature = "impersonate")]
use tokio::task::JoinSet;

/// Timeout duration for all requests (10 seconds)
#[cfg(feature = "impersonate")]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Helper function to test a single profile+URL combination
/// Returns Ok((profile, url)) if the request succeeds (status 200 or redirect)
/// Returns Err with error message if the request fails
#[cfg(feature = "impersonate")]
async fn test_profile_url(
    profile: Impersonate,
    url: String,
) -> Result<(Impersonate, String), (Impersonate, String, String)> {
    let client = Client::builder()
        .impersonate(profile)
        .no_proxy()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| {
            (
                profile,
                url.clone(),
                format!("Failed to build client: {:?}", e),
            )
        })?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| (profile, url.clone(), format!("Request failed: {:?}", e)))?;

    let status = response.status();
    if status.is_success() || status.is_redirection() {
        Ok((profile, url))
    } else {
        Err((profile, url, format!("Unexpected status: {}", status)))
    }
}

/// Test all impersonate profiles against all URLs
/// This test executes all requests in parallel for faster execution
#[cfg(feature = "impersonate")]
#[tokio::test]
async fn test_all_impersonate_profiles() {
    let profiles = [
        Impersonate::ChromeV144,
        Impersonate::ChromeV145,
        Impersonate::EdgeV144,
        Impersonate::EdgeV145,
        Impersonate::OperaV126,
        Impersonate::OperaV127,
        Impersonate::SafariV18_5,
        Impersonate::SafariV26,
        Impersonate::FirefoxV140,
        Impersonate::FirefoxV146,
    ];

    let urls = [
        "https://bing.com",
        "https://brave.com",
        "https://grokipedia.com",
        "https://mojeek.com",
        "https://yahoo.com",
        "https://yandex.com",
        "https://www.google.com",
        "https://wikipedia.com",
        "https://httpbin.org/anything",
        "https://tls.browserleaks.com/json",
    ];

    let total_tests = profiles.len() * urls.len();

    println!(
        "\n=== Testing {} profiles x {} URLs = {} combinations (parallel) ===\n",
        profiles.len(),
        urls.len(),
        total_tests
    );

    // Create a JoinSet to run all requests in parallel
    let mut set = JoinSet::new();
    for profile in profiles {
        for url in urls.iter() {
            let url = url.to_string();
            set.spawn(async move { test_profile_url(profile, url).await });
        }
    }

    // Collect results
    let mut passed = 0;
    let mut failed = 0;
    let mut failures: Vec<(Impersonate, String, String)> = Vec::new();

    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok((profile, url))) => {
                passed += 1;
                println!("  ✓ {:?} -> {} -> OK", profile, url);
            }
            Ok(Err((profile, url, error))) => {
                failed += 1;
                println!("  ✗ {:?} -> {} -> FAILED: {}", profile, url, error);
                failures.push((profile, url, error));
            }
            Err(e) => {
                failed += 1;
                println!("  ✗ Task panicked: {:?}", e);
            }
        }
    }

    println!("\n=== Results ===");
    println!(
        "Total: {}, Passed: {}, Failed: {}",
        total_tests, passed, failed
    );

    // Print failure summary if any
    if !failures.is_empty() {
        println!("\n=== Failure Summary ===");
        for (profile, url, error) in &failures {
            println!("  {:?} -> {}: {}", profile, url, error);
        }
    }

    // Assert that at least 50% of requests succeed
    let success_rate = passed as f64 / total_tests as f64;
    println!("Success rate: {:.1}%", success_rate * 100.0);

    assert!(
        success_rate >= 0.5,
        "Success rate {:.1}% is below 50% threshold. Passed: {}, Failed: {}",
        success_rate * 100.0,
        passed,
        failed
    );
}
