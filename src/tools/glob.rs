//! Glob tool - file pattern matching.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolResult};

/// Tool for finding files by pattern
pub struct GlobTool {
    working_dir: PathBuf,
}

impl GlobTool {
    /// Create a new Glob tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[derive(Debug, Deserialize)]
struct GlobInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        "Fast file pattern matching tool. Supports glob patterns like \"**/*.rs\" or \"src/**/*.ts\". \
         Returns matching file paths sorted by modification time."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in (defaults to working directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: GlobInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // Resolve base path
        let base_path = match &input.path {
            Some(p) if p.starts_with('/') => PathBuf::from(p),
            Some(p) => self.working_dir.join(p),
            None => self.working_dir.clone(),
        };

        // Build full pattern
        let full_pattern = base_path.join(&input.pattern);
        let pattern_str = full_pattern.to_string_lossy();

        // Execute glob
        let entries: Vec<PathBuf> = match glob::glob(&pattern_str) {
            Ok(paths) => paths.filter_map(|r| r.ok()).collect(),
            Err(e) => return ToolResult::error(format!("Invalid pattern: {}", e)),
        };

        if entries.is_empty() {
            return ToolResult::success("No files matched the pattern");
        }

        // Sort by modification time (most recent first)
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
