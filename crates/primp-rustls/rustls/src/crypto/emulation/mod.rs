//! Crypto emulation profiles for browser fingerprinting
//!
//! This module provides pre-defined cipher suite lists and signature algorithm lists
//! for emulating various browsers' TLS fingerprints.

use crate::msgs::enums::NamedGroup;
use crate::CipherSuite;
use crate::SignatureScheme;

/// Pre-defined cipher suite lists for browser emulation
#[cfg(feature = "impersonate")]
pub mod cipher_suites {
    use super::CipherSuite;

    /// Chrome's default cipher suite list (TLS 1.2 and 1.3)
    /// 16 cipher suites total: 1 GREASE + 3 TLS 1.3 + 12 TLS 1.2 (matching Chrome 151)
    pub const CHROME: &[CipherSuite] = &[
        // GREASE cipher suite (for Chrome fingerprinting)
        CipherSuite::TLS_RESERVED_GREASE,
        // TLS 1.3 suites
        CipherSuite::TLS13_AES_128_GCM_SHA256,
        CipherSuite::TLS13_AES_256_GCM_SHA384,
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
        // TLS 1.2 suites
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
        CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA,
        CipherSuite::TLS_RSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_RSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_RSA_WITH_AES_128_CBC_SHA,
        CipherSuite::TLS_RSA_WITH_AES_256_CBC_SHA,
    ];

    /// Chrome's TLS 1.3 cipher suite list
    pub const CHROME_TLS13: &[CipherSuite] = &[
        CipherSuite::TLS13_AES_128_GCM_SHA256,
        CipherSuite::TLS13_AES_256_GCM_SHA384,
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
    ];

    /// Chrome's TLS 1.2 cipher suite list
    /// 12 cipher suites (matching Chrome 151)
    pub const CHROME_TLS12: &[CipherSuite] = &[
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
        CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA,
        CipherSuite::TLS_RSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_RSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_RSA_WITH_AES_128_CBC_SHA,
        CipherSuite::TLS_RSA_WITH_AES_256_CBC_SHA,
    ];

    /// Safari's default cipher suite list
    /// 20 cipher suites
    pub const SAFARI: &[CipherSuite] = &[
        // TLS 1.3 suites (3) - iOS order: AES_128_GCM -> AES_256_GCM -> CHACHA20
        CipherSuite::TLS13_AES_128_GCM_SHA256,
        CipherSuite::TLS13_AES_256_GCM_SHA384,
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
        // TLS 1.2 ECDSA suites (3)
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
        // TLS 1.2 RSA suites (3)
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        // CBC mode - ECDSA (2)
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA,
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA,
        // CBC mode - RSA (2)
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA,
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA,
        // RSA key exchange suites (4)
        CipherSuite::TLS_RSA_WITH_AES_256_GCM_SHA384,
        CipherSuite::TLS_RSA_WITH_AES_128_GCM_SHA256,
        CipherSuite::TLS_RSA_WITH_AES_256_CBC_SHA,
        CipherSuite::TLS_RSA_WITH_AES_128_CBC_SHA,
        // 3DES suites (3)
        CipherSuite::TLS_ECDHE_ECDSA_WITH_3DES_EDE_CBC_SHA,
        CipherSuite::TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA,
        CipherSuite::TLS_RSA_WITH_3DES_EDE_CBC_SHA,
    ];

    /// Firefox's cipher suite list for Firefox 140/146
    /// Real Firefox 140 order: TLS 1.3 first, then ECDHE (interleaved ECDSA/RSA), then RSA
    /// JA4_R: t13d1717h2_002f,0035,009c,009d,1301,1302,1303,c009,c00a,c013,c014,c02b,c02c,c02f,c030,cca8,cca9
    pub const FIREFOX: &[CipherSuite] = &[
        // TLS 1.3 suites (Firefox order: 1301, 1303, 1302)
        CipherSuite::TLS13_AES_128_GCM_SHA256,       // 0x1301
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256, // 0x1303
        CipherSuite::TLS13_AES_256_GCM_SHA384,       // 0x1302
        // TLS 1.2 ECDHE suites (Firefox interleaves ECDSA/RSA per cipher type)
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256, // 0xc02b
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,   // 0xc02f
        CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256, // 0xcca9
        CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256, // 0xcca8
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384, // 0xc02c
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,   // 0xc030
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA,    // 0xc00a
        CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA,    // 0xc009
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA,      // 0xc013
        CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA,      // 0xc014
        // TLS 1.2 RSA suites (Firefox order: GCM first, then CBC)
        CipherSuite::TLS_RSA_WITH_AES_128_GCM_SHA256, // 0x009c
        CipherSuite::TLS_RSA_WITH_AES_256_GCM_SHA384, // 0x009d
        CipherSuite::TLS_RSA_WITH_AES_128_CBC_SHA,    // 0x002f
        CipherSuite::TLS_RSA_WITH_AES_256_CBC_SHA,    // 0x0035
    ];

    /// Edge's default cipher suite list (same as Chrome)
    pub const EDGE: &[CipherSuite] = CHROME;

    /// Opera's default cipher suite list (same as Chrome)
    pub const OPERA: &[CipherSuite] = CHROME;
}

/// Pre-defined signature algorithm lists for browser emulation
#[cfg(feature = "impersonate")]
pub mod signature_algorithms {
    use super::SignatureScheme;

