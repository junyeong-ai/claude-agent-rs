//! Claude Code CLI credential provider.

use async_trait::async_trait;
use tokio::process::Command;

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

    async fn refresh_via_cli() -> Result<Credential> {
        let output = Command::new("claude")
            .args(["auth", "refresh"])
            .output()
            .await
            .map_err(|e| Error::auth(format!("Failed to run claude auth refresh: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::auth(format!("Token refresh failed: {}", stderr)));
        }

        let creds = load_cli_credentials()
            .await?
            .ok_or_else(|| Error::auth("Credentials not found after refresh"))?;

        let oauth = creds
            .oauth()
            .ok_or_else(|| Error::auth("No OAuth credentials after refresh"))?;

        if oauth.is_expired() {
            return Err(Error::auth("Token still expired after refresh"));
        }

        Ok(Credential::OAuth(oauth.clone()))
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
            Error::auth("Claude Code CLI credentials not found. Run 'claude login' first.")
        })?;

        let oauth = creds
            .oauth()
            .ok_or_else(|| Error::auth("No OAuth credentials in Claude Code CLI config"))?;

        if oauth.is_expired() {
            return self.refresh().await;
        }

        Ok(Credential::OAuth(oauth.clone()))
    }

    async fn refresh(&self) -> Result<Credential> {
        Self::refresh_via_cli().await
    }

    fn supports_refresh(&self) -> bool {
        true
    }
}
