use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use pyo3::exceptions;
use pyo3::prelude::*;
use reqwest_impersonate::blocking::multipart;
use reqwest_impersonate::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest_impersonate::impersonate::Impersonate;
use reqwest_impersonate::redirect::Policy;
use reqwest_impersonate::Method;

mod response;
use response::Response;

#[pyclass]
/// A blocking HTTP client that can impersonate web browsers.
struct Client {
    client: reqwest_impersonate::blocking::Client,
    auth: Option<(String, Option<String>)>,
    auth_bearer: Option<String>,
    params: Option<HashMap<String, String>>,
}

#[pymethods]
impl Client {
    #[new]
    /// Initializes a blocking HTTP client that can impersonate web browsers. Not thread-safe!
    ///
    /// This function creates a new instance of a blocking HTTP client that can impersonate various web browsers.
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
    /// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is `true`.
    /// * `http1` - An optional boolean indicating whether to use only HTTP/1.1. Default is `false`.
    /// * `http2` - An optional boolean indicating whether to use only HTTP/2. Default is `false`.
    ///
    /// # Note
    /// The Client instance is not thread-safe, meaning it should be initialized once and reused across a multi-threaded environment.
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
            .trust_dns(true)
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
        if let Some(verify) = verify {
            if !verify {
                client_builder = client_builder.danger_accept_invalid_certs(true);
            }
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

        // Send request
        let mut resp = Python::with_gil(|py| {
            py.allow_threads(|| {
                request_builder.send().map_err(|e| {
                    PyErr::new::<exceptions::PyException, _>(format!("Error in request: {}", e))
                })
            })
        })?;

        // Response items
        let mut raw: Vec<u8> = vec![];
        resp.copy_to(&mut raw).map_err(|e| {
            PyErr::new::<exceptions::PyIOError, _>(format!("Error in get resp.raw: {}", e))
        })?;
        let cookies: HashMap<String, String> = resp
            .cookies()
            .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
            .collect();
        // Encoding from "Content-Type" header or "UTF-8"
        let encoding = resp
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
        let headers: HashMap<String, String> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let status_code = resp.status().as_u16();
        let url = resp.url().to_string();

        Ok(Response {
            cookies,
            encoding,
            headers,
            raw,
            status_code,
            url,
        })
    }

    fn get(
        &self,
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
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
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
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
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
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
        url: &str,
        params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Response> {
        self.request(
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

#[pymodule]
fn pyreqwest_impersonate(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Client>()?;
    Ok(())
}
