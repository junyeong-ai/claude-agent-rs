//! System prompts based on Claude Code CLI.
//!
//! Structure:
//! - `identity`: CLI identity (required for CLI OAuth authentication)
//! - `base`: Core behavioral guidelines (always included)
//! - `coding`: Software engineering instructions (when keep-coding-instructions=true)
//! - `environment`: Runtime environment block (always included)

pub mod base;
pub mod coding;
pub mod environment;
pub mod identity;

pub use base::{BASE_SYSTEM_PROMPT, MCP_INSTRUCTIONS, TOOL_USAGE_POLICY};
pub use coding::{CODING_INSTRUCTIONS, PR_PROTOCOL, coding_instructions, git_commit_protocol};
pub use environment::environment_block;
pub use identity::CLI_IDENTITY;
