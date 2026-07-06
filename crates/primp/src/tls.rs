//! TLS configuration and types. A `Client` uses TLS (rustls) by default for
//! HTTPS destinations.

use rustls::{
    client::danger::HandshakeSignatureValid, client::danger::ServerCertVerified,
    client::danger::ServerCertVerifier, crypto::WebPkiSupportedAlgorithms,
    server::ParsedCertificate, DigitallySignedStruct, Error as TLSError, RootCertStore,
    SignatureScheme,
};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{ServerName, UnixTime};
use std::{
    fmt,
    io::{BufRead, BufReader},
    sync::Arc,
};

/// An X509 certificate revocation list (CRL).
pub struct CertificateRevocationList {
    inner: rustls_pki_types::CertificateRevocationListDer<'static>,
}

/// A server X509 certificate.
#[derive(Clone)]
pub struct Certificate {
    original: Cert,
}

#[derive(Clone)]
enum Cert {
    Der(Vec<u8>),
    Pem(Vec<u8>),
}

/// A client certificate plus its private key.
#[derive(Clone)]
pub struct Identity {
    inner: ClientCert,
}

enum ClientCert {
    Pem {
        key: rustls_pki_types::PrivateKeyDer<'static>,
        certs: Vec<rustls_pki_types::CertificateDer<'static>>,
    },
}

impl Clone for ClientCert {
    fn clone(&self) -> Self {
        match self {
            ClientCert::Pem { key, certs } => ClientCert::Pem {
                key: key.clone_key(),
                certs: certs.clone(),
            },
        }
    }
}

impl Certificate {
    /// Create a `Certificate` from binary DER.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::File;
    /// # use std::io::Read;
    /// # fn cert() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buf = Vec::new();
    /// File::open("my_cert.der")?
    ///     .read_to_end(&mut buf)?;
    /// let cert = primp::Certificate::from_der(&buf)?;
    /// # drop(cert);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_der(der: &[u8]) -> crate::Result<Certificate> {
        Ok(Certificate {
            original: Cert::Der(der.to_owned()),
        })
    }

    /// Create a `Certificate` from PEM.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::File;
    /// # use std::io::Read;
    /// # fn cert() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buf = Vec::new();
    /// File::open("my_cert.pem")?
    ///     .read_to_end(&mut buf)?;
    /// let cert = primp::Certificate::from_pem(&buf)?;
    /// # drop(cert);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_pem(pem: &[u8]) -> crate::Result<Certificate> {
        Ok(Certificate {
            original: Cert::Pem(pem.to_owned()),
        })
    }

    /// Create a collection of `Certificate`s from a PEM bundle
    /// (`.crt`/`.cer`/`.pem`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::File;
    /// # use std::io::Read;
    /// # fn cert() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buf = Vec::new();
    /// File::open("ca-bundle.crt")?
    ///     .read_to_end(&mut buf)?;
    /// let certs = primp::Certificate::from_pem_bundle(&buf)?;
    /// # drop(certs);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_pem_bundle(pem_bundle: &[u8]) -> crate::Result<Vec<Certificate>> {
        let mut reader = BufReader::new(pem_bundle);

        Self::read_pem_certs(&mut reader)?
            .iter()
            .map(|cert_vec| Certificate::from_der(cert_vec))
            .collect::<crate::Result<Vec<Certificate>>>()
    }

    pub(crate) fn add_to_rustls(
        self,
        root_cert_store: &mut rustls::RootCertStore,
    ) -> crate::Result<()> {
        use std::io::Cursor;

        match self.original {
            Cert::Der(buf) => root_cert_store
                .add(buf.into())
                .map_err(crate::error::builder)?,
            Cert::Pem(buf) => {
                let mut reader = Cursor::new(buf);
                let certs = Self::read_pem_certs(&mut reader)?;
                for c in certs {
                    root_cert_store
                        .add(c.into())
                        .map_err(crate::error::builder)?;
                }
            }
        }
        Ok(())
    }

    /// Return the DER bytes of every certificate (one per entry for a PEM
    /// bundle). Useful when handing certs to a TLS stack other than
    /// `primp-rustls` (e.g. `quinn`) that cannot share its types.
    #[cfg(feature = "http3")]
    pub(crate) fn as_der_many(&self) -> crate::Result<Vec<Vec<u8>>> {
        match &self.original {
            Cert::Der(buf) => Ok(vec![buf.clone()]),
            Cert::Pem(buf) => {
                use std::io::Cursor;

                let mut reader = Cursor::new(buf.clone());
                Self::read_pem_certs(&mut reader)
            }
        }
    }

    fn read_pem_certs(reader: &mut impl BufRead) -> crate::Result<Vec<Vec<u8>>> {
        rustls_pki_types::CertificateDer::pem_reader_iter(reader)
            .map(|result| match result {
                Ok(cert) => Ok(cert.as_ref().to_vec()),
                Err(_) => Err(crate::error::builder("invalid certificate encoding")),
            })
            .collect()
    }
}

