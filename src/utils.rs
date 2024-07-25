use std::cmp::min;

use ahash::RandomState;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict};

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

/// python json.dumps
pub fn json_dumps(py: Python, pydict: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let json_module = PyModule::import_bound(py, "json")?;
    let dumps = json_module.getattr("dumps")?;
    match pydict {
        Some(dict) => dumps.call1((dict,))?.extract::<String>(),
        None => Ok("".to_string()),
    }
}

/// python urllib.parse.urlencode
pub fn url_encode(py: Python, pydict: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let urllib_parse = PyModule::import_bound(py, "urllib.parse")?;
    let urlencode = urllib_parse.getattr("urlencode")?;
    match pydict {
        Some(dict) => urlencode
            .call1((dict, ("doseq", py.get_type_bound::<PyBool>().call1(())?)))?
            .extract::<String>(),
        None => Ok("".to_string()),
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
