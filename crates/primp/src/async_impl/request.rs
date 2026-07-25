use std::convert::TryFrom;
use std::fmt;
use std::future::Future;
use std::time::Duration;

#[cfg(any(feature = "query", feature = "form", feature = "json"))]
use serde::Serialize;

use super::body::Body;
use super::client::{Client, Pending};
#[cfg(feature = "multipart")]
use super::multipart;
use super::response::Response;
use crate::config::{
    OneShotCookies, ReadTimeout, RedirectOverride, RedirectPolicyOverride, RequestConfig,
    TotalTimeout,
};
#[cfg(feature = "multipart")]
use crate::header::CONTENT_LENGTH;
#[cfg(any(feature = "multipart", feature = "form", feature = "json"))]
use crate::header::CONTENT_TYPE;
use crate::header::{HeaderMap, HeaderName, HeaderValue};
use crate::{Method, Url};
use http::{request::Parts, Extensions, Request as HttpRequest, Version};

/// A request which can be executed with `Client::execute()`.
pub struct Request {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Body>,
    version: Version,
    extensions: Extensions,
}

/// Builder for the properties of a `Request`; see `Client` to construct one.
#[must_use = "RequestBuilder does nothing until you 'send' it"]
pub struct RequestBuilder {
    client: Client,
    request: crate::Result<Request>,
}

impl Request {
    /// Constructs a new request.
    #[inline]
    pub fn new(method: Method, url: Url) -> Self {
        Request {
            method,
            url,
            headers: HeaderMap::new(),
            body: None,
            version: Version::default(),
            extensions: Extensions::new(),
        }
    }

    /// Get the method.
    #[inline]
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Get a mutable reference to the method.
    #[inline]
    pub fn method_mut(&mut self) -> &mut Method {
        &mut self.method
    }

    /// Get the url.
    #[inline]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Get a mutable reference to the url.
    #[inline]
    pub fn url_mut(&mut self) -> &mut Url {
        &mut self.url
    }

    /// Get the headers.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Get a mutable reference to the headers.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Get the body.
    #[inline]
    pub fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    /// Get a mutable reference to the body.
    #[inline]
    pub fn body_mut(&mut self) -> &mut Option<Body> {
        &mut self.body
    }

    /// Get the extensions.
    #[inline]
    pub(crate) fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Get a mutable reference to the extensions.
    #[inline]
    pub(crate) fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }

    /// Get the timeout.
    #[inline]
    pub fn timeout(&self) -> Option<&Duration> {
        RequestConfig::<TotalTimeout>::get(&self.extensions)
    }

    /// Get a mutable reference to the timeout.
    #[inline]
    pub fn timeout_mut(&mut self) -> &mut Option<Duration> {
        RequestConfig::<TotalTimeout>::get_mut(&mut self.extensions)
    }

    /// Get the read timeout.
    #[inline]
    pub fn read_timeout(&self) -> Option<&Duration> {
        RequestConfig::<ReadTimeout>::get(&self.extensions)
    }

    /// Get a mutable reference to the read timeout.
    #[inline]
    pub fn read_timeout_mut(&mut self) -> &mut Option<Duration> {
        RequestConfig::<ReadTimeout>::get_mut(&mut self.extensions)
    }

    /// Get the http version.
    #[inline]
    pub fn version(&self) -> Version {
        self.version
    }

    /// Get a mutable reference to the http version.
    #[inline]
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.version
    }

    /// Attempt to clone the request.
    ///
    /// `None` is returned if the request can not be cloned, i.e. if the body is a stream.
    pub fn try_clone(&self) -> Option<Request> {
        let body = match self.body.as_ref() {
            Some(body) => Some(body.try_clone()?),
            None => None,
        };
        let mut req = Request::new(self.method().clone(), self.url().clone());
        *req.timeout_mut() = self.timeout().copied();
        *req.headers_mut() = self.headers().clone();
        *req.version_mut() = self.version();
        *req.extensions_mut() = self.extensions().clone();
        req.body = body;
        Some(req)
    }

    pub(super) fn pieces(self) -> (Method, Url, HeaderMap, Option<Body>, Version, Extensions) {
        (
            self.method,
            self.url,
            self.headers,
            self.body,
            self.version,
            self.extensions,
        )
    }
}

