//! Permission rules and policy evaluation.

use super::{PermissionMode, is_file_tool, is_read_only_tool};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Decision for a permission check
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Allow the tool execution
    Allow,
    /// Deny the tool execution
    #[default]
    Deny,
    /// Ask the user for approval (only valid in interactive mode)
    Ask,
}

/// Result of a permission check
#[derive(Clone, Debug)]
pub struct PermissionResult {
    /// Whether the tool execution is allowed
    pub allowed: bool,
    /// Reason for the decision
    pub reason: String,
    /// Whether to ask the user (only valid in interactive mode)
    pub ask_user: bool,
}

impl PermissionResult {
    /// Create an allowed result
    pub fn allowed(reason: impl Into<String>) -> Self {
        Self {
            allowed: true,
            reason: reason.into(),
            ask_user: false,
        }
    }

    /// Create a denied result
    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
            ask_user: false,
        }
    }

    /// Create an "ask user" result
    pub fn ask(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
            ask_user: true,
        }
    }

    /// Check if the permission was granted
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }

    /// Check if the permission was denied
    pub fn is_denied(&self) -> bool {
        !self.allowed && !self.ask_user
    }

    /// Check if user approval is needed
    pub fn needs_user_approval(&self) -> bool {
        self.ask_user
    }
}

/// Tool-specific execution limits
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolLimits {
    /// Timeout in milliseconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Maximum output size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_size: Option<usize>,

    /// Maximum number of concurrent executions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent: Option<usize>,

    /// Allowed paths (glob patterns) for file operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_paths: Option<Vec<String>>,

    /// Denied paths (glob patterns) for file operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denied_paths: Option<Vec<String>>,
}

impl ToolLimits {
    /// Create new tool limits with a timeout
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self {
            timeout_ms: Some(timeout_ms),
            ..Default::default()
        }
    }

    /// Create new tool limits with max output size
    pub fn with_max_output(max_bytes: usize) -> Self {
        Self {
            max_output_size: Some(max_bytes),
            ..Default::default()
        }
    }

    /// Add allowed paths
    pub fn with_allowed_paths(mut self, paths: Vec<String>) -> Self {
        self.allowed_paths = Some(paths);
        self
    }

    /// Add denied paths
    pub fn with_denied_paths(mut self, paths: Vec<String>) -> Self {
        self.denied_paths = Some(paths);
        self
    }
}

/// A permission rule that matches tools by pattern
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Tool name pattern (regex)
    pub pattern: String,

    /// Decision to apply when the rule matches
    pub decision: PermissionDecision,

    /// Optional reason for the rule
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Priority (higher = evaluated first)
    #[serde(default)]
    pub priority: i32,

    /// Compiled regex (not serialized)
    #[serde(skip)]
    compiled: Option<Regex>,
}

impl PermissionRule {
    /// Create a new allow rule
    pub fn allow(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            decision: PermissionDecision::Allow,
            reason: None,
            priority: 0,
            compiled: None,
        }
    }

    /// Create a new deny rule
    pub fn deny(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            decision: PermissionDecision::Deny,
            reason: None,
            priority: 0,
            compiled: None,
        }
    }

    /// Create a new ask rule
    pub fn ask(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            decision: PermissionDecision::Ask,
            reason: None,
            priority: 0,
            compiled: None,
        }
    }

    /// Set the reason for this rule
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the priority for this rule
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Compile the regex pattern
    pub fn compile(&mut self) -> Result<(), regex::Error> {
        self.compiled = Some(Regex::new(&self.pattern)?);
        Ok(())
    }

    /// Check if this rule matches a tool name
    pub fn matches(&self, tool_name: &str) -> bool {
        if let Some(ref regex) = self.compiled {
            regex.is_match(tool_name)
        } else if let Ok(regex) = Regex::new(&self.pattern) {
            regex.is_match(tool_name)
        } else {
            // Fallback to exact match if regex is invalid
            self.pattern == tool_name
        }
    }
}

/// Permission policy for controlling tool execution
#[derive(Clone, Debug, Default)]
pub struct PermissionPolicy {
    /// Permission mode
    pub mode: PermissionMode,

    /// Allow rules (evaluated before deny rules)
    pub allow_rules: Vec<PermissionRule>,

    /// Deny rules
    pub deny_rules: Vec<PermissionRule>,

