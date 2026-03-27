//! Firefox browser impersonation settings.
//!
//! This module provides configuration for impersonating various Firefox browser versions.
//! Each version has its own TLS fingerprint, ALPN protocols, and default HTTP headers
//! that mimic the real Firefox browser behavior.
//!
//! # Usage
//!
//! ```rust
//! use primp::{Client, Impersonate};
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

/// Builds browser settings for a specific Firefox version and OS.
pub(crate) fn build_firefox_settings(
    firefox: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
    use rustls::crypto::emulation;
    use std::sync::OnceLock;

    let user_agent = build_user_agent(firefox, os);
    let headers = build_headers(user_agent);

    // Get cached browser emulator for Firefox (avoids Vec allocations on each call)
    let browser_emulator = match firefox {
        Impersonate::FirefoxV140 => {
            static FIREFOX_140: OnceLock<BrowserEmulator> = OnceLock::new();
            FIREFOX_140
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(140, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::FIREFOX.to_vec());
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
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(146, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::FIREFOX.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::FIREFOX.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::FIREFOX);
                    emulator
                })
                .clone()
        }
        Impersonate::FirefoxV147 => {
            static FIREFOX_147: OnceLock<BrowserEmulator> = OnceLock::new();
            FIREFOX_147
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(147, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::FIREFOX.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::FIREFOX.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::FIREFOX);
                    emulator
                })
                .clone()
        }
        Impersonate::FirefoxV148 => {
            static FIREFOX_148: OnceLock<BrowserEmulator> = OnceLock::new();
            FIREFOX_148
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(148, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::FIREFOX.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::FIREFOX.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::FIREFOX);
                    emulator
                })
                .clone()
        }
        _ => unreachable!(),
    };

    let http2 = build_http2_settings();

    crate::imp::BrowserSettings {
        browser_emulator,
        http2,
        headers,
        gzip: true,
        brotli: true,
        zstd: true,
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
            _ => build_user_agent(firefox, crate::imp::random_impersonate_os()),
        },
        Impersonate::FirefoxV146 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:146.0) Gecko/146.0 Firefox/146.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/146.0 Mobile/15E148 Safari/605.1",
            _ => build_user_agent(firefox, crate::imp::random_impersonate_os()),
        },
        Impersonate::FirefoxV147 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:147.0) Gecko/20100101 Firefox/147.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:147.0) Gecko/20100101 Firefox/147.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:147.0) Gecko/147.0 Firefox/147.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/147.0 Mobile/15E148 Safari/605.1",
            _ => build_user_agent(firefox, crate::imp::random_impersonate_os()),
        },
        Impersonate::FirefoxV148 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:148.0) Gecko/20100101 Firefox/148.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:148.0) Gecko/20100101 Firefox/148.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:148.0) Gecko/20100101 Firefox/148.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:148.0) Gecko/148.0 Firefox/148.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/148.0 Mobile/15E148 Safari/605.1",
            _ => build_user_agent(firefox, crate::imp::random_impersonate_os()),
        },
        _ => unreachable!(),
    }
}

/// Builds default headers for Firefox.
fn build_headers(user_agent: &'static str) -> http::HeaderMap {
    use http::header::*;

    let mut headers = http::HeaderMap::with_capacity(13);
    headers.insert(USER_AGENT, http::HeaderValue::from_static(user_agent));
    headers.insert(
        ACCEPT,
        http::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ),
    );
    headers.insert(
        "accept-language",
        http::HeaderValue::from_static("en-US,en;q=0.5"),
    );
    headers.insert(
        "accept-encoding",
        http::HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert("dnt", http::HeaderValue::from_static("1"));
    headers.insert("sec-gpc", http::HeaderValue::from_static("1"));
    headers.insert(
        "upgrade-insecure-requests",
        http::HeaderValue::from_static("1"),
    );
    headers.insert("sec-fetch-dest", http::HeaderValue::from_static("document"));
    headers.insert("sec-fetch-mode", http::HeaderValue::from_static("navigate"));
    headers.insert("sec-fetch-site", http::HeaderValue::from_static("none"));
    headers.insert("sec-fetch-user", http::HeaderValue::from_static("?1"));
    headers.insert("priority", http::HeaderValue::from_static("u=0, i"));
    headers.insert("te", http::HeaderValue::from_static("trailers"));

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
    use crate::imp::{PseudoId, PseudoOrder, SettingId, SettingsOrder};

    // Firefox sends settings: 1, 2, 4, 5
    // Expected fingerprint: 1:65536;2:0;4:131072;5:16384
    let settings_order = Some(
        SettingsOrder::builder()
            .push(SettingId::HeaderTableSize) // 1: 65536
            .push(SettingId::EnablePush) // 2: 0
            .push(SettingId::InitialWindowSize) // 4: 131072
            .push(SettingId::MaxFrameSize) // 5: 16384
            .build_without_extend(),
    );

    // Firefox pseudo-header order: m,p,a,s (:method, :path, :authority, :scheme)
    let headers_pseudo_order = Some(
        PseudoOrder::builder()
            .push(PseudoId::Method)
            .push(PseudoId::Path)
            .push(PseudoId::Authority)
            .push(PseudoId::Scheme)
            .build(),
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
        headers_priority: Some((41, 0, false)), // Firefox: weight=41 (displays as 42 on wire after +1), dep=0, excl=0
        headers_order: Some(vec![
            http::HeaderName::from_static("user-agent"),
            http::HeaderName::from_static("accept"),
            http::HeaderName::from_static("accept-language"),
            http::HeaderName::from_static("accept-encoding"),
            http::HeaderName::from_static("dnt"),
            http::HeaderName::from_static("sec-gpc"),
            http::HeaderName::from_static("upgrade-insecure-requests"),
            http::HeaderName::from_static("sec-fetch-dest"),
            http::HeaderName::from_static("sec-fetch-mode"),
            http::HeaderName::from_static("sec-fetch-site"),
            http::HeaderName::from_static("sec-fetch-user"),
            http::HeaderName::from_static("priority"),
            http::HeaderName::from_static("te"),
        ]),
        initial_stream_id: Some(3), // Firefox starts at stream ID 3
    }
}

#[cfg(test)]
mod tests {
    use crate::imp::Impersonate;
    use crate::Client;
    use serde::{Deserialize, Serialize};

    /// BrowserLeaks.com API response structure
    #[derive(Debug, Serialize, Deserialize)]
    pub struct BrowserLeaksResponse {
        pub user_agent: String,
        pub ja4: String,
        pub akamai_hash: String,
        pub akamai_text: String,
    }

    const FIREFOX140_USER_AGENT: &str =
        "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
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

    const FIREFOX148_USER_AGENT: &str =
        "Mozilla/5.0 (X11; Linux x86_64; rv:148.0) Gecko/20100101 Firefox/148.0";
    const FIREFOX148_JA4: &str = "t13d1717h2_5b57614c22b0_3cbfd9057e0d";
    const FIREFOX148_AKAMAI_HASH: &str = "6ea73faa8fc5aac76bded7bd238f6433";
    const FIREFOX148_AKAMAI_TEXT: &str = "1:65536;2:0;4:131072;5:16384|12517377|0|m,p,a,s";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_firefox148() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::FirefoxV148)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, FIREFOX148_USER_AGENT);
        assert_eq!(json.ja4, FIREFOX148_JA4);
        assert_eq!(json.akamai_hash, FIREFOX148_AKAMAI_HASH);
        assert_eq!(json.akamai_text, FIREFOX148_AKAMAI_TEXT);
    }
}
