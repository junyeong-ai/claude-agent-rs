//! Agent configuration types.
//!
//! Domain-separated configuration for clarity and maintainability.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::client::messages::DEFAULT_MAX_TOKENS;
use crate::output_style::OutputStyle;
use crate::permissions::PermissionPolicy;
use crate::tools::ToolAccess;

/// Model-related configuration.
#[derive(Debug, Clone)]
pub struct AgentModelConfig {
    /// Primary model for main operations
    pub primary: String,
    /// Smaller model for quick operations
    pub small: String,
    /// Maximum tokens per response
    pub max_tokens: u32,
}

impl Default for AgentModelConfig {
    fn default() -> Self {
        Self {
            primary: crate::client::DEFAULT_MODEL.to_string(),
            small: crate::client::DEFAULT_SMALL_MODEL.to_string(),
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }
}

impl AgentModelConfig {
    pub fn new(primary: impl Into<String>) -> Self {
        Self {
            primary: primary.into(),
            ..Default::default()
        }
    }

    pub fn with_small(mut self, small: impl Into<String>) -> Self {
        self.small = small.into();
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }
}

/// Execution behavior configuration.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Maximum agentic loop iterations
    pub max_iterations: usize,
    /// Overall execution timeout
    pub timeout: Option<Duration>,
    /// Timeout between streaming chunks (detects stalled connections)
    pub chunk_timeout: Duration,
    /// Enable automatic context compaction
    pub auto_compact: bool,
    /// Context usage threshold for compaction (0.0-1.0)
    pub compact_threshold: f32,
    /// Messages to preserve during compaction
    pub compact_keep_messages: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            timeout: Some(Duration::from_secs(300)),
            chunk_timeout: Duration::from_secs(60),
            auto_compact: true,
            compact_threshold: crate::types::DEFAULT_COMPACT_THRESHOLD,
            compact_keep_messages: 4,
        }
    }
}

impl ExecutionConfig {
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    pub fn with_chunk_timeout(mut self, timeout: Duration) -> Self {
        self.chunk_timeout = timeout;
        self
    }

    pub fn with_auto_compact(mut self, enabled: bool) -> Self {
        self.auto_compact = enabled;
        self
    }

    pub fn with_compact_threshold(mut self, threshold: f32) -> Self {
        self.compact_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_compact_keep_messages(mut self, count: usize) -> Self {
        self.compact_keep_messages = count;
        self
    }
}

/// Security and permission configuration.
#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    /// Tool permission policy
    pub permission_policy: PermissionPolicy,
    /// Tool access control
    pub tool_access: ToolAccess,
    /// Environment variables for tool execution
    pub env: HashMap<String, String>,
}

impl SecurityConfig {
    pub fn permissive() -> Self {
        Self {
            permission_policy: PermissionPolicy::permissive(),
            tool_access: ToolAccess::All,
            ..Default::default()
        }
    }

    pub fn read_only() -> Self {
        Self {
            permission_policy: PermissionPolicy::read_only(),
            tool_access: ToolAccess::only(["Read", "Glob", "Grep", "Task", "TaskOutput"]),
            ..Default::default()
        }
    }

    pub fn with_permission_policy(mut self, policy: PermissionPolicy) -> Self {
        self.permission_policy = policy;
        self
    }

    pub fn with_tool_access(mut self, access: ToolAccess) -> Self {
        self.tool_access = access;
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn with_envs(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in vars {
            self.env.insert(k.into(), v.into());
        }
        self
    }
}

/// Budget and cost control configuration.
#[derive(Debug, Clone, Default)]
pub struct BudgetConfig {
    /// Maximum cost in USD
    pub max_cost_usd: Option<f64>,
    /// Tenant identifier for multi-tenant tracking
    pub tenant_id: Option<String>,
    /// Model to fall back to when budget exceeded
    pub fallback_model: Option<String>,
}

impl BudgetConfig {
    pub fn unlimited() -> Self {
        Self::default()
    }

    pub fn with_max_cost(mut self, usd: f64) -> Self {
        self.max_cost_usd = Some(usd);
        self
    }

    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn with_fallback(mut self, model: impl Into<String>) -> Self {
        self.fallback_model = Some(model.into());
        self
    }
}

/// Prompt and output configuration.
#[derive(Debug, Clone, Default)]
pub struct PromptConfig {
    /// Custom system prompt
    pub system_prompt: Option<String>,
    /// How to apply system prompt
    pub system_prompt_mode: SystemPromptMode,
    /// Output style customization
    pub output_style: Option<OutputStyle>,
    /// Structured output schema
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SystemPromptMode {
    /// Replace default system prompt
    #[default]
    Replace,
    /// Append to default system prompt
    Append,
}

impl PromptConfig {
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_append_mode(mut self) -> Self {
        self.system_prompt_mode = SystemPromptMode::Append;
        self
    }

