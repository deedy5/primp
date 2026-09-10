use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;

use crate::error::PrimpErrorEnum;
use crate::error::PrimpResult;
use ::primp::Certificate;
use mime::Mime;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

/// Cached CA certs with freshness.
type CachedCerts = (std::time::SystemTime, u64, Vec<Certificate>);

/// Max cached CA bundles.
pub(crate) const CA_CACHE_CAPACITY: usize = 64;

/// Thread-safe CA cache keyed by path.
/// Check (mtime, size) without holding lock during IO.
static CA_CERT_CACHE: LazyLock<Mutex<std::collections::HashMap<PathBuf, CachedCerts>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Environment variables to check for CA certificate paths (in order).
const CA_CERT_ENV_VARS: &[&str] = &["PRIMP_CA_BUNDLE", "SSL_CERT_FILE", "CURL_CA_BUNDLE"];

/// Load cached CA certs from file.
pub fn load_ca_certs_from_file(ca_cert_path: &Path) -> PrimpResult<Vec<Certificate>> {
    // Fast path: stat only, under a short lock. The lock is dropped before
    // any file IO so concurrent clients never block on reads/parses.
    let (mdate, size) = {
        let meta = std::fs::metadata(ca_cert_path).map_err(|e| {
            PrimpErrorEnum::Builder(format!(
                "failed to read CA cert file '{}': {e}",
                ca_cert_path.display()
            ))
        })?;
        (meta.modified().unwrap_or(std::time::UNIX_EPOCH), meta.len())
    };
    if let Some((cached_mtime, cached_size, cached_certs)) = CA_CERT_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(ca_cert_path)
    {
        if *cached_mtime == mdate && *cached_size == size {
            return Ok(cached_certs.clone());
        }
    }

    // Slow path: read + parse without holding the lock, then store.
    let cert_file = std::fs::read(ca_cert_path).map_err(|e| {
        PrimpErrorEnum::Builder(format!(
            "failed to read CA cert file '{}': {e}",
            ca_cert_path.display()
        ))
    })?;
    let certs = Certificate::from_pem_bundle(&cert_file).map_err(|e| {
        PrimpErrorEnum::Builder(format!(
            "failed to parse CA cert file '{}': {e}",
            ca_cert_path.display()
        ))
    })?;
    if certs.is_empty() {
        return Err(PrimpErrorEnum::Builder(format!(
            "failed to parse CA cert file '{}': no certificates found",
            ca_cert_path.display()
        )));
    }

    // Re-stat after the read: if the file changed under us, the bytes we
    // parsed no longer match the pre-read stamp — skip caching rather than
    // poisoning the entry with a mismatched stamp.
    let (mdate_now, size_now) = match std::fs::metadata(ca_cert_path) {
        Ok(meta) => (meta.modified().unwrap_or(std::time::UNIX_EPOCH), meta.len()),
        Err(_) => return Ok(certs),
    };
    if mdate_now != mdate || size_now != cert_file.len() as u64 {
        return Ok(certs);
    }

    let mut cache = CA_CERT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= CA_CACHE_CAPACITY {
        cache.clear();
    }
    cache.insert(
        ca_cert_path.to_path_buf(),
        (mdate_now, size_now, certs.clone()),
    );

    Ok(certs)
}

/// Load CA certs from env (`None` for system roots).
fn load_ca_certs_from_env() -> Option<Vec<Certificate>> {
    for env_var in CA_CERT_ENV_VARS {
        if let Ok(ca_cert_path) = std::env::var(env_var) {
            let path = Path::new(&ca_cert_path);
            if !path.exists() {
                tracing::warn!(
                    "CA bundle env {} points to missing file '{}'; falling back to system roots",
                    env_var,
                    ca_cert_path
                );
                continue;
            }
            tracing::debug!("Loading CA certs from env var: {}", env_var);
            match load_ca_certs_from_file(path) {
                Ok(certs) => return Some(certs),
                Err(e) => {
                    tracing::warn!(
                        "Failed to load CA certs from {}='{}': {e}; falling back to system roots",
                        env_var,
                        ca_cert_path
                    );
                }
            }
        }
    }
    None
}

