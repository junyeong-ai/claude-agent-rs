//! Built-in subagent definitions.

use super::{SubagentDefinition, SubagentSourceType};
use crate::client::ModelType;

pub fn explore_subagent() -> SubagentDefinition {
    SubagentDefinition::new(
        "explore",
        "Fast agent for exploring codebases and searching code",
        r#"You are an Explore agent specialized for investigating codebases.

Your task is to quickly find relevant information through:
- Pattern matching with Glob
- Content search with Grep
- File reading with Read

Be thorough but efficient. Return a concise summary of your findings."#,
    )
    .with_source_type(SubagentSourceType::Builtin)
    .with_tools(["Read", "Grep", "Glob", "Bash"])
    .with_model_type(ModelType::Small)
}

pub fn plan_subagent() -> SubagentDefinition {
    SubagentDefinition::new(
        "plan",
        "Software architect agent for designing implementation plans",
        r#"You are a Plan agent for designing implementation strategies.

Your task is to:
1. Understand the requirements
2. Explore the codebase to understand context
3. Design a step-by-step implementation plan
4. Identify potential issues and trade-offs

Present your plan clearly with numbered steps."#,
    )
    .with_source_type(SubagentSourceType::Builtin)
    .with_model_type(ModelType::Primary)
}

pub fn general_subagent() -> SubagentDefinition {
    SubagentDefinition::new(
        "general",
        "General-purpose agent for complex, multi-step tasks",
        r#"You are a general-purpose agent capable of handling complex tasks.

You can:
- Read and modify files
- Execute shell commands
- Search and explore codebases
- Implement features and fix bugs

Work autonomously and return results when complete."#,
    )
    .with_source_type(SubagentSourceType::Builtin)
    .with_model_type(ModelType::Primary)
}

pub fn builtin_subagents() -> Vec<SubagentDefinition> {
    vec![explore_subagent(), plan_subagent(), general_subagent()]
}

pub fn find_builtin(name: &str) -> Option<SubagentDefinition> {
    match name {
        "explore" => Some(explore_subagent()),
        "plan" => Some(plan_subagent()),
        "general" => Some(general_subagent()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ToolRestricted;

    #[test]
    fn test_builtin_subagents() {
        let builtins = builtin_subagents();
        assert_eq!(builtins.len(), 3);

        let names: Vec<&str> = builtins.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"explore"));
        assert!(names.contains(&"plan"));
        assert!(names.contains(&"general"));
    }

    #[test]
    fn test_find_builtin() {
        assert!(find_builtin("explore").is_some());
        assert!(find_builtin("plan").is_some());
        assert!(find_builtin("general").is_some());
        assert!(find_builtin("nonexistent").is_none());
    }

    #[test]
    fn test_explore_has_tool_restrictions() {
        let explore = explore_subagent();
        assert!(explore.has_tool_restrictions());
        assert!(explore.is_tool_allowed("Read"));
        assert!(explore.is_tool_allowed("Grep"));
        assert!(!explore.is_tool_allowed("Write"));
    }

    #[test]
    fn test_general_no_restrictions() {
        let gp = general_subagent();
        assert!(!gp.has_tool_restrictions());
        assert!(gp.is_tool_allowed("Anything"));
    }
}
