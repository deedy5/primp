use crate::async_impl::h3_client::dns::resolve;
use crate::dns::DynResolver;
use crate::error::BoxError;
use bytes::Bytes;
use h3::client::SendRequest;
use h3_quinn::{Connection, OpenStreams};
use http::Uri;
use hyper_util::client::legacy::connect::dns::Name;
use quinn::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use quinn::rustls::client::verify_server_cert_signed_by_trust_anchor;
use quinn::rustls::crypto::WebPkiSupportedAlgorithms;
use quinn::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use quinn::rustls::server::ParsedCertificate;
use quinn::rustls::{
    DigitallySignedStruct, Error as QuinnRustlsError, RootCertStore, SignatureScheme,
};
use quinn::{ClientConfig, Endpoint, TransportConfig};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

type H3Connection = (
    h3::client::Connection<Connection, Bytes>,
    SendRequest<OpenStreams, Bytes>,
);

const HAPPY_EYEBALLS_DELAY: Duration = Duration::from_millis(250);

/// HTTP/3 client configuration.
#[derive(Clone, Default)]
pub(crate) struct H3ClientConfig {
    /// Maximum HTTP/3 header size this client accepts (see RFC 9114 §header
    /// size constraints; forwarded to `h3`'s `Builder::max_field_section_size`).
    pub(crate) max_field_section_size: Option<u64>,

    /// Whether to send HTTP/3 protocol grease, ensuring the protocol can evolve
    /// without breaking implementations (forwarded to `h3`'s `send_grease`).
    pub(crate) send_grease: Option<bool>,
}

#[derive(Clone)]
pub(crate) struct H3Connector {
    resolver: DynResolver,
    endpoint: Endpoint,
    client_config: H3ClientConfig,
    local_addr: Option<IpAddr>,
    connect_timeout: Option<Duration>,
}

impl H3Connector {
    pub fn new(
        resolver: DynResolver,
        quinn_config: ClientConfig,
        local_addr: Option<IpAddr>,
        transport_config: TransportConfig,
        client_config: H3ClientConfig,
        connect_timeout: Option<Duration>,
    ) -> Result<H3Connector, BoxError> {
        let mut config = quinn_config;
        // FIXME: Replace this when there is a setter.
        config.transport_config(Arc::new(transport_config));

        // Pipe the local address through to the endpoint creation
        let socket_addr = match local_addr {
            Some(ip) => SocketAddr::new(ip, 0),
            None => "[::]:0".parse::<SocketAddr>().unwrap(),
        };

        let mut endpoint = Endpoint::client(socket_addr)?;
        endpoint.set_default_client_config(config);

        Ok(Self {
            resolver,
            endpoint,
            client_config,
            local_addr,
            connect_timeout,
        })
    }

    pub async fn connect(&mut self, dest: Uri) -> Result<H3Connection, BoxError> {
        let host = dest
            .host()
            .ok_or("destination must have a host")?
            .trim_start_matches('[')
            .trim_end_matches(']');
        let port = dest.port_u16().unwrap_or(443);

        let addrs = if let Ok(addr) = IpAddr::from_str(host) {
            // If the host is already an IP address, skip resolving.
            vec![SocketAddr::new(addr, port)]
        } else {
            let addrs = resolve(&mut self.resolver, Name::from_str(host)?).await?;
            let explicit_port = dest.port().is_some();
            let addrs = addrs.map(|mut addr| {
                set_port(&mut addr, port, explicit_port);
                addr
            });
            addrs.collect()
        };

        self.remote_connect(addrs, host).await
    }

