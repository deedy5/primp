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
#[cfg(feature = "http2")]
use crate::imp::{PseudoId, PseudoOrder, SettingId, SettingsOrder};
use rustls::client::{BrowserEmulator, BrowserEmulatorOS, BrowserType, BrowserVersion};
use rustls::crypto::emulation;
use std::sync::{Arc, OnceLock};

/// Builds browser settings for a specific Safari version and OS.
pub(crate) fn build_safari_settings(
    safari: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    let os = if matches!(os, crate::imp::ImpersonateOS::Random) {
        crate::imp::random_impersonate_os()
    } else {
        os
    };
    let user_agent = build_user_agent(safari, os);
    let headers = build_safari_base_headers(user_agent);

    // Convert ImpersonateOS to BrowserEmulatorOS
    let browser_os = match os {
        crate::imp::ImpersonateOS::MacOS => BrowserEmulatorOS::MacOS,
        crate::imp::ImpersonateOS::IOS => BrowserEmulatorOS::IOS,
        _ => BrowserEmulatorOS::MacOS,
    };

    // Get cached browser emulator for Safari (Arc clone = cheap refcount increment)
    let browser_emulator = safari_emulator(safari, browser_os);

    let http2 = build_http2_settings();

    crate::imp::BrowserSettings {
        browser_emulator,
        http2,
        headers,
        gzip: true,
        brotli: true,
        zstd: false,
        deflate: true,
    }
}

/// Builds a User-Agent string for a Safari version and OS.
/// Safari only supports MacOS and iOS; other OSes default to MacOS.
fn build_user_agent(safari: Impersonate, os: crate::imp::ImpersonateOS) -> &'static str {
    // Random is resolved before this is called; only MacOS and IOS reach here
    let os = match os {
        crate::imp::ImpersonateOS::IOS => crate::imp::ImpersonateOS::IOS,
        _ => crate::imp::ImpersonateOS::MacOS,
    };
    match safari {
        Impersonate::SafariV18_5 => match os {
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
            _ => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15",
        },
        Impersonate::SafariV26 => match os {
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1",
            _ => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
        },
        // Safari 26.3: macOS uses Safari 26 UA, iOS uses Safari 18.5 UA
        Impersonate::SafariV26_3 => match os {
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
            _ => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.3 Safari/605.1.15",
        },
        _ => unreachable!(),
    }
}

/// Builds default headers for Safari using cached base.
fn build_safari_base_headers(user_agent: &'static str) -> http::HeaderMap {
    let mut headers = safari_base_headers().clone();
    headers.insert(
        http::header::USER_AGENT,
        http::HeaderValue::from_static(user_agent),
    );
    headers
}

/// Builds HTTP/2 settings for Safari.
#[cfg(feature = "http2")]
fn build_http2_settings() -> crate::imp::Http2Data {
    crate::imp::Http2Data {
        initial_stream_window_size: Some(crate::imp::SAFARI_INITIAL_STREAM_WINDOW),
        initial_connection_window_size: Some(crate::imp::SAFARI_INITIAL_CONNECTION_WINDOW),
        max_concurrent_streams: Some(100),
        max_header_list_size: Some(crate::imp::SAFARI_MAX_HEADER_LIST_SIZE),
        no_rfc7540_priorities: Some(true),
        settings_order: Some(safari_settings_order().clone()),
        headers_pseudo_order: Some(safari_pseudo_order().clone()),
        ..Default::default()
    }
}

