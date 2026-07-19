//! Firefox browser impersonation settings.

pub use crate::imp::Impersonate;
use rustls::client::{BrowserEmulator, BrowserType, BrowserVersion};
use rustls::crypto::emulation;
use std::sync::{Arc, OnceLock};

/// Builds browser settings for a specific Firefox version and OS.
pub(crate) fn build_firefox_settings(
    firefox: Impersonate,
    os: crate::imp::ImpersonateOS,
) -> crate::imp::BrowserSettings {
    let os = if matches!(os, crate::imp::ImpersonateOS::Random) {
        crate::imp::random_impersonate_os()
    } else {
        os
    };
    let user_agent = build_user_agent(firefox, os);
    let headers = build_headers(user_agent);

    let browser_emulator = match firefox {
        Impersonate::FirefoxV140 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(140)))
                .clone()
        }
        Impersonate::FirefoxV146 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(146)))
                .clone()
        }
        Impersonate::FirefoxV147 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(147)))
                .clone()
        }
        Impersonate::FirefoxV148 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(148)))
                .clone()
        }
        Impersonate::FirefoxV149 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(149)))
                .clone()
        }
        Impersonate::FirefoxV150 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(150)))
                .clone()
        }
        Impersonate::FirefoxV151 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(151)))
                .clone()
        }
        _ => unreachable!(),
    };

    crate::imp::BrowserSettings {
        browser_emulator,
        http2: build_http2_settings(),
        headers,
        gzip: true,
        brotli: true,
        zstd: true,
        deflate: true,
    }
}

fn new_firefox_emulator(major: u16) -> BrowserEmulator {
    let mut emulator = BrowserEmulator::new(BrowserType::Firefox, BrowserVersion::new(major, 0, 0));
    emulator.cipher_suites = Some(emulation::cipher_suites::FIREFOX.to_vec());
    emulator.signature_algorithms = Some(emulation::signature_algorithms::FIREFOX.to_vec());
    emulator.named_groups = Some(emulation::named_groups::FIREFOX.to_vec());
    emulator.extension_order_seed = Some(emulation::extension_order::FIREFOX);
    emulator
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
            _ => unreachable!(),
        },
        Impersonate::FirefoxV146 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:146.0) Gecko/20100101 Firefox/146.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:146.0) Gecko/146.0 Firefox/146.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/146.0 Mobile/15E148 Safari/605.1",
            _ => unreachable!(),
        },
        Impersonate::FirefoxV147 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:147.0) Gecko/20100101 Firefox/147.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:147.0) Gecko/20100101 Firefox/147.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:147.0) Gecko/147.0 Firefox/147.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/147.0 Mobile/15E148 Safari/605.1",
            _ => unreachable!(),
        },
        Impersonate::FirefoxV148 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:148.0) Gecko/20100101 Firefox/148.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:148.0) Gecko/20100101 Firefox/148.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:148.0) Gecko/20100101 Firefox/148.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:148.0) Gecko/148.0 Firefox/148.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/148.0 Mobile/15E148 Safari/605.1",
            _ => unreachable!(),
        },
        Impersonate::FirefoxV149 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:149.0) Gecko/20100101 Firefox/149.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:149.0) Gecko/20100101 Firefox/149.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:149.0) Gecko/20100101 Firefox/149.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:149.0) Gecko/149.0 Firefox/149.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/149.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        Impersonate::FirefoxV150 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:150.0) Gecko/20100101 Firefox/150.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:150.0) Gecko/20100101 Firefox/150.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:150.0) Gecko/20100101 Firefox/150.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:150.0) Gecko/150.0 Firefox/150.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/150.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        Impersonate::FirefoxV151 => match os {
            crate::imp::ImpersonateOS::Windows => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:151.0) Gecko/20100101 Firefox/151.0",
            crate::imp::ImpersonateOS::MacOS => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:151.0) Gecko/20100101 Firefox/151.0",
            crate::imp::ImpersonateOS::Linux => "Mozilla/5.0 (X11; Linux x86_64; rv:151.0) Gecko/20100101 Firefox/151.0",
            crate::imp::ImpersonateOS::Android => "Mozilla/5.0 (Android 14; Mobile; rv:151.0) Gecko/151.0 Firefox/151.0",
            crate::imp::ImpersonateOS::IOS => "Mozilla/5.0 (iPhone; CPU iPhone OS 18_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/151.0 Mobile/15E148 Safari/605.1.15",
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

fn firefox_base_headers() -> &'static http::HeaderMap {
    static BASE: OnceLock<http::HeaderMap> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut headers = http::HeaderMap::with_capacity(12);
        headers.insert(
            http::header::ACCEPT,
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
    })
}

/// Builds default headers for Firefox.
fn build_headers(user_agent: &'static str) -> http::HeaderMap {
    let mut headers = firefox_base_headers().clone();
    headers.insert(
        http::header::USER_AGENT,
        http::HeaderValue::from_static(user_agent),
    );
    headers
}

