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
///
/// The callers always pass a non-empty constant list (`IMPERSONATEOS_LIST`),
/// so `choose` is effectively always `Some`. We still avoid a hard `unwrap()`
/// on the client-construction path (the process runs under `panic = "abort"`,
/// where an unwrap would kill the host interpreter), falling back to the first
/// element instead.
pub fn get_random_element<T>(slice: &[T]) -> &T {
    slice.choose(&mut rand::rng()).unwrap_or_else(|| &slice[0])
}

/// Parse a string into an Impersonate variant.
pub fn parse_impersonate(s: &str) -> Result<Impersonate> {
    match s {
        // Chrome variants
        "chrome_144" => Ok(Impersonate::ChromeV144),
        "chrome_145" => Ok(Impersonate::ChromeV145),
        "chrome_146" => Ok(Impersonate::ChromeV146),
        "chrome_147" => Ok(Impersonate::ChromeV147),
        "chrome_148" => Ok(Impersonate::ChromeV148),
        "chrome_149" => Ok(Impersonate::ChromeV149),
        "chrome_150" => Ok(Impersonate::ChromeV150),
        "chrome_151" => Ok(Impersonate::ChromeV151),
        "chrome_152" => Ok(Impersonate::ChromeV152),
        "chrome" => Ok(Impersonate::Chrome),
        // Edge variants
        "edge_144" => Ok(Impersonate::EdgeV144),
        "edge_145" => Ok(Impersonate::EdgeV145),
        "edge_146" => Ok(Impersonate::EdgeV146),
        "edge_147" => Ok(Impersonate::EdgeV147),
        "edge_148" => Ok(Impersonate::EdgeV148),
        "edge_149" => Ok(Impersonate::EdgeV149),
        "edge_150" => Ok(Impersonate::EdgeV150),
        "edge_151" => Ok(Impersonate::EdgeV151),
        "edge" => Ok(Impersonate::Edge),
        // Opera variants
        "opera_126" => Ok(Impersonate::OperaV126),
        "opera_127" => Ok(Impersonate::OperaV127),
        "opera_128" => Ok(Impersonate::OperaV128),
        "opera_129" => Ok(Impersonate::OperaV129),
        "opera_130" => Ok(Impersonate::OperaV130),
        "opera_131" => Ok(Impersonate::OperaV131),
        "opera_132" => Ok(Impersonate::OperaV132),
        "opera_133" => Ok(Impersonate::OperaV133),
        "opera_134" => Ok(Impersonate::OperaV134),
        "opera_135" => Ok(Impersonate::OperaV135),
        "opera" => Ok(Impersonate::Opera),
        // Safari variants
        "safari_18.5" => Ok(Impersonate::SafariV18_5),
        "safari_26" => Ok(Impersonate::SafariV26),
        "safari_26.3" => Ok(Impersonate::SafariV26_3),
        "safari_26.4" => Ok(Impersonate::SafariV26_4),
        "safari" => Ok(Impersonate::Safari),
        // Firefox variants
        "firefox_140" => Ok(Impersonate::FirefoxV140),
        "firefox_146" => Ok(Impersonate::FirefoxV146),
        "firefox_147" => Ok(Impersonate::FirefoxV147),
        "firefox_148" => Ok(Impersonate::FirefoxV148),
        "firefox_149" => Ok(Impersonate::FirefoxV149),
        "firefox_150" => Ok(Impersonate::FirefoxV150),
        "firefox_151" => Ok(Impersonate::FirefoxV151),
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

/// Parse an `Impersonate` string, falling back to `Random` (with a one-time
/// warning) if the value is unknown.
pub fn parse_impersonate_with_fallback(s: &str) -> Impersonate {
    parse_impersonate(s).unwrap_or_else(|_| {
        IMPERSONATE_WARNING.call_once(|| {
            tracing::warn!("Impersonate '{}' does not exist, using 'random'", s);
        });
        Impersonate::Random
    })
}

/// Parse an `ImpersonateOS` string, falling back to a random OS (with a
/// one-time warning) if the value is unknown.
pub fn parse_impersonate_os_with_fallback(s: &str) -> ImpersonateOS {
    parse_impersonate_os(s).unwrap_or_else(|_| {
        IMPERSONATE_OS_WARNING.call_once(|| {
            tracing::warn!("Impersonate OS '{}' does not exist, using 'random'", s);
        });
        *get_random_element(IMPERSONATEOS_LIST)
    })
}
