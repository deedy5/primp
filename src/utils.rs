use std::sync::LazyLock;

use rquest::{X509Store, X509StoreBuilder, X509};
use tracing;

/// Loads the CA certificates from venv var PRIMP_CA_BUNDLE or the WebPKI certificate store
pub fn load_ca_certs() -> Option<&'static X509Store> {
    static CERT_STORE: LazyLock<Result<X509Store, anyhow::Error>> = LazyLock::new(|| {
        let mut ca_store = X509StoreBuilder::new()?;
        if let Ok(ca_cert_path) = std::env::var("PRIMP_CA_BUNDLE").or(std::env::var("CA_CERT_FILE"))
        {
            // Use CA certificate bundle from env var PRIMP_CA_BUNDLE
            let cert_file = &std::fs::read(ca_cert_path)
                .expect("Failed to read file from env var PRIMP_CA_BUNDLE");
            let certs = X509::stack_from_pem(cert_file)?;
            for cert in certs {
                ca_store.add_cert(cert)?;
            }
        } else {
            // Use WebPKI certificate store (Mozilla's trusted root certificates)
            for cert in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
                let x509 = X509::from_der(cert)?;
                ca_store.add_cert(x509)?;
            }
        }
        Ok(ca_store.build())
    });

    match CERT_STORE.as_ref() {
        Ok(cert_store) => {
            tracing::debug!("Loaded CA certs");
            Some(cert_store)
        }
        Err(err) => {
            tracing::error!("Failed to load CA certs: {:?}", err);
            None
        }
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

        // Check the result
        assert!(result.is_some());

        // Clean up
        fs::remove_file(ca_cert_path).unwrap();
    }

    #[test]
    fn test_load_ca_certs_without_env_var() {
        // Call the function
        let result = load_ca_certs();

        // Check the result
        assert!(result.is_some());
    }
}