    async fn remote_connect(
        &mut self,
        addrs: Vec<SocketAddr>,
        server_name: &str,
    ) -> Result<H3Connection, BoxError> {
        if addrs.is_empty() {
            return Err(crate::error::dns("dns resolution returned no addresses"));
        }

        let (mut ipv6_addrs, mut ipv4_addrs): (Vec<SocketAddr>, Vec<SocketAddr>) =
            addrs.into_iter().partition(|addr| addr.is_ipv6());

        if let Some(local_ip) = self.local_addr {
            if local_ip.is_ipv6() {
                ipv4_addrs.clear();
            } else {
                ipv6_addrs.clear();
            }
        }

        let connect_timeout = self.connect_timeout;

        if ipv6_addrs.is_empty() {
            return Self::try_addresses_static(
                &self.endpoint,
                &ipv4_addrs,
                server_name,
                &self.client_config,
                connect_timeout,
            )
            .await;
        }
        if ipv4_addrs.is_empty() {
            return Self::try_addresses_static(
                &self.endpoint,
                &ipv6_addrs,
                server_name,
                &self.client_config,
                connect_timeout,
            )
            .await;
        }

        let endpoint = self.endpoint.clone();
        let client_config = self.client_config.clone();

        Self::try_addresses_happy_eyeballs(
            &endpoint,
            &ipv6_addrs,
            &ipv4_addrs,
            server_name,
            &client_config,
            connect_timeout,
        )
        .await
    }

    async fn try_addresses_static(
        endpoint: &Endpoint,
        addrs: &[SocketAddr],
        server_name: &str,
        client_config: &H3ClientConfig,
        connect_timeout: Option<Duration>,
    ) -> Result<H3Connection, BoxError> {
        let mut last_err: Option<BoxError> = None;

        for addr in addrs {
            match endpoint.connect(*addr, server_name) {
                Ok(connecting) => {
                    // The QUIC handshake is bounded by `connect_timeout` (like
                    // the h1/h2 TCP+TLS stages); without it a blackholed peer
                    // hangs for quinn's 10s default regardless of the user's
                    // `connect_timeout`. Errors are converted to `io::Error`
                    // (quinn maps kinds itself) so `is_connect()`/`is_timeout()`
                    // classify them like other protocols' connect failures.
                    let connected = match connect_timeout {
                        Some(timeout) => match tokio::time::timeout(timeout, connecting).await {
                            Ok(result) => {
                                result.map_err(|e| -> BoxError { Box::new(io::Error::from(e)) })
                            }
                            Err(_) => Err(Box::new(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "http3 connect timed out",
                            )) as BoxError),
                        },
                        None => connecting
                            .await
                            .map_err(|e| -> BoxError { Box::new(io::Error::from(e)) }),
                    };
                    match connected {
                        Ok(new_conn) => {
                            let quinn_conn = Connection::new(new_conn);
                            let mut h3_client_builder = h3::client::builder();
                            if let Some(max_field_section_size) =
                                client_config.max_field_section_size
                            {
                                h3_client_builder.max_field_section_size(max_field_section_size);
                            }
                            if let Some(send_grease) = client_config.send_grease {
                                h3_client_builder.send_grease(send_grease);
                            }
                            return Ok(h3_client_builder.build(quinn_conn).await?);
                        }
                        Err(e) => {
                            last_err = Some(e);
                        }
                    }
                }
                Err(e) => {
                    last_err = Some(Box::new(e) as BoxError);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| "no addresses available".into()))
    }

    async fn try_addresses_happy_eyeballs(
        endpoint: &Endpoint,
        ipv6_addrs: &[SocketAddr],
        ipv4_addrs: &[SocketAddr],
        server_name: &str,
        client_config: &H3ClientConfig,
        connect_timeout: Option<Duration>,
    ) -> Result<H3Connection, BoxError> {
        let ipv6_connect = Self::try_addresses_static(
            endpoint,
            ipv6_addrs,
            server_name,
            client_config,
            connect_timeout,
        );
        tokio::pin!(ipv6_connect);

        let delay = tokio::time::sleep(HAPPY_EYEBALLS_DELAY);
        tokio::pin!(delay);

        tokio::select! {
            result = &mut ipv6_connect => {
                return match result {
                    Ok(conn) => Ok(conn),
                    Err(_) => {
                        Self::try_addresses_static(
                            endpoint,
                            ipv4_addrs,
                            server_name,
                            client_config,
                            connect_timeout,
                        )
                        .await
                    }
                };
            }
            _ = &mut delay => {}
        }

        let ipv4_connect = Self::try_addresses_static(
            endpoint,
            ipv4_addrs,
            server_name,
            client_config,
            connect_timeout,
        );
        tokio::pin!(ipv4_connect);

        let wait_for_ipv6 = tokio::select! {
            result = &mut ipv6_connect => {
                match result {
                    Ok(conn) => return Ok(conn),
                    Err(_) => false,
                }
            }
            result = &mut ipv4_connect => {
                match result {
                    Ok(conn) => return Ok(conn),
                    Err(_) => true,
                }
            }
        };

        if wait_for_ipv6 {
            ipv6_connect.await
        } else {
            ipv4_connect.await
        }
    }
}

