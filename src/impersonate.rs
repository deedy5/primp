use anyhow::{anyhow, Result};
use rand::prelude::*;
use wreq_util::{Emulation, EmulationOS};

pub const EMULATION_LIST: &[Emulation] = &[
    // Chrome
    Emulation::Chrome100,
    Emulation::Chrome101,
    Emulation::Chrome104,
    Emulation::Chrome105,
    Emulation::Chrome106,
    Emulation::Chrome107,
    Emulation::Chrome108,
    Emulation::Chrome109,
    Emulation::Chrome110,
    Emulation::Chrome114,
    Emulation::Chrome116,
    Emulation::Chrome117,
    Emulation::Chrome118,
    Emulation::Chrome119,
    Emulation::Chrome120,
    Emulation::Chrome123,
    Emulation::Chrome124,
    Emulation::Chrome126,
    Emulation::Chrome127,
    Emulation::Chrome128,
    Emulation::Chrome129,
    Emulation::Chrome130,
    Emulation::Chrome131,
    Emulation::Chrome132,
    Emulation::Chrome133,
    Emulation::Chrome134,
    Emulation::Chrome135,
    Emulation::Chrome136,
    Emulation::Chrome137,
    
    // Safari iOS
    Emulation::SafariIos16_5,
    Emulation::SafariIos17_2,
    Emulation::SafariIos17_4_1,
    Emulation::SafariIos18_1_1,
    
    // Safari iPad
    Emulation::SafariIPad18,
    
    // Safari
    Emulation::Safari15_3,
    Emulation::Safari15_5,
    Emulation::Safari15_6_1,
    Emulation::Safari16,
    Emulation::Safari16_5,
    Emulation::Safari17_0,
    Emulation::Safari17_2_1,
    Emulation::Safari17_4_1,
    Emulation::Safari17_5,
    Emulation::Safari18,
    Emulation::Safari18_2,
    Emulation::Safari18_3,
    Emulation::Safari18_3_1,
    Emulation::Safari18_5,
    
    // OkHttp
    Emulation::OkHttp3_9,
    Emulation::OkHttp3_11,
    Emulation::OkHttp3_13,
    Emulation::OkHttp3_14,
    Emulation::OkHttp4_9,
    Emulation::OkHttp4_10,
    Emulation::OkHttp4_12,
    Emulation::OkHttp5,
    
    // Edge
    Emulation::Edge101,
    Emulation::Edge122,
    Emulation::Edge127,
    Emulation::Edge131,
    Emulation::Edge134,
    
    // Firefox
    Emulation::Firefox109,
    Emulation::Firefox117,
    Emulation::Firefox128,
    Emulation::Firefox133,
    Emulation::Firefox135,
    Emulation::FirefoxPrivate135,
    Emulation::FirefoxAndroid135,
    Emulation::Firefox136,
    Emulation::FirefoxPrivate136,
    Emulation::Firefox139,
    
    // Opera
    Emulation::Opera116,
    Emulation::Opera117,
    Emulation::Opera118,
    Emulation::Opera119,
];

pub const EMULATION_OS_LIST: &[EmulationOS] = &[
    EmulationOS::Android,
    EmulationOS::IOS,
    EmulationOS::Linux,
    EmulationOS::MacOS,
    EmulationOS::Windows,
];

pub fn get_random_element<T>(input_vec: &[T]) -> &T {
    input_vec.choose(&mut rand::rng()).unwrap()
}

pub trait EmulationFromStr {
    fn from_str(s: &str) -> Result<Emulation>;
}

impl EmulationFromStr for Emulation {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            // Chrome
            "chrome_100" => Ok(Emulation::Chrome100),
            "chrome_101" => Ok(Emulation::Chrome101),
            "chrome_104" => Ok(Emulation::Chrome104),
            "chrome_105" => Ok(Emulation::Chrome105),
            "chrome_106" => Ok(Emulation::Chrome106),
            "chrome_107" => Ok(Emulation::Chrome107),
            "chrome_108" => Ok(Emulation::Chrome108),
            "chrome_109" => Ok(Emulation::Chrome109),
            "chrome_110" => Ok(Emulation::Chrome110),
            "chrome_114" => Ok(Emulation::Chrome114),
            "chrome_116" => Ok(Emulation::Chrome116),
            "chrome_117" => Ok(Emulation::Chrome117),
            "chrome_118" => Ok(Emulation::Chrome118),
            "chrome_119" => Ok(Emulation::Chrome119),
            "chrome_120" => Ok(Emulation::Chrome120),
            "chrome_123" => Ok(Emulation::Chrome123),
            "chrome_124" => Ok(Emulation::Chrome124),
            "chrome_126" => Ok(Emulation::Chrome126),
            "chrome_127" => Ok(Emulation::Chrome127),
            "chrome_128" => Ok(Emulation::Chrome128),
            "chrome_129" => Ok(Emulation::Chrome129),
            "chrome_130" => Ok(Emulation::Chrome130),
            "chrome_131" => Ok(Emulation::Chrome131),
            "chrome_132" => Ok(Emulation::Chrome132),
            "chrome_133" => Ok(Emulation::Chrome133),
            "chrome_134" => Ok(Emulation::Chrome134),
            "chrome_135" => Ok(Emulation::Chrome135),
            "chrome_136" => Ok(Emulation::Chrome136),
            "chrome_137" => Ok(Emulation::Chrome137),
            
