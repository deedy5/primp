use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use pyo3::exceptions;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::{PyDict, PyString};
use reqwest_impersonate::blocking::multipart;
use reqwest_impersonate::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest_impersonate::impersonate::Impersonate;
use reqwest_impersonate::redirect::Policy;
use reqwest_impersonate::Method;

mod response;
use response::Response;

#[pyclass]
/// HTTP client that can impersonate web browsers.
struct Client {
    client: reqwest_impersonate::blocking::Client,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    params: Option<HashMap<String, String>>,
}

#[pymethods]
impl Client {
    #[new]
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
    /// * `impersonate` - An optional entity to impersonate. Supported browsers and versions include Chrome, Safari, OkHttp, and Edge.
    /// * `follow_redirects` - A boolean to enable or disable following redirects. Default is `true`.
    /// * `max_redirects` - The maximum number of redirects to follow. Default is 20. Applies if `follow_redirects` is `true`.
    /// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is `false`.
    /// * `http1` - An optional boolean indicating whether to use only HTTP/1.1. Default is `false`.
    /// * `http2` - An optional boolean indicating whether to use only HTTP/2. Default is `false`.
    ///
    /// # Example
    ///
    /// ```
    /// from reqwest_impersonate import Client
    ///
    /// client = Client(
    ///     auth=("name", "password"),
    ///     params={"p1k": "p1v", "p2k": "p2v"},
    ///     headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/88.0.4324.150 Safari/537.36"},
    ///     cookie_store=False,
    ///     referer=False,
    ///     proxy="http://127.0.0.1:8080",
    ///     timeout=10,
    ///     impersonate="chrome_123",
    ///     follow_redirects=True,
    ///     max_redirects=1,
    ///     verify=False,
    ///     http1=True,
    ///     http2=False,
    /// )
    /// ```
    fn new(
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        cookie_store: Option<bool>,
        referer: Option<bool>,
        proxy: Option<&str>,
        timeout: Option<f64>,
        impersonate: Option<&str>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
        verify: Option<bool>,
        http1: Option<bool>,
        http2: Option<bool>,
    ) -> PyResult<Self> {
        if auth.is_some() && auth_bearer.is_some() {
            return Err(PyErr::new::<exceptions::PyValueError, _>(
                "Cannot provide both auth and auth_bearer",
            ));
        }

        let mut client_builder = reqwest_impersonate::blocking::Client::builder()
            .enable_ech_grease(true)
            .permute_extensions(true)
            .timeout(timeout.map(Duration::from_secs_f64));

        // Headers
        if let Some(headers) = headers {
            let mut headers_new = HeaderMap::new();
            for (key, value) in headers {
                headers_new.insert(
                    HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
                        PyErr::new::<exceptions::PyValueError, _>("Invalid header name")
                    })?,
                    HeaderValue::from_str(&value).map_err(|_| {
                        PyErr::new::<exceptions::PyValueError, _>("Invalid header value")
                    })?,
                );
            }
            client_builder = client_builder.default_headers(headers_new);
        }

        // Cookie_store
        if cookie_store.unwrap_or(true) {
            client_builder = client_builder.cookie_store(true);
        }

        // Referer
        if referer.unwrap_or(true) {
            client_builder = client_builder.referer(true);
        }

        // Proxy
        if let Some(proxy_url) = proxy {
            let proxy = reqwest_impersonate::Proxy::all(proxy_url)
                .map_err(|_| PyErr::new::<exceptions::PyValueError, _>("Invalid proxy URL"))?;
            client_builder = client_builder.proxy(proxy);
        }

        // Impersonate
        if let Some(impersonation_type) = impersonate {
            let impersonation = Impersonate::from_str(impersonation_type).map_err(|_| {
                PyErr::new::<exceptions::PyValueError, _>("Invalid impersonate param")
            })?;
            client_builder = client_builder.impersonate(impersonation);
        }

        // Redirects
        let max_redirects = max_redirects.unwrap_or(20); // Default to 20 if not provided
        if follow_redirects.unwrap_or(true) {
            client_builder = client_builder.redirect(Policy::limited(max_redirects));
        } else {
            client_builder = client_builder.redirect(Policy::none());
        }

        // Verify
        let verify = verify.unwrap_or(false);
        if !verify {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        // Http version: http1 || http2
        match (http1, http2) {
            (Some(true), Some(true)) => {
                return Err(PyErr::new::<exceptions::PyValueError, _>(
                    "Both http1 and http2 cannot be true",
                ));
            }
            (Some(true), _) => client_builder = client_builder.http1_only(),
            (_, Some(true)) => client_builder = client_builder.http2_prior_knowledge(),
            _ => (),
        }

        let client = client_builder
            .build()
            .map_err(|_| PyErr::new::<exceptions::PyValueError, _>("Failed to build client"))?;

        Ok(Client {
            client,
            auth,
            auth_bearer,
            params,
        })
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
    /// * `content` - The content to send in the request body as bytes. Default is None.
    /// * `data` - The form data to send in the request body. Default is None.
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
    fn request(
        &self,
        py: Python,
        method: &str,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        content: Option<Vec<u8>>,
        data: Option<HashMap<String, String>>,
        files: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        // Check if method is POST || PUT || PATCH
        let is_post_put_patch = method == "POST" || method == "PUT" || method == "PATCH";

        // Method
        let method = match method {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "HEAD" => Ok(Method::HEAD),
            "OPTIONS" => Ok(Method::OPTIONS),
            "PUT" => Ok(Method::PUT),
            "PATCH" => Ok(Method::PATCH),
            "DELETE" => Ok(Method::DELETE),
            &_ => Err(PyErr::new::<exceptions::PyException, _>(
                "Unrecognized HTTP method",
            )),
        };
        let method = method?;

        // Create request builder
        let mut request_builder = self.client.request(method, url);

        // Params (use the provided `params` if available; otherwise, fall back to `self.params`)
        let params_to_use = params.or(self.params.clone()).unwrap_or_default();
        if !params_to_use.is_empty() {
            request_builder = request_builder.query(&params_to_use);
        }

        // Headers
        if let Some(headers) = headers {
            let mut headers_new = HeaderMap::new();
            for (key, value) in headers {
                headers_new.insert(
                    HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
                        PyErr::new::<exceptions::PyValueError, _>("Invalid header name")
                    })?,
                    HeaderValue::from_str(&value).map_err(|_| {
                        PyErr::new::<exceptions::PyValueError, _>("Invalid header value")
                    })?,
                );
            }
            request_builder = request_builder.headers(headers_new);
        }

        // Only if method POST || PUT || PATCH
        if is_post_put_patch {
            // Content
            if let Some(content) = content {
                request_builder = request_builder.body(content);
            }
            // Data
            if let Some(data) = data {
                request_builder = request_builder.form(&data);
            }
            // Files
            if let Some(files) = files {
                let mut form = multipart::Form::new();
                for (field, path) in files {
                    form = form.file(field, path)?;
                }
                request_builder = request_builder.multipart(form);
            }
        }

        // Auth
        match (auth, auth_bearer, &self.auth, &self.auth_bearer) {
            (Some((username, password)), None, _, _) => {
                request_builder = request_builder.basic_auth(username, password.as_deref());
            }
            (None, Some(token), _, _) => {
                request_builder = request_builder.bearer_auth(token);
            }
            (None, None, Some((username, password)), None) => {
                request_builder = request_builder.basic_auth(username, password.as_deref());
            }
            (None, None, None, Some(token)) => {
                request_builder = request_builder.bearer_auth(token);
            }
            (Some(_), Some(_), None, None) | (None, None, Some(_), Some(_)) => {
                return Err(PyErr::new::<exceptions::PyValueError, _>(
                    "Cannot provide both auth and auth_bearer",
                ));
            }
            _ => {} // No authentication provided
        }

        // Timeout
        if let Some(seconds) = timeout {
            request_builder = request_builder.timeout(Duration::from_secs_f64(seconds));
        }

        // Send request | release GIL
        let resp = py.allow_threads(|| {
            request_builder.send().map_err(|e| {
                PyErr::new::<exceptions::PyException, _>(format!("Error in request: {}", e))
            })
        })?;

        // Response items
        let cookies_dict = PyDict::new_bound(py);
        for cookie in resp.cookies() {
            let key = cookie.name().to_string();
            let value = cookie.value().to_string();
            cookies_dict.set_item(key, value)?;
        }
        let cookies = cookies_dict.unbind();

        // Encoding from "Content-Type" header or "UTF-8"
        let encoding = {
            let encoding_str = resp
                .headers()
                .get("Content-Type")
                .and_then(|ct| ct.to_str().ok())
                .and_then(|ct| {
                    ct.split(';').find_map(|param| {
                        let mut kv = param.splitn(2, '=');
                        let key = kv.next()?.trim();
                        let value = kv.next()?.trim();
                        if key.eq_ignore_ascii_case("charset") {
                            Some(value.to_string())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or("UTF-8".to_string());
            PyString::new_bound(py, &encoding_str).unbind()
        };

        let headers_dict = PyDict::new_bound(py);
        for (key, value) in resp.headers().iter() {
            let key_str = key.as_str();
            let value_str = value.to_str().unwrap_or("");
            headers_dict.set_item(key_str, value_str)?;
        }
        let headers = headers_dict.unbind();

        let status_code = resp.status().as_u16().into_py(py);

        let url = PyString::new_bound(py, resp.url().as_ref()).into();

        let buf = resp.bytes().map_err(|e| {
            PyErr::new::<exceptions::PyException, _>(format!("Error reading response bytes: {}", e))
        })?;
        let content = PyBytes::new_bound(py, &buf).unbind();

        Ok(Response {
            content,
            cookies,
            encoding,
            headers,
            status_code,
            url,
        })
    }

    fn get(
        &self,
        py: Python,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
            py,
            "GET",
            url,
            params,
            headers,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
        )
    }
    fn head(
        &self,
        py: Python,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
            py,
            "HEAD",
            url,
            params,
            headers,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
        )
    }
    fn options(
        &self,
        py: Python,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
            py,
            "OPTIONS",
            url,
            params,
            headers,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
        )
    }
    fn delete(
        &self,
        py: Python,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
            py,
            "DELETE",
            url,
            params,
            headers,
            None,
            None,
            None,
            auth,
            auth_bearer,
            timeout,
        )
    }

    fn post(
        &self,
        py: Python,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        content: Option<Vec<u8>>,
        data: Option<HashMap<String, String>>,
        files: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
            py,
            "POST",
            url,
            params,
            headers,
            content,
            data,
            files,
            auth,
            auth_bearer,
            timeout,
        )
    }
    fn put(
        &self,
        py: Python,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        content: Option<Vec<u8>>,
        data: Option<HashMap<String, String>>,
        files: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
            py,
            "PUT",
            url,
            params,
            headers,
            content,
            data,
            files,
            auth,
            auth_bearer,
            timeout,
        )
    }
    fn patch(
        &self,
        py: Python,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        content: Option<Vec<u8>>,
        data: Option<HashMap<String, String>>,
        files: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
            py,
            "PATCH",
            url,
            params,
            headers,
            content,
            data,
            files,
            auth,
            auth_bearer,
            timeout,
        )
    }
}

