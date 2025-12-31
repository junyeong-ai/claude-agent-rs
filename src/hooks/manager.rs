//! Hook manager for registering and executing hooks.

use super::{Hook, HookContext, HookEvent, HookInput, HookOutput};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Manager for registering and executing hooks.
///
/// The HookManager maintains a collection of hooks and executes them
/// in priority order when events occur.
///
/// # Execution Order
///
/// 1. Hooks are sorted by priority (higher priority first)
/// 2. For each hook that handles the event:
///    a. Check if the hook's tool matcher matches (if applicable)
///    b. Execute the hook with timeout
///    c. If the hook blocks execution, stop and return
/// 3. Merge all hook outputs
///
/// # Example
///
/// ```rust,no_run
/// use claude_agent::hooks::{HookManager, HookEvent, HookInput, HookContext};
///
/// # async fn example() -> Result<(), claude_agent::Error> {
/// let mut manager = HookManager::new();
/// // manager.register(MyHook::new());
///
/// let input = HookInput::pre_tool_use("session-1", "Read", serde_json::json!({}));
/// let ctx = HookContext::new("session-1");
/// let output = manager.execute(HookEvent::PreToolUse, input, &ctx).await?;
///
/// if !output.continue_execution {
///     println!("Execution blocked: {:?}", output.stop_reason);
/// }
/// # Ok(())
/// # }
/// ```
pub struct HookManager {
    /// Registered hooks
    hooks: Vec<Arc<dyn Hook>>,