    /// Chrome/Edge 150+ default signature algorithm list.
    ///
    /// Real Chrome 150 / Edge 150 (tls.browserleaks.com) advertise the three
    /// ML-DSA post-quantum schemes first, followed by the classic 8. Earlier
    /// Chrome/Edge versions (<=149) do NOT advertise ML-DSA.
    pub const CHROME_V150: &[SignatureScheme] = &[
        SignatureScheme::ML_DSA_44,
        SignatureScheme::ML_DSA_65,
        SignatureScheme::ML_DSA_87,
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PKCS1_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::RSA_PKCS1_SHA512,
    ];

    /// Chrome's default signature algorithm list (8 algorithms)
    pub const CHROME: &[SignatureScheme] = &[
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PKCS1_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::RSA_PKCS1_SHA512,
    ];

    /// Safari's default signature algorithm list (10 algorithms including sha1).
    ///
    /// NOTE: `RSA_PSS_SHA384` appears twice intentionally. This matches the
    /// actual TLS ClientHello of real Safari browsers — confirmed by all Safari
    /// JA4 tests against tls.browserleaks.com. Removing the duplicate causes
    /// the JA4 third-segment hash to change, producing a fingerprint mismatch
    /// with real Safari.
    pub const SAFARI: &[SignatureScheme] = &[
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PKCS1_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::RSA_PKCS1_SHA512,
        SignatureScheme::RSA_PKCS1_SHA1,
    ];

    /// Firefox's default signature algorithm list
    pub const FIREFOX: &[SignatureScheme] = &[
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::ECDSA_NISTP521_SHA512,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::RSA_PKCS1_SHA256,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PKCS1_SHA512,
        SignatureScheme::ECDSA_SHA1_Legacy,
        SignatureScheme::RSA_PKCS1_SHA1,
    ];

    /// Edge's default signature algorithm list (same as Chrome)
    pub const EDGE: &[SignatureScheme] = CHROME;

    /// Opera's default signature algorithm list (same as Chrome)
    pub const OPERA: &[SignatureScheme] = CHROME;
}

/// Named group lists for browser emulation
#[cfg(feature = "impersonate")]
pub mod named_groups {
    use super::NamedGroup;

    /// Chrome's default named group list with GREASE and X25519MLKEM768 (matching Chrome 140+)
    /// Real Chrome 140: GREASE, X25519MLKEM768, x25519, secp256r1, secp384r1
    pub const CHROME: &[NamedGroup] = &[
        NamedGroup::GREASE,
        NamedGroup::X25519MLKEM768,
        NamedGroup::X25519,
        NamedGroup::secp256r1,
        NamedGroup::secp384r1,
    ];

    /// Chrome's named group list without GREASE (for reference)
    pub const CHROME_NO_GREASE: &[NamedGroup] = &[
        NamedGroup::X25519MLKEM768,
        NamedGroup::X25519,
        NamedGroup::secp256r1,
        NamedGroup::secp384r1,
    ];

    /// Safari's default named group list (includes X25519MLKEM768 for compatibility)
    pub const SAFARI: &[NamedGroup] = &[
        NamedGroup::X25519MLKEM768,
        NamedGroup::X25519,
        NamedGroup::secp256r1,
        NamedGroup::secp384r1,
    ];

    /// Firefox's default named group list (Firefox 140+)
    /// Includes X25519MLKEM768 for post-quantum key exchange
    pub const FIREFOX: &[NamedGroup] = &[
        NamedGroup::X25519MLKEM768,
        NamedGroup::X25519,
        NamedGroup::secp256r1,
        NamedGroup::secp384r1,
        NamedGroup::secp521r1,
        NamedGroup::FFDHE2048,
        NamedGroup::FFDHE3072,
    ];

    /// Edge's default named group list (same as Chrome)
    pub const EDGE: &[NamedGroup] = CHROME;

    /// Opera's default named group list (includes X25519MLKEM768)
    pub const OPERA: &[NamedGroup] = &[
        NamedGroup::GREASE,
        NamedGroup::X25519MLKEM768,
        NamedGroup::X25519,
        NamedGroup::secp256r1,
        NamedGroup::secp384r1,
    ];
}

/// Extension order seeds for deterministic extension ordering
/// These seeds are used to generate extension orders that match expected ja4 fingerprints
#[cfg(feature = "impersonate")]
pub mod extension_order {
    /// Chrome's extension order seed (for ja4=t13d1516h2_8daaf6152771_d8a2da3f94cd)
    pub const CHROME: u16 = 0x8daau16;

    /// Safari's extension order seed (13 extensions)
    /// Produces ja4=t13d2013h2_a09f3c656075_7f0f34a4126d
    pub const SAFARI: u16 = 0x6560u16;

    /// Firefox's typical extension order seed
    pub const FIREFOX: u16 = 0x9abcu16;

    /// Edge's extension order seed (same as Chrome)
    pub const EDGE: u16 = CHROME;

    /// Opera's extension order seed (VESTIGIAL — do not use).
    /// NOTE: `imp/opera/mod.rs::new_opera_emulator` intentionally reuses
    /// `CHROME` (0x8daa) instead — every real Opera capture has the SAME JA4
    /// extension-order hash as Chrome, so switching Opera to this seed would
    /// produce a JA4 matching no real browser. Kept only because it is
    /// referenced by tests/comments; dead code.
    pub const OPERA: u16 = 0x0271u16;

    /// Safari 18.5's extension order seed (13 extensions)
    /// Produces ja4=t13d2014h2_a09f3c656075_e42f34c56612
    pub const SAFARI_18_5: u16 = 0x9a7cu16;

    /// Safari 26's extension order seed (13 extensions)
    /// Produces ja4=t13d2013h2_a09f3c656075_7f0f34a4126d
    pub const SAFARI_26: u16 = 0x6560u16;
}