impl RequestBuilder {
    pub(super) fn new(client: Client, request: crate::Result<Request>) -> RequestBuilder {
        let mut builder = RequestBuilder { client, request };

        let auth = builder
            .request
            .as_mut()
            .ok()
            .and_then(|req| extract_authority(&mut req.url));

        if let Some((username, password)) = auth {
            builder.basic_auth(username, password)
        } else {
            builder
        }
    }

    /// Assemble a builder starting from an existing `Client` and a `Request`.
    pub fn from_parts(client: Client, request: Request) -> RequestBuilder {
        RequestBuilder {
            client,
            request: crate::Result::Ok(request),
        }
    }

    /// Add a `Header` to this Request.
    pub fn header<K, V>(self, key: K, value: V) -> RequestBuilder
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.header_sensitive(key, value, false)
    }

    /// Add a `Header` to this Request with ability to define if `header_value` is sensitive.
    fn header_sensitive<K, V>(mut self, key: K, value: V, sensitive: bool) -> RequestBuilder
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match <HeaderName as TryFrom<K>>::try_from(key) {
                Ok(key) => match <HeaderValue as TryFrom<V>>::try_from(value) {
                    Ok(mut value) => {
                        // We want to potentially make an non-sensitive header
                        // to be sensitive, not the reverse. So, don't turn off
                        // a previously sensitive header.
                        if sensitive {
                            value.set_sensitive(true);
                        }
                        req.headers_mut().append(key, value);
                    }
                    Err(e) => error = Some(crate::error::builder(e.into())),
                },
                Err(e) => error = Some(crate::error::builder(e.into())),
            };
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    /// Merge the given headers into any already set on this request.
    pub fn headers(mut self, headers: crate::header::HeaderMap) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            crate::util::replace_headers(req.headers_mut(), headers);
        }
        self
    }

    /// Enable HTTP basic authentication.
    ///
    /// ```rust
    /// # use primp::Error;
    ///
    /// # async fn run() -> Result<(), Error> {
    /// let client = primp::Client::new();
    /// let resp = client.delete("http://httpbin.org/delete")
    ///     .basic_auth("admin", Some("good password"))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn basic_auth<U, P>(self, username: U, password: Option<P>) -> RequestBuilder
    where
        U: fmt::Display,
        P: fmt::Display,
    {
        let header_value = crate::util::basic_auth(username, password);
        self.header_sensitive(crate::header::AUTHORIZATION, header_value, true)
    }

    /// Enable HTTP bearer authentication.
    pub fn bearer_auth<T>(self, token: T) -> RequestBuilder
    where
        T: fmt::Display,
    {
        let header_value = format!("Bearer {token}");
        self.header_sensitive(crate::header::AUTHORIZATION, header_value, true)
    }

    /// Set the request body.
    pub fn body<T: Into<Body>>(mut self, body: T) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *req.body_mut() = Some(body.into());
        }
        self
    }

    /// Sets a timeout for this request, from connection start until the response
    /// body finishes. Overrides `ClientBuilder::timeout()`.
    pub fn timeout(mut self, timeout: Duration) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *req.timeout_mut() = Some(timeout);
        }
        self
    }

    /// Sets a per-read timeout for this request's response body; each read that
    /// receives no data within the duration fails. Overrides `ClientBuilder::read_timeout()`.
    pub fn read_timeout(mut self, timeout: Duration) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *req.read_timeout_mut() = Some(timeout);
        }
        self
    }

    /// Override this request's redirect behavior, independent of the client's
    /// `redirect` policy. `Follow(n)` caps the chain at `n` hops;
    /// `Disabled` returns the 30x response as-is. The shared client is never
    /// mutated.
    pub fn redirect_override(mut self, override_policy: RedirectOverride) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *RequestConfig::<RedirectPolicyOverride>::get_mut(&mut req.extensions) =
                Some(override_policy);
        }
        self
    }

    /// Attach one-shot cookies to this request (not stored in the jar). While
    /// a plain explicit `Cookie` header suppresses jar injection for the whole
    /// redirect chain, these are re-merged with the fresh jar on every hop.
    pub fn one_shot_cookies(mut self, cookies: HeaderValue) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            *RequestConfig::<OneShotCookies>::get_mut(&mut req.extensions) = Some(cookies);
        }
        self
    }

    /// Sends a `multipart/form-data` body, also setting the `Content-Type`
    /// (boundary) and `Content-Length` headers.
    ///
    /// ```
    /// # use primp::Error;
    ///
    /// # async fn run() -> Result<(), Error> {
    /// let client = primp::Client::new();
    /// let form = primp::multipart::Form::new()
    ///     .text("key3", "value3")
    ///     .text("key4", "value4");
    ///
    ///
    /// let response = client.post("your url")
    ///     .multipart(form)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "multipart")]
    #[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
    pub fn multipart(self, mut multipart: multipart::Form) -> RequestBuilder {
        let mut error = None;
        let mut request = self.request;
        if let Ok(ref mut req) = request {
            // `insert` (not `header`/append): pre-set Content-Type and
            // Content-Length are preserved so a custom media type or a
            // known body size is not clobbered by the multipart defaults.
            let ct = format!("multipart/form-data; boundary={}", multipart.boundary());
            if !req.headers().contains_key(CONTENT_TYPE) {
                match HeaderValue::from_str(&ct) {
                    Ok(v) => {
                        req.headers_mut().insert(CONTENT_TYPE, v);
                    }
                    Err(e) => error = Some(crate::error::builder(e)),
                }
            }
            if let Some(length) = multipart.compute_length() {
                if !req.headers().contains_key(CONTENT_LENGTH) {
                    match HeaderValue::from_str(&length.to_string()) {
                        Ok(v) => {
                            req.headers_mut().insert(CONTENT_LENGTH, v);
                        }
                        Err(e) => error = Some(crate::error::builder(e)),
                    }
                }
            }
            *req.body_mut() = Some(multipart.stream());
        }
        if let Some(err) = error {
            request = Err(err);
        }
        RequestBuilder { request, ..self }
    }

    /// Append to the request URL's query string (existing keys are kept, not
    /// overwritten; `.query(&[("foo","a"),("foo","b")])` yields `foo=a&foo=b`).
    /// Use a sequence, not `.query(("k","v"))`; structs and maps are also supported.
    ///
    /// # Optional
    ///
    /// Requires the `query` feature.
    ///
    /// # Errors
    ///
    /// Fails if the value cannot be serialized into a query string.
    #[cfg(feature = "query")]
    #[cfg_attr(docsrs, doc(cfg(feature = "query")))]
    pub fn query<T: Serialize + ?Sized>(mut self, query: &T) -> RequestBuilder {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            let url = req.url_mut();
            let mut pairs = url.query_pairs_mut();
            let serializer = serde_urlencoded::Serializer::new(&mut pairs);

            if let Err(err) = query.serialize(serializer) {
                error = Some(crate::error::builder(err));
            }
        }
        if let Ok(ref mut req) = self.request {
            if let Some("") = req.url().query() {
                req.url_mut().set_query(None);
            }
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    /// Set HTTP version
    pub fn version(mut self, version: Version) -> RequestBuilder {
        if let Ok(ref mut req) = self.request {
            req.version = version;
        }
        self
    }

    /// Sends a `application/x-www-form-urlencoded` form body.
    ///
    /// ```rust
    /// # use primp::Error;
    /// # use std::collections::HashMap;
    /// #
    /// # async fn run() -> Result<(), Error> {
    /// let mut params = HashMap::new();
    /// params.insert("lang", "rust");
    ///
    /// let client = primp::Client::new();
    /// let res = client.post("http://httpbin.org")
    ///     .form(&params)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Optional
    ///
    /// This requires the optional `form` feature to be enabled.
    ///
    /// # Errors
    ///
    /// This method fails if the passed value cannot be serialized into
    /// url encoded format
    #[cfg(feature = "form")]
    #[cfg_attr(docsrs, doc(cfg(feature = "form")))]
    pub fn form<T: Serialize + ?Sized>(mut self, form: &T) -> RequestBuilder {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match serde_urlencoded::to_string(form) {
                Ok(body) => {
                    req.headers_mut()
                        .entry(CONTENT_TYPE)
                        .or_insert(HeaderValue::from_static(
                            "application/x-www-form-urlencoded",
                        ));
                    *req.body_mut() = Some(body.into());
                }
                Err(err) => error = Some(crate::error::builder(err)),
            }
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    /// Send a JSON body.
    ///
    /// # Optional
    ///
    /// This requires the optional `json` feature enabled.
    ///
    /// # Errors
    ///
    /// Serialization can fail if `T`'s implementation of `Serialize` decides to
    /// fail, or if `T` contains a map with non-string keys.
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn json<T: Serialize + ?Sized>(mut self, json: &T) -> RequestBuilder {
        let mut error = None;
        if let Ok(ref mut req) = self.request {
            match serde_json::to_vec(json) {
                Ok(body) => {
                    req.headers_mut()
                        .entry(CONTENT_TYPE)
                        .or_insert_with(|| HeaderValue::from_static("application/json"));
                    *req.body_mut() = Some(body.into());
                }
                Err(err) => error = Some(crate::error::json(err)),
            }
        }
        if let Some(err) = error {
            self.request = Err(err);
        }
        self
    }

    /// Build a `Request`, which can be inspected, modified and executed with
    /// `Client::execute()`.
    pub fn build(self) -> crate::Result<Request> {
        self.request
    }

    /// Like [`RequestBuilder::build()`], but also returns the embedded `Client`.
    pub fn build_split(self) -> (Client, crate::Result<Request>) {
        (self.client, self.request)
    }

    /// Constructs the Request and sends it to the target URL, returning a
    /// future Response.
    ///
    /// # Errors
    ///
    /// This method fails if there was an error while sending request,
    /// redirect loop was detected or redirect limit was exhausted.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use primp::Error;
    /// #
    /// # async fn run() -> Result<(), Error> {
    /// let response = primp::Client::new()
    ///     .get("https://hyper.rs")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send(self) -> impl Future<Output = Result<Response, crate::Error>> {
        match self.request {
            Ok(req) => self.client.execute_request(req),
            Err(err) => Pending::new_err(err),
        }
    }

    /// Attempt to clone the RequestBuilder.
    ///
    /// `None` is returned if the RequestBuilder can not be cloned,
    /// i.e. if the request body is a stream.
    ///
    /// # Examples
    ///
    /// ```
    /// # use primp::Error;
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let client = primp::Client::new();
    /// let builder = client.post("http://httpbin.org/post")
    ///     .body("from a &str!");
    /// let clone = builder.try_clone();
    /// assert!(clone.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_clone(&self) -> Option<RequestBuilder> {
        self.request
            .as_ref()
            .ok()
            .and_then(|req| req.try_clone())
            .map(|req| RequestBuilder {
                client: self.client.clone(),
                request: Ok(req),
            })
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_request_fields(&mut f.debug_struct("Request"), self).finish()
    }
}