/// No-op cert verifier for the HTTP/3 (QUIC) transport when the user opts out
/// via `danger_accept_invalid_certs`. Mirrors `crate::tls::NoVerifier` but is
/// implemented against `quinn`'s bundled `rustls`, so the two cannot share a
/// type.
#[derive(Debug)]
pub(crate) struct QuinnNoVerifier {
    signature_algorithms: WebPkiSupportedAlgorithms,
}

impl QuinnNoVerifier {
    pub(crate) fn new(signature_algorithms: WebPkiSupportedAlgorithms) -> Self {
        Self {
            signature_algorithms,
        }
    }
}

impl ServerCertVerifier for QuinnNoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, QuinnRustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, QuinnRustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, QuinnRustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.signature_algorithms.supported_schemes()
    }
}

/// Cert verifier for the HTTP/3 (QUIC) transport when the user opts out of
/// *hostname* verification (`danger_accept_invalid_hostnames`) but keeps
/// chain verification. Mirrors `crate::tls::IgnoreHostname` against `quinn`'s
/// bundled `rustls`; validates the chain but ignores the server name.
#[derive(Debug)]
pub(crate) struct QuinnIgnoreHostname {
    roots: RootCertStore,
    signature_algorithms: WebPkiSupportedAlgorithms,
}

impl QuinnIgnoreHostname {
    pub(crate) fn new(
        roots: RootCertStore,
        signature_algorithms: WebPkiSupportedAlgorithms,
    ) -> Self {
        Self {
            roots,
            signature_algorithms,
        }
    }
}

impl ServerCertVerifier for QuinnIgnoreHostname {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, QuinnRustlsError> {
        let cert = ParsedCertificate::try_from(end_entity)?;
        verify_server_cert_signed_by_trust_anchor(
            &cert,
            &self.roots,
            intermediates,
            now,
            self.signature_algorithms.all,
        )?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, QuinnRustlsError> {
        quinn::rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.signature_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, QuinnRustlsError> {
        quinn::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.signature_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.signature_algorithms.supported_schemes()
    }
}

/// Set the destination port unless the URI port is explicit or the resolver
/// already chose one. Mirrors hyper-util's `set_port` (`DynResolver::http_resolve`).
fn set_port(addr: &mut SocketAddr, host_port: u16, explicit: bool) {
    if explicit || addr.port() == 0 {
        addr.set_port(host_port)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_port_respects_explicit_uri_ports() {
        let mut addr = SocketAddr::from(([0, 0, 0, 0], 6881));
        set_port(&mut addr, 42, true);
        assert_eq!(addr.port(), 42);
    }

    #[test]
    fn set_port_keeps_non_zero_resolved_ports() {
        let mut addr = SocketAddr::from(([0, 0, 0, 0], 6881));
        set_port(&mut addr, 443, false);
        assert_eq!(addr.port(), 6881);
    }

    #[test]
    fn set_port_uses_default_when_resolved_port_is_zero() {
        let mut addr = SocketAddr::from(([0, 0, 0, 0], 0));
        set_port(&mut addr, 443, false);
        assert_eq!(addr.port(), 443);
    }
}
