#![allow(clippy::too_many_arguments)]
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use ::primp::{
    multipart, Body, Client as PrimpClient, Method, Proxy, Response as PrimpResponse, Url,
};
use pyo3::prelude::*;
use pythonize::depythonize;
use serde_json::Value;
use tokio::{
    fs::File,
    runtime::{self, Runtime},
};
use tokio_util::codec::{BytesCodec, FramedRead};

mod client_builder;
use client_builder::{
    configure_client_builder, cookies_to_header_values, headers_without_cookie,
    parse_cookies_from_header, parse_url_or_domain, IndexMapSSR,
};

mod error;
use error::{PrimpErrorEnum, PrimpResult};

mod impersonate;
mod response;
use response::{BytesIterator, LinesIterator, Response, TextIterator};

mod r#async;

mod traits;
use traits::{HeaderMapExt, HeadersTraits};

mod utils;
use utils::extract_encoding;

// Tokio global one-thread runtime
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime")
});

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
    #[pyo3(get, set)]
    proxy: Option<String>,
    #[pyo3(get, set)]
    timeout: Option<f64>,
    #[pyo3(get, set)]
    connect_timeout: Option<f64>,
    #[pyo3(get, set)]
    read_timeout: Option<f64>,
    #[pyo3(get)]
    impersonate: Option<String>,
    #[pyo3(get)]
    impersonate_os: Option<String>,
    #[pyo3(get, set)]
    base_url: Option<String>,
    cookies: Option<IndexMapSSR>,
}

pub fn extract_cookies_to_indexmap(headers: &http::HeaderMap) -> IndexMapSSR {
    let mut cookie_map = IndexMapSSR::default();
    for cookie_header in headers.get_all(http::header::SET_COOKIE).iter() {
        if let Ok(cookie_str) = cookie_header.to_str() {
            if let Some((name, value)) = cookie_str.split_once('=') {
                cookie_map.insert(
                    name.trim().to_string(),
                    value.split(';').next().unwrap_or("").trim().to_string(),
                );
            }
        }
    }
    cookie_map
}

