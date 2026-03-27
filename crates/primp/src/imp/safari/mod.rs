//! Safari browser impersonation settings.
//!
//! This module provides configuration for impersonating various Safari browser versions.
//! Each version has its own TLS fingerprint, ALPN protocols, and default HTTP headers
//! that mimic the real Safari browser behavior.
//!
//! # Usage
//!
//! ```rust
//! use primp::{Client, Impersonate};
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

/// Builds browser settings for a specific Safari version and OS.
pub(crate) fn build_safari_settings(
    safari: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    use rustls::client::{BrowserEmulator, BrowserEmulatorOS, BrowserType, BrowserVersion};
    use rustls::crypto::emulation;
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
        Impersonate::SafariV18_5 => {
            static SAFARI_18_5: OnceLock<BrowserEmulator> = OnceLock::new();
            SAFARI_18_5
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(18, 5, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::SAFARI.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_18_5);
                    emulator.include_status_request_v2 = true;
                    emulator
                })
                .clone()
        }
        Impersonate::SafariV26 => {
            static SAFARI_26: OnceLock<BrowserEmulator> = OnceLock::new();
            SAFARI_26
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(26, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::SAFARI.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_26);
                    emulator
                })
                .clone()
        }
        Impersonate::SafariV26_3 => {
            if browser_os == BrowserEmulatorOS::IOS {
                static SAFARI_26_3_IOS: OnceLock<BrowserEmulator> = OnceLock::new();
                SAFARI_26_3_IOS
                    .get_or_init(|| {
                        let mut emulator = BrowserEmulator::new(
                            BrowserType::Safari,
                            BrowserVersion::new(26, 3, 0),
                        );
                        emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
                        emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
                        emulator.signature_algorithms =
                            Some(emulation::signature_algorithms::SAFARI.to_vec());
                        emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_26);
                        emulator
                    })
                    .clone()
            } else {
                static SAFARI_26_3_MACOS: OnceLock<BrowserEmulator> = OnceLock::new();
                SAFARI_26_3_MACOS
                    .get_or_init(|| {
                        let mut emulator = BrowserEmulator::new(
                            BrowserType::Safari,
                            BrowserVersion::new(26, 3, 0),
                        );
                        emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
                        emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
                        emulator.signature_algorithms =
                            Some(emulation::signature_algorithms::SAFARI.to_vec());
                        emulator.extension_order_seed =
                            Some(emulation::extension_order::SAFARI_18_5);
                        emulator.include_status_request_v2 = true;
                        emulator
                    })
                    .clone()
            }
        }
        _ => unreachable!(),
    };

    // Set OS type on the cloned emulator
    browser_emulator.os_type = Some(browser_os);

    let http2 = build_http2_settings(os);

    crate::imp::BrowserSettings {
        browser_emulator,
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
        Impersonate::SafariV18_5 => match os {
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
            _ => build_user_agent(safari, crate::imp::random_impersonate_os()),
        },
        Impersonate::SafariV26 => match os {
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1",
            _ => build_user_agent(safari, crate::imp::random_impersonate_os()),
        },
        // Safari 26.3: macOS uses Safari 26 UA, iOS uses Safari 18.5 UA
        Impersonate::SafariV26_3 => match os {
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.3 Safari/605.1.15",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
            _ => build_user_agent(safari, crate::imp::random_impersonate_os()),
        },
        _ => unreachable!(),
    }
}

/// Builds default headers for Safari.
///
/// Safari does NOT support Client Hints, so we don't include sec-ch-ua headers.
fn build_headers(user_agent: &'static str) -> http::HeaderMap {
    use http::header::*;

    let mut headers = http::HeaderMap::with_capacity(8);
    headers.insert(USER_AGENT, http::HeaderValue::from_static(user_agent));
    headers.insert(
        ACCEPT,
        http::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ),
    );
    headers.insert(
        ACCEPT_LANGUAGE,
        http::HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert(
        "accept-encoding",
        http::HeaderValue::from_static("gzip, deflate, br"),
    );
    headers.insert("sec-fetch-dest", http::HeaderValue::from_static("document"));
    headers.insert("sec-fetch-mode", http::HeaderValue::from_static("navigate"));
    headers.insert("sec-fetch-site", http::HeaderValue::from_static("none"));
    headers.insert("priority", http::HeaderValue::from_static("u=0, i"));

    headers
}

/// Builds HTTP/2 settings for Safari.
#[cfg(feature = "http2")]
fn build_http2_settings(_os: crate::imp::ImpersonateOS) -> crate::imp::Http2Data {
    use crate::imp::{PseudoId, PseudoOrder, SettingId, SettingsOrder};

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
        headers_priority: None,
        headers_order: None,
        initial_stream_id: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::imp::{Impersonate, ImpersonateOS};
    use crate::Client;
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

    const SAFARI26_3_MACOS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.3 Safari/605.1.15";
    const SAFARI26_3_MACOS_JA4: &str = "t13d2014h2_a09f3c656075_e42f34c56612";
    const SAFARI26_3_MACOS_AKAMAI_HASH: &str = "c52879e43202aeb92740be6e8c86ea96";
    const SAFARI26_3_MACOS_AKAMAI_TEXT: &str = "2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_safari26_3_macos() {
        let client = Client::builder()
            .impersonate_os(ImpersonateOS::MacOS)
            .impersonate(Impersonate::SafariV26_3)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, SAFARI26_3_MACOS_USER_AGENT);
        assert_eq!(json.ja4, SAFARI26_3_MACOS_JA4);
        assert_eq!(json.akamai_hash, SAFARI26_3_MACOS_AKAMAI_HASH);
        assert_eq!(json.akamai_text, SAFARI26_3_MACOS_AKAMAI_TEXT);
    }

    const SAFARI26_3_IOS_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";
    const SAFARI26_3_IOS_JA4: &str = "t13d2013h2_a09f3c656075_7f0f34a4126d";
    const SAFARI26_3_IOS_AKAMAI_HASH: &str = "c52879e43202aeb92740be6e8c86ea96";
    const SAFARI26_3_IOS_AKAMAI_TEXT: &str = "2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_safari26_3_ios() {
        let client = Client::builder()
            .impersonate_os(ImpersonateOS::IOS)
            .impersonate(Impersonate::SafariV26_3)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, SAFARI26_3_IOS_USER_AGENT);
        assert_eq!(json.ja4, SAFARI26_3_IOS_JA4);
        assert_eq!(json.akamai_hash, SAFARI26_3_IOS_AKAMAI_HASH);
        assert_eq!(json.akamai_text, SAFARI26_3_IOS_AKAMAI_TEXT);
    }
}
