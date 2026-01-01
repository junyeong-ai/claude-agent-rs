//! Grep tool - content search with regex.

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::process::Command;

use super::{ToolResult, TypedTool};

/// Input for the Grep tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GrepInput {
    /// The regular expression pattern to search for.
    pub pattern: String,
    /// File or directory to search in.
    #[serde(default)]
    pub path: Option<String>,
    /// Glob pattern to filter files (e.g., "*.rs").
    #[serde(default)]
    pub glob: Option<String>,
    /// File type to search (e.g., "rs", "py", "js").
    #[serde(default, rename = "type")]
    pub file_type: Option<String>,
    /// Output mode (default: files_with_matches).
    #[serde(default)]
    pub output_mode: Option<String>,
    /// Case insensitive search.
    #[serde(default, rename = "-i")]
    pub case_insensitive: Option<bool>,
    /// Show line numbers.
    #[serde(default, rename = "-n")]
    pub line_numbers: Option<bool>,
    /// Lines to show after each match.
    #[serde(default, rename = "-A")]
    pub after_context: Option<u32>,
    /// Lines to show before each match.
    #[serde(default, rename = "-B")]
    pub before_context: Option<u32>,
    /// Lines to show before and after each match.
    #[serde(default, rename = "-C")]
    pub context: Option<u32>,
}

/// Tool for searching file contents.
pub struct GrepTool {
    working_dir: PathBuf,
}

impl GrepTool {
    /// Create a new Grep tool.
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl TypedTool for GrepTool {
    type Input = GrepInput;

    const NAME: &'static str = "Grep";
    const DESCRIPTION: &'static str = "A powerful search tool built on ripgrep. Supports full regex syntax. \
        Output modes: 'content' shows matching lines, 'files_with_matches' shows only file paths (default), \
        'count' shows match counts.";

    async fn handle(&self, input: GrepInput) -> ToolResult {
        let mut cmd = Command::new("rg");

        match input.output_mode.as_deref() {
            Some("content") | None => {
                if input.line_numbers.unwrap_or(true) {
                    cmd.arg("-n");
                }
            }
            Some("files_with_matches") => {
                cmd.arg("-l");
            }
            Some("count") => {
                cmd.arg("-c");
            }
            Some(mode) => {
                return ToolResult::error(format!("Unknown output_mode: {}", mode));
            }
        }

        if input.case_insensitive.unwrap_or(false) {
            cmd.arg("-i");
        }

        if let Some(c) = input.context {
            cmd.arg("-C").arg(c.to_string());
        } else {
            if let Some(a) = input.after_context {
                cmd.arg("-A").arg(a.to_string());
            }
            if let Some(b) = input.before_context {
                cmd.arg("-B").arg(b.to_string());
            }
        }

        if let Some(t) = &input.file_type {
            cmd.arg("-t").arg(t);
        }

        if let Some(g) = &input.glob {
            cmd.arg("-g").arg(g);
        }

        cmd.arg(&input.pattern);

        let search_path = input
            .path
            .as_ref()
            .map(|p| super::resolve_path(&self.working_dir, p))
            .unwrap_or_else(|| self.working_dir.clone());
        cmd.arg(&search_path);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = match cmd.output().await {
            Ok(o) => o,
            Err(e) => {
                return ToolResult::error(format!(
                    "Failed to execute ripgrep (is rg installed?): {}",
                    e
                ));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() && !stderr.is_empty() {
            return ToolResult::error(format!("ripgrep error: {}", stderr));
        }

        if stdout.is_empty() {
            ToolResult::success("No matches found")
        } else {
            ToolResult::success(stdout.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_input_parsing() {
        let input: GrepInput = serde_json::from_value(serde_json::json!({
            "pattern": "test",
            "-i": true
        }))
        .unwrap();

        assert_eq!(input.pattern, "test");
        assert_eq!(input.case_insensitive, Some(true));
    }
}