    pub fn with_output_style(mut self, style: OutputStyle) -> Self {
        self.output_style = Some(style);
        self
    }

    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }

    pub fn with_structured_output<T: schemars::JsonSchema>(mut self) -> Self {
        let schema = schemars::schema_for!(T);
        self.output_schema = serde_json::to_value(schema).ok();
        self
    }
}

/// Cache strategy determining which content types to cache.
///
/// Anthropic best practices recommend caching static content (system prompts,
/// tools) with longer TTLs and dynamic content (messages) with shorter TTLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheStrategy {
    /// No caching - all content sent without cache_control
    Disabled,
    /// Cache system prompt only (static content with long TTL)
    SystemOnly,
    /// Cache messages only (last user turn with short TTL)
    MessagesOnly,
    /// Cache both system and messages (recommended)
    #[default]
    Full,
}

impl CacheStrategy {
    /// Returns true if system prompt caching is enabled
    pub fn cache_system(&self) -> bool {
        matches!(self, Self::SystemOnly | Self::Full)
    }

    /// Returns true if message caching is enabled
    pub fn cache_messages(&self) -> bool {
        matches!(self, Self::MessagesOnly | Self::Full)
    }

    /// Returns true if any caching is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

/// Cache configuration for prompt caching.
///
/// Implements Anthropic's prompt caching best practices:
/// - Static content (system prompt, CLAUDE.md) uses longer TTL (1 hour default)
/// - Dynamic content (messages) uses shorter TTL (5 minutes default)
/// - Long TTL content must come before short TTL content in requests
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Cache strategy determining which content types to cache
    pub strategy: CacheStrategy,
    /// TTL for static content (system prompt, tools, CLAUDE.md)
    pub static_ttl: crate::types::CacheTtl,
    /// TTL for message content (last user turn)
    pub message_ttl: crate::types::CacheTtl,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            strategy: CacheStrategy::Full,
            static_ttl: crate::types::CacheTtl::OneHour,
            message_ttl: crate::types::CacheTtl::FiveMinutes,
        }
    }
}

impl CacheConfig {
    /// Create a disabled cache configuration
    pub fn disabled() -> Self {
        Self {
            strategy: CacheStrategy::Disabled,
            ..Default::default()
        }
    }

    /// Create a system-only cache configuration
    pub fn system_only() -> Self {
        Self {
            strategy: CacheStrategy::SystemOnly,
            ..Default::default()
        }
    }

    /// Create a messages-only cache configuration
    pub fn messages_only() -> Self {
        Self {
            strategy: CacheStrategy::MessagesOnly,
            ..Default::default()
        }
    }

    /// Set the cache strategy
    pub fn with_strategy(mut self, strategy: CacheStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the TTL for static content
    pub fn with_static_ttl(mut self, ttl: crate::types::CacheTtl) -> Self {
        self.static_ttl = ttl;
        self
    }

    /// Set the TTL for message content
    pub fn with_message_ttl(mut self, ttl: crate::types::CacheTtl) -> Self {
        self.message_ttl = ttl;
        self
    }

    /// Get message TTL if message caching is enabled, None otherwise.
    ///
    /// This is a convenience method to avoid duplicating the cache_messages() check
    /// at every call site.
    pub fn message_ttl_option(&self) -> Option<crate::types::CacheTtl> {
        if self.strategy.cache_messages() {
            Some(self.message_ttl)
        } else {
            None
        }
    }
}

/// Server-side tools configuration.
///
/// Anthropic's built-in server-side tools (Brave Search, web fetch).
/// These are automatically enabled when "WebSearch" or "WebFetch" are in ToolAccess.
#[derive(Debug, Clone, Default)]
pub struct ServerToolsConfig {
    pub web_search: Option<crate::types::WebSearchTool>,
    pub web_fetch: Option<crate::types::WebFetchTool>,
}

impl ServerToolsConfig {
    pub fn web_search() -> Self {
        Self {
            web_search: Some(crate::types::WebSearchTool::default()),
            web_fetch: None,
        }
    }

    pub fn web_fetch() -> Self {
        Self {
            web_search: None,
            web_fetch: Some(crate::types::WebFetchTool::default()),
        }
    }

    pub fn all() -> Self {
        Self {
            web_search: Some(crate::types::WebSearchTool::default()),
            web_fetch: Some(crate::types::WebFetchTool::default()),
        }
    }

    pub fn with_web_search(mut self, config: crate::types::WebSearchTool) -> Self {
        self.web_search = Some(config);
        self
    }

