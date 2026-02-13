#![allow(clippy::too_many_arguments)]
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use pyo3::prelude::*;
use pythonize::depythonize;
use reqwest::{multipart, Body, Client, Method};
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
use error::{ErrorContext, PrimpError, PrimpResult};

mod impersonate;
mod response;
use response::Response;

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

#[pyclass(subclass)]
/// HTTP client that can impersonate web browsers.
pub struct RClient {
    client: Arc<RwLock<Client>>,
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
impl RClient {
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
            Client::builder(),
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

        Ok(RClient {
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
        let client = self.client.read().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
        Ok(headers_without_cookie(client.headers()))
    }

    #[setter]
    pub fn set_headers(&self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        let mut client = self.client.write().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
        let headers = client.headers_mut();
        headers.clear();
        if let Some(new_headers) = new_headers {
            for (k, v) in new_headers {
                headers.insert_key_value(k, v);
            }
        }
        Ok(())
    }

    pub fn headers_update(&self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
        let mut client = self.client.write().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
        let headers = client.headers_mut();
        if let Some(new_headers) = new_headers {
            for (k, v) in new_headers {
                headers.insert_key_value(k, v);
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
        let rproxy = reqwest::Proxy::all(proxy.clone())?;
        let mut client = self.client.write().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
        let client_ref = &mut *client;
        client_ref.set_proxies(vec![rproxy]);
        self.proxy = Some(proxy);
        Ok(())
    }

    #[pyo3(signature = (url))]
    fn get_cookies(&self, url: &str) -> PrimpResult<IndexMapSSR> {
        let url = reqwest::Url::parse(url).map_err(|e| PrimpError::InvalidURL(e.to_string()))?;
        let client = self.client.read().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
        let cookie = client.get_cookies(&url).ok_or_else(|| PrimpError::Custom("No cookies found for URL".to_string()))?;
        let cookie_str = cookie.to_str()?;
        Ok(parse_cookies_from_header(cookie_str))
    }

    #[pyo3(signature = (url, cookies))]
    fn set_cookies(&self, url: &str, cookies: Option<IndexMapSSR>) -> PrimpResult<()> {
        let url = reqwest::Url::parse(url).map_err(|e| PrimpError::InvalidURL(e.to_string()))?;
        if let Some(cookies) = cookies {
            let header_values = cookies_to_header_values(&cookies);
            let client = self.client.read().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
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
    ///
    /// # Returns
    ///
    /// * `Response` - A response object containing the server's response to the request.
    ///
    /// # Errors
    ///
    /// * `PyException` - If there is an error making the request.
    #[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None,
        data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
    ) -> PyResult<Response> {
        let client = Arc::clone(&self.client);
        let method = Method::from_bytes(method.as_bytes()).map_err(Into::<PrimpError>::into)?;
        let is_post_put_patch = matches!(method, Method::POST | Method::PUT | Method::PATCH);
        let params = params.or_else(|| self.params.clone());
        let data_value: Option<Value> = data
            .map(depythonize)
            .transpose()
            .map_err(Into::<PrimpError>::into)?;
        let json_value: Option<Value> = json
            .map(depythonize)
            .transpose()
            .map_err(Into::<PrimpError>::into)?;
        let auth = auth.or(self.auth.clone());
        let auth_bearer = auth_bearer.or(self.auth_bearer.clone());
        let timeout: Option<f64> = timeout.or(self.timeout);

        // Build error context for enriched error messages
        let error_context = ErrorContext::new()
            .with_method(method.as_str())
            .with_url(url);
        let error_context = if let Some(t) = timeout {
            error_context.with_timeout((t * 1000.0) as u64)
        } else {
            error_context
        };

        // Cookies
        if let Some(cookies) = cookies {
            let url = reqwest::Url::parse(url).map_err(Into::<PrimpError>::into)?;
            let cookie_values = cookies_to_header_values(&cookies);
            let client = client.read().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
            client.set_cookies(&url, cookie_values);
        }

        let future = async {
            // Create request builder
            let mut request_builder = client.read().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?.request(method, url);

            // Params
            if let Some(params) = params {
                request_builder = request_builder.query(&params);
            }

            // Headers
            if let Some(headers) = headers {
                request_builder = request_builder.headers(headers.to_headermap());
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
                            .map_err(Into::<PrimpError>::into)?;
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
            let resp: reqwest::Response = request_builder
                .send()
                .await
                .map_err(|e| PrimpError::from_reqwest(e, Some(error_context.clone())))?;
            let url: String = resp.url().to_string();
            let status_code = resp.status().as_u16();

            // Check for HTTP error status codes (4xx/5xx)
            if status_code >= 400 {
                let reason = resp.status().canonical_reason().unwrap_or("Unknown");
                return Err(PrimpError::HttpStatus(status_code, reason.to_string(), url));
            }

            tracing::info!("response: {} {}", url, status_code);
            Ok((resp, url, status_code))
        };

        // Execute an async future, releasing the Python GIL for concurrency.
        // Use Tokio global runtime to block on the future.
        let response: Result<(reqwest::Response, String, u16), PrimpError> =
            py.detach(|| RUNTIME.block_on(future));
        let result = response?;
        let resp = Some(result.0);
        let url = result.1;
        let status_code = result.2;
        Ok(Response {
            resp,
            _content: None,
            _encoding: None,
            _headers: None,
            _cookies: None,
            url,
            status_code,
        })
    }
}

#[pymodule]
fn primp(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    // Re-export exception types from error module
    use error::{
        BodyError, ChunkedEncodingError, ConnectTimeout, ConnectionError, ContentDecodingError,
        DecodeError, HTTPError, InvalidHeader, InvalidJSONError, InvalidURL, JSONDecodeError,
        JSONError, ProxyError, ReadTimeout, RequestError, RequestException, SSLError,
        StreamConsumedError, Timeout, TooManyRedirects,
    };

    // Add exception types
    // PrimpError is an alias for RequestException
    m.add("RequestException", _py.get_type::<RequestException>())?;
    m.add("PrimpError", _py.get_type::<RequestException>())?;
    m.add("HTTPError", _py.get_type::<HTTPError>())?;
    m.add("ConnectionError", _py.get_type::<ConnectionError>())?;
    m.add("Timeout", _py.get_type::<Timeout>())?;
    m.add("TooManyRedirects", _py.get_type::<TooManyRedirects>())?;
    m.add("RequestError", _py.get_type::<RequestError>())?;

    // New parent exceptions
    m.add("BodyError", _py.get_type::<BodyError>())?;
    m.add("DecodeError", _py.get_type::<DecodeError>())?;
    m.add("JSONError", _py.get_type::<JSONError>())?;

    // Body-related exceptions
    m.add("StreamConsumedError", _py.get_type::<StreamConsumedError>())?;
    m.add(
        "ChunkedEncodingError",
        _py.get_type::<ChunkedEncodingError>(),
    )?;

    // Decode-related exceptions
    m.add(
        "ContentDecodingError",
        _py.get_type::<ContentDecodingError>(),
    )?;

    // JSON-related exceptions
    m.add("InvalidJSONError", _py.get_type::<InvalidJSONError>())?;
    m.add("JSONDecodeError", _py.get_type::<JSONDecodeError>())?;

    // Connection-related exceptions
    m.add("ProxyError", _py.get_type::<ProxyError>())?;
    m.add("SSLError", _py.get_type::<SSLError>())?;
    m.add("ConnectTimeout", _py.get_type::<ConnectTimeout>())?;

    // Timeout-related exceptions
    m.add("ReadTimeout", _py.get_type::<ReadTimeout>())?;

    // URL and Header exceptions
    m.add("InvalidURL", _py.get_type::<InvalidURL>())?;
    m.add("InvalidHeader", _py.get_type::<InvalidHeader>())?;

    m.add_class::<RClient>()?;
    m.add_class::<r#async::RAsyncClient>()?;
    m.add_class::<r#async::AsyncResponse>()?;
    m.add_class::<r#async::AsyncResponseStream>()?;
    Ok(())
}
