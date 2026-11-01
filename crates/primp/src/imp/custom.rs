//! Custom TLS/HTTP2 fingerprint construction.
//!
//! Enables running with a fingerprint that does not correspond to a hard-coded
//! `Impersonate` variant. Two levels of override are supported:
//!
//! * `ProfileOverrides` — a struct of individually-optional fields (cipher
//!   suites, signature algorithms, header order, HTTP/2 SETTINGS, headers,
//!   user-agent, …) that are applied on top of an already-built
//!   [`BrowserSettings`]. Use this to tweak a compiled-in profile.
//! * [`crate::ClientBuilder::impersonate_settings`] — replace the entire
//!   `BrowserSettings` with a hand-built one.
//!
//! [`PeetFingerprint`] and [`AkamaiFingerprint`] parse the fingerprint strings
//! returned by `https://tls.peet.ws/api/clean` (`peetprint` and `akamai`
//! fields). [`ProfileOverrides::from_peet_api`] builds an override set from a
//! whole `/api/clean` response plus user-supplied headers/UA.

use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(feature = "http2")]
use h2::frame::{PseudoId, PseudoOrder, SettingId, SettingsOrder};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use rustls::{CipherSuite, NamedGroup, SignatureScheme};

use crate::imp::{BrowserSettings, Http2Data};

/// Errors returned from fingerprint parsing helpers.
#[derive(Debug, Clone)]
pub enum FingerprintParseError {
    /// Peetprint string is malformed (wrong number of fields, non-numeric token, …).
    InvalidPeetprint(String),
    /// Akamai HTTP/2 fingerprint string is malformed.
    InvalidAkamai(String),
}

impl std::fmt::Display for FingerprintParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPeetprint(s) => write!(f, "invalid peetprint: {s}"),
            Self::InvalidAkamai(s) => write!(f, "invalid akamai fingerprint: {s}"),
        }
    }
}

impl std::error::Error for FingerprintParseError {}

/// TLS fingerprint parsed from a peetprint string.
///
/// Peetprint layout (pipe-separated fields):
/// `<versions>|<alps>|<groups>|<sig_algs>|<cert_compress>|<psk_ke_modes>|<ciphers>|<extensions>`
///
/// Each field is dash-separated. Numeric tokens are parsed as decimal.
/// The literal `GREASE` is preserved as its own marker and stripped from the
/// numeric lists (rustls injects GREASE itself where appropriate).
#[derive(Debug, Clone, Default)]
pub struct PeetFingerprint {
    /// TLS record versions offered (e.g. `[772, 771]` = TLS 1.3, TLS 1.2).
    /// GREASE markers dropped.
    pub tls_versions: Vec<u16>,
    /// ALPS protocols advertised (e.g. `["h2"]`, `["h2", "http/1.1"]`).
    pub alps_protocols: Vec<String>,
    /// Supported named groups in wire order (GREASE dropped).
    pub named_groups: Vec<u16>,
    /// Signature algorithms in wire order.
    pub signature_algorithms: Vec<u16>,
    /// Certificate compression algorithms.
    pub cert_compress_algorithms: Vec<u16>,
    /// PSK key-exchange modes.
    pub psk_key_exchange_modes: Vec<u16>,
    /// Cipher suites in wire order (GREASE dropped).
    pub cipher_suites: Vec<u16>,
    /// Extensions (peetprint reports them sorted ascending, GREASE dropped).
    pub extensions: Vec<u16>,
}

impl PeetFingerprint {
    /// Parse a peetprint string as returned by tls.peet.ws.
    pub fn parse(s: &str) -> Result<Self, FingerprintParseError> {
        let fields: Vec<&str> = s.split('|').collect();
        if fields.len() != 8 {
            return Err(FingerprintParseError::InvalidPeetprint(format!(
                "expected 8 pipe-separated fields, got {}",
                fields.len()
            )));
        }
        Ok(Self {
            tls_versions: parse_u16_list(fields[0])?,
            alps_protocols: parse_alps(fields[1]),
            named_groups: parse_u16_list(fields[2])?,
            signature_algorithms: parse_u16_list(fields[3])?,
            cert_compress_algorithms: parse_u16_list(fields[4])?,
            psk_key_exchange_modes: parse_u16_list(fields[5])?,
            cipher_suites: parse_u16_list(fields[6])?,
            extensions: parse_u16_list(fields[7])?,
        })
    }
}

