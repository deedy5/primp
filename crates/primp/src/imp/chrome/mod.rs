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
    if matches!(chrome, Impersonate::ChromeV156) {
        // Chrome 156 advertises JPEG XL support in navigation requests.
        headers.insert(ACCEPT, http::HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/jxl,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"));
    }
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
        Impersonate::ChromeV150 | Impersonate::ChromeV150_0_7871_187 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/150.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV156 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/156.0.0.0 Mobile/15E148 Safari/604.1",
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
        Impersonate::ChromeV150 | Impersonate::ChromeV150_0_7871_187 => {
            r#""Google Chrome";v="150", "Chromium";v="150", "Not?A_Brand";v="24""#
        }
        Impersonate::ChromeV156 => {
            r#""Not:A-Brand";v="8", "Chromium";v="156", "Google Chrome";v="156""#
        }
        _ => unreachable!(),
    }
}

/// Builds HTTP/2 settings for a Chrome version.
#[cfg(feature = "http2")]
fn build_http2_settings(chrome: Impersonate) -> crate::imp::Http2Data {
    // Chrome 148/150 use a header order with sec-ch-ua after sec-fetch-*.
    // The alternate Chrome 150_2 variant (matching tls.peet.ws) keeps the older
    // sec-ch-ua-first order.
    let headers_order = if matches!(chrome, Impersonate::ChromeV148 | Impersonate::ChromeV150) {
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
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(144, ChromeSigAlgs::Classic)))
                .clone()
        }
        Impersonate::ChromeV145 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(145, ChromeSigAlgs::Classic)))
                .clone()
        }
        Impersonate::ChromeV146 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(146, ChromeSigAlgs::Classic)))
                .clone()
        }
        Impersonate::ChromeV147 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(147, ChromeSigAlgs::Classic)))
                .clone()
        }
        Impersonate::ChromeV148 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(148, ChromeSigAlgs::Classic)))
                .clone()
        }
        Impersonate::ChromeV150 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(150, ChromeSigAlgs::V150Sorted)))
                .clone()
        }
        Impersonate::ChromeV150_0_7871_187 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(150, ChromeSigAlgs::V150Wire)))
                .clone()
        }
        Impersonate::ChromeV156 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(156, ChromeSigAlgs::V150Wire)))
                .clone()
        }
        _ => unreachable!(),
    }
}

/// Signature-algorithm profile selection for the Chrome emulator.
#[derive(Clone, Copy)]
enum ChromeSigAlgs {
    /// Classic 8-algorithm list used by Chrome <150.
    Classic,
    /// Chrome 150+ list sorted ascending (matches JA4 regardless of tool sorting).
    V150Sorted,
    /// Chrome 150 real wire order (ML-DSA first) as observed on tls.peet.ws.
    V150Wire,
}

