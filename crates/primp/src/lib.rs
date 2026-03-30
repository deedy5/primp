#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(test, deny(warnings))]

//! # primp
//!
//! The `primp` crate provides an HTTP client with browser impersonation capabilities.
//! It wraps `primp-reqwest` and adds browser fingerprinting features.
//!
//! ## Features
//!
//! - All features from `primp-reqwest` (HTTP client, TLS, cookies, etc.)
//! - Browser impersonation (Chrome, Firefox, Safari, Edge, Opera)
//! - TLS fingerprint matching
//! - HTTP/2 fingerprint matching

// Re-export everything from primp-reqwest
pub use reqwest::*;

/// Impersonation module for browser fingerprinting
#[cfg(not(target_arch = "wasm32"))]
pub mod imp;

/// Re-export impersonation types
#[cfg(not(target_arch = "wasm32"))]
pub use imp::{BrowserSettings, Http2Data, Impersonate, ImpersonateOS};

/// Re-export h2 frame types used in the public HTTP/2 API
#[cfg(feature = "http2")]
pub use h2::frame::{
    PseudoId, PseudoOrder, PseudoOrderBuilder, SettingId, SettingsOrder, SettingsOrderBuilder,
};

/// An HTTP Client with impersonation support
#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
}

/// A builder for configuring a Client with impersonation support
#[derive(Debug)]
pub struct ClientBuilder {
    inner: reqwest::ClientBuilder,
    #[cfg(not(target_arch = "wasm32"))]
    impersonate: Option<Impersonate>,
    #[cfg(not(target_arch = "wasm32"))]
    os_type: Option<ImpersonateOS>,
    #[cfg(not(target_arch = "wasm32"))]
    root_certs: Vec<reqwest::Certificate>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    /// Constructs a new `ClientBuilder`.
    pub fn new() -> Self {
        Self {
            inner: reqwest::ClientBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(30)),
            #[cfg(not(target_arch = "wasm32"))]
            impersonate: None,
            #[cfg(not(target_arch = "wasm32"))]
            os_type: None,
            #[cfg(not(target_arch = "wasm32"))]
            root_certs: Vec::new(),
        }
    }

    /// Returns a `Client` that uses this `ClientBuilder` configuration.
    pub fn build(self) -> crate::Result<Client> {
        #[cfg(not(target_arch = "wasm32"))]
        let inner = match (self.impersonate, self.os_type) {
            (None, None) => self.inner.build()?,
            _ => {
                let imp = self.impersonate.unwrap_or(Impersonate::Random);
                let os = self.os_type.unwrap_or(ImpersonateOS::Random);
                let settings = imp::get_browser_settings(imp, Some(os));
                apply_impersonation(self.inner, settings, &self.root_certs)?
            }
        };

        #[cfg(target_arch = "wasm32")]
        let inner = self.inner.build()?;

        Ok(Client { inner })
    }

    /// Sets the browser to impersonate.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn impersonate(mut self, version: Impersonate) -> Self {
        self.impersonate = Some(version);
        self
    }

    /// Sets the OS to impersonate.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn impersonate_os(mut self, os_type: ImpersonateOS) -> Self {
        self.os_type = Some(os_type);
        self
    }

    /// Sets the `User-Agent` header.
    pub fn user_agent<V>(mut self, value: V) -> Self
    where
        V: TryInto<http::HeaderValue>,
        V::Error: Into<http::Error>,
    {
        self.inner = self.inner.user_agent(value);
        self
    }

    /// Sets the default headers for every request.
    pub fn default_headers(mut self, headers: http::HeaderMap) -> Self {
        self.inner = self.inner.default_headers(headers);
        self
    }

    /// Enable a persistent cookie store.
    pub fn cookie_store(mut self, enable: bool) -> Self {
        self.inner = self.inner.cookie_store(enable);
        self
    }

    /// Set the cookie provider.
    pub fn cookie_provider<C: reqwest::cookie::CookieStore + 'static>(
        mut self,
        cookie_store: std::sync::Arc<C>,
    ) -> Self {
        self.inner = self.inner.cookie_provider(cookie_store);
        self
    }

    /// Enable auto gzip decompression.
    pub fn gzip(mut self, enable: bool) -> Self {
        self.inner = self.inner.gzip(enable);
        self
    }

    /// Enable auto brotli decompression.
    pub fn brotli(mut self, enable: bool) -> Self {
        self.inner = self.inner.brotli(enable);
        self
    }

    /// Enable auto zstd decompression.
    pub fn zstd(mut self, enable: bool) -> Self {
        self.inner = self.inner.zstd(enable);
        self
    }

    /// Enable auto deflate decompression.
    #[cfg(feature = "deflate")]
    pub fn deflate(mut self, enable: bool) -> Self {
        self.inner = self.inner.deflate(enable);
        self
    }

    /// Disable auto gzip decompression.
    pub fn no_gzip(mut self) -> Self {
        self.inner = self.inner.no_gzip();
        self
    }

    /// Disable auto brotli decompression.
    pub fn no_brotli(mut self) -> Self {
        self.inner = self.inner.no_brotli();
        self
    }

    /// Disable auto zstd decompression.
    pub fn no_zstd(mut self) -> Self {
        self.inner = self.inner.no_zstd();
        self
    }

    /// Disable auto deflate decompression.
    pub fn no_deflate(mut self) -> Self {
        self.inner = self.inner.no_deflate();
        self
    }

    /// Set a redirect policy.
    pub fn redirect(mut self, policy: reqwest::redirect::Policy) -> Self {
        self.inner = self.inner.redirect(policy);
        self
    }

    /// Enable or disable automatic Referer header.
    pub fn referer(mut self, enable: bool) -> Self {
        self.inner = self.inner.referer(enable);
        self
    }

    /// Add a proxy.
    pub fn proxy(mut self, proxy: reqwest::Proxy) -> Self {
        self.inner = self.inner.proxy(proxy);
        self
    }

    /// Clear all proxies.
    pub fn no_proxy(mut self) -> Self {
        self.inner = self.inner.no_proxy();
        self
    }

    /// Enables a total request timeout.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    /// Enables a read timeout.
    pub fn read_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.inner = self.inner.read_timeout(timeout);
        self
    }

    /// Set a timeout for only the connect phase.
    pub fn connect_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.inner = self.inner.connect_timeout(timeout);
        self
    }

    /// Set whether connections should emit verbose logs.
    pub fn connection_verbose(mut self, verbose: bool) -> Self {
        self.inner = self.inner.connection_verbose(verbose);
        self
    }

    /// Set an optional timeout for idle sockets being kept-alive.
    pub fn pool_idle_timeout<D>(mut self, val: D) -> Self
    where
        D: Into<Option<std::time::Duration>>,
    {
        self.inner = self.inner.pool_idle_timeout(val);
        self
    }

    /// Sets the maximum idle connection per host allowed in the pool.
    pub fn pool_max_idle_per_host(mut self, max: usize) -> Self {
        self.inner = self.inner.pool_max_idle_per_host(max);
        self
    }

    /// Send headers as title case instead of lowercase.
    pub fn http1_title_case_headers(mut self) -> Self {
        self.inner = self.inner.http1_title_case_headers();
        self
    }

    /// Only use HTTP/1.
    pub fn http1_only(mut self) -> Self {
        self.inner = self.inner.http1_only();
        self
    }

    /// Allow HTTP/0.9 responses.
    pub fn http09_responses(mut self) -> Self {
        self.inner = self.inner.http09_responses();
        self
    }

    /// Only use HTTP/2.
    pub fn http2_prior_knowledge(mut self) -> Self {
        self.inner = self.inner.http2_prior_knowledge();
        self
    }

    /// Sets the HTTP2 stream-level initial window size.
    pub fn http2_initial_stream_window_size(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.inner = self.inner.http2_initial_stream_window_size(sz);
        self
    }

    /// Sets the HTTP2 connection-level initial window size.
    pub fn http2_initial_connection_window_size(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.inner = self.inner.http2_initial_connection_window_size(sz);
        self
    }

    /// Sets whether to use an adaptive flow control.
    pub fn http2_adaptive_window(mut self, enabled: bool) -> Self {
        self.inner = self.inner.http2_adaptive_window(enabled);
        self
    }

    /// Sets the maximum frame size to use for HTTP2.
    pub fn http2_max_frame_size(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.inner = self.inner.http2_max_frame_size(sz);
        self
    }

    /// Sets the maximum size of received header frames for HTTP2.
    pub fn http2_max_header_list_size(mut self, max_header_size_bytes: u32) -> Self {
        self.inner = self.inner.http2_max_header_list_size(max_header_size_bytes);
        self
    }

    /// Sets an interval for HTTP2 Ping frames.
    pub fn http2_keep_alive_interval(
        mut self,
        interval: impl Into<Option<std::time::Duration>>,
    ) -> Self {
        self.inner = self.inner.http2_keep_alive_interval(interval);
        self
    }

    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    pub fn http2_keep_alive_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.inner = self.inner.http2_keep_alive_timeout(timeout);
        self
    }

    /// Sets whether HTTP2 keep-alive should apply while the connection is idle.
    pub fn http2_keep_alive_while_idle(mut self, enabled: bool) -> Self {
        self.inner = self.inner.http2_keep_alive_while_idle(enabled);
        self
    }

    /// Sets the HTTP/2 SETTINGS_HEADER_TABLE_SIZE value.
    pub fn http2_header_table_size(mut self, size: impl Into<Option<u32>>) -> Self {
        self.inner = self.inner.http2_header_table_size(size);
        self
    }

    /// Sets the HTTP/2 SETTINGS_MAX_CONCURRENT_STREAMS value.
    pub fn http2_max_concurrent_streams(mut self, max: impl Into<Option<u32>>) -> Self {
        self.inner = self.inner.http2_max_concurrent_streams(max);
        self
    }

    /// Sets the HTTP/2 SETTINGS_ENABLE_PUSH value.
    pub fn http2_enable_push(mut self, enabled: impl Into<Option<bool>>) -> Self {
        self.inner = self.inner.http2_enable_push(enabled);
        self
    }

    /// Sets the HTTP/2 SETTINGS_NO_RFC7540_PRIORITIES value.
    pub fn http2_no_rfc7540_priorities(mut self, enabled: impl Into<Option<bool>>) -> Self {
        self.inner = self.inner.http2_no_rfc7540_priorities(enabled);
        self
    }

    /// Sets the HTTP/2 SETTINGS frame order for fingerprinting.
    pub fn http2_settings_order(mut self, order: h2::frame::SettingsOrder) -> Self {
        self.inner = self.inner.http2_settings_order(order);
        self
    }

    /// Sets the HTTP/2 pseudo-header order for fingerprinting.
    pub fn http2_headers_pseudo_order(mut self, order: h2::frame::PseudoOrder) -> Self {
        self.inner = self.inner.http2_headers_pseudo_order(order);
        self
    }

    /// Sets whether to include PRIORITY flag in HTTP/2 HEADERS frames.
    pub fn http2_headers_priority(mut self, data: Option<(u8, u32, bool)>) -> Self {
        self.inner = self.inner.http2_headers_priority(data);
        self
    }

    /// Set whether sockets have TCP_NODELAY enabled.
    pub fn tcp_nodelay(mut self, enabled: bool) -> Self {
        self.inner = self.inner.tcp_nodelay(enabled);
        self
    }

    /// Bind to a local IP Address.
    pub fn local_address<T>(mut self, addr: T) -> Self
    where
        T: Into<Option<std::net::IpAddr>>,
    {
        self.inner = self.inner.local_address(addr);
        self
    }

    /// Set that all sockets have SO_KEEPALIVE set.
    pub fn tcp_keepalive<D>(mut self, val: D) -> Self
    where
        D: Into<Option<std::time::Duration>>,
    {
        self.inner = self.inner.tcp_keepalive(val);
        self
    }

    /// Add custom certificate roots.
    pub fn add_root_certificate(mut self, cert: reqwest::Certificate) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        self.root_certs.push(cert.clone());
        self.inner = self.inner.add_root_certificate(cert);
        self
    }

    /// Sets the identity to be used for client certificate authentication.
    pub fn identity(mut self, identity: reqwest::Identity) -> Self {
        self.inner = self.inner.identity(identity);
        self
    }

    /// Controls the use of hostname verification.
    pub fn danger_accept_invalid_hostnames(mut self, accept_invalid_hostname: bool) -> Self {
        self.inner = self
            .inner
            .danger_accept_invalid_hostnames(accept_invalid_hostname);
        self
    }

    /// Controls the use of certificate validation.
    pub fn danger_accept_invalid_certs(mut self, accept_invalid_certs: bool) -> Self {
        self.inner = self.inner.danger_accept_invalid_certs(accept_invalid_certs);
        self
    }

    /// Use a preconfigured TLS backend.
    pub fn use_preconfigured_tls(mut self, tls: impl std::any::Any) -> Self {
        self.inner = self.inner.use_preconfigured_tls(tls);
        self
    }

    /// Restrict the Client to be used with HTTPS only requests.
    pub fn https_only(mut self, enabled: bool) -> Self {
        self.inner = self.inner.https_only(enabled);
        self
    }

    /// Override DNS resolution for specific domains.
    pub fn resolve(mut self, domain: &str, addr: std::net::SocketAddr) -> Self {
        self.inner = self.inner.resolve(domain, addr);
        self
    }

    /// Override DNS resolution for specific domains to particular IP addresses.
    pub fn resolve_to_addrs(mut self, domain: &str, addrs: &[std::net::SocketAddr]) -> Self {
        self.inner = self.inner.resolve_to_addrs(domain, addrs);
        self
    }
}

