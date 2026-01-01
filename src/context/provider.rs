//! Memory provider trait and implementations.
//!
//! Provides unified interface for loading memory from various sources:
//! - File-based (CLAUDE.md, .claude/rules/)
//! - Programmatic (in-memory)
//! - Remote (HTTP/database)

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;

use super::{ContextError, ContextResult, MemoryContent, RuleFile};

/// Maximum import depth for @import directives.
pub const MAX_IMPORT_DEPTH: usize = 5;

/// Memory provider trait for loading memory from various sources.
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    /// Provider name for identification.
    fn name(&self) -> &str;

    /// Load memory content.
    async fn load(&self) -> ContextResult<MemoryContent>;

    /// Priority (higher = loaded later, overrides earlier).
    fn priority(&self) -> i32 {
        0
    }
}

/// In-memory provider for programmatic memory injection.
#[derive(Debug, Clone, Default)]
pub struct InMemoryProvider {
    /// System prompt content.
    pub system_prompt: Option<String>,
    /// CLAUDE.md equivalent content.
    pub claude_md: Vec<String>,
    /// Local memory content.
    pub local_md: Vec<String>,
    /// Rule definitions.
    pub rules: Vec<RuleFile>,
    /// Priority level.
    pub priority: i32,
}

impl InMemoryProvider {
    /// Create a new in-memory provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Add CLAUDE.md equivalent content.
    pub fn with_claude_md(mut self, content: impl Into<String>) -> Self {
        self.claude_md.push(content.into());
        self
    }

    /// Add a rule.
    pub fn with_rule(mut self, name: impl Into<String>, content: impl Into<String>) -> Self {
        self.rules.push(RuleFile {
            name: name.into(),
            content: content.into(),
            path: PathBuf::new(),
        });
        self
    }

    /// Set the priority level.
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
        content.rules.extend(self.rules.clone());

        Ok(content)
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

/// HTTP provider for loading memory from remote URLs.
#[derive(Debug, Clone)]
pub struct HttpMemoryProvider {
    /// Base URL for memory content.
    pub url: String,
    /// Optional headers.
    pub headers: HashMap<String, String>,
    /// Priority level.
    pub priority: i32,
}

impl HttpMemoryProvider {
    /// Create a new HTTP memory provider with the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: HashMap::new(),
            priority: 0,
        }
    }

    /// Add a custom header to the HTTP request.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Set the priority level.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl MemoryProvider for HttpMemoryProvider {
    fn name(&self) -> &str {
        "http"
    }

    async fn load(&self) -> ContextResult<MemoryContent> {
        let client = reqwest::Client::new();
        let mut request = client.get(&self.url);

        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        let response = request.send().await.map_err(|e| ContextError::Source {
            message: format!("HTTP request failed: {}", e),
        })?;

        let text = response.text().await.map_err(|e| ContextError::Source {
            message: format!("Failed to read response: {}", e),
        })?;

        let mut content = MemoryContent::default();
        content.claude_md.push(text);
        Ok(content)
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

/// Chain provider that combines multiple providers.
#[derive(Default)]
pub struct ChainMemoryProvider {
    providers: Vec<Box<dyn MemoryProvider>>,
}

impl ChainMemoryProvider {
    /// Create a new chain memory provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a provider to the chain (builder pattern).
    pub fn with(mut self, provider: impl MemoryProvider + 'static) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    /// Add a provider to the chain (mutable).
    pub fn add(&mut self, provider: impl MemoryProvider + 'static) {
        self.providers.push(Box::new(provider));
    }
}

#[async_trait]
impl MemoryProvider for ChainMemoryProvider {
    fn name(&self) -> &str {
        "chain"
    }

    async fn load(&self) -> ContextResult<MemoryContent> {
        let mut sorted: Vec<_> = self.providers.iter().collect();
        sorted.sort_by_key(|p| p.priority());

        let mut combined = MemoryContent::default();

        for provider in sorted {
            let content = provider.load().await?;
            combined.claude_md.extend(content.claude_md);
            combined.local_md.extend(content.local_md);
            combined.rules.extend(content.rules);
        }

        Ok(combined)
    }

    fn priority(&self) -> i32 {
        self.providers
            .iter()
            .map(|p| p.priority())
            .max()
            .unwrap_or(0)
    }
}

/// File-based memory provider (wraps MemoryLoader).
pub struct FileMemoryProvider {
    path: PathBuf,
    priority: i32,
}

impl FileMemoryProvider {
    /// Create a new file-based memory provider for the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            priority: 0,
        }
    }

    /// Set the priority level.
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
        loader.load_all(&self.path).await
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
            .with_claude_md("# Project Rules")
            .with_rule("security", "No hardcoded secrets");

        let content = provider.load().await.unwrap();
        assert_eq!(content.claude_md.len(), 2);
        assert_eq!(content.rules.len(), 1);
    }

    #[tokio::test]
    async fn test_chain_provider() {
        let low = InMemoryProvider::new()
            .with_claude_md("Low priority")
            .with_priority(0);

        let high = InMemoryProvider::new()
            .with_claude_md("High priority")
            .with_priority(10);

        let chain = ChainMemoryProvider::new().with(low).with(high);

        let content = chain.load().await.unwrap();
        assert_eq!(content.claude_md.len(), 2);
        assert_eq!(content.claude_md[0], "Low priority");
        assert_eq!(content.claude_md[1], "High priority");
    }
}
