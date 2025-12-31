//! Slash command system - user-defined commands from .claude/commands/.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

/// Slash command definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    /// Command name (without leading /)
    pub name: String,
    /// Short description
    pub description: Option<String>,
    /// Command prompt content
    pub content: String,
    /// Source location
    pub location: PathBuf,
    /// Allowed tools (from frontmatter)
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Argument hint for display
    #[serde(default)]
    pub argument_hint: Option<String>,
    /// Model override
    #[serde(default)]
    pub model: Option<String>,
}

impl SlashCommand {
    /// Execute the command with given arguments.
    ///
    /// Supports:
    /// - `$ARGUMENTS` - Full argument string
    /// - `$1`, `$2`, etc. - Positional arguments (space-separated)
    pub fn execute(&self, arguments: &str) -> String {
        let mut result = self.content.clone();

        // Split arguments for positional substitution
        let args: Vec<&str> = arguments.split_whitespace().collect();

        // Replace positional args $1, $2, $3, ... $9
        for (i, arg) in args.iter().take(9).enumerate() {
            result = result.replace(&format!("${}", i + 1), arg);
        }

        // Replace $ARGUMENTS with full string (after positional to avoid conflicts)
        result.replace("$ARGUMENTS", arguments)
    }

    /// Execute with async file reference processing.
    ///
    /// Supports:
    /// - `@path` - Include file contents
    /// - `$ARGUMENTS`, `$1`, `$2` - Argument substitution
    pub async fn execute_with_files(&self, arguments: &str, base_dir: &std::path::Path) -> String {
        let mut result = self.content.clone();

        // Process @path file references
        result = Self::process_file_references(&result, base_dir).await;

        // Split arguments for positional substitution
        let args: Vec<&str> = arguments.split_whitespace().collect();

        // Replace positional args
        for (i, arg) in args.iter().take(9).enumerate() {
            result = result.replace(&format!("${}", i + 1), arg);
        }

        // Replace $ARGUMENTS
        result.replace("$ARGUMENTS", arguments)
    }

    /// Process @path file references in content.
    async fn process_file_references(content: &str, base_dir: &std::path::Path) -> String {
        let mut result = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Check for @path pattern (but not @@escaped)
            if trimmed.starts_with('@') && !trimmed.starts_with("@@") {
                let path_str = trimmed.trim_start_matches('@').trim();
                if !path_str.is_empty() {
                    let full_path = if path_str.starts_with("~/") {
                        // Home directory expansion
                        if let Some(home) = dirs::home_dir() {
                            home.join(path_str.strip_prefix("~/").unwrap_or(path_str))
                        } else {
                            base_dir.join(path_str)
                        }
                    } else if path_str.starts_with('/') {
                        std::path::PathBuf::from(path_str)
                    } else {
                        base_dir.join(path_str)
                    };

                    // Try to read the file
                    if let Ok(file_content) = tokio::fs::read_to_string(&full_path).await {
                        result.push_str(&file_content);
                        result.push('\n');
                        continue;
                    }
                }
            }

            result.push_str(line);
            result.push('\n');
        }

        result
    }
}

/// Frontmatter for slash command files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct CommandFrontmatter {
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default)]
    argument_hint: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

/// Loader for slash commands from .claude/commands/ directories.
#[derive(Debug, Default)]
pub struct CommandLoader {
    commands: HashMap<String, SlashCommand>,
}

impl CommandLoader {
    /// Create a new command loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load commands from project and user directories.
    pub async fn load_all(&mut self, project_dir: &Path) -> Result<()> {
        let project_commands = project_dir.join(".claude").join("commands");
        if project_commands.exists() {
            self.load_directory(&project_commands, "").await?;
        }

        if let Some(home) = dirs::home_dir() {
            let user_commands = home.join(".claude").join("commands");
            if user_commands.exists() {
                self.load_directory(&user_commands, "").await?;
            }
        }

        Ok(())
    }

