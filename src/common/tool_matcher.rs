//! Tool name matching utilities.

/// Checks if a tool name matches an allowed tool pattern.
///
/// Supports patterns like:
/// - `"Read"` - exact match
/// - `"Bash(git:*)"` - scoped pattern (matches base tool "Bash")
pub fn matches_tool_pattern(pattern: &str, tool_name: &str) -> bool {
    let base = &pattern[..pattern.find('(').unwrap_or(pattern.len())];
    base == tool_name || pattern == tool_name
}

/// Checks if a tool is allowed based on a list of allowed patterns.
///
/// Returns `true` if:
/// - The allowed list is empty (no restrictions)
/// - The tool name matches any pattern in the list
pub fn is_tool_allowed(allowed: &[String], tool_name: &str) -> bool {
    if allowed.is_empty() {
        return true;
    }
    allowed.iter().any(|p| matches_tool_pattern(p, tool_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(matches_tool_pattern("Read", "Read"));
        assert!(!matches_tool_pattern("Read", "Write"));
    }

    #[test]
    fn test_scoped_pattern() {
        assert!(matches_tool_pattern("Bash(git:*)", "Bash"));
        assert!(!matches_tool_pattern("Bash(git:*)", "Read"));
    }

    #[test]
    fn test_is_tool_allowed_empty() {
        let allowed: Vec<String> = vec![];
        assert!(is_tool_allowed(&allowed, "Anything"));
    }

    #[test]
    fn test_is_tool_allowed_restricted() {
        let allowed = vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Bash(git:*)".to_string(),
        ];
        assert!(is_tool_allowed(&allowed, "Read"));
        assert!(is_tool_allowed(&allowed, "Grep"));
        assert!(is_tool_allowed(&allowed, "Bash"));
        assert!(!is_tool_allowed(&allowed, "Write"));
    }
}
