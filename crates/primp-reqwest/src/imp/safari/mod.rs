//! Safari browser impersonation settings.
//!
//! This module provides configuration for impersonating various Safari browser versions.
//! Each version has its own TLS fingerprint, ALPN protocols, and default HTTP headers
//! that mimic the real Safari browser behavior.
//!
//! # Usage
//!
//! ```rust
//! use reqwest::{Client, Impersonate};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::builder()
//!         .impersonate(Impersonate::SafariV26)
//!         .build()?;
//!
//!     //let response = client.get("https://example.com").send().await?;
//!     Ok(())
//! }
//! ```

pub use crate::imp::Impersonate;
use crate::ClientBuilder;

/// Configures the client builder for Safari impersonation.
///
/// This function applies all necessary settings to mimic a specific Safari version,
/// including TLS configuration, HTTP/2 settings, and default headers.
///
/// # Arguments
///
/// * `safari` - The Safari version to impersonate
/// * `builder` - The client builder to configure
/// * `os_type` - Optional operating system to mimic. If `None`, defaults to MacOS.
///
/// # Returns
///
/// The configured client builder.
pub fn configure_impersonate(
    safari: Impersonate,
    builder: ClientBuilder,
    os_type: Option<crate::imp::ImpersonateOS>,
) -> ClientBuilder {
    let (final_safari, final_os) = crate::imp::get_random_impersonate_config(Some(safari), os_type);

    let settings = build_safari_settings(final_safari, final_os);

    let mut builder = builder
        .use_rustls_impersonate(settings.browser_emulator, settings.emulation_profile)
        .http2_initial_stream_window_size(settings.http2.initial_stream_window_size)
        .http2_initial_connection_window_size(settings.http2.initial_connection_window_size)
        .http2_max_concurrent_streams(settings.http2.max_concurrent_streams)
        .http2_max_frame_size(settings.http2.max_frame_size)
        .http2_max_header_list_size(settings.http2.max_header_list_size.unwrap_or(262144))
        .http2_header_table_size(settings.http2.header_table_size)
        .http2_enable_push(settings.http2.enable_push)
        .http2_no_rfc7540_priorities(settings.http2.no_rfc7540_priorities)
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

/// Builds browser settings for a specific Safari version and OS.
fn build_safari_settings(
    safari: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion, BrowserEmulatorOS};
    use rustls::crypto::emulation;
    use crate::imp::EmulationProfile;
    use std::sync::OnceLock;

    let user_agent = build_user_agent(safari, os);
    let headers = build_headers(user_agent);

    // Convert ImpersonateOS to BrowserEmulatorOS
    let browser_os = match os {
        crate::imp::ImpersonateOS::MacOS => BrowserEmulatorOS::MacOS,
        crate::imp::ImpersonateOS::IOS => BrowserEmulatorOS::IOS,
        _ => BrowserEmulatorOS::MacOS,
    };

    // Get cached browser emulator for Safari (avoids Vec allocations on each call)
    // Note: We cache the base emulator and set OS type on the clone
    let mut browser_emulator = match safari {
        Impersonate::SafariV26 => {
            static SAFARI_26: OnceLock<BrowserEmulator> = OnceLock::new();
            SAFARI_26
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(26, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::SAFARI.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_26);
                    emulator
                })
                .clone()
        }
        Impersonate::SafariV18_5 => {
            static SAFARI_18_5: OnceLock<BrowserEmulator> = OnceLock::new();
            SAFARI_18_5
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(18, 5, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::SAFARI.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_18_5);
                    emulator
                })
                .clone()
        }
        _ => {
            static SAFARI_DEFAULT: OnceLock<BrowserEmulator> = OnceLock::new();
            SAFARI_DEFAULT
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(26, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::SAFARI.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI);
                    emulator
                })
                .clone()
        }
    };

    // Set OS type on the cloned emulator
    browser_emulator.os_type = Some(browser_os);

    // Use Safari emulation profile (version-specific)
    let emulation_profile = match safari {
        Impersonate::SafariV18_5 => EmulationProfile::safari_v18_5(),
        Impersonate::SafariV26 => EmulationProfile::safari_v26(),
        _ => EmulationProfile::safari(),
    };

    let http2 = build_http2_settings(os);

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

/// Builds a User-Agent string for a Safari version and OS.
fn build_user_agent(safari: Impersonate, os: crate::imp::ImpersonateOS) -> &'static str {
    match safari {
        Impersonate::SafariV26 => match os {
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1",
            _ => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0.1 Safari/605.1.15",
        },
        Impersonate::SafariV18_5 => match os {
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
            _ => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15",
        },
        _ => match os {
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1",
            _ => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0.1 Safari/605.1.15",
        },
    }
}

