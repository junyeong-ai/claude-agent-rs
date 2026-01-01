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
}
