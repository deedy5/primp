use foldhash::fast::RandomState;
use indexmap::IndexMap;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

type IndexMapSSR = IndexMap<String, String, RandomState>;

/// Try to convert a key-value pair to header name and value.
/// Returns None if either conversion fails, logging a warning.
fn try_make_header(key: &str, value: &str) -> Option<(HeaderName, HeaderValue)> {
    match (HeaderName::from_bytes(key.as_bytes()), HeaderValue::from_bytes(value.as_bytes())) {
        (Ok(name), Ok(value)) => Some((name, value)),
        (Err(_), Ok(_)) => {
            tracing::warn!("Skipping invalid header name: {:?}", key);
            None
        }
        (Ok(_), Err(_)) => {
            tracing::warn!("Skipping invalid header value for key '{}': {:?}", key, value);
            None
        }
        (Err(_), Err(_)) => {
            tracing::warn!("Skipping invalid header (both name and value invalid): key={:?}, value={:?}", key, value);
            None
        }
    }
}

/// Trait for converting between IndexMap and HeaderMap representations.
pub trait HeadersTraits {
    /// Convert to IndexMap representation.
    fn to_indexmap(&self) -> IndexMapSSR;
    /// Convert to HeaderMap representation.
    fn to_headermap(&self) -> HeaderMap;
}

impl HeadersTraits for IndexMapSSR {
    fn to_indexmap(&self) -> IndexMapSSR {
        self.clone()
    }

    fn to_headermap(&self) -> HeaderMap {
        let mut header_map = HeaderMap::with_capacity(self.len());
        for (k, v) in self {
            if let Some((name, value)) = try_make_header(k, v) {
                header_map.insert(name, value);
            }
        }
        header_map
    }
}

impl HeadersTraits for HeaderMap {
    fn to_indexmap(&self) -> IndexMapSSR {
        let mut index_map =
            IndexMapSSR::with_capacity_and_hasher(self.len(), RandomState::default());
        for (key, value) in self {
            if let Ok(v) = value.to_str() {
                index_map.insert(key.as_str().to_string(), v.to_string());
            }
        }
        index_map
    }

    fn to_headermap(&self) -> HeaderMap {
        self.clone()
    }
}

/// Extension trait for inserting headers into a HeaderMap.
pub trait HeaderMapExt {
    /// Insert a key-value pair into the header map.
    fn insert_key_value(&mut self, key: String, value: String);
}

impl HeaderMapExt for HeaderMap {
    fn insert_key_value(&mut self, key: String, value: String) {
        if let Some((name, value)) = try_make_header(&key, &value) {
            self.insert(name, value);
        }
    }
}
