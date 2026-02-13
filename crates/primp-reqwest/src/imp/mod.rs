//! Holds structs and information to aid in impersonating browsers.
//!
//! This module provides configuration for impersonating various browser versions,
//! including TLS settings, HTTP/2 parameters, and default headers.
//!
//! # Example
//!
//! ```rust
//! use reqwest::{Client, Impersonate};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::builder()
//!         .impersonate(Impersonate::ChromeV144)
//!         .build()?;
//!
//!     //let response = client.get("https://example.com").send().await?;
//!
//!     Ok(())
//! }
//! ```

use http::HeaderMap;
use rand::prelude::*;
use rustls::client::BrowserEmulator;
use rustls::CipherSuite;
use rustls::SignatureScheme;
use rustls::NamedGroup;
use std::sync::Arc;

/// Emulation profile containing cipher suites and signature algorithms for browser impersonation.
///
/// This struct holds the TLS fingerprinting parameters needed to emulate various browsers.
/// Uses `Arc<Vec<T>>` for cheap reference counting instead of deep cloning.
#[derive(Clone, Debug)]
pub struct EmulationProfile {
    /// Cipher suites to use (Arc for cheap cloning)
    pub cipher_suites: Arc<Vec<CipherSuite>>,
    /// Signature algorithms to use (Arc for cheap cloning)
    pub signature_algorithms: Arc<Vec<SignatureScheme>>,
    /// Named groups to use (Arc for cheap cloning)
    pub named_groups: Arc<Vec<NamedGroup>>,
    /// Extension order seed
    pub extension_order_seed: u16,
}

impl EmulationProfile {
    /// Chrome emulation profile (cached)
    pub fn chrome() -> Self {
        use std::sync::OnceLock;
        static CHROME_PROFILE: OnceLock<EmulationProfile> = OnceLock::new();
        CHROME_PROFILE
            .get_or_init(|| EmulationProfile {
                cipher_suites: Arc::new(rustls::crypto::emulation::cipher_suites::CHROME.to_vec()),
                signature_algorithms: Arc::new(rustls::crypto::emulation::signature_algorithms::CHROME.to_vec()),
                named_groups: Arc::new(rustls::crypto::emulation::named_groups::CHROME.to_vec()),
                extension_order_seed: rustls::crypto::emulation::extension_order::CHROME,
            })
            .clone()
    }

    /// Edge emulation profile (same as Chrome, cached)
    pub fn edge() -> Self {
        use std::sync::OnceLock;
        static EDGE_PROFILE: OnceLock<EmulationProfile> = OnceLock::new();
        EDGE_PROFILE
            .get_or_init(|| EmulationProfile {
                cipher_suites: Arc::new(rustls::crypto::emulation::cipher_suites::EDGE.to_vec()),
                signature_algorithms: Arc::new(rustls::crypto::emulation::signature_algorithms::EDGE.to_vec()),
                named_groups: Arc::new(rustls::crypto::emulation::named_groups::EDGE.to_vec()),
                extension_order_seed: rustls::crypto::emulation::extension_order::EDGE,
            })
            .clone()
    }

    /// Safari emulation profile (cached)
    pub fn safari() -> Self {
        use std::sync::OnceLock;
        static SAFARI_PROFILE: OnceLock<EmulationProfile> = OnceLock::new();
        SAFARI_PROFILE
            .get_or_init(|| EmulationProfile {
                cipher_suites: Arc::new(rustls::crypto::emulation::cipher_suites::SAFARI.to_vec()),
                signature_algorithms: Arc::new(rustls::crypto::emulation::signature_algorithms::SAFARI.to_vec()),
                named_groups: Arc::new(rustls::crypto::emulation::named_groups::SAFARI.to_vec()),
                extension_order_seed: rustls::crypto::emulation::extension_order::SAFARI,
            })
            .clone()
    }

    /// Firefox emulation profile (cached)
    pub fn firefox() -> Self {
        use std::sync::OnceLock;
        static FIREFOX_PROFILE: OnceLock<EmulationProfile> = OnceLock::new();
        FIREFOX_PROFILE
            .get_or_init(|| EmulationProfile {
                cipher_suites: Arc::new(rustls::crypto::emulation::cipher_suites::FIREFOX.to_vec()),
                signature_algorithms: Arc::new(rustls::crypto::emulation::signature_algorithms::FIREFOX.to_vec()),
                named_groups: Arc::new(rustls::crypto::emulation::named_groups::FIREFOX.to_vec()),
                extension_order_seed: rustls::crypto::emulation::extension_order::FIREFOX,
            })
            .clone()
    }

