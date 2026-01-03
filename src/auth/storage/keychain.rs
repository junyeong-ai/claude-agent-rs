//! macOS Keychain credential storage.

use std::process::Command;

use super::CliCredentials;
use crate::Result;

const SERVICE_NAME: &str = "Claude Code-credentials";

/// macOS Keychain storage.
pub struct KeychainStorage;

impl KeychainStorage {
    /// Load credentials from Keychain using security command.
    pub async fn load() -> Result<Option<CliCredentials>> {
        let output = Command::new("security")
            .args(["find-generic-password", "-s", SERVICE_NAME, "-w"])
            .output();

        let output = match output {
            Ok(o) => o,
            Err(e) => {
                tracing::debug!("Failed to execute security command: {}", e);
                return Ok(None);
            }
        };

        if !output.status.success() {
            tracing::debug!("Keychain entry not found for service: {}", SERVICE_NAME);
            return Ok(None);
        }

        let secret = String::from_utf8_lossy(&output.stdout);
        let secret = secret.trim();

        if secret.is_empty() {
            return Ok(None);
        }

        let creds: CliCredentials = serde_json::from_str(secret).map_err(|e| {
            crate::Error::auth(format!("Failed to parse keychain credentials: {}", e))
        })?;

        Ok(Some(creds))
    }
}
