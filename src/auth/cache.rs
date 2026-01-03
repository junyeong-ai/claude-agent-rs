//! Credential caching layer.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{Credential, CredentialProvider};
use crate::Result;

const DEFAULT_TTL: Duration = Duration::from_secs(300); // 5 minutes

struct CacheEntry {
    credential: Credential,
    fetched_at: Instant,
}

/// A caching wrapper around any CredentialProvider.
pub struct CachedProvider<P> {
    inner: P,
    cache: Arc<RwLock<Option<CacheEntry>>>,
    ttl: Duration,
}

impl<P: CredentialProvider> CachedProvider<P> {
    pub fn new(provider: P) -> Self {
        Self {
            inner: provider,
            cache: Arc::new(RwLock::new(None)),
            ttl: DEFAULT_TTL,
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub async fn invalidate(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    fn is_expired(&self, entry: &CacheEntry) -> bool {
        entry.fetched_at.elapsed() > self.ttl
    }

    fn credential_expired(&self, cred: &Credential) -> bool {
        if let Credential::OAuth(oauth) = cred {
            oauth.is_expired()
        } else {
            false
        }
    }
}

#[async_trait]
impl<P: CredentialProvider> CredentialProvider for CachedProvider<P> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    async fn resolve(&self) -> Result<Credential> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref entry) = *cache
                && !self.is_expired(entry)
                && !self.credential_expired(&entry.credential)
            {
                return Ok(entry.credential.clone());
            }
        }

        // Cache miss or expired - fetch new credential
        let credential = self.inner.resolve().await?;

        // Update cache
        let mut cache = self.cache.write().await;
        *cache = Some(CacheEntry {
            credential: credential.clone(),
            fetched_at: Instant::now(),
        });

        Ok(credential)
    }

    async fn refresh(&self) -> Result<Credential> {
        let credential = self.inner.refresh().await?;

        let mut cache = self.cache.write().await;
        *cache = Some(CacheEntry {
            credential: credential.clone(),
            fetched_at: Instant::now(),
        });

        Ok(credential)
    }

    fn supports_refresh(&self) -> bool {
        self.inner.supports_refresh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingProvider {
        calls: AtomicUsize,
    }

    impl CountingProvider {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl CredentialProvider for CountingProvider {
        fn name(&self) -> &str {
            "counting"
        }

        async fn resolve(&self) -> Result<Credential> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Credential::api_key("test-key"))
        }
    }

    #[tokio::test]
    async fn test_caching() {
        let inner = CountingProvider::new();
        let cached = CachedProvider::new(inner);

        // First call should hit the provider
        let _ = cached.resolve().await.unwrap();
        assert_eq!(1, cached.inner.call_count());

        // Second call should use cache
        let _ = cached.resolve().await.unwrap();
        assert_eq!(1, cached.inner.call_count());
    }

    #[tokio::test]
    async fn test_invalidate() {
        let inner = CountingProvider::new();
        let cached = CachedProvider::new(inner);

        let _ = cached.resolve().await.unwrap();
        assert_eq!(1, cached.inner.call_count());

        cached.invalidate().await;

        let _ = cached.resolve().await.unwrap();
        assert_eq!(2, cached.inner.call_count());
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let inner = CountingProvider::new();
        let cached = CachedProvider::new(inner).with_ttl(Duration::from_millis(10));

        let _ = cached.resolve().await.unwrap();
        assert_eq!(1, cached.inner.call_count());

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(20)).await;

        let _ = cached.resolve().await.unwrap();
        assert_eq!(2, cached.inner.call_count());
    }
}
