//! Environment variable credential provider.

use async_trait::async_trait;

use crate::auth::{Credential, CredentialProvider};
use crate::{Error, Result};

const DEFAULT_ENV_VAR: &str = "ANTHROPIC_API_KEY";

/// Provider that reads API key from environment variable.
pub struct EnvironmentProvider {
    env_var: String,
}

impl EnvironmentProvider {
    /// Create provider using default ANTHROPIC_API_KEY.
    pub fn new() -> Self {
        Self {
            env_var: DEFAULT_ENV_VAR.to_string(),
        }
    }

    /// Create provider with custom environment variable.
    pub fn from_var(env_var: impl Into<String>) -> Self {
        Self {
            env_var: env_var.into(),
        }
    }
}

impl Default for EnvironmentProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialProvider for EnvironmentProvider {
    fn name(&self) -> &str {
        "environment"
    }

    async fn resolve(&self) -> Result<Credential> {
        std::env::var(&self.env_var)
            .map(Credential::api_key)
            .map_err(|_| Error::auth(format!("{} not set", self.env_var)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[tokio::test]
    async fn test_environment_provider_missing() {
        // SAFETY: Test-only environment setup, single-threaded test context
        unsafe { std::env::remove_var("TEST_API_KEY_NOT_SET") };
        let provider = EnvironmentProvider::from_var("TEST_API_KEY_NOT_SET");
        assert!(provider.resolve().await.is_err());
    }

    #[tokio::test]
    async fn test_environment_provider_set() {
        // SAFETY: Test-only environment setup, single-threaded test context
        unsafe { std::env::set_var("TEST_API_KEY_SET", "test-key") };
        let provider = EnvironmentProvider::from_var("TEST_API_KEY_SET");
        let cred = provider.resolve().await.unwrap();
        assert!(matches!(&cred, Credential::ApiKey(k) if k.expose_secret() == "test-key"));
        unsafe { std::env::remove_var("TEST_API_KEY_SET") };
    }
}
