use std::cmp::min;
use std::sync::LazyLock;

use foldhash::fast::RandomState;
use indexmap::IndexMap;
use rquest::boring::{
    error::ErrorStack,
    x509::{
        store::{X509Store, X509StoreBuilder},
        X509,
    },
};

/// Loads the CA certificates from venv var PRIMP_CA_BUNDLE or the WebPKI certificate store
pub fn load_ca_certs() -> Option<&'static X509Store> {
    static CERT_STORE: LazyLock<Result<X509Store, ErrorStack>> = LazyLock::new(|| {
        let mut ca_store = X509StoreBuilder::new()?;
        if let Some(ca_cert_path) = std::env::var("PRIMP_CA_BUNDLE")
            .or(std::env::var("CA_CERT_FILE"))
            .ok()
        {
            // Use CA certificate bundle from env var PRIMP_CA_BUNDLE
            let cert_file = &std::fs::read(ca_cert_path)
                .expect("Failed to read file from env var PRIMP_CA_BUNDLE");
            let certs = X509::stack_from_pem(&cert_file)?;
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
            log::debug!("Loaded CA certs");
            Some(cert_store)
        }
        Err(err) => {
            log::error!("Failed to load CA certs: {:?}", err);
            None
        }
    }
}

/// Get encoding from the "Content-Type" header
pub fn get_encoding_from_headers(
    headers: &IndexMap<String, String, RandomState>,
) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.to_ascii_lowercase() == "content-type")
        .map(|(_, value)| value.as_str())
        .and_then(|content_type| {
            // Parse the Content-Type header to separate the media type and parameters
            let mut parts = content_type.split(';');
            let media_type = parts.next().unwrap_or("").trim();
            let params = parts.next().unwrap_or("").trim();

            // Check for specific conditions and return the appropriate encoding
            if let Some(param) = params.to_ascii_lowercase().strip_prefix("charset=") {
                Some(param.trim_matches('"').to_string())
            } else if media_type == "application/json" {
                Some("UTF-8".into())
            } else {
                None
            }
        })
}

/// Get encoding from the `<meta charset="...">` tag within the first 2048 bytes of HTML content.
pub fn get_encoding_from_content(raw_bytes: &[u8]) -> Option<String> {
    let start_sequence = b"charset=";
    let start_sequence_len = start_sequence.len();
    let end_sequence = b'>';
    let max_index = min(2048, raw_bytes.len());

    let start_index = raw_bytes[..max_index]
        .windows(start_sequence_len)
        .position(|window| window == start_sequence);

    if let Some(start_index) = start_index {
        let end_index = &raw_bytes[start_index..max_index]
            .iter()
            .position(|&byte| byte == end_sequence)?;

        let charset_slice = &raw_bytes[start_index + start_sequence_len..start_index + end_index];
        let charset = String::from_utf8_lossy(charset_slice)
            .trim_matches('"')
            .to_string();
        Some(charset)
    } else {
        None
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

#[cfg(test)]
mod utils_tests {
    use super::*;
    use indexmap::IndexMap;

    #[test]
    fn test_get_encoding_from_headers() {
        // Test case: Content-Type header with charset specified
        let mut headers = IndexMap::default();
        headers.insert(
            String::from("Content-Type"),
            String::from("text/html;charset=UTF-8"),
        );
        assert_eq!(get_encoding_from_headers(&headers), Some("utf-8".into()));

        // Test case: Content-Type header without charset specified
        headers.clear();
        headers.insert(String::from("Content-Type"), String::from("text/plain"));
        assert_eq!(get_encoding_from_headers(&headers), None);

        // Test case: Missing Content-Type header
        headers.clear();
        assert_eq!(get_encoding_from_headers(&headers), None);

        // Test case: Content-Type header with application/json
        headers.clear();
        headers.insert(
            String::from("Content-Type"),
            String::from("application/json"),
        );
        assert_eq!(get_encoding_from_headers(&headers), Some("UTF-8".into()));
    }

    #[test]
    fn test_get_encoding_from_content_present_charset() {
        let raw_html = b"<html><head><meta charset=windows1252\"></head></html>";
        assert_eq!(
            get_encoding_from_content(raw_html),
            Some("windows1252".into())
        );
    }

    #[test]
    fn test_get_encoding_from_content_present_charset2() {
        let raw_html = b"<html><head><meta charset=\"windows1251\"></head></html>";
        assert_eq!(
            get_encoding_from_content(raw_html),
            Some("windows1251".into())
        );
    }

    #[test]
    fn test_get_encoding_from_content_missing_charset() {
        let raw_html = b"<html><head></head></html>";
        assert_eq!(get_encoding_from_content(raw_html), None);
    }
}
