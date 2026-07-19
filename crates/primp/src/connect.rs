use http::header::HeaderValue;
use http::uri::Scheme;
use http::Uri;
use hyper::rt::{Read, ReadBufCursor, Write};
use hyper_util::client::legacy::connect::{Connected, Connection};
use hyper_util::rt::TokioIo;
use pin_project_lite::pin_project;
use tower::util::{BoxCloneSyncServiceLayer, MapRequestLayer};
use tower::{timeout::TimeoutLayer, util::BoxCloneSyncService, ServiceBuilder};
use tower_service::Service;

use std::future::Future;
use std::io::{self, IoSlice};
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::Duration;

use crate::dns::DynResolver;
use crate::error::{cast_to_internal_error, BoxError};
use crate::proxy::{redact_uri_userinfo, Intercepted, Matcher as ProxyMatcher};
use crate::strip_ipv6_brackets;
use sealed::{Conn, Unnameable};

pub(crate) type HttpConnector = hyper_util::client::legacy::connect::HttpConnector<DynResolver>;

#[derive(Clone)]
pub(crate) enum Connector {
    // base service, with or without an embedded timeout
    Simple(ConnectorService),
    // at least one custom layer along with maybe an outer timeout layer
    // from `builder.connect_timeout()`
    WithLayers(BoxCloneSyncService<Unnameable, Conn, BoxError>),
}

impl Service<Uri> for Connector {
    type Response = Conn;
    type Error = BoxError;
    type Future = Connecting;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self {
            Connector::Simple(service) => service.poll_ready(cx),
            Connector::WithLayers(service) => service.poll_ready(cx),
        }
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        match self {
            Connector::Simple(service) => service.call(dst),
            Connector::WithLayers(service) => service.call(Unnameable(dst)),
        }
    }
}

pub(crate) type BoxedConnectorService = BoxCloneSyncService<Unnameable, Conn, BoxError>;

pub(crate) type BoxedConnectorLayer =
    BoxCloneSyncServiceLayer<BoxedConnectorService, Unnameable, Conn, BoxError>;

pub(crate) struct ConnectorBuilder {
    inner: Inner,
    proxies: Arc<RwLock<Vec<ProxyMatcher>>>,
    verbose: verbose::Wrapper,
    timeout: Option<Duration>,
    nodelay: bool,
    tls_info: bool,
    user_agent: Option<HeaderValue>,
    resolver: Option<DynResolver>,
    #[cfg(unix)]
    unix_socket: Option<Arc<std::path::Path>>,
    #[cfg(target_os = "windows")]
    windows_named_pipe: Option<Arc<std::ffi::OsStr>>,
}

