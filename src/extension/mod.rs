//! Extension system for composable agent components.
//!
//! Extensions bundle related tools, hooks, skills, and providers together,
//! following the Bevy Plugin pattern for lifecycle management.

mod registry;

pub use registry::ExtensionRegistry;

use std::sync::Arc;

use crate::agent::AgentOptions;
use crate::context::{ChainMemoryProvider, ContextBuilder};
use crate::hooks::HookManager;
use crate::skills::SkillRegistry;
use crate::tools::ToolRegistry;

/// Extension metadata.
#[derive(Debug, Clone)]
pub struct ExtensionMeta {
    /// Unique name of the extension.
    pub name: &'static str,
    /// Version string (semver).
    pub version: &'static str,
    /// Short description.
    pub description: &'static str,
    /// Names of extensions this one depends on.
    pub dependencies: &'static [&'static str],
}

impl ExtensionMeta {
    /// Create new extension metadata.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            version: "0.0.0",
            description: "",
            dependencies: &[],
        }
    }

    /// Set version.
    pub const fn version(mut self, version: &'static str) -> Self {
        self.version = version;
        self
    }

    /// Set description.
    pub const fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    /// Set dependencies.
    pub const fn dependencies(mut self, deps: &'static [&'static str]) -> Self {
        self.dependencies = deps;
        self
    }
}

/// Context provided to extensions during build phase.
pub struct ExtensionContext<'a> {
    /// Tool registry for registering custom tools.
    pub tools: &'a mut ToolRegistry,
    /// Hook manager for registering lifecycle hooks.
    pub hooks: &'a mut HookManager,
    /// Skill registry for registering skills.
    pub skills: &'a mut SkillRegistry,
    /// Memory provider chain for context loading.
    pub memory: &'a mut ChainMemoryProvider,
    /// Context builder for static context configuration.
    pub context: &'a mut ContextBuilder,
    /// Agent options (read-only).
    pub options: &'a AgentOptions,
}

/// Extension trait for bundling agent components.
///
/// Extensions follow the Bevy Plugin pattern with lifecycle methods:
/// - `build`: Configure components (tools, hooks, skills, etc.)
/// - `ready`: Check if all dependencies are satisfied
/// - `finish`: Post-build finalization
/// - `cleanup`: Cleanup on agent drop
///
/// # Example
///
/// ```rust,ignore
/// use claude_agent::extension::{Extension, ExtensionContext, ExtensionMeta};
///
/// pub struct MyExtension;
///
/// impl Extension for MyExtension {
///     fn meta(&self) -> ExtensionMeta {
///         ExtensionMeta::new("my-extension")
///             .version("1.0.0")
///             .description("My custom extension")
///     }
///
///     fn build(&self, ctx: &mut ExtensionContext) {
///         ctx.tools.register(Arc::new(MyTool));
///         ctx.skills.register(my_skill());
///     }
/// }
/// ```
pub trait Extension: Send + Sync {
    /// Returns extension metadata.
    fn meta(&self) -> ExtensionMeta;

    /// Configures the agent with this extension's components.
    fn build(&self, ctx: &mut ExtensionContext);

    /// Returns true when the extension is ready to use.
    fn ready(&self, _ctx: &ExtensionContext) -> bool {
        true
    }

    /// Called after all extensions have been built.
    fn finish(&self, _ctx: &mut ExtensionContext) {}

    /// Called when the agent is dropped.
    fn cleanup(&self) {}

    /// Returns true if only one instance of this extension should exist.
    fn is_unique(&self) -> bool {
        true
    }
}

/// Wrapper for Arc<dyn Extension> to enable cloning.
#[derive(Clone)]
pub struct ExtensionRef(pub Arc<dyn Extension>);

impl std::fmt::Debug for ExtensionRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ExtensionRef")
            .field(&self.0.meta().name)
            .finish()
    }
}

impl<E: Extension + 'static> From<E> for ExtensionRef {
    fn from(ext: E) -> Self {
        Self(Arc::new(ext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestExtension {
        name: &'static str,
    }

    impl Extension for TestExtension {
        fn meta(&self) -> ExtensionMeta {
            ExtensionMeta::new(self.name)
                .version("1.0.0")
                .description("Test extension")
        }

        fn build(&self, _ctx: &mut ExtensionContext) {}
    }

    #[test]
    fn test_extension_meta() {
        let meta = ExtensionMeta::new("test")
            .version("1.0.0")
            .description("A test")
            .dependencies(&["dep1", "dep2"]);

        assert_eq!(meta.name, "test");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.dependencies.len(), 2);
    }

    #[test]
    fn test_extension_ref() {
        let ext = TestExtension { name: "test" };
        let ext_ref = ExtensionRef::from(ext);
        assert_eq!(ext_ref.0.meta().name, "test");
    }
}
