//! Firefox browser impersonation settings.
//!
//! This module provides configuration for impersonating various Firefox browser versions.
//! Each version has its own TLS fingerprint, ALPN protocols, and default HTTP headers
//! that mimic the real Firefox browser behavior.
//!
//! # Usage
//!
//! ```rust
//! use reqwest::{Client, Impersonate};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::builder()
//!         .impersonate(Impersonate::FirefoxV140)
//!         .build()?;
//!
//!     //let response = client.get("https://example.com").send().await?;
//!     Ok(())
//! }
//! ```

pub use crate::imp::Impersonate;
use crate::ClientBuilder;

/// Configures the client builder for Firefox impersonation.
///
/// This function applies all necessary settings to mimic a specific Firefox version,
/// including TLS configuration, HTTP/2 settings, and default headers.
///
/// # Arguments
///
/// * `firefox` - The Firefox version to impersonate
/// * `builder` - The client builder to configure
/// * `os_type` - Optional operating system to mimic. If `None`, defaults to Windows.
///
/// # Returns
///
/// The configured client builder.
pub fn configure_impersonate(
    firefox: Impersonate,
    builder: ClientBuilder,
    os_type: Option<crate::imp::ImpersonateOS>,
) -> ClientBuilder {
    let (final_firefox, final_os) = crate::imp::get_random_impersonate_config(Some(firefox), os_type);

    let settings = build_firefox_settings(final_firefox, final_os);

    let mut builder = builder
        .use_rustls_impersonate(settings.browser_emulator, settings.emulation_profile)
        .http2_initial_stream_window_size(settings.http2.initial_stream_window_size)
        .http2_initial_connection_window_size(settings.http2.initial_connection_window_size)
        .http2_max_concurrent_streams(settings.http2.max_concurrent_streams)
        .http2_max_frame_size(settings.http2.max_frame_size)
        .http2_header_table_size(settings.http2.header_table_size)
        .http2_enable_push(settings.http2.enable_push)
        .replace_default_headers(settings.headers);
    
    // Only set max_header_list_size if explicitly specified
    if let Some(max_header_list_size) = settings.http2.max_header_list_size {
        builder = builder.http2_max_header_list_size(max_header_list_size);
    }

    // Disable compression methods if not supported by the browser
    if !settings.gzip {
        builder = builder.no_gzip();
    }
    if !settings.brotli {
        builder = builder.no_brotli();
    }
    if !settings.zstd {
        builder = builder.no_zstd();
    }

    // Apply HTTP/2 fingerprinting settings
    if let Some(settings_order) = settings.http2.settings_order {
        builder = builder.http2_settings_order(settings_order);
    }
    if let Some(headers_pseudo_order) = settings.http2.headers_pseudo_order {
        builder = builder.http2_headers_pseudo_order(headers_pseudo_order);
    }
    if let Some(headers_stream_dependency) = settings.http2.headers_stream_dependency {
        builder = builder.http2_headers_stream_dependency(headers_stream_dependency);
    }
    if let Some(priorities) = settings.http2.priorities {
        builder = builder.http2_priorities(priorities);
    }

    builder
}

/// Builds browser settings for a specific Firefox version and OS.
fn build_firefox_settings(
    firefox: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
    use rustls::crypto::emulation;
    use crate::imp::EmulationProfile;
    use std::sync::OnceLock;

    let user_agent = build_user_agent(firefox, os);
    let headers = build_headers(user_agent);

    // Get cached browser emulator for Firefox (avoids Vec allocations on each call)
    let browser_emulator = match firefox {
        Impersonate::FirefoxV140 => {
            static FIREFOX_140: OnceLock<BrowserEmulator> = OnceLock::new();
            FIREFOX_140
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(140, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::FIREFOX.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::FIREFOX.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::FIREFOX);
                    emulator
                })
                .clone()
        }
        Impersonate::FirefoxV146 => {
            static FIREFOX_146: OnceLock<BrowserEmulator> = OnceLock::new();
            FIREFOX_146
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(146, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::FIREFOX.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::FIREFOX.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::FIREFOX);
                    emulator
                })
                .clone()
        }
        _ => {
            static FIREFOX_DEFAULT: OnceLock<BrowserEmulator> = OnceLock::new();
            FIREFOX_DEFAULT
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(140, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::FIREFOX.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::FIREFOX.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::FIREFOX);
                    emulator
                })
                .clone()
        }
    };

    // Use Firefox emulation profile
    let emulation_profile = EmulationProfile::firefox();

    let http2 = build_http2_settings();

    crate::imp::BrowserSettings {
        browser_emulator,
        emulation_profile,
        http2,
        headers,
        gzip: true,
        brotli: true,
        zstd: false,
    }
}

/// Builds a User-Agent string for a Firefox version and OS.
fn build_user_agent(firefox: Impersonate, os: crate::imp::ImpersonateOS) -> &'static str {
    match firefox {
        Impersonate::FirefoxV140 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:140.0) Gecko/20100101 Firefox/140.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:140.0) Gecko/140.0 Firefox/140.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/140.0 Mobile/15E148 Safari/605.1",
        },
        Impersonate::FirefoxV146 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:146.0) Gecko/146.0 Firefox/146.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/146.0 Mobile/15E148 Safari/605.1",
        },
        _ => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0",
    }
}

