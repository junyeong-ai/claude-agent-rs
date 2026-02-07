//! Built-in subagent definitions matching Claude Code CLI.

use super::SubagentIndex;
use crate::client::ModelType;
use crate::common::{ContentSource, SourceType};

/// Bash agent - Command execution specialist.
/// CLI name: "Bash"
pub fn bash_subagent() -> SubagentIndex {
    SubagentIndex::new(
        "Bash",
        "Command execution specialist for running bash commands. Use this for git operations, command execution, and other terminal tasks.",
    )
    .source(ContentSource::in_memory(
        r#"You are a Bash agent specialized for command execution.

Your task is to execute shell commands efficiently and safely:
- Run git operations (status, diff, log, commit, push, etc.)
- Execute build and test commands
- Perform system operations

Always verify command safety before execution. Return clear, concise results."#,
    ))
    .source_type(SourceType::Builtin)
    .tools(["Bash"])
    .model_type(ModelType::Small)
}

/// Explore agent - Fast codebase exploration.
/// CLI name: "Explore"
pub fn explore_subagent() -> SubagentIndex {
    SubagentIndex::new(
        "Explore",
        "Fast agent specialized for exploring codebases. Use this when you need to quickly find files by patterns, search code for keywords, or answer questions about the codebase. When calling this agent, specify the desired thoroughness level: \"quick\" for basic searches, \"medium\" for moderate exploration, or \"very thorough\" for comprehensive analysis across multiple locations and naming conventions.",
    )
    .source(ContentSource::in_memory(
        r#"You are an Explore agent specialized for investigating codebases.

Your task is to quickly find relevant information through:
- Pattern matching with Glob (e.g., "src/components/**/*.tsx")
- Content search with Grep (e.g., "API endpoints", "function\\s+\\w+")
- File reading with Read

Thoroughness levels:
- "quick": Basic searches, first matches only
- "medium": Moderate exploration, check multiple locations
- "very thorough": Comprehensive analysis across multiple locations and naming conventions

Be thorough but efficient. Return a concise summary of your findings."#,
    ))
    .source_type(SourceType::Builtin)
    .tools(["Read", "Grep", "Glob", "Bash", "TodoWrite", "KillShell"])
    .model_type(ModelType::Small)
}

/// Plan agent - Software architect for implementation planning.
/// CLI name: "Plan"
pub fn plan_subagent() -> SubagentIndex {
    SubagentIndex::new(
        "Plan",
        "Software architect agent for designing implementation plans. Use this when you need to plan the implementation strategy for a task. Returns step-by-step plans, identifies critical files, and considers architectural trade-offs.",
    )
    .source(ContentSource::in_memory(
        r#"You are a Plan agent for designing implementation strategies.

Your task is to:
1. Understand the requirements thoroughly
2. Explore the codebase to understand existing patterns and context
3. Identify critical files that will need modification
4. Design a step-by-step implementation plan
5. Consider architectural trade-offs and potential issues

Present your plan clearly with:
- Numbered implementation steps
- Files to be modified/created
- Potential risks or considerations
- Recommended approach with rationale"#,
    ))
    .source_type(SourceType::Builtin)
    .tools(["Read", "Grep", "Glob", "Bash", "TodoWrite", "KillShell"])
    .model_type(ModelType::Primary)
}

/// General-purpose agent - Full capability for complex tasks.
/// CLI name: "general-purpose"
pub fn general_purpose_subagent() -> SubagentIndex {
    SubagentIndex::new(
        "general-purpose",
        "General-purpose agent for researching complex questions, searching for code, and executing multi-step tasks. When you are searching for a keyword or file and are not confident that you will find the right match in the first few tries, use this agent to perform the search for you.",
    )
    .source(ContentSource::in_memory(
        r#"You are a general-purpose agent capable of handling complex, multi-step tasks.

You have full access to all tools and can:
- Read and modify files
- Execute shell commands
- Search and explore codebases
- Implement features and fix bugs
- Create and manage tasks

Work autonomously and methodically:
1. Understand the task requirements
2. Plan your approach
3. Execute step by step
4. Verify results
5. Return comprehensive results when complete"#,
    ))
    .source_type(SourceType::Builtin)
    .model_type(ModelType::Primary)
}

pub fn builtin_subagents() -> Vec<SubagentIndex> {
    vec![
        bash_subagent(),
        explore_subagent(),
        plan_subagent(),
        general_purpose_subagent(),
    ]
}

pub fn find_builtin(name: &str) -> Option<SubagentIndex> {
    match name {
        "Bash" => Some(bash_subagent()),
        "Explore" => Some(explore_subagent()),
        "Plan" => Some(plan_subagent()),
        "general-purpose" => Some(general_purpose_subagent()),
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
        assert_eq!(builtins.len(), 4);

        let names: Vec<&str> = builtins.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Bash"));
        assert!(names.contains(&"Explore"));
        assert!(names.contains(&"Plan"));
        assert!(names.contains(&"general-purpose"));
    }

    #[test]
    fn test_find_builtin_cli_names() {
        assert!(find_builtin("Bash").is_some());
        assert!(find_builtin("Explore").is_some());
        assert!(find_builtin("Plan").is_some());
        assert!(find_builtin("general-purpose").is_some());
        assert!(find_builtin("nonexistent").is_none());
    }

    #[test]
    fn test_bash_agent_tool_restriction() {
        let bash = bash_subagent();
        assert!(bash.has_tool_restrictions());
        assert!(bash.is_tool_allowed("Bash"));
        assert!(!bash.is_tool_allowed("Read"));
        assert!(!bash.is_tool_allowed("Write"));
    }

    #[test]
    fn test_explore_has_tool_restrictions() {
        let explore = explore_subagent();
        assert!(explore.has_tool_restrictions());
        assert!(explore.is_tool_allowed("Read"));
        assert!(explore.is_tool_allowed("Grep"));
        assert!(explore.is_tool_allowed("Glob"));
        assert!(explore.is_tool_allowed("Bash"));
        // Should NOT allow write operations
        assert!(!explore.is_tool_allowed("Write"));
        assert!(!explore.is_tool_allowed("Edit"));
    }

    #[test]
    fn test_plan_has_tool_restrictions() {
        let plan = plan_subagent();
        assert!(plan.has_tool_restrictions());
        assert!(plan.is_tool_allowed("Read"));
        assert!(plan.is_tool_allowed("Grep"));
        // Should NOT allow write operations
        assert!(!plan.is_tool_allowed("Write"));
        assert!(!plan.is_tool_allowed("Edit"));
    }

    #[test]
    fn test_general_purpose_no_restrictions() {
        let gp = general_purpose_subagent();
        assert!(!gp.has_tool_restrictions());
        assert!(gp.is_tool_allowed("Anything"));
    }
}