    pub fn with_web_fetch(mut self, config: crate::types::WebFetchTool) -> Self {
        self.web_fetch = Some(config);
        self
    }
}

/// Complete agent configuration combining all domain configs.
#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    pub model: AgentModelConfig,
    pub execution: ExecutionConfig,
    pub security: SecurityConfig,
    pub budget: BudgetConfig,
    pub prompt: PromptConfig,
    pub cache: CacheConfig,
    pub working_dir: Option<PathBuf>,
    pub server_tools: ServerToolsConfig,
    pub coding_mode: bool,
}

impl AgentConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model(mut self, config: AgentModelConfig) -> Self {
        self.model = config;
        self
    }

    pub fn with_execution(mut self, config: ExecutionConfig) -> Self {
        self.execution = config;
        self
    }

    pub fn with_security(mut self, config: SecurityConfig) -> Self {
        self.security = config;
        self
    }

    pub fn with_budget(mut self, config: BudgetConfig) -> Self {
        self.budget = config;
        self
    }

    pub fn with_prompt(mut self, config: PromptConfig) -> Self {
        self.prompt = config;
        self
    }

    pub fn with_cache(mut self, config: CacheConfig) -> Self {
        self.cache = config;
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_server_tools(mut self, config: ServerToolsConfig) -> Self {
        self.server_tools = config;
        self
    }

    pub fn with_coding_mode(mut self, enabled: bool) -> Self {
        self.coding_mode = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config() {
        let config = AgentModelConfig::new("claude-opus-4")
            .with_small("claude-haiku")
            .with_max_tokens(4096);

        assert_eq!(config.primary, "claude-opus-4");
        assert_eq!(config.small, "claude-haiku");
        assert_eq!(config.max_tokens, 4096);
    }

    #[test]
    fn test_execution_config() {
        let config = ExecutionConfig::default()
            .with_max_iterations(50)
            .with_timeout(Duration::from_secs(600))
            .with_auto_compact(false);

        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.timeout, Some(Duration::from_secs(600)));
        assert!(!config.auto_compact);
    }

    #[test]
    fn test_security_config() {
        let config = SecurityConfig::permissive().with_env("API_KEY", "secret");

        assert_eq!(config.env.get("API_KEY"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_budget_config() {
        let config = BudgetConfig::unlimited()
            .with_max_cost(10.0)
            .with_tenant("org-123")
            .with_fallback("claude-haiku");

        assert_eq!(config.max_cost_usd, Some(10.0));
        assert_eq!(config.tenant_id, Some("org-123".to_string()));
        assert_eq!(config.fallback_model, Some("claude-haiku".to_string()));
    }

    #[test]
    fn test_agent_config() {
        let config = AgentConfig::new()
            .with_model(AgentModelConfig::new("claude-opus-4"))
            .with_budget(BudgetConfig::unlimited().with_max_cost(5.0))
            .with_working_dir("/project");

        assert_eq!(config.model.primary, "claude-opus-4");
        assert_eq!(config.budget.max_cost_usd, Some(5.0));
        assert_eq!(config.working_dir, Some(PathBuf::from("/project")));
    }

    #[test]
    fn test_cache_strategy_default_is_full() {
        let config = CacheConfig::default();
        assert_eq!(config.strategy, CacheStrategy::Full);
        assert_eq!(config.static_ttl, crate::types::CacheTtl::OneHour);
        assert_eq!(config.message_ttl, crate::types::CacheTtl::FiveMinutes);
    }

    #[test]
    fn test_cache_strategy_disabled() {
        let config = CacheConfig::disabled();
        assert_eq!(config.strategy, CacheStrategy::Disabled);
        assert!(!config.strategy.is_enabled());
        assert!(!config.strategy.cache_system());
        assert!(!config.strategy.cache_messages());
    }

    #[test]
    fn test_cache_strategy_system_only() {
        let config = CacheConfig::system_only();
        assert_eq!(config.strategy, CacheStrategy::SystemOnly);
        assert!(config.strategy.is_enabled());
        assert!(config.strategy.cache_system());
        assert!(!config.strategy.cache_messages());
    }

    #[test]
    fn test_cache_strategy_messages_only() {
        let config = CacheConfig::messages_only();
        assert_eq!(config.strategy, CacheStrategy::MessagesOnly);
        assert!(config.strategy.is_enabled());
        assert!(!config.strategy.cache_system());
        assert!(config.strategy.cache_messages());
    }

    #[test]
    fn test_cache_strategy_full() {
        let config = CacheConfig::default();
        assert!(config.strategy.is_enabled());
        assert!(config.strategy.cache_system());
        assert!(config.strategy.cache_messages());
    }

    #[test]
    fn test_cache_config_with_ttl() {
        let config = CacheConfig::default()
            .with_static_ttl(crate::types::CacheTtl::FiveMinutes)
            .with_message_ttl(crate::types::CacheTtl::OneHour);

        assert_eq!(config.static_ttl, crate::types::CacheTtl::FiveMinutes);
        assert_eq!(config.message_ttl, crate::types::CacheTtl::OneHour);
    }
}
