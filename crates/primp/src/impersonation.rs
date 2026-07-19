//! Browser impersonation logic: applies browser-specific TLS, HTTP/2, and
//! header settings to a client builder.

use crate::imp::{BrowserSettings, Http2Data};

/// TLS options to apply when building an impersonated client. These mirror the
/// `ClientBuilder` TLS settings that impersonation otherwise skips.
pub(crate) struct ImpersonationTls {
    pub(crate) certs_verification: bool,
    pub(crate) hostname_verification: bool,
    pub(crate) tls_certs_only: bool,
    pub(crate) identity: Option<crate::Identity>,
    pub(crate) tls_sni: bool,
    pub(crate) tls_sslkeylogfile: bool,
}

/// Apply impersonation settings to a primp ClientBuilder.
pub(crate) fn apply_impersonation(
    builder: crate::ClientBuilder,
    settings: BrowserSettings,
    custom_certs: &[crate::Certificate],
    tls: ImpersonationTls,
) -> crate::Result<crate::Client> {
    let tls_config = build_impersonate_tls_config(&settings, custom_certs, &tls)?;
    let mut builder = builder.use_preconfigured_tls(tls_config);
    builder = apply_http2_settings(builder, &settings.http2);
    builder = builder.default_headers(settings.headers);

    if !settings.gzip {
        builder = builder.no_gzip();
    }
    if !settings.brotli {
        builder = builder.no_brotli();
    }
    if !settings.zstd {
        builder = builder.no_zstd();
    }
    if !settings.deflate {
        builder = builder.no_deflate();
    }

    builder.clear_impersonation().build()
}

pub(crate) fn build_impersonate_tls_config(
    settings: &BrowserSettings,
    custom_certs: &[crate::Certificate],
    tls: &ImpersonationTls,
) -> crate::Result<rustls::ClientConfig> {
    use crate::tls::{IgnoreHostname, NoVerifier};
    use rustls::client::EchMode;
    use std::sync::{Arc, OnceLock};

    static CRYPTO_PROVIDER: OnceLock<Arc<rustls::crypto::CryptoProvider>> = OnceLock::new();
    static DEFAULT_ROOT_STORE: OnceLock<Arc<rustls::RootCertStore>> = OnceLock::new();

    let provider = CRYPTO_PROVIDER
        .get_or_init(|| {
            rustls::crypto::CryptoProvider::get_default()
                .cloned()
                .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
        })
        .clone();

    // Profiles advertising ML-DSA must also verify it; others keep the stock
    // verification set and can never accept a scheme they did not offer.
    let provider = impersonation_provider(provider, &settings.browser_emulator);

    let signature_algorithms = provider.signature_verification_algorithms;

    let root_store = if custom_certs.is_empty() {
        DEFAULT_ROOT_STORE
            .get_or_init(|| Arc::new(crate::tls::default_root_store().clone()))
            .clone()
    } else {
        Arc::new(crate::tls::merged_root_store(custom_certs)?)
    };

    let needs_ech_grease =
        settings.browser_emulator.is_chrome_based() || settings.browser_emulator.is_firefox();

    // Build the base config (verifier stage) for this browser.
    let verifier_builder = if needs_ech_grease {
        rustls::ClientConfig::builder_with_provider(provider)
            .with_ech(EchMode::Grease(get_ech_grease_config()))
            .map_err(|e| crate::error::builder(format!("invalid ECH GREASE config: {e}")))?
    } else {
        rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| crate::error::builder(format!("invalid TLS versions: {e}")))?
    };

    // Honor `danger_accept_invalid_certs` / `danger_accept_invalid_hostnames`
    // and any custom root certificates, matching the non-impersonation TLS
    // build path (see `ClientBuilder::build`).
    let cert_builder = if !tls.certs_verification {
        verifier_builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
    } else if !tls.hostname_verification {
        // Both the normal and impersonation TLS build paths permit disabling
        // hostname verification without `tls_certs_only()`.
        verifier_builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(IgnoreHostname::new(
                (*root_store).clone(),
                signature_algorithms,
            )))
    } else if tls.tls_certs_only {
        // Only trust the user-provided root certs, ignoring the system and
        // webpki built-in stores, matching the TCP/TLS path.
        verifier_builder.with_root_certificates(Arc::new(crate::tls::rustls_store(custom_certs)?))
    } else {
        verifier_builder.with_root_certificates(root_store)
    };

    let mut config = if let Some(id) = &tls.identity {
        id.clone().add_to_rustls(cert_builder)?
    } else {
        cert_builder.with_no_client_auth()
    };

    config.enable_sni = tls.tls_sni;
    if tls.tls_sslkeylogfile {
        config.key_log = Arc::new(rustls::KeyLogFile::new());
    }

    config.browser_emulation = Some((*settings.browser_emulator).clone());
    config.alpn_protocols = vec!["h2".into(), "http/1.1".into()];

    if settings.browser_emulator.is_chrome_based() {
        // Real Chrome/Edge/Opera (BoringSSL) advertise ONLY brotli in the
        // `compress_certificate` extension. Advertising zlib as well changes
        // the extension body on the wire and defeats impersonation fidelity
        // (JA4 hashes extension *types*, so this mismatch is invisible to the
        // JA4 tests — see `cert_compression_tests`).
        config.cert_decompressors.retain(|d| {
            matches!(
                d.algorithm(),
                rustls::CertificateCompressionAlgorithm::Brotli
            )
        });
    } else if settings.browser_emulator.is_firefox() {
        config.cert_decompressors.sort_by(|a, b| {
            let a_is_zlib = matches!(a.algorithm(), rustls::CertificateCompressionAlgorithm::Zlib);
            let b_is_zlib = matches!(b.algorithm(), rustls::CertificateCompressionAlgorithm::Zlib);
            b_is_zlib.cmp(&a_is_zlib)
        });
    }

    Ok(config)
}

