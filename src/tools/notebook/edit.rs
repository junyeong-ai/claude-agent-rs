//! NotebookEdit tool - Jupyter notebook cell editing.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::tools::{ToolResult, TypedTool};

/// Edit mode for notebook cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum EditMode {
    /// Replace the contents of an existing cell.
    #[default]
    Replace,
    /// Insert a new cell.
    Insert,
    /// Delete an existing cell.
    Delete,
}

/// Cell type for notebook cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CellType {
    /// Code cell.
    Code,
    /// Markdown cell.
    Markdown,
}

/// Input for NotebookEdit tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NotebookEditInput {
    /// The absolute path to the Jupyter notebook file to edit
    pub notebook_path: String,

    /// The new source content for the cell
    pub new_source: String,

    /// The ID of the cell to edit. For insert, the new cell will be inserted after this cell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_id: Option<String>,

    /// The type of the cell (code or markdown). Required for insert mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_type: Option<CellType>,

    /// The type of edit to make (replace, insert, delete). Defaults to replace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_mode: Option<EditMode>,
}

/// Output from NotebookEdit tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookEditOutput {
    /// Status message
    pub message: String,

    /// The type of edit performed
    pub edit_type: String,

    /// The cell ID that was affected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_id: Option<String>,

    /// Total number of cells after edit
    pub total_cells: usize,
}

/// NotebookEdit tool for editing Jupyter notebooks.
///
/// This tool replaces, inserts, or deletes cells in a Jupyter notebook (.ipynb file).
/// The notebook_path must be an absolute path.
pub struct NotebookEditTool {
    working_dir: PathBuf,
}

impl NotebookEditTool {
    /// Create a new NotebookEditTool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Generate a new cell ID
    fn generate_cell_id() -> String {
        uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
    }

    /// Create a new notebook cell
    fn create_cell(cell_type: CellType, source: &str, cell_id: &str) -> Value {
        let source_lines: Vec<String> = source.lines().map(|l| format!("{}\n", l)).collect();

        match cell_type {
            CellType::Code => json!({
                "cell_type": "code",
                "execution_count": null,
                "id": cell_id,
                "metadata": {},
                "outputs": [],
                "source": source_lines
            }),
            CellType::Markdown => json!({
                "cell_type": "markdown",
                "id": cell_id,
                "metadata": {},
                "source": source_lines
            }),
        }
    }

    /// Get the cell ID from a cell
    fn get_cell_id(cell: &Value) -> Option<String> {
        cell.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

impl Default for NotebookEditTool {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_default())
    }
}

#[async_trait]
impl TypedTool for NotebookEditTool {
    type Input = NotebookEditInput;

    const NAME: &'static str = "NotebookEdit";
    const DESCRIPTION: &'static str = r#"Completely replaces the contents of a specific cell in a Jupyter notebook (.ipynb file).

Jupyter notebooks are interactive documents that combine code, text, and visualizations,
commonly used for data analysis and scientific computing.

The notebook_path parameter must be an absolute path, not a relative path.
The cell_number is 0-indexed.

Edit modes:
- replace: Replace the contents of an existing cell (default)
- insert: Add a new cell after the cell with cell_id (or at the beginning if not specified)
- delete: Delete the cell at the specified cell_id

When using edit_mode=insert, cell_type is required (code or markdown)."#;

    async fn handle(&self, input: NotebookEditInput) -> ToolResult {
        let edit_mode = input.edit_mode.unwrap_or_default();
        let path = crate::tools::resolve_path(&self.working_dir, &input.notebook_path);

        // Validate path extension
        if path.extension().map(|e| e.to_string_lossy().to_lowercase()) != Some("ipynb".to_string())
        {
            return ToolResult::error("File must be a Jupyter notebook (.ipynb)");
        }

        // For insert mode, cell_type is required
        if edit_mode == EditMode::Insert && input.cell_type.is_none() {
            return ToolResult::error("cell_type is required for insert mode");
        }

        // Read the notebook file
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read notebook: {}", e)),
        };

        // Parse the notebook JSON
        let mut notebook: Value = match serde_json::from_str(&content) {
            Ok(n) => n,
            Err(e) => return ToolResult::error(format!("Invalid notebook format: {}", e)),
        };

        // Get cells array
        let cells = match notebook.get_mut("cells").and_then(|c| c.as_array_mut()) {
            Some(c) => c,
            None => return ToolResult::error("Notebook has no cells array"),
        };

