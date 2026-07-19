//! Opera browser impersonation settings (Chrome-based TLS/HTTP2, Opera-specific headers).
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

use super::{PseudoId, PseudoOrder, SettingId, SettingsOrder};
pub use crate::imp::Impersonate;
use http::header::*;
use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
use rustls::crypto::emulation;
use std::sync::{Arc, OnceLock};

/// Builds browser settings for a specific Opera version and OS.
pub(crate) fn build_opera_settings(
    opera: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    let os = if matches!(os, crate::imp::ImpersonateOS::Random) {
        crate::imp::random_impersonate_os()
    } else {
        os
    };
    let user_agent = build_user_agent(opera, os);
    let sec_ch_ua = build_sec_ch_ua(opera, os);

    let mut headers = base_opera_headers().clone();
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

    // Opera 131+ adds cache-control as the first header
    if matches!(opera, Impersonate::OperaV131 | Impersonate::OperaV132) {
        headers.insert("cache-control", http::HeaderValue::from_static("max-age=0"));
    }

    // Get cached browser emulator for Opera (avoids Vec allocations on each call)
    let browser_emulator = opera_emulator(opera);

    let http2 = build_http2_settings(opera);

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
            _ => unreachable!(),
        },
        // Opera 127 is based on Chrome 143
        Impersonate::OperaV127 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Mobile Safari/537.36 OPR/127.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/127.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 128 is based on Chrome 144
        Impersonate::OperaV128 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Mobile Safari/537.36 OPR/128.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/128.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 129 is based on Chrome 145
        Impersonate::OperaV129 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36 OPR/129.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/129.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 130 is based on Chrome 146
        Impersonate::OperaV130 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36 OPR/130.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36 OPR/130.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36 OPR/130.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Mobile Safari/537.36 OPR/130.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/130.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 131 is based on Chrome 147
        Impersonate::OperaV131 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 OPR/131.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 OPR/131.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 OPR/131.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36 OPR/131.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/131.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 132 is based on Chrome 148
        Impersonate::OperaV132 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36 OPR/132.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36 OPR/132.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36 OPR/132.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Mobile Safari/537.36 OPR/132.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/132.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 133 is based on Chrome 149
        Impersonate::OperaV133 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36 OPR/133.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36 OPR/133.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36 OPR/133.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Mobile Safari/537.36 OPR/133.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/133.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 134 is based on Chrome 150
        Impersonate::OperaV134 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 OPR/134.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 OPR/134.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 OPR/134.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Mobile Safari/537.36 OPR/134.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/134.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        // Opera 135 is based on Chrome 151
        Impersonate::OperaV135 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36 OPR/135.0.0.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36 OPR/135.0.0.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36 OPR/135.0.0.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Linux; Android 10; SM-G970F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Mobile Safari/537.36 OPR/135.0.0.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) OPiOS/135.0.0.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

/// Builds a sec-ch-ua header value for an Opera version and OS.
fn build_sec_ch_ua(opera: Impersonate, _os: crate::imp::ImpersonateOS) -> &'static str {
    match opera {
        Impersonate::OperaV126 => r#""Chromium";v="142", "Opera";v="126", "Not_A Brand";v="99""#,
        Impersonate::OperaV127 => r#""Opera";v="127", "Chromium";v="143", "Not A(Brand";v="24""#,
        Impersonate::OperaV128 => r#""Not(A:Brand";v="8", "Chromium";v="144", "Opera";v="128""#,
        Impersonate::OperaV129 => r#""Not:A-Brand";v="99", "Opera";v="129", "Chromium";v="145""#,
        Impersonate::OperaV130 => r#""Chromium";v="146", "Not-A.Brand";v="24", "Opera";v="130""#,
        Impersonate::OperaV131 => r#""Opera";v="131", "Not.A/Brand";v="8", "Chromium";v="147""#,
        Impersonate::OperaV132 => r#""Chromium";v="148", "Opera";v="132", "Not/A)Brand";v="99""#,
        Impersonate::OperaV133 => r#""Opera";v="133", "Chromium";v="149", "Not)A;Brand";v="24""#,
        Impersonate::OperaV134 => r#""Not;A=Brand";v="8", "Chromium";v="150", "Opera";v="134""#,
        Impersonate::OperaV135 => r#""Not=A?Brand";v="99", "Opera";v="135", "Chromium";v="151""#,
        _ => unreachable!(),
    }
}

