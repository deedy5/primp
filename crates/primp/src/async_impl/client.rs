use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{ready, Context, Poll};
use std::time::Duration;
use std::{convert::TryInto, net::SocketAddr};
use std::{fmt, str};

use foldhash::{HashMap, HashMapExt};

use super::request::{Request, RequestBuilder};
use super::response::Response;
use super::Body;
use crate::async_impl::h1_client::{build_legacy_http1_client, Http1Pool, LegacyClientSettings};
use crate::async_impl::h2_client::connect::{H2ClientConfig, H2Connector};
use crate::async_impl::h2_client::pool::DEFAULT_H2_MAX_CONCURRENT_STREAMS;
use crate::async_impl::h2_client::Pool;
#[cfg(feature = "http3")]
use crate::async_impl::h3_client::connect::{
    H3ClientConfig, H3Connector, QuinnIgnoreHostname, QuinnNoVerifier,
};
#[cfg(feature = "http3")]
use crate::async_impl::h3_client::H3Client;
use crate::async_impl::negotiate::NegotiatingConnection;
use crate::async_impl::range_guard::{self, RangeGuard};
use crate::config::{ReadTimeout, RequestConfig, TotalTimeout};
#[cfg(unix)]
use crate::connect::uds::UnixSocketProvider;
#[cfg(target_os = "windows")]
use crate::connect::windows_named_pipe::WindowsNamedPipeProvider;
use crate::connect::{
    sealed::{Conn, Unnameable},
    BoxedConnectorLayer, BoxedConnectorService, ConnectorBuilder,
};
#[cfg(feature = "cookies")]
use crate::cookie;
#[cfg(feature = "cookies")]
use crate::cookie::service::CookieService;
#[cfg(feature = "hickory-dns")]
use crate::dns::hickory::HickoryDnsResolver;
use crate::dns::{gai::GaiResolver, DnsResolverWithOverrides, DynResolver, Resolve};
use crate::error::{self, BoxError};
use crate::into_url::try_uri;
use crate::proxy::Matcher as ProxyMatcher;
use crate::redirect::{self, TowerRedirectPolicy};
use crate::tls::CertificateRevocationList;
use crate::tls::{self, TlsBackend};
use crate::Certificate;
use crate::Identity;
use crate::{IntoUrl, Method, Proxy, Url};
#[cfg(feature = "http3")]
use quinn::rustls::pki_types::CertificateDer;

use http::header::{Entry, HeaderMap, HeaderValue, ACCEPT, PROXY_AUTHORIZATION, USER_AGENT};
use http::uri::Scheme;
use http::Uri;
use hyper_util::client::legacy::connect::HttpConnector;
use pin_project_lite::pin_project;
#[cfg(feature = "http3")]
use quinn::TransportConfig;
#[cfg(feature = "http3")]
use quinn::VarInt;

/// Convert a `u64` to a QUIC [`VarInt`], clamping to `VarInt::MAX`
/// (2^62 - 1) instead of panicking on out-of-range values.
#[cfg(feature = "http3")]
fn clamp_varint(value: u64) -> VarInt {
    VarInt::from_u64(value).unwrap_or(VarInt::MAX)
}
use tokio::time::Sleep;
use tower::util::BoxCloneSyncServiceLayer;
use tower::{Layer, Service};
#[cfg(any(
    feature = "gzip",
    feature = "brotli",
    feature = "zstd",
    feature = "deflate"
))]
use tower_http::decompression::Decompression;
use tower_http::follow_redirect::FollowRedirect;

/// An asynchronous `Client` to make Requests with.
///
/// The Client has various configuration values to tweak, but the defaults
/// are set to what is usually the most commonly desired value. To configure a
/// `Client`, use `Client::builder()`.
///
/// The `Client` holds a connection pool internally to improve performance
/// by reusing connections and avoiding setup overhead, so it is advised that
/// you create one and **reuse** it.
///
/// You do **not** have to wrap the `Client` in an [`Rc`] or [`Arc`] to **reuse** it,
/// because it already uses an [`Arc`] internally.
///
/// # Connection Pooling
///
/// The connection pool can be configured using [`ClientBuilder`] methods
/// with the `pool_` prefix, such as [`ClientBuilder::pool_idle_timeout`]
/// and [`ClientBuilder::pool_max_idle_per_host`].
///
/// [`Rc`]: std::rc::Rc
#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientRef>,
}

/// A `ClientBuilder` can be used to create a `Client` with custom configuration.
#[must_use]
pub struct ClientBuilder {
    config: Config,
}

#[derive(Clone, Copy)]
pub(crate) enum HttpVersionPref {
    Http1,
    Http2,
    #[cfg(feature = "http3")]
    Http3,
    All,
}

#[derive(Clone, Copy, Debug)]
struct Accepts {
    #[cfg(feature = "gzip")]
    gzip: bool,
    #[cfg(feature = "brotli")]
    brotli: bool,
    #[cfg(feature = "zstd")]
    zstd: bool,
    #[cfg(feature = "deflate")]
    deflate: bool,
}

// `clippy::derivable_impls` is wrong here: every field is a `bool` and the
// manual default is `true`, while the derived `Default` would yield `false`
// for each field. The feature-gated fields make clippy miss this.
#[allow(clippy::derivable_impls)]
impl Default for Accepts {
    fn default() -> Accepts {
        Accepts {
            #[cfg(feature = "gzip")]
            gzip: true,
            #[cfg(feature = "brotli")]
            brotli: true,
            #[cfg(feature = "zstd")]
            zstd: true,
            #[cfg(feature = "deflate")]
            deflate: true,
        }
    }
}

pub(crate) struct Config {
    // NOTE: When adding a new field, update `fmt::Debug for ClientBuilder`
    accepts: Accepts,
    headers: HeaderMap,
    hostname_verification: bool,
    certs_verification: bool,
    tls_sni: bool,
    tls_sslkeylogfile: bool,
    connect_timeout: Option<Duration>,
    dns_timeout: Option<Duration>,
    connection_verbose: bool,
    pub(crate) pool_idle_timeout: Option<Duration>,
    pub(crate) pool_max_idle_per_host: usize,
    pool_max_connections: usize,
    tcp_keepalive: Option<Duration>,
    tcp_keepalive_interval: Option<Duration>,
    tcp_keepalive_retries: Option<u32>,
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    tcp_user_timeout: Option<Duration>,
    identity: Option<Identity>,
    proxies: Vec<ProxyMatcher>,
    auto_sys_proxy: bool,
    redirect_policy: redirect::Policy,
    retry_policy: crate::retry::Builder,
    referer: bool,
    read_timeout: Option<Duration>,
    timeout: Option<Duration>,
    root_certs: Vec<Certificate>,
    tls_certs_only: bool,
    crls: Vec<CertificateRevocationList>,
    min_tls_version: Option<tls::Version>,
    max_tls_version: Option<tls::Version>,
    tls_info: bool,
    tls: TlsBackend,
    connector_layers: Vec<BoxedConnectorLayer>,
    http_version_pref: HttpVersionPref,
    pub(crate) http09_responses: bool,
    pub(crate) http1_title_case_headers: bool,
    pub(crate) http1_allow_obsolete_multiline_headers_in_responses: bool,
    pub(crate) http1_ignore_invalid_headers_in_responses: bool,
    pub(crate) http1_allow_spaces_after_header_name_in_responses: bool,
    pub(crate) http1_max_headers: Option<usize>,
    pub(crate) http2_initial_stream_window_size: Option<u32>,
    pub(crate) http2_initial_connection_window_size: Option<u32>,
    pub(crate) http2_adaptive_window: bool,
    pub(crate) http2_max_frame_size: Option<u32>,
    pub(crate) http2_max_header_list_size: Option<u32>,
    pub(crate) http2_keep_alive_interval: Option<Duration>,
    pub(crate) http2_keep_alive_timeout: Option<Duration>,
    pub(crate) http2_keep_alive_while_idle: bool,
    http2_header_table_size: Option<u32>,
    http2_max_concurrent_streams: Option<u32>,
    http2_enable_push: Option<bool>,
    http2_no_rfc7540_priorities: Option<bool>,
    http2_enable_connect_protocol: Option<u32>,
    http2_settings_order: Option<h2::frame::SettingsOrder>,
    http2_headers_pseudo_order: Option<h2::frame::PseudoOrder>,
    http2_headers_priority: Option<(u8, u32, bool)>,
    http2_headers_order: Option<Vec<http::HeaderName>>,
    http2_initial_stream_id: Option<u32>,
    http2_initial_stream_window_size_increment: Option<u32>,
    local_address: Option<IpAddr>,
    #[cfg(any(
        target_os = "android",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "solaris",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    ))]
    interface: Option<String>,
    nodelay: bool,
    #[cfg(feature = "cookies")]
    cookie_store: Option<Arc<dyn cookie::CookieStore>>,
    hickory_dns: bool,
    error: Option<crate::Error>,
    https_only: bool,
    #[cfg(feature = "http3")]
    tls_enable_early_data: bool,
    #[cfg(feature = "http3")]
    quic_max_idle_timeout: Option<Duration>,
    #[cfg(feature = "http3")]
    quic_stream_receive_window: Option<VarInt>,
    #[cfg(feature = "http3")]
    quic_receive_window: Option<VarInt>,
    #[cfg(feature = "http3")]
    quic_send_window: Option<u64>,
    #[cfg(feature = "http3")]
    quic_congestion_bbr: bool,
    #[cfg(feature = "http3")]
    h3_max_field_section_size: Option<u64>,
    #[cfg(feature = "http3")]
    h3_send_grease: Option<bool>,
    dns_overrides: HashMap<String, Vec<SocketAddr>>,
    dns_resolver: Option<Arc<dyn Resolve>>,
    dns_cache_ttl: Option<Duration>,

    #[cfg(unix)]
    unix_socket: Option<Arc<std::path::Path>>,
    #[cfg(target_os = "windows")]
    windows_named_pipe: Option<Arc<std::ffi::OsStr>>,
    impersonate: Option<crate::imp::Impersonate>,
    os_type: Option<crate::imp::ImpersonateOS>,
}

fn h2_client_config_from(config: &Config) -> H2ClientConfig {
    H2ClientConfig {
        settings_order: config.http2_settings_order.clone(),
        headers_pseudo_order: config.http2_headers_pseudo_order.clone(),
        headers_order: config.http2_headers_order.clone(),
        headers_priority: config.http2_headers_priority,
        initial_stream_window_size_increment: config.http2_initial_stream_window_size_increment,
        initial_connection_window_size: config.http2_initial_connection_window_size,
        initial_window_size: config.http2_initial_stream_window_size,
        max_frame_size: config.http2_max_frame_size,
        max_header_list_size: config.http2_max_header_list_size,
        max_concurrent_streams: config.http2_max_concurrent_streams,
        header_table_size: config.http2_header_table_size,
        enable_push: config.http2_enable_push,
        no_rfc7540_priorities: config.http2_no_rfc7540_priorities,
        initial_stream_id: config.http2_initial_stream_id,
        enable_connect_protocol: config.http2_enable_connect_protocol,
        adaptive_window: config.http2_adaptive_window,
        keep_alive_interval: config.http2_keep_alive_interval,
        keep_alive_timeout: config.http2_keep_alive_timeout,
        keep_alive_while_idle: config.http2_keep_alive_while_idle,
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    /// Constructs a new `ClientBuilder`.
    ///
    /// This is the same as `Client::builder()`.
    pub fn new() -> Self {
        let mut headers: HeaderMap<HeaderValue> = HeaderMap::with_capacity(2);
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));

        ClientBuilder {
            config: Config {
                error: None,
                accepts: Accepts::default(),
                headers,
                hostname_verification: true,
                certs_verification: true,
                tls_sni: true,
                tls_sslkeylogfile: false,
                connect_timeout: Some(Duration::from_secs(30)),
                dns_timeout: None,
                connection_verbose: false,
                pool_idle_timeout: Some(Duration::from_secs(90)),
                pool_max_idle_per_host: usize::MAX,
                pool_max_connections: 256,
                tcp_keepalive: Some(Duration::from_secs(15)),
                tcp_keepalive_interval: Some(Duration::from_secs(15)),
                tcp_keepalive_retries: Some(3),
                #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
                tcp_user_timeout: Some(Duration::from_secs(30)),
                proxies: Vec::new(),
                auto_sys_proxy: true,
                redirect_policy: redirect::Policy::default(),
                retry_policy: crate::retry::Builder::default(),
                referer: true,
                read_timeout: None,
                timeout: Some(Duration::from_secs(30)),
                root_certs: Vec::new(),
                tls_certs_only: false,
                identity: None,
                crls: vec![],
                min_tls_version: None,
                max_tls_version: None,
                tls_info: false,
                tls: TlsBackend::default(),
                connector_layers: Vec::new(),
                http_version_pref: HttpVersionPref::All,
                http09_responses: false,
                http1_title_case_headers: false,
                http1_allow_obsolete_multiline_headers_in_responses: false,
                http1_ignore_invalid_headers_in_responses: false,
                http1_allow_spaces_after_header_name_in_responses: false,
                http1_max_headers: None,
                http2_initial_stream_window_size: None,
                http2_initial_connection_window_size: None,
                http2_adaptive_window: false,
                http2_max_frame_size: None,
                http2_max_header_list_size: None,
                http2_keep_alive_interval: None,
                http2_keep_alive_timeout: None,
                http2_keep_alive_while_idle: false,
                http2_header_table_size: None,
                http2_max_concurrent_streams: None,
                http2_enable_push: None,
                http2_no_rfc7540_priorities: None,
                http2_enable_connect_protocol: None,
                http2_settings_order: None,
                http2_headers_pseudo_order: None,
                http2_headers_priority: None,
                http2_headers_order: None,
                http2_initial_stream_id: None,
                http2_initial_stream_window_size_increment: None,
                local_address: None,
                #[cfg(any(
                    target_os = "android",
                    target_os = "fuchsia",
                    target_os = "illumos",
                    target_os = "ios",
                    target_os = "linux",
                    target_os = "macos",
                    target_os = "solaris",
                    target_os = "tvos",
                    target_os = "visionos",
                    target_os = "watchos",
                ))]
                interface: None,
                nodelay: true,
                hickory_dns: cfg!(feature = "hickory-dns"),
                #[cfg(feature = "cookies")]
                cookie_store: None,
                https_only: false,
                dns_overrides: HashMap::new(),
                #[cfg(feature = "http3")]
                tls_enable_early_data: false,
                #[cfg(feature = "http3")]
                quic_max_idle_timeout: None,
                #[cfg(feature = "http3")]
                quic_stream_receive_window: None,
                #[cfg(feature = "http3")]
                quic_receive_window: None,
                #[cfg(feature = "http3")]
                quic_send_window: None,
                #[cfg(feature = "http3")]
                quic_congestion_bbr: false,
                #[cfg(feature = "http3")]
                h3_max_field_section_size: None,
                #[cfg(feature = "http3")]
                h3_send_grease: None,
                dns_resolver: None,
                dns_cache_ttl: Some(Duration::from_secs(30)),
                #[cfg(unix)]
                unix_socket: None,
                #[cfg(target_os = "windows")]
                windows_named_pipe: None,
                impersonate: None,
                os_type: None,
            },
        }
    }
}

