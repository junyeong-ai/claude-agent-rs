//! Tool access control definitions.

use std::collections::HashSet;

/// Controls which tools are available to the agent.
#[derive(Debug, Clone, Default)]
pub enum ToolAccess {
    /// No tools are allowed.
    None,
    /// All tools are allowed.
    #[default]
    All,
    /// Only the specified tools are allowed.
    Only(HashSet<String>),
    /// All tools except the specified ones are allowed.
    Except(HashSet<String>),
}

impl ToolAccess {
    pub fn all() -> Self {
        Self::All
    }

    pub fn none() -> Self {
        Self::None
    }

    pub fn only(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Only(tools.into_iter().map(Into::into).collect())
    }

    pub fn except(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Except(tools.into_iter().map(Into::into).collect())
    }

    #[inline]
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Only(allowed) => allowed.contains(tool_name),
            Self::Except(denied) => !denied.contains(tool_name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_allows_everything() {
        let access = ToolAccess::all();
        assert!(access.is_allowed("Read"));
        assert!(access.is_allowed("Write"));
        assert!(access.is_allowed("AnythingElse"));
    }

    #[test]
    fn test_none_denies_everything() {
        let access = ToolAccess::none();
        assert!(!access.is_allowed("Read"));
        assert!(!access.is_allowed("Write"));
    }

    #[test]
    fn test_only_allows_specified() {
        let access = ToolAccess::only(["Read", "Write"]);
        assert!(access.is_allowed("Read"));
        assert!(access.is_allowed("Write"));
        assert!(!access.is_allowed("Bash"));
        assert!(!access.is_allowed("Edit"));
    }

    #[test]
    fn test_except_denies_specified() {
        let access = ToolAccess::except(["Bash", "KillShell"]);
        assert!(access.is_allowed("Read"));
        assert!(access.is_allowed("Write"));
        assert!(!access.is_allowed("Bash"));
        assert!(!access.is_allowed("KillShell"));
    }
}