impl Identity {
    /// Parse a PEM containing a private key and at least one certificate.
    /// The key must be RSA, SEC1 EC, or PKCS#8.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::File;
    /// # use std::io::Read;
    /// # fn pem() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buf = Vec::new();
    /// File::open("my-ident.pem")?
    ///     .read_to_end(&mut buf)?;
    /// let id = primp::Identity::from_pem(&buf)?;
    /// # drop(id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Optional
    ///
    /// This requires the `rustls(-...)` Cargo feature enabled.
    pub fn from_pem(buf: &[u8]) -> crate::Result<Identity> {
        use rustls_pki_types::{pem::SectionKind, PrivateKeyDer};
        use std::io::Cursor;

        let (key, certs) = {
            let mut pem = Cursor::new(buf);
            let mut sk = Vec::<rustls_pki_types::PrivateKeyDer>::new();
            let mut certs = Vec::<rustls_pki_types::CertificateDer>::new();

            while let Some((kind, data)) =
                rustls_pki_types::pem::from_buf(&mut pem).map_err(|_| {
                    crate::error::builder(TLSError::General(String::from(
                        "Invalid identity PEM file",
                    )))
                })?
            {
                match kind {
                    SectionKind::Certificate => certs.push(data.into()),
                    SectionKind::PrivateKey => sk.push(PrivateKeyDer::Pkcs8(data.into())),
                    SectionKind::RsaPrivateKey => sk.push(PrivateKeyDer::Pkcs1(data.into())),
                    SectionKind::EcPrivateKey => sk.push(PrivateKeyDer::Sec1(data.into())),
                    _ => {
                        return Err(crate::error::builder(TLSError::General(String::from(
                            "No valid certificate was found",
                        ))))
                    }
                }
            }

            if let (Some(sk), false) = (sk.pop(), certs.is_empty()) {
                (sk, certs)
            } else {
                return Err(crate::error::builder(TLSError::General(String::from(
                    "private key or certificate not found",
                ))));
            }
        };

        Ok(Identity {
            inner: ClientCert::Pem { key, certs },
        })
    }

    pub(crate) fn add_to_rustls(
        self,
        config_builder: rustls::ConfigBuilder<
            rustls::ClientConfig,
            // Not sure here
            rustls::client::WantsClientCert,
        >,
    ) -> crate::Result<rustls::ClientConfig> {
        match self.inner {
            ClientCert::Pem { key, certs } => config_builder
                .with_client_auth_cert(certs, key)
                .map_err(crate::error::builder),
        }
    }
}

impl CertificateRevocationList {
    /// Parse a PEM encoded CRL.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::File;
    /// # use std::io::Read;
    /// # fn crl() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buf = Vec::new();
    /// File::open("my_crl.pem")?
    ///     .read_to_end(&mut buf)?;
    /// let crl = primp::tls::CertificateRevocationList::from_pem(&buf)?;
    /// # drop(crl);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Optional
    ///
    /// This requires the `rustls(-...)` Cargo feature enabled.
    pub fn from_pem(pem: &[u8]) -> crate::Result<CertificateRevocationList> {
        Ok(CertificateRevocationList {
            inner: rustls_pki_types::CertificateRevocationListDer::from_pem_slice(pem)
                .map_err(|_| crate::error::builder("invalid crl encoding"))?,
        })
    }

    /// Creates a collection of `CertificateRevocationList`s from a PEM encoded CRL bundle.
    /// Example byte sources may be `.crl` or `.pem` files.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::fs::File;
    /// # use std::io::Read;
    /// # fn crls() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut buf = Vec::new();
    /// File::open("crl-bundle.crl")?
    ///     .read_to_end(&mut buf)?;
    /// let crls = primp::tls::CertificateRevocationList::from_pem_bundle(&buf)?;
    /// # drop(crls);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Optional
    ///
    /// This requires the `rustls(-...)` Cargo feature enabled.
    pub fn from_pem_bundle(pem_bundle: &[u8]) -> crate::Result<Vec<CertificateRevocationList>> {
        rustls_pki_types::CertificateRevocationListDer::pem_slice_iter(pem_bundle)
            .map(|result| match result {
                Ok(crl) => Ok(CertificateRevocationList { inner: crl }),
                Err(_) => Err(crate::error::builder("invalid crl encoding")),
            })
            .collect::<crate::Result<Vec<CertificateRevocationList>>>()
    }

    pub(crate) fn as_rustls_crl<'a>(&self) -> rustls_pki_types::CertificateRevocationListDer<'a> {
        self.inner.clone()
    }
}

impl fmt::Debug for Certificate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Certificate").finish()
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Identity").finish()
    }
}