fn safari_emulator(safari: Impersonate, browser_os: BrowserEmulatorOS) -> Arc<BrowserEmulator> {
    match safari {
        Impersonate::SafariV18_5 => match browser_os {
            BrowserEmulatorOS::IOS => {
                static EMU_IOS: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
                EMU_IOS
                    .get_or_init(|| {
                        let mut emu = new_safari_18_5_emulator();
                        emu.os_type = Some(BrowserEmulatorOS::IOS);
                        Arc::new(emu)
                    })
                    .clone()
            }
            _ => {
                static EMU_MACOS: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
                EMU_MACOS
                    .get_or_init(|| {
                        let mut emu = new_safari_18_5_emulator();
                        emu.os_type = Some(BrowserEmulatorOS::MacOS);
                        Arc::new(emu)
                    })
                    .clone()
            }
        },
        Impersonate::SafariV26 => match browser_os {
            BrowserEmulatorOS::IOS => {
                static EMU_IOS: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
                EMU_IOS
                    .get_or_init(|| {
                        let mut emu = new_safari_26_emulator();
                        emu.os_type = Some(BrowserEmulatorOS::IOS);
                        Arc::new(emu)
                    })
                    .clone()
            }
            _ => {
                static EMU_MACOS: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
                EMU_MACOS
                    .get_or_init(|| {
                        let mut emu = new_safari_26_emulator();
                        emu.os_type = Some(BrowserEmulatorOS::MacOS);
                        Arc::new(emu)
                    })
                    .clone()
            }
        },
        Impersonate::SafariV26_3 => {
            if browser_os == BrowserEmulatorOS::IOS {
                static EMU_IOS: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
                EMU_IOS
                    .get_or_init(|| Arc::new(new_safari_26_3_ios_emulator()))
                    .clone()
            } else {
                static EMU_MACOS: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
                EMU_MACOS
                    .get_or_init(|| Arc::new(new_safari_26_3_macos_emulator()))
                    .clone()
            }
        }
        _ => unreachable!(),
    }
}

fn new_safari_18_5_emulator() -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(18, 5, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
    emulator.signature_algorithms = Some(emulation::signature_algorithms::SAFARI.to_vec());
    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_18_5);
    emulator.include_status_request_v2 = true;
    emulator
}

fn new_safari_26_emulator() -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(26, 0, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
    emulator.signature_algorithms = Some(emulation::signature_algorithms::SAFARI.to_vec());
    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_26);
    emulator
}

fn new_safari_26_3_ios_emulator() -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(26, 3, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
    emulator.signature_algorithms = Some(emulation::signature_algorithms::SAFARI.to_vec());
    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_26);
    emulator.os_type = Some(BrowserEmulatorOS::IOS);
    emulator
}

fn new_safari_26_3_macos_emulator() -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Safari, BrowserVersion::new(26, 3, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::SAFARI.to_vec());
    emulator.named_groups = Some(emulation::named_groups::SAFARI.to_vec());
    emulator.signature_algorithms = Some(emulation::signature_algorithms::SAFARI.to_vec());
    emulator.extension_order_seed = Some(emulation::extension_order::SAFARI_18_5);
    emulator.include_status_request_v2 = true;
    emulator.os_type = Some(BrowserEmulatorOS::MacOS);
    emulator
}

fn safari_base_headers() -> &'static http::HeaderMap {
    static BASE: OnceLock<http::HeaderMap> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut headers = http::HeaderMap::with_capacity(8);
        headers.insert(
            http::header::ACCEPT,
            http::HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            http::header::ACCEPT_LANGUAGE,
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
    })
}

#[cfg(feature = "http2")]
fn safari_settings_order() -> &'static SettingsOrder {
    static ORDER: OnceLock<SettingsOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        SettingsOrder::builder()
            .push(SettingId::EnablePush)
            .push(SettingId::MaxConcurrentStreams)
            .push(SettingId::InitialWindowSize)
            .push(SettingId::NoRfc7540Priorities)
            .build_without_extend()
    })
}

#[cfg(feature = "http2")]
fn safari_pseudo_order() -> &'static PseudoOrder {
    static ORDER: OnceLock<PseudoOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        PseudoOrder::builder()
            .push(PseudoId::Method)
            .push(PseudoId::Scheme)
            .push(PseudoId::Authority)
            .push(PseudoId::Path)
            .build()
    })
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
