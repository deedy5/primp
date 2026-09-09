use url::Url;

/// Convert a value into a `Url`. Sealed so only primp types implement it.
pub trait IntoUrl: IntoUrlSealed {}

impl IntoUrl for Url {}
impl IntoUrl for &Url {}
impl IntoUrl for String {}
impl IntoUrl for &str {}
impl IntoUrl for &String {}

pub trait IntoUrlSealed {
    // Besides parsing as a valid `Url`, the `Url` must be a valid
    // `http::Uri`, in that it makes sense to use in a network request.
    fn into_url(self) -> crate::Result<Url>;

    fn as_str(&self) -> &str;
}

impl IntoUrlSealed for Url {
    fn into_url(self) -> crate::Result<Url> {
        if matches!(self.scheme(), "http" | "https") && self.has_host() {
            Ok(self)
        } else {
            Err(crate::error::url_bad_scheme(self))
        }
    }

    fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl IntoUrlSealed for &Url {
    fn into_url(self) -> crate::Result<Url> {
        if matches!(self.scheme(), "http" | "https") && self.has_host() {
            Ok(self.clone())
        } else {
            Err(crate::error::url_bad_scheme(self.clone()))
        }
    }

    fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl IntoUrlSealed for &str {
    fn into_url(self) -> crate::Result<Url> {
        Url::parse(self).map_err(crate::error::builder)?.into_url()
    }

    fn as_str(&self) -> &str {
        self
    }
}

impl IntoUrlSealed for &String {
    fn into_url(self) -> crate::Result<Url> {
        (&**self).into_url()
    }

    fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl IntoUrlSealed for String {
    fn into_url(self) -> crate::Result<Url> {
        (&*self).into_url()
    }

    fn as_str(&self) -> &str {
        self.as_ref()
    }
}

pub(crate) fn try_uri(url: &Url) -> crate::Result<http::Uri> {
    url.as_str()
        .parse()
        .map_err(|_| crate::error::url_invalid_uri(url.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn into_url_file_scheme() {
        let err = "file:///etc/hosts".into_url().unwrap_err();
        assert_eq!(
            err.source().unwrap().to_string(),
            "URL scheme is not allowed"
        );
    }

    #[test]
    fn into_url_blob_scheme() {
        let err = "blob:https://example.com".into_url().unwrap_err();
        assert_eq!(
            err.source().unwrap().to_string(),
            "URL scheme is not allowed"
        );
    }

    #[test]
    fn into_url_requires_host() {
        // Defense-in-depth: http/https require a host. url crate already
        // rejects empty hosts at parse/set_host time, so into_url must at
        // least accept valid hosts and reject non-http schemes.
        let ok = Url::parse("http://example.com/").unwrap();
        assert!(ok.clone().into_url().is_ok());
        assert!((&ok).into_url().is_ok());
        // set_host(None) is rejected by url crate for special schemes.
        let mut url = Url::parse("http://example.com/").unwrap();
        assert!(url.set_host(None).is_err());
        // Non-http(s) schemes with hosts must be rejected via every impl.
        for bad in [
            "ftp://example.com/file",
            "ws://example.com/socket",
            "wss://example.com/socket",
            "file:///etc/hosts",
            "data:text/plain,hello",
        ] {
            let url = Url::parse(bad).unwrap();
            assert!(url.clone().into_url().is_err(), "{bad} via Url");
            assert!((&url).into_url().is_err(), "{bad} via &Url");
            assert!(bad.into_url().is_err(), "{bad} via &str");
            assert!(bad.to_string().into_url().is_err(), "{bad} via String");
        }
        // https with host accepted via every impl.
        let https = "https://example.com/x";
        assert!(https.into_url().is_ok());
        assert!(https.to_string().into_url().is_ok());
    }
}