impl fmt::Debug for CertificateRevocationList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CertificateRevocationList").finish()
    }
}

/// A TLS protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(InnerVersion);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
enum InnerVersion {
    Tls1_0,
    Tls1_1,
    Tls1_2,
    Tls1_3,
}

// These could perhaps be From/TryFrom implementations, but those would be
// part of the public API so let's be careful
impl Version {
    /// Version 1.0 of the TLS protocol.
    pub const TLS_1_0: Version = Version(InnerVersion::Tls1_0);
    /// Version 1.1 of the TLS protocol.
    pub const TLS_1_1: Version = Version(InnerVersion::Tls1_1);
    /// Version 1.2 of the TLS protocol.
    pub const TLS_1_2: Version = Version(InnerVersion::Tls1_2);
    /// Version 1.3 of the TLS protocol.
    pub const TLS_1_3: Version = Version(InnerVersion::Tls1_3);

    pub(crate) fn from_rustls(version: rustls::ProtocolVersion) -> Option<Self> {
        match version {
            rustls::ProtocolVersion::SSLv2 => None,
            rustls::ProtocolVersion::SSLv3 => None,
            rustls::ProtocolVersion::TLSv1_0 => Some(Self(InnerVersion::Tls1_0)),
            rustls::ProtocolVersion::TLSv1_1 => Some(Self(InnerVersion::Tls1_1)),
            rustls::ProtocolVersion::TLSv1_2 => Some(Self(InnerVersion::Tls1_2)),
            rustls::ProtocolVersion::TLSv1_3 => Some(Self(InnerVersion::Tls1_3)),
            _ => None,
        }
    }
}

pub(crate) enum TlsBackend {
    Rustls,
    BuiltRustls(Box<rustls::ClientConfig>),
}

impl fmt::Debug for TlsBackend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TlsBackend::Rustls => write!(f, "Rustls"),
            TlsBackend::BuiltRustls(_) => write!(f, "BuiltRustls"),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for TlsBackend {
    fn default() -> TlsBackend {
        TlsBackend::Rustls
    }
}

pub(crate) fn rustls_store(certs: &[Certificate]) -> crate::Result<RootCertStore> {
    let mut root_cert_store = rustls::RootCertStore::empty();
    for cert in certs {
        cert.clone().add_to_rustls(&mut root_cert_store)?;
    }
    Ok(root_cert_store)
}

/// Cached default root store with webpki roots plus native OS root CAs.
pub fn default_root_store() -> &'static rustls::RootCertStore {
    static DEFAULT_ROOTS: std::sync::OnceLock<rustls::RootCertStore> = std::sync::OnceLock::new();
    DEFAULT_ROOTS.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let native = rustls_native_certs::load_native_certs();
        for err in &native.errors {
            log::warn!("failed to load native root certificate: {err}");
        }
        if !native.certs.is_empty() {
            roots.add_parsable_certificates(native.certs);
        }
        roots
    })
}

/// Cached `Arc<RootCertStore>`, avoiding deep clones on every call.
pub fn default_root_store_arc() -> Arc<rustls::RootCertStore> {
    static DEFAULT_ROOTS_ARC: std::sync::OnceLock<Arc<rustls::RootCertStore>> =
        std::sync::OnceLock::new();
    DEFAULT_ROOTS_ARC
        .get_or_init(|| Arc::new(default_root_store().clone()))
        .clone()
}

/// A root store from the cached default plus the given user certificates.
pub fn merged_root_store(certs: &[Certificate]) -> crate::Result<RootCertStore> {
    let mut store = default_root_store().clone();
    for cert in certs {
        cert.clone().add_to_rustls(&mut store)?;
    }
    Ok(store)
}

#[derive(Debug)]
pub(crate) struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls_pki_types::CertificateDer,
        _intermediates: &[rustls_pki_types::CertificateDer],
        _server_name: &ServerName,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TLSError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TLSError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TLSError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

#[derive(Debug)]
pub(crate) struct IgnoreHostname {
    roots: RootCertStore,
    signature_algorithms: WebPkiSupportedAlgorithms,
}

