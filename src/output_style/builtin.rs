//! Built-in output styles.
//!
//! These are the standard output styles that come bundled with the SDK,
//! matching the built-in styles from Claude Code.

use crate::common::SourceType;

use super::OutputStyle;

/// Default output style.
///
/// Standard software engineering mode with full coding instructions.
/// This is a null/passthrough style that doesn't modify the default behavior.
pub fn default_style() -> OutputStyle {
    OutputStyle::new("default", "Standard mode with full coding instructions", "")
        .with_source_type(SourceType::Builtin)
        .with_keep_coding_instructions(true)
}

/// Explanatory output style.
///
/// Adds educational insights between coding tasks to explain
/// implementation choices and codebase patterns.
pub fn explanatory_style() -> OutputStyle {
    OutputStyle::new(
        "explanatory",
        "Educational mode that explains implementation choices",
        EXPLANATORY_PROMPT,
    )
    .with_source_type(SourceType::Builtin)
    .with_keep_coding_instructions(true)
}

/// Learning output style.
///
/// Collaborative learn-by-doing mode where Claude asks the user
/// to implement code pieces themselves.
pub fn learning_style() -> OutputStyle {
    OutputStyle::new(
        "learning",
        "Interactive learning mode with guided exercises",
        LEARNING_PROMPT,
    )
    .with_source_type(SourceType::Builtin)
    .with_keep_coding_instructions(true)
}

/// Returns all built-in styles.
pub fn builtin_styles() -> Vec<OutputStyle> {
    vec![default_style(), explanatory_style(), learning_style()]
}

/// Find a built-in style by name.
pub fn find_builtin(name: &str) -> Option<OutputStyle> {
    match name.to_lowercase().as_str() {
        "default" => Some(default_style()),
        "explanatory" => Some(explanatory_style()),
        "learning" => Some(learning_style()),
        _ => None,
    }
}

/// Explanatory style prompt content.
const EXPLANATORY_PROMPT: &str = r#"# Explanatory Mode

When working on tasks, provide educational insights that help the user understand:

## Between Tasks
After completing significant code changes, add an **Insights** section that explains:
- Why you chose this particular approach over alternatives
- Key patterns or idioms you're using and why they're appropriate
- How this change fits into the broader codebase architecture
- Any trade-offs you considered

## During Implementation
- Explain non-obvious code patterns when you use them
- Point out important codebase conventions you're following
- Highlight potential gotchas or edge cases
- Reference relevant documentation or best practices

## Format
Use the following format for insights:

```
## 💡 Insights

### Why This Approach
[Explanation of the chosen approach]

### Key Patterns
[Description of patterns used]

### Trade-offs
[Any trade-offs considered]
```

Keep insights concise but informative. Focus on teaching moments that will help the user become a better developer."#;

/// Learning style prompt content.
const LEARNING_PROMPT: &str = r#"# Learning Mode

This is a collaborative learn-by-doing mode. Your goal is to guide the user through implementing solutions themselves, rather than doing everything for them.

## Approach

1. **Explain the Concept**: Start by explaining what needs to be done and why
2. **Show the Pattern**: Demonstrate with a small example if needed
3. **Guide Implementation**: Let the user implement the main solution
4. **Review and Improve**: Help refine their implementation

## TODO Markers

When you want the user to implement something, use TODO markers:

```rust
// TODO(human): Implement the validation logic here
// Hint: Check that the input is not empty and matches the expected format
```

## Guidelines

- Break complex tasks into smaller, manageable steps
- Provide hints but not complete solutions
- Ask questions that lead to understanding
- Celebrate progress and provide encouragement
- Explain the "why" behind each decision

## Example Interaction

Instead of writing:
```rust
fn validate(input: &str) -> bool {
    !input.is_empty() && input.len() <= 100
}
```

Guide the user:
```rust
fn validate(input: &str) -> bool {
    // TODO(human): What conditions should we check?
    // Hint: Think about edge cases - what if input is empty?
    //       What if it's too long?
    todo!()
}
```

Remember: The goal is learning, not just task completion."#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_style() {
        let style = default_style();
        assert_eq!(style.name, "default");
        assert!(style.is_default());
        assert!(style.keep_coding_instructions);
        assert_eq!(style.source_type, SourceType::Builtin);
    }

    #[test]
    fn test_explanatory_style() {
        let style = explanatory_style();
        assert_eq!(style.name, "explanatory");
        assert!(!style.is_default());
        assert!(style.keep_coding_instructions);
        assert!(style.prompt.contains("Insights"));
    }

    #[test]
    fn test_learning_style() {
        let style = learning_style();
        assert_eq!(style.name, "learning");
        assert!(!style.is_default());
        assert!(style.keep_coding_instructions);
        assert!(style.prompt.contains("TODO(human)"));
    }

    #[test]
    fn test_builtin_styles() {
        let styles = builtin_styles();
        assert_eq!(styles.len(), 3);
        assert!(styles.iter().all(|s| s.source_type == SourceType::Builtin));
    }

    #[test]
    fn test_find_builtin() {
        assert!(find_builtin("default").is_some());
        assert!(find_builtin("Default").is_some()); // case insensitive
        assert!(find_builtin("explanatory").is_some());
        assert!(find_builtin("learning").is_some());
        assert!(find_builtin("nonexistent").is_none());
    }
}
