//! Session Cache Manager for Prompt Caching
//!
//! Implements session-level Prompt Caching using Anthropic's cache_control
//! feature for token efficiency.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use super::state::Session;
use crate::context::static_context::{StaticContext, SystemBlock};

/// Cache control type for API requests
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheControlType {
    /// The cache control type (e.g., "ephemeral")
    #[serde(rename = "type")]
    pub control_type: String,
}

impl Default for CacheControlType {
    fn default() -> Self {
        Self {
            control_type: "ephemeral".to_string(),
        }
    }
}

/// Cache statistics for a session
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total cache read tokens
    pub cache_read_tokens: u64,
    /// Total cache creation tokens
    pub cache_creation_tokens: u64,
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
}

impl CacheStats {
    /// Calculate cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    /// Estimated tokens saved through caching
    pub fn tokens_saved(&self) -> u64 {
        // Cache reads are ~90% cheaper than creation
        (self.cache_read_tokens as f64 * 0.9) as u64
    }

    /// Update stats from token usage
    pub fn update(&mut self, cache_read: u64, cache_creation: u64) {
        self.cache_read_tokens += cache_read;
        self.cache_creation_tokens += cache_creation;

        if cache_read > 0 {
            self.cache_hits += 1;
        } else if cache_creation > 0 {
            self.cache_misses += 1;
        }
    }
}

/// Session cache manager for optimizing API requests
pub struct SessionCacheManager {
    /// Hash of static context (for cache key)
    static_context_hash: Option<String>,
    /// Cache statistics
    stats: CacheStats,
    /// Whether caching is enabled
    enabled: bool,
}

impl SessionCacheManager {
    /// Create a new cache manager
    pub fn new() -> Self {
        Self {
            static_context_hash: None,
            stats: CacheStats::default(),
            enabled: true,
        }
    }

    /// Create a disabled cache manager
    pub fn disabled() -> Self {
        Self {
            static_context_hash: None,
            stats: CacheStats::default(),
            enabled: false,
        }
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable caching
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable caching
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get the static context hash
    pub fn static_context_hash(&self) -> Option<&str> {
        self.static_context_hash.as_deref()
    }

    /// Initialize with static context
    pub fn initialize(&mut self, static_ctx: &StaticContext) {
        self.static_context_hash = Some(static_ctx.content_hash());
    }

    /// Build cached system blocks for API request
    ///
    /// Returns system blocks with cache_control set on static parts.
    pub fn build_cached_system(&self, static_ctx: &StaticContext) -> Vec<SystemBlock> {
        if !self.enabled {
            return static_ctx
                .to_system_blocks()
                .into_iter()
                .map(|mut b| {
                    b.cache_control = None;
                    b
                })
                .collect();
        }

        static_ctx.to_system_blocks()
    }

    /// Check if the static context has changed (cache invalidation)
    pub fn has_context_changed(&self, static_ctx: &StaticContext) -> bool {
        match &self.static_context_hash {
            Some(hash) => *hash != static_ctx.content_hash(),
            None => true,
        }
    }

    /// Update cache after a context change
    pub fn update_context(&mut self, static_ctx: &StaticContext) {
        self.static_context_hash = Some(static_ctx.content_hash());
    }

    /// Record cache usage from API response
    pub fn record_usage(&mut self, cache_read: u64, cache_creation: u64) {
        self.stats.update(cache_read, cache_creation);
    }

    /// Build system messages for a session
    ///
    /// Combines static context (cached) with session-specific context.
    pub fn build_system_for_session(
        &self,
        static_ctx: &StaticContext,
        session: &Session,
    ) -> Vec<SystemBlock> {
        let mut blocks = self.build_cached_system(static_ctx);

        // Add session-specific context (not cached)
        if let Some(summary) = &session.summary {
            blocks.push(SystemBlock::uncached(format!(
                "[Session Summary]\n{}",
                summary
            )));
        }

        blocks
    }

    /// Compute a hash for cache key purposes
    pub fn compute_hash<T: Hash>(value: &T) -> String {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Reset cache statistics
    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }
}

impl Default for SessionCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for session cache configuration
pub struct CacheConfigBuilder {
    enabled: bool,
    breakpoints: Vec<CacheBreakpoint>,
}

/// A cache breakpoint (where to add cache_control)
#[derive(Clone, Debug)]
pub struct CacheBreakpoint {
    /// Name of this breakpoint
    pub name: String,
    /// Priority (lower = earlier in request)
    pub priority: i32,
}

impl CacheConfigBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            enabled: true,
            breakpoints: Vec::new(),
        }
    }

    /// Disable caching
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add a cache breakpoint
    pub fn with_breakpoint(mut self, name: impl Into<String>, priority: i32) -> Self {
        self.breakpoints.push(CacheBreakpoint {
            name: name.into(),
            priority,
        });
        self
    }

    /// Build the cache manager
    pub fn build(self) -> SessionCacheManager {
        if self.enabled {
            SessionCacheManager::new()
        } else {
            SessionCacheManager::disabled()
        }
    }
}

impl Default for CacheConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_stats_hit_rate() {
        let mut stats = CacheStats::default();

        // No data
        assert_eq!(stats.hit_rate(), 0.0);

        // All hits
        stats.cache_hits = 10;
        stats.cache_misses = 0;
        assert_eq!(stats.hit_rate(), 1.0);

        // 50% hit rate
        stats.cache_hits = 5;
        stats.cache_misses = 5;
        assert_eq!(stats.hit_rate(), 0.5);
    }

    #[test]
    fn test_cache_stats_update() {
        let mut stats = CacheStats::default();

        // Cache hit
        stats.update(100, 0);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_read_tokens, 100);

        // Cache miss
        stats.update(0, 200);
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_creation_tokens, 200);
    }

    #[test]
    fn test_cache_manager_enabled() {
        let manager = SessionCacheManager::new();
        assert!(manager.is_enabled());

        let manager = SessionCacheManager::disabled();
        assert!(!manager.is_enabled());
    }

    #[test]
    fn test_cache_manager_context_change() {
        let mut manager = SessionCacheManager::new();

        let ctx1 = StaticContext::new().with_system_prompt("Prompt A");
        manager.initialize(&ctx1);

        // Same context
        assert!(!manager.has_context_changed(&ctx1));

        // Different context
        let ctx2 = StaticContext::new().with_system_prompt("Prompt B");
        assert!(manager.has_context_changed(&ctx2));
    }

    #[test]
    fn test_cache_manager_build_system() {
        let manager = SessionCacheManager::new();
        let ctx = StaticContext::new()
            .with_system_prompt("You are helpful")
            .with_claude_md("# Project");

        let blocks = manager.build_cached_system(&ctx);
        assert_eq!(blocks.len(), 2);

        // All should have cache control when enabled
        assert!(blocks.iter().all(|b| b.cache_control.is_some()));
    }

    #[test]
    fn test_cache_manager_disabled_no_cache_control() {
        let manager = SessionCacheManager::disabled();
        let ctx = StaticContext::new().with_system_prompt("Test");

        let blocks = manager.build_cached_system(&ctx);
        assert!(blocks.iter().all(|b| b.cache_control.is_none()));
    }

    #[test]
    fn test_cache_config_builder() {
        let manager = CacheConfigBuilder::new()
            .with_breakpoint("system", 0)
            .with_breakpoint("context", 10)
            .build();

        assert!(manager.is_enabled());
    }

    #[test]
    fn test_cache_config_builder_disabled() {
        let manager = CacheConfigBuilder::new().disabled().build();
        assert!(!manager.is_enabled());
    }
}