/// Load CA certs; `None` for system roots, error on bad path.
pub fn load_ca_certs(ca_cert_file: &Option<String>) -> PrimpResult<Option<Vec<Certificate>>> {
    // If ca_cert_file is provided, load from that file (error on failure)
    if let Some(ca_cert_path) = ca_cert_file {
        tracing::debug!("Loading CA certs from file: {}", ca_cert_path);
        let certs = load_ca_certs_from_file(Path::new(ca_cert_path))?;
        return Ok(Some(certs));
    }

    // Try to load from environment variables
    Ok(load_ca_certs_from_env())
}

/// Encoding from the `Content-Type` charset parameter, or UTF-8 if absent.
pub fn extract_encoding(headers: &::primp::header::HeaderMap) -> &'static encoding_rs::Encoding {
    headers
        .get(::primp::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.parse::<Mime>().ok().and_then(|mime| {
                mime.get_param("charset")
                    .and_then(|c| encoding_rs::Encoding::for_label(c.as_str().as_bytes()))
            })
        })
        .unwrap_or(encoding_rs::UTF_8)
}

/// Join `url` onto `base_url`, keeping base path for `/`.
/// Fallback to string concat if parse fails.
pub fn resolve_url(base_url: Option<&str>, url: &str) -> String {
    if let Some(base) = base_url {
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            // Leading slash keeps prefix; normalize dot-segments.
            if url.starts_with('/') {
                // Strip base query/fragment (else "/users" appends after "?tok=1").
                let base = base
                    .split(['?', '#'])
                    .next()
                    .unwrap_or(base)
                    .trim_end_matches('/');
                let path = url.trim_start_matches('/');
                let joined = format!("{base}/{path}");
                if let Ok(u) = url::Url::parse(&joined) {
                    return u.to_string();
                }
                return joined;
            }
            // Base with query/fragment resolves against its path.
            let normalized = if let Ok(mut b) = url::Url::parse(base) {
                b.set_query(None);
                b.set_fragment(None);
                let mut s = b.to_string();
                if !s.ends_with('/') {
                    s.push('/');
                }
                s
            } else if base.ends_with('/') {
                base.to_string()
            } else {
                format!("{base}/")
            };
            if let Ok(base_url) = url::Url::parse(&normalized) {
                if let Ok(joined) = base_url.join(url) {
                    return joined.to_string();
                }
            }
            let base = base.trim_end_matches('/');
            let path = url.trim_start_matches('/');
            return format!("{base}/{path}");
        }
    }
    url.to_string()
}

/// Seconds to `Duration`; rejects NaN/negative/infinite.
pub(crate) fn timeout_duration(seconds: f64) -> PrimpResult<Duration> {
    Duration::try_from_secs_f64(seconds).map_err(|_| {
        PrimpErrorEnum::Builder(format!(
            "timeout must be a finite, non-negative number of seconds, got {seconds}"
        ))
    })
}

/// Is JSON value a non-empty body?
pub(crate) fn body_value_present(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(m) => !m.is_empty(),
        _ => true,
    }
}

/// Body by requests/httpx priority: `content` > `files`+`data` > `data` > `json`.
#[derive(Debug, PartialEq)]
pub enum ResolvedBody {
    None,
    Content(Vec<u8>),
    Multipart {
        files: indexmap::IndexMap<String, String>,
        data: Option<serde_json::Value>,
    },
    Form(serde_json::Value),
    RawBody(String),
    Json(serde_json::Value),
}

/// Pick one body to send; empties count as absent.
/// Priority: `content` > `files`+`data` > `data` > `json`.
pub fn resolve_body(
    content: Option<Vec<u8>>,
    data: Option<serde_json::Value>,
    json: Option<serde_json::Value>,
    files: Option<indexmap::IndexMap<String, String>>,
) -> ResolvedBody {
    let content = content.filter(|c| !c.is_empty());
    let data = data.filter(body_value_present);
    let json = json.filter(body_value_present);
    let files = files.filter(|f| !f.is_empty());
    if let Some(b) = content {
        return ResolvedBody::Content(b);
    }
    if let Some(f) = files {
        return ResolvedBody::Multipart { files: f, data };
    }
    if let Some(d) = data {
        if d.is_object() {
            return ResolvedBody::Form(d);
        }
        let s = crate::body_value_to_string(&d);
        return ResolvedBody::RawBody(s);
    }
    if let Some(j) = json {
        return ResolvedBody::Json(j);
    }
    ResolvedBody::None
}

