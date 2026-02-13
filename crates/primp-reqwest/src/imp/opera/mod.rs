//! Opera browser impersonation settings.
//!
//! This module provides configuration for impersonating various Opera browser versions.
//! Opera uses Chrome-based TLS/HTTP2 configs but with Opera-specific headers.
//! Each version has its own TLS fingerprint, ALPN protocols, and default HTTP headers
//! that mimic the real Opera browser behavior.
//!
//! # Usage
//!
//! ```rust
//! use reqwest::{Client, Impersonate};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::builder()
//!         .impersonate(Impersonate::OperaV126)
//!         .build()?;
//!
//!     //let response = client.get("https://example.com").send().await?;
//!     Ok(())
//! }
//! ```

pub use crate::imp::Impersonate;
use crate::ClientBuilder;

/// Configures the client builder for Opera impersonation.
///
/// This function applies all necessary settings to mimic a specific Opera version,
/// including TLS configuration, HTTP/2 settings, and default headers.
///
/// # Arguments
///
/// * `opera` - The Opera version to impersonate
/// * `builder` - The client builder to configure
/// * `os_type` - Optional operating system to mimic. If `None`, defaults to Windows.
///
/// # Returns
///
/// The configured client builder.
pub fn configure_impersonate(
    opera: Impersonate,
    builder: ClientBuilder,
    os_type: Option<crate::imp::ImpersonateOS>,
) -> ClientBuilder {
    let (final_opera, final_os) = crate::imp::get_random_impersonate_config(Some(opera), os_type);

    let settings = build_opera_settings(final_opera, final_os);

    let mut builder = builder
        .use_rustls_impersonate(settings.browser_emulator, settings.emulation_profile)
        .http2_initial_stream_window_size(settings.http2.initial_stream_window_size)
        .http2_initial_connection_window_size(settings.http2.initial_connection_window_size)
        .http2_max_concurrent_streams(settings.http2.max_concurrent_streams)
        .http2_max_frame_size(settings.http2.max_frame_size)
        .http2_max_header_list_size(settings.http2.max_header_list_size.unwrap_or(262144))
        .http2_header_table_size(settings.http2.header_table_size)
        .http2_enable_push(settings.http2.enable_push)
        .replace_default_headers(settings.headers);

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

/// Builds browser settings for a specific Opera version and OS.
fn build_opera_settings(
    opera: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    use http::header::*;
    use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
    use rustls::crypto::emulation;
    use crate::imp::EmulationProfile;
    use std::sync::OnceLock;

    let user_agent = build_user_agent(opera, os);
    let sec_ch_ua = build_sec_ch_ua(opera, os);

    // Use HeaderMap with capacity for 11 headers to avoid reallocations
    let mut headers = http::HeaderMap::with_capacity(11);
    headers.insert(USER_AGENT, user_agent.parse().unwrap());
    headers.insert("sec-ch-ua", sec_ch_ua.parse().unwrap());
    headers.insert("sec-ch-ua-mobile", "?0".parse().unwrap());
    headers.insert("sec-ch-ua-platform", os_platform(os).parse().unwrap());
    headers.insert("sec-fetch-dest", "document".parse().unwrap());
    headers.insert("sec-fetch-mode", "navigate".parse().unwrap());
    headers.insert("sec-fetch-site", "none".parse().unwrap());
    headers.insert("sec-fetch-user", "?1".parse().unwrap());
    headers.insert(ACCEPT, "*/*".parse().unwrap());
    headers.insert("accept-encoding", "gzip, deflate, br, zstd".parse().unwrap());
    headers.insert("priority", "u=0, i".parse().unwrap());

    // Get cached browser emulator for Opera (avoids Vec allocations on each call)
    // Opera is Chrome-based, so use Chrome fingerprints for all versions
    let browser_emulator = match opera {
        Impersonate::OperaV126 => {
            static OPERA_126: OnceLock<BrowserEmulator> = OnceLock::new();
            OPERA_126
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(126, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::CHROME.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
                    emulator
                })
                .clone()
        }
        Impersonate::OperaV127 => {
            static OPERA_127: OnceLock<BrowserEmulator> = OnceLock::new();
            OPERA_127
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(127, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::CHROME.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
                    emulator
                })
                .clone()
        }
        _ => {
            static OPERA_DEFAULT: OnceLock<BrowserEmulator> = OnceLock::new();
            OPERA_DEFAULT
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(127, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::CHROME.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
                    emulator
                })
                .clone()
        }
    };

    // Use Chrome emulation profile for Opera (Opera is Chrome-based)
    let emulation_profile = EmulationProfile::chrome();

    let http2 = build_http2_settings(opera);

    crate::imp::BrowserSettings {
        browser_emulator,
        emulation_profile,
        http2,
        headers,
        gzip: true,
        brotli: true,
        zstd: true,
    }
}

/// Builds a User-Agent string for an Opera version and OS.
fn build_user_agent(opera: Impersonate, os: crate::imp::ImpersonateOS) -> &'static str {
    match opera {
        // Opera 126 is based on Chrome 142, but uses Chrome 144's fingerprints
        Impersonate::OperaV126 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 OPR/126.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 OPR/126.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 OPR/126.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Mobile Safari/537.36 OPR/76.2.4027.73374",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/126.0.0.0 Mobile/15E148 Safari/605.1.15",
        },
        // Opera 127 is based on Chrome 143
        Impersonate::OperaV127 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Mobile Safari/537.36 OPR/76.2.4027.73374",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/127.0.0.0 Mobile/15E148 Safari/605.1.15",
        },
        _ => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
    }
}