/// Builds default headers for Firefox.
fn build_headers(user_agent: &'static str) -> http::HeaderMap {
    use http::header::*;

    // Use HeaderMap with capacity for 10 headers to avoid reallocations
    let mut headers = http::HeaderMap::with_capacity(10);
    headers.insert(USER_AGENT, user_agent.parse().unwrap());
    headers.insert(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".parse().unwrap());
    headers.insert("accept-language", "en-US,en;q=0.5".parse().unwrap());
    headers.insert("accept-encoding", "gzip, deflate, br".parse().unwrap());
    headers.insert("upgrade-insecure-requests", "1".parse().unwrap());
    headers.insert("sec-fetch-dest", "document".parse().unwrap());
    headers.insert("sec-fetch-mode", "navigate".parse().unwrap());
    headers.insert("sec-fetch-site", "none".parse().unwrap());
    headers.insert("sec-fetch-user", "?1".parse().unwrap());
    headers.insert("te", "trailers".parse().unwrap());

    headers
}

/// Builds HTTP/2 settings for Firefox.
/// 
/// Firefox 140/146 HTTP/2 fingerprint:
/// - Settings: 1:65536, 2:0, 4:131072, 5:16384
/// - Window Update: 12517377
/// - Pseudo-header order: m,p,a,s (:method, :path, :authority, :scheme)
#[cfg(feature = "http2")]
fn build_http2_settings() -> crate::imp::Http2Data {
    use crate::imp::{SettingsOrder, SettingId, PseudoOrder, PseudoId};

    // Firefox sends settings: 1, 2, 4, 5
    // Expected fingerprint: 1:65536;2:0;4:131072;5:16384
    let settings_order = Some(
        SettingsOrder::builder()
            .push(SettingId::HeaderTableSize)      // 1: 65536
            .push(SettingId::EnablePush)           // 2: 0
            .push(SettingId::InitialWindowSize)    // 4: 131072
            .push(SettingId::MaxFrameSize)         // 5: 16384
            .build_without_extend()
    );

    // Firefox pseudo-header order: m,p,a,s (:method, :path, :authority, :scheme)
    let headers_pseudo_order = Some(
        PseudoOrder::builder()
            .push(PseudoId::Method)
            .push(PseudoId::Path)
            .push(PseudoId::Authority)
            .push(PseudoId::Scheme)
            .build()
    );

    crate::imp::Http2Data {
        initial_stream_window_size: Some(131072),
        // Note: h2 subtracts the default window size (65535) from this value,
        // so we add 65535 to get the target window update of 12517377
        initial_connection_window_size: Some(12517377 + 65535),
        max_concurrent_streams: None,
        max_frame_size: Some(16384),
        max_header_list_size: None,
        header_table_size: Some(65536),
        enable_push: Some(false),
        enable_connect_protocol: None,
        no_rfc7540_priorities: None,
        settings_order,
        headers_pseudo_order,
        headers_stream_dependency: None,
        priorities: None,
    }
}



/// Builds HTTP/2 settings for Firefox (without http2 feature).
#[cfg(not(feature = "http2"))]
fn build_http2_settings() -> crate::imp::Http2Data {
    crate::imp::Http2Data {
        initial_stream_window_size: Some(131072),
        // Note: h2 subtracts the default window size (65535) from this value,
        // so we add 65535 to get the target window update of 12517377
        initial_connection_window_size: Some(12517377 + 65535),
        max_concurrent_streams: None,
        max_frame_size: Some(16384),
        max_header_list_size: None,
        header_table_size: Some(65536),
        enable_push: Some(false),
        enable_connect_protocol: None,
        no_rfc7540_priorities: None,
        settings_order: None,
        headers_pseudo_order: None,
        headers_stream_dependency: None,
        priorities: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::async_impl::client::Client;
    use crate::imp::Impersonate;
    use serde::{Deserialize, Serialize};

    /// BrowserLeaks.com API response structure
    #[derive(Debug, Serialize, Deserialize)]
    pub struct BrowserLeaksResponse {
        pub user_agent: String,
        pub ja4: String,
        pub akamai_hash: String,
        pub akamai_text: String,
    }

    const FIREFOX140_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
    const FIREFOX140_JA4: &str = "t13d1717h2_5b57614c22b0_3cbfd9057e0d";
    // Firefox 140 sends: 1:65536, 2:0, 4:131072, 5:16384 (no MAX_HEADER_LIST_SIZE)
    const FIREFOX140_AKAMAI_HASH: &str = "6ea73faa8fc5aac76bded7bd238f6433";
    const FIREFOX140_AKAMAI_TEXT: &str = "1:65536;2:0;4:131072;5:16384|12517377|0|m,p,a,s";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_firefox140() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::FirefoxV140)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, FIREFOX140_USER_AGENT);
        assert_eq!(json.ja4, FIREFOX140_JA4);
        assert_eq!(json.akamai_hash, FIREFOX140_AKAMAI_HASH);
        assert_eq!(json.akamai_text, FIREFOX140_AKAMAI_TEXT);
    }

    const FIREFOX146_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:146.0) Gecko/20100101 Firefox/146.0";
    const FIREFOX146_JA4: &str = "t13d1717h2_5b57614c22b0_3cbfd9057e0d";
    // Firefox 146 sends: 1:65536, 2:0, 4:131072, 5:16384 (no MAX_HEADER_LIST_SIZE)
    const FIREFOX146_AKAMAI_HASH: &str = "6ea73faa8fc5aac76bded7bd238f6433";
    const FIREFOX146_AKAMAI_TEXT: &str = "1:65536;2:0;4:131072;5:16384|12517377|0|m,p,a,s";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_firefox146() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::FirefoxV146)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, FIREFOX146_USER_AGENT);
        assert_eq!(json.ja4, FIREFOX146_JA4);
        assert_eq!(json.akamai_hash, FIREFOX146_AKAMAI_HASH);
        assert_eq!(json.akamai_text, FIREFOX146_AKAMAI_TEXT);
    }
}