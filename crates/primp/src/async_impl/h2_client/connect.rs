//! HTTP/2 client connector.
//!
//! Handles DNS resolution, TCP connection, TLS handshake, and h2 negotiation
//! for the dedicated HTTP/2 client.

use std::io;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use bytes::Bytes;
use h2::client::Builder;
use http::Uri;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use tokio::net::TcpStream;

use crate::dns::DynResolver;
use crate::error::BoxError;
use crate::proxy::Matcher as ProxyMatcher;
use crate::strip_ipv6_brackets;

/// Apply an optional timeout to a future returning `Result<T, BoxError>`.
async fn with_timeout<F>(timeout: Option<Duration>, f: F) -> Result<F::Output, BoxError>
where
    F: std::future::Future,
{
    if let Some(to) = timeout {
        match tokio::time::timeout(to, f).await {
            Ok(result) => Ok(result),
            Err(_) => Err(Box::new(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection timed out",
            )) as BoxError),
        }
    } else {
        Ok(f.await)
    }
}

/// Configuration for the H2 client, mapped from the request-level config.
///
/// Each field corresponds to an `http2_*` `ClientBuilder` setting and is
/// applied to the `h2::client::Builder` during connection setup.
#[derive(Clone, Debug, Default)]
pub(crate) struct H2ClientConfig {
    pub(crate) settings_order: Option<h2::frame::SettingsOrder>,
    pub(crate) headers_pseudo_order: Option<h2::frame::PseudoOrder>,
    pub(crate) headers_order: Option<Vec<http::HeaderName>>,
    pub(crate) headers_priority: Option<(u8, u32, bool)>,
    pub(crate) initial_stream_window_size_increment: Option<u32>,
    pub(crate) initial_connection_window_size: Option<u32>,
    pub(crate) enable_push: Option<bool>,
    pub(crate) max_concurrent_streams: Option<u32>,
    pub(crate) initial_window_size: Option<u32>,
    pub(crate) max_frame_size: Option<u32>,
    pub(crate) max_header_list_size: Option<u32>,
    pub(crate) header_table_size: Option<u32>,
    pub(crate) no_rfc7540_priorities: Option<bool>,
    pub(crate) initial_stream_id: Option<u32>,
    pub(crate) enable_connect_protocol: Option<u32>,
    pub(crate) adaptive_window: bool,
    pub(crate) keep_alive_interval: Option<Duration>,
    pub(crate) keep_alive_timeout: Option<Duration>,
    pub(crate) keep_alive_while_idle: bool,
}

/// Connector for establishing H2 connections over TLS.
#[derive(Clone)]
pub(crate) struct H2Connector {
    pub(crate) resolver: DynResolver,
    pub(crate) tls: Option<Arc<ClientConfig>>,
    /// TLS config for connecting to HTTPS proxies (no ALPN protocols).
    pub(crate) tls_proxy: Option<Arc<ClientConfig>>,
    pub(crate) config: H2ClientConfig,
    pub(crate) nodelay: bool,
    pub(crate) connect_timeout: Option<Duration>,
    pub(crate) proxies: Arc<RwLock<Vec<ProxyMatcher>>>,
    pub(crate) user_agent: Option<http::HeaderValue>,
    pub(crate) tls_info: bool,
}

/// Outcome of an h2 connect attempt: a negotiated HTTP/2 connection, or an
/// HTTP/1.1 connection already established on the same TLS handshake. The
/// connector-level counterpart of [`super::pool::ConnectOutcome`].
pub(crate) enum H2ConnectOutcome {
    H2(super::pool::H2ConnectResult),
    Http1 {
        key: String,
        stream: Box<dyn super::pool::AsyncStream + Unpin + Send + 'static>,
        tls_info: Option<crate::tls::TlsInfo>,
    },
}

/// Wraps a stream so bytes read ahead during CONNECT (the leftover payload
/// after the `\r\n\r\n` terminator) are replayed first, before delegating to
/// the inner stream — preventing desync of the downstream TLS/h2 handshake
/// when a proxy pipelines payload with its `200` CONNECT response.
struct LeftoverReplayStream<S> {
    inner: S,
    leftover: Bytes,
    leftover_pos: usize,
}