            // Safari iOS
            "safari_ios_16.5" => Ok(Emulation::SafariIos16_5),
            "safari_ios_17.2" => Ok(Emulation::SafariIos17_2),
            "safari_ios_17.4.1" => Ok(Emulation::SafariIos17_4_1),
            "safari_ios_18.1.1" => Ok(Emulation::SafariIos18_1_1),
            
            // Safari iPad
            "safari_ipad_18" => Ok(Emulation::SafariIPad18),
            
            // Safari
            "safari_15.3" => Ok(Emulation::Safari15_3),
            "safari_15.5" => Ok(Emulation::Safari15_5),
            "safari_15.6.1" => Ok(Emulation::Safari15_6_1),
            "safari_16" => Ok(Emulation::Safari16),
            "safari_16.5" => Ok(Emulation::Safari16_5),
            "safari_17.0" => Ok(Emulation::Safari17_0),
            "safari_17.2.1" => Ok(Emulation::Safari17_2_1),
            "safari_17.4.1" => Ok(Emulation::Safari17_4_1),
            "safari_17.5" => Ok(Emulation::Safari17_5),
            "safari_18" => Ok(Emulation::Safari18),
            "safari_18.2" => Ok(Emulation::Safari18_2),
            "safari_18.3" => Ok(Emulation::Safari18_3),
            "safari_18.3.1" => Ok(Emulation::Safari18_3_1),
            "safari_18.5" => Ok(Emulation::Safari18_5),
            
            // OkHttp
            "okhttp_3.9" => Ok(Emulation::OkHttp3_9),
            "okhttp_3.11" => Ok(Emulation::OkHttp3_11),
            "okhttp_3.13" => Ok(Emulation::OkHttp3_13),
            "okhttp_3.14" => Ok(Emulation::OkHttp3_14),
            "okhttp_4.9" => Ok(Emulation::OkHttp4_9),
            "okhttp_4.10" => Ok(Emulation::OkHttp4_10),
            "okhttp_4.12" => Ok(Emulation::OkHttp4_12),
            "okhttp_5" => Ok(Emulation::OkHttp5),
            
            // Edge
            "edge_101" => Ok(Emulation::Edge101),
            "edge_122" => Ok(Emulation::Edge122),
            "edge_127" => Ok(Emulation::Edge127),
            "edge_131" => Ok(Emulation::Edge131),
            "edge_134" => Ok(Emulation::Edge134),
            
            // Firefox
            "firefox_109" => Ok(Emulation::Firefox109),
            "firefox_117" => Ok(Emulation::Firefox117),
            "firefox_128" => Ok(Emulation::Firefox128),
            "firefox_133" => Ok(Emulation::Firefox133),
            "firefox_135" => Ok(Emulation::Firefox135),
            "firefox_private_135" => Ok(Emulation::FirefoxPrivate135),
            "firefox_android_135" => Ok(Emulation::FirefoxAndroid135),
            "firefox_136" => Ok(Emulation::Firefox136),
            "firefox_private_136" => Ok(Emulation::FirefoxPrivate136),
            "firefox_139" => Ok(Emulation::Firefox139),
            
            // Opera
            "opera_116" => Ok(Emulation::Opera116),
            "opera_117" => Ok(Emulation::Opera117),
            "opera_118" => Ok(Emulation::Opera118),
            "opera_119" => Ok(Emulation::Opera119),
            
            "random" => Ok(*get_random_element(EMULATION_LIST)),
            _ => Err(anyhow!("Invalid emulation: {:?}", s)),
        }
    }
}

pub trait EmulationOSFromStr {
    fn from_str(s: &str) -> Result<EmulationOS>;
}

impl EmulationOSFromStr for EmulationOS {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "android" => Ok(EmulationOS::Android),
            "ios" => Ok(EmulationOS::IOS),
            "linux" => Ok(EmulationOS::Linux),
            "macos" => Ok(EmulationOS::MacOS),
            "windows" => Ok(EmulationOS::Windows),
            "random" => Ok(*get_random_element(EMULATION_OS_LIST)),
            _ => Err(anyhow!("Invalid emulation_os: {:?}", s)),
        }
    }
}