/// Builds default headers for Safari.
///
/// Safari does NOT support Client Hints, so we don't include sec-ch-ua headers.
fn build_headers(user_agent: &'static str) -> http::HeaderMap {
    use http::header::*;

    // Use HeaderMap with capacity for 8 headers to avoid reallocations
    let mut headers = http::HeaderMap::with_capacity(8);
    headers.insert(USER_AGENT, user_agent.parse().unwrap());
    headers.insert(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".parse().unwrap());
    headers.insert(ACCEPT_LANGUAGE, "en-US,en;q=0.9".parse().unwrap());
    headers.insert("accept-encoding", "gzip, deflate, br".parse().unwrap());
    headers.insert("sec-fetch-dest", "document".parse().unwrap());
    headers.insert("sec-fetch-mode", "navigate".parse().unwrap());
    headers.insert("sec-fetch-site", "none".parse().unwrap());
    headers.insert("priority", "u=0, i".parse().unwrap());

    headers
}

/// Builds HTTP/2 settings for Safari.
#[cfg(feature = "http2")]
fn build_http2_settings(_os: crate::imp::ImpersonateOS) -> crate::imp::Http2Data {
    use crate::imp::{SettingsOrder, SettingId, PseudoOrder, PseudoId};

    // Safari 26 HTTP/2 settings (version-specific, same for macOS and iOS)
    // Settings order: 2, 3, 4, 9
    let settings_order = Some(
        SettingsOrder::builder()
            .push(SettingId::EnablePush)
            .push(SettingId::MaxConcurrentStreams)
            .push(SettingId::InitialWindowSize)
            .push(SettingId::NoRfc7540Priorities)
            .build_without_extend(),
    );

    // Safari 26 pseudo order: Method, Scheme, Authority, Path
    let headers_pseudo_order = Some(
        PseudoOrder::builder()
            .push(PseudoId::Method)
            .push(PseudoId::Scheme)
            .push(PseudoId::Authority)
            .push(PseudoId::Path)
            .build(),
    );

    crate::imp::Http2Data {
        // Safari sends: 2:0;3:100;4:2097152;9:1
        // WINDOW_UPDATE delta = 10420225, so initial_connection_window_size = 10420225 + 65535 = 10485760
        initial_stream_window_size: Some(2097152),
        initial_connection_window_size: Some(10485760),
        max_concurrent_streams: Some(100),
        max_frame_size: None,
        max_header_list_size: None,
        header_table_size: None,
        enable_push: Some(false),
        enable_connect_protocol: None,
        no_rfc7540_priorities: Some(true),
        settings_order,
        headers_pseudo_order,
        headers_stream_dependency: None,
        priorities: None,
    }
}

/// Builds HTTP/2 settings for Safari (without http2 feature).
#[cfg(not(feature = "http2"))]
fn build_http2_settings(_os: crate::imp::ImpersonateOS) -> crate::imp::Http2Data {
    crate::imp::Http2Data {
        initial_stream_window_size: Some(10485760),
        initial_connection_window_size: Some(0),
        max_concurrent_streams: Some(100),
        max_frame_size: None,
        max_header_list_size: None,
        header_table_size: Some(2097152),
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
    use crate::imp::{Impersonate, ImpersonateOS};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct BrowserLeaksResponse {
        pub user_agent: String,
        pub ja4: String,
        pub akamai_hash: String,
        pub akamai_text: String,
    }

    // Safari 18.5 test constants
    const SAFARI185_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15";
    const SAFARI185_JA4: &str = "t13d2014h2_a09f3c656075_e42f34c56612";
    const SAFARI185_AKAMAI_HASH: &str = "c52879e43202aeb92740be6e8c86ea96";
    const SAFARI185_AKAMAI_TEXT: &str = "2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p";
    const SAFARI185_IOS_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";
    const SAFARI185_IOS_JA4: &str = "t13d2014h2_a09f3c656075_e42f34c56612";
    const SAFARI185_IOS_AKAMAI_HASH: &str = "c52879e43202aeb92740be6e8c86ea96";
    const SAFARI185_IOS_AKAMAI_TEXT: &str = "2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p";

    const SAFARI26_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
    const SAFARI26_JA4: &str = "t13d2013h2_a09f3c656075_7f0f34a4126d";
    const SAFARI26_AKAMAI_HASH: &str = "c52879e43202aeb92740be6e8c86ea96";
    const SAFARI26_AKAMAI_TEXT: &str = "2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p";
    const SAFARI26_IOS_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1";
    const SAFARI26_IOS_JA4: &str = "t13d2013h2_a09f3c656075_7f0f34a4126d";
    const SAFARI26_IOS_AKAMAI_HASH: &str = "c52879e43202aeb92740be6e8c86ea96";
    const SAFARI26_IOS_AKAMAI_TEXT: &str = "2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p";


    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_safari185() {
        let client = Client::builder()
            .impersonate_os(ImpersonateOS::MacOS)
            .impersonate(Impersonate::SafariV18_5)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, SAFARI185_USER_AGENT);
        assert_eq!(json.ja4, SAFARI185_JA4);
        assert_eq!(json.akamai_hash, SAFARI185_AKAMAI_HASH);
        assert_eq!(json.akamai_text, SAFARI185_AKAMAI_TEXT);
    }

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_safari185_ios() {
        let client = Client::builder()
            .impersonate_os(ImpersonateOS::IOS)
            .impersonate(Impersonate::SafariV18_5)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, SAFARI185_IOS_USER_AGENT);
        assert_eq!(json.ja4, SAFARI185_IOS_JA4);
        assert_eq!(json.akamai_hash, SAFARI185_IOS_AKAMAI_HASH);
        assert_eq!(json.akamai_text, SAFARI185_IOS_AKAMAI_TEXT);
    }

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_safari26() {
        let client = Client::builder()
            .impersonate_os(ImpersonateOS::MacOS)
            .impersonate(Impersonate::SafariV26)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, SAFARI26_USER_AGENT);
        assert_eq!(json.ja4, SAFARI26_JA4);
        assert_eq!(json.akamai_hash, SAFARI26_AKAMAI_HASH);
        assert_eq!(json.akamai_text, SAFARI26_AKAMAI_TEXT);
    }

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_safari26_ios() {
        let client = Client::builder()
            .impersonate_os(ImpersonateOS::IOS)
            .impersonate(Impersonate::SafariV26)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, SAFARI26_IOS_USER_AGENT);
        assert_eq!(json.ja4, SAFARI26_IOS_JA4);
        assert_eq!(json.akamai_hash, SAFARI26_IOS_AKAMAI_HASH);
        assert_eq!(json.akamai_text, SAFARI26_IOS_AKAMAI_TEXT);
    }

    

}
