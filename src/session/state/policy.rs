//! Session-level permission configuration for storage.
//!
//! These types are simplified serializable versions for session persistence.
//! For runtime permission checking with rules, see `crate::permissions::PermissionPolicy`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    Bypass,
    Plan,
}

/// Session-level permission configuration.
///
/// This is a simplified, serializable version for session storage.
/// For runtime permission checking with rule patterns, use `crate::permissions::PermissionPolicy`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionPermissions {
    pub mode: PermissionMode,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub tool_limits: HashMap<String, SessionToolLimits>,
}

/// Session-level tool limits for storage.
///
/// For detailed runtime limits with path-based rules, see `crate::permissions::ToolLimits`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionToolLimits {
    pub timeout_ms: Option<u64>,
    pub max_output_size: Option<usize>,
}
