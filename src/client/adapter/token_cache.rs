//! Shared authentication caching for cloud provider adapters.

use std::time::{Duration, Instant, SystemTime};

use tokio::sync::RwLock;

const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(300);

pub struct CachedToken {
    token: String,
    expires_at: Instant,
}

impl std::fmt::Debug for CachedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedToken")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl CachedToken {
    pub fn new(token: String, ttl: Duration) -> Self {
        Self {
            token,
            expires_at: Instant::now() + ttl - TOKEN_REFRESH_MARGIN,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

#[derive(Clone)]
pub struct CachedAwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    expiry: Option<SystemTime>,
}

impl std::fmt::Debug for CachedAwsCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedAwsCredentials")
            .field("expiry", &self.expiry)
            .finish()
    }
}

impl CachedAwsCredentials {
    pub fn new(
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
        expiry: Option<SystemTime>,
    ) -> Self {
        Self {
            access_key_id,
            secret_access_key,
            session_token,
            expiry,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expiry
            .map(|exp| SystemTime::now() > exp)
            .unwrap_or(false)
    }

    pub fn expiry(&self) -> Option<SystemTime> {
        self.expiry
    }
}

pub type TokenCache = RwLock<Option<CachedToken>>;
pub type AwsCredentialsCache = RwLock<Option<CachedAwsCredentials>>;

pub fn new_token_cache() -> TokenCache {
    RwLock::new(None)
}

pub fn new_aws_credentials_cache() -> AwsCredentialsCache {
    RwLock::new(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_token_not_expired() {
        let token = CachedToken::new("test".into(), Duration::from_secs(3600));
        assert!(!token.is_expired());
        assert_eq!(token.token(), "test");
    }

    #[test]
    fn test_cached_token_expired() {
        let token = CachedToken::new("test".into(), Duration::from_secs(0));
        assert!(token.is_expired());
    }
}
