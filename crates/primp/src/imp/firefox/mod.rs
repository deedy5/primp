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
    let user_agent = build_user_agent(firefox, os);
    let headers = build_headers(user_agent);

    let browser_emulator = match firefox {
        Impersonate::FirefoxV140 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(140))).clone()
        }
        Impersonate::FirefoxV146 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(146))).clone()
        }
        Impersonate::FirefoxV147 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(147))).clone()
        }
        Impersonate::FirefoxV148 => {
            static EMU: OnceLock<Arc<BrowserEmulator>> = OnceLock::new();
            EMU.get_or_init(|| Arc::new(new_firefox_emulator(148))).clone()
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

fn firefox_base_headers() -> &'static http::HeaderMap {
    static BASE: OnceLock<http::HeaderMap> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut headers = http::HeaderMap::with_capacity(12);
        headers.insert(http::header::ACCEPT, http::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ));
        headers.insert("accept-language", http::HeaderValue::from_static("en-US,en;q=0.5"));
        headers.insert("accept-encoding", http::HeaderValue::from_static("gzip, deflate, br, zstd"));
        headers.insert("dnt", http::HeaderValue::from_static("1"));
        headers.insert("sec-gpc", http::HeaderValue::from_static("1"));
        headers.insert("upgrade-insecure-requests", http::HeaderValue::from_static("1"));
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
    headers.insert(http::header::USER_AGENT, http::HeaderValue::from_static(user_agent));
    headers
}

/// Builds HTTP/2 settings for Firefox.
#[cfg(feature = "http2")]
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

#[cfg(feature = "http2")]
fn firefox_settings_order() -> &'static crate::imp::SettingsOrder {
    static ORDER: OnceLock<crate::imp::SettingsOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        crate::imp::SettingsOrder::builder()
            .push(crate::imp::SettingId::HeaderTableSize)
            .push(crate::imp::SettingId::EnablePush)
            .push(crate::imp::SettingId::InitialWindowSize)
            .push(crate::imp::SettingId::MaxFrameSize)
            .build_without_extend()
    })
}

#[cfg(feature = "http2")]
fn firefox_pseudo_order() -> &'static crate::imp::PseudoOrder {
    static ORDER: OnceLock<crate::imp::PseudoOrder> = OnceLock::new();
    ORDER.get_or_init(|| {
        crate::imp::PseudoOrder::builder()
            .push(crate::imp::PseudoId::Method)
            .push(crate::imp::PseudoId::Path)
            .push(crate::imp::PseudoId::Authority)
            .push(crate::imp::PseudoId::Scheme)
            .build()
    })
}

#[cfg(feature = "http2")]
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
