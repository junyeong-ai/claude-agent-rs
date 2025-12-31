//! Read tool - reads file contents.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolResult};

/// Tool for reading file contents
pub struct ReadTool {
    working_dir: PathBuf,
}

impl ReadTool {
    /// Create a new Read tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[derive(Debug, Deserialize)]
struct ReadInput {
    file_path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "Reads a file from the local filesystem. The file_path parameter must be an absolute path. \
         By default, it reads up to 2000 lines starting from the beginning of the file. \
         You can optionally specify a line offset and limit."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "The line number to start reading from (1-indexed)"
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: ReadInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

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

        // Apply offset and limit
        let offset = input.offset.unwrap_or(0);
        let limit = input.limit.unwrap_or(2000);

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let selected: Vec<String> = lines
            .into_iter()
            .skip(offset)
            .take(limit)
            .enumerate()
            .map(|(i, line)| {
                let line_num = offset + i + 1;
                // Truncate long lines
                let truncated = if line.len() > 2000 {
                    format!("{}...", &line[..2000])
                } else {
                    line.to_string()
                };
                format!("{:>6}\t{}", line_num, truncated)
            })
            .collect();

        let output = if selected.is_empty() {
            format!(
                "File is empty or offset {} exceeds file length {}",
                offset, total_lines
            )
        } else {
            selected.join("\n")
        };

        ToolResult::success(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_read_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line 1\nline 2\nline 3")
            .await
            .unwrap();

        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "file_path": file_path.to_str().unwrap()
            }))
            .await;

        match result {
            ToolResult::Success(content) => {
                assert!(content.contains("line 1"));
                assert!(content.contains("line 2"));
                assert!(content.contains("line 3"));
            }
            _ => panic!("Expected success"),
        }
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line 1\nline 2\nline 3")
            .await
            .unwrap();

        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 1,
                "limit": 1
            }))
            .await;

        match result {
            ToolResult::Success(content) => {
                assert!(!content.contains("line 1"));
                assert!(content.contains("line 2"));
                assert!(!content.contains("line 3"));
            }
            _ => panic!("Expected success"),
        }
    }
}