/// Parsed akamai HTTP/2 fingerprint.
///
/// Akamai layout: `<settings>|<window_update>|<priority_streams>|<pseudo_headers>`
/// e.g. `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`.
#[derive(Debug, Clone, Default)]
pub struct AkamaiFingerprint {
    /// HTTP/2 SETTINGS in the order they appeared, `(id, value)`.
    pub settings: Vec<(u16, u32)>,
    /// Connection-level WINDOW_UPDATE increment (if any).
    pub window_update: Option<u32>,
    /// Pseudo-header order: 'm' (`:method`), 'a' (`:authority`),
    /// 's' (`:scheme`), 'p' (`:path`).
    pub pseudo_header_order: Vec<char>,
}

impl AkamaiFingerprint {
    /// Parse an akamai fingerprint string.
    pub fn parse(s: &str) -> Result<Self, FingerprintParseError> {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() != 4 {
            return Err(FingerprintParseError::InvalidAkamai(format!(
                "expected 4 pipe-separated fields, got {}",
                parts.len()
            )));
        }

        let mut settings = Vec::new();
        for tok in parts[0].split(';').filter(|s| !s.is_empty()) {
            let mut it = tok.splitn(2, ':');
            let (id, value) = (it.next(), it.next());
            let id = id
                .and_then(|s| s.parse::<u16>().ok())
                .ok_or_else(|| FingerprintParseError::InvalidAkamai(tok.to_string()))?;
            let value = value
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| FingerprintParseError::InvalidAkamai(tok.to_string()))?;
            settings.push((id, value));
        }

        let window_update = if parts[1].is_empty() {
            None
        } else {
            Some(
                parts[1]
                    .parse::<u32>()
                    .map_err(|_| FingerprintParseError::InvalidAkamai(parts[1].to_string()))?,
            )
        };

        let pseudo_header_order = parts[3]
            .split(',')
            .filter_map(|t| t.trim().chars().next())
            .collect();

        Ok(Self {
            settings,
            window_update,
            pseudo_header_order,
        })
    }
}

/// Selectively override fields of a base [`BrowserSettings`].
///
/// Every field is optional; `None` leaves the base value unchanged. The
/// override is applied by [`Self::apply_to`] after a base profile has been
/// resolved from [`crate::imp::Impersonate`].
#[derive(Debug, Clone, Default)]
pub struct ProfileOverrides {
    /// Replace the `User-Agent` header.
    pub user_agent: Option<String>,
    /// Replace the `sec-ch-ua` header.
    pub sec_ch_ua: Option<String>,
    /// Replace the `sec-ch-ua-platform` header.
    pub sec_ch_ua_platform: Option<String>,
    /// Replace the `sec-ch-ua-mobile` header (`"?0"` / `"?1"`).
    pub sec_ch_ua_mobile: Option<String>,
    /// Additional headers inserted/replaced verbatim.
    pub extra_headers: BTreeMap<String, String>,
    /// Explicit HTTP header order (HTTP/2). Overrides the profile's order.
    pub headers_order: Option<Vec<String>>,
    /// TLS cipher suites (numeric IDs, wire order).
    pub cipher_suites: Option<Vec<u16>>,
    /// TLS signature algorithms (numeric IDs, wire order).
    pub signature_algorithms: Option<Vec<u16>>,
    /// TLS named groups (numeric IDs, wire order; GREASE will be injected as
    /// controlled by the emulator).
    pub named_groups: Option<Vec<u16>>,
    /// Extension order seed for the rustls browser emulator.
    pub extension_order_seed: Option<u16>,
    /// HTTP/2 SETTINGS in `(id, value)` wire order.
    pub http2_settings: Option<Vec<(u16, u32)>>,
    /// HTTP/2 pseudo-header order (`'m','a','s','p'`).
    pub http2_pseudo_order: Option<Vec<char>>,
    /// HEADERS PRIORITY tuple `(weight, dep, exclusive)`, or `None` to drop.
    pub http2_headers_priority: Option<Option<(u8, u32, bool)>>,
    /// Initial stream WINDOW_UPDATE increment (WINDOW_UPDATE frame after SETTINGS).
    pub http2_initial_connection_window_size: Option<u32>,
    /// Initial per-stream window size.
    pub http2_initial_stream_window_size: Option<u32>,
    /// Header table size (SETTINGS_HEADER_TABLE_SIZE).
    pub http2_header_table_size: Option<u32>,
    /// Max header list size (SETTINGS_MAX_HEADER_LIST_SIZE).
    pub http2_max_header_list_size: Option<u32>,
}

