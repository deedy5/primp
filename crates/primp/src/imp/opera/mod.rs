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
//! use primp::{Client, Impersonate};
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

/// Builds browser settings for a specific Opera version and OS.
pub(crate) fn build_opera_settings(
    opera: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    use http::header::*;
    use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
    use rustls::crypto::emulation;
    use std::sync::OnceLock;

    let user_agent = build_user_agent(opera, os);
    let sec_ch_ua = build_sec_ch_ua(opera, os);

    let mut headers = http::HeaderMap::with_capacity(13);
    headers.insert(USER_AGENT, http::HeaderValue::from_static(user_agent));
    headers.insert(
        "upgrade-insecure-requests",
        http::HeaderValue::from_static("1"),
    );
    headers.insert("sec-ch-ua", http::HeaderValue::from_static(sec_ch_ua));
    headers.insert(
        "sec-ch-ua-mobile",
        http::HeaderValue::from_static(
            if matches!(
                os,
                crate::imp::ImpersonateOS::Android | crate::imp::ImpersonateOS::IOS
            ) {
                "?1"
            } else {
                "?0"
            },
        ),
    );
    headers.insert(
        "sec-ch-ua-platform",
        http::HeaderValue::from_static(os_platform(os)),
    );
    headers.insert("sec-fetch-dest", http::HeaderValue::from_static("document"));
    headers.insert("sec-fetch-mode", http::HeaderValue::from_static("navigate"));
    headers.insert("sec-fetch-site", http::HeaderValue::from_static("none"));
    headers.insert("sec-fetch-user", http::HeaderValue::from_static("?1"));
    headers.insert(ACCEPT, http::HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"));
    headers.insert(
        "accept-encoding",
        http::HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(
        "accept-language",
        http::HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert("priority", http::HeaderValue::from_static("u=0, i"));

    // Get cached browser emulator for Opera (avoids Vec allocations on each call)
    // Opera is Chrome-based, so use Chrome fingerprints for all versions
    let browser_emulator = match opera {
        Impersonate::OperaV126 => {
            static OPERA_126: OnceLock<BrowserEmulator> = OnceLock::new();
            OPERA_126
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(126, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::CHROME.to_vec());
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
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(127, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::CHROME.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
                    emulator
                })
                .clone()
        }
        Impersonate::OperaV128 => {
            static OPERA_128: OnceLock<BrowserEmulator> = OnceLock::new();
            OPERA_128
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(128, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::CHROME.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
                    emulator
                })
                .clone()
        }
        Impersonate::OperaV129 => {
            static OPERA_129: OnceLock<BrowserEmulator> = OnceLock::new();
            OPERA_129
                .get_or_init(|| {
                    let mut emulator =
                        BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(129, 0, 0));
                    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
                    emulator.signature_algorithms =
                        Some(emulation::signature_algorithms::CHROME.to_vec());
                    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
                    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
                    emulator
                })
                .clone()
        }
        _ => unreachable!(),
    };

    let http2 = build_http2_settings(opera);

    crate::imp::BrowserSettings {
        browser_emulator,
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
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Mobile Safari/537.36 OPR/126.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/126.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => build_user_agent(opera, crate::imp::random_impersonate_os()),
        },
        // Opera 127 is based on Chrome 143
        Impersonate::OperaV127 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Mobile Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/127.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => build_user_agent(opera, crate::imp::random_impersonate_os()),
        },
        // Opera 128 is based on Chrome 144
        Impersonate::OperaV128 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Mobile Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/128.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => build_user_agent(opera, crate::imp::random_impersonate_os()),
        },
        // Opera 129 is based on Chrome 145
        Impersonate::OperaV129 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/129.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => build_user_agent(opera, crate::imp::random_impersonate_os()),
        },
        _ => unreachable!(),
    }
}

/// Builds a sec-ch-ua header value for an Opera version and OS.
fn build_sec_ch_ua(opera: Impersonate, _os: crate::imp::ImpersonateOS) -> &'static str {
    match opera {
        Impersonate::OperaV126 => r#""Not:A-Brand";v="99", "Opera";v="126", "Chromium";v="142""#,
        Impersonate::OperaV127 => r#""Not:A-Brand";v="99", "Opera";v="127", "Chromium";v="143""#,
        Impersonate::OperaV128 => r#""Not:A-Brand";v="99", "Opera";v="128", "Chromium";v="144""#,
        Impersonate::OperaV129 => r#""Not:A-Brand";v="99", "Opera";v="129", "Chromium";v="145""#,
        _ => unreachable!(),
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
        _ => os_platform(crate::imp::random_impersonate_os()),
    }
}

/// Builds HTTP/2 settings for an Opera version.
#[cfg(feature = "http2")]
fn build_http2_settings(_opera: Impersonate) -> crate::imp::Http2Data {
    use crate::imp::{PseudoId, PseudoOrder, SettingId, SettingsOrder};

    let settings_order = Some(
        SettingsOrder::builder()
            .push(SettingId::HeaderTableSize) // 1: 65536
            .push(SettingId::EnablePush) // 2: 0
            .push(SettingId::InitialWindowSize) // 4: 6291456
            .push(SettingId::MaxHeaderListSize) // 6: 262144
            .build_without_extend(),
    );

    let headers_pseudo_order = Some(
        PseudoOrder::builder()
            .push(PseudoId::Method)
            .push(PseudoId::Authority)
            .push(PseudoId::Scheme)
            .push(PseudoId::Path)
            .build(),
    );

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
        headers_priority: Some((255, 0, true)),
        headers_order: Some(vec![
            http::HeaderName::from_static("sec-ch-ua"),
            http::HeaderName::from_static("sec-ch-ua-mobile"),
            http::HeaderName::from_static("sec-ch-ua-platform"),
            http::HeaderName::from_static("upgrade-insecure-requests"),
            http::HeaderName::from_static("user-agent"),
            http::HeaderName::from_static("accept"),
            http::HeaderName::from_static("sec-fetch-site"),
            http::HeaderName::from_static("sec-fetch-mode"),
            http::HeaderName::from_static("sec-fetch-user"),
            http::HeaderName::from_static("sec-fetch-dest"),
            http::HeaderName::from_static("accept-encoding"),
            http::HeaderName::from_static("accept-language"),
            http::HeaderName::from_static("priority"),
        ]),
        initial_stream_id: None,
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

    const OPERA129_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0";
    const OPERA129_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const OPERA129_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";
    const OPERA129_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_opera129() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::OperaV129)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, OPERA129_USER_AGENT);
        assert_eq!(json.ja4, OPERA129_JA4);
        assert_eq!(json.akamai_hash, OPERA129_AKAMAI_HASH);
        assert_eq!(json.akamai_text, OPERA129_AKAMAI_TEXT);
    }
}
