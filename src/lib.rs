#![allow(clippy::too_many_arguments)]
use std::str::FromStr;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use anyhow::{Error, Result};
use bytes::Bytes;
use foldhash::fast::RandomState;
use indexmap::IndexMap;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pythonize::depythonize;
use rquest::{
    header::{HeaderValue, COOKIE},
    multipart,
    redirect::Policy,
    tls::Impersonate,
    Body, Method,
};
use serde_json::Value;
use tokio::{
    fs::File,
    runtime::{self, Runtime},
};
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing;

mod response;
use response::Response;

mod traits;
use traits::{CookiesTraits, HeadersTraits};

mod utils;
use utils::load_ca_certs;

type IndexMapSSR = IndexMap<String, String, RandomState>;

// Tokio global one-thread runtime
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
});

#[pyclass]
/// HTTP client that can impersonate web browsers.
pub struct Client {
    client: Arc<Mutex<rquest::Client>>,
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
    /// * `cookies` - An optional map of cookies to send with requests as the `Cookie` header.
    /// * `cookie_store` - Enable a persistent cookie store. Received cookies will be preserved and included
    ///         in additional requests. Default is `true`.
    /// * `referer` - Enable or disable automatic setting of the `Referer` header. Default is `true`.
    /// * `proxy` - An optional proxy URL for HTTP requests.
    /// * `timeout` - An optional timeout for HTTP requests in seconds.
    /// * `impersonate` - An optional entity to impersonate. Supported browsers and versions include Chrome, Safari, OkHttp, and Edge.
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
    ///     cookies={"ck1": "cv1", "ck2": "cv2"},
    ///     cookie_store=False,
    ///     referer=False,
    ///     proxy="http://127.0.0.1:8080",
    ///     timeout=10,
    ///     impersonate="chrome_123",
    ///     follow_redirects=True,
    ///     max_redirects=1,
    ///     verify=True,
    ///     ca_cert_file="/cert/cacert.pem",
    ///     https_only=True,
    ///     http2_only=True,
    /// )
    /// ```
    #[new]
    #[pyo3(signature = (auth=None, auth_bearer=None, params=None, headers=None, cookies=None,
        cookie_store=true, referer=true, proxy=None, timeout=None, impersonate=None, follow_redirects=true,
        max_redirects=20, verify=true, ca_cert_file=None, https_only=false, http2_only=false))]
    fn new(
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        cookie_store: Option<bool>,
        referer: Option<bool>,
        proxy: Option<String>,
        timeout: Option<f64>,
        impersonate: Option<&str>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
        verify: Option<bool>,
        ca_cert_file: Option<String>,
        https_only: Option<bool>,
        http2_only: Option<bool>,
    ) -> Result<Self> {
        // Client builder
        let mut client_builder = rquest::Client::builder();

        // Impersonate
        if let Some(impersonation_type) = impersonate {
            let impersonation =
                Impersonate::from_str(impersonation_type).map_err(PyValueError::new_err)?;
            client_builder = client_builder.impersonate(impersonation);
        }

        // Headers || Cookies
        if headers.is_some() || cookies.is_some() {
            let headers = headers.unwrap_or_else(|| IndexMap::with_hasher(RandomState::default()));
            let mut headers_headermap = headers.to_headermap();
            if let Some(cookies) = cookies {
                let cookies_str = cookies.to_string();
                headers_headermap.insert(COOKIE, HeaderValue::from_str(&cookies_str)?);
            }
            client_builder = client_builder.default_headers(headers_headermap);
        };

        // Cookie_store
        if cookie_store.unwrap_or(true) {
            client_builder = client_builder.cookie_store(true);
        }

        // Referer
        if referer.unwrap_or(true) {
            client_builder = client_builder.referer(true);
        }

        // Proxy
        let proxy = proxy.or_else(|| std::env::var("PRIMP_PROXY").ok());
        if let Some(proxy) = &proxy {
            client_builder = client_builder.proxy(rquest::Proxy::all(proxy)?);
        }

        // Timeout
        if let Some(seconds) = timeout {
            client_builder = client_builder.timeout(Duration::from_secs_f64(seconds));
        }

        // Redirects
        if follow_redirects.unwrap_or(true) {
            client_builder = client_builder.redirect(Policy::limited(max_redirects.unwrap_or(20)));
            if proxy.is_some() {
                client_builder = client_builder.redirect_with_proxy_auth(true);
            }
        } else {
            client_builder = client_builder.redirect(Policy::none());
        }

        // Ca_cert_file. BEFORE!!! verify (fn load_ca_certs() reads env var PRIMP_CA_BUNDLE)
        if let Some(ca_bundle_path) = &ca_cert_file {
            std::env::set_var("PRIMP_CA_BUNDLE", ca_bundle_path);
        }

        // Verify
        if verify.unwrap_or(true) {
            client_builder = client_builder.root_certs_store(load_ca_certs);
        } else {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        // Https_only
        if let Some(true) = https_only {
            client_builder = client_builder.https_only(true);
        }

        // Http2_only
        if let Some(true) = http2_only {
            client_builder = client_builder.http2_only();
        }

        let client = Arc::new(Mutex::new(client_builder.build()?));

        Ok(Client {
            client,
            auth,
            auth_bearer,
            params,
            proxy,
            timeout,
        })
    }

    #[getter]
    pub fn get_headers(&self) -> Result<IndexMapSSR> {
        let mut client = self.client.lock().unwrap();
        let mut headers = client.headers_mut().clone();
        headers.remove(COOKIE);
        Ok(headers.to_indexmap())
    }

    #[setter]
    pub fn set_headers(&self, new_headers: Option<IndexMapSSR>) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        let headers = client.headers_mut();
        headers.clear();
        if let Some(new_headers) = new_headers {
            for (k, v) in new_headers {
                headers.insert_key_value(k, v)?
            }
        }
        Ok(())
    }

    #[getter]
    pub fn get_cookies(&self) -> Result<IndexMapSSR> {
        let mut client = self.client.lock().unwrap();
        let headers = client.headers_mut();
        let mut cookies: IndexMapSSR = IndexMap::with_hasher(RandomState::default());
        if let Some(cookie_header) = headers.get(COOKIE) {
            for part in cookie_header.to_str()?.split(';') {
                if let Some((key, value)) = part.trim().split_once('=') {
                    cookies.insert(key.to_string(), value.to_string());
                }
            }
        }
        Ok(cookies)
    }

    #[setter]
    pub fn set_cookies(&self, cookies: Option<IndexMapSSR>) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        let headers = client.headers_mut();
        if let Some(cookies) = cookies {
            headers.insert(COOKIE, HeaderValue::from_str(&cookies.to_string())?);
        }
        Ok(())
    }

    #[getter]
    pub fn get_proxy(&self) -> Result<Option<String>> {
        Ok(self.proxy.to_owned())
    }

    #[setter]
    pub fn set_proxy(&mut self, proxy: String) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        let rproxy = rquest::Proxy::all(proxy.clone())?;
        client.set_proxies(vec![rproxy]);
        self.proxy = Some(proxy);
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
        files: Option<IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> Result<Response> {
        let client = Arc::clone(&self.client);
        let method = Method::from_bytes(method.as_bytes())?;
        let is_post_put_patch = matches!(method, Method::POST | Method::PUT | Method::PATCH);
        let params = params.or_else(|| self.params.clone());
        let data_value: Option<Value> = data.map(depythonize).transpose()?;
        let json_value: Option<Value> = json.map(depythonize).transpose()?;
        let auth = auth.or(self.auth.clone());
        let auth_bearer = auth_bearer.or(self.auth_bearer.clone());
        let timeout: Option<f64> = timeout.or(self.timeout);

        let future = async {
            // Create request builder
            let mut request_builder = client.lock().unwrap().request(method, url);

            // Params
            if let Some(params) = params {
                request_builder = request_builder.query(&params);
            }

            // Headers
            if let Some(headers) = headers {
                request_builder = request_builder.headers(headers.to_headermap());
            }

            // Cookies
            if let Some(cookies) = cookies {
                request_builder =
                    request_builder.header(COOKIE, HeaderValue::from_str(&cookies.to_string())?);
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
                        let file = File::open(file_path).await?;
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
            let resp = request_builder.send().await?;

            // Response items
            let cookies: IndexMapSSR = resp
                .cookies()
                .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
                .collect();
            let headers: IndexMapSSR = resp.headers().to_indexmap();
            let status_code = resp.status().as_u16();
            let url = resp.url().to_string();
            let buf = resp.bytes().await?;

            tracing::info!("response: {} {} {}", url, status_code, buf.len());
            Ok((buf, cookies, headers, status_code, url))
        };

        // Execute an async future, releasing the Python GIL for concurrency.
        // Use Tokio global runtime to block on the future.
        let result: Result<(Bytes, IndexMapSSR, IndexMapSSR, u16, String), Error> =
            py.allow_threads(|| RUNTIME.block_on(future));
        let (f_buf, f_cookies, f_headers, f_status_code, f_url) = result?;

        Ok(Response {
            content: PyBytes::new(py, &f_buf).unbind(),
            cookies: f_cookies,
            encoding: String::new(),
            headers: f_headers,
            status_code: f_status_code,
            url: f_url,
        })
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None))]
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
    ) -> Result<Response> {
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None))]
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
    ) -> Result<Response> {
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None))]
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
    ) -> Result<Response> {
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None))]
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
    ) -> Result<Response> {
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None,
        json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
        files: Option<IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> Result<Response> {
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None,
        json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
        files: Option<IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> Result<Response> {
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None,
        json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
        files: Option<IndexMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> Result<Response> {
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
        )
    }
}

/// Convenience functions that use a default Client instance under the hood
#[pyfunction]
#[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None, data=None,
    json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, verify=None,
    ca_cert_file=None))]
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
    files: Option<IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None,
    timeout=None, impersonate=None, verify=None, ca_cert_file=None))]