/// JSON type name for errors.
fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Build multipart form from `files` + `data`.
/// Mapping `data` combines with `files`;
/// scalar `data` errors like requests.
pub async fn build_multipart_form(
    files: indexmap::IndexMap<String, String>,
    data: Option<serde_json::Value>,
) -> Result<::primp::multipart::Form, PrimpErrorEnum> {
    let mut form = ::primp::multipart::Form::new();
    if let Some(d) = data {
        match d {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    form = form.text(k, crate::body_value_to_string(&v));
                }
            }
            other => {
                let kind = json_type_name(&other);
                return Err(PrimpErrorEnum::Builder(format!(
                    "data must be a mapping of form fields when files are provided (got {kind})"
                )));
            }
        }
    }
    for (field_name, file_path) in files {
        let file = File::open(&file_path).await.map_err(PrimpErrorEnum::from)?;
        let stream = FramedRead::new(file, BytesCodec::new());
        let file_body = ::primp::Body::wrap_stream(stream);
        // Use basename of file_path as uploaded filename, not field name.
        let filename = std::path::Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&field_name)
            .to_string();
        let part = ::primp::multipart::Part::stream(file_body).file_name(filename);
        form = form.part(field_name, part);
    }
    Ok(form)
}

#[cfg(test)]
mod load_ca_certs_tests {
    use super::*;
    use tempfile::NamedTempFile;

    const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----
MIIC/zCCAeegAwIBAgIUdSMXyCRA8Nwi3nupoR1W6uDykpkwDQYJKoZIhvcNAQEL
BQAwDzENMAsGA1UEAwwEdGVzdDAeFw0yNjA3MDIyMDI5NTJaFw0zNjA2MjkyMDI5
NTJaMA8xDTALBgNVBAMMBHRlc3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEK
AoIBAQDJxhnIzKN+rgvse3Dute3GxXT5apL+HSX7gOZlTJuZ02QYcb75FwnOTieR
1G9MmbYMZjp12q5CNtH06+SGdRNBDbHLso59PKcGLjUz0JIhxgobXLwmJFaQWHd7
7fEdrtEarTi4vPffkNPXDvMVQ5vjv2CTXs/r9Y+t7Tjn3DWzHHjp9lKn8947m5sR
5rB/KK7GdraI/ghSw1IiBENSL6Nfz5lZYETKLLZeCBKiAXmD3w+SDoaaTPblPCyW
Yd66U7C6ZnmQhcjz2V32mPMQl8wAFu1OTS3ixnqyRyvv6VtyPjyfdKU1ilWtdpGY
IycBNHnUOcPYWVyT0IEZWG4+/+r7AgMBAAGjUzBRMB0GA1UdDgQWBBR0PT1v7HwD
bpvFV3dsArng0FUsHzAfBgNVHSMEGDAWgBR0PT1v7HwDbpvFV3dsArng0FUsHzAP
BgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQChrZCGBSeUFTIkih5R
akeIvdpmnNWmSsqGh03NDOmddudtjS9U9n8rRcJKOBQzzIj6XuPm4qx6rwgldh6Y
IcUm9TAgPQRxZzrPWRQkAZHrRTo+5UKhglXsusvDUuiRdYHuslchZcLcJD4trrJd
LAxsBcPBkbxbolaABK2/tTI2qmOdUUywgwLMu3XYPVyKVPztijcTUWcrpfRJjhdQ
fsS6b/vdr6CvJCbSed0IdnHXbgauIWiLDlVopmfGzRIDhKhzJh4y82VaPUMynWvd
Cf12wr9rBJm9bEcvZnMbm8PQ0O+oaS6i50Nfm+Qy2gAsJc9gUi8G79MrX67AxI+V
PW3u
-----END CERTIFICATE-----";

