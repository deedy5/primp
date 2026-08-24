#![allow(clippy::too_many_arguments)]
use std::sync::{Arc, RwLock};

use ::primp::{multipart, Body, Client as PrimpClient, Method, Response as PrimpResponse, Url};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use pythonize::depythonize;
use serde_json::Value;
use tokio::{
    fs::File,
    runtime::{self, Runtime},
};
use tokio_util::codec::{BytesCodec, FramedRead};

mod client_builder;
use client_builder::{
    build_request_cookie_header, configure_client_builder, parse_dns_resolver, IndexMapSSR,
};

mod error;
use error::{PrimpErrorEnum, PrimpResult};

mod impersonate;
mod response;
use response::{BytesIterator, LinesIterator, Response, TextIterator};

mod r#async;

mod response_shared;
mod traits;
use traits::HeadersTraits;

mod utils;
use utils::extract_encoding;

// Tokio global one-thread runtime
static RUNTIME: PyOnceLock<Runtime> = PyOnceLock::new();

/// Get the global Tokio runtime, initializing it via `PyOnceLock` if necessary.
///
/// Returns a `PyErr` (not a panic) on creation failure, so a host-resource
/// failure surfaces as a Python exception rather than aborting the interpreter
/// under `panic = "abort"`.
pub(crate) fn get_runtime(py: Python<'_>) -> PyResult<&Runtime> {
    RUNTIME.get_or_try_init(py, || {
        runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Failed to create Tokio runtime: {e}"
                ))
            })
    })
}

#[pyclass(subclass)]
/// HTTP client that can impersonate web browsers.
pub struct Client {
    client: Arc<RwLock<PrimpClient>>,
    #[pyo3(get, set)]
    auth: Option<(String, Option<String>)>,
    #[pyo3(get, set)]
    auth_bearer: Option<String>,
    #[pyo3(get, set)]
    params: Option<IndexMapSSR>,
    proxy: Option<String>,
    #[pyo3(get, set)]
    timeout: Option<f64>,
    #[pyo3(get)]
    connect_timeout: Option<f64>,
    #[pyo3(get, set)]
    read_timeout: Option<f64>,
    #[pyo3(get)]
    dns_timeout: Option<f64>,
    #[pyo3(get)]
    impersonate: Option<String>,
    #[pyo3(get)]
    impersonate_os: Option<String>,
    #[pyo3(get, set)]
    base_url: Option<String>,
    cookies: Option<IndexMapSSR>,
    #[pyo3(get, set)]
    max_redirects: Option<usize>,
    #[pyo3(get, set)]
    follow_redirects: Option<bool>,
}

pub fn extract_cookies_to_indexmap(headers: &http::HeaderMap) -> IndexMapSSR {
    let mut cookie_map = IndexMapSSR::default();
    for cookie_header in headers.get_all(http::header::SET_COOKIE).iter() {
        if let Ok(cookie_str) = cookie_header.to_str() {
            if let Some((name, rest)) = cookie_str.split_once('=') {
                let value = rest.split(';').next().unwrap_or("").trim();
                cookie_map.insert(name.trim().to_string(), value.to_string());
            }
        }
    }
    cookie_map
}

/// Convert a non-Object `serde_json::Value` to a raw string body.
///
/// Objects are routed through `.form()`/`.json()` instead, but a stray
/// `Object` is serialized here rather than panicked, to avoid an FFI abort.
pub(crate) fn body_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(arr) => serde_json::to_string(arr).unwrap_or_default(),
        Value::Object(_) => serde_json::to_string(v).unwrap_or_default(),
    }
}

