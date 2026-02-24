#![allow(clippy::too_many_arguments)]
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use ::primp::{
    header, multipart, Body, Client as PrimpClient, Method, Proxy, Response as PrimpResponse, Url,
};
use encoding_rs::UTF_8;
use mime::Mime;
use pyo3::prelude::*;
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
    configure_client_builder, cookies_to_header_values, headers_without_cookie,
    parse_cookies_from_header, IndexMapSSR,
};

mod error;
use error::{PrimpErrorEnum, PrimpResult};

mod impersonate;
mod response;
use response::{BytesIterator, LinesIterator, Response, StreamResponse, TextIterator};

mod r#async;

mod traits;
use traits::{HeaderMapExt, HeadersTraits};

mod utils;

// Tokio global one-thread runtime
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime")
});

/// Extract encoding from Content-Type header.
///
/// Returns the encoding specified in the charset parameter, or UTF-8 as fallback.
fn extract_encoding(headers: &header::HeaderMap) -> &'static encoding_rs::Encoding {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<Mime>().ok())
        .and_then(|mime| mime.get_param("charset").map(|c| c.as_str().to_string()))
        .and_then(|name| encoding_rs::Encoding::for_label(name.as_bytes()))
        .unwrap_or(UTF_8)
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
    #[pyo3(get, set)]
    proxy: Option<String>,
    #[pyo3(get, set)]
    timeout: Option<f64>,
    #[pyo3(get)]
    impersonate: Option<String>,
    #[pyo3(get)]
    impersonate_os: Option<String>,
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
    /// * `timeout` - An optional timeout for HTTP requests in seconds.
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
        referer=true, proxy=None, timeout=None, impersonate=None, impersonate_os=None, follow_redirects=true,
        max_redirects=20, verify=true, ca_cert_file=None, https_only=false, http2_only=false))]
    fn new(
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookie_store: Option<bool>,
        referer: Option<bool>,
        proxy: Option<String>,
        timeout: Option<f64>,
        impersonate: Option<String>,
        impersonate_os: Option<String>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
        verify: Option<bool>,
        ca_cert_file: Option<String>,
        https_only: Option<bool>,
        http2_only: Option<bool>,
    ) -> PrimpResult<Self> {
        let (client_builder, resolved_proxy) = configure_client_builder(
            PrimpClient::builder(),
            headers,
            cookie_store,
            referer,
            proxy,
            timeout,
            impersonate.clone(),
            impersonate_os.clone(),
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
            impersonate,
            impersonate_os,
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
        let url = Url::parse(url).map_err(|e| PrimpErrorEnum::InvalidURL(e.to_string()))?;
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
    /// * `timeout` - The timeout for the request in seconds. Default is 30.
    /// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
    ///
    /// # Returns
    ///
    /// * `Response` - A response object containing the server's response to the request.
    ///
    /// # Errors
    ///
    /// * `PyException` - If there is an error making the request.
    #[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None,
        data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
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
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        let client = Arc::clone(&self.client);
        let method = Method::from_bytes(method.as_bytes()).map_err(Into::<PrimpErrorEnum>::into)?;
        let is_post_put_patch = matches!(method, Method::POST | Method::PUT | Method::PATCH);
        let params = params.or_else(|| self.params.clone());
        let data_value: Option<Value> = data
            .map(depythonize)
            .transpose()
            .map_err(Into::<PrimpErrorEnum>::into)?;
        let json_value: Option<Value> = json
            .map(depythonize)
            .transpose()
            .map_err(Into::<PrimpErrorEnum>::into)?;
        let auth = auth.or(self.auth.clone());
        let auth_bearer = auth_bearer.or(self.auth_bearer.clone());
        let timeout: Option<f64> = timeout.or(self.timeout);

        // Cookies
        if let Some(cookies) = cookies {
            let url = Url::parse(url).map_err(Into::<PrimpErrorEnum>::into)?;
            let cookie_values = cookies_to_header_values(&cookies);
            let client = client.read().expect("client lock was poisoned");
            client.set_cookies(&url, cookie_values);
        }

        let future = async {
            // Create request builder
            let mut request_builder = client
                .read()
                .expect("client lock was poisoned")
                .request(method, url);

            // Params
            if let Some(params) = params {
                request_builder = request_builder.query(&params);
            }

            // Headers
            if let Some(headers) = headers {
                request_builder = request_builder.headers(headers.to_headermap()?);
            }

            // Only if method POST || PUT || PATCH
            if is_post_put_patch {
                // Content
                if let Some(content) = content {
                    request_builder = request_builder.body(content);
                }
                // Data
                if let Some(form_data) = data_value {
                    request_builder = request_builder.form(&form_data);
                }
                // Json
                if let Some(json_data) = json_value {
                    request_builder = request_builder.json(&json_data);
                }
                // Files
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
            }

            // Auth
            if let Some((username, password)) = auth {
                request_builder = request_builder.basic_auth(username, password);
            } else if let Some(token) = auth_bearer {
                request_builder = request_builder.bearer_auth(token);
            }

            // Timeout
            if let Some(seconds) = timeout {
                request_builder = request_builder.timeout(Duration::from_secs_f64(seconds));
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
        let result = response?;
        let resp = result.0;
        let url = result.1;
        let status_code = result.2;

        if stream {
            // Return StreamResponse for streaming
            let headers = PyDict::new(py);
            for (key, value) in resp.headers() {
                headers.set_item(key.as_str(), value.to_str().unwrap_or(""))?;
            }

            let cookies = PyDict::new(py);
            let set_cookie_header = resp.headers().get_all(header::SET_COOKIE);
            for cookie_header in set_cookie_header.iter() {
                if let Ok(cookie_str) = cookie_header.to_str() {
                    if let Some((name, value)) = cookie_str.split_once('=') {
                        cookies
                            .set_item(name.trim(), value.split(';').next().unwrap_or("").trim())?;
                    }
                }
            }

            let encoding = extract_encoding(resp.headers()).name().to_string();

            let stream_response = StreamResponse::new(
                resp,
                url,
                status_code,
                encoding,
                headers.unbind(),
                cookies.unbind(),
            );
            Ok(stream_response.into_pyobject(py)?.into_any().unbind())
        } else {
            // Return regular Response
            Ok(Response {
                resp: Some(resp),
                _content: None,
                _encoding: None,
                _headers: None,
                _cookies: None,
                url,
                status_code,
            }
            .into_pyobject(py)?
            .into_any()
            .unbind())
        }
    }

    /// Send a GET request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
    fn get(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "GET",
            url,
            params,
            headers,
            cookies,
            None,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
            stream,
        )
    }

    /// Send a HEAD request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
    fn head(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "HEAD",
            url,
            params,
            headers,
            cookies,
            None,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
            stream,
        )
    }

    /// Send an OPTIONS request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
    fn options(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "OPTIONS",
            url,
            params,
            headers,
            cookies,
            None,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
            stream,
        )
    }

    /// Send a DELETE request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
    fn delete(
        &self,
        py: Python,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
        stream: bool,
    ) -> PyResult<Py<PyAny>> {
        self.request(
            py,
            "DELETE",
            url,
            params,
            headers,
            cookies,
            None,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
            stream,
        )
    }

    /// Send a POST request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
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
            stream,
        )
    }

    /// Send a PUT request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
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
            stream,
        )
    }

    /// Send a PATCH request.
    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, stream=false))]
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
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
fn get(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
    )?;
    client.get(
        py,
        url,
        params,
        headers,
        cookies,
        auth,
        auth_bearer,
        timeout,
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
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
fn head(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
    )?;
    client.head(
        py,
        url,
        params,
        headers,
        cookies,
        auth,
        auth_bearer,
        timeout,
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
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
fn options(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
    )?;
    client.options(
        py,
        url,
        params,
        headers,
        cookies,
        auth,
        auth_bearer,
        timeout,
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
/// * `auth` - A tuple containing the username and an optional password for basic authentication. Default is None.
/// * `auth_bearer` - A string representing the bearer token for bearer token authentication. Default is None.
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
fn delete(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
        None,
        None,
    )?;
    client.delete(
        py,
        url,
        params,
        headers,
        cookies,
        auth,
        auth_bearer,
        timeout,
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
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
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
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
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
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
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
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
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
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
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
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
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
/// * `timeout` - The timeout for the request in seconds. Default is 30.
/// * `impersonate` - An optional entity to impersonate. Default is None.
/// * `impersonate_os` - An optional OS to impersonate. Default is None.
/// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is true.
/// * `ca_cert_file` - Path to CA certificate store. Default is None.
/// * `stream` - If True, returns a StreamResponse for streaming the response body. Default is False.
#[pyfunction]
#[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, impersonate_os=None, verify=true, ca_cert_file=None, stream=false))]
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
    impersonate: Option<String>,
    impersonate_os: Option<String>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
    stream: bool,
) -> PyResult<Py<PyAny>> {
    let client = Client::new(
        None,
        None,
        None,
        headers.clone(),
        None,
        None,
        None,
        timeout,
        impersonate,
        impersonate_os,
        None,
        None,
        verify,
        ca_cert_file,
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
        stream,
    )
}

#[pymodule]
fn primp(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

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
    m.add_class::<StreamResponse>()?;
    m.add_class::<BytesIterator>()?;
    m.add_class::<TextIterator>()?;
    m.add_class::<LinesIterator>()?;

    // Async classes
    m.add_class::<Client>()?;
    m.add_class::<r#async::AsyncClient>()?;
    m.add_class::<r#async::AsyncResponse>()?;
    m.add_class::<r#async::AsyncStreamResponse>()?;
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

    Ok(())
}
