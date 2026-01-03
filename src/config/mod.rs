//! Pluggable configuration provider system.
//!
//! ```rust,no_run
//! use claude_agent::config::ConfigBuilder;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ConfigBuilder::new()
//!     .env()
//!     .file(".claude/settings.json")
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod cloud;
pub mod composite;
pub mod env;
pub mod file;
pub mod memory;
pub mod provider;
pub mod settings;
pub mod validator;

pub use cloud::{BedrockConfig, CloudConfig, FoundryConfig, TokenLimits, VertexConfig};
pub use composite::CompositeConfigProvider;
pub use env::EnvConfigProvider;
pub use file::FileConfigProvider;
pub use memory::MemoryConfigProvider;
pub use provider::{ConfigProvider, ConfigProviderExt};
pub use settings::{
    HookConfig, HooksSettings, NetworkSandboxSettings, PermissionSettings, SandboxSettings,
    Settings, SettingsLoader, SettingsSource,
};
pub use validator::{ConfigValidator, ValueType};

use thiserror::Error;

/// Errors that can occur in configuration operations
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Key not found
    #[error("Key not found: {key}")]
    NotFound {
        /// The key that was not found
        key: String,
    },

    /// Invalid configuration value
    #[error("Invalid value for {key}: {message}")]
    InvalidValue {
        /// The key with invalid value
        key: String,
        /// Error message
        message: String,
    },

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error (file operations)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Environment variable error
    #[error("Environment error: {0}")]
    Env(#[from] std::env::VarError),

    /// Provider error
    #[error("Provider error: {message}")]
    Provider {
        /// Error message
        message: String,
    },

    /// Multiple validation errors
    #[error("{0}")]
    ValidationErrors(ValidationErrors),
}

#[derive(Debug)]
pub struct ValidationErrors(pub Vec<ConfigError>);

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation failed: ")?;
        let msgs: Vec<String> = self.0.iter().map(|e| e.to_string()).collect();
        write!(f, "{}", msgs.join("; "))
    }
}

/// Result type for configuration operations
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

/// Configuration builder for fluent API
pub struct ConfigBuilder {
    providers: Vec<Box<dyn ConfigProvider>>,
}

impl ConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add environment variable provider
    pub fn env(mut self) -> Self {
        self.providers.push(Box::new(EnvConfigProvider::new()));
        self
    }

    /// Add environment variable provider with prefix
    pub fn env_with_prefix(mut self, prefix: &str) -> Self {
        self.providers
            .push(Box::new(EnvConfigProvider::with_prefix(prefix)));
        self
    }

    /// Add file provider
    pub fn file(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.providers.push(Box::new(FileConfigProvider::new(
            path.as_ref().to_path_buf(),
        )));
        self
    }

    /// Add memory provider
    pub fn memory(mut self, provider: MemoryConfigProvider) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    /// Add a custom provider
    pub fn provider(mut self, provider: Box<dyn ConfigProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Build the composite configuration
    pub async fn build(self) -> ConfigResult<CompositeConfigProvider> {
        let mut composite = CompositeConfigProvider::new();
        for provider in self.providers {
            composite.add_provider(provider);
        }
        Ok(composite)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::NotFound {
            key: "api_key".to_string(),
        };
        assert!(err.to_string().contains("api_key"));
    }

    #[test]
    fn test_config_builder() {
        let builder = ConfigBuilder::new().env().env_with_prefix("CLAUDE_");
        assert!(!builder.providers.is_empty());
    }
}
