//! File-based credential storage.

use std::path::PathBuf;

use directories::BaseDirs;

use super::CliCredentials;
use crate::Result;

const CLAUDE_DIR: &str = ".claude";
const CREDENTIALS_FILE: &str = ".credentials.json";

/// File system credential storage.
pub struct FileStorage;

impl FileStorage {
    fn credentials_path() -> Option<PathBuf> {
        BaseDirs::new().map(|dirs| dirs.home_dir().join(CLAUDE_DIR).join(CREDENTIALS_FILE))
    }

    /// Load credentials from file.
    pub async fn load() -> Result<Option<CliCredentials>> {
        let Some(path) = Self::credentials_path() else {
            return Ok(None);
        };

        if !path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| crate::Error::Auth(format!("Failed to read credentials file: {}", e)))?;

        let creds: CliCredentials = serde_json::from_str(&content)
            .map_err(|e| crate::Error::Auth(format!("Failed to parse credentials: {}", e)))?;

        Ok(Some(creds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_path() {
        let path = FileStorage::credentials_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains(".claude"));
    }
}