impl ConnectorBuilder {
    pub(crate) fn build(self, layers: Vec<BoxedConnectorLayer>) -> Connector {
        // construct the inner tower service
        let mut base_service = ConnectorService {
            inner: self.inner,
            proxies: self.proxies,
            verbose: self.verbose,
            nodelay: self.nodelay,
            tls_info: self.tls_info,
            user_agent: self.user_agent,
            simple_timeout: None,
            resolver: self.resolver.unwrap_or_else(DynResolver::gai),
            #[cfg(unix)]
            unix_socket: self.unix_socket,
            #[cfg(target_os = "windows")]
            windows_named_pipe: self.windows_named_pipe,
        };

        #[cfg(unix)]
        if base_service.unix_socket.is_some()
            && !base_service
                .proxies
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
        {
            base_service.proxies = Default::default();
            log::trace!("unix_socket() set, proxies are ignored");
        }
        #[cfg(target_os = "windows")]
        if base_service.windows_named_pipe.is_some()
            && !base_service
                .proxies
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
        {
            base_service.proxies = Default::default();
            log::trace!("windows_named_pipe() set, proxies are ignored");
        }

        if layers.is_empty() {
            // we have no user-provided layers, only use concrete types
            base_service.simple_timeout = self.timeout;
            return Connector::Simple(base_service);
        }

        // otherwise we have user provided layers
        // so we need type erasure all the way through
        // as well as mapping the unnameable type of the layers back to Uri for the inner service
        let unnameable_service = ServiceBuilder::new()
            .layer(MapRequestLayer::new(|request: Unnameable| request.0))
            .service(base_service);
        let mut service = BoxCloneSyncService::new(unnameable_service);

        for layer in layers {
            service = ServiceBuilder::new().layer(layer).service(service);
        }

        // now we handle the concrete stuff - any `connect_timeout`,
        // plus a final map_err layer we can use to cast default tower layer
        // errors to internal errors
        match self.timeout {
            Some(timeout) => {
                let service = ServiceBuilder::new()
                    .layer(TimeoutLayer::new(timeout))
                    .service(service);
                let service = ServiceBuilder::new()
                    .map_err(|error: BoxError| cast_to_internal_error(error))
                    .service(service);
                let service = BoxCloneSyncService::new(service);

                Connector::WithLayers(service)
            }
            None => {
                // no timeout, but still map err
                // no named timeout layer but we still map errors since
                // we might have user-provided timeout layer
                let service = ServiceBuilder::new().service(service);
                let service = ServiceBuilder::new()
                    .map_err(|error: BoxError| cast_to_internal_error(error))
                    .service(service);
                let service = BoxCloneSyncService::new(service);
                Connector::WithLayers(service)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_rustls_tls<T>(
        mut http: HttpConnector,
        tls: rustls::ClientConfig,
        proxies: Arc<RwLock<Vec<ProxyMatcher>>>,
        user_agent: Option<HeaderValue>,
        local_addr: T,
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
        interface: Option<&str>,
        nodelay: bool,
        tls_info: bool,
    ) -> ConnectorBuilder
    where
        T: Into<Option<IpAddr>>,
    {
        http.set_local_address(local_addr.into());
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
        if let Some(interface) = interface {
            http.set_interface(interface.to_owned());
        }
        http.set_nodelay(nodelay);
        http.enforce_http(false);

        let (tls, tls_proxy) = if proxies.read().unwrap_or_else(|e| e.into_inner()).is_empty() {
            let tls = Arc::new(tls);
            (tls.clone(), tls)
        } else {
            let mut tls_proxy = tls.clone();
            tls_proxy.alpn_protocols.clear();
            (Arc::new(tls), Arc::new(tls_proxy))
        };

        ConnectorBuilder {
            inner: Inner::RustlsTls {
                http,
                tls,
                tls_proxy,
            },
            proxies,
            verbose: verbose::OFF,
            nodelay,
            tls_info,
            user_agent,
            timeout: None,
            resolver: None,
            #[cfg(unix)]
            unix_socket: None,
            #[cfg(target_os = "windows")]
            windows_named_pipe: None,
        }
    }

    pub(crate) fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    pub(crate) fn set_verbose(&mut self, enabled: bool) {
        self.verbose.0 = enabled;
    }

    pub(crate) fn set_keepalive(&mut self, dur: Option<Duration>) {
        let Inner::RustlsTls { http, .. } = &mut self.inner;
        http.set_keepalive(dur);
    }

    pub(crate) fn set_keepalive_interval(&mut self, dur: Option<Duration>) {
        let Inner::RustlsTls { http, .. } = &mut self.inner;
        http.set_keepalive_interval(dur);
    }

    pub(crate) fn set_keepalive_retries(&mut self, retries: Option<u32>) {
        let Inner::RustlsTls { http, .. } = &mut self.inner;
        http.set_keepalive_retries(retries);
    }

    pub(crate) fn set_socks_resolver(&mut self, resolver: DynResolver) {
        self.resolver = Some(resolver);
    }

    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    pub(crate) fn set_tcp_user_timeout(&mut self, dur: Option<Duration>) {
        let Inner::RustlsTls { http, .. } = &mut self.inner;
        http.set_tcp_user_timeout(dur);
    }

    #[cfg(unix)]
    pub(crate) fn set_unix_socket(&mut self, path: Option<Arc<std::path::Path>>) {
        self.unix_socket = path;
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn set_windows_named_pipe(&mut self, pipe: Option<Arc<std::ffi::OsStr>>) {
        self.windows_named_pipe = pipe;
    }
}

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub(crate) struct ConnectorService {
    inner: Inner,
    proxies: Arc<RwLock<Vec<ProxyMatcher>>>,
    verbose: verbose::Wrapper,
    /// When the only configured layer is a single timeout, it is embedded in
    /// `Service::call` to avoid an extra `Box::pin` (`tokio::time::Timeout` is `Unpin`).
    simple_timeout: Option<Duration>,
    nodelay: bool,
    tls_info: bool,
    user_agent: Option<HeaderValue>,
    resolver: DynResolver,
    /// If set, this always takes priority over TCP.
    #[cfg(unix)]
    unix_socket: Option<Arc<std::path::Path>>,
    #[cfg(target_os = "windows")]
    windows_named_pipe: Option<Arc<std::ffi::OsStr>>,
}

#[derive(Clone)]
enum Inner {
    RustlsTls {
        http: HttpConnector,
        tls: Arc<rustls::ClientConfig>,
        tls_proxy: Arc<rustls::ClientConfig>,
    },
}

impl Inner {
    fn get_http_connector(&mut self) -> &mut crate::connect::HttpConnector {
        match self {
            Inner::RustlsTls { http, .. } => http,
        }
    }
}

impl ConnectorService {
    async fn connect_socks(mut self, dst: Uri, proxy: Intercepted) -> Result<Conn, BoxError> {
        let dns =
            match proxy.uri().scheme_str() {
                Some("socks4") | Some("socks5") => socks::DnsResolve::Local,
                Some("socks4a") | Some("socks5h") => socks::DnsResolve::Proxy,
                // The caller (`connect_via_proxy`) only routes socks* schemes here,
                // so this is normally unreachable — but under `panic = "abort"` an
                // `unreachable!()` here would abort the whole process (including
                // the Python interpreter). Return a graceful error instead.
                _ => return Err(
                    "connect_socks called for a non-socks proxy scheme (internal routing error)"
                        .into(),
                ),
            };

        let Inner::RustlsTls { http, tls, .. } = &mut self.inner;
        if dst.scheme() == Some(&Scheme::HTTPS) {
            use std::convert::TryFrom;

            let host = dst.host().ok_or("no host in url")?.to_string();
            let conn = socks::connect(proxy, dst, dns, &self.resolver, http).await?;
            let host_stripped = strip_ipv6_brackets(&host);
            let server_name = rustls_pki_types::ServerName::try_from(host_stripped.to_owned())
                .map_err(|_| "Invalid Server Name")?;
            let io =
                crate::tls_bridge::TokioTlsStream::connect(tls.clone(), server_name, conn).await?;
            return Ok(Conn {
                inner: self.verbose.wrap(rustls_tls_conn::PrimpTlsConn {
                    inner: TokioIo::new(io),
                }),
                is_proxy: false,
                tls_info: self.tls_info,
            });
        }

        let resolver = &self.resolver;
        let http = self.inner.get_http_connector();
        socks::connect(proxy, dst, dns, resolver, http)
            .await
            .map(|tcp| Conn {
                inner: self.verbose.wrap(TokioIo::new(tcp)),
                is_proxy: false,
                tls_info: false,
            })
            .map_err(Into::into)
    }

    async fn connect_with_maybe_proxy(self, dst: Uri, is_proxy: bool) -> Result<Conn, BoxError> {
        let Inner::RustlsTls { http, tls, .. } = self.inner;
        let mut http = http.clone();

        // Disable Nagle's algorithm for TLS handshake
        //
        // https://www.openssl.org/docs/man1.1.1/man3/SSL_connect.html#NOTES
        if !self.nodelay && (dst.scheme() == Some(&Scheme::HTTPS)) {
            http.set_nodelay(true);
        }

        let is_https = dst.scheme() == Some(&Scheme::HTTPS);
        let host = dst.host().map(|h| h.to_owned());
        let tcp = http.call(dst).await?;

        if is_https {
            use rustls_pki_types::ServerName;
            use std::convert::TryFrom;

            let host = host.ok_or("no host in url")?;
            let host_stripped = strip_ipv6_brackets(&host);
            let server_name = ServerName::try_from(host_stripped.to_owned())
                .map_err(|_| "Invalid Server Name")?;
            let mut io =
                crate::tls_bridge::TokioTlsStream::connect(tls, server_name, tcp.into_inner())
                    .await?;
            if !self.nodelay {
                io.get_io_mut().set_nodelay(false)?;
            }
            Ok(Conn {
                inner: self.verbose.wrap(rustls_tls_conn::PrimpTlsConn {
                    inner: TokioIo::new(io),
                }),
                is_proxy,
                tls_info: self.tls_info,
            })
        } else {
            Ok(Conn {
                inner: self.verbose.wrap(tcp),
                is_proxy,
                tls_info: false,
            })
        }
    }

    /// Connect over a local transport: a Unix Domain Socket (Unix) or Windows Named Pipe (Windows).
    #[cfg(any(unix, target_os = "windows"))]
    async fn connect_local_transport(self, dst: Uri) -> Result<Conn, BoxError> {
        #[cfg(unix)]
        #[allow(unused_mut)]
        let mut svc = {
            let path = match self.unix_socket.as_ref() {
                Some(p) => p.clone(),
                // Under `panic = "abort"` a `.expect()` here would kill the host
                // interpreter; `connect_local_transport` is only reached when a
                // unix socket was configured, so this is a graceful fallback
                // rather than an unreachable invariant.
                None => return Err("connect local must have socket path".into()),
            };
            tower::service_fn(move |_| {
                let fut = tokio::net::UnixStream::connect(path.clone());
                async move {
                    let io = fut.await?;
                    Ok::<_, std::io::Error>(TokioIo::new(io))
                }
            })
        };
        #[cfg(target_os = "windows")]
        #[allow(unused_mut)]
        let mut svc = {
            use tokio::net::windows::named_pipe::ClientOptions;
            let pipe = match self.windows_named_pipe.as_ref() {
                Some(p) => p.clone(),
                // Graceful fallback instead of `.expect()` — see above.
                None => return Err("connect local must have pipe path".into()),
            };
            tower::service_fn(move |_| {
                let pipe = pipe.clone();
                async move { ClientOptions::new().open(pipe).map(TokioIo::new) }
            })
        };
        let is_proxy = false;
        let Inner::RustlsTls { tls, .. } = self.inner;
        let is_https = dst.scheme() == Some(&Scheme::HTTPS);
        let host = dst.host().map(|h| h.to_owned());
        let io = svc.call(dst).await?;

        if is_https {
            use rustls_pki_types::ServerName;
            use std::convert::TryFrom;

            let host = host.ok_or("no host in url")?;
            let host_stripped = strip_ipv6_brackets(&host);
            let server_name = ServerName::try_from(host_stripped.to_owned())
                .map_err(|_| "Invalid Server Name")?;
            let io = crate::tls_bridge::TokioTlsStream::connect(
                tls.clone(),
                server_name,
                io.into_inner(),
            )
            .await?;
            Ok(Conn {
                inner: self.verbose.wrap(rustls_tls_conn::PrimpTlsConn {
                    inner: TokioIo::new(io),
                }),
                is_proxy,
                tls_info: self.tls_info,
            })
        } else {
            Ok(Conn {
                inner: self.verbose.wrap(io),
                is_proxy,
                tls_info: false,
            })
        }
    }

    async fn connect_via_proxy(self, dst: Uri, proxy: Intercepted) -> Result<Conn, BoxError> {
        log::debug!(
            "proxy({}) intercepts '{:?}'",
            redact_uri_userinfo(proxy.uri()),
            dst.host()
        );

        match proxy.uri().scheme_str().ok_or("proxy scheme expected")? {
            "socks4" | "socks4a" | "socks5" | "socks5h" => {
                return self.connect_socks(dst, proxy).await
            }
            _ => (),
        }

        let proxy_dst = proxy.uri().clone();
        let auth = proxy.basic_auth().cloned();

        let misc = proxy.custom_headers();

        let Inner::RustlsTls {
            http,
            tls,
            tls_proxy,
        } = &self.inner;
        if dst.scheme() == Some(&Scheme::HTTPS) {
            use rustls_pki_types::ServerName;
            use std::convert::TryFrom;

            log::trace!("tunneling HTTPS over proxy");
            let http = http.clone();
            let tls_proxy = tls_proxy.clone();

            let inner = tower::service_fn(move |uri: Uri| {
                let mut http = http.clone();
                let tls_proxy = tls_proxy.clone();
                async move {
                    let tcp = http.call(uri.clone()).await?;
                    if uri.scheme() == Some(&Scheme::HTTPS) {
                        let host = uri.host().unwrap_or("").to_owned();
                        let host_stripped = strip_ipv6_brackets(&host);
                        let server_name = ServerName::try_from(host_stripped.to_owned())
                            .map_err(|_| "invalid server name")?;
                        let tls = crate::tls_bridge::TokioTlsStream::connect(
                            tls_proxy,
                            server_name,
                            tcp.into_inner(),
                        )
                        .await?;
                        Ok::<_, BoxError>(
                            Box::new(TokioIo::new(tls)) as Box<dyn crate::tls_bridge::TlsIoHyper>
                        )
                    } else {
                        Ok::<_, BoxError>(Box::new(tcp) as Box<dyn crate::tls_bridge::TlsIoHyper>)
                    }
                }
            });

            let mut tunnel =
                hyper_util::client::legacy::connect::proxy::Tunnel::new(proxy_dst, inner);
            if let Some(auth) = auth {
                tunnel = tunnel.with_auth(auth);
            }
            if let Some(ua) = self.user_agent {
                let mut headers = http::HeaderMap::new();
                headers.insert(http::header::USER_AGENT, ua);
                tunnel = tunnel.with_headers(headers);
            }
            // Note that custom headers may override the user agent header.
            if let Some(custom_headers) = misc {
                tunnel = tunnel.with_headers(custom_headers.clone());
            }

            let tunneled = tunnel.call(dst.clone()).await?;
            let host = dst.host().ok_or("no host in url")?.to_string();
            let host_stripped = strip_ipv6_brackets(&host);
            let server_name = ServerName::try_from(host_stripped.to_owned())
                .map_err(|_| "Invalid Server Name")?;
            let io = crate::tls_bridge::TokioTlsStream::connect(
                tls.clone(),
                server_name,
                TokioIo::new(tunneled),
            )
            .await?;

            return Ok(Conn {
                inner: self.verbose.wrap(rustls_tls_conn::PrimpTlsConn {
                    inner: TokioIo::new(io),
                }),
                is_proxy: false,
                tls_info: self.tls_info,
            });
        }

        self.connect_with_maybe_proxy(proxy_dst, true).await
    }

    #[cfg(any(unix, target_os = "windows"))]
    fn should_use_local_transport(&self) -> bool {
        #[cfg(unix)]
        return self.unix_socket.is_some();

        #[cfg(target_os = "windows")]
        return self.windows_named_pipe.is_some();
    }
}

async fn with_timeout<T, F>(timeout: Option<Duration>, f: F) -> Result<T, BoxError>
where
    F: Future<Output = Result<T, BoxError>>,
{
    if let Some(to) = timeout {
        match tokio::time::timeout(to, f).await {
            Err(_elapsed) => Err(Box::new(io::Error::new(
                io::ErrorKind::TimedOut,
                "connect timeout",
            )) as BoxError),
            Ok(Ok(try_res)) => Ok(try_res),
            Ok(Err(e)) => Err(e),
        }
    } else {
        f.await
    }
}

impl Service<Uri> for ConnectorService {
    type Response = Conn;
    type Error = BoxError;
    type Future = Connecting;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        log::debug!("starting new connection '{:?}'", dst.host());
        let timeout = self.simple_timeout;

        // Local transports (UDS, Windows Named Pipes) skip proxies
        #[cfg(any(unix, target_os = "windows"))]
        if self.should_use_local_transport() {
            return Box::pin(with_timeout(
                timeout,
                self.clone().connect_local_transport(dst),
            ));
        }

        let proxies = self.proxies.read().unwrap_or_else(|e| e.into_inner());
        for prox in proxies.iter() {
            match prox.intercept(&dst) {
                Ok(Some(intercepted)) => {
                    return Box::pin(with_timeout(
                        timeout,
                        self.clone().connect_via_proxy(dst, intercepted),
                    ));
                }
                Ok(None) => continue,
                Err(e) => {
                    return Box::pin(async move { Err(e.into()) });
                }
            }
        }

        Box::pin(with_timeout(
            timeout,
            self.clone().connect_with_maybe_proxy(dst, false),
        ))
    }
}

trait TlsInfoFactory {
    fn tls_info(&self) -> Option<crate::tls::TlsInfo>;
}

impl<T: TlsInfoFactory> TlsInfoFactory for TokioIo<T> {
    fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
        self.inner().tls_info()
    }
}

// ===== TcpStream =====

impl TlsInfoFactory for tokio::net::TcpStream {
    fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
        None
    }
}

// ===== Box<dyn AsyncConnWithInfo> =====

impl TlsInfoFactory for Box<dyn AsyncConnWithInfo> {
    fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
        (**self).tls_info()
    }
}