    fn temp_pem(name: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new()
            .prefix(name)
            .suffix(".pem")
            .tempfile()
            .unwrap();
        std::io::Write::write_all(&mut f, TEST_CERT.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_load_ca_certs_from_file() {
        let file = temp_pem("test_ca_cert");
        let result = load_ca_certs_from_file(file.path());
        assert!(result.is_ok());
        assert!(result.unwrap().len() > 0);
    }

    #[test]
    fn test_load_ca_certs_with_ca_cert_file_param() {
        let file = temp_pem("test_ca_cert2");
        let path = file.path().to_str().unwrap().to_string();
        let result = load_ca_certs(&Some(path));
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_load_ca_certs_with_none() {
        let result = load_ca_certs(&None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_ca_certs_with_bad_path_fails() {
        let result = load_ca_certs(&Some("/nonexistent/path/cert.pem".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_ca_cache_detects_same_mtime_rotation_by_size() {
        let mut f = tempfile::Builder::new()
            .prefix("test_ca_rotate")
            .suffix(".pem")
            .tempfile()
            .unwrap();
        std::io::Write::write_all(&mut f, TEST_CERT.as_bytes()).unwrap();
        let path = f.path().to_path_buf();
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();

        // Prime the cache.
        assert!(load_ca_certs_from_file(&path).is_ok());

        // Rotate in place to garbage of a different size but stamp the same
        // mtime back: mtime-only freshness would stale-hit, (mtime, size)
        // must re-read and fail parsing.
        std::fs::write(&path, b"not a pem at all, different length garbage").unwrap();
        let fh = std::fs::File::options().write(true).open(&path).unwrap();
        fh.set_modified(mtime).unwrap();

        assert!(load_ca_certs_from_file(&path).is_err());
    }

    #[test]
    fn test_ca_cache_is_bounded() {
        // Distinct temp paths must not grow the cache without limit.
        let dir = tempfile::tempdir().unwrap();
        for i in 0..300 {
            let p = dir.path().join(format!("ca-{i}.pem"));
            std::fs::write(&p, TEST_CERT.as_bytes()).unwrap();
            assert!(load_ca_certs_from_file(&p).is_ok());
        }
        let len = super::CA_CERT_CACHE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len();
        assert!(
            len <= super::CA_CACHE_CAPACITY,
            "CA cache grew without bound: {len} entries"
        );
    }
}

#[cfg(test)]
mod resolve_body_tests {
    use super::resolve_body;
    use super::ResolvedBody;
    use serde_json::{json, Value};

    #[test]
    fn body_value_present_matrix() {
        // Empty null/string/array/object count as absent; scalars count.
        use super::body_value_present;
        assert!(!body_value_present(&json!({})));
        assert!(!body_value_present(&json!([])));
        assert!(!body_value_present(&Value::Null));
        assert!(!body_value_present(&json!("")));
        assert!(body_value_present(&json!({"a": 1})));
        assert!(body_value_present(&json!([1])));
        assert!(body_value_present(&json!(0)));
        assert!(body_value_present(&json!(false)));
    }

    #[test]
    fn content_wins_over_data_files_json() {
        let mut files: indexmap::IndexMap<String, String> = Default::default();
        files.insert("f".into(), "p".into());
        match resolve_body(
            Some(vec![1u8, 2]),
            Some(json!({"a": "1"})),
            Some(json!({"b": 2})),
            Some(files),
        ) {
            ResolvedBody::Content(b) => assert_eq!(b, vec![1u8, 2]),
            other => panic!("content must win, got {other:?}"),
        }
    }

    #[test]
    fn files_combine_with_data_and_ignore_json() {
        let mut files: indexmap::IndexMap<String, String> = Default::default();
        files.insert("f".into(), "p".into());
        match resolve_body(
            None,
            Some(json!({"a": "1"})),
            Some(json!({"b": 2})),
            Some(files),
        ) {
            ResolvedBody::Multipart { files: f, data: d } => {
                assert_eq!(f.len(), 1);
                assert_eq!(d, Some(json!({"a": "1"})));
            }
            other => panic!("files+data must be multipart, got {other:?}"),
        }
    }

    #[test]
    fn files_alone_ignore_json() {
        let mut files: indexmap::IndexMap<String, String> = Default::default();
        files.insert("f".into(), "p".into());
        match resolve_body(None, None, Some(json!({"b": 2})), Some(files)) {
            ResolvedBody::Multipart { files: f, data: d } => {
                assert_eq!(f.len(), 1);
                assert!(d.is_none());
            }
            other => panic!("files alone must be multipart, got {other:?}"),
        }
    }

    #[test]
    fn data_object_beats_json() {
        match resolve_body(None, Some(json!({"a": "1"})), Some(json!({"b": 2})), None) {
            ResolvedBody::Form(d) => assert_eq!(d, json!({"a": "1"})),
            other => panic!("data must beat json, got {other:?}"),
        }
    }

    #[test]
    fn data_scalar_becomes_raw_body() {
        match resolve_body(None, Some(json!("hello")), None, None) {
            ResolvedBody::RawBody(s) => assert_eq!(s, "hello"),
            other => panic!("scalar data must be raw, got {other:?}"),
        }
    }

    #[test]
    fn json_alone_sends_json() {
        match resolve_body(None, None, Some(json!({"b": 2})), None) {
            ResolvedBody::Json(j) => assert_eq!(j, json!({"b": 2})),
            other => panic!("json alone must send, got {other:?}"),
        }
    }

    #[test]
    fn empty_values_are_absent() {
        let empty_files: indexmap::IndexMap<String, String> = Default::default();
        assert_eq!(
            resolve_body(
                Some(vec![]),
                Some(json!({})),
                Some(json!([])),
                Some(empty_files)
            ),
            ResolvedBody::None
        );
        // Empty content must not shadow real json.
        match resolve_body(Some(vec![]), None, Some(json!({"a": 1})), None) {
            ResolvedBody::Json(j) => assert_eq!(j, json!({"a": 1})),
            other => panic!("empty content must not shadow json, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod resolve_url_tests {
    use super::resolve_url;

    #[test]
    fn preserves_version_prefix_on_leading_slash() {
        // Legacy concat kept /v1/ for both "/users" and "users".
        // Url::join drops it for "/users" -> breaking change.
        assert_eq!(
            resolve_url(Some("https://h/v1/"), "/users"),
            "https://h/v1/users"
        );
        assert_eq!(
            resolve_url(Some("https://h/v1/"), "users"),
            "https://h/v1/users"
        );
    }

    #[test]
    fn handles_query_and_relative() {
        assert_eq!(
            resolve_url(Some("https://h/v1/"), "users?page=1"),
            "https://h/v1/users?page=1"
        );
    }

    #[test]
    fn preserves_version_prefix_without_trailing_slash() {
        // Legacy (pre-Url::join) behavior was pure prefix concat, so a base
        // without trailing slash kept its last segment. Url::join would drop
        // it via RFC 3986 file-replacement — a breaking change.
        assert_eq!(
            resolve_url(Some("https://h/v1"), "users"),
            "https://h/v1/users"
        );
        assert_eq!(
            resolve_url(Some("https://h/v1"), "users?page=1"),
            "https://h/v1/users?page=1"
        );
        assert_eq!(resolve_url(Some("https://h"), "users"), "https://h/users");
    }

    #[test]
    fn handles_dotdot_query_fragment_and_empty() {
        // Relative paths (no leading `/`) use RFC 3986 join.
        assert_eq!(resolve_url(Some("https://h/v1"), "../x"), "https://h/x");
        assert_eq!(
            resolve_url(Some("https://h/v1/"), "?q=1"),
            "https://h/v1/?q=1"
        );
        assert_eq!(resolve_url(Some("https://h/v1/"), "#f"), "https://h/v1/#f");
        assert_eq!(resolve_url(Some("https://h/v1/"), ""), "https://h/v1/");
    }

    #[test]
    fn leading_slash_dotdot_is_normalized() {
        // Dot-segments still normalize.
        assert_eq!(resolve_url(Some("https://h/v1/"), "/../x"), "https://h/x");
    }

    #[test]
    fn base_with_query_resolves_path_sensibly() {
        // Query/fragment on base is dropped.
        assert_eq!(
            resolve_url(Some("https://h/v1?tok=1"), "users"),
            "https://h/v1/users"
        );
    }

    #[test]
    fn leading_slash_strips_base_query_fragment() {
        // Leading slash must drop base query/fragment.
        assert_eq!(
            resolve_url(Some("https://h/v1?tok=1"), "/users"),
            "https://h/v1/users"
        );
        assert_eq!(
            resolve_url(Some("https://h/v1#f"), "/users"),
            "https://h/v1/users"
        );
        assert_eq!(
            resolve_url(Some("https://h/v1?tok=1#f"), "/users?page=2"),
            "https://h/v1/users?page=2"
        );
    }

    #[test]
    fn absolute_urls_pass_through_verbatim() {
        assert_eq!(
            resolve_url(Some("https://h/v1"), "HTTP://other/x"),
            "HTTP://other/x"
        );
        assert_eq!(
            resolve_url(Some("https://h/v1"), "https://other/x?q=1#f"),
            "https://other/x?q=1#f"
        );
        assert_eq!(resolve_url(None, "https://h/x"), "https://h/x");
    }
}
