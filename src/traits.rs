use anyhow::{anyhow, Error, Ok, Result};
use foldhash::fast::RandomState;
use indexmap::IndexMap;

use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use rquest::{Impersonate, ImpersonateOS};

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

pub trait ImpersonateFromStr {
    fn from_str(s: &str) -> Result<Impersonate>;
}

impl ImpersonateFromStr for Impersonate {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "chrome_100" => Ok(Impersonate::Chrome100),
            "chrome_101" => Ok(Impersonate::Chrome101),
            "chrome_104" => Ok(Impersonate::Chrome104),
            "chrome_105" => Ok(Impersonate::Chrome105),
            "chrome_106" => Ok(Impersonate::Chrome106),
            "chrome_107" => Ok(Impersonate::Chrome107),
            "chrome_108" => Ok(Impersonate::Chrome108),
            "chrome_109" => Ok(Impersonate::Chrome109),
            "chrome_114" => Ok(Impersonate::Chrome114),
            "chrome_116" => Ok(Impersonate::Chrome116),
            "chrome_117" => Ok(Impersonate::Chrome117),
            "chrome_118" => Ok(Impersonate::Chrome118),
            "chrome_119" => Ok(Impersonate::Chrome119),
            "chrome_120" => Ok(Impersonate::Chrome120),
            "chrome_123" => Ok(Impersonate::Chrome123),
            "chrome_124" => Ok(Impersonate::Chrome124),
            "chrome_126" => Ok(Impersonate::Chrome126),
            "chrome_127" => Ok(Impersonate::Chrome127),
            "chrome_128" => Ok(Impersonate::Chrome128),
            "chrome_129" => Ok(Impersonate::Chrome129),
            "chrome_130" => Ok(Impersonate::Chrome130),
            "chrome_131" => Ok(Impersonate::Chrome131),
            "chrome_133" => Ok(Impersonate::Chrome133),
            "safari_ios_16.5" => Ok(Impersonate::SafariIos16_5),
            "safari_ios_17.2" => Ok(Impersonate::SafariIos17_2),
            "safari_ios_17.4.1" => Ok(Impersonate::SafariIos17_4_1),
            "safari_ios_18.1.1" => Ok(Impersonate::SafariIos18_1_1),
            "safari_ipad_18" => Ok(Impersonate::SafariIPad18),
            "safari_15.3" => Ok(Impersonate::Safari15_3),
            "safari_15.5" => Ok(Impersonate::Safari15_5),
            "safari_15.6.1" => Ok(Impersonate::Safari15_6_1),
            "safari_16" => Ok(Impersonate::Safari16),
            "safari_16.5" => Ok(Impersonate::Safari16_5),
            "safari_17.0" => Ok(Impersonate::Safari17_0),
            "safari_17.2.1" => Ok(Impersonate::Safari17_2_1),
            "safari_17.4.1" => Ok(Impersonate::Safari17_4_1),
            "safari_17.5" => Ok(Impersonate::Safari17_5),
            "safari_18" => Ok(Impersonate::Safari18),
            "safari_18.2" => Ok(Impersonate::Safari18_2),
            "okhttp_3.9" => Ok(Impersonate::OkHttp3_9),
            "okhttp_3.11" => Ok(Impersonate::OkHttp3_11),
            "okhttp_3.13" => Ok(Impersonate::OkHttp3_13),
            "okhttp_3.14" => Ok(Impersonate::OkHttp3_14),
            "okhttp_4.9" => Ok(Impersonate::OkHttp4_9),
            "okhttp_4.10" => Ok(Impersonate::OkHttp4_10),
            "okhttp_5" => Ok(Impersonate::OkHttp5),
            "edge_101" => Ok(Impersonate::Edge101),
            "edge_122" => Ok(Impersonate::Edge122),
            "edge_127" => Ok(Impersonate::Edge127),
            "edge_131" => Ok(Impersonate::Edge131),
            "firefox_109" => Ok(Impersonate::Firefox109),
            "firefox_117" => Ok(Impersonate::Firefox117),
            "firefox_128" => Ok(Impersonate::Firefox128),
            "firefox_133" => Ok(Impersonate::Firefox133),
            "firefox_135" => Ok(Impersonate::Firefox135),
            _ => Err(anyhow!("Invalid impersonate: {:?}", s)),
        }
    }
}

pub trait ImpersonateOSFromStr {
    fn from_str(s: &str) -> Result<ImpersonateOS>;
}

impl ImpersonateOSFromStr for ImpersonateOS {
    fn from_str(s: &str) -> Result<ImpersonateOS> {
        match s {
            "android" => Ok(ImpersonateOS::Android),
            "ios" => Ok(ImpersonateOS::IOS),
            "linux" => Ok(ImpersonateOS::Linux),
            "macos" => Ok(ImpersonateOS::MacOS),
            "windows" => Ok(ImpersonateOS::Windows),
            _ => Err(anyhow!("Invalid impersonate_os: {:?}", s)),
        }
    }
}
