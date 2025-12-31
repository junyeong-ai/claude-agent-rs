//! Write tool - creates or overwrites files.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolResult};

/// Tool for writing file contents
pub struct WriteTool {
    working_dir: PathBuf,
}

impl WriteTool {
    /// Create a new Write tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[derive(Debug, Deserialize)]
struct WriteInput {
    file_path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        "Writes content to a file. Creates the file if it doesn't exist, \
         or completely overwrites it if it does. The file_path must be an absolute path."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: WriteInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // Resolve path
        let path = if input.file_path.starts_with('/') {
            PathBuf::from(&input.file_path)
        } else {
            self.working_dir.join(&input.file_path)
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return ToolResult::error(format!("Failed to create directories: {}", e));
            }
        }

        // Write file
        match tokio::fs::write(&path, &input.content).await {
            Ok(_) => ToolResult::success(format!(
                "Successfully wrote {} bytes to {}",
                input.content.len(),
                path.display()
            )),
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
    async fn test_write_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        let tool = WriteTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "content": "Hello, World!"
            }))
            .await;

        assert!(!result.is_error());

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[tokio::test]
    async fn test_write_creates_directories() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("subdir/nested/test.txt");

        let tool = WriteTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "content": "Nested content"
            }))
            .await;

        assert!(!result.is_error());
        assert!(file_path.exists());
    }
}