impl ProfileOverrides {
    /// Build a set of overrides from a `tls.peet.ws /api/clean` response.
    ///
    /// Only the TLS/HTTP2 fingerprint fields are populated. HTTP-level things
    /// like `user_agent`, `sec_ch_ua`, `extra_headers`, and `headers_order`
    /// must be supplied separately — the peet.ws clean endpoint does not
    /// return them.
    pub fn from_peet_api(peetprint: &str, akamai: &str) -> Result<Self, FingerprintParseError> {
        let peet = PeetFingerprint::parse(peetprint)?;
        let ak = AkamaiFingerprint::parse(akamai)?;

        Ok(Self {
            cipher_suites: (!peet.cipher_suites.is_empty()).then_some(peet.cipher_suites),
            signature_algorithms: (!peet.signature_algorithms.is_empty())
                .then_some(peet.signature_algorithms),
            named_groups: (!peet.named_groups.is_empty()).then_some(peet.named_groups),
            http2_settings: (!ak.settings.is_empty()).then_some(ak.settings),
            http2_pseudo_order: (!ak.pseudo_header_order.is_empty())
                .then_some(ak.pseudo_header_order),
            http2_initial_connection_window_size: ak.window_update,
            ..Self::default()
        })
    }

    /// Apply these overrides on top of an existing [`BrowserSettings`].
    pub fn apply_to(self, settings: &mut BrowserSettings) {
        let Self {
            user_agent,
            sec_ch_ua,
            sec_ch_ua_platform,
            sec_ch_ua_mobile,
            extra_headers,
            headers_order,
            cipher_suites,
            signature_algorithms,
            named_groups,
            extension_order_seed,
            http2_settings,
            http2_pseudo_order,
            http2_headers_priority,
            http2_initial_connection_window_size,
            http2_initial_stream_window_size,
            http2_header_table_size,
            http2_max_header_list_size,
        } = self;

        if let Some(v) = user_agent {
            insert_header(&mut settings.headers, "user-agent", &v);
        }
        if let Some(v) = sec_ch_ua {
            insert_header(&mut settings.headers, "sec-ch-ua", &v);
        }
        if let Some(v) = sec_ch_ua_platform {
            insert_header(&mut settings.headers, "sec-ch-ua-platform", &v);
        }
        if let Some(v) = sec_ch_ua_mobile {
            insert_header(&mut settings.headers, "sec-ch-ua-mobile", &v);
        }
        for (k, v) in extra_headers {
            insert_header(&mut settings.headers, &k, &v);
        }

        if cipher_suites.is_some()
            || signature_algorithms.is_some()
            || named_groups.is_some()
            || extension_order_seed.is_some()
        {
            let emu_ref = Arc::make_mut(&mut settings.browser_emulator);
            if let Some(cs) = cipher_suites {
                emu_ref.cipher_suites = Some(cs.into_iter().map(CipherSuite::from).collect());
            }
            if let Some(sa) = signature_algorithms {
                emu_ref.signature_algorithms =
                    Some(sa.into_iter().map(SignatureScheme::from).collect());
            }
            if let Some(ng) = named_groups {
                emu_ref.named_groups = Some(ng.into_iter().map(NamedGroup::from).collect());
            }
            if let Some(seed) = extension_order_seed {
                emu_ref.extension_order_seed = Some(seed);
            }
        }

        apply_http2_overrides(
            &mut settings.http2,
            headers_order,
            http2_settings,
            http2_pseudo_order,
            http2_headers_priority,
            http2_initial_connection_window_size,
            http2_initial_stream_window_size,
            http2_header_table_size,
            http2_max_header_list_size,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_http2_overrides(
    http2: &mut Http2Data,
    headers_order: Option<Vec<String>>,
    settings: Option<Vec<(u16, u32)>>,
    pseudo_order: Option<Vec<char>>,
    headers_priority: Option<Option<(u8, u32, bool)>>,
    initial_connection_window_size: Option<u32>,
    initial_stream_window_size: Option<u32>,
    header_table_size: Option<u32>,
    max_header_list_size: Option<u32>,
) {
    if let Some(order) = headers_order {
        let names: Result<Vec<HeaderName>, _> = order
            .iter()
            .map(|s| HeaderName::try_from(s.as_str()))
            .collect();
        if let Ok(names) = names {
            http2.headers_order = Some(names);
        }
    }
    if let Some(icw) = initial_connection_window_size {
        http2.initial_connection_window_size = Some(icw);
    }
    if let Some(isw) = initial_stream_window_size {
        http2.initial_stream_window_size = Some(isw);
    }
    if let Some(hts) = header_table_size {
        http2.header_table_size = Some(hts);
    }
    if let Some(mhls) = max_header_list_size {
        http2.max_header_list_size = Some(mhls);
    }
    if let Some(hp) = headers_priority {
        http2.headers_priority = hp;
    }

    #[cfg(feature = "http2")]
    {
        if let Some(sv) = settings {
            let mut b = SettingsOrder::builder();
            for (id, value) in &sv {
                match *id {
                    1 => {
                        http2.header_table_size = Some(*value);
                        b = b.push(SettingId::HeaderTableSize);
                    }
                    2 => {
                        http2.enable_push = Some(*value != 0);
                        b = b.push(SettingId::EnablePush);
                    }
                    3 => {
                        http2.max_concurrent_streams = Some(*value);
                        b = b.push(SettingId::MaxConcurrentStreams);
                    }
                    4 => {
                        http2.initial_stream_window_size = Some(*value);
                        b = b.push(SettingId::InitialWindowSize);
                    }
                    5 => {
                        http2.max_frame_size = Some(*value);
                        b = b.push(SettingId::MaxFrameSize);
                    }
                    6 => {
                        http2.max_header_list_size = Some(*value);
                        b = b.push(SettingId::MaxHeaderListSize);
                    }
                    8 => {
                        http2.enable_connect_protocol = Some(*value != 0);
                        b = b.push(SettingId::EnableConnectProtocol);
                    }
                    9 => {
                        http2.no_rfc7540_priorities = Some(*value != 0);
                        b = b.push(SettingId::NoRfc7540Priorities);
                    }
                    _ => { /* unknown SETTINGS id: ignore for order but still preserve any prior */
                    }
                }
            }
            http2.settings_order = Some(b.build_without_extend());
        }

        if let Some(po) = pseudo_order {
            let mut b = PseudoOrder::builder();
            for c in po {
                match c {
                    'm' | 'M' => b = b.push(PseudoId::Method),
                    'a' | 'A' => b = b.push(PseudoId::Authority),
                    's' | 'S' => b = b.push(PseudoId::Scheme),
                    'p' | 'P' => b = b.push(PseudoId::Path),
                    _ => {}
                }
            }
            http2.headers_pseudo_order = Some(b.build());
        }
    }

    // Silence unused warnings when http2 feature is off.
    #[cfg(not(feature = "http2"))]
    {
        let _ = (settings, pseudo_order);
    }
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(n), Ok(v)) = (HeaderName::try_from(name), HeaderValue::try_from(value)) {
        headers.insert(n, v);
    }
}

fn parse_u16_list(s: &str) -> Result<Vec<u16>, FingerprintParseError> {
    let mut out = Vec::new();
    for tok in s.split('-').filter(|t| !t.is_empty()) {
        if tok.eq_ignore_ascii_case("GREASE") {
            continue; // rustls injects GREASE where required
        }
        // Accept plain decimal, "0x" hex, or decimal-like tokens.
        let parsed = if let Some(hex) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) {
            u16::from_str_radix(hex, 16).ok()
        } else {
            tok.parse::<u16>().ok()
        };
        let v = parsed.ok_or_else(|| {
            FingerprintParseError::InvalidPeetprint(format!("non-numeric token: {tok:?}"))
        })?;
        out.push(v);
    }
    Ok(out)
}

fn parse_alps(s: &str) -> Vec<String> {
    s.split('-')
        .filter(|t| !t.is_empty())
        .map(|t| match t {
            "2" => "h2".to_string(),
            "1.1" => "http/1.1".to_string(),
            other => other.to_string(),
        })
        .collect()
}

/// Build a "Chrome-like" base [`BrowserSettings`] to which overrides can be applied.
///
/// This is the recommended base for arbitrary Chromium-family fingerprints —
/// the rustls extension order is Chrome's, HTTP/2 defaults are Chrome's, and
/// compressions default to gzip+brotli+zstd+deflate.
pub fn chrome_like_base(os: crate::imp::ImpersonateOS) -> BrowserSettings {
    crate::imp::chrome::build_chrome_settings(crate::imp::Impersonate::ChromeV150, os)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PEETPRINT: &str = "GREASE-772-771|2-1.1|GREASE-4588-29-23-24|2308-2309-2310-1027-2052-1025-1283-2053-1281-2054-1537|1|2|GREASE-4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53|0-10-11-13-16-17613-18-23-27-35-43-45-5-51-65037-65281-GREASE-GREASE";
    const SAMPLE_AKAMAI: &str = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p";

    #[test]
    fn parses_peetprint() {
        let p = PeetFingerprint::parse(SAMPLE_PEETPRINT).unwrap();
        assert_eq!(p.tls_versions, vec![772, 771]);
        assert_eq!(p.alps_protocols, vec!["h2", "http/1.1"]);
        assert_eq!(p.named_groups, vec![4588, 29, 23, 24]);
        assert_eq!(
            p.signature_algorithms,
            vec![2308, 2309, 2310, 1027, 2052, 1025, 1283, 2053, 1281, 2054, 1537]
        );
        assert_eq!(p.cert_compress_algorithms, vec![1]);
        assert_eq!(p.psk_key_exchange_modes, vec![2]);
        assert_eq!(p.cipher_suites.len(), 15); // GREASE stripped
        assert_eq!(p.extensions.len(), 16);
    }

    #[test]
    fn parses_akamai() {
        let a = AkamaiFingerprint::parse(SAMPLE_AKAMAI).unwrap();
        assert_eq!(
            a.settings,
            vec![(1, 65536), (2, 0), (4, 6291456), (6, 262144)]
        );
        assert_eq!(a.window_update, Some(15663105));
        assert_eq!(a.pseudo_header_order, vec!['m', 'a', 's', 'p']);
    }

    #[test]
    fn overrides_from_peet_api_populates_tls_and_h2() {
        let ov = ProfileOverrides::from_peet_api(SAMPLE_PEETPRINT, SAMPLE_AKAMAI).unwrap();
        assert_eq!(ov.cipher_suites.as_ref().unwrap().len(), 15);
        assert_eq!(ov.signature_algorithms.as_ref().unwrap().len(), 11);
        assert_eq!(ov.named_groups.as_ref().unwrap().len(), 4);
        assert_eq!(ov.http2_settings.as_ref().unwrap().len(), 4);
        assert_eq!(ov.http2_initial_connection_window_size, Some(15663105));
        assert_eq!(
            ov.http2_pseudo_order.as_ref().unwrap(),
            &vec!['m', 'a', 's', 'p']
        );
    }
}