/// Pair the verifier with the emulator's advertised signatures: if it offers
/// ML-DSA in the ClientHello, return a provider whose verification set also
/// covers mldsa44/65/87; otherwise return `provider` unchanged (the stock set
/// never accepts a scheme the profile did not advertise).
fn impersonation_provider(
    provider: std::sync::Arc<rustls::crypto::CryptoProvider>,
    emulator: &rustls::client::BrowserEmulator,
) -> std::sync::Arc<rustls::crypto::CryptoProvider> {
    use rustls::SignatureScheme::{ML_DSA_44, ML_DSA_65, ML_DSA_87};

    let advertises_mldsa = emulator
        .signature_algorithms
        .as_ref()
        .is_some_and(|schemes| {
            schemes
                .iter()
                .any(|s| matches!(s, ML_DSA_44 | ML_DSA_65 | ML_DSA_87))
        });

    if !advertises_mldsa {
        return provider;
    }

    std::sync::Arc::new(rustls::crypto::CryptoProvider {
        signature_verification_algorithms: rustls::crypto::aws_lc_rs::SUPPORTED_SIG_ALGS_WITH_MLDSA,
        ..(*provider).clone()
    })
}

fn apply_http2_settings(
    mut builder: crate::ClientBuilder,
    http2: &Http2Data,
) -> crate::ClientBuilder {
    if let Some(size) = http2.initial_stream_window_size {
        builder = builder.http2_initial_stream_window_size(size);
    }
    if let Some(size) = http2.initial_connection_window_size {
        builder = builder.http2_initial_connection_window_size(size);
    }
    if let Some(max) = http2.max_concurrent_streams {
        builder = builder.http2_max_concurrent_streams(max);
    }
    if let Some(size) = http2.max_frame_size {
        builder = builder.http2_max_frame_size(size);
    }
    if let Some(size) = http2.max_header_list_size {
        builder = builder.http2_max_header_list_size(size);
    }
    if let Some(size) = http2.header_table_size {
        builder = builder.http2_header_table_size(size);
    }
    if let Some(enabled) = http2.enable_push {
        builder = builder.http2_enable_push(enabled);
    }
    if let Some(order) = http2.settings_order.clone() {
        builder = builder.http2_settings_order(order);
    }
    if let Some(order) = http2.headers_pseudo_order.clone() {
        builder = builder.http2_headers_pseudo_order(order);
    }
    if let Some(data) = http2.headers_priority {
        builder = builder.http2_headers_priority(Some(data));
    }
    if let Some(ref order) = http2.headers_order {
        builder = builder.http2_headers_order(order.clone());
    }
    if let Some(enabled) = http2.no_rfc7540_priorities {
        builder = builder.http2_no_rfc7540_priorities(enabled);
    }
    if let Some(val) = http2.enable_connect_protocol {
        builder = builder.http2_enable_connect_protocol(u32::from(val));
    }
    if let Some(stream_id) = http2.initial_stream_id {
        builder = builder.http2_initial_stream_id(stream_id);
    }
    if let Some(incr) = http2.initial_stream_window_size_increment {
        builder = builder.http2_initial_stream_window_size_increment(incr);
    }
    builder
}

