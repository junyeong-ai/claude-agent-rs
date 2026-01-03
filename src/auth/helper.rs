//! API Key Helper for dynamic credential generation.

use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::RwLock;

use crate::{Error, Result};

#[derive(Debug)]
pub struct ApiKeyHelper {
    command: String,
    ttl: Duration,
    cache: RwLock<Option<CachedKey>>,
}

#[derive(Debug, Clone)]
struct CachedKey {
    key: String,
    expires_at: Instant,
}

impl ApiKeyHelper {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            ttl: Duration::from_secs(3600),
            cache: RwLock::new(None),
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn with_ttl_ms(mut self, ttl_ms: u64) -> Self {
        self.ttl = Duration::from_millis(ttl_ms);
        self
    }

    pub fn from_env() -> Option<Self> {
        let command = std::env::var("ANTHROPIC_API_KEY_HELPER").ok()?;
        let ttl_ms = std::env::var("CLAUDE_CODE_API_KEY_HELPER_TTL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3_600_000);

        Some(Self::new(command).with_ttl_ms(ttl_ms))
    }

    pub async fn get_key(&self) -> Result<String> {
        {
            let cache = self.cache.read().await;
            if let Some(ref cached) = *cache
                && Instant::now() < cached.expires_at
            {
                return Ok(cached.key.clone());
            }
        }

        let key = self.execute_helper().await?;

        let cached = CachedKey {
            key: key.clone(),
            expires_at: Instant::now() + self.ttl,
        };

        *self.cache.write().await = Some(cached);

        Ok(key)
    }

    async fn execute_helper(&self) -> Result<String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::auth(format!("Failed to execute API key helper: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::auth(format!(
                "API key helper failed: {}",
                stderr.trim()
            )));
        }

        let key = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if key.is_empty() {
            return Err(Error::auth("API key helper returned empty key"));
        }

        Ok(key)
    }

    pub async fn invalidate(&self) {
        *self.cache.write().await = None;
    }
}

#[derive(Debug)]
pub struct AwsCredentialRefresh {
    auth_refresh_cmd: Option<String>,
    credential_export_cmd: Option<String>,
}

impl AwsCredentialRefresh {
    pub fn new() -> Self {
        Self {
            auth_refresh_cmd: None,
            credential_export_cmd: None,
        }
    }

    pub fn from_settings(
        auth_refresh: Option<String>,
        credential_export: Option<String>,
    ) -> Option<Self> {
        if auth_refresh.is_none() && credential_export.is_none() {
            return None;
        }

        Some(Self {
            auth_refresh_cmd: auth_refresh,
            credential_export_cmd: credential_export,
        })
    }

    pub async fn refresh(&self) -> Result<Option<AwsCredentials>> {
        if let Some(ref cmd) = self.credential_export_cmd {
            return self.export_credentials(cmd).await.map(Some);
        }

        if let Some(ref cmd) = self.auth_refresh_cmd {
            self.run_auth_refresh(cmd).await?;
        }

        Ok(None)
    }

    async fn run_auth_refresh(&self, cmd: &str) -> Result<()> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::auth(format!("AWS auth refresh failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::auth(format!(
                "AWS auth refresh failed: {}",
                stderr.trim()
            )));
        }

        Ok(())
    }

    async fn export_credentials(&self, cmd: &str) -> Result<AwsCredentials> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::auth(format!("AWS credential export failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::auth(format!(
                "AWS credential export failed: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| Error::auth(format!("Invalid credential JSON: {}", e)))?;

        let creds = json
            .get("Credentials")
            .ok_or_else(|| Error::auth("Missing Credentials in response"))?;

        Ok(AwsCredentials {
            access_key_id: creds
                .get("AccessKeyId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::auth("Missing AccessKeyId"))?
                .to_string(),
            secret_access_key: creds
                .get("SecretAccessKey")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::auth("Missing SecretAccessKey"))?
                .to_string(),
            session_token: creds
                .get("SessionToken")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
    }
}

impl Default for AwsCredentialRefresh {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

#[derive(Debug)]
pub struct CredentialManager {
    api_key_helper: Option<Arc<ApiKeyHelper>>,
    aws_refresh: Option<Arc<AwsCredentialRefresh>>,
}

impl CredentialManager {
    pub fn new() -> Self {
        Self {
            api_key_helper: None,
            aws_refresh: None,
        }
    }

    pub fn with_api_key_helper(mut self, helper: ApiKeyHelper) -> Self {
        self.api_key_helper = Some(Arc::new(helper));
        self
    }

    pub fn with_aws_refresh(mut self, refresh: AwsCredentialRefresh) -> Self {
        self.aws_refresh = Some(Arc::new(refresh));
        self
    }

    pub async fn get_api_key(&self) -> Result<Option<String>> {
        match &self.api_key_helper {
            Some(helper) => helper.get_key().await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn refresh_aws(&self) -> Result<Option<AwsCredentials>> {
        match &self.aws_refresh {
            Some(refresh) => refresh.refresh().await,
            None => Ok(None),
        }
    }
}

impl Default for CredentialManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_helper_echo() {
        let helper = ApiKeyHelper::new("echo test-key");
        let key = helper.get_key().await.unwrap();
        assert_eq!(key, "test-key");
    }

    #[tokio::test]
    async fn test_api_key_helper_caching() {
        let helper = ApiKeyHelper::new("echo test-key").with_ttl(Duration::from_secs(60));

        let key1 = helper.get_key().await.unwrap();
        let key2 = helper.get_key().await.unwrap();
        assert_eq!(key1, key2);
    }

    #[tokio::test]
    async fn test_api_key_helper_failure() {
        let helper = ApiKeyHelper::new("exit 1");
        assert!(helper.get_key().await.is_err());
    }

    #[test]
    fn test_credential_manager_default() {
        let manager = CredentialManager::default();
        assert!(manager.api_key_helper.is_none());
        assert!(manager.aws_refresh.is_none());
    }
}
