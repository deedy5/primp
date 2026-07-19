//! Chrome browser impersonation settings (per-version TLS, HTTP/2, and headers).
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

use super::{PseudoId, PseudoOrder, SettingId, SettingsOrder};
pub use crate::imp::Impersonate;
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

    // Chrome 150 adds a `sec-purpose` header.
    if matches!(chrome, Impersonate::ChromeV150) {
        headers.insert(
            "sec-purpose",
            http::HeaderValue::from_static("prefetch;prerender"),
        );
    }

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
        Impersonate::ChromeV149 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/149.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV150 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/150.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV151 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/151.0.0.0 Mobile/15E148 Safari/604.1",
            _ => unreachable!(),
        },
        Impersonate::ChromeV152 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Mobile Safari/537.36",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/152.0.0.0 Mobile/15E148 Safari/604.1",
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
            // Two brands only, `Not-A.Brand` first — NOT the usual three-brand Chrome layout.
            r#""Not-A.Brand";v="24", "Chromium";v="146""#
        }
        Impersonate::ChromeV147 => {
            r#""Google Chrome";v="147", "Not.A/Brand";v="8", "Chromium";v="147""#
        }
        Impersonate::ChromeV148 => {
            r#""Chromium";v="148", "Google Chrome";v="148", "Not/A)Brand";v="99""#
        }
        Impersonate::ChromeV149 => {
            r#""Google Chrome";v="149", "Chromium";v="149", "Not)A;Brand";v="24""#
        }
        Impersonate::ChromeV150 => {
            r#""Not;A=Brand";v="8", "Chromium";v="150", "Google Chrome";v="150""#
        }
        Impersonate::ChromeV151 => {
            r#""Not=A?Brand";v="99", "Google Chrome";v="151", "Chromium";v="151""#
        }
        Impersonate::ChromeV152 => {
            r#""Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152""#
        }
        _ => unreachable!(),
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
        Impersonate::ChromeV149 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(149)))
                .clone()
        }
        Impersonate::ChromeV150 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(150)))
                .clone()
        }
        Impersonate::ChromeV151 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(151)))
                .clone()
        }
        Impersonate::ChromeV152 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_chrome_emulator(152)))
                .clone()
        }
        _ => unreachable!(),
    }
}