impl ClientBuilder {
    /// Returns a `Client` that uses this `ClientBuilder` configuration.
    ///
    /// # Errors
    ///
    /// This method fails if a TLS backend cannot be initialized, or the resolver
    /// cannot load the system configuration.
    pub fn build(self) -> crate::Result<Client> {
        {
            if self.config.impersonate.is_some() || self.config.os_type.is_some() {
                let imp = self
                    .config
                    .impersonate
                    .unwrap_or(crate::imp::Impersonate::Random);
                let os = self
                    .config
                    .os_type
                    .unwrap_or(crate::imp::ImpersonateOS::Random);
                let root_certs = self.config.root_certs.clone();
                let tls = crate::impersonation::ImpersonationTls {
                    certs_verification: self.config.certs_verification,
                    hostname_verification: self.config.hostname_verification,
                    tls_certs_only: self.config.tls_certs_only,
                    identity: self.config.identity.clone(),
                    tls_sni: self.config.tls_sni,
                    tls_sslkeylogfile: self.config.tls_sslkeylogfile,
                };
                let settings = crate::imp::get_browser_settings(imp, Some(os));
                return crate::impersonation::apply_impersonation(self, settings, &root_certs, tls);
            }
        }

        let config = self.config;

        // Capture the legacy HTTP/1.1 client settings before `config` is
        // partially moved during the rest of client construction.
        let legacy_settings = LegacyClientSettings::from_config(&config);

        if let Some(err) = config.error {
            return Err(err);
        }

        // Pre-compute H2 config before config fields are moved.
        let h2_cfg = h2_client_config_from(&config);

        let mut proxies = config.proxies;
        if config.auto_sys_proxy {
            proxies.push(ProxyMatcher::system());
        }
        let proxies = Arc::new(RwLock::new(proxies));

        #[allow(unused)]
        #[cfg(feature = "http3")]
        let mut h3_connector = None;
        #[allow(unused_assignments)]
        let mut h2_connector = None;

        let resolver = {
            let mut base: Arc<dyn Resolve> = match config.hickory_dns {
                false => Arc::new(GaiResolver::new()),
                #[cfg(feature = "hickory-dns")]
                true => Arc::new(HickoryDnsResolver::default()),
                #[cfg(not(feature = "hickory-dns"))]
                true => unreachable!("hickory-dns shouldn't be enabled unless the feature is"),
            };
            base = config.dns_resolver.unwrap_or(base);
            base = Arc::new(crate::dns::hosts::HostsFileResolver::new(base));
            // DNS deadline capped just below the connect deadline so a hanging
            // lookup surfaces as a tagged DNS error, never a connect timeout.
            // Cache sits *below* `DnsResolverWithOverrides`: per-client
            // overrides always win, and the cache only stores base
            // resolutions, so clients with different override sets or
            // different inner resolvers never poison each other.
            let dns_timeout = config
                .dns_timeout
                .unwrap_or(crate::dns::cache::DNS_RESOLUTION_TIMEOUT);
            let dns_timeout = match config.connect_timeout {
                Some(connect) if dns_timeout >= connect => {
                    connect.saturating_sub(Duration::from_millis(1))
                }
                _ => dns_timeout,
            };
            let cached: Arc<dyn Resolve> = Arc::new(match config.dns_cache_ttl {
                Some(ttl) => crate::dns::cache::DnsCacheResolver::with_ttl_and_timeout(
                    base,
                    ttl,
                    dns_timeout,
                ),
                None => crate::dns::cache::DnsCacheResolver::with_ttl_and_timeout(
                    base,
                    crate::dns::cache::DNS_CACHE_TTL,
                    dns_timeout,
                ),
            });
            let resolver: Arc<dyn Resolve> = if config.dns_overrides.is_empty() {
                cached
            } else {
                Arc::new(DnsResolverWithOverrides::new(cached, config.dns_overrides))
            };
            DynResolver::new(resolver)
        };

        let mut connector_builder = {
            fn user_agent(headers: &HeaderMap) -> Option<HeaderValue> {
                headers.get(USER_AGENT).cloned()
            }

            let mut http = HttpConnector::new_with_resolver(resolver.clone());
            http.set_connect_timeout(config.connect_timeout);

            #[cfg(feature = "http3")]
            let build_h3_connector =
                |resolver,
                 connect_timeout: Option<Duration>,
                 quic_max_idle_timeout: Option<Duration>,
                 quic_stream_receive_window,
                 quic_receive_window,
                 quic_send_window,
                 quic_congestion_bbr,
                 h3_max_field_section_size,
                 h3_send_grease,
                 local_address,
                 http_version_pref: &HttpVersionPref,
                 certs_verification: bool,
                 hostname_verification: bool,
                 tls_certs_only: bool,
                 root_certs: &[Certificate]| {
                    let mut transport_config = TransportConfig::default();

                    if let Some(max_idle_timeout) = quic_max_idle_timeout {
                        transport_config.max_idle_timeout(Some(
                            max_idle_timeout.try_into().map_err(error::builder)?,
                        ));
                    }

                    if let Some(stream_receive_window) = quic_stream_receive_window {
                        transport_config.stream_receive_window(stream_receive_window);
                    }

                    if let Some(receive_window) = quic_receive_window {
                        transport_config.receive_window(receive_window);
                    }

                    if let Some(send_window) = quic_send_window {
                        transport_config.send_window(send_window);
                    }

                    if quic_congestion_bbr {
                        let factory = Arc::new(quinn::congestion::BbrConfig::default());
                        transport_config.congestion_controller_factory(factory);
                    }

                    let mut h3_client_config = H3ClientConfig::default();

                    if let Some(max_field_section_size) = h3_max_field_section_size {
                        h3_client_config.max_field_section_size = Some(max_field_section_size);
                    }

                    if let Some(send_grease) = h3_send_grease {
                        h3_client_config.send_grease = Some(send_grease);
                    }

                    let provider = quinn::rustls::crypto::CryptoProvider::get_default()
                        .cloned()
                        .unwrap_or_else(|| {
                            Arc::new(quinn::rustls::crypto::aws_lc_rs::default_provider())
                        });
                    let signature_algorithms = provider.signature_verification_algorithms;

                    // Build a QUIC/rustls root store containing only the
                    // user-provided `root_certs` (no webpki-roots, no native
                    // store). Used when hostname verification is disabled or
                    // `tls_certs_only()` is set, mirroring the TCP/TLS path's
                    // `rustls_store()`.
                    let custom_roots = || -> crate::Result<quinn::rustls::RootCertStore> {
                        let mut roots = quinn::rustls::RootCertStore::empty();
                        for cert in root_certs {
                            for der in cert.as_der_many()? {
                                roots
                                    .add(CertificateDer::from(der))
                                    .map_err(|e| error::builder(e.to_string()))?;
                            }
                        }
                        Ok(roots)
                    };

                    // Build a QUIC/rustls root store with webpki-roots plus the
                    // native OS root CAs, then any user-provided root certs.
                    // Mirrors `default_root_store()` used by the TCP/TLS path.
                    let default_roots = || -> crate::Result<quinn::rustls::RootCertStore> {
                        let mut roots = quinn::rustls::RootCertStore::empty();
                        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                        let native = rustls_native_certs::load_native_certs();
                        for err in &native.errors {
                            log::warn!("failed to load native root certificate: {err}");
                        }
                        if !native.certs.is_empty() {
                            roots.add_parsable_certificates(native.certs);
                        }
                        for cert in root_certs {
                            for der in cert.as_der_many()? {
                                roots
                                    .add(CertificateDer::from(der))
                                    .map_err(|e| error::builder(e.to_string()))?;
                            }
                        }
                        Ok(roots)
                    };

                    // Build the TLS client config; advertise ALPN "h3" so the
                    // QUIC connection negotiates HTTP/3. The config is then
                    // wrapped into a `QuicClientConfig` and handed to the quinn
                    // transport.
                    let mut tls_config = if !certs_verification {
                        // Mirror `danger_accept_invalid_certs()` on the TCP/TLS
                        // path: skip certificate and signature verification.
                        quinn::rustls::ClientConfig::builder()
                            .dangerous()
                            .with_custom_certificate_verifier(Arc::new(QuinnNoVerifier::new(
                                signature_algorithms,
                            )))
                            .with_no_client_auth()
                    } else if !hostname_verification {
                        // Mirror `danger_accept_invalid_hostnames()` on the
                        // TCP/TLS path: verify the certificate chain but skip
                        // the hostname check. Uses only the user-provided root
                        // certs, matching the TCP/TLS path's `IgnoreHostname`.
                        quinn::rustls::ClientConfig::builder()
                            .dangerous()
                            .with_custom_certificate_verifier(Arc::new(QuinnIgnoreHostname::new(
                                custom_roots()?,
                                signature_algorithms,
                            )))
                            .with_no_client_auth()
                    } else if tls_certs_only {
                        // Only trust the user-provided root certs, ignoring the
                        // system + webpki stores.
                        quinn::rustls::ClientConfig::builder()
                            .with_root_certificates(Arc::new(custom_roots()?))
                            .with_no_client_auth()
                    } else {
                        // Full chain + hostname verification against the default
                        // root store.
                        quinn::rustls::ClientConfig::builder()
                            .with_root_certificates(Arc::new(default_roots()?))
                            .with_no_client_auth()
                    };
                    tls_config.alpn_protocols = vec![b"h3".into()];

                    let quic_tls = quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
                        .map_err(|e| error::builder(e.to_string()))?;
                    let quinn_client_config = quinn::ClientConfig::new(Arc::new(quic_tls));

                    let res = H3Connector::new(
                        resolver,
                        quinn_client_config,
                        local_address,
                        transport_config,
                        h3_client_config,
                        connect_timeout,
                    );

                    match res {
                        Ok(connector) => Ok(Some(connector)),
                        Err(err) => {
                            if let HttpVersionPref::Http3 = http_version_pref {
                                Err(error::builder(err))
                            } else {
                                Ok(None)
                            }
                        }
                    }
                };

            match config.tls {
                TlsBackend::BuiltRustls(conn) => {
                    #[cfg(feature = "http3")]
                    {
                        h3_connector = build_h3_connector(
                            resolver.clone(),
                            config.connect_timeout,
                            config.quic_max_idle_timeout,
                            config.quic_stream_receive_window,
                            config.quic_receive_window,
                            config.quic_send_window,
                            config.quic_congestion_bbr,
                            config.h3_max_field_section_size,
                            config.h3_send_grease,
                            config.local_address,
                            &config.http_version_pref,
                            config.certs_verification,
                            config.hostname_verification,
                            config.tls_certs_only,
                            &config.root_certs,
                        )?;
                    }

                    let mut main_tls = (*conn).clone();
                    // Preserve ALPN from preconfigured TLS config when browser
                    // emulation is active (impersonation). The caller sets ALPN
                    // to ["h2", "http/1.1"] so the TLS ClientHello includes h2,
                    // which is required for correct JA4 fingerprinting. Without
                    // this check the override to http/1.1-only causes the server
                    // to see h1 in the JA4 fingerprint.
                    //
                    // Exception: if the caller forces HTTP/1 (http1_only), the
                    // connection is driven by the legacy h1 parser, which cannot
                    // decode h2 frames. We must restrict ALPN to http/1.1 even
                    // under impersonation, accepting a slightly less accurate
                    // fingerprint, to avoid handing h2 to the h1 parser.
                    if main_tls.browser_emulation.is_none()
                        || matches!(config.http_version_pref, HttpVersionPref::Http1)
                    {
                        main_tls.alpn_protocols = vec!["http/1.1".into()];
                    }

                    {
                        let mut h2_tls = (*conn).clone();
                        // Preserve ALPN from preconfigured TLS config when
                        // browser emulation is active (impersonation). Real
                        // browsers advertise ["h2", "http/1.1"] in the
                        // ClientHello ALPN extension, not just ["h2"].
                        if h2_tls.browser_emulation.is_none()
                            || matches!(config.http_version_pref, HttpVersionPref::Http1)
                        {
                            // When http_version_pref is All, advertise both
                            // protocols so the server can pick via ALPN.
                            // If the server picks http/1.1, the H2 connector
                            // returns the established stream as
                            // ConnectOutcome::Http1 and negotiate.rs runs
                            // HTTP/1.1 over it without a second handshake.
                            // When the caller forces HTTP/1, only offer
                            // http/1.1 so the legacy h1 parser is used.
                            h2_tls.alpn_protocols = match config.http_version_pref {
                                HttpVersionPref::Http1 => vec!["http/1.1".into()],
                                HttpVersionPref::All => {
                                    vec!["h2".into(), "http/1.1".into()]
                                }
                                _ => vec!["h2".into()],
                            };
                        }
                        // tls_proxy: no ALPN, for TLS-to-proxy connections.
                        let mut h2_tls_proxy = (*conn).clone();
                        h2_tls_proxy.alpn_protocols.clear();
                        h2_connector = Some(H2Connector::new(
                            resolver.clone(),
                            Some(Arc::new(h2_tls)),
                            Some(Arc::new(h2_tls_proxy)),
                            h2_cfg,
                            config.nodelay,
                            config.connect_timeout,
                            proxies.clone(),
                            user_agent(&config.headers),
                            config.tls_info,
                        ));
                    }

                    ConnectorBuilder::new_rustls_tls(
                        http,
                        main_tls,
                        proxies.clone(),
                        user_agent(&config.headers),
                        config.local_address,
                        #[cfg(any(
                            target_os = "android",
                            target_os = "fuchsia",
                            target_os = "illumos",
                            target_os = "ios",
                            target_os = "linux",
                            target_os = "macos",
                            target_os = "solaris",
                            target_os = "tvos",
                            target_os = "visionos",
                            target_os = "watchos",
                        ))]
                        config.interface.as_deref(),
                        config.nodelay,
                        config.tls_info,
                    )
                }
                TlsBackend::Rustls => {
                    use crate::tls::{IgnoreHostname, NoVerifier};

                    // Set TLS versions.
                    let mut versions = rustls::ALL_VERSIONS.to_vec();

                    if let Some(min_tls_version) = config.min_tls_version {
                        versions.retain(|&supported_version| {
                            match tls::Version::from_rustls(supported_version.version) {
                                Some(version) => version >= min_tls_version,
                                // Assume it's so new we don't know about it, allow it
                                // (as of writing this is unreachable)
                                None => true,
                            }
                        });
                    }

                    if let Some(max_tls_version) = config.max_tls_version {
                        versions.retain(|&supported_version| {
                            match tls::Version::from_rustls(supported_version.version) {
                                Some(version) => version <= max_tls_version,
                                None => false,
                            }
                        });
                    }

                    if versions.is_empty() {
                        return Err(crate::error::builder("empty supported tls versions"));
                    }

                    // Allow user to have installed a runtime default.
                    // If not, we ship with _our_ recommended default.
                    let provider = rustls::crypto::CryptoProvider::get_default()
                        .cloned()
                        .unwrap_or_else(default_rustls_crypto_provider);

                    // Build TLS config
                    let signature_algorithms = provider.signature_verification_algorithms;
                    let config_builder =
                        rustls::ClientConfig::builder_with_provider(provider.clone())
                            .with_protocol_versions(&versions)
                            .map_err(|_| crate::error::builder("invalid TLS versions"))?;

                    let config_builder = if !config.certs_verification {
                        config_builder
                            .dangerous()
                            .with_custom_certificate_verifier(Arc::new(NoVerifier))
                    } else if !config.hostname_verification {
                        // Skip only the hostname check: the chain must still
                        // verify against a root store (default when none are
                        // configured — an empty store rejects every handshake).
                        // Mirrors the impersonation path below.
                        let roots: Arc<rustls::RootCertStore> = if config.root_certs.is_empty() {
                            crate::tls::default_root_store_arc()
                        } else {
                            Arc::new(crate::tls::merged_root_store(&config.root_certs)?)
                        };
                        config_builder
                            .dangerous()
                            .with_custom_certificate_verifier(Arc::new(IgnoreHostname::new(
                                (*roots).clone(),
                                signature_algorithms,
                            )))
                    } else if !config.tls_certs_only {
                        // Check for some misconfigurations and report them.
                        if !config.crls.is_empty() {
                            return Err(crate::error::builder(
                                "CRLs only allowed with tls_certs_only()",
                            ));
                        }

                        let roots: Arc<rustls::RootCertStore> = if config.root_certs.is_empty() {
                            crate::tls::default_root_store_arc()
                        } else {
                            Arc::new(crate::tls::merged_root_store(&config.root_certs)?)
                        };

                        config_builder.with_root_certificates(roots)
                    } else if config.crls.is_empty() {
                        config_builder
                            .with_root_certificates(crate::tls::rustls_store(&config.root_certs)?)
                    } else {
                        let crls = config
                            .crls
                            .iter()
                            .map(|e| e.as_rustls_crl())
                            .collect::<Vec<_>>();
                        let verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
                            Arc::new(crate::tls::rustls_store(&config.root_certs)?),
                            provider,
                        )
                        .with_crls(crls)
                        .build()
                        .map_err(|_| crate::error::builder("invalid TLS verification settings"))?;
                        config_builder.with_webpki_verifier(verifier)
                    };