// ===== UnixStream =====

#[cfg(unix)]
impl TlsInfoFactory for tokio::net::UnixStream {
    fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
        None
    }
}

// ===== NamedPipe =====

#[cfg(target_os = "windows")]
impl TlsInfoFactory for tokio::net::windows::named_pipe::NamedPipeClient {
    fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
        None
    }
}

pub(crate) trait AsyncConn:
    Read + Write + Connection + Send + Sync + Unpin + 'static
{
}

impl<T: Read + Write + Connection + Send + Sync + Unpin + 'static> AsyncConn for T {}

trait AsyncConnWithInfo: AsyncConn + TlsInfoFactory {}

impl<T: AsyncConn + TlsInfoFactory> AsyncConnWithInfo for T {}

type BoxConn = Box<dyn AsyncConnWithInfo>;

pub(crate) mod sealed {
    use super::*;
    #[derive(Debug)]
    pub struct Unnameable(pub(super) Uri);

    pin_project! {
        /// `is_proxy` means *is a plain-text HTTP proxy*. When false, hyper writes
        /// origin-form (`GET /path`); otherwise absolute-form (`GET http://foo.bar/path`).
        #[allow(missing_debug_implementations)]
        pub struct Conn {
            #[pin]
            pub(super)inner: BoxConn,
            pub(super) is_proxy: bool,
            // Only needed for __tls, but #[cfg()] on fields breaks pin_project!
            pub(super) tls_info: bool,
        }
    }

