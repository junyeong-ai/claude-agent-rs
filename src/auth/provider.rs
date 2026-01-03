//! Credential provider trait.

use async_trait::async_trait;

use super::Credential;
use crate::{Error, Result};

/// Trait for resolving credentials from various sources.
#[async_trait]
pub trait CredentialProvider: Send + Sync {
    /// Provider name for debugging.
    fn name(&self) -> &str;

    /// Resolve credential from this provider.
    async fn resolve(&self) -> Result<Credential>;

    /// Refresh expired credentials.
    async fn refresh(&self) -> Result<Credential> {
        Err(Error::auth("Refresh not supported"))
    }

    /// Whether this provider supports credential refresh.
    fn supports_refresh(&self) -> bool {
        false
    }
}
