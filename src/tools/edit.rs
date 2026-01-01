//! Edit tool - performs string replacements in files.

use std::path::PathBuf;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{ToolResult, TypedTool};

/// Input for the Edit tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditInput {
    /// The absolute path to the file to modify.
    pub file_path: String,
    /// The text to replace.
    pub old_string: String,
    /// The text to replace it with.
    pub new_string: String,
    /// Replace all occurrences (default false).
    #[serde(default)]
    pub replace_all: bool,
}

/// Tool for editing files via string replacement.
pub struct EditTool {
    working_dir: PathBuf,
}

impl EditTool {
    /// Create a new Edit tool.
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl TypedTool for EditTool {
    type Input = EditInput;

    const NAME: &'static str = "Edit";
    const DESCRIPTION: &'static str = "Performs exact string replacements in files. The old_string must match exactly once \
        in the file unless replace_all is true. Use replace_all for renaming variables or \
        strings across the file.";

    async fn handle(&self, input: EditInput) -> ToolResult {
        if input.old_string == input.new_string {
            return ToolResult::error("old_string and new_string must be different");
        }

        let path = super::resolve_path(&self.working_dir, &input.file_path);

        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        let count = content.matches(&input.old_string).count();

        if count == 0 {
            return ToolResult::error(
                "old_string not found in file. Make sure it matches exactly including whitespace.",
            );
        }

        if count > 1 && !input.replace_all {
            return ToolResult::error(format!(
                "old_string found {} times. Use replace_all=true to replace all, \
                 or provide more context to make it unique.",
                count
            ));
        }

        let new_content = if input.replace_all {
            content.replace(&input.old_string, &input.new_string)
        } else {
            content.replacen(&input.old_string, &input.new_string, 1)
        };

        match tokio::fs::write(&path, &new_content).await {
            Ok(_) => {
                let msg = if input.replace_all {
                    format!("Replaced {} occurrences in {}", count, path.display())
                } else {
                    format!("Replaced 1 occurrence in {}", path.display())
                };
                ToolResult::success(msg)
            }
            Err(e) => ToolResult::error(format!("Failed to write file: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_edit_single() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!").await.unwrap();

        let tool = EditTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "World",
                "new_string": "Rust"
            }))
            .await;

        assert!(!result.is_error());

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, Rust!");
    }

    #[tokio::test]
    async fn test_edit_multiple_without_flag() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "foo bar foo").await.unwrap();

        let tool = EditTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "baz"
            }))
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "foo bar foo").await.unwrap();

        let tool = EditTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "baz",
                "replace_all": true
            }))
            .await;

        assert!(!result.is_error());

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "baz bar baz");
    }
}