fn get_ech_grease_config() -> rustls::client::EchGreaseConfig {
    use rustls::client::EchGreaseConfig;
    use rustls::crypto::aws_lc_rs::hpke::DH_KEM_X25519_HKDF_SHA256_AES_128;
    use rustls::crypto::hpke::HpkePublicKey;
    use std::sync::OnceLock;

    const GREASE_25519_PUBKEY: &[u8] = &[
        0x67, 0x35, 0xCA, 0x50, 0x21, 0xFC, 0x4F, 0xE6, 0x29, 0x3B, 0x31, 0x2C, 0xB5, 0xE0, 0x97,
        0xD8, 0x55, 0x1A, 0x8F, 0x8B, 0xA4, 0x77, 0xAB, 0xFA, 0xBE, 0xA4, 0x53, 0xA3, 0x82, 0x7C,
        0x8A, 0x4B,
    ];

    static ECH_GREASE_CONFIG: OnceLock<rustls::client::EchGreaseConfig> = OnceLock::new();

    ECH_GREASE_CONFIG
        .get_or_init(|| {
            EchGreaseConfig::new(
                DH_KEM_X25519_HKDF_SHA256_AES_128,
                HpkePublicKey(GREASE_25519_PUBKEY.to_vec()),
            )
        })
        .clone()
}

#[cfg(test)]
mod cert_compression_tests {
    //! Regression tests asserting the advertised `compress_certificate` (extension
    //! 0x1b) algorithm list matches real captures. JA4
    //! hashes extension *types*, not bodies, so the live JA4 tests miss a wrong
    //! algorithm list; these offline tests inspect the built `ClientConfig`.

    use super::build_impersonate_tls_config;
    use super::ImpersonationTls;
    use crate::imp::{get_browser_settings, Impersonate, ImpersonateOS};
    use rustls::CertificateCompressionAlgorithm as Alg;

    fn advertised_algorithms(imp: Impersonate) -> Vec<Alg> {
        let settings = get_browser_settings(imp, Some(ImpersonateOS::Linux));
        let tls = ImpersonationTls {
            certs_verification: true,
            hostname_verification: true,
            tls_certs_only: false,
            identity: None,
            tls_sni: true,
            tls_sslkeylogfile: false,
        };
        let config = build_impersonate_tls_config(&settings, &[], &tls)
            .expect("build impersonate tls config");
        config
            .cert_decompressors
            .iter()
            .map(|d| d.algorithm())
            .collect()
    }

    /// Real chrome-based captures advertise `compress_certificate` with a single
    /// algorithm — brotli. Advertising zlib too is a fingerprint mismatch
    /// (invisible to JA4) that defeats impersonation.
    #[test]
    fn chrome_based_advertise_brotli_only() {
        for imp in [
            Impersonate::ChromeV149,
            Impersonate::ChromeV150,
            Impersonate::ChromeV151,
            Impersonate::ChromeV152,
            Impersonate::EdgeV149,
            Impersonate::EdgeV150,
            Impersonate::EdgeV151,
            Impersonate::OperaV132,
            Impersonate::OperaV133,
            Impersonate::OperaV134,
            Impersonate::OperaV135,
        ] {
            let algs = advertised_algorithms(imp);
            assert_eq!(
                algs,
                vec![Alg::Brotli],
                "{imp:?} must advertise brotli-only cert compression (real capture), got {algs:?}",
            );
        }
    }

    /// Firefox captures advertise zlib, brotli and zstd, with zlib first
    /// (`[zlib, brotli, zstd]`).
    #[test]
    fn firefox_advertises_zlib_first() {
        let algs = advertised_algorithms(Impersonate::FirefoxV140);
        assert_eq!(
            algs,
            vec![Alg::Zlib, Alg::Brotli, Alg::Zstd],
            "Firefox must advertise [zlib, brotli, zstd] (zlib first), got {algs:?}",
        );
    }

