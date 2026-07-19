//! Configuration for impersonating browsers (TLS, HTTP/2, and default headers).
//!
//! # Example
//!
//! ```rust
//! use primp::{Client, Impersonate};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Specific version
//!     let client = Client::builder()
//!         .impersonate(Impersonate::ChromeV146)
//!         .build()?;
//!
//!     // Or pick a random version from Chrome family
//!     let client = Client::builder()
//!         .impersonate(Impersonate::Chrome)
//!         .build()?;
//!
//!     Ok(())
//! }
//! ```

use http::HeaderMap;
use rand::prelude::*;
use rustls::client::BrowserEmulator;
use std::sync::{Arc, OnceLock};

pub mod chrome;
pub mod edge;
pub mod firefox;
pub mod opera;
pub mod safari;

// Re-export h2 frame types used for HTTP/2 fingerprinting.
pub use h2::frame::{
    PseudoId, PseudoOrder, PseudoOrderBuilder, SettingId, SettingsOrder, SettingsOrderBuilder,
};

// HTTP/2 magic numbers grouped by browser family.
pub(crate) const CHROME_INITIAL_STREAM_WINDOW: u32 = 6291456;
pub(crate) const CHROME_INITIAL_CONNECTION_WINDOW: u32 = 15728640;
pub(crate) const CHROME_MAX_HEADER_LIST_SIZE: u32 = 262144;
pub(crate) const CHROME_HEADER_TABLE_SIZE: u32 = 65536;

pub(crate) const FIREFOX_INITIAL_STREAM_WINDOW: u32 = 131072;
pub(crate) const FIREFOX_INITIAL_CONNECTION_WINDOW: u32 = 12517377 + 65535; // 12582912
pub(crate) const FIREFOX_HEADER_TABLE_SIZE: u32 = 65536;

pub(crate) const SAFARI_INITIAL_STREAM_WINDOW: u32 = 2097152;
pub(crate) const SAFARI_INITIAL_CONNECTION_WINDOW: u32 = 10485760;
pub(crate) const SAFARI_MAX_HEADER_LIST_SIZE: u32 = 262144;

/// TLS and HTTP/2 configuration for impersonating a browser version.
#[derive(Clone)]
pub struct BrowserSettings {
    /// Rustls browser emulator configuration
    pub(crate) browser_emulator: Arc<BrowserEmulator>,
    /// HTTP/2 configuration data
    pub http2: Http2Data,
    /// Default headers to include with requests
    pub headers: HeaderMap,
    /// Whether to enable gzip compression
    pub gzip: bool,
    /// Whether to enable brotli compression
    pub brotli: bool,
    /// Whether to enable zstd compression
    pub zstd: bool,
    /// Whether to enable deflate compression
    pub deflate: bool,
}

impl std::fmt::Debug for BrowserSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSettings")
            .field("browser_emulator", &self.browser_emulator)
            .field("http2", &self.http2)
            .field("headers", &self.headers)
            .field("gzip", &self.gzip)
            .field("brotli", &self.brotli)
            .field("zstd", &self.zstd)
            .field("deflate", &self.deflate)
            .finish()
    }
}

