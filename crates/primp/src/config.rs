//! Generic, type-safe, request-scoped configuration storage.
//!
//! [`RequestConfigValue`] associates a config-key marker type with its value
//! type; [`RequestConfig<T>`] wraps an optional value stored in
//! [`http::Extensions`], letting multiple distinct config types (even with the
//! same value type) coexist without ambiguity.

use std::any::type_name;
use std::fmt::Debug;
use std::time::Duration;

use http::Extensions;

/// Associates a config-key type with its value type. Empty by design.
pub(crate) trait RequestConfigValue: Copy + Clone + 'static {
    type Value: Clone + Debug + Send + Sync + 'static;
}

/// Carries a request-scoped configuration value of type `T::Value`.
#[derive(Clone, Copy)]
pub(crate) struct RequestConfig<T: RequestConfigValue>(Option<T::Value>);

impl<T: RequestConfigValue> Default for RequestConfig<T> {
    fn default() -> Self {
        RequestConfig(None)
    }
}

impl<T> RequestConfig<T>
where
    T: RequestConfigValue,
{
    pub(crate) fn new(v: Option<T::Value>) -> Self {
        RequestConfig(v)
    }

    /// Render this config value as a debug struct field, hiding the inner value
    /// from callers.
    pub(crate) fn fmt_as_field(&self, f: &mut std::fmt::DebugStruct<'_, '_>) {
        if let Some(v) = &self.0 {
            f.field(type_name::<T>(), v);
        }
    }

    /// Resolve the value: prefer the request's `Extensions`, else this
    /// client-level instance.
    pub(crate) fn fetch<'client, 'request>(
        &'client self,
        ext: &'request Extensions,
    ) -> Option<&'request T::Value>
    where
        'client: 'request,
    {
        ext.get::<RequestConfig<T>>()
            .and_then(|v| v.0.as_ref())
            .or(self.0.as_ref())
    }

    /// Get the value from the request's `Extensions`.
    pub(crate) fn get(ext: &Extensions) -> Option<&T::Value> {
        ext.get::<RequestConfig<T>>().and_then(|v| v.0.as_ref())
    }

    /// Get the value from an owned `RequestConfig` instance.
    pub(crate) fn get_value(&self) -> Option<&T::Value> {
        self.0.as_ref()
    }

    /// Get the mutable value from the request's `Extensions`.
    pub(crate) fn get_mut(ext: &mut Extensions) -> &mut Option<T::Value> {
        let cfg = ext.get_or_insert_default::<RequestConfig<T>>();
        &mut cfg.0
    }
}

// ================================
//
// The following sections are all configuration types
// provided by primp.
//
// To add a new config:
//
// 1. create a new struct for the config key like `RequestTimeout`.
// 2. implement `RequestConfigValue` for the struct, the `Value` is the config value's type.
//
// ================================

#[derive(Clone, Copy)]
pub(crate) struct TotalTimeout;

impl RequestConfigValue for TotalTimeout {
    type Value = Duration;
}

#[derive(Clone, Copy)]
pub(crate) struct ReadTimeout;

impl RequestConfigValue for ReadTimeout {
    type Value = Duration;
}

/// Per-request redirect behavior override, carried on the request `Extensions`
/// and read by `TowerRedirectPolicy` on the first request of the redirect
/// chain (replaces the old racy shared-policy mutation in the Python bindings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectOverride {
    /// Follow redirects with the given hop limit.
    Follow(usize),
    /// Do not follow redirects (return the 30x response as-is).
    Disabled,
}

/// Request-config key for the per-request redirect override.
#[derive(Clone, Copy, Debug)]
pub struct RedirectPolicyOverride;

impl RequestConfigValue for RedirectPolicyOverride {
    type Value = RedirectOverride;
}

/// Request-config key for one-shot request cookies (Python `cookies=`):
/// carried as a request `Extensions` value so the cookie service can re-merge
/// them with fresh jar state on every hop of a redirect chain. A plain
/// pre-merged `Cookie` header goes stale on hop 1+ (tower-http preserves
/// headers across same-origin hops) and suppresses jar injection.
#[derive(Clone, Copy, Debug)]
pub struct OneShotCookies;

impl RequestConfigValue for OneShotCookies {
    type Value = http::HeaderValue;
}