    /// Ask rules (for interactive mode)
    pub ask_rules: Vec<PermissionRule>,

    /// Tool-specific limits
    pub tool_limits: HashMap<String, ToolLimits>,
}

impl PermissionPolicy {
    /// Create a new permission policy with default mode
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for permission policy
    pub fn builder() -> PermissionPolicyBuilder {
        PermissionPolicyBuilder::new()
    }

    /// Create a permissive policy that allows all tools
    pub fn permissive() -> Self {
        Self {
            mode: PermissionMode::BypassPermissions,
            ..Default::default()
        }
    }

    /// Create a read-only policy
    pub fn read_only() -> Self {
        Self {
            mode: PermissionMode::Plan,
            ..Default::default()
        }
    }

    /// Create a policy that allows file operations
    pub fn accept_edits() -> Self {
        Self {
            mode: PermissionMode::AcceptEdits,
            ..Default::default()
        }
    }

    /// Check if a tool can be used with the given input
    pub fn check(&self, tool_name: &str, _input: &Value) -> PermissionResult {
        // 1. Check bypass mode first
        if self.mode.allows_all() {
            return PermissionResult::allowed("Bypass mode: all tools allowed");
        }

        // 2. Check deny rules first (highest priority)
        let mut deny_rules: Vec<_> = self.deny_rules.iter().collect();
        deny_rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        for rule in deny_rules {
            if rule.matches(tool_name) {
                return PermissionResult::denied(
                    rule.reason
                        .clone()
                        .unwrap_or_else(|| format!("Denied by rule: {}", rule.pattern)),
                );
            }
        }

        // 3. Check allow rules
        let mut allow_rules: Vec<_> = self.allow_rules.iter().collect();
        allow_rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        for rule in allow_rules {
            if rule.matches(tool_name) {
                return PermissionResult::allowed(
                    rule.reason
                        .clone()
                        .unwrap_or_else(|| format!("Allowed by rule: {}", rule.pattern)),
                );
            }
        }

        // 4. Check ask rules (for interactive mode)
        let mut ask_rules: Vec<_> = self.ask_rules.iter().collect();
        ask_rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        for rule in ask_rules {
            if rule.matches(tool_name) {
                return PermissionResult::ask(
                    rule.reason
                        .clone()
                        .unwrap_or_else(|| format!("Ask user: {}", rule.pattern)),
                );
            }
        }

        // 5. Apply mode-based defaults
        match self.mode {
            PermissionMode::BypassPermissions => {
                PermissionResult::allowed("Bypass mode: all tools allowed")
            }
            PermissionMode::Plan => {
                if is_read_only_tool(tool_name) {
                    PermissionResult::allowed("Plan mode: read-only tool allowed")
                } else {
                    PermissionResult::denied("Plan mode: only read-only tools allowed")
                }
            }
            PermissionMode::AcceptEdits => {
                if is_file_tool(tool_name) {
                    PermissionResult::allowed("AcceptEdits mode: file tool allowed")
                } else {
                    PermissionResult::denied("AcceptEdits mode: not a file tool")
                }
            }
            PermissionMode::Default => {
                PermissionResult::denied("Default mode: tool not explicitly allowed")
            }
        }
    }

    /// Get limits for a specific tool
    pub fn get_limits(&self, tool_name: &str) -> Option<&ToolLimits> {
        self.tool_limits.get(tool_name)
    }

    /// Add a tool limit
    pub fn set_limits(&mut self, tool_name: impl Into<String>, limits: ToolLimits) {
        self.tool_limits.insert(tool_name.into(), limits);
    }
}

/// Builder for PermissionPolicy
#[derive(Clone, Debug, Default)]
pub struct PermissionPolicyBuilder {
    policy: PermissionPolicy,
}