    fn advertised_signature_schemes(imp: Impersonate) -> Vec<rustls::SignatureScheme> {
        let settings = get_browser_settings(imp, Some(ImpersonateOS::Linux));
        let tls = ImpersonationTls {
            certs_verification: true,
            hostname_verification: true,
            tls_certs_only: false,
            identity: None,
            tls_sni: true,
            tls_sslkeylogfile: false,
        };
        let config = build_impersonate_tls_config(&settings, &[], &tls)
            .expect("build impersonate tls config");
        config
            .browser_emulation
            .as_ref()
            .unwrap()
            .signature_algorithms
            .clone()
            .unwrap()
    }

    /// Chrome/Edge 150 advertise the three ML-DSA schemes (mldsa44/65/87) first,
    /// then the classic 8; earlier versions (<=149) must NOT advertise ML-DSA.
    /// Opera 134 (Chrome-150-based) advertises them too. JA4's
    /// signature-algorithm hash (`c` field) depends on this.
    #[test]
    fn chrome_edge_150_advertise_mldsa() {
        use rustls::SignatureScheme::{ML_DSA_44, ML_DSA_65, ML_DSA_87};
        let expected = [ML_DSA_44, ML_DSA_65, ML_DSA_87];

        for imp in [
            Impersonate::ChromeV150,
            Impersonate::ChromeV151,
            Impersonate::ChromeV152,
            Impersonate::EdgeV150,
            Impersonate::EdgeV151,
            Impersonate::OperaV134,
            Impersonate::OperaV135,
        ] {
            let schemes = advertised_signature_schemes(imp);
            assert_eq!(
                &schemes[..3],
                &expected,
                "{imp:?} must advertise ML-DSA (mldsa44/65/87) first, got {schemes:?}",
            );
        }

        for imp in [
            Impersonate::ChromeV146,
            Impersonate::ChromeV149,
            Impersonate::EdgeV146,
            Impersonate::EdgeV149,
            Impersonate::OperaV133,
        ] {
            let schemes = advertised_signature_schemes(imp);
            assert!(
                !schemes
                    .iter()
                    .any(|s| matches!(s, ML_DSA_44 | ML_DSA_65 | ML_DSA_87)),
                "{imp:?} must NOT advertise ML-DSA (real capture has none), got {schemes:?}",
            );
        }
    }

    /// ML-DSA-advertising profiles must be able to verify ML-DSA; profiles
    /// that do not advertise it must keep the stock, ML-DSA-free set.
    #[test]
    fn mldsa_advertising_profiles_enable_mldsa_verification() {
        use rustls::SignatureScheme::{ML_DSA_44, ML_DSA_65, ML_DSA_87};
        let base = std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider());

        for imp in [
            Impersonate::ChromeV150,
            Impersonate::ChromeV151,
            Impersonate::ChromeV152,
            Impersonate::EdgeV150,
            Impersonate::EdgeV151,
            Impersonate::OperaV134,
            Impersonate::OperaV135,
        ] {
            let settings = get_browser_settings(imp, Some(crate::imp::ImpersonateOS::Linux));
            let provider = super::impersonation_provider(base.clone(), &settings.browser_emulator);
            let schemes = provider
                .signature_verification_algorithms
                .supported_schemes();
            assert!(
                [ML_DSA_44, ML_DSA_65, ML_DSA_87]
                    .iter()
                    .all(|s| schemes.contains(s)),
                "{imp:?} advertises ML-DSA, so its provider must verify it; got {schemes:?}",
            );
        }

        for imp in [
            Impersonate::ChromeV146,
            Impersonate::ChromeV149,
            Impersonate::EdgeV146,
            Impersonate::EdgeV149,
            Impersonate::OperaV133,
            Impersonate::FirefoxV140,
            Impersonate::SafariV18_5,
        ] {
            let settings = get_browser_settings(imp, Some(crate::imp::ImpersonateOS::Linux));
            let provider = super::impersonation_provider(base.clone(), &settings.browser_emulator);
            let schemes = provider
                .signature_verification_algorithms
                .supported_schemes();
            assert!(
                !schemes
                    .iter()
                    .any(|s| matches!(s, ML_DSA_44 | ML_DSA_65 | ML_DSA_87)),
                "{imp:?} does not advertise ML-DSA, so its provider must not accept it; got {schemes:?}",
            );
        }
    }
}
