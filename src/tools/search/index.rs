//! Tool index for efficient searching.

use crate::mcp::McpToolDefinition;

#[derive(Debug, Clone)]
pub struct ToolIndexEntry {
    pub qualified_name: String,
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub arg_names: Vec<String>,
    pub arg_descriptions: Vec<String>,
    pub estimated_tokens: usize,
}

impl ToolIndexEntry {
    pub fn from_mcp_tool(server: &str, tool: &McpToolDefinition) -> Self {
        let (arg_names, arg_descriptions) = Self::extract_arg_info(&tool.input_schema);
        let estimated_tokens = Self::estimate_tokens(tool);

        Self {
            qualified_name: crate::mcp::make_mcp_name(server, &tool.name),
            server_name: server.to_string(),
            tool_name: tool.name.clone(),
            description: tool.description.clone(),
            arg_names,
            arg_descriptions,
            estimated_tokens,
        }
    }

    fn extract_arg_info(schema: &serde_json::Value) -> (Vec<String>, Vec<String>) {
        let mut names = Vec::new();
        let mut descs = Vec::new();

        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (name, prop) in props {
                names.push(name.clone());
                if let Some(desc) = prop.get("description").and_then(|d| d.as_str()) {
                    descs.push(desc.to_string());
                }
            }
        }

        (names, descs)
    }

    fn estimate_tokens(tool: &McpToolDefinition) -> usize {
        let name_tokens = tool.name.len() / 4;
        let desc_tokens = tool.description.len() / 4;
        let schema_tokens = tool.input_schema.to_string().len() / 4;
        name_tokens + desc_tokens + schema_tokens + 20
    }

    pub fn searchable_text(&self) -> String {
        format!(
            "{} {} {} {}",
            self.tool_name,
            self.description,
            self.arg_names.join(" "),
            self.arg_descriptions.join(" ")
        )
    }
}

#[derive(Debug, Default)]
pub struct ToolIndex {
    entries: Vec<ToolIndexEntry>,
    total_tokens: usize,
}

impl ToolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, entry: ToolIndexEntry) {
        self.total_tokens += entry.estimated_tokens;
        self.entries.push(entry);
    }

    pub fn total_tokens(&self) -> usize {
        self.total_tokens
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[ToolIndexEntry] {
        &self.entries
    }

    pub fn get(&self, qualified_name: &str) -> Option<&ToolIndexEntry> {
        self.entries
            .iter()
            .find(|e| e.qualified_name == qualified_name)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_tokens = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_tool(name: &str, desc: &str) -> McpToolDefinition {
        McpToolDefinition {
            name: name.to_string(),
            description: desc.to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg1": { "type": "string", "description": "First argument" }
                }
            }),
        }
    }

    #[test]
    fn test_index_entry_creation() {
        let tool = make_test_tool("read_file", "Read a file from disk");
        let entry = ToolIndexEntry::from_mcp_tool("filesystem", &tool);

        assert_eq!(entry.qualified_name, "mcp__filesystem_read_file");
        assert_eq!(entry.server_name, "filesystem");
        assert_eq!(entry.tool_name, "read_file");
        assert!(entry.estimated_tokens > 0);
    }

    #[test]
    fn test_searchable_text() {
        let tool = make_test_tool("get_weather", "Get weather for location");
        let entry = ToolIndexEntry::from_mcp_tool("weather", &tool);
        let text = entry.searchable_text();

        assert!(text.contains("get_weather"));
        assert!(text.contains("weather"));
        assert!(text.contains("location"));
    }

    #[test]
    fn test_index_operations() {
        let mut index = ToolIndex::new();
        assert!(index.is_empty());

        let tool = make_test_tool("test", "Test tool");
        let entry = ToolIndexEntry::from_mcp_tool("server", &tool);
        let tokens = entry.estimated_tokens;

        index.add(entry);
        assert_eq!(index.len(), 1);
        assert_eq!(index.total_tokens(), tokens);
        assert!(index.get("mcp__server_test").is_some());
    }
}
