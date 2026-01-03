//! Session configuration.

use serde::{Deserialize, Serialize};

use super::enums::SessionMode;
use super::policy::PermissionPolicy;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionConfig {
    pub model: String,
    pub max_tokens: u32,
    #[serde(default)]
    pub permission_policy: PermissionPolicy,
    #[serde(default)]
    pub mode: SessionMode,
    pub ttl_secs: Option<u64>,
    pub system_prompt: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-5".to_string(),
            max_tokens: 16384,
            permission_policy: PermissionPolicy::default(),
            mode: SessionMode::default(),
            ttl_secs: None,
            system_prompt: None,
        }
    }
}