    impl Connection for Conn {
        fn connected(&self) -> Connected {
            let connected = self.inner.connected().proxy(self.is_proxy);
            if self.tls_info {
                let tls_info = self.inner.tls_info();
                if let Some(tls_info) = tls_info {
                    return connected.extra(tls_info);
                }
            }
            connected
        }
    }

    impl Read for Conn {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: ReadBufCursor<'_>,
        ) -> Poll<io::Result<()>> {
            let this = self.project();
            Read::poll_read(this.inner, cx, buf)
        }
    }

    impl Write for Conn {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            let this = self.project();
            Write::poll_write(this.inner, cx, buf)
        }

        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize, io::Error>> {
            let this = self.project();
            Write::poll_write_vectored(this.inner, cx, bufs)
        }

        fn is_write_vectored(&self) -> bool {
            self.inner.is_write_vectored()
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
            let this = self.project();
            Write::poll_flush(this.inner, cx)
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
            let this = self.project();
            Write::poll_shutdown(this.inner, cx)
        }
    }
}

// Some sealed things for UDS
#[cfg(unix)]
pub(crate) mod uds {
    use std::path::Path;

    /// Sealed provider of Unix Domain Socket paths; controls who can implement
    /// it so support can expand later.
    #[cfg(unix)]
    pub trait UnixSocketProvider {
        #[doc(hidden)]
        fn primp_uds_path(&self, _: Internal) -> &Path;
    }