                    // Finalize TLS config
                    let mut tls = if let Some(id) = config.identity {
                        id.add_to_rustls(config_builder)?
                    } else {
                        config_builder.with_no_client_auth()
                    };

                    tls.enable_sni = config.tls_sni;

                    if config.tls_sslkeylogfile {
                        tls.key_log = Arc::new(rustls::KeyLogFile::new());
                    }

                    // ALPN protocol
                    match config.http_version_pref {
                        HttpVersionPref::Http1 => {
                            tls.alpn_protocols = vec!["http/1.1".into()];
                        }
                        HttpVersionPref::Http2 => {
                            tls.alpn_protocols = vec!["h2".into()];
                        }
                        #[cfg(feature = "http3")]
                        HttpVersionPref::Http3 => {
                            // h3 ALPN is not valid over TCP
                        }
                        HttpVersionPref::All => {
                            tls.alpn_protocols = vec!["h2".into(), "http/1.1".into()];
                        }
                    }

                    #[cfg(feature = "http3")]
                    {
                        h3_connector = build_h3_connector(
                            resolver.clone(),
                            config.connect_timeout,
                            config.quic_max_idle_timeout,
                            config.quic_stream_receive_window,
                            config.quic_receive_window,
                            config.quic_send_window,
                            config.quic_congestion_bbr,
                            config.h3_max_field_section_size,
                            config.h3_send_grease,
                            config.local_address,
                            &config.http_version_pref,
                            config.certs_verification,
                            config.hostname_verification,
                            config.tls_certs_only,
                            &config.root_certs,
                        )?;
                    }

                    {
                        let mut h2_tls = tls.clone();
                        // When http_version_pref is All, advertise both
                        // protocols so the server can pick via ALPN.
                        // If the server picks http/1.1, the H2 connector
                        // returns the established stream as
                        // ConnectOutcome::Http1 and negotiate.rs runs HTTP/1.1
                        // over it without a second handshake.
                        h2_tls.alpn_protocols = match config.http_version_pref {
                            HttpVersionPref::All => {
                                vec!["h2".into(), "http/1.1".into()]
                            }
                            _ => vec!["h2".into()],
                        };
                        // tls_proxy: no ALPN, for TLS-to-proxy connections.
                        let mut h2_tls_proxy = tls.clone();
                        h2_tls_proxy.alpn_protocols.clear();
                        h2_connector = Some(H2Connector::new(
                            resolver.clone(),
                            Some(Arc::new(h2_tls)),
                            Some(Arc::new(h2_tls_proxy)),
                            h2_cfg,
                            config.nodelay,
                            config.connect_timeout,
                            proxies.clone(),
                            user_agent(&config.headers),
                            config.tls_info,
                        ));
                    }

                    // Clone the config and force HTTP/1.1 ALPN for the main
                    // connector, so it never negotiates h2 (which the legacy
                    // hyper-based HTTP/1.x parser cannot handle).
                    let mut main_tls = tls.clone();
                    main_tls.alpn_protocols = vec!["http/1.1".into()];