/// HTTP/2 connection settings used during browser impersonation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Http2Data {
    /// Initial HTTP/2 stream window size (in bytes)
    ///
    /// Default: 6291456 (6 MiB)
    pub initial_stream_window_size: Option<u32>,
    /// Initial HTTP/2 connection window size (in bytes)
    ///
    /// Default: 15728640 (15 MiB)
    pub initial_connection_window_size: Option<u32>,
    /// Maximum number of concurrent HTTP/2 streams
    ///
    /// Default: 1000
    pub max_concurrent_streams: Option<u32>,
    /// Maximum size of HTTP/2 frames (in bytes)
    ///
    /// Default: 16384
    pub max_frame_size: Option<u32>,
    /// Maximum size of HTTP/2 header list (in bytes)
    ///
    /// Default: 262144 (256 KiB)
    pub max_header_list_size: Option<u32>,
    /// HTTP/2 header table size (in bytes)
    ///
    /// Default: 65536 (64 KiB)
    pub header_table_size: Option<u32>,
    /// Whether HTTP/2 server push is enabled
    ///
    /// Default: false
    pub enable_push: Option<bool>,
    /// Whether to enable Extended CONNECT protocol
    ///
    /// Default: false
    pub enable_connect_protocol: Option<bool>,
    /// Whether to disable RFC 7540 Stream Priorities
    ///
    /// Default: false
    pub no_rfc7540_priorities: Option<bool>,
    /// HTTP/2 SETTINGS frame order
    ///
    /// Controls the order of settings in the SETTINGS frame for fingerprinting.
    pub settings_order: Option<h2::frame::SettingsOrder>,
    /// HTTP/2 pseudo-header order
    ///
    /// Controls the order of pseudo-headers in HEADERS frames for fingerprinting.
    pub headers_pseudo_order: Option<h2::frame::PseudoOrder>,
    /// Whether to include PRIORITY flag in HEADERS frames and its parameters (weight, dep, exclusive)
    ///
    /// `None` = no PRIORITY flag, `Some((w, d, e))` = PRIORITY with those values.
    /// Chrome/Edge/Opera: `Some((255, 0, true))`, Firefox: `Some((41, 0, false))`
    pub headers_priority: Option<(u8, u32, bool)>,
    /// Optional ordering for HTTP/2 regular headers
    ///
    /// When set, headers are encoded in the specified order instead of hash-based order.
    pub headers_order: Option<Vec<http::HeaderName>>,
    /// Optional initial stream ID for HTTP/2 (odd number for client-initiated streams)
    ///
    /// Default: 1 (standard HTTP/2 behavior)
    /// Firefox uses 3 (skips stream 1)
    pub initial_stream_id: Option<u32>,

    /// Extra receive window capacity to add to new locally-initiated streams.
    ///
    /// When set, after creating a new stream, a WINDOW_UPDATE frame will be sent
    /// to increase the stream's receive window by this amount.
    /// This is used for browser fingerprinting (e.g. Firefox adds 12451840
    /// to the first stream's receive window).
    pub initial_stream_window_size_increment: Option<u32>,
}

impl Default for Http2Data {
    fn default() -> Self {
        Self {
            initial_stream_window_size: Some(6291456),
            initial_connection_window_size: Some(15728640),
            max_concurrent_streams: Some(1000),
            max_frame_size: Some(16384),
            max_header_list_size: Some(262144),
            header_table_size: Some(65536),
            enable_push: Some(false),
            enable_connect_protocol: Some(false),
            no_rfc7540_priorities: Some(false),
            settings_order: None,
            headers_pseudo_order: None,
            headers_priority: None,
            headers_order: None,
            initial_stream_id: None,
            initial_stream_window_size_increment: None,
        }
    }
}

/// Browser version to impersonate (each variant maps to a TLS fingerprint and default headers).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Impersonate {
    // Chrome variants
    ChromeV144,
    ChromeV145,
    ChromeV146,
    ChromeV147,
    ChromeV148,
    ChromeV149,
    ChromeV150,
    ChromeV151,
    ChromeV152,
    /// Random Chrome version
    Chrome,
    // Edge variants
    EdgeV144,
    EdgeV145,
    EdgeV146,
    EdgeV147,
    EdgeV148,
    EdgeV149,
    EdgeV150,
    EdgeV151,
    /// Random Edge version
    Edge,
    // Opera variants
    OperaV126,
    OperaV127,
    OperaV128,
    OperaV129,
    OperaV130,
    OperaV131,
    OperaV132,
    OperaV133,
    OperaV134,
    OperaV135,
    /// Random Opera version
    Opera,
    // Safari variants
    SafariV18_5,
    SafariV26,
    SafariV26_3,
    SafariV26_4,
    /// Random Safari version
    Safari,
    // Firefox variants
    FirefoxV140,
    FirefoxV146,
    FirefoxV147,
    FirefoxV148,
    FirefoxV149,
    FirefoxV150,
    FirefoxV151,
    /// Random Firefox version
    Firefox,
    /// Random browser and version
    Random,
}

/// OS platform to mimic when generating browser fingerprints.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ImpersonateOS {
    /// Windows operating system
    #[default]
    Windows,
    /// macOS operating system
    MacOS,
    /// Linux operating system
    Linux,
    /// Android mobile operating system
    Android,
    /// iOS mobile operating system
    IOS,
    /// Random OS selection
    Random,
}

