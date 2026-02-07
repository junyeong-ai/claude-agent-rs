//! File-based Configuration Provider
//!
//! Loads configuration from JSON files (CLI compatible).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::ConfigResult;
use super::provider::ConfigProvider;

/// File-based configuration provider
pub struct FileConfigProvider {
    /// Path to the configuration file
    path: PathBuf,
    /// Cached data
    data: Arc<RwLock<Option<HashMap<String, serde_json::Value>>>>,
    /// Whether to auto-reload on get
    auto_reload: bool,
}

impl FileConfigProvider {
    /// Create a new file provider
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            data: Arc::new(RwLock::new(None)),
            auto_reload: false,
        }
    }

    /// Create a file provider with auto-reload enabled
    pub fn auto_reload(path: PathBuf) -> Self {
        Self {
            path,
            data: Arc::new(RwLock::new(None)),
            auto_reload: true,
        }
    }

    /// Load configuration from file
    async fn load(&self) -> ConfigResult<HashMap<String, serde_json::Value>> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let content = tokio::fs::read_to_string(&self.path).await?;
        let data: HashMap<String, serde_json::Value> = serde_json::from_str(&content)?;
        Ok(data)
    }

    /// Ensure data is loaded
    async fn ensure_loaded(&self) -> ConfigResult<()> {
        let mut data = self.data.write().await;
        if data.is_none() || self.auto_reload {
            *data = Some(self.load().await?);
        }
        Ok(())
    }

    /// Save configuration to file
    async fn save(&self, data: &HashMap<String, serde_json::Value>) -> ConfigResult<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(data)?;
        tokio::fs::write(&self.path, content).await?;
        Ok(())
    }

    /// Reload configuration from file
    pub async fn reload(&self) -> ConfigResult<()> {
        let mut data = self.data.write().await;
        *data = Some(self.load().await?);
        Ok(())
    }

    /// Get the file path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[async_trait::async_trait]
impl ConfigProvider for FileConfigProvider {
    fn name(&self) -> &str {
        "file"
    }

    async fn get_raw(&self, key: &str) -> ConfigResult<Option<String>> {
        self.ensure_loaded().await?;

        let data = self.data.read().await;
        if let Some(ref map) = *data {
            // Support nested keys with dot notation
            let parts: Vec<&str> = key.split('.').collect();
            let mut current: Option<&serde_json::Value> = None;

            for (i, part) in parts.iter().enumerate() {
                if i == 0 {
                    current = map.get(*part);
                } else {
                    current = current.and_then(|v| v.get(*part));
                }
            }

            match current {
                Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
                Some(v) => Ok(Some(v.to_string())),
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    async fn set_raw(&self, key: &str, value: &str) -> ConfigResult<()> {
        self.ensure_loaded().await?;

        let mut data = self.data.write().await;
        let map = data.get_or_insert_with(HashMap::new);

        // Parse value as JSON if possible, otherwise store as string
        let json_value: serde_json::Value = serde_json::from_str(value)
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));

        // Simple top-level set (doesn't support nested paths for writes)
        map.insert(key.to_string(), json_value);

        self.save(map).await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> ConfigResult<bool> {
        self.ensure_loaded().await?;

        let mut data = self.data.write().await;
        if let Some(ref mut map) = *data {
            let existed = map.remove(key).is_some();
            if existed {
                self.save(map).await?;
            }
            Ok(existed)
        } else {
            Ok(false)
        }
    }

    async fn list_keys(&self, prefix: &str) -> ConfigResult<Vec<String>> {
        self.ensure_loaded().await?;

        let data = self.data.read().await;
        if let Some(ref map) = *data {
            let keys: Vec<String> = map
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            Ok(keys)
        } else {
            Ok(Vec::new())
        }
    }
}

impl std::fmt::Debug for FileConfigProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileConfigProvider")
            .field("path", &self.path)
            .field("auto_reload", &self.auto_reload)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_provider_create_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Create config file
        let config = serde_json::json!({
            "api_key": "sk-test-123",
            "model": "claude-sonnet-4-5",
            "nested": {
                "value": "inner"
            }
        });
        tokio::fs::write(&config_path, config.to_string())
            .await
            .unwrap();

        let provider = FileConfigProvider::new(config_path);

        // Read values
        assert_eq!(
            provider.get_raw("api_key").await.unwrap(),
            Some("sk-test-123".to_string())
        );
        assert_eq!(
            provider.get_raw("model").await.unwrap(),
            Some("claude-sonnet-4-5".to_string())
        );

        // Read nested - string values from nested objects
        assert_eq!(
            provider.get_raw("nested.value").await.unwrap(),
            Some("inner".to_string())
        );
    }

    #[tokio::test]
    async fn test_file_provider_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.json");

        let provider = FileConfigProvider::new(config_path);

        // Should return None for non-existent file
        assert_eq!(provider.get_raw("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_file_provider_write() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("new_config.json");

        let provider = FileConfigProvider::new(config_path.clone());

        // Write
        provider.set_raw("key1", "value1").await.unwrap();
        provider.set_raw("key2", "42").await.unwrap();

        // Verify file was created
        assert!(config_path.exists());

        // Read back
        assert_eq!(
            provider.get_raw("key1").await.unwrap(),
            Some("value1".to_string())
        );
    }

    #[tokio::test]
    async fn test_file_provider_delete() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("delete_config.json");

        let provider = FileConfigProvider::new(config_path);

        // Set then delete
        provider.set_raw("temp", "value").await.unwrap();
        assert!(provider.delete("temp").await.unwrap());
        assert_eq!(provider.get_raw("temp").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_file_provider_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("list_config.json");

        let provider = FileConfigProvider::new(config_path);

        provider.set_raw("app.name", "\"test\"").await.unwrap();
        provider.set_raw("app.version", "\"1.0\"").await.unwrap();
        provider.set_raw("other", "\"value\"").await.unwrap();

        let keys = provider.list_keys("app.").await.unwrap();
        assert_eq!(keys.len(), 2);
    }
}