fn opera_emulator(opera: Impersonate) -> Arc<BrowserEmulator> {
    match opera {
        Impersonate::OperaV126 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(126)))
                .clone()
        }
        Impersonate::OperaV127 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(127)))
                .clone()
        }
        Impersonate::OperaV128 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(128)))
                .clone()
        }
        Impersonate::OperaV129 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(129)))
                .clone()
        }
        Impersonate::OperaV130 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(130)))
                .clone()
        }
        Impersonate::OperaV131 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(131)))
                .clone()
        }
        Impersonate::OperaV132 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(132)))
                .clone()
        }
        Impersonate::OperaV133 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(133)))
                .clone()
        }
        Impersonate::OperaV134 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(134)))
                .clone()
        }
        Impersonate::OperaV135 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_opera_emulator(135)))
                .clone()
        }
        _ => unreachable!(),
    }
}

fn new_opera_emulator(major: u16) -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Opera, BrowserVersion::new(major, 0, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::CHROME.to_vec());
    // Opera 134+ (Chrome-150-based) advertises ML-DSA post-quantum signature
    // schemes (see the real Opera 134/135 captures); earlier versions do not.
    emulator.signature_algorithms = Some(if major >= 134 {
        emulation::signature_algorithms::CHROME_V150.to_vec()
    } else {
        emulation::signature_algorithms::CHROME.to_vec()
    });
    emulator.named_groups = Some(emulation::named_groups::CHROME.to_vec());
    emulator.extension_order_seed = Some(emulation::extension_order::CHROME);
    emulator
}

fn base_opera_headers() -> &'static http::HeaderMap {
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

/// Builds HTTP/2 settings for an Opera version.
fn build_http2_settings(opera: Impersonate) -> crate::imp::Http2Data {
    // Opera 131+ uses cache-control-first header order
    let headers_order = if matches!(opera, Impersonate::OperaV131 | Impersonate::OperaV132) {
        Some(crate::imp::header_order_cache_control_first().clone())
    } else {
        Some(crate::imp::header_order_sec_chua_first().clone())
    };

    crate::imp::Http2Data {
        settings_order: Some(opera_settings_order().clone()),
        headers_pseudo_order: Some(opera_pseudo_order().clone()),
        headers_order,
        headers_priority: Some((255, 0, true)),
        initial_stream_window_size: Some(crate::imp::CHROME_INITIAL_STREAM_WINDOW),
        initial_connection_window_size: Some(crate::imp::CHROME_INITIAL_CONNECTION_WINDOW),
        max_header_list_size: Some(crate::imp::CHROME_MAX_HEADER_LIST_SIZE),
        header_table_size: Some(crate::imp::CHROME_HEADER_TABLE_SIZE),
        ..Default::default()
    }
}

fn opera_settings_order() -> &'static SettingsOrder {
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

