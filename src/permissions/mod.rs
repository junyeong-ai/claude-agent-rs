//! Permission system for controlling tool execution.

mod modes;
mod rules;

pub use modes::PermissionMode;
pub use rules::{
    PermissionDecision, PermissionPolicy, PermissionPolicyBuilder, PermissionResult,
    PermissionRule, ToolLimits,
};

/// Common patterns for read-only tools
pub const READ_ONLY_TOOLS: &[&str] = &["Read", "Glob", "Grep", "WebSearch", "WebFetch"];

/// Common patterns for file modification tools
pub const FILE_TOOLS: &[&str] = &["Read", "Write", "Edit", "Glob", "Grep", "NotebookEdit"];

/// Common patterns for shell/execution tools
pub const SHELL_TOOLS: &[&str] = &["Bash", "KillShell"];

/// Check if a tool is read-only
pub fn is_read_only_tool(tool_name: &str) -> bool {
    READ_ONLY_TOOLS.contains(&tool_name)
}

/// Check if a tool is a file operation tool
pub fn is_file_tool(tool_name: &str) -> bool {
    FILE_TOOLS.contains(&tool_name)
}

/// Check if a tool is a shell/execution tool
pub fn is_shell_tool(tool_name: &str) -> bool {
    SHELL_TOOLS.contains(&tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_read_only_tool() {
        assert!(is_read_only_tool("Read"));
        assert!(is_read_only_tool("Glob"));
        assert!(is_read_only_tool("Grep"));
        assert!(!is_read_only_tool("Write"));
        assert!(!is_read_only_tool("Bash"));
    }

    #[test]
    fn test_is_file_tool() {
        assert!(is_file_tool("Read"));
        assert!(is_file_tool("Write"));
        assert!(is_file_tool("Edit"));
        assert!(!is_file_tool("Bash"));
        assert!(!is_file_tool("WebSearch"));
    }

    #[test]
    fn test_is_shell_tool() {
        assert!(is_shell_tool("Bash"));
        assert!(is_shell_tool("KillShell"));
        assert!(!is_shell_tool("Read"));
        assert!(!is_shell_tool("Write"));
    }
}
