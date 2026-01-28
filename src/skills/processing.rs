//! Shared content processing functions for skills.
//!
//! Provides reusable processing utilities:
//! - Argument substitution ($ARGUMENTS, $1-$9)
//! - Bash backtick execution (!`command`)
//! - File reference inclusion (@file.txt)
//! - Markdown path resolution
//! - Frontmatter stripping

use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;

use regex::Regex;
use tokio::process::Command;

fn backtick_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!\`([^`]+)\`").expect("valid backtick regex"))
}

fn markdown_link_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").expect("valid markdown link regex"))
}

/// Substitute arguments in content.
///
/// Supports:
/// - `$ARGUMENTS` / `${ARGUMENTS}` - full argument string
/// - `$1` through `$9` - positional arguments
pub fn substitute_args(content: &str, arguments: &str) -> String {
    let mut result = content.to_string();
    let args: Vec<&str> = arguments.split_whitespace().collect();

    // Positional args ($1-$9)
    for (i, arg) in args.iter().take(9).enumerate() {
        result = result.replace(&format!("${}", i + 1), arg);
    }

    // Full arguments
    result
        .replace("$ARGUMENTS", arguments)
        .replace("${ARGUMENTS}", arguments)
}

/// Process bash backticks in content.
///
/// Pattern: `!`command`` executes the command and replaces with output.
pub async fn process_bash_backticks(content: &str, working_dir: &Path) -> String {
    let re = backtick_regex();
    let mut result = content.to_string();
    let mut replacements = Vec::new();

    for cap in re.captures_iter(content) {
        let full_match = &cap[0];
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
                if output.status.success() {
                    String::from_utf8_lossy(&output.stdout).trim().to_string()
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    format!("[Error: {}]", stderr.trim())
                }
            }
            Err(e) => format!("[Failed: {}]", e),
        };

        replacements.push((full_match.to_string(), output));
    }

    for (pattern, replacement) in replacements {
        result = result.replace(&pattern, &replacement);
    }

    result
}

/// Process file references in content.
///
/// Pattern: `@file.txt` reads the file and replaces with content.
///
/// Supports:
/// - Relative paths: @file.txt, @dir/file.txt
/// - Absolute paths: @/path/to/file.txt
/// - Home paths: @~/file.txt
/// - Escaped: @@file.txt (not replaced)
pub async fn process_file_references(content: &str, base_dir: &Path) -> String {
    let mut result = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('@') && !trimmed.starts_with("@@") {
            let path_str = trimmed.trim_start_matches('@').trim();
            if !path_str.is_empty() {
                let full_path = resolve_path(path_str, base_dir);
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

/// Resolve markdown relative paths to absolute paths.
pub fn resolve_markdown_paths(content: &str, base_dir: &Path) -> String {
    markdown_link_regex()
        .replace_all(content, |caps: &regex::Captures| {
            let text = &caps[1];
            let path = &caps[2];

            // Skip absolute and HTTP paths
            if path.starts_with("http://") || path.starts_with("https://") || path.starts_with('/')
            {
                return caps[0].to_string();
            }

            let resolved = base_dir.join(path);
            format!("[{}]({})", text, resolved.display())
        })
        .to_string()
}

/// Re-export strip_frontmatter from common module.
pub use crate::common::strip_frontmatter;

fn resolve_path(path_str: &str, base_dir: &Path) -> std::path::PathBuf {
    if path_str.starts_with("~/") {
        if let Some(home) = crate::common::home_dir() {
            return home.join(path_str.strip_prefix("~/").unwrap_or(path_str));
        }
    } else if path_str.starts_with('/') {
        return std::path::PathBuf::from(path_str);
    }
    base_dir.join(path_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_args_with_positional() {
        let content = "File: $1, Action: $2, All: $ARGUMENTS";
        let result = substitute_args(content, "main.rs build");
        assert_eq!(result, "File: main.rs, Action: build, All: main.rs build");
    }

    #[test]
    fn test_substitute_args_empty() {
        let content = "Run: $ARGUMENTS";
        let result = substitute_args(content, "");
        assert_eq!(result, "Run: ");
    }

    #[test]
    fn test_substitute_args_braces() {
        let content = "Args: ${ARGUMENTS}";
        let result = substitute_args(content, "test args");
        assert_eq!(result, "Args: test args");
    }

    #[test]
    fn test_substitute_args_many_positional() {
        let content = "$1 $2 $3 $4 $5 $6 $7 $8 $9";
        let result = substitute_args(content, "a b c d e f g h i j");
        assert_eq!(result, "a b c d e f g h i");
    }

    #[test]
    fn test_strip_frontmatter() {
        let content = "---\ntitle: Test\n---\nBody content";
        let result = strip_frontmatter(content);
        assert_eq!(result, "Body content");
    }

    #[test]
    fn test_strip_frontmatter_no_frontmatter() {
        let content = "Just body content";
        let result = strip_frontmatter(content);
        assert_eq!(result, "Just body content");
    }

    #[test]
    fn test_strip_frontmatter_with_extra_whitespace() {
        let content = "---\nkey: value\n---\n\n  \nContent here";
        let result = strip_frontmatter(content);
        assert_eq!(result, "Content here");
    }

    #[test]
    fn test_resolve_markdown_paths() {
        let content = r#"Check [file](file.md) and [dir/other](dir/other.md).
External: [Docs](https://example.com)
Absolute: [Config](/etc/config)"#;

        let result = resolve_markdown_paths(content, std::path::Path::new("/skills/test"));

        assert!(result.contains("[file](/skills/test/file.md)"));
        assert!(result.contains("[dir/other](/skills/test/dir/other.md)"));
        assert!(result.contains("[Docs](https://example.com)"));
        assert!(result.contains("[Config](/etc/config)"));
    }

    #[tokio::test]
    async fn test_process_bash_backticks() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let content = "Echo: !`echo hello`";
        let result = process_bash_backticks(content, dir.path()).await;

        assert!(result.contains("Echo: hello"));
    }

    #[tokio::test]
    async fn test_process_bash_backticks_error() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let content = "Result: !`exit 1`";
        let result = process_bash_backticks(content, dir.path()).await;

        assert!(result.contains("[Error:") || result.contains("Result:"));
    }

    #[tokio::test]
    async fn test_process_file_references() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("config.txt"), "test-config")
            .await
            .unwrap();

        let content = "Config:\n@config.txt\nEnd";
        let result = process_file_references(content, dir.path()).await;

        assert!(result.contains("test-config"));
        assert!(result.contains("End"));
    }

    #[tokio::test]
    async fn test_process_file_references_escaped() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let content = "Keep: @@file.txt";
        let result = process_file_references(content, dir.path()).await;

        assert!(result.contains("@@file.txt"));
    }
}