    #[allow(missing_debug_implementations)]
    pub struct Internal;

    macro_rules! as_path {
        ($($t:ty,)+) => {
            $(
                impl UnixSocketProvider for $t {
                    #[doc(hidden)]
                    fn primp_uds_path(&self, _: Internal) -> &Path {
                        self.as_ref()
                    }
                }
            )+
        }
    }

    as_path![
        String,
        &'_ str,
        &'_ Path,
        std::path::PathBuf,
        std::sync::Arc<Path>,
    ];
}

// Sealed trait for Windows Named Pipe support
#[cfg(target_os = "windows")]
pub(crate) mod windows_named_pipe {
    use std::ffi::OsStr;
    /// Sealed provider of Windows Named Pipe paths; controls who can implement
    /// it so support can expand later.
    #[cfg(target_os = "windows")]
    pub trait WindowsNamedPipeProvider {
        #[doc(hidden)]
        fn primp_windows_named_pipe_path(&self, _: Internal) -> &OsStr;
    }

    #[allow(missing_debug_implementations)]
    pub struct Internal;

    macro_rules! as_os_str {
        ($($t:ty,)+) => {
            $(
                impl WindowsNamedPipeProvider for $t {
                    #[doc(hidden)]
                    fn primp_windows_named_pipe_path(&self, _: Internal) -> &OsStr {
                        self.as_ref()
                    }
                }
            )+
        }
    }

    as_os_str![
        String,
        &'_ str,
        std::path::PathBuf,
        &'_ std::path::Path,
        std::ffi::OsString,
        &'_ OsStr,
    ];
}

pub(crate) type Connecting = Pin<Box<dyn Future<Output = Result<Conn, BoxError>> + Send>>;