/// Convenience functions that use a default Client instance under the hood
#[pyfunction]
fn get(
    py: Python,
    url: &str,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
) -> PyResult<Response> {
    let client = Client::new(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    client.get(py, url, params, headers, auth, auth_bearer, timeout)
}

#[pyfunction]
fn head(
    py: Python,
    url: &str,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
) -> PyResult<Response> {
    let client = Client::new(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    client.head(py, url, params, headers, auth, auth_bearer, timeout)
}

#[pyfunction]
fn options(
    py: Python,
    url: &str,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
) -> PyResult<Response> {
    let client = Client::new(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    client.options(py, url, params, headers, auth, auth_bearer, timeout)
}

#[pyfunction]
fn delete(
    py: Python,
    url: &str,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
) -> PyResult<Response> {
    let client = Client::new(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    client.delete(py, url, params, headers, auth, auth_bearer, timeout)
}

#[pyfunction]
fn post(
    py: Python,
    url: &str,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    content: Option<Vec<u8>>,
    data: Option<HashMap<String, String>>,
    files: Option<HashMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
) -> PyResult<Response> {
    let client = Client::new(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    client.post(
        py,
        url,
        params,
        headers,
        content,
        data,
        files,
        auth,
        auth_bearer,
        timeout,
    )
}

#[pyfunction]
fn put(
    py: Python,
    url: &str,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    content: Option<Vec<u8>>,
    data: Option<HashMap<String, String>>,
    files: Option<HashMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
) -> PyResult<Response> {
    let client = Client::new(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    client.put(
        py,
        url,
        params,
        headers,
        content,
        data,
        files,
        auth,
        auth_bearer,
        timeout,
    )
}

#[pyfunction]
fn patch(
    py: Python,
    url: &str,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    content: Option<Vec<u8>>,
    data: Option<HashMap<String, String>>,
    files: Option<HashMap<String, String>>,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    timeout: Option<f64>,
) -> PyResult<Response> {
    let client = Client::new(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    client.patch(
        py,
        url,
        params,
        headers,
        content,
        data,
        files,
        auth,
        auth_bearer,
        timeout,
    )
}

#[pymodule]
fn pyreqwest_impersonate(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Client>()?;
    m.add_function(wrap_pyfunction!(get, m)?)?;
    m.add_function(wrap_pyfunction!(head, m)?)?;
    m.add_function(wrap_pyfunction!(options, m)?)?;
    m.add_function(wrap_pyfunction!(delete, m)?)?;
    m.add_function(wrap_pyfunction!(post, m)?)?;
    m.add_function(wrap_pyfunction!(patch, m)?)?;
    m.add_function(wrap_pyfunction!(put, m)?)?;
    Ok(())
}
