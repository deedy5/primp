use pyo3::exceptions;
use pyo3::prelude::*;
use reqwest_impersonate::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest_impersonate::impersonate::Impersonate;
use reqwest_impersonate::redirect::Policy;
use reqwest_impersonate::Method;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

#[pyclass]
struct Response {
    #[pyo3(get)]
    cookies: HashMap<String, String>,
    #[pyo3(get)]
    headers: HashMap<String, String>,
    #[pyo3(get)]
    status_code: u16,
    #[pyo3(get)]
    text: String,
    #[pyo3(get)]
    url: String,
}

#[pyclass]
/// A blocking HTTP client that can impersonate web browsers.
struct Client {
    client: reqwest_impersonate::blocking::Client,
}

#[pymethods]
impl Client {
    #[new]
    /// Initializes a blocking HTTP client that can impersonate web browsers.
    ///
    /// This function creates a new instance of a blocking HTTP client that can impersonate various web browsers.
    /// It allows for customization of headers, proxy settings, timeout, impersonation type, SSL certificate verification,
    /// and HTTP version preferences.
    ///
    /// # Arguments
    ///
    /// * `headers` - An optional map of HTTP headers to send with requests. If `impersonate` is set, this will be ignored.
    /// * `proxy` - An optional proxy URL for HTTP requests.
    /// * `timeout` - An optional timeout for HTTP requests in seconds.
    /// * `impersonate` - An optional entity to impersonate. Supported browsers and versions include Chrome, Safari, OkHttp, and Edge.
    /// * `redirects` - An optional number of redirects to follow. If set to 0, no redirects will be followed. Default is 10.
    /// * `verify` - An optional boolean indicating whether to verify SSL certificates. Default is `true`.
    /// * `http1` - An optional boolean indicating whether to use only HTTP/1.1. Default is `false`.
    /// * `http2` - An optional boolean indicating whether to use only HTTP/2. Default is `false`.
    ///
    /// # Example
    ///
    /// ```
    /// from reqwest_impersonate import Client
    ///
    /// client = Client(
    ///     headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/88.0.4324.150 Safari/537.36"},
    ///     proxy="http://127.0.0.1:8080",
    ///     timeout=10,
    ///     impersonate="chrome_123",
    ///     redirects=0,
    ///     verify=False,
    ///     http1=True,
    ///     http2=False,
    /// )
    /// ```
    fn new(
        headers: Option<HashMap<String, String>>,
        proxy: Option<&str>,
        timeout: Option<f64>,
        impersonate: Option<&str>,
        redirects: Option<usize>,
        verify: Option<bool>,
        http1: Option<bool>,
        http2: Option<bool>,
    ) -> PyResult<Self> {
        let mut client_builder = reqwest_impersonate::blocking::Client::builder()
            .enable_ech_grease(true)
            .permute_extensions(true)
            .cookie_store(true)
            .trust_dns(true)
            .timeout(timeout.map(Duration::from_secs_f64));

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

        if let Some(proxy_url) = proxy {
            let proxy = reqwest_impersonate::Proxy::all(proxy_url)
                .map_err(|_| PyErr::new::<exceptions::PyValueError, _>("Invalid proxy URL"))?;
            client_builder = client_builder.proxy(proxy);
        }

        if let Some(impersonation_type) = impersonate {
            let impersonation = Impersonate::from_str(impersonation_type).map_err(|_| {
                PyErr::new::<exceptions::PyValueError, _>("Invalid impersonate param")
            })?;
            client_builder = client_builder.impersonate(impersonation);
        }

        match redirects {
            Some(0) => client_builder = client_builder.redirect(Policy::none()),
            Some(n) => client_builder = client_builder.redirect(Policy::limited(n)),
            None => client_builder = client_builder.redirect(Policy::limited(10)), // default to 10 redirects
        }

        if let Some(verify) = verify {
            if !verify {
                client_builder = client_builder.danger_accept_invalid_certs(true);
            }
        }

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

        Ok(Client { client })
    }

    /// This method constructs an HTTP request with the given method and URL, and optionally sets a timeout.
    /// It then sends the request and returns a `Response` object containing the server's response.
    ///
    /// Args:
    ///     method (str): The HTTP method to use (e.g., "GET", "POST").
    ///     url (str): The URL to which the request will be made.
    ///     timeout (float, optional): The timeout for the request in seconds. Default is 30.
    ///
    /// Returns:
    ///     Response: A response object containing the server's response to the request.
    ///
    /// Raises:
    ///     PyException: If there is an error making the request.
    fn request(&self, method: &str, url: &str, timeout: Option<f64>) -> PyResult<Response> {
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
        let request_builder = match timeout {
            Some(timeout_seconds) => {
                let timeout_duration = Duration::from_secs_f64(timeout_seconds);
                self.client.request(method, url).timeout(timeout_duration)
            }
            None => self.client.request(method, url),
        };
        let resp = request_builder.send().map_err(|e| {
            PyErr::new::<exceptions::PyException, _>(format!("Error in request: {}", e))
        })?;
        let cookies: HashMap<String, String> = resp
            .cookies()
            .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
            .collect();
        let headers = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let status_code = resp.status().as_u16();
        let url = resp.url().to_string();
        let text = resp.text().unwrap_or_else(|_| String::new());

        Ok(Response {
            cookies,
            headers,
            status_code,
            text,
            url,
        })
    }

    fn get(&self, url: &str, timeout: Option<f64>) -> PyResult<Response> {
        self.request("GET", url, timeout)
    }

    fn post(&self, url: &str, timeout: Option<f64>) -> PyResult<Response> {
        self.request("POST", url, timeout)
    }

    fn head(&self, url: &str, timeout: Option<f64>) -> PyResult<Response> {
        self.request("HEAD", url, timeout)
    }

    fn options(&self, url: &str, timeout: Option<f64>) -> PyResult<Response> {
        self.request("OPTIONS", url, timeout)
    }

    fn put(&self, url: &str, timeout: Option<f64>) -> PyResult<Response> {
        self.request("PUT", url, timeout)
    }

    fn patch(&self, url: &str, timeout: Option<f64>) -> PyResult<Response> {
        self.request("PATCH", url, timeout)
    }

    fn delete(&self, url: &str, timeout: Option<f64>) -> PyResult<Response> {
        self.request("DELETE", url, timeout)
    }
}

#[pymodule]
fn pyreqwest_impersonate(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Client>()?;
    Ok(())
}