    /// Default timeout for hook execution (in seconds)
    default_timeout_secs: u64,
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HookManager {
    /// Create a new hook manager
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            default_timeout_secs: 60,
        }
    }

    /// Create a hook manager with a custom default timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            hooks: Vec::new(),
            default_timeout_secs: timeout_secs,
        }
    }

    /// Register a hook
    pub fn register<H: Hook + 'static>(&mut self, hook: H) {
        self.hooks.push(Arc::new(hook));
    }

    /// Register a hook (Arc version)
    pub fn register_arc(&mut self, hook: Arc<dyn Hook>) {
        self.hooks.push(hook);
    }

    /// Unregister a hook by name
    pub fn unregister(&mut self, name: &str) {
        self.hooks.retain(|h| h.name() != name);
    }

    /// Get all registered hook names
    pub fn hook_names(&self) -> Vec<&str> {
        self.hooks.iter().map(|h| h.name()).collect()
    }

    /// Check if a hook is registered
    pub fn has_hook(&self, name: &str) -> bool {
        self.hooks.iter().any(|h| h.name() == name)
    }

    /// Get hooks that handle a specific event
    pub fn hooks_for_event(&self, event: HookEvent) -> Vec<&Arc<dyn Hook>> {
        let mut hooks: Vec<_> = self
            .hooks
            .iter()
            .filter(|h| h.events().contains(&event))
            .collect();

        // Sort by priority (higher first)
        hooks.sort_by_key(|h| std::cmp::Reverse(h.priority()));
        hooks
    }

    /// Execute hooks for an event
    ///
    /// Returns the merged output from all hooks. If any hook blocks execution,
    /// the remaining hooks are not executed.
    pub async fn execute(
        &self,
        event: HookEvent,
        input: HookInput,
        ctx: &HookContext,
    ) -> Result<HookOutput, crate::Error> {
        let hooks = self.hooks_for_event(event);

        if hooks.is_empty() {
            return Ok(HookOutput::allow());
        }

        let mut merged_output = HookOutput::allow();

        for hook in hooks {
            // Check tool matcher if applicable
            if let (Some(matcher), Some(tool_name)) = (hook.tool_matcher(), &input.tool_name) {
                if !matcher.is_match(tool_name) {
                    continue;
                }
            }

            // Execute with timeout
            let hook_timeout = hook.timeout_secs().max(self.default_timeout_secs);
            let result = timeout(
                Duration::from_secs(hook_timeout),
                hook.execute(input.clone(), ctx),
            )
            .await;

            let output = match result {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    // Hook execution failed - log but continue
                    tracing::warn!(
                        hook = hook.name(),
                        error = %e,
                        "Hook execution failed"
                    );
                    continue;
                }
                Err(_) => {
                    // Hook timed out - log but continue
                    tracing::warn!(
                        hook = hook.name(),
                        timeout_secs = hook_timeout,
                        "Hook execution timed out"
                    );
                    continue;
                }
            };

            // Merge output
            merged_output = Self::merge_outputs(merged_output, output);

            // If execution is blocked, stop processing
            if !merged_output.continue_execution {
                break;
            }
        }

        Ok(merged_output)
    }

    /// Execute hooks with a custom handler for each hook output
    pub async fn execute_with_handler<F>(
        &self,
        event: HookEvent,
        input: HookInput,
        ctx: &HookContext,
        mut handler: F,
    ) -> Result<HookOutput, crate::Error>
    where
        F: FnMut(&str, &HookOutput),
    {
        let hooks = self.hooks_for_event(event);

        if hooks.is_empty() {
            return Ok(HookOutput::allow());
        }

        let mut merged_output = HookOutput::allow();

        for hook in hooks {
            // Check tool matcher if applicable
            if let (Some(matcher), Some(tool_name)) = (hook.tool_matcher(), &input.tool_name) {
                if !matcher.is_match(tool_name) {
                    continue;
                }
            }

            // Execute with timeout
            let hook_timeout = hook.timeout_secs().max(self.default_timeout_secs);
            let result = timeout(
                Duration::from_secs(hook_timeout),
                hook.execute(input.clone(), ctx),
            )
            .await;

            let output = match result {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    tracing::warn!(
                        hook = hook.name(),
                        error = %e,
                        "Hook execution failed"
                    );
                    continue;
                }
                Err(_) => {
                    tracing::warn!(
                        hook = hook.name(),
                        timeout_secs = hook_timeout,
                        "Hook execution timed out"
                    );
                    continue;
                }
            };

            // Call handler
            handler(hook.name(), &output);

            // Merge output
            merged_output = Self::merge_outputs(merged_output, output);

            // If execution is blocked, stop processing
            if !merged_output.continue_execution {
                break;
            }
        }

        Ok(merged_output)
    }

    /// Merge two hook outputs
    fn merge_outputs(base: HookOutput, new: HookOutput) -> HookOutput {
        HookOutput {
            // If any hook blocks, block
            continue_execution: base.continue_execution && new.continue_execution,

            // Use the most recent stop reason
            stop_reason: new.stop_reason.or(base.stop_reason),

            // If any hook suppresses, suppress
            suppress_output: base.suppress_output || new.suppress_output,

            // Use the most recent system message
            system_message: new.system_message.or(base.system_message),

            // Use the most recent permission decision
            permission_decision: new.permission_decision.or(base.permission_decision),

            // Use the most recent updated input
            updated_input: new.updated_input.or(base.updated_input),

            // Concatenate additional context
            additional_context: match (base.additional_context, new.additional_context) {
                (Some(a), Some(b)) => Some(format!("{}\n{}", a, b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },

            // Use the most recent user message
            user_message: new.user_message.or(base.user_message),
        }
    }
}

impl std::fmt::Debug for HookManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookManager")
            .field("hook_count", &self.hooks.len())
            .field("hook_names", &self.hook_names())
            .field("default_timeout_secs", &self.default_timeout_secs)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct TestHook {
        name: String,
        events: Vec<HookEvent>,
        priority: i32,
        block: bool,
    }

    impl TestHook {
        fn new(name: impl Into<String>, events: Vec<HookEvent>, priority: i32) -> Self {
            Self {
                name: name.into(),
                events,
                priority,
                block: false,
            }
        }

        fn blocking(name: impl Into<String>, events: Vec<HookEvent>, priority: i32) -> Self {
            Self {
                name: name.into(),
                events,
                priority,
                block: true,
            }
        }
    }

    #[async_trait]
    impl Hook for TestHook {
        fn name(&self) -> &str {
            &self.name
        }

        fn events(&self) -> &[HookEvent] {
            &self.events
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        async fn execute(
            &self,
            _input: HookInput,
            _ctx: &HookContext,
        ) -> Result<HookOutput, crate::Error> {
            if self.block {
                Ok(HookOutput::block(format!("Blocked by {}", self.name)))
            } else {
                Ok(HookOutput::allow())
            }
        }
    }

    #[tokio::test]
    async fn test_hook_registration() {
        let mut manager = HookManager::new();
        manager.register(TestHook::new("hook1", vec![HookEvent::PreToolUse], 0));
        manager.register(TestHook::new("hook2", vec![HookEvent::PostToolUse], 0));

        assert!(manager.has_hook("hook1"));
        assert!(manager.has_hook("hook2"));
        assert!(!manager.has_hook("hook3"));
        assert_eq!(manager.hook_names().len(), 2);
    }

    #[tokio::test]
    async fn test_hook_unregistration() {
        let mut manager = HookManager::new();
        manager.register(TestHook::new("hook1", vec![HookEvent::PreToolUse], 0));
        manager.register(TestHook::new("hook2", vec![HookEvent::PreToolUse], 0));

        manager.unregister("hook1");

        assert!(!manager.has_hook("hook1"));
        assert!(manager.has_hook("hook2"));
    }

    #[tokio::test]
    async fn test_hooks_for_event() {
        let mut manager = HookManager::new();
        manager.register(TestHook::new("hook1", vec![HookEvent::PreToolUse], 10));
        manager.register(TestHook::new(
            "hook2",
            vec![HookEvent::PreToolUse, HookEvent::PostToolUse],
            5,
        ));
        manager.register(TestHook::new("hook3", vec![HookEvent::SessionStart], 0));

        let pre_hooks = manager.hooks_for_event(HookEvent::PreToolUse);
        assert_eq!(pre_hooks.len(), 2);
        // Check priority order (hook1 has higher priority)
        assert_eq!(pre_hooks[0].name(), "hook1");
        assert_eq!(pre_hooks[1].name(), "hook2");

        let session_hooks = manager.hooks_for_event(HookEvent::SessionStart);
        assert_eq!(session_hooks.len(), 1);
        assert_eq!(session_hooks[0].name(), "hook3");
    }

    #[tokio::test]
    async fn test_execute_allows() {
        let mut manager = HookManager::new();
        manager.register(TestHook::new("hook1", vec![HookEvent::PreToolUse], 0));
        manager.register(TestHook::new("hook2", vec![HookEvent::PreToolUse], 0));

        let input = HookInput::pre_tool_use("session-1", "Read", serde_json::json!({}));
        let ctx = HookContext::new("session-1");
        let output = manager
            .execute(HookEvent::PreToolUse, input, &ctx)
            .await
            .unwrap();

        assert!(output.continue_execution);
    }

    #[tokio::test]
    async fn test_execute_blocks() {
        let mut manager = HookManager::new();
        manager.register(TestHook::new("hook1", vec![HookEvent::PreToolUse], 0));
        manager.register(TestHook::blocking(
            "hook2",
            vec![HookEvent::PreToolUse],
            10, // Higher priority, runs first
        ));

        let input = HookInput::pre_tool_use("session-1", "Read", serde_json::json!({}));
        let ctx = HookContext::new("session-1");
        let output = manager
            .execute(HookEvent::PreToolUse, input, &ctx)
            .await
            .unwrap();

        assert!(!output.continue_execution);
        assert_eq!(output.stop_reason, Some("Blocked by hook2".to_string()));
    }

    #[tokio::test]
    async fn test_no_hooks_allows() {
        let manager = HookManager::new();

        let input = HookInput::pre_tool_use("session-1", "Read", serde_json::json!({}));
        let ctx = HookContext::new("session-1");
        let output = manager
            .execute(HookEvent::PreToolUse, input, &ctx)
            .await
            .unwrap();

        assert!(output.continue_execution);
    }
}
