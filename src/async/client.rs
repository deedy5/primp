use std::sync::{Arc, RwLock};
use std::time::Duration;

use pyo3::prelude::*;
use pythonize::depythonize;
use reqwest::{multipart, Body, Client, Method};
use serde_json::Value;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::client_builder::{
    configure_client_builder, cookies_to_header_values, headers_without_cookie,
    parse_cookies_from_header, IndexMapSSR,
};
use crate::error::{ErrorContext, PrimpError, PrimpResult};
use crate::traits::{HeaderMapExt, HeadersTraits};

/// Async HTTP client that can impersonate web browsers.
#[pyclass(subclass)]
pub struct RAsyncClient {
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
impl RAsyncClient {
    /// Initializes an async HTTP client that can impersonate web browsers.
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

        let client = client_builder.build()?;

        Ok(RAsyncClient {
            client: Arc::new(RwLock::new(client)),
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
    pub fn set_headers(&mut self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
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
        let cookie = client
            .get_cookies(&url)
            .ok_or_else(|| PrimpError::Custom("Failed to get cookies".to_string()))?;
        let cookie_str = cookie
            .to_str()
            .map_err(|e| PrimpError::Custom(e.to_string()))?;
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

    /// Constructs an async HTTP request with the given method, URL, and optionally sets a timeout, headers, and query parameters.
    /// Sends the request and returns a `Response` object containing the server's response.
    #[pyo3(signature = (method, url, params=None, headers=None, cookies=None, content=None,
        data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
    ) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;

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
        let url_owned = url.to_string();

        // Build error context for enriched error messages
        let error_context = ErrorContext::new()
            .with_method(method.as_str())
            .with_url(&url_owned);
        let error_context = if let Some(t) = timeout {
            error_context.with_timeout((t * 1000.0) as u64)
        } else {
            error_context
        };

        // Cookies
        if let Some(cookies) = cookies {
            let url = reqwest::Url::parse(&url_owned).map_err(Into::<PrimpError>::into)?;
            let cookie_values = cookies_to_header_values(&cookies);
            let client = self.client.read().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
            client.set_cookies(&url, cookie_values);
        }

        // Clone the client before entering the async block to avoid holding RwLockGuard across await
        let client = {
            let client_guard = self.client.read().map_err(|_| PrimpError::Custom("Failed to acquire client lock".to_string()))?;
            client_guard.clone()
        };

        let future = async move {
            // Create request builder
            let mut request_builder = client.request(method, &url_owned);

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
            Ok::<crate::r#async::response::AsyncResponse, PrimpError>(
                crate::r#async::response::AsyncResponse::new(resp, url, status_code),
            )
        };

        // Convert Rust future to Python awaitable
        // The future must resolve to PyResult<T>, so we convert PrimpError to PyErr
        let py_future = async move {
            match future.await {
                Ok(async_response) => Ok(async_response),
                Err(e) => Err(e.into()),
            }
        };

        future_into_py(py, py_future)
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, auth=None, auth_bearer=None, timeout=None))]
    fn get<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
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
    fn head<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
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
    fn options<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
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
    fn delete<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        params: Option<IndexMapSSR>,
        headers: Option<IndexMapSSR>,
        cookies: Option<IndexMapSSR>,
        auth: Option<(String, Option<String>)>,
        auth_bearer: Option<String>,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
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

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
        )
    }

    #[pyo3(signature = (url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None))]
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
        )
    }
}
