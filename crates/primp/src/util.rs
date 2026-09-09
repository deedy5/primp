use crate::header::{Entry, HeaderMap, HeaderValue, OccupiedEntry};
use std::fmt;
use std::sync::{Mutex, MutexGuard};

/// Recover from a poisoned mutex: clear the poison and return the inner guard,
/// so a panic in one task can't kill every later caller of the same lock.
pub(crate) fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn basic_auth<U, P>(username: U, password: Option<P>) -> HeaderValue
where
    U: fmt::Display,
    P: fmt::Display,
{
    use base64::prelude::BASE64_STANDARD;
    use base64::write::EncoderWriter;
    use std::io::Write;

    let mut buf = b"Basic ".to_vec();
    {
        let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
        let _ = write!(encoder, "{username}:");
        if let Some(password) = password {
            let _ = write!(encoder, "{password}");
        }
    }
    // base64 output is always ASCII, so this never fails; the fallback
    // avoids panicking on malformed input.
    let mut header = HeaderValue::from_maybe_shared(bytes::Bytes::from(buf))
        .unwrap_or_else(|_| HeaderValue::from_static("Basic "));
    header.set_sensitive(true);
    header
}

pub(crate) fn fast_random() -> u64 {
    use std::cell::Cell;
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    thread_local! {
        static KEY: RandomState = RandomState::new();
        static COUNTER: Cell<u64> = const { Cell::new(0) };
    }

    KEY.with(|key| {
        COUNTER.with(|ctr| {
            let n = ctr.get().wrapping_add(1);
            ctr.set(n);

            let mut h = key.build_hasher();
            h.write_u64(n);
            h.finish()
        })
    })
}

pub(crate) fn replace_headers(dst: &mut HeaderMap, src: HeaderMap) {
    // IntoIter of HeaderMap yields (Option<HeaderName>, HeaderValue).
    // The first time a name is yielded, it will be Some(name), and if
    // there are more values with the same name, the next yield will be
    // None.

    let mut prev_entry: Option<OccupiedEntry<_>> = None;
    for (key, value) in src {
        match key {
            Some(key) => match dst.entry(key) {
                Entry::Occupied(mut e) => {
                    e.insert(value);
                    prev_entry = Some(e);
                }
                Entry::Vacant(e) => {
                    let e = e.insert_entry(value);
                    prev_entry = Some(e);
                }
            },
            None => match prev_entry {
                Some(ref mut entry) => {
                    entry.append(value);
                }
                None => unreachable!("HeaderMap::into_iter yielded None first"),
            },
        }
    }
}

#[cfg(feature = "cookies")]
pub(crate) fn add_cookie_header(
    headers: &mut HeaderMap,
    cookie_store: &dyn crate::cookie::CookieStore,
    url: &url::Url,
) {
    if let Some(header) = cookie_store.cookies(url) {
        headers.insert(crate::header::COOKIE, header);
    }
}

/// Strip CTLs, trim OWS, keep visible text.
#[cfg(feature = "cookies")]
pub(crate) fn sanitize_cookie_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut sanitized: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|&b| b == b' ' || b == b'\t' || (0x20..=0x7e).contains(&b) || b >= 0x80)
        .collect();
    let leading = sanitized
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    sanitized.drain(..leading);
    while matches!(sanitized.last(), Some(b' ') | Some(b'\t')) {
        sanitized.pop();
    }
    sanitized
}

