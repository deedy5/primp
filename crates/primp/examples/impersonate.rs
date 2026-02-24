#![deny(warnings)]

//! Browser Impersonation Example
//!
//! This example demonstrates how to use primp's browser impersonation feature
//! to make requests that appear to come from real browsers like Chrome, Firefox,
//! Safari, Edge, or Opera.
//!
//! Browser impersonation helps bypass bot detection by mimicking the TLS
//! fingerprint, HTTP/2 settings, and other characteristics of real browsers.
//!
//! Run with: cargo run --example impersonate

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), primp::Error> {
    // Get URL from CLI or use default
    let url = if let Some(url) = std::env::args().nth(1) {
        url
    } else {
        println!("No CLI URL provided, using default.");
        "https://tls.peet.ws/api/all".into()
    };

    println!("=== Browser Impersonation Examples ===\n");

    // Example 1: Chrome on Windows
    // This mimics Chrome 145 running on Windows
    println!("1. Chrome V145 on Windows:");
    let client = primp::Client::builder()
        .impersonate(primp::Impersonate::ChromeV145)
        .impersonate_os(primp::ImpersonateOS::Windows)
        .build()?;

    make_request(&client, &url).await?;

    // Example 2: Safari on macOS
    // This mimics Safari 18.5 running on macOS
    println!("\n2. Safari V18.5 on macOS:");
    let client = primp::Client::builder()
        .impersonate(primp::Impersonate::SafariV18_5)
        .impersonate_os(primp::ImpersonateOS::MacOS)
        .build()?;

    make_request(&client, &url).await?;

    // Example 3: Firefox on Linux
    // This mimics Firefox 146 running on Linux
    println!("\n3. Firefox V146 on Linux:");
    let client = primp::Client::builder()
        .impersonate(primp::Impersonate::FirefoxV146)
        .impersonate_os(primp::ImpersonateOS::Linux)
        .build()?;

    make_request(&client, &url).await?;

    // Example 4: Edge on Windows
    // This mimics Edge 145 running on Windows
    println!("\n4. Edge V145 on Windows:");
    let client = primp::Client::builder()
        .impersonate(primp::Impersonate::EdgeV145)
        .impersonate_os(primp::ImpersonateOS::Windows)
        .build()?;

    make_request(&client, &url).await?;

    // Example 5: Opera on Windows
    // This mimics Opera 127 running on Windows
    println!("\n5. Opera V127 on Windows:");
    let client = primp::Client::builder()
        .impersonate(primp::Impersonate::OperaV127)
        .impersonate_os(primp::ImpersonateOS::Windows)
        .build()?;

    make_request(&client, &url).await?;

    // Example 6: Random browser impersonation
    // This randomly selects a browser version for impersonation
    // Useful for avoiding detection based on repeated patterns
    println!("\n6. Random browser impersonation:");
    let client = primp::Client::builder().impersonate_random().build()?;

    make_request(&client, &url).await?;

    // Example 7: Chrome on Android (mobile)
    // This mimics Chrome running on an Android device
    println!("\n7. Chrome V145 on Android:");
    let client = primp::Client::builder()
        .impersonate(primp::Impersonate::ChromeV145)
        .impersonate_os(primp::ImpersonateOS::Android)
        .build()?;

    make_request(&client, &url).await?;

    // Example 8: Safari on iOS (mobile)
    // This mimics Safari running on an iOS device
    println!("\n8. Safari V18.5 on iOS:");
    let client = primp::Client::builder()
        .impersonate(primp::Impersonate::SafariV18_5)
        .impersonate_os(primp::ImpersonateOS::IOS)
        .build()?;

    make_request(&client, &url).await?;

    println!("\n=== All impersonation examples completed ===");

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn make_request(client: &primp::Client, url: &str) -> Result<(), primp::Error> {
    eprintln!("  Fetching {url:?}...");

    let res = client.get(url).send().await?;

    eprintln!("  Response: {:?} {}", res.version(), res.status());

    // For the TLS fingerprint test URL, show a truncated response
    let body = res.text().await?;
    let preview = if body.len() > 500 {
        format!(
            "{}...\n  (truncated, total {} bytes)",
            &body[..500],
            body.len()
        )
    } else {
        body
    };
    println!("  Body: {}", preview.replace('\n', "\n  "));

    Ok(())
}

// The [cfg(not(target_arch = "wasm32"))] above prevents building the tokio::main function
// for wasm32 target, because tokio isn't compatible with wasm32.
// If you aren't building for wasm32, you don't need that line.
// The two lines below avoid the "'main' function not found" error when building for wasm32 target.
#[cfg(target_arch = "wasm32")]
fn main() {}