                    ConnectorBuilder::new_rustls_tls(
                        http,
                        main_tls,
                        proxies.clone(),
                        user_agent(&config.headers),
                        config.local_address,
                        #[cfg(any(
                            target_os = "android",
                            target_os = "fuchsia",
                            target_os = "illumos",
                            target_os = "ios",
                            target_os = "linux",
                            target_os = "macos",
                            target_os = "solaris",
                            target_os = "tvos",
                            target_os = "visionos",
                            target_os = "watchos",
                        ))]
                        config.interface.as_deref(),
                        config.nodelay,
                        config.tls_info,
                    )
                }
            }
        };

        connector_builder.set_timeout(config.connect_timeout);
        connector_builder.set_verbose(config.connection_verbose);
        connector_builder.set_keepalive(config.tcp_keepalive);
        connector_builder.set_keepalive_interval(config.tcp_keepalive_interval);
        connector_builder.set_keepalive_retries(config.tcp_keepalive_retries);
        #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
        connector_builder.set_tcp_user_timeout(config.tcp_user_timeout);

        connector_builder.set_socks_resolver(resolver);

        // TODO: It'd be best to refactor this so the HttpConnector is never
        // constructed at all. But there's a lot of code for all the different
        // ways TLS can be configured...
        #[cfg(unix)]
        connector_builder.set_unix_socket(config.unix_socket.clone());
        #[cfg(target_os = "windows")]
        connector_builder.set_windows_named_pipe(config.windows_named_pipe.clone());

        let proxies_maybe_http_auth = proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|p| p.maybe_has_http_auth());
        let proxies_maybe_http_custom_headers = proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|p| p.maybe_has_http_custom_headers());

        let redirect_policy_desc = if config.redirect_policy.is_default() {
            None
        } else {
            Some(format!("{:?}", config.redirect_policy))
        };

        let connector = connector_builder.build(config.connector_layers.clone());

        // Built before any `config` fields are partially moved below.
        let http1_client = build_legacy_http1_client(&legacy_settings, connector.clone());

        let redirect_policy = {
            let mut p = TowerRedirectPolicy::new(config.redirect_policy);
            p.with_referer(config.referer)
                .with_https_only(config.https_only)
                .with_proxies(proxies.clone());
            p
        };

        let retry_policy = config.retry_policy.into_policy();

        let h2_pool = {
            let max_streams = config
                .http2_max_concurrent_streams
                .map(|v| v as usize)
                .unwrap_or(DEFAULT_H2_MAX_CONCURRENT_STREAMS);
            let mut pool = Pool::new(
                config.pool_idle_timeout,
                config.pool_max_connections,
                max_streams,
            );
            pool.spawn_idle_cleanup();
            pool
        };

        let h2_connector =
            h2_connector.ok_or_else(|| crate::error::builder("h2 connector is required"))?;

        let connection = NegotiatingConnection {
            h2_pool,
            h2_connector: Arc::new(h2_connector),
            http1_pool: Http1Pool::with_idle_timeout(
                config.pool_max_connections,
                config.pool_idle_timeout,
            ),
            http1_client,
            http_version_pref: config.http_version_pref,
        };

        let svc = tower::retry::Retry::new(retry_policy.clone(), connection);

        #[cfg(feature = "cookies")]
        let svc = CookieService::new(svc, config.cookie_store.clone());
        let svc = FollowRedirect::with_policy(svc, redirect_policy.clone());
        #[cfg(any(
            feature = "gzip",
            feature = "brotli",
            feature = "zstd",
            feature = "deflate"
        ))]
        let svc = Decompression::new(svc)
            // set everything to NO, in case tower-http has it enabled but
            // primp does not. then set to config value if cfg allows.
            .no_gzip()
            .no_deflate()
            .no_br()
            .no_zstd();
        #[cfg(feature = "gzip")]
        let svc = svc.gzip(config.accepts.gzip);
        #[cfg(feature = "brotli")]
        let svc = svc.br(config.accepts.brotli);
        #[cfg(feature = "zstd")]
        let svc = svc.zstd(config.accepts.zstd);
        #[cfg(feature = "deflate")]
        let svc = svc.deflate(config.accepts.deflate);
        #[cfg(any(
            feature = "gzip",
            feature = "brotli",
            feature = "zstd",
            feature = "deflate"
        ))]
        let svc = range_guard::RangeGuard::new(svc);

        Ok(Client {
            inner: Arc::new(ClientRef {
                accepts: config.accepts,
                #[cfg(feature = "cookies")]
                cookie_store: config.cookie_store.clone(),
                #[cfg(feature = "http3")]
                h3_client: match h3_connector {
                    Some(h3_connector) => {
                        let h3_service = H3Client::new(h3_connector, config.pool_idle_timeout);
                        let svc = tower::retry::Retry::new(retry_policy.clone(), h3_service);
                        #[cfg(feature = "cookies")]
                        let svc = CookieService::new(svc, config.cookie_store.clone());
                        let svc = FollowRedirect::with_policy(svc, redirect_policy.clone());
                        #[cfg(any(
                            feature = "gzip",
                            feature = "brotli",
                            feature = "zstd",
                            feature = "deflate"
                        ))]
                        let svc = Decompression::new(svc)
                            // set everything to NO, in case tower-http has it enabled but
                            // primp does not. then set to config value if cfg allows.
                            .no_gzip()
                            .no_deflate()
                            .no_br()
                            .no_zstd();
                        #[cfg(feature = "gzip")]
                        let svc = svc.gzip(config.accepts.gzip);
                        #[cfg(feature = "brotli")]
                        let svc = svc.br(config.accepts.brotli);
                        #[cfg(feature = "zstd")]
                        let svc = svc.zstd(config.accepts.zstd);
                        #[cfg(feature = "deflate")]
                        let svc = svc.deflate(config.accepts.deflate);
                        #[cfg(any(
                            feature = "gzip",
                            feature = "brotli",
                            feature = "zstd",
                            feature = "deflate"
                        ))]
                        let svc = range_guard::RangeGuard::new(svc);
                        Some(svc)
                    }
                    None => None,
                },
                headers: config.headers,
                service: svc,
                #[cfg(feature = "http3")]
                http_version_pref: config.http_version_pref,
                referer: config.referer,
                read_timeout: RequestConfig::new(config.read_timeout),
                total_timeout: RequestConfig::new(config.timeout),
                proxies,
                proxies_maybe_http_auth,
                proxies_maybe_http_custom_headers,
                https_only: config.https_only,
                redirect_policy_desc,
                redirect_policy,
            }),
        })
    }

    /// Sets the browser to impersonate.
    pub fn impersonate(mut self, version: crate::imp::Impersonate) -> Self {
        self.config.impersonate = Some(version);
        self
    }

    /// Sets the OS to impersonate.
    ///
    /// Without [`impersonate`](Self::impersonate), `build()` pairs it with
    /// a **random** browser — the fingerprint differs per build.
    pub fn impersonate_os(mut self, os: crate::imp::ImpersonateOS) -> Self {
        self.config.os_type = Some(os);
        self
    }

    /// Clears impersonation fields so that `build()` takes the normal path.
    /// Called internally after impersonation settings have been applied.
    pub(crate) fn clear_impersonation(mut self) -> Self {
        self.config.impersonate = None;
        self.config.os_type = None;
        self
    }

    // Higher-level options

    /// Sets the `User-Agent` header to be used by this client.
    ///
    /// # Example
    ///
    /// ```rust
    /// # async fn doc() -> Result<(), primp::Error> {
    /// // Name your user agent after your app?
    /// static APP_USER_AGENT: &str = concat!(
    ///     env!("CARGO_PKG_NAME"),
    ///     "/",
    ///     env!("CARGO_PKG_VERSION"),
    /// );
    ///
    /// let client = primp::Client::builder()
    ///     .user_agent(APP_USER_AGENT)
    ///     .build()?;
    /// let res = client.get("https://www.rust-lang.org").send().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn user_agent<V>(mut self, value: V) -> ClientBuilder
    where
        V: TryInto<HeaderValue>,
        V::Error: Into<http::Error>,
    {
        match value.try_into() {
            Ok(value) => {
                self.config.headers.insert(USER_AGENT, value);
            }
            Err(e) => {
                self.config.error = Some(crate::error::builder(e.into()));
            }
        };
        self
    }
    /// Sets the default headers for every request.
    ///
    /// # Example
    ///
    /// ```rust
    /// use primp::header;
    /// # async fn doc() -> Result<(), primp::Error> {
    /// let mut headers = header::HeaderMap::new();
    /// headers.insert("X-MY-HEADER", header::HeaderValue::from_static("value"));
    ///
    /// // Consider marking security-sensitive headers with `set_sensitive`.
    /// let mut auth_value = header::HeaderValue::from_static("secret");
    /// auth_value.set_sensitive(true);
    /// headers.insert(header::AUTHORIZATION, auth_value);
    ///
    /// // get a client builder
    /// let client = primp::Client::builder()
    ///     .default_headers(headers)
    ///     .build()?;
    /// let res = client.get("https://www.rust-lang.org").send().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn default_headers(mut self, headers: HeaderMap) -> ClientBuilder {
        for (key, value) in headers.iter() {
            self.config.headers.insert(key, value.clone());
        }
        self
    }

    /// Enable a persistent cookie store for the client.
    ///
    /// Cookies received in responses will be preserved and included in
    /// additional requests.
    ///
    /// By default, no cookie store is used. Enabling the cookie store
    /// with `cookie_store(true)` will set the store to a default implementation.
    /// It is **not** necessary to call [cookie_store(true)](crate::ClientBuilder::cookie_store) if [cookie_provider(my_cookie_store)](crate::ClientBuilder::cookie_provider)
    /// is used; calling [cookie_store(true)](crate::ClientBuilder::cookie_store) _after_ [cookie_provider(my_cookie_store)](crate::ClientBuilder::cookie_provider) will result
    /// in the provided `my_cookie_store` being **overridden** with a default implementation.
    ///
    /// # Optional
    ///
    /// This requires the optional `cookies` feature to be enabled.
    #[cfg(feature = "cookies")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cookies")))]
    pub fn cookie_store(mut self, enable: bool) -> ClientBuilder {
        if enable {
            self.cookie_provider(Arc::new(cookie::Jar::default()))
        } else {
            self.config.cookie_store = None;
            self
        }
    }

    /// Set the persistent cookie store for the client.
    ///
    /// Cookies received in responses will be passed to this store, and
    /// additional requests will query this store for cookies.
    ///
    /// By default, no cookie store is used. It is **not** necessary to also call
    /// [cookie_store(true)](crate::ClientBuilder::cookie_store) if [cookie_provider(my_cookie_store)](crate::ClientBuilder::cookie_provider) is used; calling
    /// [cookie_store(true)](crate::ClientBuilder::cookie_store) _after_ [cookie_provider(my_cookie_store)](crate::ClientBuilder::cookie_provider) will result
    /// in the provided `my_cookie_store` being **overridden** with a default implementation.
    ///
    /// # Optional
    ///
    /// This requires the optional `cookies` feature to be enabled.
    #[cfg(feature = "cookies")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cookies")))]
    pub fn cookie_provider<C: cookie::CookieStore + 'static>(
        mut self,
        cookie_store: Arc<C>,
    ) -> ClientBuilder {
        self.config.cookie_store = Some(cookie_store as _);
        self
    }

    /// Enable auto gzip decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto gzip decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `gzip`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if its headers contain a `Content-Encoding` value of
    ///   `gzip`, both `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `gzip` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `gzip` feature to be enabled
    #[cfg(feature = "gzip")]
    #[cfg_attr(docsrs, doc(cfg(feature = "gzip")))]
    pub fn gzip(mut self, enable: bool) -> ClientBuilder {
        self.config.accepts.gzip = enable;
        self
    }

    /// Enable auto brotli decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto brotli decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `br`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if its headers contain a `Content-Encoding` value of
    ///   `br`, both `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `brotli` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `brotli` feature to be enabled
    #[cfg(feature = "brotli")]
    #[cfg_attr(docsrs, doc(cfg(feature = "brotli")))]
    pub fn brotli(mut self, enable: bool) -> ClientBuilder {
        self.config.accepts.brotli = enable;
        self
    }

    /// Enable auto zstd decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto zstd decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `zstd`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if its headers contain a `Content-Encoding` value of
    ///   `zstd`, both `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `zstd` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `zstd` feature to be enabled
    #[cfg(feature = "zstd")]
    #[cfg_attr(docsrs, doc(cfg(feature = "zstd")))]
    pub fn zstd(mut self, enable: bool) -> ClientBuilder {
        self.config.accepts.zstd = enable;
        self
    }

    /// Enable auto deflate decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto deflate decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `deflate`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if it's headers contain a `Content-Encoding` value that
    ///   equals to `deflate`, both values `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `deflate` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `deflate` feature to be enabled
    #[cfg(feature = "deflate")]
    #[cfg_attr(docsrs, doc(cfg(feature = "deflate")))]
    pub fn deflate(mut self, enable: bool) -> ClientBuilder {
        self.config.accepts.deflate = enable;
        self
    }

    /// Disable auto response body gzip decompression.
    ///
    /// This method exists even if the optional `gzip` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use gzip decompression
    /// even if another dependency were to enable the optional `gzip` feature.
    pub fn no_gzip(self) -> ClientBuilder {
        #[cfg(feature = "gzip")]
        {
            self.gzip(false)
        }

        #[cfg(not(feature = "gzip"))]
        {
            self
        }
    }

    /// Disable auto response body brotli decompression.
    ///
    /// This method exists even if the optional `brotli` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use brotli decompression
    /// even if another dependency were to enable the optional `brotli` feature.
    pub fn no_brotli(self) -> ClientBuilder {
        #[cfg(feature = "brotli")]
        {
            self.brotli(false)
        }

        #[cfg(not(feature = "brotli"))]
        {
            self
        }
    }

    /// Disable auto response body zstd decompression.
    ///
    /// This method exists even if the optional `zstd` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use zstd decompression
    /// even if another dependency were to enable the optional `zstd` feature.
    pub fn no_zstd(self) -> ClientBuilder {
        #[cfg(feature = "zstd")]
        {
            self.zstd(false)
        }

        #[cfg(not(feature = "zstd"))]
        {
            self
        }
    }

    /// Disable auto response body deflate decompression.
    ///
    /// This method exists even if the optional `deflate` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use deflate decompression
    /// even if another dependency were to enable the optional `deflate` feature.
    pub fn no_deflate(self) -> ClientBuilder {
        #[cfg(feature = "deflate")]
        {
            self.deflate(false)
        }

        #[cfg(not(feature = "deflate"))]
        {
            self
        }
    }

    // Redirect options

    /// Set a `RedirectPolicy` for this client.
    ///
    /// Default will follow redirects up to a maximum of 10.
    pub fn redirect(mut self, policy: redirect::Policy) -> ClientBuilder {
        self.config.redirect_policy = policy;
        self
    }

    /// Enable or disable automatic setting of the `Referer` header.
    ///
    /// Default is `true`.
    pub fn referer(mut self, enable: bool) -> ClientBuilder {
        self.config.referer = enable;
        self
    }

    // Retry options

    /// Set a request retry policy.
    ///
    /// Default behavior is to retry protocol NACKs.
    // XXX: accept an `impl retry::IntoPolicy` instead?
    pub fn retry(mut self, policy: crate::retry::Builder) -> ClientBuilder {
        self.config.retry_policy = policy;
        self
    }

    // Proxy options

    /// Add a `Proxy` to the list of proxies the `Client` will use.
    ///
    /// # Note
    ///
    /// Adding a proxy will disable the automatic usage of the "system" proxy.
    pub fn proxy(mut self, proxy: Proxy) -> ClientBuilder {
        self.config.proxies.push(proxy.into_matcher());
        self.config.auto_sys_proxy = false;
        self
    }

    /// Clear all `Proxies`, so `Client` will use no proxy anymore.
    ///
    /// # Note
    /// To add a proxy exclusion list, use [crate::proxy::Proxy::no_proxy()]
    /// on all desired proxies instead.
    ///
    /// This also disables the automatic usage of the "system" proxy.
    pub fn no_proxy(mut self) -> ClientBuilder {
        self.config.proxies.clear();
        self.config.auto_sys_proxy = false;
        self
    }

    // Timeout options

    /// Enables a total request timeout.
    ///
    /// The timeout is applied from when the request starts connecting until the
    /// response body has finished. Also considered a total deadline.
    ///
    /// Default is 30 seconds.
    pub fn timeout(mut self, timeout: Duration) -> ClientBuilder {
        self.config.timeout = Some(timeout);
        self
    }

    /// Enables a read timeout.
    ///
    /// The timeout applies to each read operation, and resets after a
    /// successful read. This is more appropriate for detecting stalled
    /// connections when the size isn't known beforehand.
    ///
    /// Default is no timeout.
    pub fn read_timeout(mut self, timeout: Duration) -> ClientBuilder {
        self.config.read_timeout = Some(timeout);
        self
    }

    /// Set a timeout for only the connect phase of a `Client`.
    ///
    /// Default is 30 seconds.
    ///
    /// # Note
    ///
    /// This **requires** the futures be executed in a tokio runtime with
    /// a tokio timer enabled.
    pub fn connect_timeout(mut self, timeout: Duration) -> ClientBuilder {
        self.config.connect_timeout = Some(timeout);
        self
    }

    /// Set the timeout for a single DNS resolution attempt (default 30s).
    ///
    /// Capped just below `connect_timeout` so a hanging lookup is reported as
    /// a DNS error, never an untagged connect timeout.
    pub fn dns_timeout(mut self, timeout: Duration) -> ClientBuilder {
        self.config.dns_timeout = Some(timeout);
        self
    }

    /// Set whether connections should emit verbose logs.
    ///
    /// Enabling this option will emit [log][] messages at the `TRACE` level
    /// for read and write operations on connections.
    ///
    /// [log]: https://crates.io/crates/log
    pub fn connection_verbose(mut self, verbose: bool) -> ClientBuilder {
        self.config.connection_verbose = verbose;
        self
    }

    // HTTP options

    /// Set an optional timeout for idle sockets being kept-alive.
    ///
    /// Pass `None` to disable timeout.
    ///
    /// Default is 90 seconds.
    pub fn pool_idle_timeout<D>(mut self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.config.pool_idle_timeout = val.into();
        self
    }

    /// Sets the maximum idle connection per host allowed in the pool.
    ///
    /// Default is `usize::MAX` (no limit).
    pub fn pool_max_idle_per_host(mut self, max: usize) -> ClientBuilder {
        self.config.pool_max_idle_per_host = max;
        self
    }

    /// Sets the maximum number of H2 connections to keep in the pool.
    ///
    /// When the pool is full, the oldest idle connection is evicted to make
    /// room for a new one.
    ///
    /// Default is 256.
    pub fn pool_max_connections(mut self, max: usize) -> ClientBuilder {
        self.config.pool_max_connections = max.max(1);
        self
    }

    /// Send headers as title case instead of lowercase.
    pub fn http1_title_case_headers(mut self) -> ClientBuilder {
        self.config.http1_title_case_headers = true;
        self
    }

    /// Set whether HTTP/1 connections will accept obsolete line folding for
    /// header values.
    ///
    /// Newline codepoints (`\r` and `\n`) will be transformed to spaces when
    /// parsing.
    pub fn http1_allow_obsolete_multiline_headers_in_responses(
        mut self,
        value: bool,
    ) -> ClientBuilder {
        self.config
            .http1_allow_obsolete_multiline_headers_in_responses = value;
        self
    }

    /// Sets whether invalid header lines should be silently ignored in HTTP/1 responses.
    pub fn http1_ignore_invalid_headers_in_responses(mut self, value: bool) -> ClientBuilder {
        self.config.http1_ignore_invalid_headers_in_responses = value;
        self
    }

    /// Set whether HTTP/1 connections will accept spaces between header
    /// names and the colon that follow them in responses.
    ///
    /// Newline codepoints (`\r` and `\n`) will be transformed to spaces when
    /// parsing.
    pub fn http1_allow_spaces_after_header_name_in_responses(
        mut self,
        value: bool,
    ) -> ClientBuilder {
        self.config
            .http1_allow_spaces_after_header_name_in_responses = value;
        self
    }

    /// Set the maximum number of headers accepted in an HTTP/1 response.
    ///
    /// When a response contains more headers than this value, it is rejected
    /// with a parse error and the request fails.
    ///
    /// Default is 100.
    pub fn http1_max_headers(mut self, max: usize) -> ClientBuilder {
        self.config.http1_max_headers = Some(max);
        self
    }

    /// Only use HTTP/1.
    pub fn http1_only(mut self) -> ClientBuilder {
        self.config.http_version_pref = HttpVersionPref::Http1;
        self
    }

    /// Allow HTTP/0.9 responses
    pub fn http09_responses(mut self) -> ClientBuilder {
        self.config.http09_responses = true;
        self
    }

    /// Only use HTTP/2.
    pub fn http2_prior_knowledge(mut self) -> ClientBuilder {
        self.config.http_version_pref = HttpVersionPref::Http2;
        self
    }

    /// Only use HTTP/3.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_prior_knowledge(mut self) -> ClientBuilder {
        self.config.http_version_pref = HttpVersionPref::Http3;
        self
    }

    /// Sets the `SETTINGS_INITIAL_WINDOW_SIZE` option for HTTP2 stream-level flow control.
    ///
    /// Default may change internally to optimize for common uses.
    pub fn http2_initial_stream_window_size(mut self, sz: impl Into<Option<u32>>) -> ClientBuilder {
        self.config.http2_initial_stream_window_size = sz.into();
        self
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Default may change internally to optimize for common uses.
    pub fn http2_initial_connection_window_size(
        mut self,
        sz: impl Into<Option<u32>>,
    ) -> ClientBuilder {
        self.config.http2_initial_connection_window_size = sz.into();
        self
    }

    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in `http2_initial_stream_window_size` and
    /// `http2_initial_connection_window_size`.
    pub fn http2_adaptive_window(mut self, enabled: bool) -> ClientBuilder {
        self.config.http2_adaptive_window = enabled;
        self
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Default is currently 16,384 but may change internally to optimize for common uses.
    pub fn http2_max_frame_size(mut self, sz: impl Into<Option<u32>>) -> ClientBuilder {
        self.config.http2_max_frame_size = sz.into();
        self
    }

    /// Sets the maximum size of received header frames for HTTP2.
    ///
    /// Default is currently 16KB, but can change.
    pub fn http2_max_header_list_size(mut self, max_header_size_bytes: u32) -> ClientBuilder {
        self.config.http2_max_header_list_size = Some(max_header_size_bytes);
        self
    }

    /// Sets an interval for HTTP2 Ping frames should be sent to keep a connection alive.
    ///
    /// Pass `None` to disable HTTP2 keep-alive.
    /// Default is currently disabled.
    pub fn http2_keep_alive_interval(
        mut self,
        interval: impl Into<Option<Duration>>,
    ) -> ClientBuilder {
        self.config.http2_keep_alive_interval = interval.into();
        self
    }

    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will be closed.
    /// Does nothing if `http2_keep_alive_interval` is disabled.
    /// Default is currently disabled.
    pub fn http2_keep_alive_timeout(mut self, timeout: Duration) -> ClientBuilder {
        self.config.http2_keep_alive_timeout = Some(timeout);
        self
    }

    /// Sets whether HTTP2 keep-alive should apply while the connection is idle.
    ///
    /// If disabled, keep-alive pings are only sent while there are open request/responses streams.
    /// If enabled, pings are also sent when no streams are active.
    /// Does nothing if `http2_keep_alive_interval` is disabled.
    /// Default is `false`.
    pub fn http2_keep_alive_while_idle(mut self, enabled: bool) -> ClientBuilder {
        self.config.http2_keep_alive_while_idle = enabled;
        self
    }

    /// Sets the HTTP/2 SETTINGS_HEADER_TABLE_SIZE value.
    pub fn http2_header_table_size(mut self, size: impl Into<Option<u32>>) -> ClientBuilder {
        self.config.http2_header_table_size = size.into();
        self
    }

    /// Sets the HTTP/2 SETTINGS_MAX_CONCURRENT_STREAMS value.
    pub fn http2_max_concurrent_streams(mut self, max: impl Into<Option<u32>>) -> ClientBuilder {
        self.config.http2_max_concurrent_streams = max.into();
        self
    }

    /// Sets the HTTP/2 SETTINGS_ENABLE_PUSH value.
    pub fn http2_enable_push(mut self, enabled: impl Into<Option<bool>>) -> ClientBuilder {
        self.config.http2_enable_push = enabled.into();
        self
    }

    /// Sets the HTTP/2 SETTINGS_NO_RFC7540_PRIORITIES value.
    pub fn http2_no_rfc7540_priorities(
        mut self,
        enabled: impl Into<Option<bool>>,
    ) -> ClientBuilder {
        self.config.http2_no_rfc7540_priorities = enabled.into();
        self
    }

    /// Sets the HTTP/2 SETTINGS_ENABLE_CONNECT_PROTOCOL value.
    pub fn http2_enable_connect_protocol(mut self, val: impl Into<Option<u32>>) -> ClientBuilder {
        self.config.http2_enable_connect_protocol = val.into();
        self
    }

    /// Sets the HTTP/2 SETTINGS frame order for fingerprinting.
    pub fn http2_settings_order(mut self, order: h2::frame::SettingsOrder) -> ClientBuilder {
        self.config.http2_settings_order = Some(order);
        self
    }

    /// Sets the HTTP/2 pseudo-header order for fingerprinting.
    pub fn http2_headers_pseudo_order(mut self, order: h2::frame::PseudoOrder) -> ClientBuilder {
        self.config.http2_headers_pseudo_order = Some(order);
        self
    }

    /// Sets whether to include PRIORITY flag in HTTP/2 HEADERS frames.
    ///
    /// When enabled, HEADERS frames will include priority data matching Chrome's behavior.
    pub fn http2_headers_priority(mut self, data: Option<(u8, u32, bool)>) -> ClientBuilder {
        self.config.http2_headers_priority = data;
        self
    }

    /// Sets the HTTP/2 regular header ordering for browser fingerprinting.
    pub fn http2_headers_order(mut self, order: Vec<http::HeaderName>) -> ClientBuilder {
        self.config.http2_headers_order = Some(order);
        self
    }

    /// Sets the initial HTTP/2 stream ID for browser fingerprinting.
    ///
    /// Firefox starts at stream ID 3, Chrome starts at 1 (default).
    pub fn http2_initial_stream_id(mut self, stream_id: u32) -> ClientBuilder {
        self.config.http2_initial_stream_id = Some(stream_id);
        self
    }

    /// Sets extra receive window capacity to add to new locally-initiated streams.
    ///
    /// When set, after creating a new stream, a WINDOW_UPDATE frame will be sent
    /// to increase the stream's receive window by this amount.
    /// This is used for browser fingerprinting (e.g. Firefox adds 12451840
    /// to the first stream's receive window).
    pub fn http2_initial_stream_window_size_increment(
        mut self,
        sz: impl Into<Option<u32>>,
    ) -> ClientBuilder {
        self.config.http2_initial_stream_window_size_increment = sz.into();
        self
    }

    // TCP options

    /// Set whether sockets have `TCP_NODELAY` enabled.
    ///
    /// Default is `true`.
    pub fn tcp_nodelay(mut self, enabled: bool) -> ClientBuilder {
        self.config.nodelay = enabled;
        self
    }

    /// Bind to a local IP Address.
    ///
    /// # Example
    ///
    /// ```
    /// # fn doc() -> Result<(), primp::Error> {
    /// use std::net::IpAddr;
    /// let local_addr = IpAddr::from([12, 4, 1, 8]);
    /// let client = primp::Client::builder()
    ///     .local_address(local_addr)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn local_address<T>(mut self, addr: T) -> ClientBuilder
    where
        T: Into<Option<IpAddr>>,
    {
        self.config.local_address = addr.into();
        self
    }

    /// Bind connections only on the specified network interface.
    ///
    /// This option is only available on the following operating systems:
    ///
    /// - Android
    /// - Fuchsia
    /// - Linux,
    /// - macOS and macOS-like systems (iOS, tvOS, watchOS and visionOS)
    /// - Solaris and illumos
    ///
    /// On Android, Linux, and Fuchsia, this uses the
    /// [`SO_BINDTODEVICE`][man-7-socket] socket option. On macOS and macOS-like
    /// systems, Solaris, and illumos, this instead uses the [`IP_BOUND_IF` and
    /// `IPV6_BOUND_IF`][man-7p-ip] socket options (as appropriate).
    ///
    /// Note that connections will fail if the provided interface name is not a
    /// network interface that currently exists when a connection is established.
    ///
    /// # Example
    ///
    /// ```
    /// # fn doc() -> Result<(), primp::Error> {
    /// let interface = "lo";
    /// let client = primp::Client::builder()
    ///     .interface(interface)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [man-7-socket]: https://man7.org/linux/man-pages/man7/socket.7.html
    /// [man-7p-ip]: https://docs.oracle.com/cd/E86824_01/html/E54777/ip-7p.html
    #[cfg(any(
        target_os = "android",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "solaris",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    ))]
    pub fn interface(mut self, interface: &str) -> ClientBuilder {
        self.config.interface = Some(interface.to_string());
        self
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied duration.
    ///
    /// If `None`, the option will not be set.
    pub fn tcp_keepalive<D>(mut self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.config.tcp_keepalive = val.into();
        self
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied interval.
    ///
    /// If `None`, the option will not be set.
    pub fn tcp_keepalive_interval<D>(mut self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.config.tcp_keepalive_interval = val.into();
        self
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied retry count.
    ///
    /// If `None`, the option will not be set.
    pub fn tcp_keepalive_retries<C>(mut self, retries: C) -> ClientBuilder
    where
        C: Into<Option<u32>>,
    {
        self.config.tcp_keepalive_retries = retries.into();
        self
    }

    /// Set that all sockets have `TCP_USER_TIMEOUT` set with the supplied duration.
    ///
    /// This option controls how long transmitted data may remain unacknowledged before
    /// the connection is force-closed.
    ///
    /// If `None`, the option will not be set.
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    pub fn tcp_user_timeout<D>(mut self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.config.tcp_user_timeout = val.into();
        self
    }

    // Alt Transports

    /// Set that all connections will use this Unix socket.
    ///
    /// If a request URI uses the `https` scheme, TLS will still be used over
    /// the Unix socket.
    ///
    /// # Note
    ///
    /// This option is not compatible with any of the TCP or Proxy options.
    /// Setting this will ignore all those options previously set.
    ///
    /// Likewise, DNS resolution will not be done on the domain name.
    #[cfg(unix)]
    pub fn unix_socket(mut self, path: impl UnixSocketProvider) -> ClientBuilder {
        self.config.unix_socket = Some(path.primp_uds_path(crate::connect::uds::Internal).into());
        self
    }

    /// Set that all connections will use this Windows named pipe.
    ///
    /// If a request URI uses the `https` scheme, TLS will still be used over
    /// the Windows named pipe.
    ///
    /// # Note
    ///
    /// This option is not compatible with any of the TCP or Proxy options.
    /// Setting this will ignore all those options previously set.
    ///
    /// Likewise, DNS resolution will not be done on the domain name.
    #[cfg(target_os = "windows")]
    pub fn windows_named_pipe(mut self, pipe: impl WindowsNamedPipeProvider) -> ClientBuilder {
        self.config.windows_named_pipe = Some(
            pipe.primp_windows_named_pipe_path(crate::connect::windows_named_pipe::Internal)
                .into(),
        );
        self
    }

    // TLS options

    /// Add custom certificate roots.
    ///
    /// This can be used to connect to a server that has a self-signed
    /// certificate for example.
    ///
    /// This optional attempts to merge with any native or built-in roots.
    ///
    /// # Errors
    ///
    /// If the selected TLS backend or verifier does not support merging
    /// certificates, the builder will return an error.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_certs_merge(
        mut self,
        certs: impl IntoIterator<Item = Certificate>,
    ) -> ClientBuilder {
        self.config.root_certs.extend(certs);
        self
    }

    /// Use only the provided certificate roots.
    ///
    /// This can be used to connect to a server that has a self-signed
    /// certificate for example.
    ///
    /// This option disables any native or built-in roots, and **only** uses
    /// the roots provided to this method.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_certs_only(mut self, certs: impl IntoIterator<Item = Certificate>) -> ClientBuilder {
        self.config.root_certs.extend(certs);
        self.config.tls_certs_only = true;
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_certs_merge()`] or
    /// [`ClientBuilder::tls_certs_only()`] instead.
    pub fn add_root_certificate(mut self, cert: Certificate) -> ClientBuilder {
        self.config.root_certs.push(cert);
        self
    }

    /// Add multiple certificate revocation lists.
    ///
    /// # Errors
    ///
    /// This only works if also using only provided root certificates. This
    /// cannot work with the native verifier.
    ///
    /// If CRLs are added but `tls_certs_only()` is not called, the builder
    /// will return an error.
    ///
    /// # Optional
    ///
    /// This requires the `rustls(-...)` Cargo feature enabled.
    pub fn tls_crls_only(
        mut self,
        crls: impl IntoIterator<Item = CertificateRevocationList>,
    ) -> ClientBuilder {
        self.config.crls.extend(crls);
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_crls_only()`] instead.
    pub fn add_crl(mut self, crl: CertificateRevocationList) -> ClientBuilder {
        self.config.crls.push(crl);
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_crls_only()`] instead.
    pub fn add_crls(
        mut self,
        crls: impl IntoIterator<Item = CertificateRevocationList>,
    ) -> ClientBuilder {
        self.config.crls.extend(crls);
        self
    }

    /// Sets the identity to be used for client certificate authentication.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)` feature to be
    /// enabled.
    pub fn identity(mut self, identity: Identity) -> ClientBuilder {
        self.config.identity = Some(identity);
        self
    }

    /// Controls the use of hostname verification.
    ///
    /// Defaults to `false`.
    ///
    /// # Warning
    ///
    /// You should think very carefully before you use this method. If
    /// hostname verification is not used, any valid certificate for any
    /// site will be trusted for use from any other. This introduces a
    /// significant vulnerability to man-in-the-middle attacks.
    ///
    /// # Errors
    ///
    /// Depending on the TLS backend and verifier, this might not work with
    /// native certificates, only those added with [`ClientBuilder::tls_certs_only()`].
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_danger_accept_invalid_hostnames(
        mut self,
        accept_invalid_hostname: bool,
    ) -> ClientBuilder {
        self.config.hostname_verification = !accept_invalid_hostname;
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_danger_accept_invalid_hostnames()`] instead.
    pub fn danger_accept_invalid_hostnames(self, accept_invalid_hostname: bool) -> ClientBuilder {
        self.tls_danger_accept_invalid_hostnames(accept_invalid_hostname)
    }

    /// Controls the use of certificate validation.
    ///
    /// Defaults to `false`.
    ///
    /// # Warning
    ///
    /// You should think very carefully before using this method. If
    /// invalid certificates are trusted, *any* certificate for *any* site
    /// will be trusted for use. This includes expired certificates. This
    /// introduces significant vulnerabilities, and should only be used
    /// as a last resort.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_danger_accept_invalid_certs(mut self, accept_invalid_certs: bool) -> ClientBuilder {
        self.config.certs_verification = !accept_invalid_certs;
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_danger_accept_invalid_certs()`] instead.
    pub fn danger_accept_invalid_certs(self, accept_invalid_certs: bool) -> ClientBuilder {
        self.tls_danger_accept_invalid_certs(accept_invalid_certs)
    }

    /// Controls the use of TLS server name indication.
    ///
    /// Defaults to `true`.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_sni(mut self, tls_sni: bool) -> ClientBuilder {
        self.config.tls_sni = tls_sni;
        self
    }

    /// Controls if the SSLKEYLOGFILE environment variable is respected.
    ///
    /// When enabled, if the environment variable `SSLKEYLOGFILE` is present at runtime,
    /// TLS keys will be logged to the file at the path described in the variable.
    /// This can be used by end-users to allow debugging TLS connections.
    ///
    /// Defaults to `false`.
    ///
    /// # Optional
    ///
    /// This requires the `rustls(-...)` Cargo feature enabled.
    pub fn tls_sslkeylogfile(mut self, on: bool) -> ClientBuilder {
        self.config.tls_sslkeylogfile = on;
        self
    }

    /// Set the minimum required TLS version for connections.
    ///
    /// By default, the TLS backend's own default is used.
    ///
    /// # Errors
    ///
    /// A value of `tls::Version::TLS_1_3` may cause an error if
    /// the version isn't supported by the backend.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_version_min(mut self, version: tls::Version) -> ClientBuilder {
        self.config.min_tls_version = Some(version);
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_version_min()`] instead.
    pub fn min_tls_version(self, version: tls::Version) -> ClientBuilder {
        self.tls_version_min(version)
    }

    /// Set the maximum allowed TLS version for connections.
    ///
    /// By default, there's no maximum.
    ///
    /// # Errors
    ///
    /// A value of `tls::Version::TLS_1_3` may cause an error if
    /// the version isn't supported by the backend.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_version_max(mut self, version: tls::Version) -> ClientBuilder {
        self.config.max_tls_version = Some(version);
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_version_max()`] instead.
    pub fn max_tls_version(self, version: tls::Version) -> ClientBuilder {
        self.tls_version_max(version)
    }

    /// Force using the Rustls TLS backend.
    ///
    /// Since multiple TLS backends can be optionally enabled, this option will
    /// force the `rustls` backend to be used for this `Client`.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)` feature to be enabled.
    pub fn tls_backend_rustls(mut self) -> ClientBuilder {
        self.config.tls = TlsBackend::Rustls;
        self
    }

    /// Deprecated: use [`ClientBuilder::tls_backend_rustls()`] instead.
    pub fn use_rustls_tls(self) -> ClientBuilder {
        self.tls_backend_rustls()
    }

    /// Use a preconfigured `rustls::ClientConfig` as-is for every TLS
    /// connection, replacing the builder's own TLS settings.
    ///
    /// Advanced: internals carry no semver stability — prefer the typed
    /// `ClientBuilder` TLS methods when possible.
    pub fn use_preconfigured_tls(mut self, tls: rustls::ClientConfig) -> ClientBuilder {
        self.config.tls = crate::tls::TlsBackend::BuiltRustls(Box::new(tls));
        self
    }

    /// Add TLS information as `TlsInfo` extension to responses.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls(-...)`
    /// feature to be enabled.
    pub fn tls_info(mut self, tls_info: bool) -> ClientBuilder {
        self.config.tls_info = tls_info;
        self
    }

    /// Restrict the Client to be used with HTTPS only requests.
    ///
    /// Defaults to false.
    pub fn https_only(mut self, enabled: bool) -> ClientBuilder {
        self.config.https_only = enabled;
        self
    }

    /// Enables the [hickory-dns](hickory_resolver) async resolver instead of a default threadpool
    /// using `getaddrinfo`.
    ///
    /// If the `hickory-dns` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `hickory-dns` feature to be enabled
    ///
    /// # Warning
    ///
    /// The hickory resolver does not work exactly the same, or on all the platforms
    /// that the default resolver does
    #[cfg(feature = "hickory-dns")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hickory-dns")))]
    pub fn hickory_dns(mut self, enable: bool) -> ClientBuilder {
        self.config.hickory_dns = enable;
        self
    }

    /// Disables the hickory-dns async resolver.
    ///
    /// This method exists even if the optional `hickory-dns` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use the hickory-dns async resolver
    /// even if another dependency were to enable the optional `hickory-dns` feature.
    pub fn no_hickory_dns(self) -> ClientBuilder {
        #[cfg(feature = "hickory-dns")]
        {
            self.hickory_dns(false)
        }

        #[cfg(not(feature = "hickory-dns"))]
        {
            self
        }
    }

    /// Override DNS resolution for specific domains to a particular IP address.
    ///
    /// Set the port to `0` to use the conventional port for the given scheme (e.g. 80 for http).
    /// Ports in the URL itself will always be used instead of the port in the overridden addr.
    pub fn resolve(self, domain: &str, addr: SocketAddr) -> ClientBuilder {
        self.resolve_to_addrs(domain, &[addr])
    }

    /// Override DNS resolution for specific domains to particular IP addresses.
    ///
    /// Set the port to `0` to use the conventional port for the given scheme (e.g. 80 for http).
    /// Ports in the URL itself will always be used instead of the port in the overridden addr.
    pub fn resolve_to_addrs(mut self, domain: &str, addrs: &[SocketAddr]) -> ClientBuilder {
        self.config
            .dns_overrides
            .insert(domain.to_ascii_lowercase(), addrs.to_vec());
        self
    }

    /// Override the DNS resolver implementation.
    ///
    /// Overrides for specific names passed to `resolve` and `resolve_to_addrs` will
    /// still be applied on top of this resolver.
    pub fn dns_resolver<R>(mut self, resolver: R) -> ClientBuilder
    where
        R: crate::dns::resolve::IntoResolve,
    {
        self.config.dns_resolver = Some(resolver.into_resolve());
        self
    }

    /// Set how long resolved DNS entries are served from the in-memory cache
    /// before they are re-resolved.
    ///
    /// The underlying resolver yields only socket addresses (no per-record
    /// TTL), so a single client-wide TTL is applied to every host. The default
    /// is 30 seconds. Passing `Duration::ZERO` disables caching so every
    /// request re-resolves (concurrent in-flight lookups for the same host are
    /// still deduplicated).
    pub fn dns_cache_ttl(mut self, ttl: Duration) -> ClientBuilder {
        self.config.dns_cache_ttl = Some(ttl);
        self
    }

    /// Whether to send data on the first flight ("early data") in TLS 1.3 handshakes
    /// for HTTP/3 connections.
    ///
    /// The default is false.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn tls_early_data(mut self, enabled: bool) -> ClientBuilder {
        self.config.tls_enable_early_data = enabled;
        self
    }

    /// Maximum duration of inactivity to accept before timing out the QUIC connection.
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_max_idle_timeout(mut self, value: Duration) -> ClientBuilder {
        self.config.quic_max_idle_timeout = Some(value);
        self
    }

    /// Maximum number of bytes the peer may transmit without acknowledgement on any one stream
    /// before becoming blocked.
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    ///
    /// Values above the QUIC maximum (2^62 - 1) are clamped to that maximum.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_stream_receive_window(mut self, value: u64) -> ClientBuilder {
        self.config.quic_stream_receive_window = Some(clamp_varint(value));
        self
    }

    /// Maximum number of bytes the peer may transmit across all streams of a connection before
    /// becoming blocked.
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    ///
    /// Values above the QUIC maximum (2^62 - 1) are clamped to that maximum.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_conn_receive_window(mut self, value: u64) -> ClientBuilder {
        self.config.quic_receive_window = Some(clamp_varint(value));
        self
    }

    /// Maximum number of bytes to transmit to a peer without acknowledgment
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_send_window(mut self, value: u64) -> ClientBuilder {
        self.config.quic_send_window = Some(value);
        self
    }

    /// Override the default congestion control algorithm to use [BBR]
    ///
    /// The current default congestion control algorithm is [CUBIC]. This method overrides the
    /// default.
    ///
    /// [BBR]: https://datatracker.ietf.org/doc/html/draft-ietf-ccwg-bbr
    /// [CUBIC]: https://datatracker.ietf.org/doc/html/rfc8312
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_congestion_bbr(mut self) -> ClientBuilder {
        self.config.quic_congestion_bbr = true;
        self
    }

    /// Set the maximum HTTP/3 header size this client is willing to accept.
    ///
    /// See [header size constraints] section of the specification for details.
    ///
    /// [header size constraints]: https://www.rfc-editor.org/rfc/rfc9114.html#name-header-size-constraints
    ///
    /// Please see docs in [`Builder`] in [`h3`].
    ///
    /// [`Builder`]: https://docs.rs/h3/latest/h3/client/struct.Builder.html#method.max_field_section_size
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_max_field_section_size(mut self, value: u64) -> ClientBuilder {
        self.config.h3_max_field_section_size = Some(value);
        self
    }

    /// Enable whether to send HTTP/3 protocol grease on the connections.
    ///
    /// HTTP/3 uses the concept of "grease"
    ///
    /// to prevent potential interoperability issues in the future.
    /// In HTTP/3, the concept of grease is used to ensure that the protocol can evolve
    /// and accommodate future changes without breaking existing implementations.
    ///
    /// Please see docs in [`Builder`] in [`h3`].
    ///
    /// [`Builder`]: https://docs.rs/h3/latest/h3/client/struct.Builder.html#method.send_grease
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_send_grease(mut self, enabled: bool) -> ClientBuilder {
        self.config.h3_send_grease = Some(enabled);
        self
    }

    /// Adds a new Tower [`Layer`](https://docs.rs/tower/latest/tower/trait.Layer.html) to the
    /// base connector [`Service`](https://docs.rs/tower/latest/tower/trait.Service.html) which
    /// is responsible for connection establishment.
    ///
    /// Each subsequent invocation of this function will wrap previous layers.
    ///
    /// If configured, the `connect_timeout` will be the outermost layer.
    ///
    /// Example usage:
    /// ```
    /// use std::time::Duration;
    ///
    /// # let client = primp::Client::builder()
    ///                      // resolved to outermost layer, meaning while we are waiting on concurrency limit
    ///                      .connect_timeout(Duration::from_millis(200))
    ///                      // underneath the concurrency check, so only after concurrency limit lets us through
    ///                      .connector_layer(tower::timeout::TimeoutLayer::new(Duration::from_millis(50)))
    ///                      .connector_layer(tower::limit::concurrency::ConcurrencyLimitLayer::new(2))
    ///                      .build()
    ///                      .unwrap();
    /// ```
    ///
    pub fn connector_layer<L>(mut self, layer: L) -> ClientBuilder
    where
        L: Layer<BoxedConnectorService> + Clone + Send + Sync + 'static,
        L::Service:
            Service<Unnameable, Response = Conn, Error = BoxError> + Clone + Send + Sync + 'static,
        <L::Service as Service<Unnameable>>::Future: Send + 'static,
    {
        let layer = BoxCloneSyncServiceLayer::new(layer);

        self.config.connector_layers.push(layer);

        self
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

fn default_rustls_crypto_provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::aws_lc_rs::default_provider())
}

