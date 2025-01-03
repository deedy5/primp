use anyhow::{Error, Ok, Result};
use foldhash::fast::RandomState;
use indexmap::IndexMap;

use rquest::header::{HeaderMap, HeaderName, HeaderValue};

pub trait HeadersTraits {
    fn to_indexmap(&self) -> IndexMap<String, String, RandomState>;
    fn to_headermap(&self) -> HeaderMap;
    fn insert_key_value(&mut self, key: String, value: String) -> Result<(), Error>;
}

impl HeadersTraits for IndexMap<String, String, RandomState> {
    fn to_indexmap(&self) -> IndexMap<String, String, RandomState> {
        self.clone()
    }
    fn to_headermap(&self) -> HeaderMap {
        self.iter()
            .map(|(k, v)| {
                (
                    HeaderName::from_bytes(k.as_bytes())
                        .unwrap_or_else(|k| panic!("Invalid header name: {k:?}")),
                    HeaderValue::from_bytes(v.as_bytes())
                        .unwrap_or_else(|v| panic!("Invalid header value: {v:?}")),
                )
            })
            .collect()
    }
    fn insert_key_value(&mut self, key: String, value: String) -> Result<(), Error> {
        self.insert(key.to_string(), value.to_string());
        Ok(())
    }
}

impl HeadersTraits for HeaderMap {
    fn to_indexmap(&self) -> IndexMap<String, String, RandomState> {
        self.iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    v.to_str()
                        .unwrap_or_else(|v| panic!("Invalid header value: {v:?}"))
                        .to_string(),
                )
            })
            .collect()
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

impl CookiesTraits for IndexMap<String, String, RandomState> {
    fn to_string(&self) -> String {
        self.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join("; ")
    }
}
