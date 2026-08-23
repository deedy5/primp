use std::sync::{Arc, RwLock};

use ::primp::{multipart, Body, Client as PrimpClient, Method, Response as PrimpResponse, Url};
use pyo3::prelude::*;
use pythonize::depythonize;
use serde_json::Value;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::body_value_to_string;
use crate::client_builder::{
    build_request_cookie_header, configure_client_builder, parse_dns_resolver, IndexMapSSR,
};
use crate::error::{PrimpErrorEnum, PrimpResult};
use crate::extract_cookies_to_indexmap;
use crate::traits::HeadersTraits;
use crate::utils::extract_encoding;

/// Async HTTP client that can impersonate web browsers.
#[pyclass(subclass)]
pub struct AsyncClient {
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

#[pymethods]
impl AsyncClient {
    /// Initializes an async HTTP client that can impersonate web browsers.
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

        Ok(AsyncClient {
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
        crate::client_builder::client_headers(&self.client)
    }

    #[setter]
    pub fn set_headers(&mut self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        crate::client_builder::client_set_headers(&self.client, new_headers)
    }

    pub fn headers_update(&self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        crate::client_builder::client_headers_update(&self.client, new_headers)
    }

    #[getter]
    pub fn get_proxy(&self) -> PrimpResult<Option<String>> {
        Ok(self.proxy.to_owned())
    }

    #[setter]
    pub fn set_proxy(&mut self, proxy: Option<String>) -> PrimpResult<()> {
        self.proxy = crate::client_builder::client_set_proxy(&self.client, proxy)?;
        Ok(())
    }

    #[pyo3(signature = (url))]
    fn get_cookies(&self, url: &str) -> PrimpResult<IndexMapSSR> {
        crate::client_builder::client_get_cookies(&self.client, url)
    }

    #[pyo3(signature = (url, cookies))]
    fn set_cookies(&self, url: &str, cookies: Option<IndexMapSSR>) -> PrimpResult<()> {
        crate::client_builder::client_set_cookies(&self.client, url, cookies)
    }

    /// Build and send an async request, returning a `Response`.
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
    fn request<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::{future_into_coroutine, BridgeTaskError};

        let method = Method::from_bytes(method.as_bytes()).map_err(Into::<PrimpErrorEnum>::into)?;

        let resolved_timeout: Option<f64> = timeout.or(self.timeout);
        let resolved_read_timeout: Option<f64> = read_timeout.or(self.read_timeout);

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

        let params = match (params, self.params.as_ref()) {
            (Some(p), _) => Some(p),
            (None, Some(c)) if !c.is_empty() => Some(c.clone()),
            _ => None,
        };
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
        let auth = auth.or(self.auth.clone());
        let auth_bearer = auth_bearer.or(self.auth_bearer.clone());

        // Cookies: client-level persist in the store; per-request are merged
        // into a one-shot `Cookie` header so they don't leak into the store
        // (matches `requests`/`httpx`). The jar itself is merged per hop by
        // the core cookie service, so redirect chains get fresh Set-Cookies.
        let request_cookie_header: Option<String> = {
            let url_parsed = Url::parse(&resolved_url).map_err(Into::<PrimpErrorEnum>::into)?;
            let client_guard = self.client.read().unwrap_or_else(|e| e.into_inner());
            build_request_cookie_header(
                &client_guard,
                &url_parsed,
                self.cookies.as_ref(),
                cookies.as_ref(),
            )
        };

        // Clone the client before entering the async block so the future is
        // self-contained and `Send` (a borrowed `RwLockGuard` would not be
        // safe to hold across an await / send to another runtime thread). The
        // `Client` clone is cheap — it shares the underlying connector, pools
        // and cookie store via `Arc` — so this is a few atomic refcount bumps
        // per request rather than a deep copy.
        let client = {
            let client_guard = self.client.read().unwrap_or_else(|e| e.into_inner());
            client_guard.clone()
        };

        // Per-request redirect override via a request extension — the shared
        // client is never mutated. Param wins over the `follow_redirects`
        // attribute; `max_redirects` caps `Follow`.
        let resolved_follow_redirects = follow_redirects.or(self.follow_redirects);
        let client_max_redirects = self.max_redirects;

        let future = async move {
            // Create request builder
            let mut request_builder = client.request(method, &resolved_url);

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
            if let Some(p) = &params {
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
                    Err(_) => {
                        tracing::warn!("primp: invalid characters in cookie header, skipping");
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
            if let Some((username, password)) = auth {
                request_builder = request_builder.basic_auth(username, password);
            } else if let Some(token) = auth_bearer {
                request_builder = request_builder.bearer_auth(token);
            }

            // Timeout
            if let Some(seconds) = resolved_timeout {
                request_builder = request_builder.timeout(crate::utils::timeout_duration(seconds)?);
            }

            // Per-request read timeout (falls back to client-level setting)
            if let Some(seconds) = resolved_read_timeout {
                request_builder =
                    request_builder.read_timeout(crate::utils::timeout_duration(seconds)?);
            }

            // Send the request and await the response
            let send_result = request_builder.send().await;
            // The redirect override is request-scoped (a per-request
            // extension), covering both success and error paths.
            let resp: PrimpResponse = send_result.map_err(Into::<PrimpErrorEnum>::into)?;
            let url: String = resp.url().to_string();
            let status_code = resp.status().as_u16();

            tracing::info!("response: {} {}", url, status_code);
            Ok::<(PrimpResponse, String, u16), PrimpErrorEnum>((resp, url, status_code))
        };

        // Convert Rust future to Python awaitable
        if stream {
            let py_future = async move {
                match future.await {
                    Ok((resp, url, status_code)) => {
                        let headers: IndexMapSSR = resp.headers().to_indexmap();
                        let cookies: IndexMapSSR = extract_cookies_to_indexmap(resp.headers());
                        let encoding = extract_encoding(resp.headers()).name().to_string();

                        Ok::<crate::r#async::response::AsyncResponse, BridgeTaskError>(
                            crate::r#async::response::AsyncResponse::new_streaming(
                                resp,
                                url,
                                status_code,
                                encoding,
                                headers,
                                cookies,
                            ),
                        )
                    }
                    Err(e) => Err(BridgeTaskError::Deferred(e)),
                }
            };
            future_into_coroutine(py, py_future)
        } else {
            let py_future = async move {
                match future.await {
                    Ok((resp, url, status_code)) => Ok(
                        crate::r#async::response::AsyncResponse::new(resp, url, status_code),
                    ),
                    Err(e) => Err(BridgeTaskError::Deferred(e)),
                }
            };
            future_into_coroutine(py, py_future)
        }
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn get<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
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

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn head<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
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

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn options<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
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

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn delete<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
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

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn post<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
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

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn put<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
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

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=false))]
    fn patch<'py>(
        &self,
        py: Python<'py>,
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
    ) -> PyResult<Bound<'py, PyAny>> {
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

    /// Support for async context manager protocol.
    fn __aenter__(slf: Py<Self>, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        use crate::r#async::bridge::{future_into_coroutine, BridgeTaskError};
        future_into_coroutine(py, async move { Ok::<_, BridgeTaskError>(slf) })
    }

    /// Exit the async context manager.
    fn __aexit__<'py>(
        &mut self,
        py: Python<'py>,
        _exc_type: Option<Bound<'_, PyAny>>,
        _exc_value: Option<Bound<'_, PyAny>>,
        _traceback: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        use crate::r#async::bridge::{future_into_coroutine, BridgeTaskError};
        future_into_coroutine(py, async move { Ok::<(), BridgeTaskError>(()) })
    }
}