impl Client {
    /// Constructs a new `Client`.
    ///
    /// # Panics
    ///
    /// This method panics if a TLS backend cannot be initialized, or the resolver
    /// cannot load the system configuration.
    ///
    /// Use `Client::builder()` if you wish to handle the failure as an `Error`
    /// instead of panicking.
    pub fn new() -> Client {
        ClientBuilder::new().build().expect("Client::new()")
    }

    /// Creates a `ClientBuilder` to configure a `Client`.
    ///
    /// This is the same as `ClientBuilder::new()`.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Convenience method to make a `GET` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever the supplied `Url` cannot be parsed.
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    /// Convenience method to make a `POST` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever the supplied `Url` cannot be parsed.
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    /// Convenience method to make a `PUT` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever the supplied `Url` cannot be parsed.
    pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PUT, url)
    }

    /// Convenience method to make a `PATCH` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever the supplied `Url` cannot be parsed.
    pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }

    /// Convenience method to make a `DELETE` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever the supplied `Url` cannot be parsed.
    pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }

    /// Convenience method to make a `HEAD` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever the supplied `Url` cannot be parsed.
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::HEAD, url)
    }

    /// Start building a `Request` with the `Method` and `Url`.
    ///
    /// Returns a `RequestBuilder`, which will allow setting headers and
    /// the request body before sending.
    ///
    /// # Errors
    ///
    /// This method fails whenever the supplied `Url` cannot be parsed.
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        let req = url.into_url().map(move |url| Request::new(method, url));
        RequestBuilder::new(self.clone(), req)
    }

    /// Executes a `Request`.
    ///
    /// A `Request` can be built manually with `Request::new()` or obtained
    /// from a RequestBuilder with `RequestBuilder::build()`.
    ///
    /// You should prefer to use the `RequestBuilder` and
    /// `RequestBuilder::send()`.
    ///
    /// # Errors
    ///
    /// This method fails if there was an error while sending request,
    /// redirect loop was detected or redirect limit was exhausted.
    pub fn execute(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Response, crate::Error>> {
        self.execute_request(request)
    }

    /// Get the default headers for this client.
    pub fn headers(&self) -> &HeaderMap {
        &self.inner.headers
    }

    /// Get a mutable reference to the default headers for this client.
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut Arc::make_mut(&mut self.inner).headers
    }

    /// Get cookies for the given URL.
    #[cfg(feature = "cookies")]
    pub fn get_cookies(&self, url: &Url) -> Option<HeaderValue> {
        self.inner
            .cookie_store
            .as_ref()
            .and_then(|store| store.cookies(url))
    }

    /// Set cookies for the given URL.
    #[cfg(feature = "cookies")]
    pub fn set_cookies(&self, url: &Url, cookies: Vec<HeaderValue>) {
        if let Some(store) = self.inner.cookie_store.as_ref() {
            let mut iter = cookies.iter();
            store.set_cookies(&mut iter, url);
        }
    }

    /// Set proxies for the client.
    ///
    /// Updates the shared proxy state that both the Client (for HTTP auth
    /// header attachment) and the ConnectorService (for connection routing)
    /// read from. This is an in-place update — the service stack is NOT
    /// rebuilt, but both halves see the new proxies because they share the
    /// same `Arc<RwLock<...>>`.
    pub fn set_proxies(&mut self, proxies: Vec<Proxy>) {
        let proxy_matchers: Vec<ProxyMatcher> =
            proxies.into_iter().map(|p| p.into_matcher()).collect();
        let inner = Arc::make_mut(&mut self.inner);
        *inner.proxies.write().unwrap_or_else(|e| e.into_inner()) = proxy_matchers;
        inner.proxies_maybe_http_auth = inner
            .proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|p| p.maybe_has_http_auth());
        inner.proxies_maybe_http_custom_headers = inner
            .proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|p| p.maybe_has_http_custom_headers());
    }

    /// Set the redirect policy for the client.
    ///
    /// Replaces the active policy in-place so that all subsequent requests
    /// honor the new policy (including its `max` limit, custom classifier,
    /// or disabled state). Does not rebuild the HTTP service stack.
    pub fn set_redirect_policy(&mut self, policy: redirect::Policy) {
        // `Policy` holds a `Box<dyn Fn>` for the custom variant, so it
        // can't be `Clone`. Format the description first (borrows `&policy`)
        // and only then move the policy into the live handle.
        let desc = if policy.is_default() {
            None
        } else {
            Some(format!("{:?}", policy))
        };
        let inner = Arc::make_mut(&mut self.inner);
        inner.redirect_policy.set_policy(policy);
        inner.redirect_policy_desc = desc;
    }

    /// Map a request version to one the h1 stack can serialize. HTTP/2 is
    /// passed through (hyper coerces it to HTTP/1.1); HTTP/3 without an H3
    /// dispatch and HTTP/0.9 fall back to HTTP/1.1.
    fn clamp_h1_version(version: http::Version) -> http::Version {
        match version {
            http::Version::HTTP_09 | http::Version::HTTP_3 => http::Version::HTTP_11,
            _ => version,
        }
    }

    pub(super) fn execute_request(&self, req: Request) -> Pending {
        let (method, url, mut headers, body, version, extensions) = req.pieces();
        if url.scheme() != "http" && url.scheme() != "https" {
            return Pending::new_err(error::url_bad_scheme(url));
        }

        // check if we're in https_only mode and check the scheme of the current URL
        if self.inner.https_only && url.scheme() != "https" {
            return Pending::new_err(error::url_bad_scheme(url));
        }

        // When the client is built with `http3_prior_knowledge()`, every request
        // must be dispatched over HTTP/3. Mirror the per-request `.version(HTTP_3)`
        // path so the dispatch below routes to the H3 client.
        #[cfg(feature = "http3")]
        let version = if matches!(self.inner.http_version_pref, HttpVersionPref::Http3) {
            http::Version::HTTP_3
        } else {
            version
        };

        // Clamp versions the h1 stack cannot serialize (hyper 1.11 would
        // either panic or reject with `UserUnsupportedVersion`). HTTP/2 is
        // left alone: hyper coerces it to HTTP/1.1 and the h2 paths overwrite
        // it anyway (`set_h2_version`). HTTP/3 survives only when the request
        // will actually be dispatched to the H3 client.
        #[cfg(feature = "http3")]
        let version = if version == http::Version::HTTP_3 && self.inner.h3_client.is_some() {
            version
        } else {
            Self::clamp_h1_version(version)
        };
        #[cfg(not(feature = "http3"))]
        let version = Self::clamp_h1_version(version);

        // insert default headers in the request headers
        // without overwriting already appended headers.
        for (key, value) in &self.inner.headers {
            if let Entry::Vacant(entry) = headers.entry(key) {
                entry.insert(value.clone());
            }
        }

        let uri = match try_uri(&url) {
            Ok(uri) => uri,
            _ => return Pending::new_err(error::url_invalid_uri(url)),
        };

        let body = body.unwrap_or_else(Body::empty);

        self.proxy_auth(&uri, &mut headers);
        self.proxy_custom_headers(&uri, &mut headers);

        let builder = hyper::Request::builder()
            .method(method.clone())
            .uri(uri)
            .version(version);

        let mut req = match builder.body(body) {
            Ok(req) => req,
            Err(e) => return Pending::new_err(crate::error::request(e.to_string())),
        };
        *req.headers_mut() = headers.clone();

        // Carry the per-request redirect override onto the wire request so the
        // redirect policy can read it from the first request of the chain.
        if let Some(override_policy) = extensions
            .get::<crate::config::RequestConfig<crate::config::RedirectPolicyOverride>>()
            .and_then(|c| c.get_value())
            .copied()
        {
            req.extensions_mut().insert(override_policy);
        }

        // Carry the per-request one-shot cookies onto the wire request so the
        // cookie service can re-merge them with the fresh jar on every
        // redirect hop (the rebuilt hyper request above starts with empty
        // extensions).
        if let Some(one_shot) = extensions
            .get::<crate::config::RequestConfig<crate::config::OneShotCookies>>()
            .and_then(|c| c.get_value())
            .cloned()
        {
            req.extensions_mut().insert(crate::config::RequestConfig::<
                crate::config::OneShotCookies,
            >::new(Some(one_shot)));
        }

        #[cfg(feature = "http3")]
        let in_flight = if version == http::Version::HTTP_3 {
            if let Some(h3_client) = self.inner.h3_client.as_ref() {
                let mut h3 = h3_client.clone();
                ResponseFuture::H3(Box::pin(h3.call(req)))
            } else {
                let mut svc = self.inner.service.clone();
                ResponseFuture::Response(Box::pin(svc.call(req)))
            }
        } else {
            let mut svc = self.inner.service.clone();
            ResponseFuture::Response(Box::pin(svc.call(req)))
        };
        #[cfg(not(feature = "http3"))]
        let in_flight = {
            let mut svc = self.inner.service.clone();
            ResponseFuture::Response(Box::pin(svc.call(req)))
        };

        let total_timeout = self
            .inner
            .total_timeout
            .fetch(&extensions)
            .copied()
            .map(tokio::time::sleep)
            .map(Box::pin);

        let read_timeout_duration = self.inner.read_timeout.fetch(&extensions).copied();
        let read_timeout_fut = read_timeout_duration.map(tokio::time::sleep).map(Box::pin);

        Pending {
            inner: PendingInner::Request(Box::pin(PendingRequest {
                method,
                url,

                client: self.inner.clone(),

                in_flight,
                total_timeout,
                read_timeout_fut,
                read_timeout: read_timeout_duration,
            })),
        }
    }

    fn proxy_auth(&self, dst: &Uri, headers: &mut HeaderMap) {
        if !self.inner.proxies_maybe_http_auth {
            return;
        }

        // Only set the header here if the destination scheme is 'http',
        // since otherwise, the header will be included in the CONNECT tunnel
        // request instead.
        if dst.scheme() != Some(&Scheme::HTTP) {
            return;
        }

        if headers.contains_key(PROXY_AUTHORIZATION) {
            return;
        }

        for proxy in self
            .inner
            .proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            match proxy.intercept(dst) {
                Ok(Some(intercepted)) => {
                    if let Some(scheme) = intercepted.uri().scheme() {
                        if scheme == &Scheme::HTTP || scheme == &Scheme::HTTPS {
                            if let Some(header) = intercepted.basic_auth().cloned() {
                                headers.insert(PROXY_AUTHORIZATION, header);
                            }
                        }
                    }
                    break;
                }
                Ok(None) => continue,
                Err(e) => {
                    log::warn!("proxy intercept error in proxy_auth: {e}");
                    break;
                }
            }
        }
    }

    fn proxy_custom_headers(&self, dst: &Uri, headers: &mut HeaderMap) {
        if !self.inner.proxies_maybe_http_custom_headers {
            return;
        }

        if dst.scheme() != Some(&Scheme::HTTP) {
            return;
        }

        for proxy in self
            .inner
            .proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            match proxy.intercept(dst) {
                Ok(Some(intercepted)) => {
                    if let Some(scheme) = intercepted.uri().scheme() {
                        if scheme == &Scheme::HTTP || scheme == &Scheme::HTTPS {
                            if let Some(map) = intercepted.custom_headers() {
                                map.iter().for_each(|(key, value)| {
                                    headers.insert(key, value.clone());
                                });
                            }
                        }
                    }
                    break;
                }
                Ok(None) => continue,
                Err(e) => {
                    log::warn!("proxy intercept error in proxy_custom_headers: {e}");
                    break;
                }
            }
        }
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("Client");
        self.inner.fmt_fields(&mut builder);
        builder.finish()
    }
}

