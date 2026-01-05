//! Chain credential provider.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::auth::{ClaudeCliProvider, Credential, CredentialProvider, EnvironmentProvider};
use crate::{Error, Result};

pub struct ChainProvider {
    providers: Vec<Arc<dyn CredentialProvider>>,
    last_successful: RwLock<Option<Arc<dyn CredentialProvider>>>,
}

impl ChainProvider {
    pub fn new(providers: Vec<Box<dyn CredentialProvider>>) -> Self {
        Self {
            providers: providers.into_iter().map(Arc::from).collect(),
            last_successful: RwLock::new(None),
        }
    }

    pub fn with<P: CredentialProvider + 'static>(mut self, provider: P) -> Self {
        self.providers.push(Arc::new(provider));
        self
    }
}

impl Default for ChainProvider {
    fn default() -> Self {
        Self {
            providers: vec![
                Arc::new(EnvironmentProvider::new()),
                Arc::new(ClaudeCliProvider::new()),
            ],
            last_successful: RwLock::new(None),
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
                    *self.last_successful.write().await = Some(Arc::clone(provider));
                    return Ok(cred);
                }
                Err(e) => {
                    tracing::debug!("Provider {} failed: {}", provider.name(), e);
                    errors.push(format!("{}: {}", provider.name(), e));
                }
            }
        }

        Err(Error::auth(format!(
            "No credentials found. Tried: {}",
            errors.join(", ")
        )))
    }

    async fn refresh(&self) -> Result<Credential> {
        let provider = self.last_successful.read().await;
        match provider.as_ref() {
            Some(p) if p.supports_refresh() => p.refresh().await,
            Some(_) => Err(Error::auth(
                "Last successful provider does not support refresh",
            )),
            None => Err(Error::auth("No provider has successfully resolved yet")),
        }
    }

    fn supports_refresh(&self) -> bool {
        false
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

    #[tokio::test]
    async fn test_chain_tracks_last_successful() {
        let chain = ChainProvider::new(vec![])
            .with(EnvironmentProvider::with_var("NONEXISTENT_VAR"))
            .with(ExplicitProvider::api_key("fallback"));

        let _ = chain.resolve().await.unwrap();

        let last = chain.last_successful.read().await;
        assert!(last.is_some());
        assert_eq!(last.as_ref().unwrap().name(), "explicit");
    }
}