#[pymethods]
impl Client {
    /// Initializes an HTTP client that can impersonate web browsers.
    ///
    /// This function creates a new HTTP client instance that can impersonate various web browsers.
    /// It allows for customization of headers, proxy settings, timeout, impersonation type, SSL certificate verification,
    /// and HTTP version preferences.
    ///
    /// # Arguments
    ///
    /// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
    /// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
    /// * `params` - A map of query parameters to append to the URL. Default is None.
    /// * `headers` - An optional map of HTTP headers to send with requests. If `impersonate` is set, this will be ignored.
    /// * `cookie_store` - Enable a persistent cookie store. Received cookies will be preserved and included
    ///         in additional requests. Default is `true`.
    /// * `referer` - Enable or disable automatic setting of the `Referer` header. Default is `true`.
    /// * `proxy` - An optional proxy URL for HTTP requests.
    /// * `timeout` - An optional total timeout for HTTP requests in seconds.
    /// * `connect_timeout` - An optional timeout for establishing connections in seconds.
    /// * `read_timeout` - An optional timeout for reading the response body in seconds.
    /// * `impersonate` - An optional entity to impersonate. Supported browsers and versions include Chrome, Safari, Edge.
    /// * `impersonate_os` - An optional entity to impersonate OS. Supported OS: android, ios, linux, macos, windows.
    /// * `follow_redirects` - A boolean to enable or disable following redirects. Default is `true`.
    /// * `max_redirects` - The maximum number of redirects to follow. Default is 20. Applies if `follow_redirects` is `true`.
    /// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is `true`.
    /// * `ca_cert_file` - Path to CA certificate store. Default is None.
    /// * `https_only` - Restrict the Client to be used with HTTPS only requests. Default is `false`.
    /// * `http2_only` - If true - use only HTTP/2, if false - use only HTTP/1. Default is `false`.
    ///
    /// # Example
    ///
    /// ```
    /// from primp import Client
    ///
    /// client = Client(
    ///     auth=("name", "password"),
    ///     params={"p1k": "p1v", "p2k": "p2v"},
    ///     headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/88.0.4324.150 Safari/537.36"},
    ///     cookie_store=False,
    ///     referer=False,
    ///     proxy="http://127.0.0.1:8080",
    ///     timeout=10,
    ///     impersonate="chrome_144",
    ///     impersonate_os="windows",
    ///     follow_redirects=True,
    ///     max_redirects=1,
    ///     verify=True,
    ///     ca_cert_file="/cert/cacert.pem",
    ///     https_only=True,
    ///     http2_only=True,
    /// )
    /// ```
    #[new]
    #[pyo3(signature = (auth=None, auth_bearer=None, params=None, headers=None, cookie_store=true,
        referer=true, proxy=None, timeout=None, connect_timeout=None, read_timeout=None,
        impersonate=None, impersonate_os=None, follow_redirects=true,
        max_redirects=20, verify=true, ca_cert_file=None, https_only=false, http2_only=false,
        base_url=None, cookies=None))]
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
        impersonate: Option<String>,
        impersonate_os: Option<String>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
        verify: Option<bool>,
        ca_cert_file: Option<String>,
        https_only: Option<bool>,
        http2_only: Option<bool>,
        base_url: Option<String>,
        cookies: Option<IndexMapSSR>,
    ) -> PrimpResult<Self> {
        // Release the GIL while building the reqwest client. The build path
        // runs TLS / root-CA / impersonation initialization which emits
        // `log::*!` / `tracing::*!` callsites inside primp-reqwest, hyper,
        // and rustls. Those forward through pyo3-log and need to acquire
        // the GIL to call `logging.getLogger`. If we held the GIL here,
        // concurrent threads constructing a Client would deadlock against
        // Python's `logging._lock` / GIL ordering — classic pyo3-log
        // pattern documented in vorner/pyo3-log#65, observed downstream in
        // deedy5/ddgs#452, HKUDS/nanobot#2804, microsoft/amplifier#219.
        //
        // Everything inside `allow_threads` is pure Rust; no PyObject is
        // touched, so releasing the GIL is safe and enforced at compile
        // time (the closure is `Send`).
        py.detach(|| {
            let (client_builder, resolved_proxy) = configure_client_builder(
                PrimpClient::builder(),
                headers,
                cookie_store,
                referer,
                proxy,
                timeout,
                connect_timeout,
                read_timeout,
                impersonate.as_deref(),
                impersonate_os.as_deref(),
                follow_redirects,
                max_redirects,
                verify,
                ca_cert_file,
                https_only,
                http2_only,
            )?;

            let client = Arc::new(RwLock::new(client_builder.build()?));

            Ok(Client {
                client,
                auth,
                auth_bearer,
                params,
                proxy: resolved_proxy,
                timeout,
                connect_timeout,
                read_timeout,
                impersonate,
                impersonate_os,
                base_url,
                cookies,
            })
        })
    }

    #[getter]
    pub fn get_headers(&self) -> PrimpResult<IndexMapSSR> {
        let client = self.client.read().expect("client lock was poisoned");
        Ok(headers_without_cookie(client.headers()))
    }

    #[setter]
    pub fn set_headers(&self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        let mut client = self.client.write().expect("client lock was poisoned");
        let headers = client.headers_mut();
        headers.clear();
        if let Some(new_headers) = new_headers {
            for (k, v) in new_headers {
                headers.insert_key_value(k, v)?;
            }
        }
        Ok(())
    }

    pub fn headers_update(&self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        let mut client = self.client.write().expect("client lock was poisoned");
        let headers = client.headers_mut();
        if let Some(new_headers) = new_headers {
            for (k, v) in new_headers {
                headers.insert_key_value(k, v)?;
            }
        }
        Ok(())
    }

    #[getter]
    pub fn get_proxy(&self) -> PrimpResult<Option<String>> {
        Ok(self.proxy.to_owned())
    }

    #[setter]
    pub fn set_proxy(&mut self, proxy: String) -> PrimpResult<()> {
        let rproxy = Proxy::all(proxy.clone())?;
        let mut client = self.client.write().expect("client lock was poisoned");
        let client_ref = &mut *client;
        client_ref.set_proxies(vec![rproxy]);
        self.proxy = Some(proxy);
        Ok(())
    }

    #[pyo3(signature = (url))]
    fn get_cookies(&self, url: &str) -> PrimpResult<IndexMapSSR> {
        let url = Url::parse(url).map_err(|e| PrimpErrorEnum::InvalidURL(e.to_string()))?;
        let client = self.client.read().expect("client lock was poisoned");
        let cookie = client
            .get_cookies(&url)
            .ok_or_else(|| PrimpErrorEnum::Custom("No cookies found for URL".to_string()))?;
        let cookie_str = cookie.to_str()?;
        Ok(parse_cookies_from_header(cookie_str))
    }

    #[pyo3(signature = (url, cookies))]
    fn set_cookies(&self, url: &str, cookies: Option<IndexMapSSR>) -> PrimpResult<()> {
        let url =
            parse_url_or_domain(url).map_err(|e| PrimpErrorEnum::InvalidURL(e.to_string()))?;
        if let Some(cookies) = cookies {
            let header_values = cookies_to_header_values(&cookies);
            let client = self.client.read().expect("client lock was poisoned");
            client.set_cookies(&url, header_values);
        }
        Ok(())
    }

    /// Constructs an HTTP request with the given method, URL, and optionally sets a timeout, headers, and query parameters.
    /// Sends the request and returns a `Response` object containing the server's response.
    ///
    /// # Arguments
    ///
    /// * `method` - The HTTP method to use (e.g., "GET", "POST").
    /// * `url` - The URL to which the request will be made.
    /// * `params` - A map of query parameters to append to the URL. Default is None.
    /// * `headers` - A map of HTTP headers to send with the request. Default is None.
    /// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
    /// * `content` - The content to send in the request body as bytes. Default is None.
    /// * `data` - The form data to send in the request body. Default is None.
    /// * `json` -  A JSON serializable object to send in the request body. Default is None.
    /// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
    /// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
    /// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
    /// * `timeout` - The timeout for the request in seconds. Default is None.
    /// * `read_timeout` - The read timeout in seconds (max gap between bytes). Default is None.
    /// * `follow_redirects` - Whether to follow redirects for this request. Default is None (uses client setting).
    /// * `stream` - If True, returns a Response for streaming the response body. Default is False.
    ///
    /// # Returns
    ///
    /// * `Response` - A response object containing the server's response to the request.
    ///
    /// # Errors
    ///
    /// * `PyException` - If there is an error making the request.
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

        // Apply client-level cookies
        if let Some(ref client_cookies) = self.cookies {
            if !client_cookies.is_empty() {
                let url_parsed = Url::parse(&resolved_url).map_err(Into::<PrimpErrorEnum>::into)?;
                let cookie_values = cookies_to_header_values(client_cookies);
                let client_guard = client.read().expect("client lock was poisoned");
                client_guard.set_cookies(&url_parsed, cookie_values);
            }
        }

        // Request-level cookies
        if let Some(cookies) = cookies.filter(|c| !c.is_empty()) {
            let url_parsed = Url::parse(&resolved_url).map_err(Into::<PrimpErrorEnum>::into)?;
            let cookie_values = cookies_to_header_values(&cookies);
            let client_guard = client.read().expect("client lock was poisoned");
            client_guard.set_cookies(&url_parsed, cookie_values);
        }

        // Handle follow_redirects: set policy before cloning client
        if let Some(fr) = follow_redirects {
            let mut client_guard = client.write().expect("client lock was poisoned");
            if fr {
                client_guard.set_redirect_policy(::primp::redirect::Policy::limited(20));
            } else {
                client_guard.set_redirect_policy(::primp::redirect::Policy::none());
            }
        }

        // Clone the inner client to avoid holding the RwLock across await points
        let client_clone = client.read().expect("client lock was poisoned").clone();

        let self_params = params
            .as_ref()
            .is_none()
            .then_some(self.params.as_ref())
            .flatten();
        let self_auth = auth
            .as_ref()
            .is_none()
            .then_some(self.auth.as_ref())
            .flatten();
        let self_auth_bearer = auth_bearer
            .as_ref()
            .is_none()
            .then_some(self.auth_bearer.as_ref())
            .flatten();

        let future = async move {
            // Create request builder using the cloned client
            let mut request_builder = client_clone.request(method, &resolved_url);

            // Params
            match (&params, self_params) {
                (Some(p), _) => {
                    request_builder = request_builder.query(p);
                }
                (None, Some(sp)) => {
                    request_builder = request_builder.query(sp);
                }
                (None, None) => {}
            }

            // Headers
            if let Some(headers) = headers {
                request_builder = request_builder.headers(headers.to_headermap()?);
            }

            // Body content (if provided)
            if let Some(content) = content {
                request_builder = request_builder.body(content);
            }
            // Form data (if provided)
            if let Some(form_data) = data_value {
                request_builder = request_builder.form(&form_data);
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
            match (&auth, self_auth) {
                (Some((u, p)), _) => {
                    request_builder = request_builder.basic_auth(u, p.as_deref());
                }
                (None, Some((u, p))) => {
                    request_builder = request_builder.basic_auth(u, p.as_deref());
                }
                (None, None) => {
                    // Try bearer auth if no basic auth
                    match (&auth_bearer, self_auth_bearer) {
                        (Some(t), _) => {
                            request_builder = request_builder.bearer_auth(t);
                        }
                        (None, Some(t)) => {
                            request_builder = request_builder.bearer_auth(t);
                        }
                        (None, None) => {}
                    }
                }
            }

            // Timeout
            if let Some(seconds) = resolved_timeout {
                request_builder = request_builder.timeout(Duration::from_secs_f64(seconds));
            }

            // Per-request read timeout
            if let Some(seconds) = read_timeout {
                request_builder = request_builder.read_timeout(Duration::from_secs_f64(seconds));
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
        // Use Tokio global runtime to block on the future.
        let response: Result<(PrimpResponse, String, u16), PrimpErrorEnum> =
            py.detach(|| RUNTIME.block_on(future));

        // Restore redirect policy if it was changed
        if follow_redirects.is_some() {
            let mut client_guard = client.write().expect("client lock was poisoned");
            client_guard.set_redirect_policy(::primp::redirect::Policy::limited(20));
        }

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

/// Send a GET request with a temporary client.
///
/// # Arguments
///
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` -  A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.get(
        py,
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

/// Send a HEAD request with a temporary client.
///
/// # Arguments
///
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` -  A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.head(
        py,
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

/// Send an OPTIONS request with a temporary client.
///
/// # Arguments
///
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` -  A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.options(
        py,
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

/// Send a DELETE request with a temporary client.
///
/// # Arguments
///
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` -  A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.delete(
        py,
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

/// Send a POST request with a temporary client.
///
/// # Arguments
///
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` - A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.post(
        py,
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

/// Send a PUT request with a temporary client.
///
/// # Arguments
///
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` - A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.put(
        py,
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

/// Send a PATCH request with a temporary client.
///
/// # Arguments
///
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` - A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.patch(
        py,
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

/// Send a request with a custom method using a temporary client.
///
/// # Arguments
///
/// * `method` - The HTTP method to use (e.g., "GET", "POST").
/// * `url` - The URL to which the request will be made.
/// * `params` - A map of query parameters to append to the URL. Default is None.
/// * `headers` - A map of HTTP headers to send with the request. Default is None.
/// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
/// * `content` - The content to send in the request body as bytes. Default is None.
/// * `data` - The form data to send in the request body. Default is None.
/// * `json` - A JSON serializable object to send in the request body. Default is None.
/// * `files` - A map of file fields to file paths to be sent as multipart/form-data. Default is None.
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The total timeout for the request in seconds. Default is None.
/// * `connect_timeout` - The timeout for establishing a connection in seconds. Default is None.
/// * `read_timeout` - The timeout for reading the response body in seconds. Default is None.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a Response for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None,     timeout=None, connect_timeout=None, read_timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, follow_redirects=None, stream=false))]
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
        headers.clone(),
        None,
        None,
        None,
        timeout,
        connect_timeout,
        None,
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
    )?;
    client.request(
        py,
        method,
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

#[pymodule]
fn primp(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Install pyo3-log with per-target `(logger, level)` caching so
    // tracing/log callsites inside primp (and its transitive deps reqwest /
    // hyper / rustls) only call back into Python's logging module on the
    // first emission per target. Subsequent emissions hit a Rust-side cache
    // and never cross the GIL boundary.
    //
    // This complements — but does not replace — the `py.detach` fix
    // applied to `Client::new` and `AsyncClient::new`. The `allow_threads`
    // release is what actually prevents the first-call race from deadlocking
    // against `logging._lock`; caching here is documented pyo3-log best
    // practice and keeps steady-state hot paths GIL-free for logging.
    match pyo3_log::Logger::new(_py, pyo3_log::Caching::LoggersAndLevels) {
        Ok(logger) => {
            let _ = logger.install();
        }
        Err(_) => {
            // Fall back to the uncached default if LoggersAndLevels is
            // unavailable for some reason; allow_threads still keeps us
            // deadlock-free.
            pyo3_log::init();
        }
    }

    // Re-export exception types from error module - new hierarchy
    use error::{
        BodyError, BuilderError, ConnectError, DecodeError, PrimpError, RedirectError,
        RequestError, StatusError, TimeoutError, UpgradeError,
    };

    // Add exception types - new primp-reqwest native hierarchy
    // Base exception
    m.add("PrimpError", _py.get_type::<PrimpError>())?;

    // Builder errors
    m.add("BuilderError", _py.get_type::<BuilderError>())?;

    // Request errors
    m.add("RequestError", _py.get_type::<RequestError>())?;
    m.add("ConnectError", _py.get_type::<ConnectError>())?;
    m.add("TimeoutError", _py.get_type::<TimeoutError>())?;

    // Other errors
    m.add("StatusError", _py.get_type::<StatusError>())?;
    m.add("RedirectError", _py.get_type::<RedirectError>())?;
    m.add("BodyError", _py.get_type::<BodyError>())?;
    m.add("DecodeError", _py.get_type::<DecodeError>())?;
    m.add("UpgradeError", _py.get_type::<UpgradeError>())?;

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