mod rustls_tls_conn {
    use super::TlsInfoFactory;
    use hyper::rt::{Read, ReadBufCursor, Write};
    use hyper_util::client::legacy::connect::{Connected, Connection};
    use hyper_util::rt::TokioIo;
    use pin_project_lite::pin_project;
    use std::{
        io::{self, IoSlice},
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::io::{AsyncRead, AsyncWrite};

    /// Delegates to [`Connection::connected`] on the inner IO, used by
    /// [`PrimpTlsConn`] to propagate transport metadata (addresses, keepalive)
    /// through TLS. The `IO: Connection` bound on its `Connection` impl
    /// guarantees delegation is valid for all construction-site IO types.
    trait HasConnected {
        fn has_connected(&self) -> Connected;
    }

    impl<T: Connection> HasConnected for T {
        fn has_connected(&self) -> Connected {
            self.connected()
        }
    }

    pin_project! {
        pub(super) struct PrimpTlsConn<IO> {
            #[pin] pub(super) inner: TokioIo<crate::tls_bridge::TokioTlsStream<IO>>,
        }
    }

    impl<IO: Connection> Connection for PrimpTlsConn<IO> {
        fn connected(&self) -> Connected {
            let tls = self.inner.inner();
            let io = tls.get_ref();
            let base = io.has_connected();
            if tls.alpn_protocol() == Some(b"h2") {
                base.negotiated_h2()
            } else {
                base
            }
        }
    }

    impl<IO: AsyncRead + AsyncWrite + Unpin> Read for PrimpTlsConn<IO> {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: ReadBufCursor<'_>,
        ) -> Poll<io::Result<()>> {
            let this = self.project();
            Read::poll_read(this.inner, cx, buf)
        }
    }

    impl<IO: AsyncRead + AsyncWrite + Unpin> Write for PrimpTlsConn<IO> {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            let this = self.project();
            Write::poll_write(this.inner, cx, buf)
        }

        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize, io::Error>> {
            let this = self.project();
            Write::poll_write_vectored(this.inner, cx, bufs)
        }

        fn is_write_vectored(&self) -> bool {
            self.inner.is_write_vectored()
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
            let this = self.project();
            Write::poll_flush(this.inner, cx)
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
            let this = self.project();
            Write::poll_shutdown(this.inner, cx)
        }
    }

    impl<IO> TlsInfoFactory for PrimpTlsConn<IO> {
        fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
            let tls = self.inner.inner();
            let peer_certificate = tls
                .peer_certificates()
                .and_then(|certs| certs.first())
                .map(|cert| cert.as_ref().to_vec());
            let version = tls
                .protocol_version()
                .and_then(crate::tls::Version::from_rustls);
            peer_certificate.map(|pc| crate::tls::TlsInfo {
                peer_certificate: Some(pc),
                version,
            })
        }
    }
}

pub(crate) mod socks {
    use std::net::IpAddr;

    use http::uri::Scheme;
    use http::Uri;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tower_service::Service;

    use super::BoxError;
    use crate::proxy::Intercepted;

    pub(crate) enum DnsResolve {
        Local,
        Proxy,
    }

    #[derive(Debug)]
    #[allow(clippy::enum_variant_names)]
    pub(crate) enum SocksProxyError {
        SocksNoHostInUrl,
        SocksLocalResolve(BoxError),
        SocksConnect(BoxError),
        SocksInvalidAuth,
        SocksUnsupportedScheme,
    }

    fn socks_err(e: std::io::Error) -> SocksProxyError {
        SocksProxyError::SocksConnect(Box::new(e))
    }

