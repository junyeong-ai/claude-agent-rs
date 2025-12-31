//! Model-related constants and utilities.

/// Context window sizes for Claude models.
pub mod context_window {
    /// Claude 3.5/4 Opus context window size.
    pub const OPUS: u64 = 200_000;
    /// Claude 3.5/4 Sonnet context window size.
    pub const SONNET: u64 = 200_000;
    /// Claude 3.5/4 Haiku context window size.
    pub const HAIKU: u64 = 200_000;
    /// Default context window for unknown models.
    pub const DEFAULT: u64 = 128_000;

    /// Get context window size for a model.
    pub fn for_model(model: &str) -> u64 {
        match model {
            m if m.contains("opus") => OPUS,
            m if m.contains("sonnet") => SONNET,
            m if m.contains("haiku") => HAIKU,
            _ => DEFAULT,
        }
    }
}

/// Default compact threshold (80% of context window).
pub const DEFAULT_COMPACT_THRESHOLD: f32 = 0.8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_window_for_model() {
        assert_eq!(context_window::for_model("claude-opus-4"), 200_000);
        assert_eq!(context_window::for_model("claude-sonnet-4-5"), 200_000);
        assert_eq!(context_window::for_model("claude-3-haiku"), 200_000);
        assert_eq!(context_window::for_model("unknown-model"), 128_000);
    }
}
