//! Chrome browser impersonation settings.
//!
//! This module provides configuration for impersonating various Chrome browser versions.
//! Each version has its own TLS fingerprint, ALPN protocols, and default HTTP headers
//! that mimic the real Chrome browser behavior.
//!
//! # Usage
//!
//! ```rust
//! use primp::{Client, Impersonate};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::builder()
//!         .impersonate(Impersonate::ChromeV144)
//!         .build()?;
//!
//!     //let response = client.get("https://example.com").send().await?;
//!     Ok(())
//! }
//! ```

pub use crate::imp::Impersonate;
#[cfg(feature = "http2")]
use crate::imp::{PseudoId, PseudoOrder, SettingId, SettingsOrder};
use http::header::*;
use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
use rustls::crypto::emulation;
use std::sync::{Arc, OnceLock};

/// Builds browser settings for a specific Chrome version and OS.
pub(crate) fn build_chrome_settings(
    chrome: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    let os = if matches!(os, crate::imp::ImpersonateOS::Random) {
        crate::imp::random_impersonate_os()
    } else {
        os
    };
    let user_agent = build_user_agent(chrome, os);
    let sec_ch_ua = build_sec_ch_ua(chrome, os);

    let mut headers = base_chrome_headers().clone();
    headers.insert(USER_AGENT, http::HeaderValue::from_static(user_agent));
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
        http::HeaderValue::from_static(crate::imp::os_platform(os)),
    );

    // Get cached browser emulator for Chrome (avoids Vec allocations on each call)
    let browser_emulator = chrome_emulator(chrome);

    let http2 = build_http2_settings(chrome);

    crate::imp::BrowserSettings {
        browser_emulator,
        http2,
        headers,
        gzip: true,
        brotli: true,
        zstd: true,
        deflate: true,
    }
}

/// Builds a User-Agent string for a Chrome version and OS.
fn build_user_agent(chrome: Impersonate, os: crate::imp::ImpersonateOS) -> &'static str {
    match chrome {
        Impersonate::ChromeV144 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/144.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV145 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/145.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV146 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/146.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV147 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/147.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV148 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/148.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

/// Builds a sec-ch-ua header value for a Chrome version and OS.
fn build_sec_ch_ua(chrome: Impersonate, _os: crate::imp::ImpersonateOS) -> &'static str {
    match chrome {
        Impersonate::ChromeV144 => {
            r#""Not(A:Brand";v="8", "Chromium";v="144", "Google Chrome";v="144""#
        }
        Impersonate::ChromeV145 => {
            r#""Not:A-Brand";v="99", "Google Chrome";v="145", "Chromium";v="145""#
        }
        Impersonate::ChromeV146 => {
            r#""Chromium";v="146", "Not-A.Brand";v="24", "Google Chrome";v="146""#
        }
        Impersonate::ChromeV147 => {
            r#""Google Chrome";v="147", "Not.A/Brand";v="8", "Chromium";v="147""#
        }
        Impersonate::ChromeV148 => {
            r#""Chromium";v="148", "Google Chrome";v="148", "Not/A)Brand";v="99""#
        }
        _ => unreachable!(),
    }
}

/// Builds HTTP/2 settings for a Chrome version.
#[cfg(feature = "http2")]
fn build_http2_settings(chrome: Impersonate) -> crate::imp::Http2Data {
    // Chrome 148 uses different header order (sec-ch-ua after sec-fetch-*)
    let headers_order = if matches!(chrome, Impersonate::ChromeV148) {
        Some(crate::imp::header_order_upgrade_first_sec_chua_last().clone())
    } else {
        Some(crate::imp::header_order_sec_chua_first().clone())
    };

    crate::imp::Http2Data {
        settings_order: Some(chrome_settings_order().clone()),
        headers_pseudo_order: Some(chrome_pseudo_order().clone()),
        headers_order,
        headers_priority: Some((255, 0, true)),
        initial_stream_window_size: Some(crate::imp::CHROME_INITIAL_STREAM_WINDOW),
        initial_connection_window_size: Some(crate::imp::CHROME_INITIAL_CONNECTION_WINDOW),
        max_header_list_size: Some(crate::imp::CHROME_MAX_HEADER_LIST_SIZE),
        header_table_size: Some(crate::imp::CHROME_HEADER_TABLE_SIZE),
        ..Default::default()
    }
}

fn chrome_emulator(chrome: Impersonate) -> Arc<BrowserEmulator> {
    match chrome {
        Impersonate::ChromeV144 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(144)))
                .clone()
        }
        Impersonate::ChromeV145 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(145)))
                .clone()
        }
        Impersonate::ChromeV146 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(146)))
                .clone()
        }
        Impersonate::ChromeV147 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(147)))
                .clone()
        }
        Impersonate::ChromeV148 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(148)))
                .clone()
        }
        _ => unreachable!(),
    }
}

fn new_chrome_emulator(major: u16) -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Chrome, BrowserVersion::new(major, 0, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
    emulator.signature_algorithms = Some(emulation::signature_algorithms::CHROME.to_vec());
    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
    emulator
}

fn base_chrome_headers() -> &'static http::HeaderMap {
    static BASE: OnceLock<http::HeaderMap> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut headers = http::HeaderMap::with_capacity(13);
        headers.insert(ACCEPT, http::HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"));
        headers.insert("accept-encoding", http::HeaderValue::from_static("gzip, deflate, br, zstd"));
        headers.insert("accept-language", http::HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert("upgrade-insecure-requests", http::HeaderValue::from_static("1"));
        headers.insert("sec-fetch-site", http::HeaderValue::from_static("none"));
        headers.insert("sec-fetch-mode", http::HeaderValue::from_static("navigate"));
        headers.insert("sec-fetch-dest", http::HeaderValue::from_static("document"));
        headers.insert("sec-fetch-user", http::HeaderValue::from_static("?1"));
        headers.insert("priority", http::HeaderValue::from_static("u=0, i"));
        headers
    })
}

#[cfg(feature = "http2")]
fn chrome_settings_order() -> &'static SettingsOrder {
    static ORDER: OnceLock<SettingsOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        SettingsOrder::builder()
            .push(SettingId::HeaderTableSize)
            .push(SettingId::EnablePush)
            .push(SettingId::InitialWindowSize)
            .push(SettingId::MaxHeaderListSize)
            .build_without_extend()
    })
}

#[cfg(feature = "http2")]
fn chrome_pseudo_order() -> &'static PseudoOrder {
    static ORDER: OnceLock<PseudoOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        PseudoOrder::builder()
            .push(PseudoId::Method)
            .push(PseudoId::Authority)
            .push(PseudoId::Scheme)
            .push(PseudoId::Path)
            .build()
    })
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

    const CHROME146_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    const CHROME146_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const CHROME146_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";
    const CHROME146_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_chrome146() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::ChromeV146)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, CHROME146_USER_AGENT);
        assert_eq!(json.ja4, CHROME146_JA4);
        assert_eq!(json.akamai_hash, CHROME146_AKAMAI_HASH);
        assert_eq!(json.akamai_text, CHROME146_AKAMAI_TEXT);
    }
}
