//! MCP Resource management
//!
//! This module provides utilities for working with MCP resources.

use super::{McpContent, McpError, McpManager, McpResourceDefinition, McpResult};

/// Resource subscription handle
pub struct ResourceSubscription {
    /// Server name
    pub server_name: String,
    /// Resource URI
    pub uri: String,
    /// Whether the subscription is active
    pub active: bool,
}

/// Resource manager for handling MCP resource operations
pub struct ResourceManager {
    /// Reference to the MCP manager
    manager: std::sync::Arc<McpManager>,
    /// Active subscriptions
    subscriptions: Vec<ResourceSubscription>,
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new(manager: std::sync::Arc<McpManager>) -> Self {
        Self {
            manager,
            subscriptions: Vec::new(),
        }
    }

    /// List all available resources from all connected servers
    pub async fn list_all(&self) -> Vec<(String, McpResourceDefinition)> {
        self.manager.list_resources().await
    }

    /// List resources from a specific server
    #[cfg(feature = "mcp")]
    pub async fn list_from_server(
        &self,
        server_name: &str,
    ) -> McpResult<Vec<McpResourceDefinition>> {
        let all_resources = self.manager.list_resources().await;
        let resources: Vec<_> = all_resources
            .into_iter()
            .filter(|(name, _)| name == server_name)
            .map(|(_, r)| r)
            .collect();

        if resources.is_empty() {
            // Check if server exists
            let servers = self.manager.list_servers().await;
            if !servers.contains(&server_name.to_string()) {
                return Err(McpError::ServerNotFound {
                    name: server_name.to_string(),
                });
            }
        }

        Ok(resources)
    }

    /// List resources from a specific server (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn list_from_server(
        &self,
        _server_name: &str,
    ) -> McpResult<Vec<McpResourceDefinition>> {
        Ok(Vec::new())
    }

    /// Read a resource
    pub async fn read(&self, server_name: &str, uri: &str) -> McpResult<Vec<McpContent>> {
        self.manager.read_resource(server_name, uri).await
    }

    /// Read a resource as text
    pub async fn read_text(&self, server_name: &str, uri: &str) -> McpResult<String> {
        let contents = self.read(server_name, uri).await?;

        let text: Vec<String> = contents
            .iter()
            .filter_map(|c| c.as_text().map(|s| s.to_string()))
            .collect();

        if text.is_empty() {
            Err(McpError::ResourceNotFound {
                uri: format!("{} (no text content)", uri),
            })
        } else {
            Ok(text.join("\n"))
        }
    }

    /// Subscribe to resource changes (placeholder for future implementation)
    ///
    /// Returns the index of the newly created subscription, which can be used
    /// with `get_subscription()` to retrieve it.
    pub fn subscribe(&mut self, server_name: &str, uri: &str) -> usize {
        let subscription = ResourceSubscription {
            server_name: server_name.to_string(),
            uri: uri.to_string(),
            active: true,
        };
        self.subscriptions.push(subscription);
        self.subscriptions.len() - 1
    }

    /// Get a subscription by index
    pub fn get_subscription(&self, index: usize) -> Option<&ResourceSubscription> {
        self.subscriptions.get(index)
    }

    /// Unsubscribe from resource changes
    pub fn unsubscribe(&mut self, server_name: &str, uri: &str) {
        self.subscriptions
            .retain(|s| !(s.server_name == server_name && s.uri == uri));
    }

    /// Get active subscriptions
    pub fn subscriptions(&self) -> &[ResourceSubscription] {
        &self.subscriptions
    }

    /// Find resources by URI pattern
    pub async fn find_by_pattern(&self, pattern: &str) -> Vec<(String, McpResourceDefinition)> {
        let all_resources = self.manager.list_resources().await;

        // Simple glob-like pattern matching
        let is_prefix_match = pattern.ends_with('*');
        let pattern = pattern.trim_end_matches('*');

        all_resources
            .into_iter()
            .filter(|(_, r)| {
                if is_prefix_match {
                    r.uri.starts_with(pattern)
                } else {
                    r.uri == pattern
                }
            })
            .collect()
    }
}

/// Builder for resource queries
pub struct ResourceQuery {
    /// Server filter
    server: Option<String>,
    /// URI pattern
    pattern: Option<String>,
    /// MIME type filter
    mime_type: Option<String>,
}

impl ResourceQuery {
    /// Create a new resource query
    pub fn new() -> Self {
        Self {
            server: None,
            pattern: None,
            mime_type: None,
        }
    }

    /// Filter by server name
    pub fn server(mut self, name: impl Into<String>) -> Self {
        self.server = Some(name.into());
        self
    }

    /// Filter by URI pattern
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    /// Filter by MIME type
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Execute the query against a resource manager
    pub async fn execute(&self, manager: &ResourceManager) -> Vec<(String, McpResourceDefinition)> {
        let mut results = manager.list_all().await;

        // Filter by server
        if let Some(ref server) = self.server {
            results.retain(|(s, _)| s == server);
        }

        // Filter by pattern
        if let Some(ref pattern) = self.pattern {
            let is_prefix = pattern.ends_with('*');
            let pattern = pattern.trim_end_matches('*');
            results.retain(|(_, r)| {
                if is_prefix {
                    r.uri.starts_with(pattern)
                } else {
                    r.uri == pattern
                }
            });
        }

        // Filter by MIME type
        if let Some(ref mime_type) = self.mime_type {
            results.retain(|(_, r)| {
                r.mime_type
                    .as_ref()
                    .map(|m| m == mime_type)
                    .unwrap_or(false)
            });
        }

        results
    }
}

impl Default for ResourceQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_query_builder() {
        let query = ResourceQuery::new()
            .server("filesystem")
            .pattern("file://*")
            .mime_type("text/plain");

        assert_eq!(query.server, Some("filesystem".to_string()));
        assert_eq!(query.pattern, Some("file://*".to_string()));
        assert_eq!(query.mime_type, Some("text/plain".to_string()));
    }
}
