//! Write tool - creates or overwrites files.

use std::path::PathBuf;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{ToolResult, TypedTool};

/// Input for the Write tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteInput {
    /// The absolute path to the file to write.
    pub file_path: String,
    /// The content to write to the file.
    pub content: String,
}

/// Tool for writing file contents.
pub struct WriteTool {
    working_dir: PathBuf,
}

impl WriteTool {
    /// Create a new Write tool.
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl TypedTool for WriteTool {
    type Input = WriteInput;

    const NAME: &'static str = "Write";
    const DESCRIPTION: &'static str = "Writes content to a file. Creates the file if it doesn't exist, \
        or completely overwrites it if it does. The file_path must be an absolute path.";

    async fn handle(&self, input: WriteInput) -> ToolResult {
        let path = super::resolve_path(&self.working_dir, &input.file_path);

        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            return ToolResult::error(format!("Failed to create directories: {}", e));
        }

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
    use crate::tools::Tool;
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
