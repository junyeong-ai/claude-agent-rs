//! MCP Manager for multiple server connections
//!
//! This module provides the `McpManager` type for managing multiple MCP server
//! connections and exposing their tools through the unified tool registry.

#[cfg(feature = "mcp")]
use std::collections::HashMap;
#[cfg(feature = "mcp")]
use std::sync::Arc;
#[cfg(feature = "mcp")]
use tokio::sync::RwLock;

use super::{
    McpContent, McpError, McpResourceDefinition, McpResult, McpServerConfig, McpServerState,
    McpToolDefinition, McpToolResult,
};

#[cfg(feature = "mcp")]
use super::client::McpClient;

/// MCP Manager for handling multiple server connections
pub struct McpManager {
    /// Connected servers
    #[cfg(feature = "mcp")]
    servers: Arc<RwLock<HashMap<String, McpClient>>>,
    #[cfg(not(feature = "mcp"))]
    _phantom: std::marker::PhantomData<()>,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpManager {
    /// Create a new MCP manager
    #[cfg(feature = "mcp")]
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new MCP manager (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add and connect to an MCP server
    #[cfg(feature = "mcp")]
    pub async fn add_server(&self, name: impl Into<String>, config: McpServerConfig) -> McpResult<()> {
        let name = name.into();

        // Check if server already exists
        {
            let servers = self.servers.read().await;
            if servers.contains_key(&name) {
                return Err(McpError::Protocol {
                    message: format!("Server '{}' already exists", name),
                });
            }
        }

        // Create and connect client
        let mut client = McpClient::new(name.clone(), config);
        client.connect().await?;

        // Store client
        let mut servers = self.servers.write().await;
        servers.insert(name, client);

        Ok(())
    }

    /// Add and connect to an MCP server (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn add_server(&self, _name: impl Into<String>, _config: McpServerConfig) -> McpResult<()> {
        Err(McpError::Protocol {
            message: "MCP feature not enabled".to_string(),
        })
    }

    /// Remove and disconnect from an MCP server
    #[cfg(feature = "mcp")]
    pub async fn remove_server(&self, name: &str) -> McpResult<()> {
        let mut servers = self.servers.write().await;
        if let Some(mut client) = servers.remove(name) {
            client.close().await?;
            Ok(())
        } else {
            Err(McpError::ServerNotFound {
                name: name.to_string(),
            })
        }
    }

    /// Remove and disconnect from an MCP server (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn remove_server(&self, _name: &str) -> McpResult<()> {
        Err(McpError::Protocol {
            message: "MCP feature not enabled".to_string(),
        })
    }

    /// List all connected servers
    #[cfg(feature = "mcp")]
    pub async fn list_servers(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        servers.keys().cloned().collect()
    }

