//! Claude Code settings.json provider with hierarchical loading.
//!
//! Loads settings from:
//! 1. User settings: ~/.claude/settings.json
//! 2. Project settings: .claude/settings.json
//! 3. Local settings: .claude/settings.local.json (not committed)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ConfigResult;

/// Loaded settings from all sources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Permission deny patterns.
    #[serde(default)]
    pub permissions: PermissionSettings,
    /// MCP server configurations.
    #[serde(default)]
    pub mcp_servers: HashMap<String, serde_json::Value>,
    /// Other settings.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Permission settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionSettings {
    /// Patterns to deny (e.g., "Read(./.env)").
    #[serde(default)]
    pub deny: Vec<String>,
    /// Patterns to allow.
    #[serde(default)]
    pub allow: Vec<String>,
}

/// Settings loader that merges from multiple sources.
#[derive(Debug, Default)]
pub struct SettingsLoader {
    settings: Settings,
}

impl SettingsLoader {
    /// Create a new settings loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load settings from all sources for a project.
    pub async fn load(&mut self, project_dir: &Path) -> ConfigResult<&Settings> {
        // 1. User settings (lowest priority)
        if let Some(home) = dirs::home_dir() {
            let user_settings = home.join(".claude").join("settings.json");
            if user_settings.exists() {
                self.merge_file(&user_settings).await?;
            }
        }

        // 2. Project settings
        let project_settings = project_dir.join(".claude").join("settings.json");
        if project_settings.exists() {
            self.merge_file(&project_settings).await?;
        }

        // 3. Local settings (highest priority, not committed)
        let local_settings = project_dir.join(".claude").join("settings.local.json");
        if local_settings.exists() {
            self.merge_file(&local_settings).await?;
        }

        Ok(&self.settings)
    }

    /// Merge settings from a file.
    async fn merge_file(&mut self, path: &PathBuf) -> ConfigResult<()> {
        let content = tokio::fs::read_to_string(path).await?;
        let file_settings: Settings = serde_json::from_str(&content)?;

        // Merge env variables
        self.settings.env.extend(file_settings.env);

        // Merge permissions
        self.settings
            .permissions
            .deny
            .extend(file_settings.permissions.deny);
        self.settings
            .permissions
            .allow
            .extend(file_settings.permissions.allow);

        // Merge MCP servers
        self.settings.mcp_servers.extend(file_settings.mcp_servers);

        // Merge extra settings
        self.settings.extra.extend(file_settings.extra);

        Ok(())
    }

    /// Apply environment variables from settings.
    pub fn apply_env(&self) {
        for (key, value) in &self.settings.env {
            unsafe { std::env::set_var(key, value) };
        }
    }

    /// Get the loaded settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Check if a pattern should be denied.
    pub fn is_denied(&self, pattern: &str) -> bool {
        self.settings
            .permissions
            .deny
            .iter()
            .any(|p| Self::matches_pattern(p, pattern))
    }

    /// Check if a pattern is allowed (overrides deny).
    pub fn is_allowed(&self, pattern: &str) -> bool {
        self.settings
            .permissions
            .allow
            .iter()
            .any(|p| Self::matches_pattern(p, pattern))
    }

    /// Match a permission pattern against a target.
    fn matches_pattern(pattern: &str, target: &str) -> bool {
        if let Some(start) = pattern.find('(')
            && let Some(end) = pattern.rfind(')')
        {
            let inner = &pattern[start + 1..end];
            let inner_normalized = inner.strip_prefix("./").unwrap_or(inner);
            let target_normalized = target.strip_prefix("./").unwrap_or(target);

            if inner.contains('*') {
                if inner_normalized.contains("**") {
                    let prefix = inner_normalized.split("**").next().unwrap_or("");
                    return target_normalized.starts_with(prefix);
                } else if inner_normalized.contains('*') {
                    let parts: Vec<&str> = inner_normalized.split('*').collect();
                    if parts.len() == 2 {
                        return target_normalized.starts_with(parts[0])
                            && target_normalized.ends_with(parts[1]);
                    }
                }
            }

            return inner_normalized == target_normalized
                || target_normalized.ends_with(inner_normalized);
        }

        pattern == target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching_exact() {
        assert!(SettingsLoader::matches_pattern("Read(./.env)", "./.env"));
        assert!(SettingsLoader::matches_pattern("Read(./.env)", ".env"));
    }

    #[test]
    fn test_pattern_matching_glob() {
        assert!(SettingsLoader::matches_pattern(
            "Read(./secrets/**)",
            "./secrets/api_key"
        ));
        assert!(SettingsLoader::matches_pattern(
            "Read(./secrets/**)",
            "secrets/db_password"
        ));
    }

    #[test]
    fn test_pattern_matching_wildcard() {
        assert!(SettingsLoader::matches_pattern("Read(./*.env)", "./.env"));
        assert!(SettingsLoader::matches_pattern(
            "Read(./*.env)",
            "./local.env"
        ));
    }

    #[tokio::test]
    async fn test_settings_loader() {
        let loader = SettingsLoader::new();
        assert!(loader.settings.env.is_empty());
    }
}