/// Debug view of a `HeaderMap` that masks sensitive header values so
/// `Debug for Client`/`ClientBuilder` never leak credentials.
struct RedactedHeaders<'a>(&'a HeaderMap);

fn is_sensitive_header(name: &http::HeaderName) -> bool {
    name == http::header::AUTHORIZATION
        || name == http::header::COOKIE
        || name == "cookie2"
        || name == http::header::PROXY_AUTHORIZATION
}

impl fmt::Debug for RedactedHeaders<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        enum Value<'a> {
            Plain(&'a HeaderValue),
            Masked(&'a HeaderValue),
        }
        impl fmt::Debug for Value<'_> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self {
                    Value::Plain(v) => v.fmt(f),
                    // Show the byte length only.
                    Value::Masked(v) => write!(f, "\"[REDACTED:{} bytes]\"", v.as_bytes().len()),
                }
            }
        }

        f.debug_map()
            .entries(self.0.iter().map(|(name, value)| {
                let value = if is_sensitive_header(name) {
                    Value::Masked(value)
                } else {
                    Value::Plain(value)
                };
                (name, value)
            }))
            .finish()
    }
}

impl tower_service::Service<Request> for Client {
    type Response = Response;
    type Error = crate::Error;
    type Future = Pending;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.execute_request(req)
    }
}