    pub(crate) async fn connect(
        proxy: Intercepted,
        dst: Uri,
        dns_mode: DnsResolve,
        resolver: &crate::dns::DynResolver,
        http_connector: &mut crate::connect::HttpConnector,
    ) -> Result<TcpStream, SocksProxyError> {
        let https = dst.scheme() == Some(&Scheme::HTTPS);
        // `Uri::host()` keeps IPv6 brackets; the handshake needs the bare
        // literal, or the proxy would receive `[::1]` as a DOMAIN name.
        let original_host = dst.host().ok_or(SocksProxyError::SocksNoHostInUrl)?;
        let mut host = crate::strip_ipv6_brackets(original_host).to_owned();
        let port = match dst.port() {
            Some(p) => p.as_u16(),
            None if https => 443u16,
            _ => 80u16,
        };

        if let DnsResolve::Local = dns_mode {
            let maybe_new_target = resolver
                .http_resolve(&dst)
                .await
                .map_err(SocksProxyError::SocksLocalResolve)?
                .next();
            if let Some(new_target) = maybe_new_target {
                log::trace!("socks local dns resolved {new_target:?}");
                host = new_target.ip().to_string();
            }
        }

        let scheme = proxy
            .uri()
            .scheme_str()
            .ok_or(SocksProxyError::SocksUnsupportedScheme)?;
        let tcp = http_connector
            .call(proxy.uri().clone())
            .await
            .map_err(|e| SocksProxyError::SocksConnect(Box::new(e)))?;
        let tcp = tcp.into_inner();

        match scheme {
            "socks4" | "socks4a" => {
                if host.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
                    // SOCKS4 addresses are 4 bytes; IPv6 cannot be expressed.
                    return Err(socks_err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "socks4 does not support IPv6 destinations",
                    )));
                }
                let is_4a = scheme == "socks4a";
                handshake_v4(tcp, &host, port, is_4a).await
            }
            "socks5" | "socks5h" => {
                // Source SOCKS5 credentials from the proxy. URL-embedded
                // credentials are stored as `Auth::Raw` by hyper-util and are
                // only reachable via `socks_auth()` (NOT `basic_auth()`, which
                // returns `None` for those URLs). Explicit `custom_http_auth`
                // Basic headers are also honored as a fallback.
                let auth = proxy.socks_auth();
                if let Some((username, password)) = &auth {
                    // RFC 1929 single-octet length prefixes cap each at 255.
                    if username.len() > 255 || password.len() > 255 {
                        return Err(SocksProxyError::SocksInvalidAuth);
                    }
                }
                handshake_v5(tcp, &host, port, auth).await
            }
            _ => Err(SocksProxyError::SocksUnsupportedScheme),
        }
    }

    /// SOCKS4/4a CONNECT handshake. `is_4a` enables the domain-name request
    /// form (DSTIP = 0.0.0.0 + domain suffix); plain SOCKS4 requires the
    /// destination to already be an IPv4 address.
    async fn handshake_v4(
        mut tcp: TcpStream,
        host: &str,
        port: u16,
        is_4a: bool,
    ) -> Result<TcpStream, SocksProxyError> {
        let host_is_ip = host.parse::<IpAddr>().is_ok();
        let mut req = Vec::with_capacity(9 + host.len());
        req.extend_from_slice(&[0x04, 0x01]);
        req.extend_from_slice(&port.to_be_bytes());
        match host.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => req.extend_from_slice(&ip.octets()),
            _ if is_4a => req.extend_from_slice(&[0, 0, 0, 0]),
            _ => {
                return Err(socks_err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("socks4 cannot route the host {host:?} without SOCKS4a"),
                )));
            }
        }
        req.push(0); // empty user id
        if is_4a && !host_is_ip {
            req.extend_from_slice(host.as_bytes());
            req.push(0);
        }
        tcp.write_all(&req).await.map_err(socks_err)?;

        // Reply: VN CD DSTPORT DSTIP (8 bytes); CD 0x5A = success.
        let mut res = [0u8; 8];
        tcp.read_exact(&mut res).await.map_err(socks_err)?;
        if res[1] != 0x5A {
            return Err(socks_err(std::io::Error::other(format!(
                "socks4 proxy connect failed with status {}",
                res[1]
            ))));
        }
        Ok(tcp)
    }

    /// SOCKS5 CONNECT handshake: greeting, optional user/pass (RFC 1929), the
    /// CONNECT request (IPv4/IPv6/domain), and the proxy reply.
    async fn handshake_v5(
        mut tcp: TcpStream,
        host: &str,
        port: u16,
        auth: Option<(String, String)>,
    ) -> Result<TcpStream, SocksProxyError> {
        let method = if auth.is_some() { 0x02 } else { 0x00 };
        tcp.write_all(&[0x05, 0x01, method])
            .await
            .map_err(socks_err)?;

        let mut res = [0u8; 2];
        tcp.read_exact(&mut res).await.map_err(socks_err)?;
        if res[0] != 0x05 {
            return Err(socks_err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "socks5 proxy replied with an invalid version",
            )));
        }
        if res[1] == 0xFF {
            return Err(socks_err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "socks5 proxy accepts no authentication methods",
            )));
        }
        if res[1] != method {
            return Err(socks_err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "socks5 proxy chose an unexpected authentication method",
            )));
        }

        if let Some((username, password)) = auth {
            let mut req = Vec::with_capacity(3 + username.len() + password.len());
            req.push(0x01);
            req.push(username.len() as u8);
            req.extend_from_slice(username.as_bytes());
            req.push(password.len() as u8);
            req.extend_from_slice(password.as_bytes());
            tcp.write_all(&req).await.map_err(socks_err)?;
            tcp.read_exact(&mut res).await.map_err(socks_err)?;
            if res[0] != 0x01 || res[1] != 0x00 {
                return Err(socks_err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "socks5 proxy rejected the credentials",
                )));
            }
        }

        let mut req = vec![0x05, 0x01, 0x00];
        match host.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => {
                req.push(0x01);
                req.extend_from_slice(&ip.octets());
            }
            Ok(IpAddr::V6(ip)) => {
                req.push(0x04);
                req.extend_from_slice(&ip.octets());
            }
            Err(_) if host.len() <= 255 => {
                req.push(0x03);
                req.push(host.len() as u8);
                req.extend_from_slice(host.as_bytes());
            }
            Err(_) => {
                return Err(socks_err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "socks5 destination host exceeds 255 bytes",
                )));
            }
        }
        req.extend_from_slice(&port.to_be_bytes());
        tcp.write_all(&req).await.map_err(socks_err)?;

        // Reply: VER REP RSV ATYP BND.ADDR BND.PORT (max 261 bytes).
        let mut head = [0u8; 4];
        tcp.read_exact(&mut head).await.map_err(socks_err)?;
        if head[0] != 0x05 {
            return Err(socks_err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "socks5 proxy replied with an invalid version",
            )));
        }
        if head[1] != 0x00 {
            return Err(socks_err(std::io::Error::other(format!(
                "socks5 proxy connect failed with status {}",
                head[1]
            ))));
        }
        let addr_len = match head[3] {
            0x01 => 4,
            0x03 => {
                let mut len = [0u8; 1];
                tcp.read_exact(&mut len).await.map_err(socks_err)?;
                len[0] as usize
            }
            0x04 => 16,
            atyp => {
                return Err(socks_err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("socks5 proxy replied with an invalid ATYP {atyp}"),
                )));
            }
        };
        let mut rest = vec![0u8; addr_len + 2];
        tcp.read_exact(&mut rest).await.map_err(socks_err)?;
        Ok(tcp)
    }

    impl std::fmt::Display for SocksProxyError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::SocksNoHostInUrl => f.write_str("socks proxy destination has no host"),
                Self::SocksLocalResolve(_) => f.write_str("error resolving for socks proxy"),
                Self::SocksConnect(_) => f.write_str("error connecting to socks proxy"),
                Self::SocksInvalidAuth => {
                    f.write_str("socks5 username/password must each be at most 255 bytes")
                }
                Self::SocksUnsupportedScheme => f.write_str("unsupported socks proxy scheme"),
            }
        }
    }

    impl std::error::Error for SocksProxyError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::SocksNoHostInUrl => None,
                Self::SocksLocalResolve(ref e) => Some(&**e),
                Self::SocksConnect(ref e) => Some(&**e),
                Self::SocksInvalidAuth => None,
                Self::SocksUnsupportedScheme => None,
            }
        }
    }
}

