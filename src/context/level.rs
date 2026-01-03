//! Memory level system for hierarchical configuration.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{ContextResult, MemoryContent, MemoryLoader, MemoryProvider, RuleIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemoryLevel {
    Enterprise = 0,
    User = 1,
    Project = 2,
    Local = 3,
}

impl MemoryLevel {
    pub fn all() -> &'static [MemoryLevel] {
        &[
            MemoryLevel::Enterprise,
            MemoryLevel::User,
            MemoryLevel::Project,
            MemoryLevel::Local,
        ]
    }
}

#[derive(Debug, Default)]
pub struct LeveledMemoryProvider {
    enterprise: LevelContent,
    user: LevelContent,
    project: LevelContent,
    local: LevelContent,
}

#[derive(Debug, Default)]
struct LevelContent {
    path: Option<PathBuf>,
    content: Vec<String>,
    rules: Vec<RuleIndex>,
}

impl LeveledMemoryProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_project(project_dir: impl AsRef<Path>) -> Self {
        let dir = project_dir.as_ref();
        Self {
            project: LevelContent {
                path: Some(dir.to_path_buf()),
                ..Default::default()
            },
            local: LevelContent {
                path: Some(dir.to_path_buf()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn with_user(mut self) -> Self {
        if let Some(home) = crate::common::home_dir() {
            self.user.path = Some(home.join(".claude"));
        }
        self
    }

    pub fn with_enterprise(mut self) -> Self {
        #[cfg(target_os = "macos")]
        {
            let path = PathBuf::from("/Library/Application Support/ClaudeCode");
            if path.exists() {
                self.enterprise.path = Some(path);
            }
        }
        #[cfg(target_os = "linux")]
        {
            let path = PathBuf::from("/etc/claude-code");
            if path.exists() {
                self.enterprise.path = Some(path);
            }
        }
        self
    }

    pub fn with_content(mut self, level: MemoryLevel, content: impl Into<String>) -> Self {
        let target = self.level_mut(level);
        target.content.push(content.into());
        self
    }

    pub fn with_rule(mut self, level: MemoryLevel, rule: RuleIndex) -> Self {
        let target = self.level_mut(level);
        target.rules.push(rule);
        self
    }

    pub fn add_content(&mut self, level: MemoryLevel, content: impl Into<String>) {
        self.level_mut(level).content.push(content.into());
    }

    pub fn add_rule(&mut self, level: MemoryLevel, rule: RuleIndex) {
        self.level_mut(level).rules.push(rule);
    }

    fn level_mut(&mut self, level: MemoryLevel) -> &mut LevelContent {
        match level {
            MemoryLevel::Enterprise => &mut self.enterprise,
            MemoryLevel::User => &mut self.user,
            MemoryLevel::Project => &mut self.project,
            MemoryLevel::Local => &mut self.local,
        }
    }

    async fn load_level(
        &self,
        level_content: &LevelContent,
        is_local: bool,
    ) -> ContextResult<MemoryContent> {
        let mut content = MemoryContent::default();

        if let Some(ref path) = level_content.path
            && path.exists()
        {
            let mut loader = MemoryLoader::new();
            if is_local {
                if let Ok(loaded) = loader.load_local_only(path).await {
                    content = loaded;
                }
            } else if let Ok(loaded) = loader.load_all(path).await {
                content = loaded;
            }
        }

        for c in &level_content.content {
            content.claude_md.push(c.clone());
        }

        content
            .rule_indices
            .extend(level_content.rules.iter().cloned());

        Ok(content)
    }
}

#[async_trait]
impl MemoryProvider for LeveledMemoryProvider {
    fn name(&self) -> &str {
        "leveled"
    }

    async fn load(&self) -> ContextResult<MemoryContent> {
        let mut combined = MemoryContent::default();

        // Load in priority order: Enterprise → User → Project → Local
        let enterprise = self.load_level(&self.enterprise, false).await?;
        let user = self.load_level(&self.user, false).await?;
        let project = self.load_level(&self.project, false).await?;
        let local = self.load_level(&self.local, true).await?;

        combined.claude_md.extend(enterprise.claude_md);
        combined.claude_md.extend(user.claude_md);
        combined.claude_md.extend(project.claude_md);
        combined.local_md.extend(local.local_md);
        combined.claude_md.extend(local.claude_md);

        combined.rule_indices.extend(enterprise.rule_indices);
        combined.rule_indices.extend(user.rule_indices);
        combined.rule_indices.extend(project.rule_indices);
        combined.rule_indices.extend(local.rule_indices);

        Ok(combined)
    }

    fn priority(&self) -> i32 {
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_leveled_memory_provider() {
        let provider = LeveledMemoryProvider::new()
            .with_content(MemoryLevel::Enterprise, "# Enterprise Rules")
            .with_content(MemoryLevel::User, "# User Preferences")
            .with_content(MemoryLevel::Project, "# Project Guidelines");

        let content = provider.load().await.unwrap();
        assert_eq!(content.claude_md.len(), 3);
        assert_eq!(content.claude_md[0], "# Enterprise Rules");
        assert_eq!(content.claude_md[1], "# User Preferences");
        assert_eq!(content.claude_md[2], "# Project Guidelines");
    }

    #[test]
    fn test_memory_level_order() {
        assert!(MemoryLevel::Enterprise < MemoryLevel::User);
        assert!(MemoryLevel::User < MemoryLevel::Project);
        assert!(MemoryLevel::Project < MemoryLevel::Local);
    }
}