fn new_chrome_emulator(major: u16) -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Chrome, BrowserVersion::new(major, 0, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
    // Chrome 150+ advertises ML-DSA post-quantum signature schemes; earlier versions do not.
    emulator.signature_algorithms = Some(if major >= 150 {
        emulation::signature_algorithms::CHROME_V150.to_vec()
    } else {
        emulation::signature_algorithms::CHROME.to_vec()
    });
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

/// Builds HTTP/2 settings for a Chrome version.
fn build_http2_settings(chrome: Impersonate) -> crate::imp::Http2Data {
    // Chrome 148+ uses different header order (sec-ch-ua after sec-fetch-*),
    // except Chrome 150 which reverts to sec-ch-ua first and adds `sec-purpose`,
    // and 152 which again uses upgrade-first.
    let headers_order = if matches!(
        chrome,
        Impersonate::ChromeV148 | Impersonate::ChromeV149 | Impersonate::ChromeV152
    ) {
        Some(crate::imp::header_order_upgrade_first_sec_chua_last().clone())
    } else if matches!(chrome, Impersonate::ChromeV150) {
        Some(chrome150_header_order().clone())
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

/// Chrome 150 header order, identical to `header_order_sec_chua_first` but with
/// `sec-purpose` inserted right after `user-agent` (per real capture).
fn chrome150_header_order() -> &'static Vec<http::HeaderName> {
    static ORDER: OnceLock<Vec<http::HeaderName>> = OnceLock::new();
    ORDER.get_or_init(|| {
        vec![
            http::HeaderName::from_static("sec-ch-ua"),
            http::HeaderName::from_static("sec-ch-ua-mobile"),
            http::HeaderName::from_static("sec-ch-ua-platform"),
            http::HeaderName::from_static("upgrade-insecure-requests"),
            http::HeaderName::from_static("user-agent"),
            http::HeaderName::from_static("sec-purpose"),
            http::HeaderName::from_static("accept"),
            http::HeaderName::from_static("sec-fetch-site"),
            http::HeaderName::from_static("sec-fetch-mode"),
            http::HeaderName::from_static("sec-fetch-user"),
            http::HeaderName::from_static("sec-fetch-dest"),
            http::HeaderName::from_static("accept-encoding"),
            http::HeaderName::from_static("accept-language"),
            http::HeaderName::from_static("priority"),
        ]
    })
}

#[cfg(test)]
mod tests {
    use crate::imp::{get_browser_settings, Impersonate, ImpersonateOS};

    const CHROME_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";
    const CHROME_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";

    const CHROME146_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const CHROME146_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_fe0d,002b,0012,44cd,000a,ff01,0033,0005,0023,0000,000d,0010,000b,001b,0017,002d_0403,0804,0401,0503,0805,0501,0806,0601";
    const CHROME146_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";

    #[test]
    fn chrome146_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::ChromeV146);
        assert_eq!(ja4, CHROME146_JA4, "Chrome 146 JA4 mismatch");
        assert_eq!(ja4_ro, CHROME146_JA4_RO, "Chrome 146 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::ChromeV146, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            CHROME146_USER_AGENT
        );
        // Two brands, "Not-A.Brand" FIRST — code used to send 3 brands in the reverse order.
        assert_eq!(
            settings.headers.get("sec-ch-ua").unwrap().to_str().unwrap(),
            "\"Not-A.Brand\";v=\"24\", \"Chromium\";v=\"146\"",
            "Chrome 146 sec-ch-ua mismatch vs capture"
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, CHROME_AKAMAI_TEXT, "Chrome 146 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            CHROME_AKAMAI_HASH,
            "Chrome 146 akamai_hash mismatch"
        );
    }

    const CHROME148_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const CHROME148_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_0005,fe0d,0033,0023,0010,0000,002b,0017,001b,44cd,ff01,000d,002d,000b,0012,000a_0403,0804,0401,0503,0805,0501,0806,0601";
    const CHROME148_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

    #[test]
    fn chrome148_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::ChromeV148);
        assert_eq!(ja4, CHROME148_JA4, "Chrome 148 JA4 mismatch");
        assert_eq!(ja4_ro, CHROME148_JA4_RO, "Chrome 148 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::ChromeV148, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            CHROME148_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, CHROME_AKAMAI_TEXT, "Chrome 148 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            CHROME_AKAMAI_HASH,
            "Chrome 148 akamai_hash mismatch"
        );
    }

    const CHROME149_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const CHROME149_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_000d,0005,ff01,0023,0033,0010,0000,000b,fe0d,0017,002b,001b,002d,000a,44cd,0012_0403,0804,0401,0503,0805,0501,0806,0601";
    const CHROME149_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36";

    #[test]
    fn chrome149_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::ChromeV149);
        assert_eq!(ja4, CHROME149_JA4, "Chrome 149 JA4 mismatch");
        assert_eq!(ja4_ro, CHROME149_JA4_RO, "Chrome 149 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::ChromeV149, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            CHROME149_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, CHROME_AKAMAI_TEXT, "Chrome 149 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            CHROME_AKAMAI_HASH,
            "Chrome 149 akamai_hash mismatch"
        );
    }

    const CHROME150_JA4: &str = "t13d1516h2_8daaf6152771_806a8c22fdea";
    const CHROME150_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_ff01,0000,000d,0010,fe0d,0023,000a,000b,0017,0012,0033,002b,001b,0005,44cd,002d_0904,0905,0906,0403,0804,0401,0503,0805,0501,0806,0601";
    const CHROME150_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

    #[test]
    fn chrome150_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::ChromeV150);
        assert_eq!(ja4, CHROME150_JA4, "Chrome 150 JA4 mismatch");
        assert_eq!(ja4_ro, CHROME150_JA4_RO, "Chrome 150 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::ChromeV150, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            CHROME150_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, CHROME_AKAMAI_TEXT, "Chrome 150 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            CHROME_AKAMAI_HASH,
            "Chrome 150 akamai_hash mismatch"
        );
    }

    const CHROME151_JA4: &str = "t13d1516h2_8daaf6152771_806a8c22fdea";
    const CHROME151_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_001b,0000,44cd,000b,0033,ff01,000d,002d,0010,002b,fe0d,0023,000a,0012,0017,0005_0904,0905,0906,0403,0804,0401,0503,0805,0501,0806,0601";
    const CHROME151_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36";

    #[test]
    fn chrome151_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::ChromeV151);
        assert_eq!(ja4, CHROME151_JA4, "Chrome 151 JA4 mismatch");
        assert_eq!(ja4_ro, CHROME151_JA4_RO, "Chrome 151 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::ChromeV151, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            CHROME151_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, CHROME_AKAMAI_TEXT, "Chrome 151 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            CHROME_AKAMAI_HASH,
            "Chrome 151 akamai_hash mismatch"
        );
    }

    const CHROME152_JA4: &str = "t13d1517h2_8daaf6152771_cb7bf5808d99";
    const CHROME152_JA4_RO: &str = "t13d1517h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_ff01,0000,fe0d,001b,000b,0012,0033,002d,ca34,000a,44cd,0017,002b,0005,0023,000d,0010_0904,0905,0906,0403,0804,0401,0503,0805,0501,0806,0601";
    const CHROME152_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";

    #[test]
    fn chrome152_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::ChromeV152);
        assert_eq!(ja4, CHROME152_JA4, "Chrome 152 JA4 mismatch");
        assert_eq!(ja4_ro, CHROME152_JA4_RO, "Chrome 152 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::ChromeV152, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            CHROME152_USER_AGENT
        );
        assert_eq!(
            settings.headers.get("sec-ch-ua").unwrap().to_str().unwrap(),
            "\"Chromium\";v=\"152\", \"Not?A_Brand\";v=\"24\", \"Google Chrome\";v=\"152\"",
            "Chrome 152 sec-ch-ua mismatch vs capture"
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, CHROME_AKAMAI_TEXT, "Chrome 152 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            CHROME_AKAMAI_HASH,
            "Chrome 152 akamai_hash mismatch"
        );
    }
}