#[pymethods]
impl Client {
    /// Initialize an HTTP client that impersonates web browsers.
    ///
    /// Customizes headers, proxy, timeouts, impersonation, TLS verification, and
    /// HTTP-version preference. With `impersonate` set, `headers` are ignored.
    ///
    /// Defaults: `cookie_store=true`, `referer=true`, `follow_redirects=true`,
    /// `max_redirects=20`, `verify=true`, `https_only=false`, `http2_only=false`.
    #[new]
    #[pyo3(signature = (auth=None, auth_bearer=None, params=None, headers=None, cookie_store=true,
        referer=true, proxy=None, timeout=None, connect_timeout=None, read_timeout=None,
        dns_timeout=None, impersonate=None, impersonate_os=None, follow_redirects=true,
        max_redirects=20, verify=true, ca_cert_file=None, https_only=false, http2_only=false,
        dns_resolver=None, base_url=None, cookies=None))]
    fn new(
        py: Python<'_>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookie_store: Option<bool>,
        referer: Option<bool>,
        proxy: Option<String>,
        timeout: Option<f64>,
        connect_timeout: Option<f64>,
        read_timeout: Option<f64>,
        dns_timeout: Option<f64>,
        impersonate: Option<String>,
        impersonate_os: Option<String>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
        verify: Option<bool>,
        ca_cert_file: Option<String>,
        https_only: Option<bool>,
        http2_only: Option<bool>,
        dns_resolver: Option<pyo3::Bound<'_, pyo3::types::PyAny>>,
        base_url: Option<String>,
        cookies: Option<IndexMapSSR>,
    ) -> PrimpResult<Self> {
        let dns_resolvers = parse_dns_resolver(dns_resolver)?;
        let (resolved_proxy, client) = py.detach(|| -> PrimpResult<_> {
            let (client_builder, resolved_proxy) = configure_client_builder(
                PrimpClient::builder(),
                headers,
                cookie_store,
                referer,
                proxy,
                timeout,
                connect_timeout,
                read_timeout,
                dns_timeout,
                impersonate.as_deref(),
                impersonate_os.as_deref(),
                follow_redirects,
                max_redirects,
                verify,
                ca_cert_file,
                https_only,
                http2_only,
                dns_resolvers,
            )?;

            let client = Arc::new(RwLock::new(client_builder.build()?));
            Ok((resolved_proxy, client))
        })?;

        Ok(Client {
            client,
            auth,
            auth_bearer,
            params,
            proxy: resolved_proxy,
            timeout,
            connect_timeout,
            read_timeout,
            dns_timeout,
            impersonate,
            impersonate_os,
            base_url,
            cookies,
            max_redirects,
            follow_redirects,
        })
    }

    #[getter]
    pub fn get_headers(&self) -> PrimpResult<IndexMapSSR> {
        client_builder::client_headers(&self.client)
    }

    #[setter]
    pub fn set_headers(&self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        client_builder::client_set_headers(&self.client, new_headers)
    }

    pub fn headers_update(&self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        client_builder::client_headers_update(&self.client, new_headers)
    }

    #[getter]
    pub fn get_proxy(&self) -> PrimpResult<Option<String>> {
        Ok(self.proxy.to_owned())
    }

    #[setter]
    pub fn set_proxy(&mut self, proxy: Option<String>) -> PrimpResult<()> {
        self.proxy = client_builder::client_set_proxy(&self.client, proxy)?;
        Ok(())
    }

    #[pyo3(signature = (url))]
    fn get_cookies(&self, url: &str) -> PrimpResult<IndexMapSSR> {
        client_builder::client_get_cookies(&self.client, url)
    }

    #[pyo3(signature = (url, cookies))]
    fn set_cookies(&self, url: &str, cookies: Option<IndexMapSSR>) -> PrimpResult<()> {
        client_builder::client_set_cookies(&self.client, url, cookies)
    }

    /// Build and send a request, returning a `Response`.
    ///
    /// Per-request options (override the client for this call only):
    /// `params`, `headers`, `cookies`, `content`, `data`, `json`, `files`,
    /// `auth`, `auth_bearer`, `timeout`, `read_timeout`, `follow_redirects`,
    /// `stream`.
    ///
    /// Client-scoped and NOT settable per request — only at construction (or
    /// via module-level helpers, which build a throwaway client):
    /// `impersonate`, `impersonate_os`, `connect_timeout`, `https_only`,
    /// `http2_only`.
    #[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None,
        data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None,
        read_timeout=None, follow_redirects=None, stream=false))]
    fn request(
        &self,
        py: Python,
        method: &str,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        let client = Arc::clone(&self.client);
        let method = Method::from_bytes(method.as_bytes()).map_err(Into::<PrimpErrorEnum>::into)?;
        let data_value: Option<Value> = data
            .map(depythonize)
            .transpose()
            .map_err(Into::<PrimpErrorEnum>::into)?;
        if data.is_some() && files.is_some() {
            return Err(PrimpErrorEnum::Custom(
                "data and files cannot both be provided (use files alone for multipart uploads)"
                    .into(),
            )
            .into());
        }
        let json_value: Option<Value> = json
            .map(depythonize)
            .transpose()
            .map_err(Into::<PrimpErrorEnum>::into)?;

        let resolved_timeout: Option<f64> = timeout.or(self.timeout);

        // Resolve URL with base_url
        let resolved_url = if let Some(ref base_url) = self.base_url {
            if url.starts_with("http://") || url.starts_with("https://") {
                url.to_string()
            } else {
                let base = base_url.trim_end_matches('/');
                let path = url.trim_start_matches('/');
                format!("{}/{}", base, path)
            }
        } else {
            url.to_string()
        };

        // Cookies: client-level persist in the store; per-request are merged
        // into a one-shot `Cookie` header so they don't leak into the store
        // (matches `requests`/`httpx`). The jar itself is merged per hop by
        // the core cookie service, so redirect chains get fresh Set-Cookies.
        let request_cookie_header: Option<String> = {
            let url_parsed = Url::parse(&resolved_url).map_err(Into::<PrimpErrorEnum>::into)?;
            let client_guard = client.read().unwrap_or_else(|e| e.into_inner());
            build_request_cookie_header(
                &client_guard,
                &url_parsed,
                self.cookies.as_ref(),
                cookies.as_ref(),
            )
        };

        // Clone the inner client to avoid holding the RwLock across await points
        let client_clone = client.read().unwrap_or_else(|e| e.into_inner()).clone();

        // Per-request redirect override via a request extension — the shared
        // client is never mutated. Param wins over the `follow_redirects`
        // attribute; `max_redirects` caps `Follow`.
        let resolved_follow_redirects = follow_redirects.or(self.follow_redirects);
        let client_max_redirects = self.max_redirects;

        let future = async move {
            // Create request builder using the cloned client
            let mut request_builder = client_clone.request(method, &resolved_url);

            // Per-request redirect override, carried on the request extensions.
            if let Some(fr) = resolved_follow_redirects {
                let override_policy = if fr {
                    ::primp::RedirectOverride::Follow(client_max_redirects.unwrap_or(20))
                } else {
                    ::primp::RedirectOverride::Disabled
                };
                request_builder = request_builder.redirect_override(override_policy);
            }

            // Params
            if let Some(p) = params.as_ref().or(self.params.as_ref()) {
                request_builder = request_builder.query(p);
            }

            // Set the one-shot Cookie header (jar is merged per hop by the
            // core cookie service; see `OneShotCookies` request config).
            if let Some(cookie_str) = request_cookie_header {
                match http::HeaderValue::from_str(&cookie_str) {
                    Ok(hv) => {
                        request_builder = request_builder.header(http::header::COOKIE, hv.clone());
                        request_builder = request_builder.one_shot_cookies(hv);
                    }
                    Err(e) => {
                        tracing::warn!("primp: invalid characters in cookie header, skipping: {e}");
                    }
                }
            }

            // Headers
            if let Some(headers) = headers {
                request_builder = request_builder.headers(headers.to_headermap()?);
            }

            // Body content (if provided)
            if let Some(content) = content {
                request_builder = request_builder.body(content);
            }
            // Form data (if provided) — only form-encode objects; send scalars as raw body
            if let Some(form_data) = data_value {
                match form_data {
                    Value::Object(_) => {
                        request_builder = request_builder.form(&form_data);
                    }
                    other => {
                        let body = body_value_to_string(&other);
                        request_builder = request_builder.body(body);
                    }
                }
            }
            // JSON (if provided)
            if let Some(json_data) = json_value {
                request_builder = request_builder.json(&json_data);
            }
            // Files (if provided)
            if let Some(files) = files {
                let mut form = multipart::Form::new();
                for (file_name, file_path) in files {
                    let file = File::open(file_path)
                        .await
                        .map_err(Into::<PrimpErrorEnum>::into)?;
                    let stream = FramedRead::new(file, BytesCodec::new());
                    let file_body = Body::wrap_stream(stream);
                    let part = multipart::Part::stream(file_body).file_name(file_name.clone());
                    form = form.part(file_name, part);
                }
                request_builder = request_builder.multipart(form);
            }

            // Auth
            if let Some((u, p)) = auth.as_ref().or(self.auth.as_ref()) {
                request_builder = request_builder.basic_auth(u, p.as_deref());
            } else if let Some(t) = auth_bearer.as_ref().or(self.auth_bearer.as_ref()) {
                request_builder = request_builder.bearer_auth(t);
            }

            // Timeout
            if let Some(seconds) = resolved_timeout {
                request_builder = request_builder.timeout(crate::utils::timeout_duration(seconds)?);
            }

            // Per-request read timeout (falls back to client-level setting)
            let resolved_read_timeout = read_timeout.or(self.read_timeout);
            if let Some(seconds) = resolved_read_timeout {
                request_builder =
                    request_builder.read_timeout(crate::utils::timeout_duration(seconds)?);
            }

            // Send the request and await the response
            let resp: PrimpResponse = request_builder
                .send()
                .await
                .map_err(Into::<PrimpErrorEnum>::into)?;
            let url: String = resp.url().to_string();
            let status_code = resp.status().as_u16();

            tracing::info!("response: {} {}", url, status_code);
            Ok((resp, url, status_code))
        };

        // Execute an async future, releasing the Python GIL for concurrency.
        // Use Tokio global runtime to block on the future. The redirect
        // override is request-scoped (a per-request extension), so no
        // post-hoc restore is needed here.
        let runtime = get_runtime(py)?;
        let response: Result<(PrimpResponse, String, u16), PrimpErrorEnum> =
            py.detach(move || runtime.block_on(future));

        let result = response?;
        let resp = result.0;
        let url = result.1;
        let status_code = result.2;

        if stream {
            let headers: IndexMapSSR = resp.headers().to_indexmap();
            let cookies = extract_cookies_to_indexmap(resp.headers());
            let encoding = extract_encoding(resp.headers()).name().to_string();

            let response =
                Response::new_streaming(resp, url, status_code, encoding, headers, cookies);
            Ok(response.into_pyobject(py)?.into_any().unbind())
        } else {
            let headers: IndexMapSSR = resp.headers().to_indexmap();
            let cookies = extract_cookies_to_indexmap(resp.headers());
            let encoding = extract_encoding(resp.headers()).name().to_string();

            let response = Response::new(resp, url, status_code, headers, cookies, encoding);
            Ok(response.into_pyobject(py)?.into_any().unbind())
        }
    }

    /// Send a GET request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn get(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "GET",
            url,
            params,
            headers,
            cookies,
            content,
            data,
            json,
            files,
            auth,
            auth_bearer,
            timeout,
            read_timeout,
            follow_redirects,
            stream,
        )
    }

    /// Send a HEAD request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn head(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "HEAD",
            url,
            params,
            headers,
            cookies,
            content,
            data,
            json,
            files,
            auth,
            auth_bearer,
            timeout,
            read_timeout,
            follow_redirects,
            stream,
        )
    }

    /// Send an OPTIONS request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn options(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "OPTIONS",
            url,
            params,
            headers,
            cookies,
            content,
            data,
            json,
            files,
            auth,
            auth_bearer,
            timeout,
            read_timeout,
            follow_redirects,
            stream,
        )
    }

    /// Send a DELETE request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn delete(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "DELETE",
            url,
            params,
            headers,
            cookies,
            content,
            data,
            json,
            files,
            auth,
            auth_bearer,
            timeout,
            read_timeout,
            follow_redirects,
            stream,
        )
    }

    /// Send a POST request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn post(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "POST",
            url,
            params,
            headers,
            cookies,
            content,
            data,
            json,
            files,
            auth,
            auth_bearer,
            timeout,
            read_timeout,
            follow_redirects,
            stream,
        )
    }

    /// Send a PUT request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn put(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "PUT",
            url,
            params,
            headers,
            cookies,
            content,
            data,
            json,
            files,
            auth,
            auth_bearer,
            timeout,
            read_timeout,
            follow_redirects,
            stream,
        )
    }

    /// Send a PATCH request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn patch(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        content: Option<Vec<u8>>,
        data: Option<&Bound<'_, PyAny>>,
        json: Option<&Bound<'_, PyAny>>,
        files: Option<indexmap::IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        read_timeout: Option<f64>,
        follow_redirects: Option<bool>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "PATCH",
            url,
            params,
            headers,
            cookies,
            content,
            data,
            json,
            files,
            auth,
            auth_bearer,
            timeout,
            read_timeout,
            follow_redirects,
            stream,
        )
    }

    /// Support for context manager protocol.
    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// Exit the context manager.
    fn __exit__(
        &mut self,
        _exc_type: Option<Bound<'_, PyAny>>,
        _exc_value: Option<Bound<'_, PyAny>>,
        _traceback: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        Ok(())
    }
}

