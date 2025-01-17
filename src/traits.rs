use anyhow::{Error, Ok, Result};
use foldhash::fast::RandomState;
use indexmap::IndexMap;

use rquest::header::{HeaderMap, HeaderName, HeaderValue};

type IndexMapSSR = IndexMap<String, String, RandomState>;

pub trait HeadersTraits {
    fn to_indexmap(&self) -> IndexMapSSR;
    fn to_headermap(&self) -> HeaderMap;
    fn insert_key_value(&mut self, key: String, value: String) -> Result<(), Error>;
}

impl HeadersTraits for IndexMapSSR {
    fn to_indexmap(&self) -> IndexMapSSR {
        self.clone()
    }
    fn to_headermap(&self) -> HeaderMap {
        let mut header_map = HeaderMap::with_capacity(self.len());
        for (k, v) in self {
            header_map.insert(
                HeaderName::from_bytes(k.as_bytes())
                    .unwrap_or_else(|k| panic!("Invalid header name: {k:?}")),
                HeaderValue::from_bytes(v.as_bytes())
                    .unwrap_or_else(|v| panic!("Invalid header value: {v:?}")),
            );
        }
        header_map
    }

    fn insert_key_value(&mut self, key: String, value: String) -> Result<(), Error> {
        self.insert(key.to_string(), value.to_string());
        Ok(())
    }
}

impl HeadersTraits for HeaderMap {
    fn to_indexmap(&self) -> IndexMapSSR {
        let mut index_map =
            IndexMapSSR::with_capacity_and_hasher(self.len(), RandomState::default());
        for (key, value) in self {
            index_map.insert(
                key.as_str().to_string(),
                value
                    .to_str()
                    .unwrap_or_else(|v| panic!("Invalid header value: {v:?}"))
                    .to_string(),
            );
        }
        index_map
    }

    fn to_headermap(&self) -> HeaderMap {
        self.clone()
    }

    fn insert_key_value(&mut self, key: String, value: String) -> Result<(), Error> {
        let header_name = HeaderName::from_bytes(key.as_bytes())
            .unwrap_or_else(|k| panic!("Invalid header name: {k:?}"));
        let header_value = HeaderValue::from_bytes(value.as_bytes())
            .unwrap_or_else(|k| panic!("Invalid header value: {k:?}"));
        self.insert(header_name, header_value);
        Ok(())
    }
}

pub trait CookiesTraits {
    fn to_string(&self) -> String;
}

impl CookiesTraits for IndexMapSSR {
    fn to_string(&self) -> String {
        let mut result = String::with_capacity(self.len() * 40);
        for (k, v) in self {
            if !result.is_empty() {
                result.push_str("; ");
            }
            result.push_str(k);
            result.push('=');
            result.push_str(v);
        }
        result
    }
}
