//! Grep tool - content search with regex.

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;

use super::{Tool, ToolResult};

/// Tool for searching file contents
pub struct GrepTool {
    working_dir: PathBuf,
}

impl GrepTool {
    /// Create a new Grep tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[derive(Debug, Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default, rename = "type")]
    file_type: Option<String>,
    #[serde(default)]
    output_mode: Option<String>,
    #[serde(default, rename = "-i")]
    case_insensitive: Option<bool>,
    #[serde(default, rename = "-n")]
    line_numbers: Option<bool>,
    #[serde(default, rename = "-A")]
    after_context: Option<u32>,
    #[serde(default, rename = "-B")]
    before_context: Option<u32>,
    #[serde(default, rename = "-C")]
    context: Option<u32>,
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "A powerful search tool built on ripgrep. Supports full regex syntax. \
         Output modes: 'content' shows matching lines, 'files_with_matches' shows only file paths (default), \
         'count' shows match counts."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regular expression pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., \"*.rs\")"
                },
                "type": {
                    "type": "string",
                    "description": "File type to search (e.g., \"rs\", \"py\", \"js\")"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode (default: files_with_matches)"
                },
                "-i": {
                    "type": "boolean",
                    "description": "Case insensitive search"
                },
                "-n": {
                    "type": "boolean",
                    "description": "Show line numbers"
                },
                "-A": {
                    "type": "number",
                    "description": "Lines to show after each match"
                },
                "-B": {
                    "type": "number",
                    "description": "Lines to show before each match"
                },
                "-C": {
                    "type": "number",
                    "description": "Lines to show before and after each match"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: GrepInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // Build rg command
        let mut cmd = Command::new("rg");

        // Output mode
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

        // Case insensitive
        if input.case_insensitive.unwrap_or(false) {
            cmd.arg("-i");
        }

        // Context
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

        // File type filter
        if let Some(t) = &input.file_type {
            cmd.arg("-t").arg(t);
        }

        // Glob filter
        if let Some(g) = &input.glob {
            cmd.arg("-g").arg(g);
        }

        // Pattern
        cmd.arg(&input.pattern);

        // Path
        let search_path = match &input.path {
            Some(p) if p.starts_with('/') => PathBuf::from(p),
            Some(p) => self.working_dir.join(p),
            None => self.working_dir.clone(),
        };
        cmd.arg(&search_path);

        // Execute
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
