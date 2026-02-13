//! Browser emulator types for TLS fingerprinting
//!
//! This module provides types for emulating browser TLS fingerprints.

use alloc::vec::Vec;

use crate::enums::{CipherSuite, SignatureScheme};
use crate::msgs::enums::NamedGroup;

/// Represents a browser type for TLS fingerprinting
#[cfg(feature = "impersonate")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserType {
    /// Google Chrome
    Chrome,
    /// Mozilla Firefox
    Firefox,
    /// Microsoft Edge
    Edge,
    /// Apple Safari
    Safari,
    /// Opera
    Opera,
}

/// Represents a browser version (major.minor.patch)
#[cfg(feature = "impersonate")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BrowserVersion {
    /// Major version number
    pub major: u16,
    /// Minor version number
    pub minor: u16,
    /// Patch version number
    pub patch: u16,
}

#[cfg(feature = "impersonate")]
impl BrowserVersion {
    /// Create a new browser version
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a version string like "120.0.6099.130"
    pub fn parse(version: &str) -> Option<Self> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        let major = parts.first()?.parse().ok()?;
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        Some(Self::new(major, minor, patch))
    }
}

impl core::fmt::Display for BrowserVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Browser emulator configuration for TLS fingerprinting
#[cfg(feature = "impersonate")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEmulator {
    /// The browser type to emulate
    pub browser_type: BrowserType,
    /// The browser version to emulate
    pub version: BrowserVersion,
    /// Optional seed for deterministic extension ordering
    pub extension_order_seed: Option<u16>,
    /// Optional cipher suites to use (instead of provider defaults)
    pub cipher_suites: Option<Vec<CipherSuite>>,
    /// Optional signature algorithms to use (instead of verifier defaults)
    pub signature_algorithms: Option<Vec<SignatureScheme>>,
    /// Optional named groups to use (instead of provider defaults)
    pub named_groups: Option<Vec<NamedGroup>>,
    /// Optional OS type for platform-specific fingerprinting
    pub os_type: Option<BrowserEmulatorOS>,
}

/// OS types for browser emulation
#[cfg(feature = "impersonate")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserEmulatorOS {
    /// macOS
    MacOS,
    /// iOS
    IOS,
    /// Windows
    Windows,
    /// Linux
    Linux,
    /// Android
    Android,
}

#[cfg(feature = "impersonate")]
impl BrowserEmulator {
    /// Create a new browser emulator
    pub const fn new(browser_type: BrowserType, version: BrowserVersion) -> Self {
        Self {
            browser_type,
            version,
            extension_order_seed: None,
            cipher_suites: None,
            signature_algorithms: None,
            named_groups: None,
            os_type: None,
        }
    }

    /// Set the OS type for platform-specific fingerprinting
    pub fn with_os_type(mut self, os: BrowserEmulatorOS) -> Self {
        self.os_type = Some(os);
        self
    }

    /// Create a new Chrome emulator
    pub fn chrome(version: &str) -> Option<Self> {
        Some(Self::new(BrowserType::Chrome, BrowserVersion::parse(version)?))
    }

    /// Create a new Firefox emulator
    pub fn firefox(version: &str) -> Option<Self> {
        Some(Self::new(BrowserType::Firefox, BrowserVersion::parse(version)?))
    }

    /// Create a new Edge emulator
    pub fn edge(version: &str) -> Option<Self> {
        Some(Self::new(BrowserType::Edge, BrowserVersion::parse(version)?))
    }

    /// Create a new Safari emulator
    pub fn safari(version: &str) -> Option<Self> {
        Some(Self::new(BrowserType::Safari, BrowserVersion::parse(version)?))
    }

    /// Create a new Opera emulator
    pub fn opera(version: &str) -> Option<Self> {
        Some(Self::new(BrowserType::Opera, BrowserVersion::parse(version)?))
    }

    /// Set the extension order seed
    pub fn with_extension_order_seed(mut self, seed: u16) -> Self {
        self.extension_order_seed = Some(seed);
        self
    }

    /// Check if this is a Chrome-based browser (Chrome, Edge, Opera)
    pub fn is_chrome_based(&self) -> bool {
        matches!(
            self.browser_type,
            BrowserType::Chrome | BrowserType::Edge | BrowserType::Opera
        )
    }

    /// Check if this is Firefox
    pub fn is_firefox(&self) -> bool {
        matches!(self.browser_type, BrowserType::Firefox)
    }

    /// Check if this is Safari
    pub fn is_safari(&self) -> bool {
        matches!(self.browser_type, BrowserType::Safari)
    }
}

impl Default for BrowserEmulator {
    fn default() -> Self {
        Self {
            browser_type: BrowserType::Chrome,
            version: BrowserVersion::new(120, 0, 0),
            extension_order_seed: None,
            cipher_suites: None,
            signature_algorithms: None,
            named_groups: None,
            os_type: None,
        }
    }
}
