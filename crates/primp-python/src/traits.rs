use foldhash::fast::RandomState;

use ::primp::header::{HeaderMap, HeaderName, HeaderValue};

use crate::client_builder::IndexMapSSR;
use crate::error::PrimpErrorEnum;

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
        for (k, v) in self.iter() {
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
                let key_str = key.as_str().to_string();
                let is_set_cookie = key_str.eq_ignore_ascii_case("set-cookie");
                match index_map.entry(key_str) {
                    indexmap::map::Entry::Occupied(mut e) => {
                        // Set-Cookie must NOT be joined with ", " (Expires contains ", ").
                        // Use "\n" to keep values separable and RFC 6265 compliant.
                        if is_set_cookie {
                            e.get_mut().push('\n');
                            e.get_mut().push_str(v);
                        } else {
                            e.get_mut().push_str(", ");
                            e.get_mut().push_str(v);
                        }
                    }
                    indexmap::map::Entry::Vacant(e) => {
                        e.insert(v.to_string());
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::HeadersTraits;
    use ::primp::header::{HeaderMap, HeaderValue, SET_COOKIE, VARY};

    #[test]
    fn set_cookie_with_expires_comma_uses_newline_separator() {
        let mut headers = HeaderMap::new();
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("a=1; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Path=/"),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("b=2; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Path=/"),
        );
        let map = headers.to_indexmap();
        let v = map.get("set-cookie").expect("set-cookie present");
        assert!(
            v.contains('\n'),
            "Set-Cookie must be newline-joined, got {v:?}"
        );
        let parts: Vec<&str> = v.split('\n').collect();
        assert_eq!(parts.len(), 2, "must preserve 2 cookies, got {v:?}");
        for p in &parts {
            assert!(
                p.contains("Expires=Wed, 21 Oct 2015"),
                "Expires comma must survive, got {p:?}"
            );
        }
        // Non-Set-Cookie keeps RFC7230 ", " join.
        let mut other = HeaderMap::new();
        other.append(VARY, HeaderValue::from_static("Accept-Encoding"));
        other.append(VARY, HeaderValue::from_static("Origin"));
        assert_eq!(
            other.to_indexmap().get("vary").map(String::as_str),
            Some("Accept-Encoding, Origin")
        );
    }
}