fn opera_pseudo_order() -> &'static PseudoOrder {
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
    use crate::imp::{get_browser_settings, Impersonate, ImpersonateOS};

    const OPERA_AKAMAI_TEXT: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";
    const OPERA_AKAMAI_HASH: &str = "52d84b11737d980aef856699f885ca86";

    const OPERA129_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const OPERA129_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_fe0d,0033,ff01,0017,001b,000a,0000,0010,0005,002b,002d,0012,000b,44cd,000d,0023_0403,0804,0401,0503,0805,0501,0806,0601";
    const OPERA129_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 OPR/129.0.0.0";

    #[test]
    fn opera129_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::OperaV129);
        assert_eq!(ja4, OPERA129_JA4, "Opera 129 JA4 mismatch");
        assert_eq!(ja4_ro, OPERA129_JA4_RO, "Opera 129 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::OperaV129, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            OPERA129_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, OPERA_AKAMAI_TEXT, "Opera 129 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            OPERA_AKAMAI_HASH,
            "Opera 129 akamai_hash mismatch"
        );
    }

    const OPERA131_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const OPERA131_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_002b,002d,44cd,fe0d,0012,0023,0033,0010,001b,000a,0017,0000,000b,ff01,0005,000d_0403,0804,0401,0503,0805,0501,0806,0601";
    const OPERA131_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 OPR/131.0.0.0";

    #[test]
    fn opera131_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::OperaV131);
        assert_eq!(ja4, OPERA131_JA4, "Opera 131 JA4 mismatch");
        assert_eq!(ja4_ro, OPERA131_JA4_RO, "Opera 131 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::OperaV131, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            OPERA131_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, OPERA_AKAMAI_TEXT, "Opera 131 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            OPERA_AKAMAI_HASH,
            "Opera 131 akamai_hash mismatch"
        );
    }

    const OPERA132_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const OPERA132_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_ff01,0005,fe0d,000b,002d,000a,002b,0017,001b,0033,0000,0012,0010,44cd,0023,000d_0403,0804,0401,0503,0805,0501,0806,0601";
    const OPERA132_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36 OPR/132.0.0.0";

    #[test]
    fn opera132_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::OperaV132);
        assert_eq!(ja4, OPERA132_JA4, "Opera 132 JA4 mismatch");
        assert_eq!(ja4_ro, OPERA132_JA4_RO, "Opera 132 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::OperaV132, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            OPERA132_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, OPERA_AKAMAI_TEXT, "Opera 132 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            OPERA_AKAMAI_HASH,
            "Opera 132 akamai_hash mismatch"
        );
    }

    const OPERA133_JA4: &str = "t13d1516h2_8daaf6152771_d8a2da3f94cd";
    const OPERA133_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_002b,0033,0010,000d,0012,ff01,000a,001b,002d,0023,000b,0017,0000,fe0d,44cd,0005_0403,0804,0401,0503,0805,0501,0806,0601";
    const OPERA133_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36 OPR/133.0.0.0";
    const OPERA133_SEC_CH_UA: &str = r#""Opera";v="133", "Chromium";v="149", "Not)A;Brand";v="24""#;

    #[test]
    fn opera133_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::OperaV133);
        assert_eq!(ja4, OPERA133_JA4, "Opera 133 JA4 mismatch");
        assert_eq!(ja4_ro, OPERA133_JA4_RO, "Opera 133 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::OperaV133, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            OPERA133_USER_AGENT
        );
        assert_eq!(
            settings.headers.get("sec-ch-ua").unwrap().to_str().unwrap(),
            OPERA133_SEC_CH_UA
        );
        assert!(
            settings.headers.get("cache-control").is_none(),
            "Opera 133 must not send cache-control"
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, OPERA_AKAMAI_TEXT, "Opera 133 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            OPERA_AKAMAI_HASH,
            "Opera 133 akamai_hash mismatch"
        );
    }

    const OPERA134_JA4: &str = "t13d1516h2_8daaf6152771_806a8c22fdea";
    const OPERA134_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_0010,0023,0017,000a,000d,0000,fe0d,0005,001b,002d,0033,ff01,0012,002b,000b,44cd_0904,0905,0906,0403,0804,0401,0503,0805,0501,0806,0601";
    const OPERA134_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 OPR/134.0.0.0";
    const OPERA134_SEC_CH_UA: &str = r#""Not;A=Brand";v="8", "Chromium";v="150", "Opera";v="134""#;

    #[test]
    fn opera134_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::OperaV134);
        assert_eq!(ja4, OPERA134_JA4, "Opera 134 JA4 mismatch");
        assert_eq!(ja4_ro, OPERA134_JA4_RO, "Opera 134 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::OperaV134, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            OPERA134_USER_AGENT
        );
        assert_eq!(
            settings.headers.get("sec-ch-ua").unwrap().to_str().unwrap(),
            OPERA134_SEC_CH_UA
        );
        assert!(
            settings.headers.get("cache-control").is_none(),
            "Opera 134 must not send cache-control"
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, OPERA_AKAMAI_TEXT, "Opera 134 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            OPERA_AKAMAI_HASH,
            "Opera 134 akamai_hash mismatch"
        );
    }

    const OPERA135_JA4: &str = "t13d1516h2_8daaf6152771_806a8c22fdea";
    const OPERA135_JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_0005,000d,0033,0012,44cd,0010,000a,002d,002b,0023,ff01,0000,000b,fe0d,001b,0017_0904,0905,0906,0403,0804,0401,0503,0805,0501,0806,0601";
    const OPERA135_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36 OPR/135.0.0.0";
    const OPERA135_SEC_CH_UA: &str = r#""Not=A?Brand";v="99", "Opera";v="135", "Chromium";v="151""#;

    #[test]
    fn opera135_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::OperaV135);
        assert_eq!(ja4, OPERA135_JA4, "Opera 135 JA4 mismatch");
        assert_eq!(ja4_ro, OPERA135_JA4_RO, "Opera 135 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::OperaV135, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            OPERA135_USER_AGENT
        );
        assert_eq!(
            settings.headers.get("sec-ch-ua").unwrap().to_str().unwrap(),
            OPERA135_SEC_CH_UA
        );
        assert!(
            settings.headers.get("cache-control").is_none(),
            "Opera 135 must not send cache-control"
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(text, OPERA_AKAMAI_TEXT, "Opera 135 akamai_text mismatch");
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            OPERA_AKAMAI_HASH,
            "Opera 135 akamai_hash mismatch"
        );
    }
}
