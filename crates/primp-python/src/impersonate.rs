use std::sync::Once;

use anyhow::{anyhow, Result};
pub use primp::imp::{Impersonate, ImpersonateOS};
use rand::prelude::*;

/// Available OS impersonation options.
pub const IMPERSONATEOS_LIST: &[ImpersonateOS] = &[
    ImpersonateOS::Android,
    ImpersonateOS::IOS,
    ImpersonateOS::Linux,
    ImpersonateOS::MacOS,
    ImpersonateOS::Windows,
];

/// One-time flags for warnings
static IMPERSONATE_WARNING: Once = Once::new();
static IMPERSONATE_OS_WARNING: Once = Once::new();

/// Select a random element from a slice.
pub fn get_random_element<T>(slice: &[T]) -> &T {
    slice.choose(&mut rand::rng()).unwrap()
}

/// Parse a string into an Impersonate variant.
pub fn parse_impersonate(s: &str) -> Result<Impersonate> {
    match s {
        // Chrome variants
        "chrome_144" => Ok(Impersonate::ChromeV144),
        "chrome_145" => Ok(Impersonate::ChromeV145),
        "chrome_146" => Ok(Impersonate::ChromeV146),
        "chrome" => Ok(Impersonate::Chrome),
        // Edge variants
        "edge_144" => Ok(Impersonate::EdgeV144),
        "edge_145" => Ok(Impersonate::EdgeV145),
        "edge_146" => Ok(Impersonate::EdgeV146),
        "edge" => Ok(Impersonate::Edge),
        // Opera variants
        "opera_126" => Ok(Impersonate::OperaV126),
        "opera_127" => Ok(Impersonate::OperaV127),
        "opera_128" => Ok(Impersonate::OperaV128),
        "opera_129" => Ok(Impersonate::OperaV129),
        "opera" => Ok(Impersonate::Opera),
        // Safari variants
        "safari_18.5" => Ok(Impersonate::SafariV18_5),
        "safari_26" => Ok(Impersonate::SafariV26),
        "safari_26.3" => Ok(Impersonate::SafariV26_3),
        "safari" => Ok(Impersonate::Safari),
        // Firefox variants
        "firefox_140" => Ok(Impersonate::FirefoxV140),
        "firefox_146" => Ok(Impersonate::FirefoxV146),
        "firefox_147" => Ok(Impersonate::FirefoxV147),
        "firefox_148" => Ok(Impersonate::FirefoxV148),
        "firefox" => Ok(Impersonate::Firefox),
        // Random selection
        "random" => Ok(Impersonate::Random),
        _ => Err(anyhow!("Invalid impersonate: {:?}", s)),
    }
}

/// Parse a string into an ImpersonateOS variant.
pub fn parse_impersonate_os(s: &str) -> Result<ImpersonateOS> {
    match s {
        "android" => Ok(ImpersonateOS::Android),
        "ios" => Ok(ImpersonateOS::IOS),
        "linux" => Ok(ImpersonateOS::Linux),
        "macos" => Ok(ImpersonateOS::MacOS),
        "windows" => Ok(ImpersonateOS::Windows),
        "random" => Ok(*get_random_element(IMPERSONATEOS_LIST)),
        _ => Err(anyhow!("Invalid impersonate_os: {:?}", s)),
    }
}

/// Parse a string into an Impersonate variant with fallback to random value.
/// If the provided value doesn't exist, logs a one-time warning and returns a random valid value.
pub fn parse_impersonate_with_fallback(s: &str) -> Impersonate {
    parse_impersonate(s).unwrap_or_else(|_| {
        IMPERSONATE_WARNING.call_once(|| {
            tracing::warn!("Impersonate '{}' does not exist, using 'random'", s);
        });
        Impersonate::Random
    })
}

/// Parse a string into an ImpersonateOS variant with fallback to random value.
/// If the provided value doesn't exist, logs a one-time warning and returns a random valid value.
pub fn parse_impersonate_os_with_fallback(s: &str) -> ImpersonateOS {
    parse_impersonate_os(s).unwrap_or_else(|_| {
        IMPERSONATE_OS_WARNING.call_once(|| {
            tracing::warn!("Impersonate OS '{}' does not exist, using 'random'", s);
        });
        *get_random_element(IMPERSONATEOS_LIST)
    })
}