/// Builds HTTP/2 settings for Firefox.
fn build_http2_settings() -> crate::imp::Http2Data {
    crate::imp::Http2Data {
        initial_stream_window_size: Some(crate::imp::FIREFOX_INITIAL_STREAM_WINDOW),
        initial_connection_window_size: Some(crate::imp::FIREFOX_INITIAL_CONNECTION_WINDOW),
        max_frame_size: Some(16384),
        header_table_size: Some(crate::imp::FIREFOX_HEADER_TABLE_SIZE),
        enable_push: Some(false),
        settings_order: Some(firefox_settings_order().clone()),
        headers_pseudo_order: Some(firefox_pseudo_order().clone()),
        headers_priority: Some((41, 0, false)),
        headers_order: Some(firefox_headers_order().clone()),
        initial_stream_id: Some(3),
        initial_stream_window_size_increment: Some(12451840),
        ..Default::default()
    }
}

fn firefox_settings_order() -> &'static super::SettingsOrder {
    static ORDER: OnceLock<super::SettingsOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        super::SettingsOrder::builder()
            .push(super::SettingId::HeaderTableSize)
            .push(super::SettingId::EnablePush)
            .push(super::SettingId::InitialWindowSize)
            .push(super::SettingId::MaxFrameSize)
            .build_without_extend()
    })
}

fn firefox_pseudo_order() -> &'static super::PseudoOrder {
    static ORDER: OnceLock<super::PseudoOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        super::PseudoOrder::builder()
            .push(super::PseudoId::Method)
            .push(super::PseudoId::Path)
            .push(super::PseudoId::Authority)
            .push(super::PseudoId::Scheme)
            .build()
    })
}

fn firefox_headers_order() -> &'static Vec<http::HeaderName> {
    static ORDER: OnceLock<Vec<http::HeaderName>> = OnceLock::new();
    ORDER.get_or_init(|| {
        vec![
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
        ]
    })
}

#[cfg(test)]
mod tests {
    use crate::imp::{get_browser_settings, Impersonate, ImpersonateOS};

    const FIREFOX_AKAMAI_TEXT: &str = "1:65536;2:0;4:131072;5:16384|12517377|0|m,p,a,s";
    const FIREFOX_AKAMAI_HASH: &str = "6ea73faa8fc5aac76bded7bd238f6433";

    const FIREFOX140_JA4: &str = "t13d1717h2_5b57614c22b0_3cbfd9057e0d";
    const FIREFOX140_JA4_RO: &str = "t13d1717h2_1301,1303,1302,c02b,c02f,cca9,cca8,c02c,c030,c00a,c009,c013,c014,009c,009d,002f,0035_0000,0017,ff01,000a,000b,0023,0010,0005,0022,0012,0033,002b,000d,002d,001c,001b,fe0d_0403,0503,0603,0804,0805,0806,0401,0501,0601,0203,0201";
    const FIREFOX140_USER_AGENT: &str =
        "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";

    #[test]
    fn firefox140_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::FirefoxV140);
        assert_eq!(ja4, FIREFOX140_JA4, "Firefox 140 JA4 mismatch");
        assert_eq!(ja4_ro, FIREFOX140_JA4_RO, "Firefox 140 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::FirefoxV140, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            FIREFOX140_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        assert_eq!(
            text, FIREFOX_AKAMAI_TEXT,
            "Firefox 140 akamai_text mismatch"
        );
        assert_eq!(
            super::super::compute_akamai_hash(&text),
            FIREFOX_AKAMAI_HASH,
            "Firefox 140 akamai_hash mismatch"
        );
    }

    const FIREFOX148_JA4: &str = "t13d1717h2_5b57614c22b0_3cbfd9057e0d";
    const FIREFOX148_JA4_RO: &str = "t13d1717h2_1301,1303,1302,c02b,c02f,cca9,cca8,c02c,c030,c00a,c009,c013,c014,009c,009d,002f,0035_0000,0017,ff01,000a,000b,0023,0010,0005,0022,0012,0033,002b,000d,002d,001c,001b,fe0d_0403,0503,0603,0804,0805,0806,0401,0501,0601,0203,0201";
    const FIREFOX148_USER_AGENT: &str =
        "Mozilla/5.0 (X11; Linux x86_64; rv:148.0) Gecko/20100101 Firefox/148.0";

    #[test]
    fn firefox148_offline() {
        let (ja4, ja4_ro) = super::super::extract_ja4(Impersonate::FirefoxV148);
        assert_eq!(ja4, FIREFOX148_JA4, "Firefox 148 JA4 mismatch");
        assert_eq!(ja4_ro, FIREFOX148_JA4_RO, "Firefox 148 JA4_ro mismatch");
        let settings = get_browser_settings(Impersonate::FirefoxV148, Some(ImpersonateOS::Linux));
        assert_eq!(
            settings
                .headers
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap(),
            FIREFOX148_USER_AGENT
        );
        let text = super::super::compute_akamai_text(&settings.http2);
        let hash = super::super::compute_akamai_hash(&text);
        assert_eq!(
            text, FIREFOX_AKAMAI_TEXT,
            "Firefox 148 akamai_text mismatch"
        );
        assert_eq!(
            hash, FIREFOX_AKAMAI_HASH,
            "Firefox 148 akamai_hash mismatch"
        );
    }
}