impl Client {
    /// Constructs a new `Client`.
    pub fn new() -> Client {
        ClientBuilder::new().build().expect("Client::new()")
    }

    /// Creates a `ClientBuilder` to configure a `Client`.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Convenience method to make a `GET` request to a URL.
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.get(url)
    }

    /// Convenience method to make a `POST` request to a URL.
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.post(url)
    }

    /// Convenience method to make a `PUT` request to a URL.
    pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.put(url)
    }

    /// Convenience method to make a `PATCH` request to a URL.
    pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.patch(url)
    }

    /// Convenience method to make a `DELETE` request to a URL.
    pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.delete(url)
    }

    /// Convenience method to make a `HEAD` request to a URL.
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.inner.head(url)
    }

    /// Start building a `Request` with the `Method` and `Url`.
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        self.inner.request(method, url)
    }

    /// Executes a `Request`.
    pub async fn execute(&self, request: Request) -> Result<Response> {
        self.inner.execute(request).await
    }

    /// Get the default headers.
    pub fn headers(&self) -> &http::HeaderMap {
        self.inner.headers()
    }

    /// Get a mutable reference to the default headers.
    pub fn headers_mut(&mut self) -> &mut http::HeaderMap {
        self.inner.headers_mut()
    }

    /// Get cookies for the given URL.
    pub fn get_cookies(&self, url: &url::Url) -> Option<http::HeaderValue> {
        self.inner.get_cookies(url)
    }

    /// Set cookies for the given URL.
    pub fn set_cookies(&self, url: &url::Url, cookies: Vec<http::HeaderValue>) {
        self.inner.set_cookies(url, cookies)
    }

    /// Set proxies for the client.
    pub fn set_proxies(&mut self, proxies: Vec<reqwest::Proxy>) {
        self.inner.set_proxies(proxies)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Shortcut method to quickly make a `GET` request.
pub async fn get<T: IntoUrl>(url: T) -> crate::Result<Response> {
    Client::builder().build()?.get(url).send().await
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_impersonation(
    builder: reqwest::ClientBuilder,
    settings: BrowserSettings,
    custom_certs: &[reqwest::Certificate],
) -> crate::Result<reqwest::Client> {
    // Build rustls::ClientConfig with impersonation TLS settings
    let tls_config = build_impersonate_tls_config(&settings, custom_certs)?;

    // Apply TLS configuration
    let builder = builder.use_preconfigured_tls(tls_config);

    // Apply HTTP/2 settings
    let builder = apply_http2_settings(builder, &settings.http2);

    // Apply headers
    let builder = builder.default_headers(settings.headers);

    // Apply compression settings
    let mut builder = builder;
    if !settings.gzip {
        builder = builder.no_gzip();
    }
    if !settings.brotli {
        builder = builder.no_brotli();
    }
    if !settings.zstd {
        builder = builder.no_zstd();
    }

    builder.build()
}

#[cfg(not(target_arch = "wasm32"))]
fn build_impersonate_tls_config(
    settings: &BrowserSettings,
    custom_certs: &[reqwest::Certificate],
) -> crate::Result<rustls::ClientConfig> {
    use rustls::client::EchMode;
    use std::sync::Arc;

    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));

    // Use cached root store with both webpki roots and native OS root CAs.
    // If custom certs are provided, merge them in.
    let root_store = if custom_certs.is_empty() {
        reqwest::tls::default_root_store().clone()
    } else {
        reqwest::tls::merged_root_store(custom_certs.to_vec())?
    };

    // ECH GREASE is only used for Chrome-based browsers and Firefox to match JA4 fingerprint
    // Safari does NOT use ECH GREASE
    let needs_ech_grease =
        settings.browser_emulator.is_chrome_based() || settings.browser_emulator.is_firefox();

    let mut config = if needs_ech_grease {
        rustls::ClientConfig::builder_with_provider(provider)
            .with_ech(EchMode::Grease(get_ech_grease_config()))
            .expect("ECH GREASE config should be valid")
            .with_root_certificates(root_store)
            .with_no_client_auth()
    } else {
        rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("TLS versions should be valid")
            .with_root_certificates(root_store)
            .with_no_client_auth()
    };

    // Set browser emulation
    config.browser_emulation = Some(settings.browser_emulator.clone());

    // Set ALPN protocols
    config.alpn_protocols = vec!["h2".into(), "http/1.1".into()];

    // Filter cert_decompressors to match real browser behavior
    // Chrome/Edge/Opera only advertise brotli; Firefox advertises zlib,brotli
    if settings.browser_emulator.is_chrome_based() {
        config.cert_decompressors.retain(|d| {
            matches!(
                d.algorithm(),
                rustls::CertificateCompressionAlgorithm::Brotli
            )
        });
    } else if settings.browser_emulator.is_firefox() {
        // Firefox sends zlib before brotli
        config.cert_decompressors.sort_by(|a, b| {
            let a_is_zlib = matches!(a.algorithm(), rustls::CertificateCompressionAlgorithm::Zlib);
            let b_is_zlib = matches!(b.algorithm(), rustls::CertificateCompressionAlgorithm::Zlib);
            b_is_zlib.cmp(&a_is_zlib) // zlib first
        });
    }

    Ok(config)
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_http2_settings(
    builder: reqwest::ClientBuilder,
    http2: &Http2Data,
) -> reqwest::ClientBuilder {
    let mut builder = builder;

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
        builder = builder.http2_enable_connect_protocol(if val { 1u32 } else { 0u32 });
    }
    if let Some(stream_id) = http2.initial_stream_id {
        builder = builder.http2_initial_stream_id(stream_id);
    }

    builder
}

#[cfg(not(target_arch = "wasm32"))]
fn get_ech_grease_config() -> rustls::client::EchGreaseConfig {
    use rustls::client::EchGreaseConfig;
    use rustls::crypto::aws_lc_rs::hpke::DH_KEM_X25519_HKDF_SHA256_AES_128;
    use rustls::crypto::hpke::HpkePublicKey;
    use std::sync::OnceLock;

    /// GREASE_25519_PUBKEY - placeholder ECH public key for GREASE
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