/// Send a GET request with a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn get(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.get(
        py,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

/// Send a HEAD request with a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn head(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.head(
        py,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

/// Send an OPTIONS request with a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn options(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.options(
        py,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

/// Send a DELETE request with a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn delete(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.delete(
        py,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

/// Send a POST request with a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn post(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.post(
        py,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

/// Send a PUT request with a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn put(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.put(
        py,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

/// Send a PATCH request with a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn patch(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.patch(
        py,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

/// Send a request with a custom method using a temporary (throwaway) client.
#[pyfunction]
#[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, dns_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
fn request(
    py: Python,
    method: &str,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<indexmap::IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    connect_timeout: Option<f64>,
    read_timeout: Option<f64>,
    dns_timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    follow_redirects: Option<bool>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        py,
        None,
        None,
        None,
        headers,
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
        dns_timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
        None,
        None,
        None,
    )?;
    client.request(
        py,
        method,
        url,
        params,
        None,
        cookies,
        content,
        data,
        json,
        files,
        auth,
        auth_bearer,
        timeout,
        read_timeout,
        follow_redirects,
        stream,
    )
}

#[pymodule]
fn primp(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    // Async bridge: `_wrap_awaitable` coroutine + atexit abort of in-flight
    // bridge tasks (prevents post-finalize panics under `panic = "abort"`).
    crate::r#async::bridge::init(_py, m)?;

    // Re-export exception types from error module - new hierarchy
    use error::{
        BodyError, BuilderError, ConnectError, DNSError, DecodeError, PrimpError, RedirectError,
        RequestError, StatusError, TimeoutError, UpgradeError,
    };

    // Add exception types - primp native hierarchy
    // Base exception
    m.add("PrimpError", _py.get_type::<PrimpError>())?;

    // Builder errors
    m.add("BuilderError", _py.get_type::<BuilderError>())?;

    // Request errors
    m.add("RequestError", _py.get_type::<RequestError>())?;
    m.add("ConnectError", _py.get_type::<ConnectError>())?;
    m.add("TimeoutError", _py.get_type::<TimeoutError>())?;
    m.add("DNSError", _py.get_type::<DNSError>())?;

    // Other errors
    m.add("StatusError", _py.get_type::<StatusError>())?;
    // Expose status_code/url as properties for docs example `e.status_code`.
    {
        let ty = _py.get_type::<StatusError>();
        let locals = PyDict::new(_py);
        locals.set_item("ty", ty.clone())?;
        _py.run(
            c"ty.status_code = property(lambda self: self.args[0] if self.args else None)",
            None,
            Some(&locals),
        )?;
        _py.run(
            c"ty.url = property(lambda self: self.args[2] if len(self.args) > 2 else None)",
            None,
            Some(&locals),
        )?;
    }
    m.add("RedirectError", _py.get_type::<RedirectError>())?;
    m.add("BodyError", _py.get_type::<BodyError>())?;
    m.add("DecodeError", _py.get_type::<DecodeError>())?;
    m.add("UpgradeError", _py.get_type::<UpgradeError>())?;

    // Create a combined JSONDecodeError inheriting from both DecodeError
    // (PrimpError subclass) and json.JSONDecodeError (ValueError subclass)
    // so JSON parse failures are catchable via both `except PrimpError`
    // and `except json.JSONDecodeError`. This mirrors the `requests` library
    // pattern and preserves position info (.doc, .pos, .lineno, .colno).
    {
        let locals = PyDict::new(_py);
        locals.set_item("decode_err", _py.get_type::<DecodeError>().clone())?;
        locals.set_item(
            "json_dec_err",
            _py.import("json")?.getattr("JSONDecodeError")?.clone(),
        )?;
        let combined = _py.eval(
            c"type('JSONDecodeError', (decode_err, json_dec_err), {'__module__': 'primp'})",
            None,
            Some(&locals),
        )?;
        m.add("JSONDecodeError", combined)?;
    }

    // Combined DNSTimeoutError (DNSError + TimeoutError) so a DNS lookup
    // timeout is catchable via either parent. Built once here and cached for
    // GIL-free per-error conversion (`error::init_dnstimeout_error`).
    crate::error::init_dnstimeout_error(_py, m)?;

    // Response classes
    m.add_class::<Response>()?;
    m.add_class::<BytesIterator>()?;
    m.add_class::<TextIterator>()?;
    m.add_class::<LinesIterator>()?;

    // Async classes
    m.add_class::<Client>()?;
    m.add_class::<r#async::AsyncClient>()?;
    m.add_class::<r#async::AsyncResponse>()?;
    m.add_class::<r#async::AsyncBytesIterator>()?;
    m.add_class::<r#async::AsyncTextIterator>()?;
    m.add_class::<r#async::AsyncLinesIterator>()?;

    // Module-level convenience functions
    m.add_function(wrap_pyfunction!(get, m)?)?;
    m.add_function(wrap_pyfunction!(head, m)?)?;
    m.add_function(wrap_pyfunction!(options, m)?)?;
    m.add_function(wrap_pyfunction!(delete, m)?)?;
    m.add_function(wrap_pyfunction!(post, m)?)?;
    m.add_function(wrap_pyfunction!(put, m)?)?;
    m.add_function(wrap_pyfunction!(patch, m)?)?;
    m.add_function(wrap_pyfunction!(request, m)?)?;

    // Version
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
