//! Explicit credential provider.

use async_trait::async_trait;

use crate::auth::{Credential, CredentialProvider};
use crate::Result;

/// Provider with explicitly set credentials.
pub struct ExplicitProvider {
    credential: Credential,
}

impl ExplicitProvider {
    /// Create with credential.
    pub fn new(credential: Credential) -> Self {
        Self { credential }
    }

    /// Create with API key.
    pub fn api_key(key: impl Into<String>) -> Self {
        Self::new(Credential::api_key(key))
    }

    /// Create with OAuth token.
    pub fn oauth(token: impl Into<String>) -> Self {
        Self::new(Credential::oauth(token))
    }
}

#[async_trait]
impl CredentialProvider for ExplicitProvider {
    fn name(&self) -> &str {
        "explicit"
    }

    async fn resolve(&self) -> Result<Credential> {
        Ok(self.credential.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_explicit_api_key() {
        let provider = ExplicitProvider::api_key("test-key");
        let cred = provider.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "test-key"));
    }

    #[tokio::test]
    async fn test_explicit_oauth() {
        let provider = ExplicitProvider::oauth("test-token");
        let cred = provider.resolve().await.unwrap();
        match cred {
            Credential::OAuth(oauth) => {
                assert_eq!(oauth.access_token, "test-token");
            }
            _ => panic!("Expected OAuth credential"),
        }
    }
}
