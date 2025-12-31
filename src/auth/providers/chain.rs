//! Chain credential provider.

use async_trait::async_trait;

use crate::auth::{ClaudeCliProvider, Credential, CredentialProvider, EnvironmentProvider};
use crate::{Error, Result};

/// Chain provider that tries multiple providers in order.
pub struct ChainProvider {
    providers: Vec<Box<dyn CredentialProvider>>,
}

impl ChainProvider {
    /// Create with specified providers.
    pub fn new(providers: Vec<Box<dyn CredentialProvider>>) -> Self {
        Self { providers }
    }

    /// Add a provider to the chain.
    pub fn with<P: CredentialProvider + 'static>(mut self, provider: P) -> Self {
        self.providers.push(Box::new(provider));
        self
    }
}

impl Default for ChainProvider {
    fn default() -> Self {
        Self {
            providers: vec![
                Box::new(EnvironmentProvider::new()),
                Box::new(ClaudeCliProvider::new()),
            ],
        }
    }
}

#[async_trait]
impl CredentialProvider for ChainProvider {
    fn name(&self) -> &str {
        "chain"
    }

    async fn resolve(&self) -> Result<Credential> {
        let mut errors = Vec::new();

        for provider in &self.providers {
            match provider.resolve().await {
                Ok(cred) => {
                    tracing::debug!("Credential resolved from: {}", provider.name());
                    return Ok(cred);
                }
                Err(e) => {
                    tracing::debug!("Provider {} failed: {}", provider.name(), e);
                    errors.push(format!("{}: {}", provider.name(), e));
                }
            }
        }

        Err(Error::Auth(format!(
            "No credentials found. Tried: {}",
            errors.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ExplicitProvider;

    #[tokio::test]
    async fn test_chain_first_success() {
        let chain = ChainProvider::new(vec![])
            .with(ExplicitProvider::api_key("first"))
            .with(ExplicitProvider::api_key("second"));

        let cred = chain.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "first"));
    }

    #[tokio::test]
    async fn test_chain_fallback() {
        let chain = ChainProvider::new(vec![])
            .with(EnvironmentProvider::with_var("NONEXISTENT_VAR"))
            .with(ExplicitProvider::api_key("fallback"));

        let cred = chain.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "fallback"));
    }

    #[tokio::test]
    async fn test_chain_all_fail() {
        let chain = ChainProvider::new(vec![])
            .with(EnvironmentProvider::with_var("NONEXISTENT_VAR_1"))
            .with(EnvironmentProvider::with_var("NONEXISTENT_VAR_2"));

        assert!(chain.resolve().await.is_err());
    }
}
