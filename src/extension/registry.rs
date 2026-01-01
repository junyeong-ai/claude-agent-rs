//! Extension registry with dependency resolution.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::{Extension, ExtensionContext, ExtensionRef};
use crate::{Error, Result};

/// Registry for managing extensions with dependency resolution.
#[derive(Default)]
pub struct ExtensionRegistry {
    extensions: Vec<ExtensionRef>,
    initialized: HashSet<String>,
}

impl ExtensionRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an extension to the registry.
    ///
    /// If the extension is unique and already registered, it's silently ignored.
    pub fn add<E: Extension + 'static>(&mut self, ext: E) -> &mut Self {
        let name = ext.meta().name.to_string();
        if ext.is_unique() && self.contains(&name) {
            return self;
        }
        self.extensions.push(ExtensionRef(Arc::new(ext)));
        self
    }

    /// Adds a pre-wrapped extension reference.
    pub fn add_ref(&mut self, ext: ExtensionRef) -> &mut Self {
        let name = ext.0.meta().name.to_string();
        if ext.0.is_unique() && self.contains(&name) {
            return self;
        }
        self.extensions.push(ext);
        self
    }

    /// Checks if an extension with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.extensions.iter().any(|e| e.0.meta().name == name)
    }

    /// Returns the number of registered extensions.
    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    /// Returns true if no extensions are registered.
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }

    /// Returns extension names in registration order.
    pub fn names(&self) -> Vec<&str> {
        self.extensions.iter().map(|e| e.0.meta().name).collect()
    }

    /// Builds all extensions in dependency order.
    ///
    /// This method:
    /// 1. Resolves dependencies using topological sort
    /// 2. Calls `build()` on each extension in order
    /// 3. Verifies all extensions are ready via `ready()`
    /// 4. Calls `finish()` on all extensions
    pub fn build_all(&mut self, ctx: &mut ExtensionContext) -> Result<()> {
        if self.extensions.is_empty() {
            return Ok(());
        }

        let order = self.resolve_dependencies()?;

        // Build in dependency order
        for name in &order {
            if !self.initialized.contains(name) {
                let ext = self
                    .extensions
                    .iter()
                    .find(|e| e.0.meta().name == name)
                    .unwrap();
                ext.0.build(ctx);
                self.initialized.insert(name.clone());
            }
        }

        // Verify all ready
        for ext in &self.extensions {
            if !ext.0.ready(ctx) {
                return Err(Error::Config(format!(
                    "Extension '{}' is not ready after build",
                    ext.0.meta().name
                )));
            }
        }

        // Finish all
        for ext in &self.extensions {
            ext.0.finish(ctx);
        }

        Ok(())
    }

    /// Cleans up all extensions in reverse order.
    pub fn cleanup_all(&self) {
        for ext in self.extensions.iter().rev() {
            ext.0.cleanup();
        }
    }

    /// Resolves extension dependencies using Kahn's algorithm (topological sort).
    fn resolve_dependencies(&self) -> Result<Vec<String>> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Build adjacency list and in-degree map
        for ext in &self.extensions {
            let meta = ext.0.meta();
            let name = meta.name.to_string();

            graph.entry(name.clone()).or_default();
            in_degree.entry(name.clone()).or_insert(0);

            for dep in meta.dependencies {
                let dep_name = dep.to_string();

                // Check dependency exists
                if !self.contains(dep) {
                    return Err(Error::Config(format!(
                        "Extension '{}' depends on '{}' which is not registered",
                        meta.name, dep
                    )));
                }

                graph
                    .entry(dep_name.clone())
                    .or_default()
                    .push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, d)| *d == 0)
            .map(|(n, _)| n.clone())
            .collect();

        let mut result = Vec::with_capacity(self.extensions.len());

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != self.extensions.len() {
            let remaining: Vec<_> = in_degree
                .iter()
                .filter(|&(_, d)| *d > 0)
                .map(|(n, _)| n.as_str())
                .collect();
            return Err(Error::Config(format!(
                "Circular dependency detected among extensions: {:?}",
                remaining
            )));
        }

        Ok(result)
    }
}

impl std::fmt::Debug for ExtensionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionRegistry")
            .field("extensions", &self.names())
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl Clone for ExtensionRegistry {
    fn clone(&self) -> Self {
        Self {
            extensions: self.extensions.clone(),
            initialized: self.initialized.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentOptions;
    use crate::context::{ChainMemoryProvider, ContextBuilder};
    use crate::hooks::HookManager;
    use crate::skills::SkillRegistry;
    use crate::tools::ToolRegistry;

    struct TestExt {
        name: &'static str,
        deps: &'static [&'static str],
    }

    impl Extension for TestExt {
        fn meta(&self) -> super::super::ExtensionMeta {
            super::super::ExtensionMeta::new(self.name).dependencies(self.deps)
        }
        fn build(&self, _ctx: &mut ExtensionContext) {}
    }

    fn make_context() -> (
        ToolRegistry,
        HookManager,
        SkillRegistry,
        ChainMemoryProvider,
        ContextBuilder,
        AgentOptions,
    ) {
        (
            ToolRegistry::new(),
            HookManager::new(),
            SkillRegistry::new(),
            ChainMemoryProvider::new(),
            ContextBuilder::new(),
            AgentOptions::default(),
        )
    }

    #[test]
    fn test_empty_registry() {
        let registry = ExtensionRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_add_extension() {
        let mut registry = ExtensionRegistry::new();
        registry.add(TestExt {
            name: "test",
            deps: &[],
        });
        assert!(registry.contains("test"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_unique_extension() {
        let mut registry = ExtensionRegistry::new();
        registry.add(TestExt {
            name: "test",
            deps: &[],
        });
        registry.add(TestExt {
            name: "test",
            deps: &[],
        });
        assert_eq!(registry.len(), 1); // Duplicate ignored
    }

    #[test]
    fn test_dependency_resolution() {
        let mut registry = ExtensionRegistry::new();
        registry.add(TestExt {
            name: "c",
            deps: &["b"],
        });
        registry.add(TestExt {
            name: "b",
            deps: &["a"],
        });
        registry.add(TestExt {
            name: "a",
            deps: &[],
        });

        let order = registry.resolve_dependencies().unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_missing_dependency() {
        let mut registry = ExtensionRegistry::new();
        registry.add(TestExt {
            name: "a",
            deps: &["missing"],
        });

        let result = registry.resolve_dependencies();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing"));
    }

    #[test]
    fn test_circular_dependency() {
        let mut registry = ExtensionRegistry::new();
        registry.add(TestExt {
            name: "a",
            deps: &["b"],
        });
        registry.add(TestExt {
            name: "b",
            deps: &["a"],
        });

        let result = registry.resolve_dependencies();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular"));
    }

    #[test]
    fn test_build_all() {
        let mut registry = ExtensionRegistry::new();
        registry.add(TestExt {
            name: "a",
            deps: &[],
        });
        registry.add(TestExt {
            name: "b",
            deps: &["a"],
        });

        let (mut tools, mut hooks, mut skills, mut memory, mut context, options) = make_context();
        let mut ctx = ExtensionContext {
            tools: &mut tools,
            hooks: &mut hooks,
            skills: &mut skills,
            memory: &mut memory,
            context: &mut context,
            options: &options,
        };

        let result = registry.build_all(&mut ctx);
        assert!(result.is_ok());
    }
}
