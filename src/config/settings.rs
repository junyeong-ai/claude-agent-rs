//! Claude Code settings.json provider with hierarchical loading.
//!
//! Loads settings from (lowest to highest priority):
//! 1. User settings: ~/.claude/settings.json
//! 2. Project settings: .claude/settings.json
//! 3. Local settings: .claude/settings.local.json (not committed)
//! 4. Managed settings: organization policy (locked, cannot be overridden)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ConfigResult;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsSource {
    Builtin,
    #[default]
    User,
    Project,
    Local,
    Managed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(skip)]
    pub source: SettingsSource,

    #[serde(default)]
    pub env: HashMap<String, String>,

    #[serde(default)]
    pub permissions: PermissionSettings,

    #[serde(default)]
    pub sandbox: SandboxSettings,

    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, serde_json::Value>,

    #[serde(default)]
    pub model: Option<String>,

    #[serde(default, rename = "smallModel")]
    pub small_model: Option<String>,

    #[serde(default, rename = "maxTokens")]
    pub max_tokens: Option<u32>,

    #[serde(default)]
    pub hooks: Option<HooksSettings>,

    #[serde(default, rename = "outputStyle")]
    pub output_style: Option<String>,

    #[serde(default, rename = "awsAuthRefresh")]
    pub aws_auth_refresh: Option<String>,

    #[serde(default, rename = "awsCredentialExport")]
    pub aws_credential_export: Option<String>,

    #[serde(default, rename = "apiKeyHelper")]
    pub api_key_helper: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Settings {
    pub fn with_source(mut self, source: SettingsSource) -> Self {
        self.source = source;
        self
    }

    pub fn is_managed(&self) -> bool {
        self.source == SettingsSource::Managed
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksSettings {
    #[serde(default, rename = "PreToolUse")]
    pub pre_tool_use: HashMap<String, HookConfig>,

    #[serde(default, rename = "PostToolUse")]
    pub post_tool_use: HashMap<String, HookConfig>,

    #[serde(default, rename = "SessionStart")]
    pub session_start: Vec<HookConfig>,

    #[serde(default, rename = "SessionEnd")]
    pub session_end: Vec<HookConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookConfig {
    Command(String),
    Full {
        command: String,
        #[serde(default)]
        timeout_secs: Option<u64>,
        #[serde(default)]
        matcher: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionSettings {
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default, rename = "defaultMode")]
    pub default_mode: Option<String>,
}

impl PermissionSettings {
    pub fn to_policy(&self) -> crate::permissions::PermissionPolicy {
        use crate::permissions::{PermissionMode, PermissionPolicy};

        let mut builder = PermissionPolicy::builder();

        if let Some(mode_str) = &self.default_mode
            && let Ok(mode) = mode_str.parse::<PermissionMode>()
        {
            builder = builder.mode(mode);
        }

        for pattern in &self.deny {
            builder = builder.deny(pattern);
        }

        for pattern in &self.allow {
            builder = builder.allow(pattern);
        }

        builder.build()
    }

    pub fn is_empty(&self) -> bool {
        self.deny.is_empty() && self.allow.is_empty() && self.default_mode.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxSettings {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub network: NetworkSandboxSettings,

    #[serde(default, rename = "excludedCommands")]
    pub excluded_commands: Vec<String>,

    #[serde(default, rename = "allowUnsandboxedCommands")]
    pub allow_unsandboxed_commands: bool,

    #[serde(default, rename = "autoAllowBashIfSandboxed")]
    pub auto_allow_bash_if_sandboxed: Option<bool>,
}

impl SandboxSettings {
    /// Convert settings to SandboxConfig for use with SecurityContext.
    ///
    /// # Default Behaviors
    /// - `auto_allow_bash_if_sandboxed`: defaults to `true` for backward compatibility
    /// - `enable_weaker_nested_sandbox`: defaults to `false` (strict mode)
    /// - `allowed_paths` and `denied_paths`: empty by default (use working_dir as root)
    pub fn to_sandbox_config(
        &self,
        working_dir: std::path::PathBuf,
    ) -> crate::security::sandbox::SandboxConfig {
        use crate::security::sandbox::{NetworkConfig, SandboxConfig};

        SandboxConfig {
            enabled: self.enabled,
            auto_allow_bash_if_sandboxed: self.auto_allow_bash_if_sandboxed.unwrap_or(true),
            excluded_commands: self.excluded_commands.iter().cloned().collect(),
            allow_unsandboxed_commands: self.allow_unsandboxed_commands,
            network: NetworkConfig {
                http_proxy_port: self.network.http_proxy_port,
                socks_proxy_port: self.network.socks_proxy_port,
                allow_unix_sockets: Vec::new(),
                allow_local_binding: false,
            },
            working_dir,
            allowed_domains: self.network.allowed_domains.clone(),
            blocked_domains: self.network.blocked_domains.clone(),
            // Explicit defaults for clarity
            enable_weaker_nested_sandbox: false,
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn has_network_settings(&self) -> bool {
        !self.network.allowed_domains.is_empty() || !self.network.blocked_domains.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkSandboxSettings {
    #[serde(default, rename = "allowedDomains")]
    pub allowed_domains: HashSet<String>,

    #[serde(default, rename = "blockedDomains")]
    pub blocked_domains: HashSet<String>,

    #[serde(default, rename = "httpProxyPort")]
    pub http_proxy_port: Option<u16>,

    #[serde(default, rename = "socksProxyPort")]
    pub socks_proxy_port: Option<u16>,
}

/// Settings loader that merges from multiple sources.
#[derive(Debug, Default)]
pub struct SettingsLoader {
    settings: Settings,
    locked_keys: HashSet<String>,
}

impl SettingsLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load settings from all sources for a project.
    /// Priority (lowest to highest): User → Project → Local
    /// Managed settings are loaded separately and lock keys.
    pub async fn load(&mut self, project_dir: &Path) -> ConfigResult<&Settings> {
        // 1. User settings (lowest priority)
        if let Some(home) = crate::common::home_dir() {
            let user_settings = home.join(".claude").join("settings.json");
            if user_settings.exists() {
                self.merge_file(&user_settings, SettingsSource::User)
                    .await?;
            }
        }

        // 2. Project settings
        let project_settings = project_dir.join(".claude").join("settings.json");
        if project_settings.exists() {
            self.merge_file(&project_settings, SettingsSource::Project)
                .await?;
        }

        // 3. Local settings (highest non-managed priority)
        let local_settings = project_dir.join(".claude").join("settings.local.json");
        if local_settings.exists() {
            self.merge_file(&local_settings, SettingsSource::Local)
                .await?;
        }

        Ok(&self.settings)
    }

    /// Load managed settings (organization policy) - these lock keys and cannot be overridden.
    pub async fn load_managed(&mut self, path: &Path) -> ConfigResult<()> {
        if path.exists() {
            let content = tokio::fs::read_to_string(path).await?;
            let managed: Settings = serde_json::from_str(&content)?;

            // Lock all non-empty fields from managed settings
            if !managed.permissions.deny.is_empty() {
                self.locked_keys.insert("permissions.deny".to_string());
            }
            if !managed.permissions.allow.is_empty() {
                self.locked_keys.insert("permissions.allow".to_string());
            }
            if managed.model.is_some() {
                self.locked_keys.insert("model".to_string());
            }

            self.merge_settings(managed, true);
        }
        Ok(())
    }

    async fn merge_file(&mut self, path: &PathBuf, source: SettingsSource) -> ConfigResult<()> {
        let content = tokio::fs::read_to_string(path).await?;
        let mut file_settings: Settings = serde_json::from_str(&content)?;
        file_settings.source = source;
        self.merge_settings(file_settings, false);
        Ok(())
    }

    fn merge_settings(&mut self, other: Settings, is_managed: bool) {
        self.settings.env.extend(other.env);

        if !self.locked_keys.contains("permissions.deny") || is_managed {
            self.settings
                .permissions
                .deny
                .extend(other.permissions.deny);
        }
        if !self.locked_keys.contains("permissions.allow") || is_managed {
            self.settings
                .permissions
                .allow
                .extend(other.permissions.allow);
        }
        if other.permissions.default_mode.is_some() {
            self.settings.permissions.default_mode = other.permissions.default_mode;
        }

        self.settings
            .sandbox
            .network
            .allowed_domains
            .extend(other.sandbox.network.allowed_domains);
        self.settings
            .sandbox
            .network
            .blocked_domains
            .extend(other.sandbox.network.blocked_domains);
        self.settings
            .sandbox
            .excluded_commands
            .extend(other.sandbox.excluded_commands);

        if other.sandbox.enabled {
            self.settings.sandbox.enabled = true;
        }
        if other.sandbox.allow_unsandboxed_commands {
            self.settings.sandbox.allow_unsandboxed_commands = true;
        }
        if other.sandbox.auto_allow_bash_if_sandboxed.is_some() {
            self.settings.sandbox.auto_allow_bash_if_sandboxed =
                other.sandbox.auto_allow_bash_if_sandboxed;
        }
        if let Some(port) = other.sandbox.network.http_proxy_port {
            self.settings.sandbox.network.http_proxy_port = Some(port);
        }
        if let Some(port) = other.sandbox.network.socks_proxy_port {
            self.settings.sandbox.network.socks_proxy_port = Some(port);
        }

        self.settings.mcp_servers.extend(other.mcp_servers);

        if other.aws_auth_refresh.is_some() {
            self.settings.aws_auth_refresh = other.aws_auth_refresh;
        }
        if other.aws_credential_export.is_some() {
            self.settings.aws_credential_export = other.aws_credential_export;
        }
        if other.api_key_helper.is_some() {
            self.settings.api_key_helper = other.api_key_helper;
        }

        self.settings.extra.extend(other.extra);

        if (!self.locked_keys.contains("model") || is_managed) && other.model.is_some() {
            self.settings.model = other.model;
        }
        if other.small_model.is_some() {
            self.settings.small_model = other.small_model;
        }
        if other.max_tokens.is_some() {
            self.settings.max_tokens = other.max_tokens;
        }
        if let Some(other_hooks) = other.hooks {
            match &mut self.settings.hooks {
                Some(existing) => {
                    existing.pre_tool_use.extend(other_hooks.pre_tool_use);
                    existing.post_tool_use.extend(other_hooks.post_tool_use);
                    existing.session_start.extend(other_hooks.session_start);
                    existing.session_end.extend(other_hooks.session_end);
                }
                None => self.settings.hooks = Some(other_hooks),
            }
        }
        if other.output_style.is_some() {
            self.settings.output_style = other.output_style;
        }
    }

    pub async fn load_from_directory(dir: &Path) -> ConfigResult<Settings> {
        let mut loader = Self::new();
        let settings_path = dir.join(".claude").join("settings.json");
        if settings_path.exists() {
            loader
                .merge_file(&settings_path, SettingsSource::Project)
                .await?;
        }
        let local_path = dir.join(".claude").join("settings.local.json");
        if local_path.exists() {
            loader
                .merge_file(&local_path, SettingsSource::Local)
                .await?;
        }
        Ok(loader.settings)
    }

    pub async fn load_merged(project_dir: &Path) -> ConfigResult<Settings> {
        let mut loader = Self::new();
        loader.load(project_dir).await?;
        Ok(loader.settings)
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_settings_loader() {
        let loader = SettingsLoader::new();
        assert!(loader.settings.env.is_empty());
    }

    #[test]
    fn test_permission_settings_to_policy() {
        use crate::permissions::PermissionMode;

        let settings = PermissionSettings {
            deny: vec!["Bash(rm:*)".to_string()],
            allow: vec!["Bash(git:*)".to_string()],
            default_mode: Some("acceptEdits".to_string()),
        };

        let policy = settings.to_policy();
        assert_eq!(policy.mode, PermissionMode::AcceptEdits);
        assert_eq!(policy.rules.len(), 2);
    }

    #[test]
    fn test_permission_settings_is_empty() {
        let empty = PermissionSettings::default();
        assert!(empty.is_empty());

        let with_deny = PermissionSettings {
            deny: vec!["Bash".to_string()],
            ..Default::default()
        };
        assert!(!with_deny.is_empty());
    }

    #[test]
    fn test_sandbox_settings_enabled() {
        let settings = SandboxSettings {
            enabled: true,
            ..Default::default()
        };
        assert!(settings.is_enabled());

        let disabled = SandboxSettings::default();
        assert!(!disabled.is_enabled());
    }

    #[test]
    fn test_sandbox_settings_to_sandbox_config() {
        use std::path::PathBuf;

        let settings = SandboxSettings {
            enabled: true,
            network: NetworkSandboxSettings {
                allowed_domains: ["example.com".to_string()].into_iter().collect(),
                blocked_domains: ["malware.com".to_string()].into_iter().collect(),
                ..Default::default()
            },
            ..Default::default()
        };

        let config = settings.to_sandbox_config(PathBuf::from("/tmp"));
        assert!(config.enabled);
        assert!(config.allowed_domains.contains("example.com"));
        assert!(config.blocked_domains.contains("malware.com"));

        let network_sandbox = config.to_network_sandbox();
        assert!(network_sandbox.allowed_domains().contains("example.com"));
        assert!(network_sandbox.blocked_domains().contains("malware.com"));
    }
}