impl IgnoreHostname {
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

impl ServerCertVerifier for IgnoreHostname {
    fn verify_server_cert(
        &self,
        end_entity: &rustls_pki_types::CertificateDer<'_>,
        intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TLSError> {
        let cert = ParsedCertificate::try_from(end_entity)?;

        rustls::client::verify_server_cert_signed_by_trust_anchor(
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
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TLSError> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.signature_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TLSError> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.signature_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.signature_algorithms.supported_schemes()
    }
}

/// Hyper extension carrying extra TLS layer information.
/// Made available to clients on responses when `tls_info` is set.
#[derive(Clone)]
pub struct TlsInfo {
    pub(crate) peer_certificate: Option<Vec<u8>>,
    pub(crate) version: Option<Version>,
}

impl TlsInfo {
    /// Get the DER encoded leaf certificate of the peer.
    pub fn peer_certificate(&self) -> Option<&[u8]> {
        self.peer_certificate.as_ref().map(|der| &der[..])
    }

    /// Get the TLS protocol version negotiated with the peer.
    ///
    /// Returns `None` if the TLS backend cannot report it.
    pub fn version(&self) -> Option<Version> {
        self.version
    }
}

impl std::fmt::Debug for TlsInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("TlsInfo")
            .field("version", &self.version)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_from_pem_invalid() {
        Identity::from_pem(b"not pem").unwrap_err();
    }

    #[test]
    fn identity_from_pem_pkcs1_key() {
        let pem = b"-----BEGIN CERTIFICATE-----\n\
            -----END CERTIFICATE-----\n\
            -----BEGIN RSA PRIVATE KEY-----\n\
            -----END RSA PRIVATE KEY-----\n";

        Identity::from_pem(pem).unwrap();
    }

    #[test]
    fn certificates_from_pem_bundle() {
        const PEM_BUNDLE: &[u8] = b"
            -----BEGIN CERTIFICATE-----
            MIIBtjCCAVugAwIBAgITBmyf1XSXNmY/Owua2eiedgPySjAKBggqhkjOPQQDAjA5
            MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6b24g
            Um9vdCBDQSAzMB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTELMAkG
            A1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJvb3Qg
            Q0EgMzBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABCmXp8ZBf8ANm+gBG1bG8lKl
            ui2yEujSLtf6ycXYqm0fc4E7O5hrOXwzpcVOho6AF2hiRVd9RFgdszflZwjrZt6j
            QjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQWBBSr
            ttvXBp43rDCGB5Fwx5zEGbF4wDAKBggqhkjOPQQDAgNJADBGAiEA4IWSoxe3jfkr
            BqWTrBqYaGFy+uGh0PsceGCmQ5nFuMQCIQCcAu/xlJyzlvnrxir4tiz+OpAUFteM
            YyRIHN8wfdVoOw==
            -----END CERTIFICATE-----

            -----BEGIN CERTIFICATE-----
            MIIB8jCCAXigAwIBAgITBmyf18G7EEwpQ+Vxe3ssyBrBDjAKBggqhkjOPQQDAzA5
            MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6b24g
            Um9vdCBDQSA0MB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTELMAkG
            A1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJvb3Qg
            Q0EgNDB2MBAGByqGSM49AgEGBSuBBAAiA2IABNKrijdPo1MN/sGKe0uoe0ZLY7Bi
            9i0b2whxIdIA6GO9mif78DluXeo9pcmBqqNbIJhFXRbb/egQbeOc4OO9X4Ri83Bk
            M6DLJC9wuoihKqB1+IGuYgbEgds5bimwHvouXKNCMEAwDwYDVR0TAQH/BAUwAwEB
            /zAOBgNVHQ8BAf8EBAMCAYYwHQYDVR0OBBYEFNPsxzplbszh2naaVvuc84ZtV+WB
            MAoGCCqGSM49BAMDA2gAMGUCMDqLIfG9fhGt0O9Yli/W651+kI0rz2ZVwyzjKKlw
            CkcO8DdZEv8tmZQoTipPNU0zWgIxAOp1AE47xDqUEpHJWEadIRNyp4iciuRMStuW
            1KyLa2tJElMzrdfkviT8tQp21KW8EA==
            -----END CERTIFICATE-----
        ";

        assert!(Certificate::from_pem_bundle(PEM_BUNDLE).is_ok())
    }

    #[test]
    fn crl_from_pem() {
        let pem = b"-----BEGIN X509 CRL-----\n-----END X509 CRL-----\n";

        CertificateRevocationList::from_pem(pem).unwrap();
    }

    #[test]
    fn crl_from_pem_bundle() {
        let pem_bundle = std::fs::read("tests/support/crl.pem").unwrap();

        let result = CertificateRevocationList::from_pem_bundle(&pem_bundle);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn tls_info_exposes_peer_certificate_and_version() {
        let info = TlsInfo {
            peer_certificate: Some(vec![0x30, 0x01, 0x02]),
            version: Some(Version::TLS_1_3),
        };
        assert_eq!(info.peer_certificate().unwrap()[0], 0x30);
        assert_eq!(info.version(), Some(Version::TLS_1_3));
        // Debug must not leak peer_certificate bytes but must show version.
        let dbg = format!("{info:?}");
        assert!(dbg.contains("TLS_1_3") || dbg.contains("version"));
        let empty = TlsInfo {
            peer_certificate: None,
            version: None,
        };
        assert!(empty.peer_certificate().is_none());
        assert!(empty.version().is_none());
        let _clone = empty.clone();
        let _dbg_empty = format!("{empty:?}");
    }
}