impl fmt::Debug for RequestBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("RequestBuilder");
        match self.request {
            Ok(ref req) => fmt_request_fields(&mut builder, req).finish(),
            Err(ref err) => builder.field("error", err).finish(),
        }
    }
}

fn fmt_request_fields<'a, 'b>(
    f: &'a mut fmt::DebugStruct<'a, 'b>,
    req: &Request,
) -> &'a mut fmt::DebugStruct<'a, 'b> {
    f.field("method", &req.method)
        .field("url", &req.url)
        .field("headers", &req.headers)
}

/// Check the request URL for a "username:password" type authority, and if
/// found, remove it from the URL and return it.
pub(crate) fn extract_authority(url: &mut Url) -> Option<(String, Option<String>)> {
    use percent_encoding::percent_decode;

    if url.has_authority() {
        let username: String = percent_decode(url.username().as_bytes())
            .decode_utf8()
            .ok()?
            .into();
        let password = url.password().and_then(|pass| {
            percent_decode(pass.as_bytes())
                .decode_utf8()
                .ok()
                .map(String::from)
        });
        if !username.is_empty() || password.is_some() {
            // `has_authority()` implies a host, so these cannot fail with the
            // current url crate; ignore errors gracefully rather than aborting
            // the host (`panic = "abort"`) on a future url-crate change.
            if url.set_username("").is_err() || url.set_password(None).is_err() {
                return None;
            }
            return Some((username, password));
        }
    }

    None
}

