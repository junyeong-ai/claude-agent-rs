//! Permission modes for controlling tool execution behavior.

use serde::{Deserialize, Serialize};

/// Permission mode that determines the default behavior for tool execution.
///
/// This is particularly important for Chat API environments where interactive
/// user approval is not possible.
///
/// # Modes
///
/// - **Default**: Standard permission flow - tools must be explicitly allowed
///   or will be denied. Use allow/deny rules to control access.
///
/// - **AcceptEdits**: Auto-approve file operations (Read, Write, Edit, Glob, Grep).
///   Useful for development scenarios where file access is expected.
///
/// - **BypassPermissions**: Allow all tool executions without permission checks.
///   ⚠️ Use with extreme caution - only in trusted, sandboxed environments.
///
/// - **Plan**: Read-only mode. Only allows read operations like Read, Glob, Grep,
///   WebSearch, and WebFetch. Blocks all write/execute operations.
///
/// # Example
///
/// ```rust
/// use claude_agent::permissions::PermissionMode;
///
/// let mode = PermissionMode::AcceptEdits;
/// assert!(!mode.allows_all());
/// assert!(!mode.is_read_only());
///
/// let mode = PermissionMode::Plan;
/// assert!(mode.is_read_only());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Standard permission flow - use allow/deny rules
    ///
    /// In this mode, tools are evaluated against allow/deny rules.
    /// If no rule matches, the tool is denied by default.
    #[default]
    Default,

    /// Auto-approve file operations
    ///
    /// This mode automatically approves file-related tools:
    /// - Read, Write, Edit, Glob, Grep
    ///
    /// Other tools still require explicit allow rules.
    AcceptEdits,

    /// Allow all tool executions without permission checks
    ///
    /// ⚠️ **Warning**: This mode bypasses all permission checks.
    /// Only use in fully trusted, sandboxed environments.
    BypassPermissions,

    /// Read-only mode
    ///
    /// Only allows read-only tools:
    /// - Read, Glob, Grep, WebSearch, WebFetch
    ///
    /// All write and execute operations are blocked.
    Plan,
}

impl PermissionMode {
    pub fn allows_all(&self) -> bool {
        matches!(self, PermissionMode::BypassPermissions)
    }

    pub fn is_read_only(&self) -> bool {
        matches!(self, PermissionMode::Plan)
    }

    pub fn auto_approves_files(&self) -> bool {
        matches!(self, PermissionMode::AcceptEdits)
    }

    pub fn is_default(&self) -> bool {
        matches!(self, PermissionMode::Default)
    }

    pub fn description(&self) -> &'static str {
        match self {
            PermissionMode::Default => "Standard permission flow with allow/deny rules",
            PermissionMode::AcceptEdits => "Auto-approve file operations",
            PermissionMode::BypassPermissions => "Allow all operations (dangerous)",
            PermissionMode::Plan => "Read-only mode",
        }
    }
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionMode::Default => write!(f, "default"),
            PermissionMode::AcceptEdits => write!(f, "acceptEdits"),
            PermissionMode::BypassPermissions => write!(f, "bypassPermissions"),
            PermissionMode::Plan => write!(f, "plan"),
        }
    }
}

impl std::str::FromStr for PermissionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(PermissionMode::Default),
            "acceptedits" | "accept-edits" | "accept_edits" => Ok(PermissionMode::AcceptEdits),
            "bypasspermissions" | "bypass-permissions" | "bypass_permissions" | "bypass" => {
                Ok(PermissionMode::BypassPermissions)
            }
            "plan" | "readonly" | "read-only" | "read_only" => Ok(PermissionMode::Plan),
            _ => Err(format!("Unknown permission mode: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode() {
        let mode = PermissionMode::default();
        assert!(mode.is_default());
        assert!(!mode.allows_all());
        assert!(!mode.is_read_only());
        assert!(!mode.auto_approves_files());
    }

    #[test]
    fn test_accept_edits_mode() {
        let mode = PermissionMode::AcceptEdits;
        assert!(!mode.is_default());
        assert!(!mode.allows_all());
        assert!(!mode.is_read_only());
        assert!(mode.auto_approves_files());
    }

    #[test]
    fn test_bypass_mode() {
        let mode = PermissionMode::BypassPermissions;
        assert!(!mode.is_default());
        assert!(mode.allows_all());
        assert!(!mode.is_read_only());
        assert!(!mode.auto_approves_files());
    }

    #[test]
    fn test_plan_mode() {
        let mode = PermissionMode::Plan;
        assert!(!mode.is_default());
        assert!(!mode.allows_all());
        assert!(mode.is_read_only());
        assert!(!mode.auto_approves_files());
    }

    #[test]
    fn test_display() {
        assert_eq!(PermissionMode::Default.to_string(), "default");
        assert_eq!(PermissionMode::AcceptEdits.to_string(), "acceptEdits");
        assert_eq!(
            PermissionMode::BypassPermissions.to_string(),
            "bypassPermissions"
        );
        assert_eq!(PermissionMode::Plan.to_string(), "plan");
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "default".parse::<PermissionMode>().unwrap(),
            PermissionMode::Default
        );
        assert_eq!(
            "acceptEdits".parse::<PermissionMode>().unwrap(),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            "accept-edits".parse::<PermissionMode>().unwrap(),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            "bypass".parse::<PermissionMode>().unwrap(),
            PermissionMode::BypassPermissions
        );
        assert_eq!(
            "plan".parse::<PermissionMode>().unwrap(),
            PermissionMode::Plan
        );
        assert_eq!(
            "readonly".parse::<PermissionMode>().unwrap(),
            PermissionMode::Plan
        );
    }

    #[test]
    fn test_serde() {
        let mode = PermissionMode::AcceptEdits;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"acceptEdits\"");

        let parsed: PermissionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, mode);
    }
}
