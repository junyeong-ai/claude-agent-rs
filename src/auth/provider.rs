//! Credential provider trait.

use async_trait::async_trait;

use super::Credential;
use crate::Result;

/// Trait for resolving credentials from various sources.
#[async_trait]
pub trait CredentialProvider: Send + Sync {
    /// Provider name for debugging.
    fn name(&self) -> &str;

    /// Resolve credential from this provider.
    async fn resolve(&self) -> Result<Credential>;

    /// Whether this provider supports token refresh.
    fn supports_refresh(&self) -> bool {
        false
    }

    /// Refresh an expired credential.
    async fn refresh(&self, _credential: &Credential) -> Result<Credential> {
        Err(crate::Error::Auth("Refresh not supported".into()))
    }
}
