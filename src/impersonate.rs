use anyhow::{anyhow, Result};
use rand::prelude::*;
use rquest::{Impersonate, ImpersonateOS};

pub const IMPERSONATE_LIST: &[Impersonate] = &[
    Impersonate::Chrome100,
    Impersonate::Chrome101,
    Impersonate::Chrome104,
    Impersonate::Chrome105,
    Impersonate::Chrome106,
    Impersonate::Chrome107,
    Impersonate::Chrome108,
    Impersonate::Chrome109,
    Impersonate::Chrome114,
    Impersonate::Chrome116,
    Impersonate::Chrome117,
    Impersonate::Chrome118,
    Impersonate::Chrome119,
    Impersonate::Chrome120,
    Impersonate::Chrome123,
    Impersonate::Chrome124,
    Impersonate::Chrome126,
    Impersonate::Chrome127,
    Impersonate::Chrome128,
    Impersonate::Chrome129,
    Impersonate::Chrome130,
    Impersonate::Chrome131,
    Impersonate::Chrome133,
    Impersonate::SafariIos16_5,
    Impersonate::SafariIos17_2,
    Impersonate::SafariIos17_4_1,
    Impersonate::SafariIos18_1_1,
    Impersonate::SafariIPad18,
    Impersonate::Safari15_3,
    Impersonate::Safari15_5,
    Impersonate::Safari15_6_1,
    Impersonate::Safari16,
    Impersonate::Safari16_5,
    Impersonate::Safari17_0,
    Impersonate::Safari17_2_1,
    Impersonate::Safari17_4_1,
    Impersonate::Safari17_5,
    Impersonate::Safari18,
    Impersonate::Safari18_2,
    //Impersonate::OkHttp3_9,
    //Impersonate::OkHttp3_11,
    Impersonate::OkHttp3_13,
    Impersonate::OkHttp3_14,
    Impersonate::OkHttp4_9,
    Impersonate::OkHttp4_10,
    Impersonate::OkHttp5,
    Impersonate::Edge101,
    Impersonate::Edge122,
    Impersonate::Edge127,
    Impersonate::Edge131,
    Impersonate::Firefox109,
    Impersonate::Firefox117,
    Impersonate::Firefox128,
    Impersonate::Firefox133,
    Impersonate::Firefox135,
];
pub const IMPERSONATEOS_LIST: &[ImpersonateOS] = &[
    ImpersonateOS::Android,
    ImpersonateOS::IOS,
    ImpersonateOS::Linux,
    ImpersonateOS::MacOS,
    ImpersonateOS::Windows,
];

pub fn get_random_element<T>(input_vec: &[T]) -> &T {
    input_vec.choose(&mut rand::rng()).unwrap()
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
            "random" => Ok(*get_random_element(IMPERSONATE_LIST)),
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
            "random" => Ok(*get_random_element(IMPERSONATEOS_LIST)),
            _ => Err(anyhow!("Invalid impersonate_os: {:?}", s)),
        }
    }
}