/// Picks a random `Impersonate` variant across all available browser versions.
pub fn random_impersonate() -> Impersonate {
    const IMPERSONATE_VARIANTS: &[Impersonate] = &[
        Impersonate::ChromeV144,
        Impersonate::ChromeV145,
        Impersonate::ChromeV146,
        Impersonate::ChromeV147,
        Impersonate::ChromeV148,
        Impersonate::ChromeV149,
        Impersonate::ChromeV150,
        Impersonate::ChromeV151,
        Impersonate::ChromeV152,
        Impersonate::EdgeV144,
        Impersonate::EdgeV145,
        Impersonate::EdgeV146,
        Impersonate::EdgeV147,
        Impersonate::EdgeV148,
        Impersonate::EdgeV149,
        Impersonate::EdgeV150,
        Impersonate::EdgeV151,
        Impersonate::OperaV126,
        Impersonate::OperaV127,
        Impersonate::OperaV128,
        Impersonate::OperaV129,
        Impersonate::OperaV130,
        Impersonate::OperaV131,
        Impersonate::OperaV132,
        Impersonate::OperaV133,
        Impersonate::OperaV134,
        Impersonate::OperaV135,
        Impersonate::SafariV18_5,
        Impersonate::SafariV26,
        Impersonate::SafariV26_3,
        Impersonate::SafariV26_4,
        Impersonate::FirefoxV140,
        Impersonate::FirefoxV146,
        Impersonate::FirefoxV147,
        Impersonate::FirefoxV148,
        Impersonate::FirefoxV149,
        Impersonate::FirefoxV150,
        Impersonate::FirefoxV151,
    ];

    *IMPERSONATE_VARIANTS.choose(&mut rand::rng()).unwrap()
}

