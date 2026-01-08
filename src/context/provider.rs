//! Memory provider trait and implementations.

use std::path::PathBuf;

use async_trait::async_trait;

use super::{ContextResult, MemoryContent};

pub const MAX_IMPORT_DEPTH: usize = 5;

#[async_trait]
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn load(&self) -> ContextResult<MemoryContent>;
    fn priority(&self) -> i32 {
        0
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryProvider {
    pub system_prompt: Option<String>,
    pub claude_md: Vec<String>,
    pub local_md: Vec<String>,
    pub priority: i32,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_claude_md(mut self, content: impl Into<String>) -> Self {
        self.claude_md.push(content.into());
        self
    }

    pub fn with_local_md(mut self, content: impl Into<String>) -> Self {
        self.local_md.push(content.into());
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl MemoryProvider for InMemoryProvider {
    fn name(&self) -> &str {
        "in-memory"
    }

    async fn load(&self) -> ContextResult<MemoryContent> {
        let mut content = MemoryContent::default();

        if let Some(ref prompt) = self.system_prompt {
            content.claude_md.push(prompt.clone());
        }

        content.claude_md.extend(self.claude_md.clone());
        content.local_md.extend(self.local_md.clone());

        Ok(content)
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

pub struct FileMemoryProvider {
    path: PathBuf,
    priority: i32,
}

impl FileMemoryProvider {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            priority: 0,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl MemoryProvider for FileMemoryProvider {
    fn name(&self) -> &str {
        "file"
    }

    async fn load(&self) -> ContextResult<MemoryContent> {
        let mut loader = super::MemoryLoader::new();
        loader.load(&self.path).await
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_provider() {
        let provider = InMemoryProvider::new()
            .with_system_prompt("You are a helpful assistant.")
            .with_claude_md("# Project Rules");

        let content = provider.load().await.unwrap();
        assert_eq!(content.claude_md.len(), 2);
    }
}
