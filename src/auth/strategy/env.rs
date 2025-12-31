//! Environment variable utilities for authentication strategies.

/// Get an optional environment variable.
pub fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Parse a boolean environment variable.
///
/// Returns `true` if the value is "1" or "true" (case-insensitive).
pub fn env_bool(key: &str) -> bool {
    std::env::var(key)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Get an environment variable with fallback keys.
pub fn env_with_fallbacks(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| env_opt(key))
}

/// Get an environment variable with fallback keys and a default value.
pub fn env_with_fallbacks_or(keys: &[&str], default: &str) -> String {
    env_with_fallbacks(keys).unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_bool() {
        std::env::set_var("TEST_BOOL_1", "1");
        std::env::set_var("TEST_BOOL_TRUE", "true");
        std::env::set_var("TEST_BOOL_TRUE_UPPER", "TRUE");
        std::env::set_var("TEST_BOOL_FALSE", "false");
        std::env::set_var("TEST_BOOL_ZERO", "0");

        assert!(env_bool("TEST_BOOL_1"));
        assert!(env_bool("TEST_BOOL_TRUE"));
        assert!(env_bool("TEST_BOOL_TRUE_UPPER"));
        assert!(!env_bool("TEST_BOOL_FALSE"));
        assert!(!env_bool("TEST_BOOL_ZERO"));
        assert!(!env_bool("TEST_BOOL_NONEXISTENT"));

        std::env::remove_var("TEST_BOOL_1");
        std::env::remove_var("TEST_BOOL_TRUE");
        std::env::remove_var("TEST_BOOL_TRUE_UPPER");
        std::env::remove_var("TEST_BOOL_FALSE");
        std::env::remove_var("TEST_BOOL_ZERO");
    }
}
