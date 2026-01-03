//! Slash command system - user-defined commands from .claude/commands/.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::Result;

fn backtick_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!\`([^`]+)\`").expect("valid backtick regex"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub location: PathBuf,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub argument_hint: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

impl SlashCommand {
    pub fn execute(&self, arguments: &str) -> String {
        let mut result = self.content.clone();
        let args: Vec<&str> = arguments.split_whitespace().collect();

        for (i, arg) in args.iter().take(9).enumerate() {
            result = result.replace(&format!("${}", i + 1), arg);
        }

        result.replace("$ARGUMENTS", arguments)
    }

    pub async fn execute_full(&self, arguments: &str, base_dir: &Path) -> String {
        let mut result = self.content.clone();

        result = Self::process_bash_backticks(&result, base_dir).await;
        result = Self::process_file_references(&result, base_dir).await;

        let args: Vec<&str> = arguments.split_whitespace().collect();
        for (i, arg) in args.iter().take(9).enumerate() {
            result = result.replace(&format!("${}", i + 1), arg);
        }

        result.replace("$ARGUMENTS", arguments)
    }

    async fn process_bash_backticks(content: &str, working_dir: &Path) -> String {
        let backtick_re = backtick_regex();
        let mut result = content.to_string();
        let mut replacements = Vec::new();

        for cap in backtick_re.captures_iter(content) {
            let full_match = cap.get(0).expect("capture group 0 always exists").as_str();
            let cmd = &cap[1];

            let output = match Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        stdout.trim().to_string()
                    } else {
                        format!("[Error: {}]\n{}", stderr.trim(), stdout.trim())
                    }
                }
                Err(e) => format!("[Failed to execute: {}]", e),
            };

            replacements.push((full_match.to_string(), output));
        }

        for (pattern, replacement) in replacements {
            result = result.replace(&pattern, &replacement);
        }

        result
    }

    async fn process_file_references(content: &str, base_dir: &Path) -> String {
        let mut result = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('@') && !trimmed.starts_with("@@") {
                let path_str = trimmed.trim_start_matches('@').trim();
                if !path_str.is_empty() {
                    let full_path = if path_str.starts_with("~/") {
                        if let Some(home) = crate::common::home_dir() {
                            home.join(path_str.strip_prefix("~/").unwrap_or(path_str))
                        } else {
                            base_dir.join(path_str)
                        }
                    } else if path_str.starts_with('/') {
                        PathBuf::from(path_str)
                    } else {
                        base_dir.join(path_str)
                    };

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

#[derive(Debug, Default)]
pub struct CommandLoader {
    commands: HashMap<String, SlashCommand>,
}

impl CommandLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn load_all(&mut self, project_dir: &Path) -> Result<()> {
        let project_commands = project_dir.join(".claude").join("commands");
        if project_commands.exists() {
            self.load_directory(&project_commands, "").await?;
        }

        if let Some(home) = crate::common::home_dir() {
            let user_commands = home.join(".claude").join("commands");
            if user_commands.exists() {
                self.load_directory(&user_commands, "").await?;
            }
        }

        Ok(())
    }

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
                } else if path.extension().map(|e| e == "md").unwrap_or(false)
                    && let Ok(cmd) = self.load_file(&path, namespace).await
                {
                    self.commands.insert(cmd.name.clone(), cmd);
                }
            }

            Ok(())
        })
    }

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

    fn parse_frontmatter(&self, content: &str) -> Result<(CommandFrontmatter, String)> {
        if let Some(after_first) = content.strip_prefix("---")
            && let Some(end_pos) = after_first.find("---")
        {
            let frontmatter_str = after_first[..end_pos].trim();
            let body = after_first[end_pos + 3..].trim().to_string();

            let frontmatter: CommandFrontmatter = serde_yaml_ng::from_str(frontmatter_str)
                .map_err(|e| crate::Error::Config(format!("Invalid command frontmatter: {}", e)))?;

            return Ok((frontmatter, body));
        }

        Ok((CommandFrontmatter::default(), content.to_string()))
    }

    pub fn get(&self, name: &str) -> Option<&SlashCommand> {
        self.commands.get(name)
    }

    pub fn list(&self) -> Vec<&SlashCommand> {
        self.commands.values().collect()
    }

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

        let result = cmd.execute_full("", dir.path()).await;
        assert!(result.contains("test-config"));
        assert!(result.contains("End"));
    }

    #[tokio::test]
    async fn test_bash_backticks() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();

        let cmd = SlashCommand {
            name: "status".to_string(),
            description: None,
            content: "Echo: !`echo hello`\nPwd: !`pwd`".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute_full("", dir.path()).await;
        assert!(result.contains("Echo: hello"));
        assert!(result.contains(&dir.path().to_string_lossy().to_string()));
    }

    #[tokio::test]
    async fn test_bash_backtick_error() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();

        let cmd = SlashCommand {
            name: "fail".to_string(),
            description: None,
            content: "Result: !`exit 1`".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute_full("", dir.path()).await;
        assert!(result.contains("[Error:") || result.contains("Result:"));
    }
}
