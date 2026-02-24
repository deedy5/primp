use foldhash::fast::RandomState;
use indexmap::IndexMap;

use ::primp::header::{HeaderMap, HeaderName, HeaderValue};

use crate::error::PrimpErrorEnum;

type IndexMapSSR = IndexMap<String, String, RandomState>;

/// Try to convert a key-value pair to header name and value.
/// Returns an error if either conversion fails.
fn try_make_header(key: &str, value: &str) -> Result<(HeaderName, HeaderValue), PrimpErrorEnum> {
    match (
        HeaderName::from_bytes(key.as_bytes()),
        HeaderValue::from_bytes(value.as_bytes()),
    ) {
        (Ok(name), Ok(value)) => Ok((name, value)),
        (Err(_), Ok(_)) => Err(PrimpErrorEnum::InvalidHeaderValue(format!(
            "Invalid header name: {:?}",
            key
        ))),
        (Ok(_), Err(_)) => Err(PrimpErrorEnum::InvalidHeaderValue(format!(
            "Invalid header value for key '{}': {:?}",
            key, value
        ))),
        (Err(_), Err(_)) => Err(PrimpErrorEnum::InvalidHeaderValue(format!(
            "Invalid header (both name and value invalid): key={:?}, value={:?}",
            key, value
        ))),
    }
}

/// Trait for converting between IndexMap and HeaderMap representations.
pub trait HeadersTraits {
    /// Convert to IndexMap representation.
    fn to_indexmap(&self) -> IndexMapSSR;
    /// Convert to HeaderMap representation, returning error on invalid headers.
    fn to_headermap(&self) -> Result<HeaderMap, PrimpErrorEnum>;
}

impl HeadersTraits for IndexMapSSR {
    fn to_indexmap(&self) -> IndexMapSSR {
        self.clone()
    }

    fn to_headermap(&self) -> Result<HeaderMap, PrimpErrorEnum> {
        let mut header_map = HeaderMap::with_capacity(self.len());
        for (k, v) in self {
            let (name, value) = try_make_header(k, v)?;
            header_map.insert(name, value);
        }
        Ok(header_map)
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

    fn to_headermap(&self) -> Result<HeaderMap, PrimpErrorEnum> {
        Ok(self.clone())
    }
}

/// Extension trait for inserting headers into a HeaderMap.
pub trait HeaderMapExt {
    /// Insert a key-value pair into the header map, returning error on invalid header.
    fn insert_key_value(&mut self, key: String, value: String) -> Result<(), PrimpErrorEnum>;
}

impl HeaderMapExt for HeaderMap {
    fn insert_key_value(&mut self, key: String, value: String) -> Result<(), PrimpErrorEnum> {
        let (name, value) = try_make_header(&key, &value)?;
        self.insert(name, value);
        Ok(())
    }
}
