#![allow(clippy::too_many_arguments)]
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use anyhow::Result;
use foldhash::fast::RandomState;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pythonize::depythonize;
use wreq::{
    header::{HeaderValue, COOKIE},
    multipart,
    redirect::Policy,
    Body, Method,
};
use wreq_util::{Emulation, EmulationOS, EmulationOption};
use serde_json::Value;
use tokio::{
    fs::File,
    runtime::{self, Runtime},
};
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing;

mod impersonate;
use impersonate::{EmulationFromStr, EmulationOSFromStr};
mod response;
use response::Response;

mod traits;
use traits::HeadersTraits;

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

#[pyclass(subclass)]
/// HTTP client that can impersonate web browsers.
pub struct Client {
    client: Arc<Mutex<wreq::Client>>,
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
    // Store builder options to recreate client when needed
    headers: Option<IndexMapSSR>,
    cookie_store: bool,
    referer: bool,
    follow_redirects: bool,
    max_redirects: usize,
    verify: bool,
    ca_cert_file: Option<String>,
    https_only: bool,
    http2_only: bool,
}

#[pymethods]
impl Client {
    #[new]
    #[pyo3(signature = (auth=None, auth_bearer=None, params=None, headers=None, cookie_store=true,
        referer=true, proxy=None, timeout=None, impersonate=None, impersonate_os=None, follow_redirects=true,
        max_redirects=20, verify=true, ca_cert_file=None, https_only=false, http2_only=false))]
    /// HTTP client that can impersonate web browsers.
    ///
    /// Args:
    ///     auth (tuple[str, str] | None): Basic authentication credentials (username, password). Default is None.
    ///     auth_bearer (str | None): Bearer token for authentication. Default is None.
    ///     params (dict | None): Default query parameters to include in all requests. Default is None.
    ///     headers (dict | None): Default headers to include in all requests. Default is None.
    ///     cookie_store (bool): Enable automatic cookie storage and management. Default is True.
    ///     referer (bool): Automatically set referer header. Default is True.
    ///     proxy (str | None): Proxy URL (http/https/socks5). Default is None.
    ///     timeout (float | None): Request timeout in seconds. Default is None.
    ///     impersonate (str | None): Browser to impersonate. Example: "chrome_131". Default is None.
    ///         Chrome: "chrome_100", "chrome_101", "chrome_104", "chrome_105", "chrome_106", "chrome_107", 
    ///         "chrome_108", "chrome_109", "chrome_114", "chrome_116", "chrome_117", "chrome_118", 
    ///         "chrome_119", "chrome_120", "chrome_123", "chrome_124", "chrome_126", "chrome_127", 
    ///         "chrome_128", "chrome_129", "chrome_130", "chrome_131", "chrome_133", "chrome_137"
    ///         Safari: "safari_15.3", "safari_15.5", "safari_15.6.1", "safari_16", "safari_16.5", 
    ///         "safari_17.0", "safari_17.2.1", "safari_17.4.1", "safari_17.5", "safari_18", "safari_18.2"
    ///         Safari iOS: "safari_ios_16.5", "safari_ios_17.2", "safari_ios_17.4.1", "safari_ios_18.1.1", "safari_ipad_18"
    ///         Edge: "edge_101", "edge_122", "edge_127", "edge_131"
    ///         Firefox: "firefox_109", "firefox_117", "firefox_128", "firefox_133", "firefox_135", "firefox_136"
    ///         OkHttp: "okhttp_3.13", "okhttp_3.14", "okhttp_4.9", "okhttp_4.10", "okhttp_5"
    ///         Select random: "random"
    ///     impersonate_os (str | None): impersonate OS. Example: "windows". Default is "linux".
    ///         Android: "android", iOS: "ios", Linux: "linux", Mac OS: "macos", Windows: "windows"
    ///         Select random: "random"
    ///     follow_redirects (bool): Follow HTTP redirects automatically. Default is True.
    ///     max_redirects (int): Maximum number of redirects to follow. Default is 20.
    ///     verify (bool): Verify SSL certificates. Default is True.
    ///     ca_cert_file (str | None): Path to custom CA certificate file. Default is None.
    ///     https_only (bool): Only allow HTTPS connections. Default is False.
    ///     http2_only (bool): Force HTTP/2 only. Default is False.
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
    ) -> Result<Self> {
        let cookie_store = cookie_store.unwrap_or(true);
        let referer = referer.unwrap_or(true);
        let follow_redirects = follow_redirects.unwrap_or(true);
        let max_redirects = max_redirects.unwrap_or(20);
        let verify = verify.unwrap_or(true);
        let https_only = https_only.unwrap_or(false);
        let http2_only = http2_only.unwrap_or(false);
        
        // Set default impersonate_os to "linux" if not provided
        let impersonate_os = impersonate_os.unwrap_or_else(|| "linux".to_string());

        let client = Self::build_client(
            &headers,
            cookie_store,
            referer,
            &proxy,
            timeout,
            &impersonate,
            &Some(impersonate_os.clone()),
            follow_redirects,
            max_redirects,
            verify,
            &ca_cert_file,
            https_only,
            http2_only,
        )?;

        Ok(Client {
            client: Arc::new(Mutex::new(client)),
            auth,
            auth_bearer,
            params,
            proxy,
            timeout,
            impersonate,
            impersonate_os: Some(impersonate_os),
            headers,
            cookie_store,
            referer,
            follow_redirects,
            max_redirects,
            verify,
            ca_cert_file,
            https_only,
            http2_only,
        })
    }

    #[getter]
    pub fn get_headers(&self) -> Result<IndexMapSSR> {
        let client = self.client.lock().unwrap();
        let mut headers = client.headers().clone();
        headers.remove(COOKIE);
        Ok(headers.to_indexmap())
    }

    #[setter]
    pub fn set_headers(&mut self, new_headers: Option<IndexMapSSR>) -> Result<()> {
        self.headers = new_headers;
        self.rebuild_client()?;
        Ok(())
    }

    pub fn headers_update(&mut self, new_headers: Option<IndexMapSSR>) -> Result<()> {
        if let Some(new_headers) = new_headers {
            if let Some(ref mut existing_headers) = self.headers {
                existing_headers.extend(new_headers);
            } else {
                self.headers = Some(new_headers);
            }
        }
        self.rebuild_client()?;
        Ok(())
    }

    #[getter]
    pub fn get_proxy(&self) -> Result<Option<String>> {
        Ok(self.proxy.to_owned())
    }

    #[setter]
    pub fn set_proxy(&mut self, proxy: String) -> Result<()> {
        self.proxy = Some(proxy);
        self.rebuild_client()?;
        Ok(())
    }

    #[setter]
    pub fn set_impersonate(&mut self, impersonate: String) -> Result<()> {
        self.impersonate = Some(impersonate);
        self.rebuild_client()?;
        Ok(())
    }

    #[setter]
    pub fn set_impersonate_os(&mut self, impersonate_os: String) -> Result<()> {
        self.impersonate_os = Some(impersonate_os);
        self.rebuild_client()?;
        Ok(())
    }

    #[pyo3(signature = (url))]
    fn get_cookies(&self, url: &str) -> Result<IndexMapSSR> {
        let url = wreq::Url::parse(url).expect("Error parsing URL");
        let client = self.client.lock().unwrap();
        let cookie = client.get_cookies(&url).expect("No cookies found");
        let cookie_str = cookie.to_str()?;
        let mut cookie_map = IndexMap::with_capacity_and_hasher(10, RandomState::default());
        for cookie in cookie_str.split(';') {
            let mut parts = cookie.splitn(2, '=');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                cookie_map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        Ok(cookie_map)
    }

    #[pyo3(signature = (url, cookies))]
    fn set_cookies(&self, url: &str, cookies: Option<IndexMapSSR>) -> Result<()> {
        let url = wreq::Url::parse(url).expect("Error parsing URL");
        if let Some(cookies) = cookies {
            let header_values: Vec<HeaderValue> = cookies
                .iter()
                .filter_map(|(key, value)| {
                    HeaderValue::from_str(&format!("{}={}", key, value)).ok()
                })
                .collect();
            let client = self.client.lock().unwrap();
            client.set_cookies(&url, header_values);
        }
        Ok(())
    }

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

        // Cookies
        if let Some(cookies) = cookies {
            let url = wreq::Url::parse(url)?;
            let cookie_values: Vec<HeaderValue> = cookies
                .iter()
                .filter_map(|(key, value)| {
                    let cookie_string = format!("{}={}", key, value.to_string());
                    HeaderValue::from_str(&cookie_string).ok()
                })
                .collect();
            let client = client.lock().unwrap();
            client.set_cookies(&url, cookie_values);
        }

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
            let resp: wreq::Response = request_builder.send().await?;
            let url: String = resp.url().to_string();
            let status_code = resp.status().as_u16();

            tracing::info!("response: {} {}", url, status_code);
            Ok((resp, url, status_code))
        };

        // Execute an async future, releasing the Python GIL for concurrency.
        // Use Tokio global runtime to block on the future.
        let response: Result<(wreq::Response, String, u16)> =
            py.allow_threads(|| RUNTIME.block_on(future));
        let result = response?;
        let resp = http::Response::from(result.0);
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