/// Merge jar + one-shot cookies (bytes, keeps non-UTF8).
#[cfg(feature = "cookies")]
pub(crate) fn merge_one_shot_cookie_header(
    cookie_store: &dyn crate::cookie::CookieStore,
    url: &url::Url,
    one_shot: &HeaderValue,
) -> HeaderValue {
    fn trim_ows(mut s: &[u8]) -> &[u8] {
        while matches!(s.first(), Some(b' ') | Some(b'\t')) {
            s = &s[1..];
        }
        while matches!(s.last(), Some(b' ') | Some(b'\t')) {
            s = &s[..s.len() - 1];
        }
        s
    }
    fn pair_name(pair: &[u8]) -> &[u8] {
        trim_ows(pair.split(|&b| b == b'=').next().unwrap_or(b""))
    }

    let one_shot_bytes = one_shot.as_bytes();
    let one_shot_names: std::collections::HashSet<&[u8]> = one_shot_bytes
        .split(|&b| b == b';')
        .map(pair_name)
        .filter(|name| !name.is_empty())
        .collect();

    let mut out: Vec<u8> = Vec::new();
    if let Some(jar) = cookie_store.cookies(url) {
        for pair in jar.as_bytes().split(|&b| b == b';') {
            let name = pair_name(pair);
            if name.is_empty() || one_shot_names.contains(name) {
                continue;
            }
            if !out.is_empty() {
                out.extend_from_slice(b"; ");
            }
            out.extend_from_slice(trim_ows(pair));
        }
    }
    let one_shot_trimmed = trim_ows(one_shot_bytes);
    if !one_shot_trimmed.is_empty() {
        if !out.is_empty() {
            out.extend_from_slice(b"; ");
        }
        out.extend_from_slice(one_shot_trimmed);
    }

    match HeaderValue::from_bytes(&out) {
        Ok(hv) => hv,
        Err(_) => {
            // Invalid combined header: sanitize, fallback to one-shot.
            let sanitized = sanitize_cookie_bytes(&out);
            HeaderValue::from_bytes(&sanitized).unwrap_or_else(|_| one_shot.clone())
        }
    }
}

pub(crate) struct Escape<'a>(&'a [u8]);

impl<'a> Escape<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Escape(bytes)
    }
}

impl fmt::Debug for Escape<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "b\"{}\"", self)?;
        Ok(())
    }
}

