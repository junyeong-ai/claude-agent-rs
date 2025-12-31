//! Edit tool - performs string replacements in files.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolResult};

/// Tool for editing files via string replacement
pub struct EditTool {
    working_dir: PathBuf,
}

impl EditTool {
    /// Create a new Edit tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[derive(Debug, Deserialize)]
struct EditInput {
    file_path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        "Performs exact string replacements in files. The old_string must match exactly once \
         in the file unless replace_all is true. Use replace_all for renaming variables or \
         strings across the file."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: EditInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        if input.old_string == input.new_string {
            return ToolResult::error("old_string and new_string must be different");
        }

        // Resolve path
        let path = if input.file_path.starts_with('/') {
            PathBuf::from(&input.file_path)
        } else {
            self.working_dir.join(&input.file_path)
        };

        // Read file
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        // Count occurrences
        let count = content.matches(&input.old_string).count();

        if count == 0 {
            return ToolResult::error(
                "old_string not found in file. Make sure it matches exactly including whitespace.".to_string()
            );
        }

        if count > 1 && !input.replace_all {
            return ToolResult::error(format!(
                "old_string found {} times. Use replace_all=true to replace all, \
                 or provide more context to make it unique.",
                count
            ));
        }

        // Perform replacement
        let new_content = if input.replace_all {
            content.replace(&input.old_string, &input.new_string)
        } else {
            content.replacen(&input.old_string, &input.new_string, 1)
        };

        // Write file
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