impl PermissionPolicyBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the permission mode
    pub fn mode(mut self, mode: PermissionMode) -> Self {
        self.policy.mode = mode;
        self
    }

    /// Add an allow pattern
    pub fn allow_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.policy.allow_rules.push(PermissionRule::allow(pattern));
        self
    }

    /// Add an allow rule
    pub fn allow_rule(mut self, rule: PermissionRule) -> Self {
        self.policy.allow_rules.push(rule);
        self
    }

    /// Add a deny pattern
    pub fn deny_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.policy.deny_rules.push(PermissionRule::deny(pattern));
        self
    }

    /// Add a deny rule
    pub fn deny_rule(mut self, rule: PermissionRule) -> Self {
        self.policy.deny_rules.push(rule);
        self
    }

    /// Add an ask pattern (for interactive mode)
    pub fn ask_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.policy.ask_rules.push(PermissionRule::ask(pattern));
        self
    }

    /// Add an ask rule
    pub fn ask_rule(mut self, rule: PermissionRule) -> Self {
        self.policy.ask_rules.push(rule);
        self
    }

    /// Add tool limits
    pub fn tool_limits(mut self, tool_name: impl Into<String>, limits: ToolLimits) -> Self {
        self.policy.tool_limits.insert(tool_name.into(), limits);
        self
    }

    /// Build the permission policy
    pub fn build(mut self) -> PermissionPolicy {
        // Compile all regex patterns
        for rule in &mut self.policy.allow_rules {
            let _ = rule.compile();
        }
        for rule in &mut self.policy.deny_rules {
            let _ = rule.compile();
        }
        for rule in &mut self.policy.ask_rules {
            let _ = rule.compile();
        }
        self.policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_result() {
        let allowed = PermissionResult::allowed("test");
        assert!(allowed.is_allowed());
        assert!(!allowed.is_denied());
        assert!(!allowed.needs_user_approval());

        let denied = PermissionResult::denied("test");
        assert!(!denied.is_allowed());
        assert!(denied.is_denied());
        assert!(!denied.needs_user_approval());

        let ask = PermissionResult::ask("test");
        assert!(!ask.is_allowed());
        assert!(!ask.is_denied());
        assert!(ask.needs_user_approval());
    }

    #[test]
    fn test_permission_rule_exact_match() {
        let rule = PermissionRule::allow("Read");
        assert!(rule.matches("Read"));
        assert!(!rule.matches("Write"));
    }

    #[test]
    fn test_permission_rule_regex() {
        let mut rule = PermissionRule::allow("Read|Write|Edit");
        rule.compile().unwrap();
        assert!(rule.matches("Read"));
        assert!(rule.matches("Write"));
        assert!(rule.matches("Edit"));
        assert!(!rule.matches("Bash"));
    }

    #[test]
    fn test_policy_bypass_mode() {
        let policy = PermissionPolicy::permissive();
        let result = policy.check("AnyTool", &Value::Null);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_policy_plan_mode() {
        let policy = PermissionPolicy::read_only();

        // Read-only tools should be allowed
        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Glob", &Value::Null).is_allowed());
        assert!(policy.check("Grep", &Value::Null).is_allowed());

        // Write tools should be denied
        assert!(policy.check("Write", &Value::Null).is_denied());
        assert!(policy.check("Bash", &Value::Null).is_denied());
    }

    #[test]
    fn test_policy_accept_edits_mode() {
        let policy = PermissionPolicy::accept_edits();

        // File tools should be allowed
        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Write", &Value::Null).is_allowed());
        assert!(policy.check("Edit", &Value::Null).is_allowed());

        // Non-file tools should be denied
        assert!(policy.check("Bash", &Value::Null).is_denied());
        assert!(policy.check("WebSearch", &Value::Null).is_denied());
    }

    #[test]
    fn test_policy_deny_rules_take_precedence() {
        let policy = PermissionPolicy::builder()
            .mode(PermissionMode::AcceptEdits)
            .deny_pattern("Write") // Explicitly deny Write even in AcceptEdits mode
            .build();

        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Write", &Value::Null).is_denied());
    }

    #[test]
    fn test_policy_allow_rules() {
        let policy = PermissionPolicy::builder()
            .mode(PermissionMode::Default)
            .allow_pattern("Bash")
            .allow_pattern("Read")
            .build();

        assert!(policy.check("Bash", &Value::Null).is_allowed());
        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Write", &Value::Null).is_denied());
    }

    #[test]
    fn test_tool_limits() {
        let policy = PermissionPolicy::builder()
            .tool_limits("Bash", ToolLimits::with_timeout(30000))
            .build();

        let limits = policy.get_limits("Bash").unwrap();
        assert_eq!(limits.timeout_ms, Some(30000));
        assert!(policy.get_limits("Read").is_none());
    }
}