fn get(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None,
    timeout=None, impersonate=None, verify=None, ca_cert_file=None))]
fn head(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None,
    timeout=None, impersonate=None, verify=None, ca_cert_file=None))]
fn options(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None,
    timeout=None, impersonate=None, verify=None, ca_cert_file=None))]
fn delete(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None,
    json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, verify=None,
    ca_cert_file=None))]
fn post(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None,
    json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, verify=None,
    ca_cert_file=None))]
fn put(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pyfunction]
#[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None,
    json=None, files=None, auth=None, auth_bearer=None, timeout=None, impersonate=None, verify=None,
    ca_cert_file=None))]
fn patch(
    py: Python,
    url: &str,
    params: Option<IndexMapSSR>,
    headers: Option<IndexMapSSR>,
    cookies: Option<IndexMapSSR>,
    content: Option<Vec<u8>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<IndexMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
    impersonate: Option<&str>,
    verify: Option<bool>,
    ca_cert_file: Option<String>,
) -> Result<Response> {
    let client = Client::new(
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        impersonate,
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
    )
}

#[pymodule]
fn primp(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    m.add_class::<Client>()?;
    m.add_function(wrap_pyfunction!(request, m)?)?;
    m.add_function(wrap_pyfunction!(get, m)?)?;
    m.add_function(wrap_pyfunction!(head, m)?)?;
    m.add_function(wrap_pyfunction!(options, m)?)?;
    m.add_function(wrap_pyfunction!(delete, m)?)?;
    m.add_function(wrap_pyfunction!(post, m)?)?;
    m.add_function(wrap_pyfunction!(patch, m)?)?;
    m.add_function(wrap_pyfunction!(put, m)?)?;
    Ok(())
}
