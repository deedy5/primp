//! Edge browser impersonation settings.
//!
//! This module provides configuration for impersonating various Edge browser versions.
//! Edge uses Chrome-based TLS/HTTP2 configs but with Edge-specific headers.
//! Each version has its own TLS fingerprint, ALPN protocols, and default HTTP headers
//! that mimic the real Edge browser behavior.
//!
//! # Usage
//!
//! ```rust
//! use reqwest::{Client, Impersonate};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::builder()
//!         .impersonate(Impersonate::EdgeV144)
//!         .build()?;
//!
//!     //let response = client.get("https://example.com").send().await?;
//!     Ok(())
//! }
//! ```

pub use crate::imp::Impersonate;
use crate::ClientBuilder;

/// Configures the client builder for Edge impersonation.
///
/// This function applies all necessary settings to mimic a specific Edge version,
/// including TLS configuration, HTTP/2 settings, and default headers.
///
/// # Arguments
///
/// * `edge` - The Edge version to impersonate
/// * `builder` - The client builder to configure
/// * `os_type` - Optional operating system to mimic. If `None`, defaults to Windows.
///
/// # Returns
///
/// The configured client builder.
pub fn configure_impersonate(
    edge: Impersonate,
    builder: ClientBuilder,
    os_type: Option<crate::imp::ImpersonateOS>,
) -> ClientBuilder {
    let (final_edge, final_os) = crate::imp::get_random_impersonate_config(Some(edge), os_type);

    let settings = build_edge_settings(final_edge, final_os);

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

/// Builds browser settings for a specific Edge version and OS.
fn build_edge_settings(
    edge: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    use http::header::*;
    use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
    use rustls::crypto::emulation;
    use crate::imp::EmulationProfile;
    use std::sync::OnceLock;

    let user_agent = build_user_agent(edge, os);
    let sec_ch_ua = build_sec_ch_ua(edge, os);

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

    // Get cached browser emulator for Edge (avoids Vec allocations on each call)
    let browser_emulator = match edge {
        Impersonate::EdgeV144 => {
            static EDGE_144: OnceLock<BrowserEmulator> = OnceLock::new();
            EDGE_144
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Edge, BrowserVersion::new(144, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::EDGE.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::EDGE.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::EDGE.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::EDGE);
                    emulator
                })
                .clone()
        }
        Impersonate::EdgeV145 => {
            static EDGE_145: OnceLock<BrowserEmulator> = OnceLock::new();
            EDGE_145
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Edge, BrowserVersion::new(145, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::EDGE.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::EDGE.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::EDGE.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::EDGE);
                    emulator
                })
                .clone()
        }
        _ => {
            static EDGE_DEFAULT: OnceLock<BrowserEmulator> = OnceLock::new();
            EDGE_DEFAULT
                .get_or_init(|| {
                    let mut emulator = BrowserEmulator::new(BrowserType::Edge, BrowserVersion::new(144, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::EDGE.to_vec());
                    emulator.signature_algorithms = Some(emulation::signature_algorithms::EDGE.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::EDGE.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::EDGE);
                    emulator
                })
                .clone()
        }
    };

    // Use Edge emulation profile (same as Chrome)
    let emulation_profile = EmulationProfile::edge();

    let http2 = build_http2_settings(edge);

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

/// Builds a User-Agent string for an Edge version and OS.
fn build_user_agent(edge: Impersonate, os: crate::imp::ImpersonateOS) -> &'static str {
    match edge {
        Impersonate::EdgeV144 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 Edg/144.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 Edg/144.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 Edg/144.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; Pixel 3 XL) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Mobile Safari/537.36 EdgA/144.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 EdgiOS/144.0.0.0 Mobile/15E148 Safari/605.1.15",
        },
        Impersonate::EdgeV145 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; Pixel 3 XL) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36 EdgA/145.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 EdgiOS/145.0.0.0 Mobile/15E148 Safari/605.1.15",
        },
        _ => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 Edg/144.0.0.0",
    }
}

/// Builds a sec-ch-ua header value for an Edge version and OS.
fn build_sec_ch_ua(edge: Impersonate, _os: crate::imp::ImpersonateOS) -> &'static str {
    match edge {
        Impersonate::EdgeV144 => r#""Microsoft Edge";v="144", "Chromium";v="144", "Not/A)Brand";v="24""#,
        Impersonate::EdgeV145 => r#""Microsoft Edge";v="145", "Chromium";v="145", "Not/A)Brand";v="24""#,
        _ => r#""Microsoft Edge";v="144", "Chromium";v="144", "Not/A)Brand";v="24""#,
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

/// Builds HTTP/2 settings for an Edge version.
#[cfg(feature = "http2")]
fn build_http2_settings(_edge: Impersonate) -> crate::imp::Http2Data {
    use crate::imp::{SettingsOrder, SettingId, PseudoOrder, PseudoId, StreamDependency};

    // Edge 144 sends only settings: 1, 2, 4, 6 (same as Chrome)
    // Expected fingerprint: 1:65536;2:0;4:6291456;6:262144
    let settings_order = Some(
        SettingsOrder::builder()
            .push(SettingId::HeaderTableSize)      // 1: 65536
            .push(SettingId::EnablePush)           // 2: 0
            .push(SettingId::InitialWindowSize)    // 4: 6291456
            .push(SettingId::MaxHeaderListSize)    // 6: 262144
            .build_without_extend()
    );

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
        enable_connect_protocol: None,
        no_rfc7540_priorities: None,
        settings_order,
        headers_pseudo_order,
        headers_stream_dependency,
        priorities: None,
    }
}

/// Builds HTTP/2 settings for an Edge version (without http2 feature).
#[cfg(not(feature = "http2"))]
fn build_http2_settings(_edge: Impersonate) -> crate::imp::Http2Data {
    crate::imp::Http2Data {
        initial_stream_window_size: Some(6291456),
        initial_connection_window_size: Some(15728640),
        max_concurrent_streams: None,
        max_frame_size: None,
        max_header_list_size: Some(262144),
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

    const EDGE144_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 Edg/144.0.0.0";
    const EDGE144_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const EDGE144_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";
    const EDGE144_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_edge144() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::EdgeV144)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, EDGE144_USER_AGENT);
        assert_eq!(json.ja4, EDGE144_JA4);
        assert_eq!(json.akamai_hash, EDGE144_AKAMAI_HASH);
        assert_eq!(json.akamai_text, EDGE144_AKAMAI_TEXT);
    }
}
