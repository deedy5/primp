use wreq::CertStore;
use tracing;

/// Loads the CA certificates from env var PRIMP_CA_BUNDLE or the WebPKI certificate store
pub fn load_ca_certs() -> Result<CertStore, anyhow::Error> {
    if let Ok(ca_cert_path) = std::env::var("PRIMP_CA_BUNDLE").or(std::env::var("CA_CERT_FILE")) {
        // Use CA certificate bundle from env var PRIMP_CA_BUNDLE
        let cert_file = std::fs::read(ca_cert_path)
            .expect("Failed to read file from env var PRIMP_CA_BUNDLE");
        tracing::debug!("Loaded CA certs from file");
        Ok(CertStore::from_pem_certs([cert_file.as_slice()])?)
    } else {
        // Use WebPKI certificate store (Mozilla's trusted root certificates)
        let certs: Vec<&[u8]> = webpki_root_certs::TLS_SERVER_ROOT_CERTS.iter().map(|cert| cert.as_ref()).collect();
        tracing::debug!("Loaded WebPKI CA certs");
        Ok(CertStore::from_der_certs(certs)?)
    }
}

#[cfg(test)]
mod load_ca_certs_tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_load_ca_certs_with_env_var() {
        // Create a temporary file with a CA certificate
        let ca_cert_path = Path::new("test_ca_cert.pem");
        let ca_cert = "-----BEGIN CERTIFICATE-----
MIIDdTCCAl2gAwIBAgIVAMIIujU9wQIBADANBgkqhkiG9w0BAQUFADBGMQswCQYD
VQQGEwJVUzETMBEGA1UECAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4g
Q29sbGVjdGlvbjEgMB4GA1UECgwXUG9zdGdyZXMgQ29uc3VsdGF0aW9uczEhMB8G
A1UECwwYUG9zdGdyZXMgQ29uc3VsdGF0aW9uczEhMB8GA1UEAwwYUG9zdGdyZXMg
Q29uc3VsdGF0aW9uczEiMCAGCSqGSIb3DQEJARYTcGVyc29uYWwtZW1haWwuY29t
-----END CERTIFICATE-----";
        fs::write(ca_cert_path, ca_cert).unwrap();

        // Set the environment variable
        env::set_var("PRIMP_CA_BUNDLE", ca_cert_path);

        // Call the function
        let result = load_ca_certs();
        assert!(result.is_ok());

        // Clean up
        fs::remove_file(ca_cert_path).unwrap();
    }

    #[test]
    fn test_load_ca_certs_without_env_var() {
        // Call the function
        let result = load_ca_certs();
        assert!(result.is_ok());
    }
}