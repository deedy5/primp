#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(test, deny(warnings))]

//! # primp

pub use http::header;
pub use http::Method;
pub use http::{StatusCode, Version};
pub use url::Url;

mod error;
pub use self::error::{Error, Result};

mod into_url;
pub use self::into_url::IntoUrl;

mod response;
pub use self::response::ResponseBuilderExt;

mod config;
pub use self::config::{OneShotCookies, RedirectOverride, RedirectPolicyOverride};

pub mod tls;
pub use self::tls::{Certificate, Identity, TlsInfo};

mod util;

#[cfg(feature = "cookies")]
pub mod cookie;
