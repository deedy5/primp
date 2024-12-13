use std::borrow::Cow;

use anyhow::Error;
use rand::{seq::SliceRandom, thread_rng};
use rquest::{
    boring::ssl::{SslConnector, SslCurve, SslMethod},
    header::{self, HeaderMap, HeaderName, HeaderValue, ACCEPT},
    tls::{
        cert_compression::CertCompressionAlgorithm, Http2Settings, ImpersonateSettings,
        TlsSettings, TlsVersion,
    },
    HttpVersionPref,
    PseudoOrder::{self, *},
    SettingsOrder::{self, *},
};

pub const IMPERSONATES: &[&str] = &[
    "chrome_100",
    "chrome_101",
    "chrome_104",
    "chrome_105",
    "chrome_106",
    "chrome_107",
    "chrome_108",
    "chrome_109",
    "chrome_114",
    "chrome_116",
    "chrome_117",
    "chrome_118",
    "chrome_119",
    "chrome_120",
    "chrome_123",
    "chrome_124",
    "chrome_126",
    "chrome_127",
    "chrome_128",
    "chrome_129",
    "chrome_130",
    "chrome_131",
    "safari_ios_16.5",
    "safari_ios_17.2",
    "safari_ios_17.4.1",
    "safari_15.3",
    "safari_15.5",
    "safari_15.6.1",
    "safari_16",
    "safari_16.5",
    "safari_17.0",
    "safari_17.2.1",
    "safari_17.4.1",
    "safari_17.5",
    "safari_18",
    "safari_ipad_18",
    "okhttp_3.9",
    "okhttp_3.11",
    "okhttp_3.13",
    "okhttp_3.14",
    "okhttp_4.9",
    "okhttp_4.10",
    "okhttp_5",
    "edge_101",
    "edge_122",
    "edge_127",
];

const HEADER_ORDER: &[HeaderName] = &[
    header::USER_AGENT,
    header::ACCEPT,
    header::ACCEPT_LANGUAGE,
    header::ACCEPT_ENCODING,
];

const CURVES: &[SslCurve] = &[
    SslCurve::X25519_MLKEM768,
    SslCurve::X25519,
    SslCurve::SECP256R1,
    SslCurve::SECP384R1,
];

const CIPHER_LIST: &[&str] = &[
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA",
    "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA",
    "TLS_RSA_WITH_AES_128_GCM_SHA256",
    "TLS_RSA_WITH_AES_256_GCM_SHA384",
    "TLS_RSA_WITH_AES_128_CBC_SHA",
    "TLS_RSA_WITH_AES_256_CBC_SHA",
];

const SIGALGS_LIST: &[&str] = &[
    "ecdsa_secp256r1_sha256",
    "rsa_pss_rsae_sha256",
    "rsa_pkcs1_sha256",
    "ecdsa_secp384r1_sha384",
    "rsa_pss_rsae_sha384",
    "rsa_pkcs1_sha384",
    "rsa_pss_rsae_sha512",
    "rsa_pkcs1_sha512",
];

const INITIAL_STREAM_WINDOW_SIZE: &[u32] = &[2097152, 4194304, 6291456];
const INITIAL_CONNECTION_WINDOW_SIZE: &[u32] = &[10485760, 10551295, 15728640];
const MAX_CONCURRENT_STREAMS: &[u32] = &[100, 1000];
const HEADERS_PSEUDO_ORDER: &[[PseudoOrder; 4]] = &[
    [Method, Authority, Scheme, Path],
    [Method, Path, Authority, Scheme],
    [Method, Scheme, Authority, Path],
    [Method, Scheme, Path, Authority],
];
const SETTINGS_ORDER: &[[SettingsOrder; 8]] = &[
    [
        HeaderTableSize,
        EnablePush,
        MaxConcurrentStreams,
        InitialWindowSize,
        MaxFrameSize,
        MaxHeaderListSize,
        UnknownSetting8,
        UnknownSetting9,
    ],
    [
        HeaderTableSize,
        EnablePush,
        InitialWindowSize,
        MaxConcurrentStreams,
        MaxFrameSize,
        MaxHeaderListSize,
        UnknownSetting8,
        UnknownSetting9,
    ],
];

pub fn get_random_element<T>(input_vec: &[T]) -> &T {
    input_vec.choose(&mut thread_rng()).unwrap()
}

fn shuffle_list<T: Clone>(input_vec: &[T]) -> Vec<T> {
    let mut vec_cloned = input_vec.to_vec();
    vec_cloned.shuffle(&mut thread_rng());
    vec_cloned
}

pub fn get_chaos_impersonate_settings() -> Result<ImpersonateSettings, Error> {
    // Create a TLS connector builder
    let connector = SslConnector::no_default_verify_builder(SslMethod::tls_client())?;

    // Create a pre-configured TLS settings
    let tls_settings = TlsSettings::builder()
        .connector(connector)
        .grease_enabled(true)
        .enable_ocsp_stapling(true)
        .enable_signed_cert_timestamps(true)
        .curves(Cow::Owned(shuffle_list(CURVES)))
        .sigalgs_list(Cow::Owned(shuffle_list(SIGALGS_LIST).join(":")))
        .cipher_list(Cow::Owned(shuffle_list(CIPHER_LIST).join(":")))
        .min_tls_version(TlsVersion::TLS_1_2)
        .max_tls_version(TlsVersion::TLS_1_3)
        .permute_extensions(true)
        .pre_shared_key(true)
        .enable_ech_grease(true)
        .application_settings(true)
        .cert_compression_algorithm(CertCompressionAlgorithm::Brotli)
        .tls_sni(true)
        .alpn_protos(HttpVersionPref::All)
        .build();

    // Create http2 settings
    let http2_settings = Http2Settings::builder()
        .initial_stream_window_size(*get_random_element(INITIAL_STREAM_WINDOW_SIZE))
        .initial_connection_window_size(*get_random_element(INITIAL_CONNECTION_WINDOW_SIZE))
        .max_concurrent_streams(*get_random_element(MAX_CONCURRENT_STREAMS))
        .max_header_list_size(262144)
        .header_table_size(65536)
        .enable_push(false)
        .headers_priority((0, 255, true))
        .headers_pseudo_order(*get_random_element(HEADERS_PSEUDO_ORDER))
        .settings_order(*get_random_element(SETTINGS_ORDER))
        .build();

    // Create headers
    let headers = {
        let mut headers = HeaderMap::new();
        //headers.insert(header::USER_AGENT, HeaderValue::from_static("primp"));
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, deflate, br, zstd"),
        );
        Cow::Owned(headers)
    };

    // Create impersonate settings
    let impersonate_settings = ImpersonateSettings::builder()
        .tls(tls_settings)
        .http2(http2_settings)
        .headers(headers)
        .headers_order(Cow::Borrowed(HEADER_ORDER))
        .build();

    Ok(impersonate_settings)
}
