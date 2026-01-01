//! Glob tool - file pattern matching.

use std::path::PathBuf;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{ToolResult, TypedTool};

/// Input for the Glob tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GlobInput {
    /// The glob pattern to match files against.
    pub pattern: String,
    /// The directory to search in (defaults to working directory).
    #[serde(default)]
    pub path: Option<String>,
}

/// Tool for finding files by pattern.
pub struct GlobTool {
    working_dir: PathBuf,
}

impl GlobTool {
    /// Create a new Glob tool.
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl TypedTool for GlobTool {
    type Input = GlobInput;

    const NAME: &'static str = "Glob";
    const DESCRIPTION: &'static str = "Fast file pattern matching tool. Supports glob patterns like \"**/*.rs\" or \"src/**/*.ts\". \
        Returns matching file paths sorted by modification time.";

    async fn handle(&self, input: GlobInput) -> ToolResult {
        let base_path = input
            .path
            .as_ref()
            .map(|p| super::resolve_path(&self.working_dir, p))
            .unwrap_or_else(|| self.working_dir.clone());

        let full_pattern = base_path.join(&input.pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let entries: Vec<PathBuf> = match glob::glob(&pattern_str) {
            Ok(paths) => paths.filter_map(|r| r.ok()).collect(),
            Err(e) => return ToolResult::error(format!("Invalid pattern: {}", e)),
        };

        if entries.is_empty() {
            return ToolResult::success("No files matched the pattern");
        }

        let mut entries_with_time: Vec<(PathBuf, std::time::SystemTime)> = entries
            .into_iter()
            .filter_map(|p| {
                p.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| (p, t))
            })
            .collect();

        entries_with_time.sort_by(|a, b| b.1.cmp(&a.1));

        let output: Vec<String> = entries_with_time
            .into_iter()
            .map(|(p, _)| p.display().to_string())
            .collect();

        ToolResult::success(output.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_glob_pattern() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test1.txt"), "").await.unwrap();
        fs::write(dir.path().join("test2.txt"), "").await.unwrap();
        fs::write(dir.path().join("other.rs"), "").await.unwrap();

        let tool = GlobTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "pattern": "*.txt"
            }))
            .await;

        match result {
            ToolResult::Success(content) => {
                assert!(content.contains("test1.txt"));
                assert!(content.contains("test2.txt"));
                assert!(!content.contains("other.rs"));
            }
            _ => panic!("Expected success"),
        }
    }
}
