//! Holds structs and information to aid in impersonating browsers.
//!
//! This module provides configuration for impersonating various browser versions,
//! including TLS settings, HTTP/2 parameters, and default headers.
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

// Re-export h2 types for HTTP/2 fingerprinting when http2 feature is enabled
#[cfg(feature = "http2")]
pub use h2::frame::{
    PseudoId, PseudoOrder, PseudoOrderBuilder, SettingId, SettingsOrder, SettingsOrderBuilder,
};

pub mod chrome;
pub mod edge;
pub mod firefox;
pub mod opera;
pub mod safari;

/// Browser TLS and HTTP/2 configuration settings.
///
/// This struct contains all the configuration needed to impersonate a specific
/// browser version, including TLS connection settings, HTTP/2 parameters, and
/// default request headers.
#[derive(Clone)]
pub struct BrowserSettings {
    /// Rustls browser emulator configuration
    pub(crate) browser_emulator: BrowserEmulator,
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
            .finish()
    }
}

/// HTTP/2 configuration parameters.
///
/// Controls various HTTP/2 connection settings used during browser impersonation.
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
    /// Whether to include PRIORITY flag in HEADERS frames
    ///
    /// When true, HEADERS frames include priority data (weight=0, stream_dependency=0,
    /// exclusive=1) matching Chrome's HTTP/2 fingerprint behavior.
    /// Whether to include PRIORITY flag in HEADERS frames and its parameters (weight, dep, exclusive)
    ///
    /// None = no PRIORITY flag, Some((w,d,e)) = PRIORITY with those values.
    /// Chrome: Some((0, 0, true)), Firefox: Some((42, 0, false))
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
        }
    }
}

/// Available browser versions for impersonation.
///
/// Each variant represents a specific browser version with its corresponding
/// TLS fingerprint and default headers.
///
/// # Note
///
/// Not all browser versions are equally effective for bypassing detection.
/// Newer versions generally provide better fingerprinting resistance.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg(feature = "impersonate")]
#[allow(missing_docs)]
pub enum Impersonate {
    // Chrome variants
    ChromeV144,
    ChromeV145,
    ChromeV146,
    /// Random Chrome version
    Chrome,
    // Edge variants
    EdgeV144,
    EdgeV145,
    EdgeV146,
    /// Random Edge version
    Edge,
    // Opera variants
    OperaV126,
    OperaV127,
    OperaV128,
    OperaV129,
    /// Random Opera version
    Opera,
    // Safari variants
    SafariV18_5,
    SafariV26,
    SafariV26_3,
    /// Random Safari version
    Safari,
    // Firefox variants
    FirefoxV140,
    FirefoxV146,
    FirefoxV147,
    FirefoxV148,
    /// Random Firefox version
    Firefox,
    /// Random browser and version
    Random,
}

/// Operating system platforms for browser impersonation.
///
/// Specifies the OS platform to mimic when generating browser fingerprints.
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

/// Randomly selects a browser version for impersonation.
///
/// This function can be used when you want to randomize the browser fingerprint
/// across requests to avoid detection based on repeated patterns.
///
/// # Returns
///
/// A randomly selected `Impersonate` variant from all available browser versions.
#[cfg(feature = "impersonate")]
pub fn random_impersonate() -> Impersonate {
    const IMPERSONATE_VARIANTS: &[Impersonate] = &[
        Impersonate::ChromeV144,
        Impersonate::ChromeV145,
        Impersonate::ChromeV146,
        Impersonate::EdgeV144,
        Impersonate::EdgeV145,
        Impersonate::EdgeV146,
        Impersonate::OperaV126,
        Impersonate::OperaV127,
        Impersonate::OperaV128,
        Impersonate::OperaV129,
        Impersonate::SafariV26,
        Impersonate::SafariV26_3,
        Impersonate::SafariV18_5,
        Impersonate::FirefoxV140,
        Impersonate::FirefoxV146,
        Impersonate::FirefoxV147,
        Impersonate::FirefoxV148,
    ];

    *IMPERSONATE_VARIANTS.choose(&mut rand::rng()).unwrap()
}

/// Resolves an unnumbered `Impersonate` variant to a random specific version.
#[cfg(feature = "impersonate")]
pub fn resolve_impersonate(version: Impersonate) -> Impersonate {
    match version {
        Impersonate::Chrome => {
            const CHROME: &[Impersonate] = &[
                Impersonate::ChromeV144,
                Impersonate::ChromeV145,
                Impersonate::ChromeV146,
            ];
            *CHROME.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Edge => {
            const EDGE: &[Impersonate] = &[
                Impersonate::EdgeV144,
                Impersonate::EdgeV145,
                Impersonate::EdgeV146,
            ];
            *EDGE.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Opera => {
            const OPERA: &[Impersonate] = &[
                Impersonate::OperaV126,
                Impersonate::OperaV127,
                Impersonate::OperaV128,
                Impersonate::OperaV129,
            ];
            *OPERA.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Safari => {
            const SAFARI: &[Impersonate] = &[
                Impersonate::SafariV18_5,
                Impersonate::SafariV26,
                Impersonate::SafariV26_3,
            ];
            *SAFARI.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Firefox => {
            const FIREFOX: &[Impersonate] = &[
                Impersonate::FirefoxV140,
                Impersonate::FirefoxV146,
                Impersonate::FirefoxV147,
                Impersonate::FirefoxV148,
            ];
            *FIREFOX.choose(&mut rand::rng()).unwrap()
        }
        Impersonate::Random => random_impersonate(),
        other => other,
    }
}

/// Randomly selects an operating system for impersonation.
///
/// # Returns
///
/// A randomly selected `ImpersonateOS` variant.
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

/// Gets browser settings for impersonation.
///
/// # Arguments
///
/// * `version` - The browser version to impersonate
/// * `os_type` - Optional OS to impersonate
///
/// # Returns
///
/// BrowserSettings with TLS, HTTP/2, and header configuration
pub fn get_browser_settings(
    version: Impersonate,
    os_type: Option<ImpersonateOS>,
) -> BrowserSettings {
    let version = resolve_impersonate(version);
    let os_type = os_type.unwrap_or_default();

    match version {
        Impersonate::ChromeV144 | Impersonate::ChromeV145 | Impersonate::ChromeV146 => {
            chrome::build_chrome_settings(version, os_type)
        }
        Impersonate::EdgeV144 | Impersonate::EdgeV145 | Impersonate::EdgeV146 => {
            edge::build_edge_settings(version, os_type)
        }
        Impersonate::OperaV126
        | Impersonate::OperaV127
        | Impersonate::OperaV128
        | Impersonate::OperaV129 => opera::build_opera_settings(version, os_type),
        Impersonate::SafariV26 | Impersonate::SafariV26_3 | Impersonate::SafariV18_5 => {
            safari::build_safari_settings(version, os_type)
        }
        Impersonate::FirefoxV140
        | Impersonate::FirefoxV146
        | Impersonate::FirefoxV147
        | Impersonate::FirefoxV148 => firefox::build_firefox_settings(version, os_type),
        _ => unreachable!(),
    }
}
