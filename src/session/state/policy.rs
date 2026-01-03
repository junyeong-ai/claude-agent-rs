//! Permission policies for sessions.

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PermissionPolicy {
    pub mode: PermissionMode,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub tool_limits: HashMap<String, ToolLimits>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolLimits {
    pub timeout_ms: Option<u64>,
    pub max_output_size: Option<usize>,
}
