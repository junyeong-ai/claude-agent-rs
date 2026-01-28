//! Permission rules and policy evaluation.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{PermissionMode, is_file_tool, is_read_only_tool};

/// Permission decision for SDK.
///
/// SDK only supports Allow/Deny. For interactive approval workflows,
/// implement your own pre-check hook using the HookManager.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    Allow,
    #[default]
    Deny,
}

impl PermissionDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionStatus {
    Allowed,
    Denied,
}

#[derive(Clone, Debug)]
pub struct PermissionResult {
    pub status: PermissionStatus,
    pub reason: String,
    pub tool_name: Option<String>,
    pub input: Option<Value>,
}

impl PermissionResult {
    pub fn allowed(reason: impl Into<String>) -> Self {
        Self {
            status: PermissionStatus::Allowed,
            reason: reason.into(),
            tool_name: None,
            input: None,
        }
    }

    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            status: PermissionStatus::Denied,
            reason: reason.into(),
            tool_name: None,
            input: None,
        }
    }

    pub fn is_allowed(&self) -> bool {
        matches!(self.status, PermissionStatus::Allowed)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self.status, PermissionStatus::Denied)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_size: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_paths: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub denied_paths: Option<Vec<String>>,
}

impl ToolLimits {
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self {
            timeout_ms: Some(timeout_ms),
            ..Default::default()
        }
    }

    pub fn with_max_output(max_bytes: usize) -> Self {
        Self {
            max_output_size: Some(max_bytes),
            ..Default::default()
        }
    }

    pub fn with_allowed_paths(mut self, paths: Vec<String>) -> Self {
        self.allowed_paths = Some(paths);
        self
    }

    pub fn with_denied_paths(mut self, paths: Vec<String>) -> Self {
        self.denied_paths = Some(paths);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionRule {
    pub pattern: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_pattern: Option<String>,

    pub decision: PermissionDecision,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    #[serde(skip)]
    compiled: Option<Regex>,
}

impl PermissionRule {
    pub fn allow(pattern: impl Into<String>) -> Self {
        Self::new(pattern, PermissionDecision::Allow)
    }

    pub fn deny(pattern: impl Into<String>) -> Self {
        Self::new(pattern, PermissionDecision::Deny)
    }

    fn new(pattern: impl Into<String>, decision: PermissionDecision) -> Self {
        Self {
            pattern: pattern.into(),
            input_pattern: None,
            decision,
            reason: None,
            compiled: None,
        }
    }

    pub fn from_scoped(scoped: &str, decision: PermissionDecision) -> Self {
        if let Some((tool, scope)) = Self::parse_scope(scoped) {
            Self {
                pattern: tool,
                input_pattern: Some(scope),
                decision,
                reason: None,
                compiled: None,
            }
        } else {
            Self::new(scoped, decision)
        }
    }

    pub fn allow_scoped(scoped: &str) -> Self {
        Self::from_scoped(scoped, PermissionDecision::Allow)
    }

    pub fn deny_scoped(scoped: &str) -> Self {
        Self::from_scoped(scoped, PermissionDecision::Deny)
    }

    fn parse_scope(s: &str) -> Option<(String, String)> {
        let start = s.find('(')?;
        let end = s.rfind(')')?;
        if start < end {
            Some((s[..start].to_string(), s[start + 1..end].to_string()))
        } else {
            None
        }
    }

    pub fn with_input_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.input_pattern = Some(pattern.into());
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn compile(&mut self) -> Result<(), regex::Error> {
        self.compiled = Some(Regex::new(&self.pattern)?);
        Ok(())
    }

    pub fn matches(&self, tool_name: &str) -> bool {
        if let Some(ref regex) = self.compiled {
            regex.is_match(tool_name)
        } else if let Ok(regex) = Regex::new(&self.pattern) {
            regex.is_match(tool_name)
        } else {
            self.pattern == tool_name
        }
    }

    pub fn matches_with_input(&self, tool_name: &str, input: &Value) -> bool {
        if !self.matches(tool_name) {
            return false;
        }

        match &self.input_pattern {
            Some(pattern) => self.match_input_pattern(pattern, tool_name, input),
            None => true,
        }
    }

    fn match_input_pattern(&self, pattern: &str, tool_name: &str, input: &Value) -> bool {
        let input_str = match tool_name {
            "Bash" => input.get("command").and_then(|v| v.as_str()),
            "Read" | "Write" | "Edit" => input.get("file_path").and_then(|v| v.as_str()),
            "Glob" | "Grep" => input.get("path").and_then(|v| v.as_str()),
            "WebFetch" => {
                if let Some(domain) = pattern.strip_prefix("domain:") {
                    return input
                        .get("url")
                        .and_then(|v| v.as_str())
                        .map(|url| url.contains(domain))
                        .unwrap_or(false);
                }
                input.get("url").and_then(|v| v.as_str())
            }
            _ => None,
        };

        let Some(input_str) = input_str else {
            return false;
        };

        self.match_pattern(pattern, input_str)
    }

    fn match_pattern(&self, pattern: &str, input: &str) -> bool {
        if pattern.ends_with(":*") || pattern.ends_with("**") {
            let prefix = &pattern[..pattern.len() - 2];
            input.starts_with(prefix)
        } else if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                input.starts_with(parts[0]) && input.ends_with(parts[1])
            } else {
                input == pattern
            }
        } else {
            input == pattern || input.starts_with(&format!("{}/", pattern))
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PermissionPolicy {
    pub mode: PermissionMode,
    pub rules: Vec<PermissionRule>,
    pub tool_limits: HashMap<String, ToolLimits>,
}

impl PermissionPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> PermissionPolicyBuilder {
        PermissionPolicyBuilder::new()
    }

    pub fn permissive() -> Self {
        Self {
            mode: PermissionMode::BypassPermissions,
            ..Default::default()
        }
    }

    pub fn read_only() -> Self {
        Self {
            mode: PermissionMode::Plan,
            ..Default::default()
        }
    }

    pub fn accept_edits() -> Self {
        Self {
            mode: PermissionMode::AcceptEdits,
            ..Default::default()
        }
    }

    pub fn check(&self, tool_name: &str, input: &Value) -> PermissionResult {
        if self.mode.allows_all() {
            return PermissionResult::allowed("Bypass mode: all tools allowed");
        }

        // Deny rules first (highest priority)
        for rule in self
            .rules
            .iter()
            .filter(|r| r.decision == PermissionDecision::Deny)
        {
            if rule.matches_with_input(tool_name, input) {
                return PermissionResult::denied(
                    rule.reason
                        .clone()
                        .unwrap_or_else(|| format!("Denied by rule: {}", rule.pattern)),
                );
            }
        }

        // Allow rules
        for rule in self
            .rules
            .iter()
            .filter(|r| r.decision == PermissionDecision::Allow)
        {
            if rule.matches_with_input(tool_name, input) {
                return PermissionResult::allowed(
                    rule.reason
                        .clone()
                        .unwrap_or_else(|| format!("Allowed by rule: {}", rule.pattern)),
                );
            }
        }

        // Mode-based defaults
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

    pub fn limits(&self, tool_name: &str) -> Option<&ToolLimits> {
        self.tool_limits.get(tool_name)
    }

    pub fn set_limits(&mut self, tool_name: impl Into<String>, limits: ToolLimits) {
        self.tool_limits.insert(tool_name.into(), limits);
    }
}

#[derive(Clone, Debug, Default)]
pub struct PermissionPolicyBuilder {
    policy: PermissionPolicy,
}

impl PermissionPolicyBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mode(mut self, mode: PermissionMode) -> Self {
        self.policy.mode = mode;
        self
    }

    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        let pattern_str = pattern.into();
        let rule = if pattern_str.contains('(') {
            PermissionRule::allow_scoped(&pattern_str)
        } else {
            PermissionRule::allow(pattern_str)
        };
        self.policy.rules.push(rule);
        self
    }

    pub fn deny(mut self, pattern: impl Into<String>) -> Self {
        let pattern_str = pattern.into();
        let rule = if pattern_str.contains('(') {
            PermissionRule::deny_scoped(&pattern_str)
        } else {
            PermissionRule::deny(pattern_str)
        };
        self.policy.rules.push(rule);
        self
    }

    pub fn rule(mut self, rule: PermissionRule) -> Self {
        self.policy.rules.push(rule);
        self
    }

    pub fn tool_limits(mut self, tool_name: impl Into<String>, limits: ToolLimits) -> Self {
        self.policy.tool_limits.insert(tool_name.into(), limits);
        self
    }

    pub fn build(mut self) -> PermissionPolicy {
        for rule in &mut self.policy.rules {
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

        let denied = PermissionResult::denied("test");
        assert!(!denied.is_allowed());
        assert!(denied.is_denied());
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
    fn test_scoped_rule() {
        let rule = PermissionRule::allow_scoped("Bash(git:*)");
        assert_eq!(rule.pattern, "Bash");
        assert_eq!(rule.input_pattern, Some("git:*".to_string()));
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

        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Glob", &Value::Null).is_allowed());
        assert!(policy.check("Grep", &Value::Null).is_allowed());

        assert!(policy.check("Write", &Value::Null).is_denied());
        assert!(policy.check("Bash", &Value::Null).is_denied());
    }

    #[test]
    fn test_policy_accept_edits_mode() {
        let policy = PermissionPolicy::accept_edits();

        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Write", &Value::Null).is_allowed());
        assert!(policy.check("Edit", &Value::Null).is_allowed());

        assert!(policy.check("Bash", &Value::Null).is_denied());
        assert!(policy.check("WebSearch", &Value::Null).is_denied());
    }

    #[test]
    fn test_policy_deny_takes_precedence() {
        let policy = PermissionPolicy::builder()
            .mode(PermissionMode::AcceptEdits)
            .deny("Write")
            .build();

        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Write", &Value::Null).is_denied());
    }

    #[test]
    fn test_policy_allow_rules() {
        let policy = PermissionPolicy::builder()
            .mode(PermissionMode::Default)
            .allow("Bash")
            .allow("Read")
            .build();

        assert!(policy.check("Bash", &Value::Null).is_allowed());
        assert!(policy.check("Read", &Value::Null).is_allowed());
        assert!(policy.check("Write", &Value::Null).is_denied());
    }

    #[test]
    fn test_scoped_allow() {
        let policy = PermissionPolicy::builder()
            .mode(PermissionMode::Default)
            .allow("Bash(git:*)")
            .build();

        let git_input = serde_json::json!({"command": "git status"});
        let rm_input = serde_json::json!({"command": "rm -rf /"});

        assert!(policy.check("Bash", &git_input).is_allowed());
        assert!(policy.check("Bash", &rm_input).is_denied());
    }

    #[test]
    fn test_tool_limits() {
        let policy = PermissionPolicy::builder()
            .tool_limits("Bash", ToolLimits::with_timeout(30000))
            .build();

        let limits = policy.limits("Bash").unwrap();
        assert_eq!(limits.timeout_ms, Some(30000));
        assert!(policy.limits("Read").is_none());
    }

    #[test]
    fn test_domain_filter() {
        let policy = PermissionPolicy::builder()
            .mode(PermissionMode::Default)
            .allow("WebFetch(domain:github.com)")
            .build();

        let github_input = serde_json::json!({"url": "https://github.com/user/repo"});
        let other_input = serde_json::json!({"url": "https://example.com/page"});

        assert!(policy.check("WebFetch", &github_input).is_allowed());
        assert!(policy.check("WebFetch", &other_input).is_denied());
    }
}