impl fmt::Display for Escape<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &c in self.0 {
            // https://doc.rust-lang.org/reference.html#byte-escapes
            if c == b'\n' {
                write!(f, "\\n")?;
            } else if c == b'\r' {
                write!(f, "\\r")?;
            } else if c == b'\t' {
                write!(f, "\\t")?;
            } else if c == b'\\' || c == b'"' {
                write!(f, "\\{}", c as char)?;
            } else if c == b'\0' {
                write!(f, "\\0")?;
            // ASCII printable
            } else if (0x20..0x7f).contains(&c) {
                write!(f, "{}", c as char)?;
            } else {
                write!(f, "\\x{c:02x}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn recover_lock_returns_guard_on_unpoisoned_mutex() {
        let mutex = Mutex::new(42);
        let guard = recover_lock(&mutex);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn recover_lock_recovers_from_poisoned_mutex() {
        let mutex = Arc::new(Mutex::new(0u32));
        let m = Arc::clone(&mutex);
        let _ = thread::spawn(move || {
            let _guard = m.lock().unwrap();
            panic!("intentional panic to poison the mutex");
        })
        .join();

        // Mutex is now poisoned. `lock().unwrap()` would panic, but
        // `recover_lock` should clear the poison and return the guard.
        assert!(mutex.lock().is_err(), "mutex should be poisoned");
        let mut guard = recover_lock(&mutex);
        assert_eq!(*guard, 0);
        *guard = 42;
    }

    #[cfg(feature = "cookies")]
    #[test]
    fn merge_preserves_non_utf8_cookie_bytes() {
        use crate::cookie::CookieStore;

        struct FixedJar(Option<crate::header::HeaderValue>);
        impl CookieStore for FixedJar {
            fn set_cookies(
                &self,
                _cookie_headers: &mut dyn Iterator<Item = &crate::header::HeaderValue>,
                _url: &url::Url,
            ) {
            }
            fn cookies(&self, _url: &url::Url) -> Option<crate::header::HeaderValue> {
                self.0.clone()
            }
        }

        let url = url::Url::parse("http://example.com/").unwrap();
        // 0xFF is valid obs-text in a HeaderValue but invalid UTF-8: a lossy
        // `to_str` conversion would rewrite it to U+FFFD (E2 BF BD).
        let jar = FixedJar(Some(
            crate::header::HeaderValue::from_bytes(b"a=\xff; b=2").unwrap(),
        ));
        let one_shot = crate::header::HeaderValue::from_static("c=3");
        let merged = super::merge_one_shot_cookie_header(&jar, &url, &one_shot);
        assert_eq!(merged.as_bytes(), b"a=\xff; b=2; c=3");
    }

    #[cfg(feature = "cookies")]
    #[test]
    fn merge_one_shot_overrides_jar_names() {
        use crate::cookie::CookieStore;

        struct FixedJar(Option<crate::header::HeaderValue>);
        impl CookieStore for FixedJar {
            fn set_cookies(
                &self,
                _cookie_headers: &mut dyn Iterator<Item = &crate::header::HeaderValue>,
                _url: &url::Url,
            ) {
            }
            fn cookies(&self, _url: &url::Url) -> Option<crate::header::HeaderValue> {
                self.0.clone()
            }
        }

        let url = url::Url::parse("http://example.com/").unwrap();
        let jar = FixedJar(Some(crate::header::HeaderValue::from_static("a=1; b=2")));
        let one_shot = crate::header::HeaderValue::from_static("b=override");
        let merged = super::merge_one_shot_cookie_header(&jar, &url, &one_shot);
        assert_eq!(merged.as_bytes(), b"a=1; b=override");
    }

    #[cfg(feature = "cookies")]
    #[test]
    fn sanitize_cookie_bytes_strips_ctls() {
        // Err branch unreachable via safe API; test helper directly.
        use super::sanitize_cookie_bytes;
        // Stripped: 0x01/1F/7F/NUL/CR; kept: SP/HTAB/obs-text.
        assert_eq!(sanitize_cookie_bytes(b"a=\x01bad; b=2"), b"a=bad; b=2");
        assert_eq!(sanitize_cookie_bytes(b"a=1\x1f; b=2"), b"a=1; b=2");
        assert_eq!(sanitize_cookie_bytes(b"a=1\x7f; b=2"), b"a=1; b=2");
        assert_eq!(sanitize_cookie_bytes(b"a=1\n; b=2\r"), b"a=1; b=2");
        // SP/HTAB preserved inside, OWS trimmed at edges.
        assert_eq!(sanitize_cookie_bytes(b"  a=1\t; b=2  "), b"a=1\t; b=2");
        assert_eq!(sanitize_cookie_bytes(b"\ta=\xff; b=2"), b"a=\xff; b=2");
        // 0x80+ obs-text preserved.
        assert_eq!(sanitize_cookie_bytes(b"a=\xff; b=\x80"), b"a=\xff; b=\x80");
        assert!(crate::header::HeaderValue::from_bytes(b"a=\x01bad").is_err());
    }

    #[cfg(feature = "cookies")]
    #[test]
    fn merge_sanitize_branch_strips_ctls() {
        use crate::cookie::CookieStore;

        struct FixedJar(Option<crate::header::HeaderValue>);
        impl CookieStore for FixedJar {
            fn set_cookies(
                &self,
                _cookie_headers: &mut dyn Iterator<Item = &crate::header::HeaderValue>,
                _url: &url::Url,
            ) {
            }
            fn cookies(&self, _url: &url::Url) -> Option<crate::header::HeaderValue> {
                self.0.clone()
            }
        }

        let url = url::Url::parse("http://example.com/").unwrap();
        // Valid jars take Ok path.
        let jar = FixedJar(Some(crate::header::HeaderValue::from_static("a=1; b=2")));
        let one_shot = crate::header::HeaderValue::from_static("c=3");
        let merged = super::merge_one_shot_cookie_header(&jar, &url, &one_shot);
        assert_eq!(merged.as_bytes(), b"a=1; b=2; c=3");
    }
}