impl Client {
    fn build_client(
        headers: &Option<IndexMapSSR>,
        cookie_store: bool,
        referer: bool,
        proxy: &Option<String>,
        timeout: Option<f64>,
        impersonate: &Option<String>,
        impersonate_os: &Option<String>,
        follow_redirects: bool,
        max_redirects: usize,
        verify: bool,
        ca_cert_file: &Option<String>,
        https_only: bool,
        http2_only: bool,
    ) -> Result<wreq::Client> {
        // Client builder
        let mut client_builder = wreq::Client::builder();

        // Emulation and EmulationOS using EmulationOption
        match (impersonate, impersonate_os) {
            (Some(imp), Some(os)) => {
                let emulation = Emulation::from_str(imp)?;
                let emulation_os = EmulationOS::from_str(os)?;
                
                let emulation_option = EmulationOption::builder()
                    .emulation(emulation)
                    .emulation_os(emulation_os)
                    .build();
                
                client_builder = client_builder.emulation(emulation_option);
            }
            (Some(imp), None) => {
                let emulation = Emulation::from_str(imp)?;
                // Use default "linux" OS
                let emulation_os = EmulationOS::from_str("linux")?;
                
                let emulation_option = EmulationOption::builder()
                    .emulation(emulation)
                    .emulation_os(emulation_os)
                    .build();
                
                client_builder = client_builder.emulation(emulation_option);
            }
            (None, Some(os)) => {
                let emulation_os = EmulationOS::from_str(os)?;
                
                let emulation_option = EmulationOption::builder()
                    .emulation_os(emulation_os)
                    .build();
                
                client_builder = client_builder.emulation(emulation_option);
            }
            (None, None) => {
                // Default to linux OS only
                let emulation_os = EmulationOS::from_str("linux")?;
                
                let emulation_option = EmulationOption::builder()
                    .emulation_os(emulation_os)
                    .build();
                
                client_builder = client_builder.emulation(emulation_option);
            }
        }

        // Headers
        if let Some(headers) = headers {
            let headers_headermap = headers.to_headermap();
            client_builder = client_builder.default_headers(headers_headermap);
        }

        // Cookie_store
        if cookie_store {
            client_builder = client_builder.cookie_store(true);
        }

        // Referer
        if referer {
            client_builder = client_builder.referer(true);
        }

        // Proxy - FIXED: Handle environment variable properly
        let env_proxy = std::env::var("PRIMP_PROXY").ok();
        let proxy = proxy.as_ref().or(env_proxy.as_ref());
        if let Some(proxy) = proxy {
            client_builder = client_builder.proxy(wreq::Proxy::all(proxy)?);
        }

        // Timeout
        if let Some(seconds) = timeout {
            client_builder = client_builder.timeout(Duration::from_secs_f64(seconds));
        }

        // Redirects
        if follow_redirects {
            client_builder = client_builder.redirect(Policy::limited(max_redirects));
        } else {
            client_builder = client_builder.redirect(Policy::none());
        }

        // Ca_cert_file. BEFORE!!! verify
        if let Some(ca_bundle_path) = ca_cert_file {
            std::env::set_var("PRIMP_CA_BUNDLE", ca_bundle_path);
        }

        // Verify
        if verify {
            client_builder = client_builder.cert_store(load_ca_certs()?);
        } else {
            client_builder = client_builder.cert_verification(false);
        }

        // Https_only
        if https_only {
            client_builder = client_builder.https_only(true);
        }

        // Http2_only
        if http2_only {
            client_builder = client_builder.http2_only();
        }

        Ok(client_builder.build()?)
    }

    fn rebuild_client(&mut self) -> Result<()> {
        let new_client = Self::build_client(
            &self.headers,
            self.cookie_store,
            self.referer,
            &self.proxy,
            self.timeout,
            &self.impersonate,
            &self.impersonate_os,
            self.follow_redirects,
            self.max_redirects,
            self.verify,
            &self.ca_cert_file,
            self.https_only,
            self.http2_only,
        )?;
        
        *self.client.lock().unwrap() = new_client;
        Ok(())
    }
}

#[pymodule]
fn primp(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    m.add_class::<Client>()?;
    Ok(())
}