/// Builds a sec-ch-ua header value for an Opera version and OS.
fn build_sec_ch_ua(opera: Impersonate, _os: crate::imp::ImpersonateOS) -> &'static str {
    match opera {
        Impersonate::OperaV126 => r#""Opera";v="126", "Chromium";v="142", "Not/A)Brand";v="24""#,
        Impersonate::OperaV127 => r#""Opera";v="127", "Chromium";v="143", "Not/A)Brand";v="24""#,
        _ => r#""Opera";v="127", "Chromium";v="143", "Not/A)Brand";v="24""#,
    }
}

/// Returns a platform string for sec-ch-ua-platform header.
fn os_platform(os: crate::imp::ImpersonateOS) -> &'static str {
    match os {
        crate::imp::ImpersonateOS::Windows => r#""Windows""#,
        crate::imp::ImpersonateOS::MacOS => r#""macOS""#,
        crate::imp::ImpersonateOS::Linux => r#""Linux""#,
        crate::imp::ImpersonateOS::Android => r#""Android""#,
        crate::imp::ImpersonateOS::IOS => r#""iOS""#,
    }
}

/// Builds HTTP/2 settings for an Opera version.
#[cfg(feature = "http2")]
fn build_http2_settings(opera: Impersonate) -> crate::imp::Http2Data {
    use crate::imp::{SettingsOrder, SettingId, PseudoOrder, PseudoId, StreamDependency};

    // Opera 126 uses Chrome 144's HTTP/2 settings: 1, 2, 4, 6
    // Older Opera versions use: 1, 2, 4, 6, 8, 9
    let settings_order = match opera {
        Impersonate::OperaV126 | Impersonate::OperaV127 => Some(
            SettingsOrder::builder()
                .push(SettingId::HeaderTableSize)      // 1: 65536
                .push(SettingId::EnablePush)           // 2: 0
                .push(SettingId::InitialWindowSize)    // 4: 6291456
                .push(SettingId::MaxHeaderListSize)    // 6: 262144
                .build_without_extend()
        ),
        _ => Some(
            SettingsOrder::builder()
                .push(SettingId::HeaderTableSize)      // 1: 65536
                .push(SettingId::EnablePush)           // 2: 0
                .push(SettingId::InitialWindowSize)    // 4: 6291456
                .push(SettingId::MaxHeaderListSize)    // 6: 262144
                .push(SettingId::EnableConnectProtocol)   // 8: 1
                .push(SettingId::NoRfc7540Priorities)     // 9: 1
                .build_without_extend()
        ),
    };

    let headers_pseudo_order = Some(
        PseudoOrder::builder()
            .push(PseudoId::Method)
            .push(PseudoId::Authority)
            .push(PseudoId::Scheme)
            .push(PseudoId::Path)
            .build()
    );

    let headers_stream_dependency = Some(StreamDependency::chrome());

    crate::imp::Http2Data {
        initial_stream_window_size: Some(6291456),
        initial_connection_window_size: Some(15728640),
        max_concurrent_streams: None,
        max_frame_size: None,
        max_header_list_size: Some(262144),
        header_table_size: Some(65536),
        enable_push: Some(false),
        enable_connect_protocol: match opera {
            Impersonate::OperaV126 | Impersonate::OperaV127 => None,
            _ => Some(true),
        },
        no_rfc7540_priorities: match opera {
            Impersonate::OperaV126 | Impersonate::OperaV127 => None,
            _ => Some(true),
        },
        settings_order,
        headers_pseudo_order,
        headers_stream_dependency,
        priorities: None,
    }
}

/// Builds HTTP/2 settings for an Opera version (without http2 feature).
#[cfg(not(feature = "http2"))]
fn build_http2_settings(opera: Impersonate) -> crate::imp::Http2Data {
    crate::imp::Http2Data {
        initial_stream_window_size: Some(6291456),
        initial_connection_window_size: Some(15728640),
        max_concurrent_streams: None,
        max_frame_size: None,
        max_header_list_size: Some(262144),
        header_table_size: Some(65536),
        enable_push: Some(false),
        enable_connect_protocol: match opera {
            Impersonate::OperaV126 | Impersonate::OperaV127 => None,
            _ => Some(true),
        },
        no_rfc7540_priorities: match opera {
            Impersonate::OperaV126 | Impersonate::OperaV127 => None,
            _ => Some(true),
        },
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

    const OPERA126_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 OPR/126.0.0.0";
    const OPERA126_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const OPERA126_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";
    const OPERA126_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_opera126() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::OperaV126)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, OPERA126_USER_AGENT);
        assert_eq!(json.ja4, OPERA126_JA4);
        assert_eq!(json.akamai_hash, OPERA126_AKAMAI_HASH);
        assert_eq!(json.akamai_text, OPERA126_AKAMAI_TEXT);
    }

    const OPERA127_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0";
    const OPERA127_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const OPERA127_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";
    const OPERA127_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_opera127() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::OperaV127)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, OPERA127_USER_AGENT);
        assert_eq!(json.ja4, OPERA127_JA4);
        assert_eq!(json.akamai_hash, OPERA127_AKAMAI_HASH);
        assert_eq!(json.akamai_text, OPERA127_AKAMAI_TEXT);
    }
}
