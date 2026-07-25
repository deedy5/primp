#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(test, deny(warnings))]

//! # primp
//!
//! HTTP client with browser impersonation (TLS/HTTP2 fingerprinting) support.

use sync_wrapper as _;

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
pub mod dns;
pub mod imp;
mod impersonation;
pub mod tls;
mod tls_bridge;
mod util;
pub use imp::{BrowserSettings, Http2Data, Impersonate, ImpersonateOS};
mod connect;
#[cfg(feature = "cookies")]
pub mod cookie;
mod proxy;
pub mod redirect;
pub mod retry;
#[cfg(feature = "multipart")]
pub use self::async_impl::multipart;
pub use self::config::{OneShotCookies, RedirectOverride, RedirectPolicyOverride};
pub use self::proxy::{NoProxy, Proxy};
pub use self::tls::{Certificate, Identity, TlsInfo};
mod async_impl;
pub use self::async_impl::{
    Body, Client, ClientBuilder, Request, RequestBuilder, Response, Upgraded,
};

/// Shortcut method to quickly make a `GET` request.
///
/// Convenience for `Client::builder().build()?.get(url).send().await`.
/// Creates a temporary client with default settings.
pub async fn get<T: IntoUrl>(url: T) -> crate::Result<Response> {
    Client::builder().build()?.get(url).send().await
}
pub(crate) fn strip_ipv6_brackets(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host)
}

#[cfg(test)]
#[macro_use]
extern crate doc_comment;

#[cfg(test)]
doctest!("../README.md");
