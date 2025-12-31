//! Skill provider trait and implementations.
//!
//! Provides unified interface for loading skills from various sources:
//! - File-based (.claude/skills/, ~/.claude/skills/)
//! - Programmatic (in-memory registration)
//! - Remote (HTTP/database)
//! - Plugin bundles

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{SkillDefinition, SkillSourceType};

/// Error type for skill provider operations.
#[derive(Debug, thiserror::Error)]
pub enum SkillProviderError {
    /// Failed to load skill from source.
    #[error("Failed to load skill: {0}")]
    LoadError(String),
    /// Skill not found.
    #[error("Skill not found: {0}")]
    NotFound(String),
    /// Failed to parse skill file.
    #[error("Parse error: {0}")]
    ParseError(String),
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for skill provider operations.
pub type SkillProviderResult<T> = Result<T, SkillProviderError>;

/// Skill provider trait for loading skills from various sources.
#[async_trait]
pub trait SkillProvider: Send + Sync {
    /// Provider name for identification.
    fn name(&self) -> &str;

    /// List available skill names (for discovery).
    async fn list(&self) -> SkillProviderResult<Vec<String>>;

    /// Load a specific skill by name.
    async fn get(&self, name: &str) -> SkillProviderResult<Option<SkillDefinition>>;

    /// Load all skills.
    async fn load_all(&self) -> SkillProviderResult<Vec<SkillDefinition>>;

    /// Priority (higher = loaded later, overrides earlier).
    fn priority(&self) -> i32 {
        0
    }

    /// Source type for skills from this provider.
    fn source_type(&self) -> SkillSourceType {
        SkillSourceType::User
    }
}

/// In-memory skill provider for programmatic registration.
#[derive(Debug, Clone, Default)]
pub struct InMemorySkillProvider {
    skills: HashMap<String, SkillDefinition>,
    priority: i32,
    source_type: SkillSourceType,
}

impl InMemorySkillProvider {
    /// Create a new in-memory skill provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a skill to the provider.
    pub fn with_skill(mut self, skill: SkillDefinition) -> Self {
        self.skills.insert(skill.name.clone(), skill);
        self
    }

    /// Add a skill to the provider (mutable).
    pub fn add(&mut self, skill: SkillDefinition) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Set the priority level.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the source type.
    pub fn with_source_type(mut self, source_type: SkillSourceType) -> Self {
        self.source_type = source_type;
        self
    }
}

#[async_trait]
impl SkillProvider for InMemorySkillProvider {
    fn name(&self) -> &str {
        "in-memory"
    }

    async fn list(&self) -> SkillProviderResult<Vec<String>> {
        Ok(self.skills.keys().cloned().collect())
    }

    async fn get(&self, name: &str) -> SkillProviderResult<Option<SkillDefinition>> {
        Ok(self.skills.get(name).cloned())
    }

    async fn load_all(&self) -> SkillProviderResult<Vec<SkillDefinition>> {
        Ok(self.skills.values().cloned().collect())
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn source_type(&self) -> SkillSourceType {
        self.source_type.clone()
    }
}

/// File-based skill provider for loading from directories.
pub struct FileSkillProvider {
    paths: Vec<PathBuf>,
    priority: i32,
    source_type: SkillSourceType,
}

impl FileSkillProvider {
    /// Create a new file-based skill provider.
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            priority: 0,
            source_type: SkillSourceType::Project,
        }
    }

    /// Add a path to search for skills.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Add project skills directory (.claude/skills/).
    pub fn with_project_skills(mut self, project_dir: &Path) -> Self {
        self.paths.push(project_dir.join(".claude").join("skills"));
        self
    }

    /// Add user skills directory (~/.claude/skills/).
    pub fn with_user_skills(mut self) -> Self {
        if let Some(home) = dirs::home_dir() {
            self.paths.push(home.join(".claude").join("skills"));
        }
        self.source_type = SkillSourceType::User;
        self
    }

    /// Set the priority level.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the source type.
    pub fn with_source_type(mut self, source_type: SkillSourceType) -> Self {
        self.source_type = source_type;
        self
    }

    async fn load_skill_file(&self, path: &Path) -> SkillProviderResult<SkillDefinition> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| SkillProviderError::LoadError(e.to_string()))?;

        parse_skill_file(&content, path)
    }
}

impl Default for FileSkillProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SkillProvider for FileSkillProvider {
    fn name(&self) -> &str {
        "file"
    }

    async fn list(&self) -> SkillProviderResult<Vec<String>> {
        let skills = self.load_all().await?;
        Ok(skills.into_iter().map(|s| s.name).collect())
    }

    async fn get(&self, name: &str) -> SkillProviderResult<Option<SkillDefinition>> {
        for path in &self.paths {
            if !path.exists() {
                continue;
            }

            // Check for SKILL.md in subdirectory
            let skill_dir = path.join(name);
            let skill_file = skill_dir.join("SKILL.md");
            if skill_file.exists() {
                return Ok(Some(self.load_skill_file(&skill_file).await?));
            }

            // Check for {name}.skill.md
            let skill_file = path.join(format!("{}.skill.md", name));
            if skill_file.exists() {
                return Ok(Some(self.load_skill_file(&skill_file).await?));
            }
        }

        Ok(None)
    }