impl<T> TryFrom<HttpRequest<T>> for Request
where
    T: Into<Body>,
{
    type Error = crate::Error;

    fn try_from(req: HttpRequest<T>) -> crate::Result<Self> {
        let (parts, body) = req.into_parts();
        let Parts {
            method,
            uri,
            headers,
            version,
            extensions,
            ..
        } = parts;
        let url = Url::parse(&uri.to_string()).map_err(crate::error::builder)?;
        Ok(Request {
            method,
            url,
            headers,
            body: Some(body.into()),
            version,
            extensions,
        })
    }
}

impl TryFrom<Request> for HttpRequest<Body> {
    type Error = crate::Error;

    fn try_from(req: Request) -> crate::Result<Self> {
        let Request {
            method,
            url,
            headers,
            body,
            version,
            extensions,
            ..
        } = req;

        let mut req = HttpRequest::builder()
            .version(version)
            .method(method)
            .uri(url.as_str())
            .body(body.unwrap_or_else(Body::empty))
            .map_err(crate::error::builder)?;

        *req.headers_mut() = headers;
        *req.extensions_mut() = extensions;
        Ok(req)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    #[cfg(feature = "query")]
    use std::collections::BTreeMap;

    #[test]
    #[cfg(feature = "query")]
    fn add_query_append() {
        let client = Client::new();
        let some_url = "https://www.google.com/";
        let r = client.get(some_url);

        let r = r.query(&[("foo", "bar")]);
        let r = r.query(&[("qux", 3)]);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=3"));
    }

    #[test]
    #[cfg(feature = "query")]
    fn add_query_append_same() {
        let client = Client::new();
        let some_url = "https://www.google.com/";
        let r = client.get(some_url);

        let r = r.query(&[("foo", "a"), ("foo", "b")]);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=a&foo=b"));
    }

    #[test]
    #[cfg(feature = "query")]
    fn add_query_struct() {
        #[derive(Serialize)]
        struct Params {
            foo: String,
            qux: i32,
        }

        let client = Client::new();
        let some_url = "https://www.google.com/";
        let r = client.get(some_url);

        let params = Params {
            foo: "bar".into(),
            qux: 3,
        };

        let r = r.query(&params);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=3"));
    }

    #[test]
    #[cfg(feature = "query")]
    fn add_query_map() {
        let mut params = BTreeMap::new();
        params.insert("foo", "bar");
        params.insert("qux", "three");

        let client = Client::new();
        let some_url = "https://www.google.com/";
        let r = client.get(some_url);

        let r = r.query(&params);

        let req = r.build().expect("request is valid");
        assert_eq!(req.url().query(), Some("foo=bar&qux=three"));
    }

    #[test]
    fn test_replace_headers() {
        use http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert("foo", "bar".parse().unwrap());
        headers.append("foo", "baz".parse().unwrap());

        let client = Client::new();
        let req = client
            .get("https://hyper.rs")
            .header("im-a", "keeper")
            .header("foo", "pop me")
            .headers(headers)
            .build()
            .expect("request build");

        assert_eq!(req.headers()["im-a"], "keeper");

        let foo = req.headers().get_all("foo").iter().collect::<Vec<_>>();
        assert_eq!(foo.len(), 2);
        assert_eq!(foo[0], "bar");
        assert_eq!(foo[1], "baz");
    }

    #[test]
    #[cfg(feature = "query")]
    fn normalize_empty_query() {
        let client = Client::new();
        let some_url = "https://www.google.com/";
        let empty_query: &[(&str, &str)] = &[];

        let req = client
            .get(some_url)
            .query(empty_query)
            .build()
            .expect("request build");

        assert_eq!(req.url().query(), None);
        assert_eq!(req.url().as_str(), "https://www.google.com/");
    }

    #[test]
    fn try_clone_reusable() {
        let client = Client::new();
        let builder = client
            .post("http://httpbin.org/post")
            .header("foo", "bar")
            .body("from a &str!");
        let req = builder
            .try_clone()
            .expect("clone successful")
            .build()
            .expect("request is valid");
        assert_eq!(req.url().as_str(), "http://httpbin.org/post");
        assert_eq!(req.method(), Method::POST);
        assert_eq!(req.headers()["foo"], "bar");
    }

    #[test]
    fn try_clone_no_body() {
        let client = Client::new();
        let builder = client.get("http://httpbin.org/get");
        let req = builder
            .try_clone()
            .expect("clone successful")
            .build()
            .expect("request is valid");
        assert_eq!(req.url().as_str(), "http://httpbin.org/get");
        assert_eq!(req.method(), Method::GET);
        assert!(req.body().is_none());
    }

    #[test]
    #[cfg(feature = "stream")]
    fn try_clone_stream() {
        let chunks: Vec<Result<_, ::std::io::Error>> = vec![Ok("hello"), Ok(" "), Ok("world")];
        let stream = futures_util::stream::iter(chunks);
        let client = Client::new();
        let builder = client
            .get("http://httpbin.org/get")
            .body(super::Body::wrap_stream(stream));
        let clone = builder.try_clone();
        assert!(clone.is_none());
    }

    #[test]
    fn convert_url_authority_into_basic_auth() {
        let client = Client::new();
        let some_url = "https://Aladdin:open sesame@localhost/";

        let req = client.get(some_url).build().expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(
            req.headers()["authorization"],
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
    }

    #[test]
    fn test_basic_auth_sensitive_header() {
        let client = Client::new();
        let some_url = "https://localhost/";

        let req = client
            .get(some_url)
            .basic_auth("Aladdin", Some("open sesame"))
            .build()
            .expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(
            req.headers()["authorization"],
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
        assert!(req.headers()["authorization"].is_sensitive());
    }

    #[test]
    fn test_bearer_auth_sensitive_header() {
        let client = Client::new();
        let some_url = "https://localhost/";

        let req = client
            .get(some_url)
            .bearer_auth("Hold my bear")
            .build()
            .expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(req.headers()["authorization"], "Bearer Hold my bear");
        assert!(req.headers()["authorization"].is_sensitive());
    }

    #[test]
    fn test_explicit_sensitive_header() {
        let client = Client::new();
        let some_url = "https://localhost/";

        let mut header = http::HeaderValue::from_static("in plain sight");
        header.set_sensitive(true);

        let req = client
            .get(some_url)
            .header("hiding", header)
            .build()
            .expect("request build");

        assert_eq!(req.url().as_str(), "https://localhost/");
        assert_eq!(req.headers()["hiding"], "in plain sight");
        assert!(req.headers()["hiding"].is_sensitive());
    }

    #[test]
    fn convert_from_http_request() {
        let http_request = HttpRequest::builder()
            .method("GET")
            .uri("http://localhost/")
            .header("User-Agent", "my-awesome-agent/1.0")
            .body("test test test")
            .unwrap();
        let req: Request = Request::try_from(http_request).unwrap();
        assert!(req.body().is_some());
        let test_data = b"test test test";
        assert_eq!(req.body().unwrap().as_bytes(), Some(&test_data[..]));
        let headers = req.headers();
        assert_eq!(headers.get("User-Agent").unwrap(), "my-awesome-agent/1.0");
        assert_eq!(req.method(), Method::GET);
        assert_eq!(req.url().as_str(), "http://localhost/");
    }

    #[test]
    fn set_http_request_version() {
        let http_request = HttpRequest::builder()
            .method("GET")
            .uri("http://localhost/")
            .header("User-Agent", "my-awesome-agent/1.0")
            .version(Version::HTTP_11)
            .body("test test test")
            .unwrap();
        let req: Request = Request::try_from(http_request).unwrap();
        assert!(req.body().is_some());
        let test_data = b"test test test";
        assert_eq!(req.body().unwrap().as_bytes(), Some(&test_data[..]));
        let headers = req.headers();
        assert_eq!(headers.get("User-Agent").unwrap(), "my-awesome-agent/1.0");
        assert_eq!(req.method(), Method::GET);
        assert_eq!(req.url().as_str(), "http://localhost/");
        assert_eq!(req.version(), Version::HTTP_11);
    }

    #[test]
    fn builder_split_reassemble() {
        let builder = {
            let client = Client::new();
            client.get("http://example.com")
        };
        let (client, inner) = builder.build_split();
        let request = inner.unwrap();
        let builder = RequestBuilder::from_parts(client, request);
        builder.build().unwrap();
    }

    #[test]
    fn extract_authority_never_panics_on_adversarial_userinfo() {
        // The `expect()`s in `set_username`/`set_password` were removed in
        // favor of graceful fallbacks: any URL shape the url crate accepts
        // must never abort the host (`panic = "abort"`). Includes invalid
        // UTF-8 percent-escapes, which bail out before the setter calls.
        for url in [
            "http://u:p@example.com/",
            "http://user@example.com/",
            "http://u%FF:p@example.com/",
            "http://user%FF@example.com/",
            "http://%FF%FE@example.com/",
            "http://u:p%FF@example.com/",
            "https://%C3%A9:p@example.com/",
            "http://example.com/", // no userinfo at all
        ] {
            let mut url = crate::Url::parse(url).expect("url must parse");
            let _ = extract_authority(&mut url);
        }
    }

    #[test]
    fn extract_authority_extracts_and_strips_userinfo() {
        let mut url = crate::Url::parse("http://alice:secret@example.com/").unwrap();
        let auth = extract_authority(&mut url).expect("userinfo present");
        assert_eq!(auth.0, "alice");
        assert_eq!(auth.1.as_deref(), Some("secret"));
        assert_eq!(url.username(), "");
        assert_eq!(url.password(), None);

        // Percent-encoded credentials round-trip through decoding.
        let mut url = crate::Url::parse("http://al%20ice:s%40cret@example.com/").unwrap();
        let auth = extract_authority(&mut url).expect("userinfo present");
        assert_eq!(auth.0, "al ice");
        assert_eq!(auth.1.as_deref(), Some("s@cret"));
    }

    #[test]
    fn builder_methods_set_method_and_url() {
        let client = Client::new();
        for (builder, method) in [
            (client.head("http://example.com/"), Method::HEAD),
            (client.put("http://example.com/"), Method::PUT),
            (client.patch("http://example.com/"), Method::PATCH),
            (client.delete("http://example.com/"), Method::DELETE),
        ] {
            let req = builder.build().expect("request is valid");
            assert_eq!(req.method(), method);
            assert_eq!(req.url().as_str(), "http://example.com/");
        }
    }

    #[test]
    #[cfg(feature = "form")]
    fn form_sets_content_type_and_urlencoded_body() {
        use std::collections::HashMap;

        let data = HashMap::from([("foo", "bar")]);
        let req = Client::new()
            .post("http://example.com/")
            .form(&data)
            .build()
            .expect("request is valid");

        assert_eq!(
            req.headers()[CONTENT_TYPE],
            "application/x-www-form-urlencoded"
        );
        let body = req.body().unwrap().as_bytes().expect("full body");
        let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(
            &String::from_utf8(body.to_vec()).expect("utf8 body"),
        )
        .expect("urlencoded body decodes");
        assert_eq!(pairs, vec![("foo".into(), "bar".into())]);
    }

    #[test]
    #[cfg(feature = "json")]
    fn json_sets_content_type_and_body() {
        use std::collections::HashMap;

        let data = HashMap::from([("foo", "bar")]);
        let req = Client::new()
            .post("http://example.com/")
            .json(&data)
            .build()
            .expect("request is valid");

        assert_eq!(req.headers()[CONTENT_TYPE], "application/json");
        let body = req.body().unwrap().as_bytes().expect("full body");
        let parsed: serde_json::Value = serde_json::from_slice(body).expect("valid json");
        assert_eq!(parsed, serde_json::json!({"foo": "bar"}));
    }

    #[test]
    #[cfg(feature = "form")]
    fn form_serialization_failure_is_builder_error() {
        use serde::Serializer;

        struct NeverSerializes;
        impl serde::Serialize for NeverSerializes {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                Err(serde::ser::Error::custom("nope"))
            }
        }

        let err = Client::new()
            .post("http://example.com/")
            .form(&NeverSerializes)
            .build()
            .expect_err("serialization must fail");
        assert!(err.is_builder());
    }

    #[test]
    #[cfg(feature = "json")]
    fn json_serialization_failure_is_json_error() {
        use serde::Serializer;

        struct NeverSerializes;
        impl serde::Serialize for NeverSerializes {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                Err(serde::ser::Error::custom("nope"))
            }
        }

        let err = Client::new()
            .post("http://example.com/")
            .json(&NeverSerializes)
            .build()
            .expect_err("serialization must fail");
        assert!(err.is_json());
    }
}