        // Perform the edit based on mode
        let (message, affected_cell_id) = match edit_mode {
            EditMode::Replace => {
                // Find cell by ID
                let cell_id = match &input.cell_id {
                    Some(id) => id.clone(),
                    None => return ToolResult::error("cell_id is required for replace mode"),
                };

                let cell_idx = cells
                    .iter()
                    .position(|c| Self::get_cell_id(c) == Some(cell_id.clone()));

                match cell_idx {
                    Some(idx) => {
                        // Get current cell type if not specified
                        let cell_type = input.cell_type.unwrap_or_else(|| {
                            if cells[idx].get("cell_type").and_then(|v| v.as_str())
                                == Some("markdown")
                            {
                                CellType::Markdown
                            } else {
                                CellType::Code
                            }
                        });

                        // Replace the cell
                        cells[idx] = Self::create_cell(cell_type, &input.new_source, &cell_id);
                        (format!("Replaced cell {}", cell_id), Some(cell_id))
                    }
                    None => {
                        return ToolResult::error(format!("Cell with ID '{}' not found", cell_id))
                    }
                }
            }

            EditMode::Insert => {
                let cell_type = input.cell_type.unwrap(); // Already validated above
                let new_id = Self::generate_cell_id();
                let new_cell = Self::create_cell(cell_type, &input.new_source, &new_id);

                // Find insertion point
                let insert_idx = match &input.cell_id {
                    Some(after_id) => {
                        let idx = cells
                            .iter()
                            .position(|c| Self::get_cell_id(c) == Some(after_id.clone()));
                        match idx {
                            Some(i) => i + 1, // Insert after the found cell
                            None => {
                                return ToolResult::error(format!(
                                    "Cell with ID '{}' not found",
                                    after_id
                                ))
                            }
                        }
                    }
                    None => 0, // Insert at the beginning
                };

                cells.insert(insert_idx, new_cell);
                (
                    format!(
                        "Inserted new {} cell with ID {}",
                        if cell_type == CellType::Code {
                            "code"
                        } else {
                            "markdown"
                        },
                        new_id
                    ),
                    Some(new_id),
                )
            }

            EditMode::Delete => {
                let cell_id = match &input.cell_id {
                    Some(id) => id.clone(),
                    None => return ToolResult::error("cell_id is required for delete mode"),
                };

                let cell_idx = cells
                    .iter()
                    .position(|c| Self::get_cell_id(c) == Some(cell_id.clone()));

                match cell_idx {
                    Some(idx) => {
                        cells.remove(idx);
                        (format!("Deleted cell {}", cell_id), Some(cell_id))
                    }
                    None => {
                        return ToolResult::error(format!("Cell with ID '{}' not found", cell_id))
                    }
                }
            }
        };

        // Get cells count before dropping the mutable borrow
        let total_cells = cells.len();

        // Write the modified notebook back
        let pretty_json = match serde_json::to_string_pretty(&notebook) {
            Ok(j) => j,
            Err(e) => return ToolResult::error(format!("Failed to serialize notebook: {}", e)),
        };

        if let Err(e) = tokio::fs::write(&path, &pretty_json).await {
            return ToolResult::error(format!("Failed to write notebook: {}", e));
        }

        let output = NotebookEditOutput {
            message: message.clone(),
            edit_type: format!("{:?}", edit_mode).to_lowercase(),
            cell_id: affected_cell_id,
            total_cells,
        };

        ToolResult::success(format!(
            "{}\nTotal cells: {}",
            output.message, output.total_cells
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_notebook(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        let notebook = json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "id": "cell1",
                    "metadata": {},
                    "source": ["# Hello World\n"]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "id": "cell2",
                    "metadata": {},
                    "outputs": [],
                    "source": ["print('hello')\n"]
                }
            ],
            "metadata": {
                "kernelspec": {
                    "display_name": "Python 3",
                    "language": "python",
                    "name": "python3"
                }
            },
            "nbformat": 4,
            "nbformat_minor": 5
        });

        fs::write(&path, serde_json::to_string_pretty(&notebook).unwrap())
            .await
            .unwrap();
        path
    }

    #[tokio::test]
    async fn test_replace_cell() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = create_test_notebook(&temp_dir, "test.ipynb").await;
        let tool = NotebookEditTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "notebook_path": notebook_path.to_string_lossy(),
                "cell_id": "cell2",
                "new_source": "print('world')",
                "edit_mode": "replace"
            }))
            .await;

        assert!(!result.is_error());

        // Verify the change
        let content = fs::read_to_string(&notebook_path).await.unwrap();
        let notebook: Value = serde_json::from_str(&content).unwrap();
        let cells = notebook["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 2);

        let source = cells[1]["source"].as_array().unwrap();
        assert!(source[0].as_str().unwrap().contains("world"));
    }

    #[tokio::test]
    async fn test_insert_cell() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = create_test_notebook(&temp_dir, "test.ipynb").await;
        let tool = NotebookEditTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "notebook_path": notebook_path.to_string_lossy(),
                "cell_id": "cell1",
                "cell_type": "code",
                "new_source": "x = 42",
                "edit_mode": "insert"
            }))
            .await;

        assert!(!result.is_error());

        // Verify the change
        let content = fs::read_to_string(&notebook_path).await.unwrap();
        let notebook: Value = serde_json::from_str(&content).unwrap();
        let cells = notebook["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_cell() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = create_test_notebook(&temp_dir, "test.ipynb").await;
        let tool = NotebookEditTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "notebook_path": notebook_path.to_string_lossy(),
                "cell_id": "cell1",
                "new_source": "",
                "edit_mode": "delete"
            }))
            .await;

        assert!(!result.is_error());

        // Verify the change
        let content = fs::read_to_string(&notebook_path).await.unwrap();
        let notebook: Value = serde_json::from_str(&content).unwrap();
        let cells = notebook["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 1);
    }

    #[tokio::test]
    async fn test_invalid_file_extension() {
        let temp_dir = TempDir::new().unwrap();
        let tool = NotebookEditTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "notebook_path": "/path/to/file.txt",
                "cell_id": "cell1",
                "new_source": "test"
            }))
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_insert_without_cell_type() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = create_test_notebook(&temp_dir, "test.ipynb").await;
        let tool = NotebookEditTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "notebook_path": notebook_path.to_string_lossy(),
                "new_source": "test",
                "edit_mode": "insert"
            }))
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_cell_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = create_test_notebook(&temp_dir, "test.ipynb").await;
        let tool = NotebookEditTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "notebook_path": notebook_path.to_string_lossy(),
                "cell_id": "nonexistent",
                "new_source": "test",
                "edit_mode": "replace"
            }))
            .await;

        assert!(result.is_error());
    }

    #[test]
    fn test_tool_definition() {
        let tool = NotebookEditTool::default();

        assert_eq!(tool.name(), "NotebookEdit");
        assert!(!tool.description().is_empty());

        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["notebook_path"].is_object());
        assert!(schema["properties"]["new_source"].is_object());
    }
}
