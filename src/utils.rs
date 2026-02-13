use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use reqwest::Certificate;

/// Thread-safe cache for CA certificates, keyed by file path
static CA_CERT_CACHE: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));
static CA_CERTS: Lazy<Mutex<Option<Vec<Certificate>>>> = Lazy::new(|| Mutex::new(None));

/// Environment variables to check for CA certificate paths (in order).
const CA_CERT_ENV_VARS: &[&str] = &[
    "PRIMP_CA_BUNDLE",
    "SSL_CERT_FILE",
    "CURL_CA_BUNDLE",
];

/// Loads CA certificates from a file path with caching.
///
/// The certificates are loaded once and cached in memory.
/// Subsequent calls with the same path return the cached certificates.
pub fn load_ca_certs_from_file(ca_cert_path: &Path) -> Option<Vec<Certificate>> {
    let mut cache_path = CA_CERT_CACHE.lock().unwrap();
    let mut cache_certs = CA_CERTS.lock().unwrap();

    let input_path_buf = ca_cert_path.to_path_buf();

    // Return cached certificates if path matches
    if cache_path.as_ref() == Some(&input_path_buf) {
        return cache_certs.clone();
    }

    // Load and cache certificates
    let cert_file = std::fs::read(ca_cert_path).ok()?;
    let certs = Certificate::from_pem_bundle(&cert_file).ok()?;

    *cache_path = Some(input_path_buf);
    *cache_certs = Some(certs.clone());

    Some(certs)
}

/// Loads CA certificates from environment variables.
fn load_ca_certs_from_env() -> Option<Vec<Certificate>> {
    for env_var in CA_CERT_ENV_VARS {
        if let Ok(ca_cert_path) = std::env::var(env_var) {
            let path = Path::new(&ca_cert_path);
            if path.exists() {
                tracing::debug!("Loading CA certs from env var: {}", env_var);
                if let Some(certs) = load_ca_certs_from_file(path) {
                    return Some(certs);
                }
            }
        }
    }
    None
}

/// Loads CA certificates based on the provided parameters.
///
/// # Arguments
/// * `ca_cert_file` - Optional path to a CA certificate file
///
/// # Returns
/// * `Some(Vec<Certificate>)` - If certificates were loaded successfully
/// * `None` - If no certificates should be loaded (use system default)
pub fn load_ca_certs(ca_cert_file: &Option<String>) -> Option<Vec<Certificate>> {
    // If ca_cert_file is provided, load from that file
    if let Some(ca_cert_path) = ca_cert_file {
        tracing::debug!("Loading CA certs from file: {}", ca_cert_path);
        return load_ca_certs_from_file(Path::new(ca_cert_path));
    }

    // Try to load from environment variables
    load_ca_certs_from_env()
}

#[cfg(test)]
mod load_ca_certs_tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_load_ca_certs_from_file() {
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

        // Call the function
        let result = load_ca_certs_from_file(ca_cert_path);

        // Check the result
        assert!(result.is_some());

        // Clean up
        fs::remove_file(ca_cert_path).unwrap();
    }

    #[test]
    fn test_load_ca_certs_with_ca_cert_file_param() {
        // Create a temporary file with a CA certificate
        let ca_cert_path = Path::new("test_ca_cert2.pem");
        let ca_cert = "-----BEGIN CERTIFICATE-----
MIIDdTCCAl2gAwIBAgIVAMIIujU9wQIBADANBgkqhkiG9w0BAQUFADBGMQswCQYD
VQQGEwJVUzETMBEGA1UECAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4g
Q29sbGVjdGlvbjEgMB4GA1UECgwXUG9zdGdyZXMgQ29uc3VsdGF0aW9uczEhMB8G
A1UECwwYUG9zdGdyZXMgQ29uc3VsdGF0aW9uczEhMB8GA1UEAwwYUG9zdGdyZXMg
Q29uc3VsdGF0aW9uczEiMCAGCSqGSIb3DQEJARYTcGVyc29uYWwtZW1haWwuY29t
-----END CERTIFICATE-----";
        fs::write(ca_cert_path, ca_cert).unwrap();

        // Call the function with Some path
        let result = load_ca_certs(&Some(ca_cert_path.to_str().unwrap().to_string()));

        // Check the result
        assert!(result.is_some());

        // Clean up
        fs::remove_file(ca_cert_path).unwrap();
    }

    #[test]
    fn test_load_ca_certs_with_none() {
        // Call the function with None
        let result = load_ca_certs(&None);

        // Check the result - should be None since no env vars are set in test
        assert!(result.is_none());
    }
}
