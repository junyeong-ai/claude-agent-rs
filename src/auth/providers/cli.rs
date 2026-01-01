//! Claude Code CLI credential provider.

use async_trait::async_trait;

use crate::auth::storage::load_cli_credentials;
use crate::auth::{Credential, CredentialProvider};
use crate::{Error, Result};

/// Provider that reads credentials from Claude Code CLI.
pub struct ClaudeCliProvider;

impl ClaudeCliProvider {
    /// Create a new CLI provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeCliProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialProvider for ClaudeCliProvider {
    fn name(&self) -> &str {
        "claude_cli"
    }

    async fn resolve(&self) -> Result<Credential> {
        let creds = load_cli_credentials().await?.ok_or_else(|| {
            Error::Auth("Claude Code CLI credentials not found. Run 'claude login' first.".into())
        })?;

        let oauth = creds
            .oauth()
            .ok_or_else(|| Error::Auth("No OAuth credentials in Claude Code CLI config".into()))?;

        if oauth.is_expired() {
            return Err(Error::Auth(
                "Claude Code CLI token expired. Run 'claude login' to refresh.".into(),
            ));
        }

        Ok(Credential::OAuth(oauth.clone()))
    }
}
