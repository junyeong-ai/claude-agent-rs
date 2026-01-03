//! Agent configuration options.

mod build;
mod builder;
#[cfg(feature = "cli-integration")]
mod cli;

pub use builder::{AgentBuilder, DEFAULT_COMPACT_KEEP_MESSAGES};
