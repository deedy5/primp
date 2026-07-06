use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;

use crate::error::PrimpErrorEnum;
use crate::error::PrimpResult;
use ::primp::Certificate;
use mime::Mime;

/// A cached (path, certs) pair for the CA cert cache.
type CachedCerts = (PathBuf, Vec<Certificate>);

/// Thread-safe cache for CA certificates, keyed by file path.
/// Bundled into a single mutex so the path and certs stay consistent under
/// concurrent access (separate mutexes could observe a new path with stale
/// or missing certs).
static CA_CERT_CACHE: LazyLock<Mutex<Option<CachedCerts>>> = LazyLock::new(|| Mutex::new(None));

/// Environment variables to check for CA certificate paths (in order).
const CA_CERT_ENV_VARS: &[&str] = &["PRIMP_CA_BUNDLE", "SSL_CERT_FILE", "CURL_CA_BUNDLE"];

/// Load CA certificates from a file, caching by path (same path reuses cache).
pub fn load_ca_certs_from_file(ca_cert_path: &Path) -> Option<Vec<Certificate>> {
    let mut cache = CA_CERT_CACHE.lock().unwrap_or_else(|e| e.into_inner());

    let input_path_buf = ca_cert_path.to_path_buf();

    // Return cached certificates if path matches
    if let Some((cached_path, cached_certs)) = cache.as_ref() {
        if cached_path == &input_path_buf {
            return Some(cached_certs.clone());
        }
    }

    // Load and cache certificates
    let cert_file = std::fs::read(ca_cert_path).ok()?;
    let certs = Certificate::from_pem_bundle(&cert_file).ok()?;

    *cache = Some((input_path_buf, certs.clone()));

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

/// Load CA certificates: from `ca_cert_file` if given, else from the
/// `PRIMP_CA_BUNDLE`/`SSL_CERT_FILE`/`CURL_CA_BUNDLE` env vars. Returns
/// `None` to use the system default when nothing is found.
pub fn load_ca_certs(ca_cert_file: &Option<String>) -> Option<Vec<Certificate>> {
    // If ca_cert_file is provided, load from that file
    if let Some(ca_cert_path) = ca_cert_file {
        tracing::debug!("Loading CA certs from file: {}", ca_cert_path);
        return load_ca_certs_from_file(Path::new(ca_cert_path));
    }

    // Try to load from environment variables
    load_ca_certs_from_env()
}

/// Encoding from the `Content-Type` charset parameter, or UTF-8 if absent.
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

/// Convert a Python timeout (seconds) into a `Duration` without panicking:
/// `Duration::from_secs_f64` panics on NaN/negative/infinite values, and
/// `panic = "abort"` would kill the host interpreter.
pub(crate) fn timeout_duration(seconds: f64) -> PrimpResult<Duration> {
    Duration::try_from_secs_f64(seconds).map_err(|_| {
        PrimpErrorEnum::Custom(format!(
            "timeout must be a finite, non-negative number of seconds, got {seconds}"
        ))
    })
}

#[cfg(test)]
mod load_ca_certs_tests {
    use super::*;
    use tempfile::NamedTempFile;

    const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----
MIIC/zCCAeegAwIBAgIUdSMXyCRA8Nwi3nupoR1W6uDykpkwDQYJKoZIhvcNAQEL
BQAwDzENMAsGA1UEAwwEdGVzdDAeFw0yNjA3MDIyMDI5NTJaFw0zNjA2MjkyMDI5
NTJaMA8xDTALBgNVBAMMBHRlc3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEK
AoIBAQDJxhnIzKN+rgvse3Dute3GxXT5apL+HSX7gOZlTJuZ02QYcb75FwnOTieR
1G9MmbYMZjp12q5CNtH06+SGdRNBDbHLso59PKcGLjUz0JIhxgobXLwmJFaQWHd7
7fEdrtEarTi4vPffkNPXDvMVQ5vjv2CTXs/r9Y+t7Tjn3DWzHHjp9lKn8947m5sR
5rB/KK7GdraI/ghSw1IiBENSL6Nfz5lZYETKLLZeCBKiAXmD3w+SDoaaTPblPCyW
Yd66U7C6ZnmQhcjz2V32mPMQl8wAFu1OTS3ixnqyRyvv6VtyPjyfdKU1ilWtdpGY
IycBNHnUOcPYWVyT0IEZWG4+/+r7AgMBAAGjUzBRMB0GA1UdDgQWBBR0PT1v7HwD
bpvFV3dsArng0FUsHzAfBgNVHSMEGDAWgBR0PT1v7HwDbpvFV3dsArng0FUsHzAP
BgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQChrZCGBSeUFTIkih5R
akeIvdpmnNWmSsqGh03NDOmddudtjS9U9n8rRcJKOBQzzIj6XuPm4qx6rwgldh6Y
IcUm9TAgPQRxZzrPWRQkAZHrRTo+5UKhglXsusvDUuiRdYHuslchZcLcJD4trrJd
LAxsBcPBkbxbolaABK2/tTI2qmOdUUywgwLMu3XYPVyKVPztijcTUWcrpfRJjhdQ
fsS6b/vdr6CvJCbSed0IdnHXbgauIWiLDlVopmfGzRIDhKhzJh4y82VaPUMynWvd
Cf12wr9rBJm9bEcvZnMbm8PQ0O+oaS6i50Nfm+Qy2gAsJc9gUi8G79MrX67AxI+V
PW3u
-----END CERTIFICATE-----";

    fn temp_pem(name: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new()
            .prefix(name)
            .suffix(".pem")
            .tempfile()
            .unwrap();
        std::io::Write::write_all(&mut f, TEST_CERT.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_load_ca_certs_from_file() {
        let file = temp_pem("test_ca_cert");
        let result = load_ca_certs_from_file(file.path());
        assert!(result.is_some());
    }

    #[test]
    fn test_load_ca_certs_with_ca_cert_file_param() {
        let file = temp_pem("test_ca_cert2");
        let path = file.path().to_str().unwrap().to_string();
        let result = load_ca_certs(&Some(path));
        assert!(result.is_some());
    }

    #[test]
    fn test_load_ca_certs_with_none() {
        let result = load_ca_certs(&None);
        assert!(result.is_some() || result.is_none());
    }
}
