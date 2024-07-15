use std::cmp::min;

use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict};

// Get encoding from the "Content-Type" header
pub fn get_encoding_from_headers(headers: &IndexMap<String, String>) -> Option<String> {
    // Extract and decode the Content-Type header
    let content_type = headers
        .iter()
        .find_map(|(key, value)| {
            if key.to_lowercase() == "content-type" {
                Some(value.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();

    // Parse the Content-Type header to separate the media type and parameters
    let mut parts = content_type.split(';');
    let media_type = parts.next().unwrap_or("").trim();
    let params = parts.next().unwrap_or("").trim();

    // Check for specific conditions and return the appropriate encoding
    if let Some(param) = params.to_lowercase().strip_prefix("charset=") {
        Some(param.trim_matches('"').to_string())
    } else if media_type == "application/json" {
        Some("UTF-8".into())
    } else {
        None
    }
}

/// Get encoding from the `<meta charset="...">` tag within the first 1000 bytes of HTML content.
pub fn get_encoding_from_content(raw_bytes: &[u8]) -> Option<String> {
    let html_str = String::from_utf8_lossy(&raw_bytes[..min(1000, raw_bytes.len())]);

    let charset_index_start = &html_str.find("charset=")? + 8;
    let charset_index_end = html_str[charset_index_start..].find('"')? + charset_index_start;

    let charset_str = &html_str[charset_index_start..charset_index_end];
    let charset = charset_str.to_string();
    Some(charset)
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
        let mut headers = IndexMap::new();
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
    fn test_get_encoding_from_content_missing_charset() {
        let raw_html = b"<html><head></head></html>";
        assert_eq!(get_encoding_from_content(raw_html), None);
    }
}
