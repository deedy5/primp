use primp::{Client, Impersonate, ImpersonateOS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://tls.browserleaks.com/json";

    // 1. Specific browser version + specific OS
    let client = Client::builder()
        .impersonate_os(ImpersonateOS::Windows)
        .impersonate(Impersonate::ChromeV146)
        .build()?;
    println!("=== Chrome 146 on Windows ===");
    println!("{}\n", client.get(url).send().await?.text().await?);

    // 2. Specific Firefox on macOS
    let client = Client::builder()
        .impersonate_os(ImpersonateOS::MacOS)
        .impersonate(Impersonate::FirefoxV148)
        .build()?;
    println!("=== Firefox 148 on macOS ===");
    println!("{}\n", client.get(url).send().await?.text().await?);

    // 3. Specific Safari on iOS
    let client = Client::builder()
        .impersonate_os(ImpersonateOS::IOS)
        .impersonate(Impersonate::SafariV26)
        .build()?;
    println!("=== Safari 26 on iOS ===");
    println!("{}\n", client.get(url).send().await?.text().await?);

    // 4. Random version from a specific browser family
    let client = Client::builder().impersonate(Impersonate::Opera).build()?;
    println!("=== Random Opera version (random OS) ===");
    println!("{}\n", client.get(url).send().await?.text().await?);

    // 5. Random version from Edge family on Android
    let client = Client::builder()
        .impersonate_os(ImpersonateOS::Android)
        .impersonate(Impersonate::Edge)
        .build()?;
    println!("=== Random Edge on Android ===");
    println!("{}\n", client.get(url).send().await?.text().await?);

    // 6. Completely random browser and version (OS also random)
    let client = Client::builder()
        .impersonate(Impersonate::Random)
        .impersonate_os(ImpersonateOS::Random)
        .build()?;
    println!("=== Random browser + Random OS ===");
    println!("{}\n", client.get(url).send().await?.text().await?);

    Ok(())
}
