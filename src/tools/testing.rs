//! Test utilities for tools module.

#[cfg(test)]
#[allow(dead_code)]
pub mod helpers {
    use crate::tools::ExecutionContext;
    use crate::types::{ToolOutput, ToolResult};
    use std::path::PathBuf;
    use tempfile::TempDir;

    pub struct TestContext {
        pub dir: TempDir,
        pub context: ExecutionContext,
    }

    impl TestContext {
        pub fn new() -> Self {
            let dir = tempfile::tempdir().expect("Failed to create temp directory");
            let root = std::fs::canonicalize(dir.path()).expect("Failed to canonicalize path");
            let context =
                ExecutionContext::from_path(&root).expect("Failed to create ExecutionContext");
            Self { dir, context }
        }

        pub fn root(&self) -> PathBuf {
            std::fs::canonicalize(self.dir.path()).expect("Failed to canonicalize path")
        }

        pub fn path(&self, name: &str) -> PathBuf {
            self.root().join(name)
        }

        pub fn write_file(&self, name: &str, content: &str) -> PathBuf {
            let path = self.dir.path().join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("Failed to create parent directories");
            }
            std::fs::write(&path, content).expect("Failed to write file");
            std::fs::canonicalize(&path).expect("Failed to canonicalize path")
        }

        pub fn read_file(&self, name: &str) -> String {
            let path = self.dir.path().join(name);
            std::fs::read_to_string(path).expect("Failed to read file")
        }

        pub fn file_exists(&self, name: &str) -> bool {
            self.dir.path().join(name).exists()
        }

        pub fn create_dir(&self, name: &str) -> PathBuf {
            let path = self.dir.path().join(name);
            std::fs::create_dir_all(&path).expect("Failed to create directory");
            std::fs::canonicalize(&path).expect("Failed to canonicalize path")
        }
    }

    impl Default for TestContext {
        fn default() -> Self {
            Self::new()
        }
    }

    pub fn assert_tool_success(result: &ToolResult) -> String {
        use crate::types::ToolOutputBlock;
        match &result.output {
            ToolOutput::Success(content) => content.clone(),
            ToolOutput::SuccessBlocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ToolOutputBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            ToolOutput::Empty => String::new(),
            ToolOutput::Error(e) => panic!("Expected success, got error: {}", e),
        }
    }

    pub fn assert_tool_error(result: &ToolResult) -> String {
        match &result.output {
            ToolOutput::Error(e) => e.to_string(),
            ToolOutput::Success(s) => panic!("Expected error, got success: {}", s),
            ToolOutput::SuccessBlocks(_) => panic!("Expected error, got success blocks"),
            ToolOutput::Empty => panic!("Expected error, got empty"),
        }
    }

    pub fn assert_success_contains(result: &ToolResult, expected: &str) {
        let content = assert_tool_success(result);
        assert!(
            content.contains(expected),
            "Expected output to contain '{}', got: {}",
            expected,
            content
        );
    }

    pub fn assert_error_contains(result: &ToolResult, expected: &str) {
        let content = assert_tool_error(result);
        assert!(
            content.contains(expected),
            "Expected error to contain '{}', got: {}",
            expected,
            content
        );
    }
}
