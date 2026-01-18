//! Model-related constants and utilities.

/// Context window sizes for Claude models.
pub mod context_window {
    pub const OPUS: u64 = 200_000;
    pub const SONNET: u64 = 200_000;
    pub const HAIKU: u64 = 200_000;
    pub const DEFAULT: u64 = 128_000;

    pub fn for_model(model: &str) -> u64 {
        match model {
            m if m.contains("opus") => OPUS,
            m if m.contains("sonnet") => SONNET,
            m if m.contains("haiku") => HAIKU,
            _ => DEFAULT,
        }
    }
}

/// Maximum output tokens for Claude models.
pub mod output_tokens {
    pub const OPUS: u64 = 64_000;
    pub const SONNET: u64 = 64_000;
    pub const HAIKU: u64 = 64_000;
    pub const DEFAULT: u64 = 8_192;

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

    #[test]
    fn test_output_tokens_for_model() {
        assert_eq!(output_tokens::for_model("claude-opus-4-5"), 64_000);
        assert_eq!(output_tokens::for_model("claude-sonnet-4-5"), 64_000);
        assert_eq!(output_tokens::for_model("claude-haiku-4-5"), 64_000);
        assert_eq!(output_tokens::for_model("unknown-model"), 8_192);
    }
}