impl<S> tokio::io::AsyncRead for LeftoverReplayStream<S>
where
    S: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = &mut *self;
        if this.leftover_pos < this.leftover.len() {
            // If the caller handed us a zero-length buffer we can make no
            // progress on the replayed leftover. Per tokio's AsyncRead
            // convention, handing a full buffer to poll_read is a caller
            // error and the correct response is to return `Ready(Ok(()))`
            // without polling the inner stream (which has no leftover data
            // left to provide and would only register a waker for data we
            // must not consume yet — the caller still owes us bytes).
            if buf.remaining() == 0 {
                return std::task::Poll::Ready(Ok(()));
            }
            let remaining = &this.leftover[this.leftover_pos..];
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            this.leftover_pos += to_copy;
            return std::task::Poll::Ready(Ok(()));
        }
        // Leftover fully drained: delegate to the inner stream, which
        // registers `cx.waker()` for the next read.
        std::pin::Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl<S> tokio::io::AsyncWrite for LeftoverReplayStream<S>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl H2Connector {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        resolver: DynResolver,
        tls: Option<Arc<ClientConfig>>,
        tls_proxy: Option<Arc<ClientConfig>>,
        config: H2ClientConfig,
        nodelay: bool,
        connect_timeout: Option<Duration>,
        proxies: Arc<RwLock<Vec<ProxyMatcher>>>,
        user_agent: Option<http::HeaderValue>,
        tls_info: bool,
    ) -> Self {
        H2Connector {
            resolver,
            tls,
            tls_proxy,
            config,
            nodelay,
            connect_timeout,
            proxies,
            user_agent,
            tls_info,
        }
    }

    pub(crate) async fn connect(&self, uri: &Uri) -> Result<H2ConnectOutcome, BoxError> {
        // Check if a proxy intercepts this request.
        let mut proxy: Option<crate::proxy::Intercepted> = None;
        for p in self
            .proxies
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            match p.intercept(uri) {
                Ok(Some(inter)) => {
                    proxy = Some(inter);
                    break;
                }
                Ok(None) => continue,
                Err(e) => return Err(e.into()),
            }
        }

        if let Some(ref proxy) = proxy {
            let scheme = proxy.uri().scheme_str().unwrap_or("");
            match scheme {
                "socks4" | "socks4a" | "socks5" | "socks5h" => {
                    return self.connect_via_socks_proxy(uri, proxy).await;
                }
                _ => {
                    return self.connect_via_http_proxy(uri, proxy).await;
                }
            }
        }

        self.connect_direct(uri).await
    }

    /// Connect through an HTTP/HTTPS proxy via CONNECT tunneling.
    ///
    /// H2 requires TLS; plain HTTP targets through a proxy return
    /// [`H2NegotiationFailed`] so the caller falls back to HTTP/1.1.
    async fn connect_via_http_proxy(
        &self,
        uri: &Uri,
        proxy: &crate::proxy::Intercepted,
    ) -> Result<H2ConnectOutcome, BoxError> {
        let use_tls = uri.scheme_str() == Some("https");
        if !use_tls {
            // H2 requires TLS; HTTP-over-HTTP-proxy must use HTTP/1.1.
            return Err(super::H2NegotiationFailed.into());
        }

        let host = uri.host().ok_or("no host in url")?;
        let port = uri.port_u16().unwrap_or(443);

        let host_stripped = strip_ipv6_brackets(host);
        let server_name = Some(
            ServerName::try_from(host_stripped.to_owned()).map_err(|_| "Invalid Server Name")?,
        );

        // Resolve the proxy address.
        let proxy_uri = proxy.uri();
        let proxy_host = proxy_uri.host().ok_or("proxy uri has no host")?;
        let proxy_port =
            proxy_uri
                .port_u16()
                .unwrap_or(if proxy_uri.scheme_str() == Some("https") {
                    443
                } else {
                    8080
                });
        // Build a synthetic URI so we can use http_resolve.
        let proxy_resolve_uri = Uri::builder()
            .scheme("http")
            .authority(format!("{}:{}", proxy_host, proxy_port))
            .path_and_query("/")
            .build()
            .map_err(|e| -> BoxError { e.into() })?;
        let proxy_addrs: Vec<_> = self
            .resolver
            .http_resolve(&proxy_resolve_uri)
            .await?
            .collect();
        if proxy_addrs.is_empty() {
            return Err(crate::error::dns(
                "proxy dns resolution returned no addresses",
            ));
        }

        // Try each proxy address.
        let mut last_err: BoxError = "no proxy addresses to try".into();
        for proxy_addr in &proxy_addrs {
            // TCP connect to proxy.
            let tcp = match with_timeout(self.connect_timeout, TcpStream::connect(proxy_addr)).await
            {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => {
                    last_err = Box::new(e);
                    continue;
                }
                Err(e) => {
                    last_err = e;
                    continue;
                }
            };
            if let Err(e) = tcp.set_nodelay(self.nodelay) {
                last_err = Box::new(e);
                continue;
            }

            // For HTTPS proxies, establish TLS to the proxy before CONNECT.
            let proxy_is_https = proxy_uri.scheme_str() == Some("https");
            if proxy_is_https {
                let tls_proxy_config = match self.tls_proxy.as_ref() {
                    Some(c) => c.clone(),
                    None => {
                        last_err = "https proxy requires TLS configuration".into();
                        continue;
                    }
                };
                let proxy_host_for_tls = proxy_uri.host().unwrap_or("").to_owned();
                let proxy_host_stripped = strip_ipv6_brackets(&proxy_host_for_tls);
                let proxy_server_name = match ServerName::try_from(proxy_host_stripped.to_owned()) {
                    Ok(s) => s,
                    Err(_) => {
                        last_err = "invalid proxy server name".into();
                        continue;
                    }
                };
                let tls_connect = crate::tls_bridge::TokioTlsStream::connect(
                    tls_proxy_config,
                    proxy_server_name,
                    tcp,
                );
                let tls_stream = match with_timeout(self.connect_timeout, tls_connect).await {
                    Ok(Ok(s)) => s,
                    Ok(Err(e)) => {
                        last_err = Box::new(e);
                        continue;
                    }
                    Err(e) => {
                        last_err = e;
                        continue;
                    }
                };
                // CONNECT through the TLS-to-proxy tunnel.
                match self
                    .connect_through_proxy_tcp(tls_stream, host, port, proxy)
                    .await
                {
                    Ok(tunneled) => {
                        let result = self.tls_and_h2_handshake(tunneled, uri, &server_name).await;
                        match result {
                            Err(e) => {
                                last_err = e;
                                continue;
                            }
                            Ok(result) => return Ok(result),
                        }
                    }
                    Err(e) => {
                        last_err = e;
                        continue;
                    }
                }
            }

            // HTTP proxy: CONNECT directly through the TCP stream.
            match self.connect_through_proxy_tcp(tcp, host, port, proxy).await {
                Ok(tunneled) => {
                    let result = self.tls_and_h2_handshake(tunneled, uri, &server_name).await;
                    match result {
                        Err(e) => {
                            last_err = e;
                            continue;
                        }
                        Ok(result) => return Ok(result),
                    }
                }
                Err(e) => {
                    last_err = e;
                    continue;
                }
            }
        }
        Err(last_err)
    }

    /// Connect through a SOCKS proxy.
    async fn connect_via_socks_proxy(
        &self,
        uri: &Uri,
        proxy: &crate::proxy::Intercepted,
    ) -> Result<H2ConnectOutcome, BoxError> {
        let host = uri.host().ok_or("no host in url")?;
        let use_tls = uri.scheme_str() == Some("https");

        let server_name = if use_tls {
            let host_stripped = strip_ipv6_brackets(host);
            Some(
                ServerName::try_from(host_stripped.to_owned())
                    .map_err(|_| "Invalid Server Name")?,
            )
        } else {
            None
        };

        let dns_mode = match proxy.uri().scheme_str() {
            Some("socks4") | Some("socks5") => crate::connect::socks::DnsResolve::Local,
            Some("socks4a") | Some("socks5h") => crate::connect::socks::DnsResolve::Proxy,
            // Only socks* schemes reach here; still, under `panic = "abort"`
            // an `unreachable!()` would abort the whole process including the
            // Python interpreter — so return a graceful error.
            _ => {
                return Err(
                    "connect_via_socks_proxy called for a non-socks proxy scheme (internal routing error)"
                        .into(),
                )
            }
        };

        let mut http_connector =
            crate::connect::HttpConnector::new_with_resolver(self.resolver.clone());
        http_connector.enforce_http(false);
        http_connector.set_nodelay(self.nodelay);

        // SOCKS handshake: TCP to the proxy + protocol negotiation. Wrap in
        // the same connect_timeout used by the HTTP-proxy / direct paths so
        // an unreachable proxy doesn't hang the request indefinitely.
        // `tls_and_h2_handshake` below already has its own inner timeouts.
        let tcp = match with_timeout(
            self.connect_timeout,
            crate::connect::socks::connect(
                proxy.clone(),
                uri.clone(),
                dns_mode,
                &self.resolver,
                &mut http_connector,
            ),
        )
        .await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => return Err(e.into()),
            Err(e) => return Err(e),
        };

        self.tls_and_h2_handshake(tcp, uri, &server_name).await
    }

    /// Send HTTP CONNECT through a TCP stream and return the tunneled stream.
    ///
    /// Any bytes read from the proxy *after* the CONNECT response
    /// (`\r\n\r\n`) are buffered and replayed first on the returned stream,
    /// so the downstream TLS/h2 handshake never sees a desynced stream when
    /// a proxy pipelines payload with its `200` response.
    #[allow(unused_assignments)]
    async fn connect_through_proxy_tcp<S>(
        &self,
        mut stream: S,
        target_host: &str,
        target_port: u16,
        proxy: &crate::proxy::Intercepted,
    ) -> Result<Box<dyn super::pool::AsyncStream + Unpin + Send + 'static>, BoxError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        // Build CONNECT request.
        let mut request = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n",
            target_host, target_port, target_host, target_port,
        );
        if let Some(auth) = proxy.basic_auth() {
            use std::fmt::Write;
            write!(
                request,
                "Proxy-Authorization: {}\r\n",
                auth.to_str().unwrap_or("")
            )
            .ok();
        }
        if let Some(ua) = &self.user_agent {
            use std::fmt::Write;
            write!(request, "User-Agent: {}\r\n", ua.to_str().unwrap_or("")).ok();
        }
        if let Some(headers) = proxy.custom_headers() {
            for (key, value) in headers.iter() {
                use std::fmt::Write;
                write!(
                    request,
                    "{}: {}\r\n",
                    key.as_str(),
                    value.to_str().unwrap_or("")
                )
                .ok();
            }
        }
        request.push_str("\r\n");

        // Send the CONNECT request and read the response. The exchange is
        // bounded by `connect_timeout` so a proxy that accepts but never
        // answers cannot hang the request.
        // Cap the header section to prevent unbounded growth from a hostile or
        // misbehaving proxy (typical HTTP header limits are 8–16 KiB). The cap
        // is enforced against the *header portion only* — the proxy is free to
        // pipeline arbitrary payload bytes (e.g. a TLS ClientHello or early
        // data) after the `\r\n\r\n` terminator, and those leftover bytes are
        // preserved for replay.
        const MAX_PROXY_RESPONSE_BYTES: usize = 8 * 1024;
        let exchange = async {
            tokio::io::AsyncWriteExt::write_all(&mut stream, request.as_bytes()).await?;
            tokio::io::AsyncWriteExt::flush(&mut stream).await?;

            // Read the CONNECT response, looking for \r\n\r\n (end of headers).
            let mut buf = Vec::with_capacity(512);
            let mut chunk = [0u8; 1024];
            // Index just past the `\r\n\r\n` terminator; bytes after it are the
            // leftover payload that must be replayed before reading the stream.
            let mut header_end: usize = 0;

            'outer: loop {
                let n = tokio::io::AsyncReadExt::read(&mut stream, &mut chunk).await?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "proxy closed connection before CONNECT response",
                    )
                    .into());
                }
                // Append the entire chunk first so any bytes that follow the
                // header terminator within this read are preserved (they become
                // the replayed leftover) rather than skipped.
                let start = buf.len();
                buf.extend_from_slice(&chunk[..n]);
                // Scan from `start - 3` so a `\r\n\r\n` sequence that straddles the
                // boundary between the old buffer and the newly read chunk (e.g.
                // `\r\n\r` in the previous read + `\n` in this one) is still
                // detected. `saturating_sub` handles the first iteration when the
                // buffer is still short.
                let mut i = start.saturating_sub(3);
                while i + 3 < buf.len() {
                    if &buf[i..i + 4] == b"\r\n\r\n" {
                        header_end = i + 4;
                        break 'outer;
                    }
                    i += 1;
                }
                // No terminator yet: only the header section can keep growing.
                // Enforce the cap against the buffered (header) bytes. A header
                // section that reaches the limit is rejected; the exact boundary
                // (== MAX) is allowed so a valid header that ends exactly at MAX
                // is not wrongly refused.
                if buf.len() > MAX_PROXY_RESPONSE_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "proxy CONNECT response exceeded maximum size",
                    )
                    .into());
                }
            }

            Ok::<_, BoxError>((buf, header_end))
        };

        let (buf, header_end) = match with_timeout(self.connect_timeout, exchange).await {
            Ok(Ok(pair)) => pair,
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(e),
        };

        // The terminator was found: enforce the cap against the header
        // portion only (`buf` may hold pipelined payload bytes beyond it).
        // Without this, a header whose `\r\n\r\n` lands inside the read that
        // crosses the cap would slip through at up to MAX + chunk_size.
        if header_end > MAX_PROXY_RESPONSE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "proxy CONNECT response exceeded maximum size",
            )
            .into());
        }

        // Check the response status.
        let response = String::from_utf8_lossy(&buf[..header_end]);
        let mut tokens = response.split_whitespace();
        let version = tokens.next().unwrap_or("");
        let status = tokens.next().unwrap_or("000");
        if !version.starts_with("HTTP/") || status != "200" {
            return Err(
                io::Error::other(format!("proxy CONNECT failed: {}", response.trim())).into(),
            );
        }

        // Replay any bytes the proxy sent after the CONNECT response before
        // forwarding reads to the underlying stream.
        let leftover: Bytes = Bytes::copy_from_slice(&buf[header_end..]);
        Ok(Box::new(LeftoverReplayStream {
            inner: stream,
            leftover,
            leftover_pos: 0,
        }))
    }

    /// Perform TLS and H2 handshake on a pre-established (possibly tunneled) stream.
    async fn tls_and_h2_handshake<S>(
        &self,
        tcp: S,
        uri: &Uri,
        server_name: &Option<ServerName<'static>>,
    ) -> Result<H2ConnectOutcome, BoxError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let use_tls = uri.scheme_str() == Some("https");

        let h2_builder = self.make_h2_builder();

        if use_tls {
            let server_name = server_name
                .as_ref()
                .ok_or("https requires a server name for the TLS handshake")?
                .clone();

            let tls = self
                .tls
                .as_ref()
                .ok_or("https requires TLS configuration")?;
            let tls_stream = with_timeout(
                self.connect_timeout,
                crate::tls_bridge::TokioTlsStream::connect(tls.clone(), server_name, tcp),
            )
            .await??;

            match tls_stream.alpn_protocol() {
                Some(alpn) if alpn == b"h2" => {}
                _ => {
                    // Server picked HTTP/1.1 — return the *already established*
                    // TLS stream so the caller can run HTTP/1.1 over it without
                    // performing a second TLS handshake.
                    let tls_info = if self.tls_info {
                        let peer_certificate = tls_stream
                            .peer_certificates()
                            .and_then(|certs| certs.first())
                            .map(|cert| cert.as_ref().to_vec());
                        let version = tls_stream
                            .protocol_version()
                            .and_then(crate::tls::Version::from_rustls);
                        peer_certificate.map(|pc| crate::tls::TlsInfo {
                            peer_certificate: Some(pc),
                            version,
                        })
                    } else {
                        None
                    };
                    return Ok(H2ConnectOutcome::Http1 {
                        key: super::pool::pool_key(uri),
                        stream: Box::new(tls_stream)
                            as Box<dyn super::pool::AsyncStream + Unpin + Send + 'static>,
                        tls_info,
                    });
                }
            }

            let tls_info = if self.tls_info {
                let peer_certificate = tls_stream
                    .peer_certificates()
                    .and_then(|certs| certs.first())
                    .map(|cert| cert.as_ref().to_vec());
                let version = tls_stream
                    .protocol_version()
                    .and_then(crate::tls::Version::from_rustls);
                peer_certificate.map(|pc| crate::tls::TlsInfo {
                    peer_certificate: Some(pc),
                    version,
                })
            } else {
                None
            };

            let (sr, mut conn) = with_timeout(
                self.connect_timeout,
                h2_builder.handshake::<_, Bytes>(tls_stream),
            )
            .await??;

            if self.config.adaptive_window {
                conn.set_target_window_size(1 << 20);
            }

            let ping_pong = conn.ping_pong();

            let conn: super::pool::H2Connection = Box::pin(conn);
            let keep_alive = super::pool::KeepAliveConfig {
                interval: self.config.keep_alive_interval,
                timeout: self.config.keep_alive_timeout,
                while_idle: self.config.keep_alive_while_idle,
            };

            Ok(H2ConnectOutcome::H2(super::pool::H2ConnectResult {
                send_request: sr,
                connection: conn,
                ping_pong,
                keep_alive,
                tls_info,
            }))
        } else {
            // h2c: direct h2 handshake over cleartext (prior knowledge).
            let (sr, mut conn) =
                with_timeout(self.connect_timeout, h2_builder.handshake::<_, Bytes>(tcp)).await??;

            if self.config.adaptive_window {
                conn.set_target_window_size(1 << 20);
            }

            let ping_pong = conn.ping_pong();

            let conn: super::pool::H2Connection = Box::pin(conn);
            let keep_alive = super::pool::KeepAliveConfig {
                interval: self.config.keep_alive_interval,
                timeout: self.config.keep_alive_timeout,
                while_idle: self.config.keep_alive_while_idle,
            };

            Ok(H2ConnectOutcome::H2(super::pool::H2ConnectResult {
                send_request: sr,
                connection: conn,
                ping_pong,
                keep_alive,
                tls_info: None,
            }))
        }
    }

    /// Direct connection (no proxy).
    async fn connect_direct(&self, uri: &Uri) -> Result<H2ConnectOutcome, BoxError> {
        let host = uri.host().ok_or("no host in url")?;
        let use_tls = uri.scheme_str() == Some("https");

        let server_name = if use_tls {
            let host_stripped = strip_ipv6_brackets(host);
            Some(
                ServerName::try_from(host_stripped.to_owned())
                    .map_err(|_| "Invalid Server Name")?,
            )
        } else {
            None
        };

        let addrs: Vec<_> = self.resolver.http_resolve(uri).await?.collect();
        if addrs.is_empty() {
            return Err(crate::error::dns("dns resolution returned no addresses"));
        }

        let mut last_err: BoxError = "no addresses to try".into();
        for addr in &addrs {
            let tcp = match with_timeout(self.connect_timeout, TcpStream::connect(addr)).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => {
                    last_err = Box::new(e);
                    continue;
                }
                Err(e) => {
                    last_err = e;
                    continue;
                }
            };
            if let Err(e) = tcp.set_nodelay(self.nodelay) {
                last_err = Box::new(e);
                continue;
            }

            match self.tls_and_h2_handshake(tcp, uri, &server_name).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_err = e;
                    continue;
                }
            }
        }

        Err(last_err)
    }

    fn make_h2_builder(&self) -> Builder {
        let mut h2_builder = Builder::new();

        if let Some(ref order) = self.config.settings_order {
            h2_builder.settings_order(order.clone());
        }
        if let Some(ref order) = self.config.headers_pseudo_order {
            h2_builder.headers_pseudo_order(order.clone());
        }
        if let Some(ref order) = self.config.headers_order {
            h2_builder.headers_order(order.clone());
        }
        if let Some(data) = self.config.headers_priority {
            h2_builder.headers_priority(Some(data));
        }
        if let Some(incr) = self.config.initial_stream_window_size_increment {
            h2_builder.initial_stream_window_size_increment(incr);
        }
        if let Some(size) = self.config.initial_connection_window_size {
            h2_builder.initial_connection_window_size(size);
        }
        if let Some(enabled) = self.config.enable_push {
            h2_builder.enable_push(enabled);
        }
        if let Some(max) = self.config.max_concurrent_streams {
            h2_builder.max_concurrent_streams(max);
        }
        if let Some(size) = self.config.initial_window_size {
            h2_builder.initial_window_size(size);
        }
        if let Some(size) = self.config.max_frame_size {
            h2_builder.max_frame_size(size);
        }
        if let Some(size) = self.config.max_header_list_size {
            h2_builder.max_header_list_size(size);
        }
        if let Some(size) = self.config.header_table_size {
            h2_builder.header_table_size(size);
        }
        if let Some(enabled) = self.config.no_rfc7540_priorities {
            h2_builder.no_rfc7540_priorities(u32::from(enabled));
        }
        if let Some(stream_id) = self.config.initial_stream_id {
            h2_builder.initial_stream_id(stream_id);
        }
        if let Some(val) = self.config.enable_connect_protocol {
            h2_builder.enable_connect_protocol(val);
        }

        h2_builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn connect_through_proxy_tcp_honors_connect_timeout() {
        // A proxy that accepts but never replies must fail within
        // connect_timeout instead of hanging the request.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            // Read the CONNECT request, then never reply — stall forever.
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            std::future::pending::<()>().await;
        });

        let proxy_uri: Uri = format!("http://{}", addr).parse().unwrap();
        let matcher = crate::proxy::Proxy::all(proxy_uri.to_string())
            .unwrap()
            .into_matcher();
        let target: Uri = "https://example.com:443/".parse().unwrap();
        let intercepted = matcher.intercept(&target).unwrap().unwrap();

        let connector = H2Connector::new(
            crate::dns::DynResolver::gai(),
            None,
            None,
            H2ClientConfig::default(),
            false,
            Some(std::time::Duration::from_millis(300)),
            Arc::new(RwLock::new(Vec::new())),
            None,
            false,
        );

        let tcp = TcpStream::connect(addr).await.unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            connector.connect_through_proxy_tcp(tcp, "example.com", 443, &intercepted),
        )
        .await;

        server_task.abort();

        let result = result.expect("connect_through_proxy_tcp must not hang past the timeout");
        assert!(
            result.is_err(),
            "a stalling proxy must fail with a connect_timeout error, got Ok"
        );
    }

    #[tokio::test]
    async fn connect_via_socks_proxy_rejects_non_socks_scheme_with_error() {
        // The dns-mode match used to `unreachable!()` on non-socks schemes;
        // that would abort the process under `panic = "abort"`. It must
        // surface as a graceful `Err` instead.
        let proxy_uri: Uri = "http://127.0.0.1:8080".parse().unwrap();
        let matcher = crate::proxy::Proxy::all(proxy_uri.to_string())
            .unwrap()
            .into_matcher();
        let target: Uri = "https://example.com/".parse().unwrap();
        let intercepted = matcher.intercept(&target).unwrap().unwrap();

        let connector = H2Connector::new(
            crate::dns::DynResolver::gai(),
            None,
            None,
            H2ClientConfig::default(),
            false,
            None,
            Arc::new(RwLock::new(Vec::new())),
            None,
            false,
        );

        let result = connector
            .connect_via_socks_proxy(&target, &intercepted)
            .await;
        assert!(
            result.is_err(),
            "a non-socks proxy scheme must be a graceful error, not an abort"
        );
    }

    #[tokio::test]
    async fn leftover_replay_stream_preserves_bytes() {
        // The desync bug: bytes read ahead during CONNECT (the leftover
        // payload following the response terminator) must be replayed first,
        // before the underlying stream is read.
        let inner = std::io::Cursor::new(b"INNER_BYTES".to_vec());
        let mut stream = LeftoverReplayStream {
            inner,
            leftover: Bytes::from_static(b"LEFTOVER_"),
            leftover_pos: 0,
        };

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, b"LEFTOVER_INNER_BYTES");
    }

    #[tokio::test]
    async fn connect_through_proxy_tcp_replays_trailing_bytes() {
        // A proxy that pipelines its first response payload together with the
        // `200` CONNECT response must not desync the tunneled stream.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let proxy_uri: Uri = format!("http://{}", addr).parse().unwrap();
        let matcher = crate::proxy::Proxy::all(proxy_uri.to_string())
            .unwrap()
            .into_matcher();
        let target: Uri = "https://example.com:443/".parse().unwrap();
        let intercepted = matcher.intercept(&target).unwrap().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            // Read the CONNECT request.
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await.unwrap();
            // Reply with 200 + pipelined payload, then wait until the peer
            // has consumed the trailing payload before closing, so the
            // tunneled stream receives it intact.
            sock.write_all(b"HTTP/1.1 200 Connection established\r\n\r\nTRAFFIC_AFTER_CONNECT")
                .await
                .unwrap();
            sock.flush().await.unwrap();
            // Keep the connection open until the client closes or a short
            // timeout elapses; this lets the trailing bytes drain to the
            // tunneled stream instead of being discarded on close.
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(5), sock.read(&mut buf)).await;
        });

        let connector = H2Connector::new(
            crate::dns::DynResolver::gai(),
            None,
            None,
            H2ClientConfig::default(),
            false,
            None,
            Arc::new(RwLock::new(Vec::new())),
            None,
            false,
        );

        let tcp = TcpStream::connect(addr).await.unwrap();
        let mut tunneled = connector
            .connect_through_proxy_tcp(tcp, "example.com", 443, &intercepted)
            .await
            .unwrap();

        // The trailing payload the proxy pipelined after the `200` must be
        // replayed on the returned stream. Read until we see it (with a
        // timeout so a regression fails loudly instead of hanging).
        let mut buf = Vec::new();
        let read_task = tokio::spawn(async move {
            let mut chunk = [0u8; 64];
            loop {
                match tunneled.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    Err(_) => break,
                }
                if buf.len() >= 20 {
                    break;
                }
            }
            buf
        });
        let got = tokio::time::timeout(std::time::Duration::from_secs(5), read_task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&got[..], b"TRAFFIC_AFTER_CONNECT");

        server_task.abort();
    }

    /// Regression test: a valid `200` CONNECT response whose header section is
    /// large (but under the cap) must NOT be rejected. Previously the size
    /// guard ran *before* the `\r\n\r\n` scan and rejected any buffered bytes
    /// reaching 8 KiB, even when the status was valid (broke corporate
    /// forward proxies with large header blocks / TLS-terminating proxies that
    /// pipeline early data after the `200`).
    #[tokio::test]
    async fn connect_through_proxy_tcp_allows_large_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let proxy_uri: Uri = format!("http://{}", addr).parse().unwrap();
        let matcher = crate::proxy::Proxy::all(proxy_uri.to_string())
            .unwrap()
            .into_matcher();
        let target: Uri = "https://example.com:443/".parse().unwrap();
        let intercepted = matcher.intercept(&target).unwrap().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await.unwrap();
            // Header block of exactly 4 KiB of padding (well under the 8 KiB
            // cap) plus the status line and pad header name.
            let padding: String = "X".repeat(4 * 1024);
            let resp = format!(
                "HTTP/1.1 200 Connection established\r\nX-Pad: {}\r\n\r\n",
                padding
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.flush().await.unwrap();
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(5), sock.read(&mut buf)).await;
        });

        let connector = H2Connector::new(
            crate::dns::DynResolver::gai(),
            None,
            None,
            H2ClientConfig::default(),
            false,
            None,
            Arc::new(RwLock::new(Vec::new())),
            None,
            false,
        );

        let tcp = TcpStream::connect(addr).await.unwrap();
        let mut tunneled = connector
            .connect_through_proxy_tcp(tcp, "example.com", 443, &intercepted)
            .await
            .expect("valid 200 CONNECT with large headers must not be rejected");

        use tokio::io::AsyncWriteExt;
        tunneled.write_all(b"PING").await.unwrap();
        tunneled.flush().await.unwrap();

        server_task.abort();
    }

    /// Regression test: a CONNECT header section that *exceeds* the cap is
    /// still rejected, so the size guard retains its protection.
    #[tokio::test]
    async fn connect_through_proxy_tcp_rejects_oversized_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let proxy_uri: Uri = format!("http://{}", addr).parse().unwrap();
        let matcher = crate::proxy::Proxy::all(proxy_uri.to_string())
            .unwrap()
            .into_matcher();
        let target: Uri = "https://example.com:443/".parse().unwrap();
        let intercepted = matcher.intercept(&target).unwrap().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await.unwrap();
            let padding: String = "X".repeat(16 * 1024);
            let resp = format!(
                "HTTP/1.1 200 Connection established\r\nX-Pad: {}\r\n\r\n",
                padding
            );
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.flush().await;
        });

        let connector = H2Connector::new(
            crate::dns::DynResolver::gai(),
            None,
            None,
            H2ClientConfig::default(),
            false,
            None,
            Arc::new(RwLock::new(Vec::new())),
            None,
            false,
        );

        let tcp = TcpStream::connect(addr).await.unwrap();
        let err = connector
            .connect_through_proxy_tcp(tcp, "example.com", 443, &intercepted)
            .await
            .err()
            .expect("oversized CONNECT header must be rejected");
        assert!(
            err.to_string().contains("exceeded maximum size"),
            "unexpected error: {err}"
        );

        server_task.abort();
    }

    /// Regression test for the `\r\n\r\n` scan: when the terminator straddles
    /// the boundary between two reads (e.g. `\r\n\r` ends the first read and
    /// `\n` starts the second), the sliding-window scan must still find it.
    ///
    /// The old code started scanning from `start` (the position where new data
    /// was appended), so a straddling pattern at positions
    /// `[start-1, start, start+1, start+2]` was never examined. The fix scans
    /// from `start.saturating_sub(3)` to cover boundary-crossing sequences.
    ///
    /// The response is crafted so that the first chunk is exactly 1024 bytes
    /// (filling the client's read buffer) ending with `\r`, and the second
    /// chunk starts with `\n\r\n`, completing the terminator at the seam.
    /// The server uses `set_nodelay(true)` and a 100ms inter-write delay so
    /// the client reads each chunk in a separate `read()` call.
    #[tokio::test]
    async fn connect_through_proxy_tcp_detects_straddling_terminator() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Build the two chunks. Chunk 1 must be exactly 1024 bytes so the
        // client's first read (buf = [0u8; 1024]) returns it all.
        let status_line = b"HTTP/1.1 200 Connection established\r\n";
        let pad_len = 1024 - status_line.len() - 1; // reserve final byte for '\r'
        let mut chunk1 = Vec::with_capacity(1024);
        chunk1.extend_from_slice(status_line);
        chunk1.extend(std::iter::repeat(b'X').take(pad_len));
        chunk1.push(b'\r'); // position 1023 — last byte of first read
        assert_eq!(chunk1.len(), 1024);

        // Chunk 2 starts with '\n' which, combined with the trailing '\r' of
        // chunk 1, completes the \r\n\r\n terminator at positions 1023-1026.
        let mut chunk2 = Vec::new();
        chunk2.extend_from_slice(b"\n\r\nTRAILING_DATA_AFTER_CONNECT");

        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            sock.set_nodelay(true).unwrap_or(());
            // Read the CONNECT request the client sends.
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            // Write chunk 1, flush, then wait so the client reads this
            // chunk before chunk 2 arrives (two separate read() calls).
            sock.write_all(&chunk1).await.unwrap();
            sock.flush().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            // Write chunk 2 (starts with \n, completing the straddling \r\n\r\n).
            sock.write_all(&chunk2).await.unwrap();
            sock.flush().await.unwrap();
            // Wait for the client to close (or timeout).
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(5), sock.read(&mut buf)).await;
        });

        let proxy_uri: Uri = format!("http://{}", addr).parse().unwrap();
        let matcher = crate::proxy::Proxy::all(proxy_uri.to_string())
            .unwrap()
            .into_matcher();
        let target: Uri = "https://example.com:443/".parse().unwrap();
        let intercepted = matcher.intercept(&target).unwrap().unwrap();

        let connector = H2Connector::new(
            crate::dns::DynResolver::gai(),
            None,
            None,
            H2ClientConfig::default(),
            false,
            None,
            Arc::new(RwLock::new(Vec::new())),
            None,
            false,
        );

        let tcp = TcpStream::connect(addr).await.unwrap();
        let result = connector
            .connect_through_proxy_tcp(tcp, "example.com", 443, &intercepted)
            .await;

        assert!(
            result.is_ok(),
            "CONNECT must succeed when \\r\\n\\r\\n straddles a read boundary: {:?}",
            result.err()
        );

        // The trailing data pipelined after the CONNECT response must be
        // replayed on the tunneled stream.
        let mut tunneled = result.unwrap();
        let mut buf = Vec::new();
        let read_task = tokio::spawn(async move {
            let mut chunk = [0u8; 64];
            loop {
                match tunneled.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    Err(_) => break,
                }
                if buf.len() >= 22 {
                    break;
                }
            }
            buf
        });
        let got = tokio::time::timeout(std::time::Duration::from_secs(5), read_task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&got[..], b"TRAILING_DATA_AFTER_CONNECT");

        server_task.abort();
    }

    /// Regression test: the cap applies to the header *portion* even when the
    /// `\r\n\r\n` terminator is found on a read that crosses the 8 KiB mark.
    /// Previously the size check only ran on the no-terminator path, so an
    /// oversized header whose terminator landed in the crossing read slipped
    /// through: the first 8 KiB are read as 8×1024 bytes (no terminator), the
    /// final read brings the section past the cap and *ends* with the
    /// terminator, and the old code `break`-out before checking the cap.
    #[tokio::test]
    async fn connect_through_proxy_tcp_rejects_terminator_past_cap() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let proxy_uri: Uri = format!("http://{}", addr).parse().unwrap();
        let matcher = crate::proxy::Proxy::all(proxy_uri.to_string())
            .unwrap()
            .into_matcher();
        let target: Uri = "https://example.com:443/".parse().unwrap();
        let intercepted = matcher.intercept(&target).unwrap().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await.unwrap();
            // Header portion (incl. terminator) = 8 KiB + 6: the terminator
            // sits just past the 8 KiB cap and is only ever found on the read
            // that crosses the boundary. Compute the padding from the exact
            // response structure so the assert below is self-consistent.
            let status_line = "HTTP/1.1 200 Connection established\r\n";
            let pad_name = "X-Pad: ";
            let header_len = 8 * 1024 + 6;
            let padding_len = header_len - status_line.len() - pad_name.len() - 4;
            let resp = format!(
                "{}{}{}\r\n\r\n",
                status_line,
                pad_name,
                "X".repeat(padding_len)
            );
            assert_eq!(resp.len(), header_len);
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.flush().await;
        });

        let connector = H2Connector::new(
            crate::dns::DynResolver::gai(),
            None,
            None,
            H2ClientConfig::default(),
            false,
            None,
            Arc::new(RwLock::new(Vec::new())),
            None,
            false,
        );

        let tcp = TcpStream::connect(addr).await.unwrap();
        let err = connector
            .connect_through_proxy_tcp(tcp, "example.com", 443, &intercepted)
            .await
            .err()
            .expect("CONNECT header section past the 8 KiB cap must be rejected");
        assert!(
            err.to_string().contains("exceeded maximum size"),
            "unexpected error: {err}"
        );

        server_task.abort();
    }
}