    /// Safari 18.5 emulation profile (cached)
    pub fn safari_v18_5() -> Self {
        use std::sync::OnceLock;
        static SAFARI_18_5_PROFILE: OnceLock<EmulationProfile> = OnceLock::new();
        SAFARI_18_5_PROFILE
            .get_or_init(|| EmulationProfile {
                cipher_suites: Arc::new(rustls::crypto::emulation::cipher_suites::SAFARI.to_vec()),
                signature_algorithms: Arc::new(rustls::crypto::emulation::signature_algorithms::SAFARI.to_vec()),
                named_groups: Arc::new(rustls::crypto::emulation::named_groups::SAFARI.to_vec()),
                extension_order_seed: rustls::crypto::emulation::extension_order::SAFARI_18_5,
            })
            .clone()
    }

    /// Safari 26 emulation profile (cached)
    pub fn safari_v26() -> Self {
        use std::sync::OnceLock;
        static SAFARI_26_PROFILE: OnceLock<EmulationProfile> = OnceLock::new();
        SAFARI_26_PROFILE
            .get_or_init(|| EmulationProfile {
                cipher_suites: Arc::new(rustls::crypto::emulation::cipher_suites::SAFARI.to_vec()),
                signature_algorithms: Arc::new(rustls::crypto::emulation::signature_algorithms::SAFARI.to_vec()),
                named_groups: Arc::new(rustls::crypto::emulation::named_groups::SAFARI.to_vec()),
                extension_order_seed: rustls::crypto::emulation::extension_order::SAFARI_26,
            })
            .clone()
    }
}

// Re-export h2 types for HTTP/2 fingerprinting when http2 feature is enabled
#[cfg(feature = "http2")]
pub use h2::frame::{SettingsOrder, SettingsOrderBuilder, SettingId, PseudoOrder, PseudoOrderBuilder, PseudoId, StreamDependency, Priorities, PrioritiesBuilder};

#[cfg(feature = "impersonate")]
pub use chrome::configure_impersonate;

#[cfg(feature = "impersonate")]
mod chrome;

#[cfg(feature = "impersonate")]
pub mod edge;

#[cfg(feature = "impersonate")]
pub mod opera;

#[cfg(feature = "impersonate")]
pub mod safari;

#[cfg(feature = "impersonate")]
pub mod firefox;

pub mod cert_compressor;

/// Browser TLS and HTTP/2 configuration settings.
///
/// This struct contains all the configuration needed to impersonate a specific
/// browser version, including TLS connection settings, HTTP/2 parameters, and
/// default request headers.
#[derive(Clone)]
pub struct BrowserSettings {
    /// Rustls browser emulator configuration
    #[doc(hidden)]
    pub browser_emulator: BrowserEmulator,
    /// Emulation profile for cipher suites and signature algorithms
    #[doc(hidden)]
    pub emulation_profile: EmulationProfile,
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
            .field("emulation_profile", &"<EmulationProfile>")
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
    /// HTTP/2 header stream dependency
    ///
    /// Controls the stream dependency information for HEADERS frames.
    pub headers_stream_dependency: Option<h2::frame::StreamDependency>,
    /// HTTP/2 PRIORITY frames
    ///
    /// Controls PRIORITY frames sent during connection setup.
    pub priorities: Option<h2::frame::Priorities>,
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
            headers_stream_dependency: None,
            priorities: None,
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
    // Edge variants
    EdgeV144,
    EdgeV145,
    // Opera variants
    OperaV126,
    OperaV127,
    // Safari variants
    SafariV18_5,
    SafariV26,
    // Firefox variants
    FirefoxV140,
    FirefoxV146,
}

/// Operating system platforms for browser impersonation.
///
/// Specifies the OS platform to mimic when generating browser fingerprints.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImpersonateOS {
    /// Windows operating system
    Windows,
    /// macOS operating system
    MacOS,
    /// Linux operating system
    Linux,
    /// Android mobile operating system
    Android,
    /// iOS mobile operating system
    IOS,
}

impl Default for ImpersonateOS {
    fn default() -> Self {
        ImpersonateOS::Windows
    }
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
        Impersonate::EdgeV144,
        Impersonate::EdgeV145,
        Impersonate::OperaV126,
        Impersonate::OperaV127,
        Impersonate::SafariV26,
        Impersonate::SafariV18_5,
        Impersonate::FirefoxV140,
        Impersonate::FirefoxV146,
    ];

    *IMPERSONATE_VARIANTS.choose(&mut rand::rng()).unwrap()
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

/// Resolves impersonation configuration with optional random fallback.
///
/// If either `chrome` or `os_type` is `None`, this function will randomly
/// select a value for the missing parameter.
///
/// # Arguments
///
/// * `chrome` - Optional Chrome version to impersonate
/// * `os_type` - Optional OS platform to mimic
///
/// # Returns
///
/// A tuple of `(Impersonate, ImpersonateOS)` with resolved values.
#[inline]
pub(crate) fn get_random_impersonate_config(
    chrome: Option<Impersonate>,
    os_type: Option<ImpersonateOS>,
) -> (Impersonate, ImpersonateOS) {
    let final_chrome = chrome.unwrap_or_else(random_impersonate);
    let final_os = os_type.unwrap_or_else(random_impersonate_os);

    (final_chrome, final_os)
}