impl tower_service::Service<Request> for &'_ Client {
    type Response = Response;
    type Error = crate::Error;
    type Future = Pending;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.execute_request(req)
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("ClientBuilder");
        self.config.fmt_fields(&mut builder);
        builder.finish()
    }
}

impl Config {
    fn fmt_fields(&self, f: &mut fmt::DebugStruct<'_, '_>) {
        // Instead of deriving Debug, only print fields when their output
        // would provide relevant or interesting data.

        #[cfg(feature = "cookies")]
        {
            if self.cookie_store.is_some() {
                f.field("cookie_store", &true);
            }
        }

        f.field("accepts", &self.accepts);

        if !self.proxies.is_empty() {
            f.field("proxies", &self.proxies);
        }

        if !self.redirect_policy.is_default() {
            f.field("redirect_policy", &self.redirect_policy);
        }

        if self.referer {
            f.field("referer", &true);
        }

        f.field("default_headers", &RedactedHeaders(&self.headers));

        if self.http1_title_case_headers {
            f.field("http1_title_case_headers", &true);
        }

        if self.http1_allow_obsolete_multiline_headers_in_responses {
            f.field("http1_allow_obsolete_multiline_headers_in_responses", &true);
        }

        if self.http1_ignore_invalid_headers_in_responses {
            f.field("http1_ignore_invalid_headers_in_responses", &true);
        }

        if self.http1_allow_spaces_after_header_name_in_responses {
            f.field("http1_allow_spaces_after_header_name_in_responses", &true);
        }

        if let Some(http1_max_headers) = self.http1_max_headers {
            f.field("http1_max_headers", &http1_max_headers);
        }

        if matches!(self.http_version_pref, HttpVersionPref::Http1) {
            f.field("http1_only", &true);
        }

        if matches!(self.http_version_pref, HttpVersionPref::Http2) {
            f.field("http2_prior_knowledge", &true);
        }

        if let Some(ref d) = self.connect_timeout {
            f.field("connect_timeout", d);
        }

        if let Some(ref d) = self.timeout {
            f.field("timeout", d);
        }

        if let Some(ref v) = self.local_address {
            f.field("local_address", v);
        }

        #[cfg(any(
            target_os = "android",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "ios",
            target_os = "linux",
            target_os = "macos",
            target_os = "solaris",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
        ))]
        if let Some(ref v) = self.interface {
            f.field("interface", v);
        }

        if self.nodelay {
            f.field("tcp_nodelay", &true);
        }

        {
            if !self.hostname_verification {
                f.field("tls_danger_accept_invalid_hostnames", &true);
            }
        }

        {
            if !self.certs_verification {
                f.field("tls_danger_accept_invalid_certs", &true);
            }

            if let Some(ref min_tls_version) = self.min_tls_version {
                f.field("tls_version_min", min_tls_version);
            }

            if let Some(ref max_tls_version) = self.max_tls_version {
                f.field("tls_version_max", max_tls_version);
            }

            f.field("tls_sni", &self.tls_sni);

            f.field("tls_info", &self.tls_info);
        }

        {
            f.field("tls_sslkeylogfile", &self.tls_sslkeylogfile);
        }

        {
            f.field("tls_backend", &self.tls);
        }

        if !self.dns_overrides.is_empty() {
            f.field("dns_overrides", &self.dns_overrides);
        }

        #[cfg(feature = "http3")]
        {
            if self.tls_enable_early_data {
                f.field("tls_enable_early_data", &true);
            }
        }

        #[cfg(unix)]
        if let Some(ref p) = self.unix_socket {
            f.field("unix_socket", p);
        }
    }
}

#[cfg(not(feature = "cookies"))]
type MaybeCookieService<T> = T;

#[cfg(feature = "cookies")]
type MaybeCookieService<T> = CookieService<T>;

