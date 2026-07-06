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
    // base64 emits only visible ASCII, so this conversion can never fail;
    // the fallback keeps a malformed username from aborting the process
    // (`panic = "abort"`) on the request path.
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

/// Rebuild the `Cookie` header for a request carrying one-shot cookies: the
/// jar's CURRENT cookies (fresh per redirect hop, minus any name the one-shot
/// set overrides) followed by the one-shot cookies. Falls back to the
/// one-shots alone if the jar produces an invalid header value — never panics.
#[cfg(feature = "cookies")]
pub(crate) fn merge_one_shot_cookie_header(
    cookie_store: &dyn crate::cookie::CookieStore,
    url: &url::Url,
    one_shot: &HeaderValue,
) -> HeaderValue {
    let one_shot_str = one_shot.to_str().unwrap_or_default();
    let one_shot_names: std::collections::HashSet<&str> = one_shot_str
        .split(';')
        .filter_map(|pair| {
            let name = pair.split('=').next().unwrap_or("").trim();
            (!name.is_empty()).then_some(name)
        })
        .collect();

    let mut out = String::new();
    if let Some(jar) = cookie_store.cookies(url) {
        for pair in jar.to_str().unwrap_or_default().split(';') {
            let pair = pair.trim();
            let name = pair.split('=').next().unwrap_or("").trim();
            if name.is_empty() || one_shot_names.contains(name) {
                continue;
            }
            if !out.is_empty() {
                out.push_str("; ");
            }
            out.push_str(pair);
        }
    }
    if !one_shot_str.is_empty() {
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(one_shot_str.trim());
    }

    match HeaderValue::from_str(out.trim()) {
        Ok(hv) => hv,
        Err(_) => one_shot.clone(),
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
}