fn new_chrome_emulator(major: u16, sig_algs: ChromeSigAlgs) -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Chrome, BrowserVersion::new(major, 0, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
    let signature_algorithms = match sig_algs {
        ChromeSigAlgs::Classic => emulation::signature_algorithms::CHROME,
        ChromeSigAlgs::V150Sorted => emulation::signature_algorithms::CHROME_150,
        ChromeSigAlgs::V150Wire => emulation::signature_algorithms::CHROME_150_WIRE,
    };
    emulator.signature_algorithms = Some(signature_algorithms.to_vec());
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

    const CHROME150_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";
    // Chrome 150 adds the ML-DSA signature algorithms, so only the JA4_c extension
    // hash changes versus Chrome 146; the ciphers and HTTP/2 fingerprint are unchanged.
    // browserleaks counts SNI + ALPN in the extension total (t13d1516h2); tools that
    // omit them (e.g. tls.peet.ws) report the same hashes as t13d1514h2.
    const CHROME150_JA4: &str = "t13d1516h2_8daaf6152771_d85c08a3ce5e";
    const CHROME150_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";
    const CHROME150_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_chrome150() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Linux)
            .impersonate(Impersonate::ChromeV150)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.browserleaks.com/json")
            .send()
            .await
            .unwrap();

        let json: BrowserLeaksResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, CHROME150_USER_AGENT);
        assert_eq!(json.ja4, CHROME150_JA4);
        assert_eq!(json.akamai_hash, CHROME150_AKAMAI_HASH);
        assert_eq!(json.akamai_text, CHROME150_AKAMAI_TEXT);
    }
    /// tls.peet.ws `/api/all` subset used by the Chrome 156 check.
    #[derive(Debug, Deserialize)]
    struct PeetTls {
        ja3: String,
        ja4: String,
        peetprint_hash: String,
    }
    #[derive(Debug, Deserialize)]
    struct PeetHttp2 {
        akamai_fingerprint_hash: String,
    }
    #[derive(Debug, Deserialize)]
    struct PeetResponse {
        user_agent: String,
        tls: PeetTls,
        http2: PeetHttp2,
    }

    // Values from a real Chrome 156 (Windows) capture on tls.peet.ws.
    const CHROME156_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36";
    const CHROME156_JA3: &str = "771,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,5-45-65281-27-10-51764-65037-16-0-13-23-43-11-18-17613-35-51,4588-29-23-24,0";
    const CHROME156_JA4: &str = "t13d1517h2_8daaf6152771_cb7bf5808d99";
    const CHROME156_PEETPRINT_HASH: &str = "fc97c1cdfb1409c9a9326c1b726d1dee";
    const CHROME156_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";

    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_chrome156() {
        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Windows)
            .impersonate(Impersonate::ChromeV156)
            .build()
            .unwrap();

        let response = client
            .get("https://tls.peet.ws/api/all")
            .send()
            .await
            .unwrap();

        let json: PeetResponse = response.json().await.unwrap();

        assert_eq!(json.user_agent, CHROME156_USER_AGENT);
        assert_eq!(json.tls.ja3, CHROME156_JA3);
        assert_eq!(json.tls.ja4, CHROME156_JA4);
        assert_eq!(json.tls.peetprint_hash, CHROME156_PEETPRINT_HASH);
        assert_eq!(json.http2.akamai_fingerprint_hash, CHROME156_AKAMAI_HASH);
    }

    /// Reads the first TLS record (the ClientHello) the client sends to a local
    /// listener and returns `(cipher_suites, extensions[(type, data)])`.
    #[cfg(feature = "impersonate")]
    async fn capture_client_hello(imp: Impersonate) -> (Vec<u16>, Vec<(u16, Vec<u8>)>) {
        use std::io::Read;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut hdr = [0u8; 5];
            sock.read_exact(&mut hdr).unwrap();
            assert_eq!(hdr[0], 0x16, "expected a TLS handshake record");
            let len = u16::from_be_bytes([hdr[3], hdr[4]]) as usize;
            let mut body = vec![0u8; len];
            sock.read_exact(&mut body).unwrap();
            body // dropping `sock` aborts the handshake
        });

        let client = Client::builder()
            .impersonate_os(crate::imp::ImpersonateOS::Windows)
            .impersonate(imp)
            .build()
            .unwrap();
        let _ = client
            .get(format!("https://localhost:{port}/"))
            .send()
            .await;
        let body = server.join().unwrap();

        // Handshake header (4) + client_version (2) + random (32).
        let mut i = 4 + 2 + 32;
        let sid_len = body[i] as usize;
        i += 1 + sid_len;
        let cs_len = u16::from_be_bytes([body[i], body[i + 1]]) as usize;
        i += 2;
        let ciphers = body[i..i + cs_len]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        i += cs_len;
        let comp_len = body[i] as usize;
        i += 1 + comp_len;
        let ext_total = u16::from_be_bytes([body[i], body[i + 1]]) as usize;
        i += 2;
        let end = i + ext_total;
        let mut exts = Vec::new();
        while i < end {
            let ty = u16::from_be_bytes([body[i], body[i + 1]]);
            let len = u16::from_be_bytes([body[i + 2], body[i + 3]]) as usize;
            exts.push((ty, body[i + 4..i + 4 + len].to_vec()));
            i += 4 + len;
        }
        (ciphers, exts)
    }

    #[cfg(feature = "impersonate")]
    fn is_grease(v: u16) -> bool {
        (v & 0x0f0f) == 0x0a0a && (v >> 8) == (v & 0xff)
    }

    /// Offline check: Chrome 156 ClientHello matches the real browser's
    /// tls.peet.ws capture (cipher + extension order => JA3, and the
    /// `trust_anchors` 0xCA34 payload).
    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_chrome156_client_hello_offline() {
        let (ciphers, exts) = capture_client_hello(Impersonate::ChromeV156).await;

        let ciphers: Vec<String> = ciphers
            .into_iter()
            .filter(|c| !is_grease(*c))
            .map(|c| c.to_string())
            .collect();
        let ext_types: Vec<String> = exts
            .iter()
            .map(|(t, _)| *t)
            .filter(|t| !is_grease(*t))
            .map(|t| t.to_string())
            .collect();

        assert!(
            is_grease(exts.first().unwrap().0),
            "first extension must be GREASE"
        );
        assert!(
            is_grease(exts.last().unwrap().0),
            "last extension must be GREASE"
        );
        assert_eq!(
            ciphers.join("-"),
            "4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53"
        );
        assert_eq!(
            ext_types.join("-"),
            "5-45-65281-27-10-51764-65037-16-0-13-23-43-11-18-17613-35-51"
        );

        let u16_list = |ty: u16, prefix: usize| -> Vec<u16> {
            let data = &exts.iter().find(|(t, _)| *t == ty).unwrap().1;
            data[prefix..]
                .chunks(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .filter(|v| !is_grease(*v))
                .collect()
        };
        // supported_groups (u16 list length prefix).
        assert_eq!(u16_list(10, 2), vec![4588, 29, 23, 24]);
        // signature_algorithms, wire order (ML-DSA first).
        assert_eq!(
            u16_list(13, 2),
            vec![
                0x0904, 0x0905, 0x0906, 0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501, 0x0806,
                0x0601
            ]
        );

        let trust_anchors = &exts.iter().find(|(t, _)| *t == 0xCA34).unwrap().1;
        assert_eq!(
            trust_anchors.as_slice(),
            rustls::crypto::emulation::trust_anchors::CHROME_156
        );
        assert_eq!(trust_anchors.len(), 186);
    }

    /// Older Chrome profiles must keep their previous 16-extension fingerprint.
    #[tokio::test]
    #[cfg(feature = "impersonate")]
    async fn test_chrome150_has_no_trust_anchors_offline() {
        let (_, exts) = capture_client_hello(Impersonate::ChromeV150_0_7871_187).await;
        assert!(exts.iter().all(|(t, _)| *t != 0xCA34));
    }
}