#[cfg(not(any(
    feature = "gzip",
    feature = "brotli",
    feature = "zstd",
    feature = "deflate"
)))]
type MaybeDecompression<T> = T;

#[cfg(any(
    feature = "gzip",
    feature = "brotli",
    feature = "zstd",
    feature = "deflate"
))]
type MaybeDecompression<T> = RangeGuard<Decompression<T>>;

type LayeredService<T> = MaybeDecompression<
    FollowRedirect<
        MaybeCookieService<tower::retry::Retry<crate::retry::Policy, T>>,
        TowerRedirectPolicy,
    >,
>;
type LayeredFuture<T> = <LayeredService<T> as Service<http::Request<Body>>>::Future;

#[derive(Clone)]
struct ClientRef {
    accepts: Accepts,
    #[cfg(feature = "cookies")]
    cookie_store: Option<Arc<dyn cookie::CookieStore>>,
    headers: HeaderMap,
    service: LayeredService<NegotiatingConnection>,
    #[cfg(feature = "http3")]
    h3_client: Option<LayeredService<H3Client>>,
    #[cfg(feature = "http3")]
    http_version_pref: HttpVersionPref,
    referer: bool,
    total_timeout: RequestConfig<TotalTimeout>,
    read_timeout: RequestConfig<ReadTimeout>,
    proxies: Arc<RwLock<Vec<ProxyMatcher>>>,
    proxies_maybe_http_auth: bool,
    proxies_maybe_http_custom_headers: bool,
    https_only: bool,
    redirect_policy_desc: Option<String>,
    redirect_policy: TowerRedirectPolicy,
}

impl ClientRef {
    fn fmt_fields(&self, f: &mut fmt::DebugStruct<'_, '_>) {
        // Instead of deriving Debug, only print fields when their output
        // would provide relevant or interesting data.

        #[cfg(feature = "cookies")]
        {
            if self.cookie_store.is_some() {
                f.field("cookie_store", &true);
            }
        }

        f.field("accepts", &self.accepts);

        if !self
            .proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
        {
            f.field("proxies", &self.proxies);
        }

        if let Some(s) = &self.redirect_policy_desc {
            f.field("redirect_policy", s);
        }

        if self.referer {
            f.field("referer", &true);
        }

        f.field("default_headers", &RedactedHeaders(&self.headers));

        self.total_timeout.fmt_as_field(f);

        self.read_timeout.fmt_as_field(f);
    }
}

pin_project! {
    pub struct Pending {
        #[pin]
        inner: PendingInner,
    }
}

enum PendingInner {
    Request(Pin<Box<PendingRequest>>),
    Error(Option<crate::Error>),
}

pin_project! {
    struct PendingRequest {
        method: Method,
        url: Url,

        client: Arc<ClientRef>,

        #[pin]
        in_flight: ResponseFuture,
        #[pin]
        total_timeout: Option<Pin<Box<Sleep>>>,
        #[pin]
        read_timeout_fut: Option<Pin<Box<Sleep>>>,
        read_timeout: Option<Duration>,
    }
}

enum ResponseFuture {
    Response(Pin<Box<LayeredFuture<NegotiatingConnection>>>),
    #[cfg(feature = "http3")]
    H3(Pin<Box<LayeredFuture<H3Client>>>),
}

impl PendingRequest {
    fn in_flight(self: Pin<&mut Self>) -> Pin<&mut ResponseFuture> {
        self.project().in_flight
    }

    fn total_timeout(self: Pin<&mut Self>) -> Pin<&mut Option<Pin<Box<Sleep>>>> {
        self.project().total_timeout
    }

    fn read_timeout(self: Pin<&mut Self>) -> Pin<&mut Option<Pin<Box<Sleep>>>> {
        self.project().read_timeout_fut
    }
}

impl Pending {
    pub(super) fn new_err(err: crate::Error) -> Pending {
        Pending {
            inner: PendingInner::Error(Some(err)),
        }
    }

    fn inner(self: Pin<&mut Self>) -> Pin<&mut PendingInner> {
        self.project().inner
    }
}

impl Future for Pending {
    type Output = Result<Response, crate::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = self.inner();
        match inner.get_mut() {
            PendingInner::Request(ref mut req) => Pin::new(req).poll(cx),
            PendingInner::Error(ref mut err) => Poll::Ready(Err(err
                .take()
                .unwrap_or_else(|| crate::error::request("pending already returned an error")))),
        }
    }
}

impl Future for PendingRequest {
    type Output = Result<Response, crate::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(delay) = self.as_mut().total_timeout().as_mut().as_pin_mut() {
            if let Poll::Ready(()) = delay.poll(cx) {
                // Total timeout during SOCKS handshake surfaces as bare
                // `TimedOut` (not `is_connect`). For SOCKS, emit a connect
                // timeout so the `socks5_wrong_auth_is_rejected` check holds.
                let is_socks = {
                    let uri: Result<http::Uri, _> = self.url.as_str().parse();
                    if let Ok(uri) = uri {
                        let proxies = self
                            .client
                            .proxies
                            .read()
                            .unwrap_or_else(|e| e.into_inner());
                        proxies.iter().any(|m| {
                            m.intercept(&uri)
                                .ok()
                                .flatten()
                                .map(|ic| {
                                    ic.uri()
                                        .scheme_str()
                                        .map(|s| s.starts_with("socks"))
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        })
                    } else {
                        false
                    }
                };
                if is_socks {
                    return Poll::Ready(Err(crate::error::request(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "socks connect timeout",
                    ))
                    .with_url(self.url.clone())));
                }
                return Poll::Ready(Err(
                    crate::error::request(crate::error::TimedOut).with_url(self.url.clone())
                ));
            }
        }

        if let Some(delay) = self.as_mut().read_timeout().as_mut().as_pin_mut() {
            if let Poll::Ready(()) = delay.poll(cx) {
                return Poll::Ready(Err(
                    crate::error::request(crate::error::TimedOut).with_url(self.url.clone())
                ));
            }
        }

        let res = match self.as_mut().in_flight().get_mut() {
            ResponseFuture::Response(r) => match ready!(r.as_mut().poll(cx)) {
                Err(e) => {
                    return Poll::Ready(Err(e.if_no_url(|| self.url.clone())));
                }
                Ok(res) => res.map(super::body::boxed),
            },
            #[cfg(feature = "http3")]
            ResponseFuture::H3(r) => match ready!(r.as_mut().poll(cx)) {
                Err(e) => {
                    return Poll::Ready(Err(e.if_no_url(|| self.url.clone())));
                }
                Ok(res) => res.map(super::body::boxed),
            },
        };

        if let Some(url) = &res
            .extensions()
            .get::<tower_http::follow_redirect::RequestUri>()
        {
            self.url = match Url::parse(&url.0.to_string()) {
                Ok(url) => url,
                Err(e) => return Poll::Ready(Err(crate::error::decode(e))),
            }
        };

        let res = Response::new(
            res,
            self.url.clone(),
            self.total_timeout.take(),
            self.read_timeout,
        );
        Poll::Ready(Ok(res))
    }
}

impl fmt::Debug for Pending {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.inner {
            PendingInner::Request(ref req) => f
                .debug_struct("Pending")
                .field("method", &req.method)
                .field("url", &req.url)
                .finish(),
            PendingInner::Error(ref err) => f.debug_struct("Pending").field("error", err).finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::proxy::Proxy;

    /// Regression test: `set_proxies()` correctly updates the shared proxy
    /// state for both header-attachment and connector routing.
    ///
    /// `proxies` is stored as `Arc<RwLock<Vec<ProxyMatcher>>>` shared between
    /// `ClientRef` and `ConnectorService`. `set_proxies()` writes through the
    /// RwLock so both halves see the new proxies. It also recomputes the
    /// cached boolean hints.
    #[test]
    fn set_proxies_updates_shared_proxy_state() {
        // --- 1. Build client WITHOUT auth on proxies ------------------------
        let mut client = super::Client::builder()
            .no_proxy()
            .build()
            .expect("valid client");

        // Sanity: ClientRef.proxies is empty, flags are both false.
        assert!(client.inner.proxies.read().unwrap().is_empty());
        assert!(!client.inner.proxies_maybe_http_auth);
        assert!(!client.inner.proxies_maybe_http_custom_headers);

        // --- 2. Add a proxy that HAS basic auth -----------------------------
        let auth_proxy = Proxy::http("http://user:pass@my-proxy:8080").unwrap();
        client.set_proxies(vec![auth_proxy]);

        // ClientRef.proxies IS updated — read through the shared RwLock.
        assert_eq!(client.inner.proxies.read().unwrap().len(), 1);

        // The cached boolean hints ARE recomputed by set_proxies().
        assert!(
            client.inner.proxies_maybe_http_auth,
            "proxies_maybe_http_auth must be recomputed after set_proxies()"
        );

        // The ConnectorService shares the same Arc<RwLock<...>>, so it also
        // sees the new proxies. The Arc pointer is identical because we never
        // allocate a new one (we write through the existing RwLock).
    }

    #[tokio::test]
    async fn execute_request_rejects_invalid_urls() {
        let url_str = "hxxps://www.rust-lang.org/";
        let url = url::Url::parse(url_str).unwrap();
        let result = crate::get(url.clone()).await;

        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.is_builder());
        assert_eq!(url_str, err.url().unwrap().as_str());
    }

    #[tokio::test]
    async fn execute_request_rejects_invalid_hostname() {
        let url_str = "https://{{hostname}}/";
        let url = url::Url::parse(url_str).unwrap();
        let result = crate::get(url.clone()).await;

        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.is_builder());
        assert_eq!(url_str, err.url().unwrap().as_str());
    }

    #[test]
    fn test_future_size() {
        let s = std::mem::size_of::<super::Pending>();
        assert!(s < 128, "size_of::<Pending>() == {s}, too big");
    }

    #[test]
    fn debug_redacts_sensitive_default_headers() {
        // Debug output must not leak credentials stored in default_headers —
        // `Authorization`, `Cookie`, `Proxy-Authorization` values are printed
        // as part of `Client`/`ClientBuilder` Debug.
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("Bearer super-secret-token"),
        );
        headers.insert(
            http::header::COOKIE,
            http::HeaderValue::from_static("session=secret-cookie"),
        );
        headers.insert(
            http::header::PROXY_AUTHORIZATION,
            http::HeaderValue::from_static("Basic ZGlzbmV5OnB3"),
        );
        headers.insert(
            http::header::ACCEPT,
            http::HeaderValue::from_static("application/json"),
        );

        let builder = crate::Client::builder().default_headers(headers);
        let builder_debug = format!("{builder:?}");
        assert!(
            !builder_debug.contains("super-secret-token"),
            "ClientBuilder Debug leaked Authorization: {builder_debug}"
        );
        assert!(
            !builder_debug.contains("secret-cookie"),
            "ClientBuilder Debug leaked Cookie: {builder_debug}"
        );
        assert!(
            !builder_debug.contains("ZGlzbmV5OnB3"),
            "ClientBuilder Debug leaked Proxy-Authorization: {builder_debug}"
        );
        assert!(
            builder_debug.contains("application/json"),
            "ClientBuilder Debug must keep non-sensitive headers: {builder_debug}"
        );

        let client = builder.build().expect("valid client");
        let client_debug = format!("{client:?}");
        assert!(
            !client_debug.contains("super-secret-token"),
            "Client Debug leaked Authorization: {client_debug}"
        );
        assert!(
            !client_debug.contains("secret-cookie"),
            "Client Debug leaked Cookie: {client_debug}"
        );
        assert!(
            !client_debug.contains("ZGlzbmV5OnB3"),
            "Client Debug leaked Proxy-Authorization: {client_debug}"
        );
    }

    #[test]
    fn impersonate_honors_danger_flags_and_identity() {
        // `danger_accept_invalid_*` must be applied even while impersonating
        // (the preconfigured-TLS path used to discard them).
        let client = crate::Client::builder()
            .impersonate(crate::Impersonate::Chrome)
            .tls_danger_accept_invalid_certs(true)
            .tls_danger_accept_invalid_hostnames(true)
            .build();
        assert!(
            client.is_ok(),
            "impersonation + danger flags should build: {:?}",
            client.err()
        );

        // A client identity must also be applied while impersonating.
        const IDENTITY_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIIBczCCARmgAwIBAgIUK07VPLl7ynw8Bcx6AQHuJkjqp2wwCgYIKoZIzj0EAwIw\n\
DzENMAsGA1UEAwwEdGVzdDAeFw0yNjA3MTQwODUxNDZaFw0zNjA3MTEwODUxNDZa\n\
MA8xDTALBgNVBAMMBHRlc3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAATzwvAt\n\
9EdlZSEr2Rog6m66wmdniAyboY2cklEMr3rB2d4zBYoOXv2WkHe8yOWAcGrC1k5u\n\
+dVK96bQB9EvsTMco1MwUTAdBgNVHQ4EFgQUeyiysfgXNI7leajiMRbrSLTkjeEw\n\
HwYDVR0jBBgwFoAUeyiysfgXNI7leajiMRbrSLTkjeEwDwYDVR0TAQH/BAUwAwEB\n\
/zAKBggqhkjOPQQDAgNIADBFAiBoiO/P2xu1DTMMnKrfB0sx8z9Za3jJkaNB2aHX\n\
GBdcpAIhALMXjzfzjdC9ih+UdGKcpiuAqeJmQ9zeBZpGjyJCavB1\n\
-----END CERTIFICATE-----\n\
-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgmNUWBOYwsZssv12q\n\
tBIyiLW0W+8v9ZahMscX84+rLVehRANCAATzwvAt9EdlZSEr2Rog6m66wmdniAyb\n\
oY2cklEMr3rB2d4zBYoOXv2WkHe8yOWAcGrC1k5u+dVK96bQB9EvsTMc\n\
-----END PRIVATE KEY-----\n";
        let identity =
            crate::Identity::from_pem(IDENTITY_PEM.as_bytes()).expect("valid identity pem");
        let client = crate::Client::builder()
            .impersonate(crate::Impersonate::Chrome)
            .identity(identity)
            .build();
        assert!(
            client.is_ok(),
            "impersonation + identity should build: {:?}",
            client.err()
        );
    }
}