    /// Load commands from a directory with namespace prefix.
    fn load_directory<'a>(
        &'a mut self,
        dir: &'a Path,
        namespace: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(dir).await.map_err(|e| {
                crate::Error::Config(format!("Failed to read commands directory: {}", e))
            })?;

            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                crate::Error::Config(format!("Failed to read directory entry: {}", e))
            })? {
                let path = entry.path();

                if path.is_dir() {
                    let dir_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();
                    let new_namespace = if namespace.is_empty() {
                        dir_name.to_string()
                    } else {
                        format!("{}:{}", namespace, dir_name)
                    };
                    self.load_directory(&path, &new_namespace).await?;
                } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Ok(cmd) = self.load_file(&path, namespace).await {
                        self.commands.insert(cmd.name.clone(), cmd);
                    }
                }
            }

            Ok(())
        })
    }

    /// Load a single command file.
    async fn load_file(&self, path: &Path, namespace: &str) -> Result<SlashCommand> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| crate::Error::Config(format!("Failed to read command file: {}", e)))?;

        let file_name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let name = if namespace.is_empty() {
            file_name.to_string()
        } else {
            format!("{}:{}", namespace, file_name)
        };

        let (frontmatter, body) = self.parse_frontmatter(&content)?;

        Ok(SlashCommand {
            name,
            description: frontmatter.description,
            content: body,
            location: path.to_path_buf(),
            allowed_tools: frontmatter.allowed_tools,
            argument_hint: frontmatter.argument_hint,
            model: frontmatter.model,
        })
    }

    /// Parse YAML frontmatter from content.
    fn parse_frontmatter(&self, content: &str) -> Result<(CommandFrontmatter, String)> {
        if let Some(after_first) = content.strip_prefix("---") {
            if let Some(end_pos) = after_first.find("---") {
                let frontmatter_str = after_first[..end_pos].trim();
                let body = after_first[end_pos + 3..].trim().to_string();

                let frontmatter: CommandFrontmatter =
                    serde_yaml::from_str(frontmatter_str).unwrap_or_default();

                return Ok((frontmatter, body));
            }
        }

        Ok((CommandFrontmatter::default(), content.to_string()))
    }

    /// Get a command by name.
    pub fn get(&self, name: &str) -> Option<&SlashCommand> {
        self.commands.get(name)
    }

    /// List all loaded commands.
    pub fn list(&self) -> Vec<&SlashCommand> {
        self.commands.values().collect()
    }

    /// Check if a command exists.
    pub fn exists(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argument_substitution() {
        let cmd = SlashCommand {
            name: "test".to_string(),
            description: Some("Test command".to_string()),
            content: "Fix the issue: $ARGUMENTS".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute("bug in login");
        assert_eq!(result, "Fix the issue: bug in login");
    }

    #[test]
    fn test_multiple_argument_substitution() {
        let cmd = SlashCommand {
            name: "test".to_string(),
            description: None,
            content: "First: $ARGUMENTS\nSecond: $ARGUMENTS".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute("value");
        assert!(result.contains("First: value"));
        assert!(result.contains("Second: value"));
    }

    #[test]
    fn test_positional_arguments() {
        let cmd = SlashCommand {
            name: "assign".to_string(),
            description: Some("Assign issue".to_string()),
            content: "Issue: $1, Priority: $2, Assignee: $3".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: Some("[issue] [priority] [assignee]".to_string()),
            model: None,
        };

        let result = cmd.execute("123 high alice");
        assert_eq!(result, "Issue: 123, Priority: high, Assignee: alice");
    }

    #[test]
    fn test_mixed_arguments() {
        let cmd = SlashCommand {
            name: "review".to_string(),
            description: None,
            content: "PR #$1 with args: $ARGUMENTS".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute("456 high priority");
        assert_eq!(result, "PR #456 with args: 456 high priority");
    }

    #[tokio::test]
    async fn test_file_references() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("config.txt"), "test-config")
            .await
            .unwrap();

        let cmd = SlashCommand {
            name: "test".to_string(),
            description: None,
            content: "Config:\n@config.txt\nEnd".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute_with_files("", dir.path()).await;
        assert!(result.contains("test-config"));
        assert!(result.contains("End"));
    }
}