    /// List all connected servers (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn list_servers(&self) -> Vec<String> {
        Vec::new()
    }

    /// Get server state
    #[cfg(feature = "mcp")]
    pub async fn get_server_state(&self, name: &str) -> Option<McpServerState> {
        let servers = self.servers.read().await;
        servers.get(name).map(|c| c.state().clone())
    }

    /// Get server state (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn get_server_state(&self, _name: &str) -> Option<McpServerState> {
        None
    }

    /// List all available tools from all servers
    ///
    /// Tools are returned with the naming convention `mcp__{server}__{tool}`
    #[cfg(feature = "mcp")]
    pub async fn list_tools(&self) -> Vec<(String, McpToolDefinition)> {
        let servers = self.servers.read().await;
        let mut tools = Vec::new();

        for (server_name, client) in servers.iter() {
            for tool in client.tools() {
                let qualified_name = format!("mcp__{}_{}", server_name, tool.name);
                tools.push((qualified_name, tool.clone()));
            }
        }

        tools
    }

    /// List all available tools from all servers (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn list_tools(&self) -> Vec<(String, McpToolDefinition)> {
        Vec::new()
    }

    /// Call a tool by its qualified name (`mcp__{server}__{tool}`)
    #[cfg(feature = "mcp")]
    pub async fn call_tool(&self, qualified_name: &str, arguments: serde_json::Value) -> McpResult<McpToolResult> {
        let (server_name, tool_name) = parse_qualified_name(qualified_name)?;

        let servers = self.servers.read().await;
        let client = servers.get(&server_name).ok_or_else(|| McpError::ServerNotFound {
            name: server_name.clone(),
        })?;

        client.call_tool(&tool_name, arguments).await
    }

    /// Call a tool by its qualified name (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn call_tool(&self, _qualified_name: &str, _arguments: serde_json::Value) -> McpResult<McpToolResult> {
        Err(McpError::Protocol {
            message: "MCP feature not enabled".to_string(),
        })
    }

    /// List all available resources from all servers
    #[cfg(feature = "mcp")]
    pub async fn list_resources(&self) -> Vec<(String, McpResourceDefinition)> {
        let servers = self.servers.read().await;
        let mut resources = Vec::new();

        for (server_name, client) in servers.iter() {
            for resource in client.resources() {
                resources.push((server_name.clone(), resource.clone()));
            }
        }

        resources
    }

    /// List all available resources from all servers (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn list_resources(&self) -> Vec<(String, McpResourceDefinition)> {
        Vec::new()
    }

    /// Read a resource from a specific server
    #[cfg(feature = "mcp")]
    pub async fn read_resource(&self, server_name: &str, uri: &str) -> McpResult<Vec<McpContent>> {
        let servers = self.servers.read().await;
        let client = servers.get(server_name).ok_or_else(|| McpError::ServerNotFound {
            name: server_name.to_string(),
        })?;

        client.read_resource(uri).await
    }

    /// Read a resource from a specific server (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn read_resource(&self, _server_name: &str, _uri: &str) -> McpResult<Vec<McpContent>> {
        Err(McpError::Protocol {
            message: "MCP feature not enabled".to_string(),
        })
    }

    /// Close all server connections
    #[cfg(feature = "mcp")]
    pub async fn close_all(&self) -> McpResult<()> {
        let mut servers = self.servers.write().await;
        for (_, mut client) in servers.drain() {
            let _ = client.close().await;
        }
        Ok(())
    }

    /// Close all server connections (stub when feature disabled)
    #[cfg(not(feature = "mcp"))]
    pub async fn close_all(&self) -> McpResult<()> {
        Ok(())
    }

    /// Check if a tool name matches the MCP naming pattern
    pub fn is_mcp_tool(name: &str) -> bool {
        name.starts_with("mcp__")
    }
}

/// Parse a qualified tool name into (server_name, tool_name)
#[cfg(feature = "mcp")]
fn parse_qualified_name(qualified_name: &str) -> McpResult<(String, String)> {
    if !qualified_name.starts_with("mcp__") {
        return Err(McpError::ToolNotFound {
            name: qualified_name.to_string(),
        });
    }

    let rest = &qualified_name[5..]; // Skip "mcp__"
    let parts: Vec<&str> = rest.splitn(2, '_').collect();

    if parts.len() != 2 {
        return Err(McpError::ToolNotFound {
            name: qualified_name.to_string(),
        });
    }

    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "mcp")]
    #[test]
    fn test_parse_qualified_name() {
        let (server, tool) = parse_qualified_name("mcp__filesystem_read_file").unwrap();
        assert_eq!(server, "filesystem");
        assert_eq!(tool, "read_file");
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn test_parse_qualified_name_invalid() {
        assert!(parse_qualified_name("read_file").is_err());
        assert!(parse_qualified_name("mcp_filesystem").is_err());
    }

    #[test]
    fn test_is_mcp_tool() {
        assert!(McpManager::is_mcp_tool("mcp__server_tool"));
        assert!(!McpManager::is_mcp_tool("Read"));
        assert!(!McpManager::is_mcp_tool("Bash"));
    }

    #[tokio::test]
    async fn test_manager_new() {
        let manager = McpManager::new();
        let servers = manager.list_servers().await;
        assert!(servers.is_empty());
    }
}
