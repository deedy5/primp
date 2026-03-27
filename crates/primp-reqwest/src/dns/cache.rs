use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{Addrs, Name, Resolve, Resolving};

const DNS_CACHE_TTL: Duration = Duration::from_secs(30);
const DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10);

struct CacheEntry {
    addrs: Vec<SocketAddr>,
    inserted_at: Instant,
}

pub(crate) struct DnsCacheResolver {
    inner: Arc<dyn Resolve>,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
}

impl DnsCacheResolver {
    pub(crate) fn new(inner: Arc<dyn Resolve>) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Resolve for DnsCacheResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();

        if let Ok(mut cache) = self.cache.try_lock() {
            if let Some(entry) = cache.get(&host) {
                if entry.inserted_at.elapsed() < DNS_CACHE_TTL {
                    let addrs: Addrs = Box::new(entry.addrs.clone().into_iter());
                    return Box::pin(std::future::ready(Ok(addrs)));
                }
                cache.remove(&host);
            }
        }

        let inner = Arc::clone(&self.inner);
        let cache = Arc::clone(&self.cache);

        Box::pin(async move {
            let name = Name::from_str(&host).expect("host was already a valid DNS name");
            let addrs_iter = tokio::time::timeout(
                DNS_RESOLUTION_TIMEOUT,
                inner.resolve(name),
            )
            .await
            .map_err(|_| std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("DNS resolution timed out after {DNS_RESOLUTION_TIMEOUT:?}"),
            ))??;
            let result: Vec<SocketAddr> = addrs_iter.collect();
            if let Ok(mut cache) = cache.try_lock() {
                cache.insert(
                    host,
                    CacheEntry {
                        addrs: result.clone(),
                        inserted_at: Instant::now(),
                    },
                );
            }
            Ok(Box::new(result.into_iter()) as Addrs)
        })
    }
}