mod verbose {
    use crate::util::Escape;
    use hyper::rt::{Read, ReadBufCursor, Write};
    use hyper_util::client::legacy::connect::{Connected, Connection};
    use std::cmp::min;
    use std::fmt;
    use std::io::{self, IoSlice};
    use std::pin::Pin;
    use std::task::{Context, Poll};

    pub(super) const OFF: Wrapper = Wrapper(false);

    #[derive(Clone, Copy)]
    pub(super) struct Wrapper(pub(super) bool);

    impl Wrapper {
        pub(super) fn wrap<T: super::AsyncConnWithInfo>(&self, conn: T) -> super::BoxConn {
            if self.0 && log::log_enabled!(log::Level::Trace) {
                Box::new(Verbose {
                    // truncate is fine
                    id: crate::util::fast_random() as u32,
                    inner: conn,
                })
            } else {
                Box::new(conn)
            }
        }
    }

    struct Verbose<T> {
        id: u32,
        inner: T,
    }

    impl<T: Connection + Read + Write + Unpin> Connection for Verbose<T> {
        fn connected(&self) -> Connected {
            self.inner.connected()
        }
    }

    impl<T: Read + Write + Unpin> Read for Verbose<T> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            mut buf: ReadBufCursor<'_>,
        ) -> Poll<std::io::Result<()>> {
            // TODO: This _does_ forget the `init` len, so it could result in
            // re-initializing twice. Needs upstream support, perhaps.
            // SAFETY: Passing to a ReadBuf will never de-initialize any bytes.
            let mut vbuf = hyper::rt::ReadBuf::uninit(unsafe { buf.as_mut() });
            match Pin::new(&mut self.inner).poll_read(cx, vbuf.unfilled()) {
                Poll::Ready(Ok(())) => {
                    log::trace!("{:08x} read: {:?}", self.id, Escape::new(vbuf.filled()));
                    let len = vbuf.filled().len();
                    // SAFETY: The two cursors were for the same buffer. What was
                    // filled in one is safe in the other.
                    unsafe {
                        buf.advance(len);
                    }
                    Poll::Ready(Ok(()))
                }
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    impl<T: Read + Write + Unpin> Write for Verbose<T> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            match Pin::new(&mut self.inner).poll_write(cx, buf) {
                Poll::Ready(Ok(n)) => {
                    log::trace!("{:08x} write: {:?}", self.id, Escape::new(&buf[..n]));
                    Poll::Ready(Ok(n))
                }
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Pending => Poll::Pending,
            }
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize, io::Error>> {
            match Pin::new(&mut self.inner).poll_write_vectored(cx, bufs) {
                Poll::Ready(Ok(nwritten)) => {
                    log::trace!(
                        "{:08x} write (vectored): {:?}",
                        self.id,
                        Vectored { bufs, nwritten }
                    );
                    Poll::Ready(Ok(nwritten))
                }
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Pending => Poll::Pending,
            }
        }

        fn is_write_vectored(&self) -> bool {
            self.inner.is_write_vectored()
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
        ) -> Poll<Result<(), std::io::Error>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context,
        ) -> Poll<Result<(), std::io::Error>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }

    impl<T: super::TlsInfoFactory> super::TlsInfoFactory for Verbose<T> {
        fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
            self.inner.tls_info()
        }
    }

    struct Vectored<'a, 'b> {
        bufs: &'a [IoSlice<'b>],
        nwritten: usize,
    }

    impl fmt::Debug for Vectored<'_, '_> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let mut left = self.nwritten;
            for buf in self.bufs.iter() {
                if left == 0 {
                    break;
                }
                let n = min(left, buf.len());
                Escape::new(&buf[..n]).fmt(f)?;
                left -= n;
            }
            Ok(())
        }
    }
}
