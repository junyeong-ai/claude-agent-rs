//! CLI Identity - required when using Claude CLI OAuth authentication.

/// The CLI identity statement that MUST be included when using Claude CLI OAuth.
/// This cannot be replaced or removed when using CLI-based authentication.
pub const CLI_IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";