/// Resolves an unnumbered `Impersonate` variant to a random specific version.
pub fn resolve_impersonate(version: Impersonate) -> Impersonate {
    match version {
        Impersonate::Chrome => {
            const CHROME: &[Impersonate] = &[
                Impersonate::ChromeV144,
                Impersonate::ChromeV145,
                Impersonate::ChromeV146,
                Impersonate::ChromeV147,
                Impersonate::ChromeV148,
                Impersonate::ChromeV149,
                Impersonate::ChromeV150,
                Impersonate::ChromeV151,
                Impersonate::ChromeV152,
            ];
            *CHROME.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Edge => {
            const EDGE: &[Impersonate] = &[
                Impersonate::EdgeV144,
                Impersonate::EdgeV145,
                Impersonate::EdgeV146,
                Impersonate::EdgeV147,
                Impersonate::EdgeV148,
                Impersonate::EdgeV149,
                Impersonate::EdgeV150,
                Impersonate::EdgeV151,
            ];
            *EDGE.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Opera => {
            const OPERA: &[Impersonate] = &[
                Impersonate::OperaV126,
                Impersonate::OperaV127,
                Impersonate::OperaV128,
                Impersonate::OperaV129,
                Impersonate::OperaV130,
                Impersonate::OperaV131,
                Impersonate::OperaV132,
                Impersonate::OperaV133,
                Impersonate::OperaV134,
                Impersonate::OperaV135,
            ];
            *OPERA.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Safari => {
            const SAFARI: &[Impersonate] = &[
                Impersonate::SafariV18_5,
                Impersonate::SafariV26,
                Impersonate::SafariV26_3,
                Impersonate::SafariV26_4,
            ];
            *SAFARI.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Firefox => {
            const FIREFOX: &[Impersonate] = &[
                Impersonate::FirefoxV140,
                Impersonate::FirefoxV146,
                Impersonate::FirefoxV147,
                Impersonate::FirefoxV148,
                Impersonate::FirefoxV149,
                Impersonate::FirefoxV150,
                Impersonate::FirefoxV151,
            ];
            *FIREFOX.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Random => random_impersonate(),
        other => other,
    }
}

/// Picks a random OS variant for impersonation.
pub fn random_impersonate_os() -> ImpersonateOS {
    const OS_VARIANTS: &[ImpersonateOS] = &[
        ImpersonateOS::Windows,
        ImpersonateOS::MacOS,
        ImpersonateOS::Linux,
        ImpersonateOS::Android,
        ImpersonateOS::IOS,
    ];

    *OS_VARIANTS.choose(&mut rand::rng()).unwrap()
}

/// Returns the OS-specific sec-ch-ua-platform header value.
pub(crate) fn os_platform(os: ImpersonateOS) -> &'static str {
    let os = if matches!(os, ImpersonateOS::Random) {
        random_impersonate_os()
    } else {
        os
    };
    match os {
        ImpersonateOS::Windows => r#""Windows""#,
        ImpersonateOS::MacOS => r#""macOS""#,
        ImpersonateOS::Linux => r#""Linux""#,
        ImpersonateOS::Android => r#""Android""#,
        ImpersonateOS::IOS => r#""iOS""#,
        ImpersonateOS::Random => unreachable!(),
    }
}

/// Standard Chrome/Edge/Opera header order with sec-ch-ua first.
pub(crate) fn header_order_sec_chua_first() -> &'static Vec<http::HeaderName> {
    static ORDER: OnceLock<Vec<http::HeaderName>> = OnceLock::new();
    ORDER.get_or_init(|| {
        vec![
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
        ]
    })
}

/// Chrome 148-149 / Edge 146-148 header order with sec-ch-ua after sec-fetch-*.
/// (Chrome 150 reverts to sec-ch-ua first with `sec-purpose`;
/// Edge 149+ reverts to sec-ch-ua first.)
pub(crate) fn header_order_upgrade_first_sec_chua_last() -> &'static Vec<http::HeaderName> {
    static ORDER: OnceLock<Vec<http::HeaderName>> = OnceLock::new();
    ORDER.get_or_init(|| {
        vec![
            http::HeaderName::from_static("upgrade-insecure-requests"),
            http::HeaderName::from_static("user-agent"),
            http::HeaderName::from_static("accept"),
            http::HeaderName::from_static("sec-fetch-site"),
            http::HeaderName::from_static("sec-fetch-mode"),
            http::HeaderName::from_static("sec-fetch-user"),
            http::HeaderName::from_static("sec-fetch-dest"),
            http::HeaderName::from_static("sec-ch-ua"),
            http::HeaderName::from_static("sec-ch-ua-mobile"),
            http::HeaderName::from_static("sec-ch-ua-platform"),
            http::HeaderName::from_static("accept-encoding"),
            http::HeaderName::from_static("accept-language"),
            http::HeaderName::from_static("priority"),
        ]
    })
}

/// Opera 131+ header order with cache-control first.
pub(crate) fn header_order_cache_control_first() -> &'static Vec<http::HeaderName> {
    static ORDER: OnceLock<Vec<http::HeaderName>> = OnceLock::new();
    ORDER.get_or_init(|| {
        vec![
            http::HeaderName::from_static("cache-control"),
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
        ]
    })
}

/// Resolves browser settings (TLS, HTTP/2, headers) for the requested version and OS.
pub fn get_browser_settings(
    version: Impersonate,
    os_type: Option<ImpersonateOS>,
) -> BrowserSettings {
    let version = resolve_impersonate(version);
    let os_type = os_type.unwrap_or_default();

    match version {
        Impersonate::ChromeV144
        | Impersonate::ChromeV145
        | Impersonate::ChromeV146
        | Impersonate::ChromeV147
        | Impersonate::ChromeV148
        | Impersonate::ChromeV149
        | Impersonate::ChromeV150
        | Impersonate::ChromeV151
        | Impersonate::ChromeV152 => chrome::build_chrome_settings(version, os_type),
        Impersonate::EdgeV144
        | Impersonate::EdgeV145
        | Impersonate::EdgeV146
        | Impersonate::EdgeV147
        | Impersonate::EdgeV148
        | Impersonate::EdgeV149
        | Impersonate::EdgeV150
        | Impersonate::EdgeV151 => edge::build_edge_settings(version, os_type),
        Impersonate::OperaV126
        | Impersonate::OperaV127
        | Impersonate::OperaV128
        | Impersonate::OperaV129
        | Impersonate::OperaV130
        | Impersonate::OperaV131
        | Impersonate::OperaV132
        | Impersonate::OperaV133
        | Impersonate::OperaV134
        | Impersonate::OperaV135 => opera::build_opera_settings(version, os_type),
        Impersonate::SafariV26
        | Impersonate::SafariV26_3
        | Impersonate::SafariV26_4
        | Impersonate::SafariV18_5 => safari::build_safari_settings(version, os_type),
        Impersonate::FirefoxV140
        | Impersonate::FirefoxV146
        | Impersonate::FirefoxV147
        | Impersonate::FirefoxV148
        | Impersonate::FirefoxV149
        | Impersonate::FirefoxV150
        | Impersonate::FirefoxV151 => firefox::build_firefox_settings(version, os_type),
        _ => unreachable!(),
    }
}

// ---- Offline test helpers (akamai fingerprint) ----

/// Compute the akamai_text fingerprint from HTTP/2 settings.
#[cfg(test)]
pub(crate) fn compute_akamai_text(http2: &Http2Data) -> String {
    use h2::frame::PseudoId;
    use h2::frame::SettingId::{self, *};

    let settings_str = match http2.settings_order {
        Some(ref order) => {
            let mut parts: Vec<String> = Vec::new();
            for id in order {
                let (id_num, value) = match id {
                    HeaderTableSize => (1u16, http2.header_table_size.map(|v| v as u32)),
                    EnablePush => (2, http2.enable_push.map(|v| v as u32)),
                    MaxConcurrentStreams => (3, http2.max_concurrent_streams),
                    InitialWindowSize => (4, http2.initial_stream_window_size),
                    MaxFrameSize => (5, http2.max_frame_size),
                    MaxHeaderListSize => (6, http2.max_header_list_size),
                    SettingId::EnableConnectProtocol => {
                        (8, http2.enable_connect_protocol.map(|v| v as u32))
                    }
                    SettingId::NoRfc7540Priorities => {
                        (9, http2.no_rfc7540_priorities.map(|v| v as u32))
                    }
                };
                if let Some(v) = value {
                    parts.push(format!("{id_num}:{v}"));
                }
            }
            parts.join(";")
        }
        None => String::new(),
    };

    let window_update = http2
        .initial_connection_window_size
        .map(|w| w.saturating_sub(65535))
        .unwrap_or(0);

    let pseudo_str = match http2.headers_pseudo_order {
        Some(ref order) => {
            let mut chars = Vec::new();
            for id in order {
                let ch = match id {
                    PseudoId::Method => Some('m'),
                    PseudoId::Scheme => Some('s'),
                    PseudoId::Authority => Some('a'),
                    PseudoId::Path => Some('p'),
                    // Protocol and Status are CONNECT/response-only; not in akamai
                    _ => None,
                };
                if let Some(c) = ch {
                    chars.push(c);
                }
            }
            chars
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(",")
        }
        None => String::new(),
    };

    format!("{settings_str}|{window_update}|0|{pseudo_str}")
}

/// Compute the MD5 hex hash of an akamai_text string.
#[cfg(test)]
pub(crate) fn compute_akamai_hash(text: &str) -> String {
    format!("{:x}", md5::compute(text.as_bytes()))
}

/// Build the impersonated `ClientConfig`, extract the raw TLS ClientHello
/// bytes (without connecting to any network), parse them with
/// `huginn-net-tls`, and return (ja4_canonical, ja4_ro_raw).
#[cfg(test)]
pub(crate) fn extract_ja4(imp: Impersonate) -> (String, String) {
    extract_ja4_os(imp, None)
}

#[cfg(test)]
pub(crate) fn extract_ja4_os(imp: Impersonate, os: Option<ImpersonateOS>) -> (String, String) {
    use crate::impersonation::{build_impersonate_tls_config, ImpersonationTls};
    use huginn_net_tls::parse_tls_client_hello;

    let settings = get_browser_settings(imp, Some(os.unwrap_or(ImpersonateOS::Linux)));
    let tls = ImpersonationTls {
        certs_verification: true,
        hostname_verification: true,
        tls_certs_only: false,
        identity: None,
        tls_sni: true,
        tls_sslkeylogfile: false,
    };
    let config =
        build_impersonate_tls_config(&settings, &[], &tls).expect("build impersonate tls config");

    let mut conn = rustls::ClientConnection::new(
        std::sync::Arc::new(config),
        rustls::pki_types::ServerName::try_from("localhost").expect("valid server name"),
    )
    .expect("ClientConnection init");

    let mut buf = Vec::new();
    conn.write_tls(&mut buf).expect("write_tls to vec");
    assert!(!buf.is_empty(), "ClientHello must produce bytes");

    let sig = parse_tls_client_hello(&buf).expect("parse ClientHello");

    let canonical = sig.generate_ja4();
    let original = sig.generate_ja4_original();

    let ja4 = match canonical.full {
        huginn_net_tls::fingerprint::ja4::Ja4Fingerprint::Sorted(v) => v,
        _ => panic!("expected Sorted JA4"),
    };
    let ja4_ro = match original.raw {
        huginn_net_tls::fingerprint::ja4::Ja4RawFingerprint::Unsorted(v) => v,
        _ => panic!("expected Unsorted JA4 raw"),
    };

    (ja4, ja4_ro)
}