    async fn load_all(&self) -> SkillProviderResult<Vec<SkillDefinition>> {
        let mut skills = Vec::new();

        for path in &self.paths {
            if !path.exists() {
                continue;
            }

            let mut entries = tokio::fs::read_dir(path)
                .await
                .map_err(|e| SkillProviderError::LoadError(e.to_string()))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| SkillProviderError::LoadError(e.to_string()))?
            {
                let entry_path = entry.path();

                // Directory with SKILL.md
                if entry_path.is_dir() {
                    let skill_file = entry_path.join("SKILL.md");
                    if skill_file.exists() {
                        if let Ok(skill) = self.load_skill_file(&skill_file).await {
                            skills.push(skill);
                        }
                    }
                }

                // .skill.md file
                if entry_path
                    .extension()
                    .map(|e| e == "md")
                    .unwrap_or(false)
                    && entry_path
                        .file_name()
                        .map(|n| n.to_string_lossy().ends_with(".skill.md"))
                        .unwrap_or(false)
                {
                    if let Ok(skill) = self.load_skill_file(&entry_path).await {
                        skills.push(skill);
                    }
                }
            }
        }

        Ok(skills)
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn source_type(&self) -> SkillSourceType {
        self.source_type.clone()
    }
}

/// Chain provider that combines multiple skill providers.
#[derive(Default)]
pub struct ChainSkillProvider {
    providers: Vec<Box<dyn SkillProvider>>,
}

impl ChainSkillProvider {
    /// Create a new chain skill provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a provider to the chain.
    pub fn with(mut self, provider: impl SkillProvider + 'static) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    /// Add a provider to the chain (mutable).
    pub fn add(&mut self, provider: impl SkillProvider + 'static) {
        self.providers.push(Box::new(provider));
    }
}

#[async_trait]
impl SkillProvider for ChainSkillProvider {
    fn name(&self) -> &str {
        "chain"
    }

    async fn list(&self) -> SkillProviderResult<Vec<String>> {
        let mut all_names = Vec::new();
        for provider in &self.providers {
            all_names.extend(provider.list().await?);
        }
        all_names.sort();
        all_names.dedup();
        Ok(all_names)
    }

    async fn get(&self, name: &str) -> SkillProviderResult<Option<SkillDefinition>> {
        // Search in priority order (higher priority = later = wins)
        let mut sorted: Vec<_> = self.providers.iter().collect();
        sorted.sort_by_key(|p| p.priority());

        let mut result = None;
        for provider in sorted {
            if let Some(skill) = provider.get(name).await? {
                result = Some(skill);
            }
        }
        Ok(result)
    }

    async fn load_all(&self) -> SkillProviderResult<Vec<SkillDefinition>> {
        let mut skills_map: HashMap<String, SkillDefinition> = HashMap::new();

        let mut sorted: Vec<_> = self.providers.iter().collect();
        sorted.sort_by_key(|p| p.priority());

        for provider in sorted {
            for skill in provider.load_all().await? {
                skills_map.insert(skill.name.clone(), skill);
            }
        }

        Ok(skills_map.into_values().collect())
    }

    fn priority(&self) -> i32 {
        self.providers
            .iter()
            .map(|p| p.priority())
            .max()
            .unwrap_or(0)
    }
}

/// Parse a skill file with YAML frontmatter.
fn parse_skill_file(content: &str, path: &Path) -> SkillProviderResult<SkillDefinition> {
    let (frontmatter, body) = parse_frontmatter(content);

    let name = frontmatter
        .get("name")
        .cloned()
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_end_matches(".skill").to_string())
        })
        .ok_or_else(|| SkillProviderError::ParseError("Missing skill name".to_string()))?;

    let description = frontmatter
        .get("description")
        .cloned()
        .unwrap_or_else(|| format!("Skill: {}", name));

    let mut skill = SkillDefinition::new(name, description, body);

    if let Some(tools) = frontmatter.get("allowed-tools") {
        skill.allowed_tools = tools.split(',').map(|s| s.trim().to_string()).collect();
    }

    if let Some(model) = frontmatter.get("model") {
        skill.model = Some(model.clone());
    }

    if let Some(triggers) = frontmatter.get("triggers") {
        skill.triggers = triggers.split(',').map(|s| s.trim().to_string()).collect();
    }

    skill.location = Some(path.display().to_string());

    Ok(skill)
}

/// Parse YAML frontmatter from content.
fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let mut frontmatter = HashMap::new();

    if !content.starts_with("---") {
        return (frontmatter, content.to_string());
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return (frontmatter, content.to_string());
    }

    let yaml_content = parts[1].trim();
    let body = parts[2].trim().to_string();

    for line in yaml_content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            frontmatter.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    (frontmatter, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_provider() {
        let skill = SkillDefinition::new("test", "Test skill", "Do something");

        let provider = InMemorySkillProvider::new().with_skill(skill);

        let names = provider.list().await.unwrap();
        assert_eq!(names, vec!["test"]);

        let loaded = provider.get("test").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test");
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: my-skill
description: A test skill
allowed-tools: Read, Grep
---

# Instructions
Do the thing.
"#;

        let (fm, body) = parse_frontmatter(content);
        assert_eq!(fm.get("name"), Some(&"my-skill".to_string()));
        assert_eq!(fm.get("description"), Some(&"A test skill".to_string()));
        assert!(body.contains("Instructions"));
    }

    #[tokio::test]
    async fn test_chain_provider() {
        let low = InMemorySkillProvider::new()
            .with_skill(SkillDefinition::new("shared", "Low", "Low content"))
            .with_priority(0);

        let high = InMemorySkillProvider::new()
            .with_skill(SkillDefinition::new("shared", "High", "High content"))
            .with_priority(10);

        let chain = ChainSkillProvider::new().with(low).with(high);

        let skill = chain.get("shared").await.unwrap().unwrap();
        assert_eq!(skill.description, "High"); // Higher priority wins
    }
}
