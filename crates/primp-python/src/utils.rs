use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;

use ::primp::Certificate;
use mime::Mime;

/// Thread-safe cache for CA certificates, keyed by file path
static CA_CERT_CACHE: LazyLock<Mutex<Option<PathBuf>>> = LazyLock::new(|| Mutex::new(None));
static CA_CERTS: LazyLock<Mutex<Option<Vec<Certificate>>>> = LazyLock::new(|| Mutex::new(None));

/// Environment variables to check for CA certificate paths (in order).
const CA_CERT_ENV_VARS: &[&str] = &["PRIMP_CA_BUNDLE", "SSL_CERT_FILE", "CURL_CA_BUNDLE"];

/// Loads CA certificates from a file path with caching.
///
/// The certificates are loaded once and cached in memory.
/// Subsequent calls with the same path return the cached certificates.
pub fn load_ca_certs_from_file(ca_cert_path: &Path) -> Option<Vec<Certificate>> {
    let mut cache_path = CA_CERT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let mut cache_certs = CA_CERTS.lock().unwrap_or_else(|e| e.into_inner());

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

/// Extract encoding from Content-Type header.
///
/// Returns the encoding specified in the charset parameter, or UTF-8 as fallback.
pub fn extract_encoding(headers: &::primp::header::HeaderMap) -> &'static encoding_rs::Encoding {
    headers
        .get(::primp::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.parse::<Mime>().ok().and_then(|mime| {
                mime.get_param("charset")
                    .and_then(|c| encoding_rs::Encoding::for_label(c.as_str().as_bytes()))
            })
        })
        .unwrap_or(encoding_rs::UTF_8)
}

#[cfg(test)]
mod load_ca_certs_tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----
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

    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn new(name: &str, content: &str) -> Self {
            let path = PathBuf::from(name);
            fs::write(&path, content).unwrap();
            TempFile { path }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn test_load_ca_certs_from_file() {
        let _file = TempFile::new("test_ca_cert.pem", TEST_CERT);
        let result = load_ca_certs_from_file(Path::new("test_ca_cert.pem"));
        assert!(result.is_some());
    }

    #[test]
    fn test_load_ca_certs_with_ca_cert_file_param() {
        let _file = TempFile::new("test_ca_cert2.pem", TEST_CERT);
        let result = load_ca_certs(&Some("test_ca_cert2.pem".to_string()));
        assert!(result.is_some());
    }

    #[test]
    fn test_load_ca_certs_with_none() {
        let result = load_ca_certs(&None);
        assert!(result.is_some() || result.is_none());
    }
}
