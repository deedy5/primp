use std::sync::{Arc, RwLock};
use std::time::Duration;

use ::primp::{
    multipart, Body, Client as PrimpClient, Method, Proxy, Response as PrimpResponse, Url,
};
use pyo3::prelude::*;
use pythonize::depythonize;
use serde_json::Value;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::client_builder::{
    configure_client_builder, cookies_to_header_values, headers_without_cookie,
    parse_cookies_from_header, parse_url_or_domain, IndexMapSSR,
};
use crate::error::{PrimpErrorEnum, PrimpResult};
use crate::extract_cookies_to_indexmap;
use crate::traits::{HeaderMapExt, HeadersTraits};
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

#[pymethods]
impl AsyncClient {
    /// Initializes an async HTTP client that can impersonate web browsers.
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
        // Release the GIL around the reqwest client build — identical
        // reasoning to `Client::new`. See the comment there and
        // vorner/pyo3-log#65 for the deadlock mechanism.
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

            Ok(AsyncClient {
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
    pub fn set_headers(&mut self, new_headers: Option<IndexMapSSR>) -> PrimpResult<()> {
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
            .ok_or_else(|| PrimpErrorEnum::Custom("Failed to get cookies".to_string()))?;
        let cookie_str = cookie
            .to_str()
            .map_err(|e| PrimpErrorEnum::Custom(e.to_string()))?;
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

    /// Constructs an async HTTP request with the given method, URL, and optionally sets a timeout, headers, and query parameters.
    /// Sends the request and returns a `Response` object containing the server's response.
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
        use pyo3_async_runtimes::tokio::future_into_py;

        let method = Method::from_bytes(method.as_bytes()).map_err(Into::<PrimpErrorEnum>::into)?;

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

        // Apply client-level cookies
        if let Some(ref client_cookies) = self.cookies {
            if !client_cookies.is_empty() {
                let url_parsed = Url::parse(&resolved_url).map_err(Into::<PrimpErrorEnum>::into)?;
                let cookie_values = cookies_to_header_values(client_cookies);
                let client_guard = self.client.read().expect("client lock was poisoned");
                client_guard.set_cookies(&url_parsed, cookie_values);
            }
        }

        // Request-level cookies
        if let Some(cookies) = cookies.filter(|c| !c.is_empty()) {
            let url_parsed = Url::parse(&resolved_url).map_err(Into::<PrimpErrorEnum>::into)?;
            let cookie_values = cookies_to_header_values(&cookies);
            let client_guard = self.client.read().expect("client lock was poisoned");
            client_guard.set_cookies(&url_parsed, cookie_values);
        }

        // Handle follow_redirects: set policy before cloning client
        if let Some(fr) = follow_redirects {
            let mut client_guard = self.client.write().expect("client lock was poisoned");
            if fr {
                client_guard.set_redirect_policy(::primp::redirect::Policy::limited(20));
            } else {
                client_guard.set_redirect_policy(::primp::redirect::Policy::none());
            }
        }

        // Clone the client before entering the async block to avoid holding RwLockGuard across await
        let client = {
            let client_guard = self.client.read().expect("client lock was poisoned");
            client_guard.clone()
        };

        let future = async move {
            // Create request builder
            let mut request_builder = client.request(method, &resolved_url);

            // Params
            if let Some(p) = &params {
                request_builder = request_builder.query(p);
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
            if let Some((username, password)) = auth {
                request_builder = request_builder.basic_auth(username, password);
            } else if let Some(token) = auth_bearer {
                request_builder = request_builder.bearer_auth(token);
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
            Ok::<(PrimpResponse, String, u16), PrimpErrorEnum>((resp, url, status_code))
        };

        // Restore redirect policy if it was changed
        if follow_redirects.is_some() {
            let mut client_guard = self.client.write().expect("client lock was poisoned");
            client_guard.set_redirect_policy(::primp::redirect::Policy::limited(20));
        }

        // Convert Rust future to Python awaitable
        if stream {
            let py_future = async move {
                match future.await {
                    Ok((resp, url, status_code)) => {
                        let headers: IndexMapSSR = resp.headers().to_indexmap();
                        let cookies: IndexMapSSR = extract_cookies_to_indexmap(resp.headers());
                        let encoding = extract_encoding(resp.headers()).name().to_string();

                        Ok(crate::r#async::response::AsyncResponse::new_streaming(
                            resp,
                            url,
                            status_code,
                            encoding,
                            headers,
                            cookies,
                        ))
                    }
                    Err(e) => Err(e.into()),
                }
            };
            future_into_py(py, py_future)
        } else {
            let py_future = async move {
                match future.await {
                    Ok((resp, url, status_code)) => Ok(
                        crate::r#async::response::AsyncResponse::new(resp, url, status_code),
                    ),
                    Err(e) => Err(e.into()),
                }
            };
            future_into_py(py, py_future)
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
        use pyo3_async_runtimes::tokio::future_into_py;
        future_into_py(py, async move { Ok(slf) })
    }

    /// Exit the async context manager.
    fn __aexit__<'py>(
        &mut self,
        py: Python<'py>,
        _exc_type: Option<Bound<'_, PyAny>>,
        _exc_value: Option<Bound<'_, PyAny>>,
        _traceback: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        use pyo3_async_runtimes::tokio::future_into_py;
        future_into_py(py, async move { Ok(()) })
    }
}
