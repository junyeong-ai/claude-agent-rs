//! Tool-specific prompts and descriptions.

/// Extended description for the Bash tool
pub const BASH_EXTENDED: &str = r#"
## Git Safety Protocol

When making git commits:
- Never update git config
- Never run destructive git commands without explicit request
- Never skip hooks unless explicitly requested
- Never force push to main/master without warning
- Use HEREDOC format for commit messages

## Command Best Practices

- Quote file paths with spaces: `cd "path with spaces"`
- Use `&&` to chain dependent commands
- Avoid interactive flags like `-i`
"#;

/// Extended description for file editing
pub const EDIT_EXTENDED: &str = r#"
## Edit Tool Best Practices

- Always read the file first to understand context
- Preserve exact indentation (tabs vs spaces)
- Make old_string unique enough to match only once
- Use replace_all for renaming variables across the file
"#;
