use pyo3::exceptions;
use pyo3::prelude::*;
use reqwest_impersonate::impersonate::Impersonate;
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
/// A client for making HTTP requests with specific configurations.
///
/// This class allows for making HTTP requests with customizable timeout, proxy, and impersonation settings.
/// It is designed to be used in scenarios where you need to make HTTP requests that mimic the behavior of a specific browser or entity.
struct Client {
    client: reqwest_impersonate::blocking::Client,
}

#[pymethods]
impl Client {
    #[new]
    /// Creates a new `Client` instance with the specified configurations.
    ///
    /// Args:
    ///     timeout (float, optional): The timeout for the HTTP requests in seconds. Default is None.
    ///     proxy (str, optional): The proxy URL to use for the HTTP requests. Default is None.
    ///     impersonate (str, optional): The identifier for the entity to impersonate. Default is None.
    fn new(timeout: Option<f64>, proxy: Option<&str>, impersonate: Option<&str>) -> PyResult<Self> {
        let mut client_builder = reqwest_impersonate::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .enable_ech_grease(true)
            .permute_extensions(true)
            .cookie_store(true)
            .timeout(timeout.map(Duration::from_secs_f64));

        if let Some(impersonation_type) = impersonate {
            let impersonation = Impersonate::from_str(impersonation_type).map_err(|_| {
                PyErr::new::<exceptions::PyValueError, _>("Invalid impersonate param")
            })?;
            client_builder = client_builder.impersonate(impersonation);
        }

        if let Some(proxy_url) = proxy {
            let proxy = reqwest_impersonate::Proxy::all(proxy_url)
                .map_err(|_| PyErr::new::<exceptions::PyValueError, _>("Invalid proxy URL"))?;
            client_builder = client_builder.proxy(proxy);
        }

        let client = client_builder
            .build()
            .map_err(|_| PyErr::new::<exceptions::PyValueError, _>("Failed to build client"))?;

        Ok(Client { client })
    }

    ///
    /// This method constructs an HTTP request with the given method and URL, and optionally sets a timeout.
    /// It then sends the request and returns a `Response` object containing the server's response.
    ///
    /// Args:
    ///     method (str): The HTTP method to use (e.g., "GET", "POST").
    ///     url (str): The URL to which the request will be made.
    ///     timeout (float, optional): The timeout for the request in seconds. Default is None.
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
            "PUT" => Ok(Method::PUT),
            "DELETE" => Ok(Method::DELETE),
            "HEAD" => Ok(Method::HEAD),
            "OPTIONS" => Ok(Method::OPTIONS),
            "CONNECT" => Ok(Method::CONNECT),
            "TRACE" => Ok(Method::TRACE),
            "PATCH" => Ok(Method::PATCH),
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
}

#[pymodule]
fn pyreqwest_impersonate(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Client>()?;
    Ok(())
}
