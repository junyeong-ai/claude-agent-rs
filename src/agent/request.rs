//! Request building utilities for agent execution.

use std::path::PathBuf;
use std::sync::Arc;

use crate::agent::config::{AgentConfig, CacheConfig, ServerToolsConfig, SystemPromptMode};
use crate::client::messages::CreateMessageRequest;
use crate::output_style::{OutputStyle, SystemPromptGenerator};
use crate::tools::ToolRegistry;
use crate::tools::search::{PreparedTools, SearchMode};
use crate::types::{Message, SystemBlock, SystemPrompt, ToolSearchTool};

pub struct RequestBuilder {
    model: String,
    max_tokens: u32,
    tools: Arc<ToolRegistry>,
    server_tools: ServerToolsConfig,
    tool_access: crate::tools::ToolAccess,
    system_prompt_mode: SystemPromptMode,
    custom_system_prompt: Option<String>,
    base_system_prompt: String,
    cache_config: CacheConfig,
    prepared_mcp_tools: Option<PreparedTools>,
}

impl RequestBuilder {
    pub fn new(config: &AgentConfig, tools: Arc<ToolRegistry>) -> Self {
        let base_system_prompt = Self::generate_base_prompt(
            &config.model.primary,
            config.working_dir.as_ref(),
            config.prompt.output_style.as_ref(),
        );

        Self {
            model: config.model.primary.clone(),
            max_tokens: config.model.max_tokens,
            tools,
            server_tools: config.server_tools.clone(),
            tool_access: config.security.tool_access.clone(),
            system_prompt_mode: config.prompt.system_prompt_mode,
            custom_system_prompt: config.prompt.system_prompt.clone(),
            base_system_prompt,
            cache_config: config.cache.clone(),
            prepared_mcp_tools: None,
        }
    }

    pub fn with_prepared_tools(mut self, prepared: PreparedTools) -> Self {
        self.prepared_mcp_tools = Some(prepared);
        self
    }

    pub fn build(&self, messages: Vec<Message>, dynamic_rules: &str) -> CreateMessageRequest {
        let system_prompt = self.build_system_prompt_blocks(dynamic_rules);

        let mut request = CreateMessageRequest::new(&self.model, messages)
            .with_max_tokens(self.max_tokens)
            .with_system(system_prompt);

        // Build tool definitions with optional Progressive Disclosure
        request = match &self.prepared_mcp_tools {
            Some(prepared) => {
                // Progressive Disclosure mode: separate built-in and MCP tools
                let registry_tools = self.tools.definitions();
                let builtin_tools: Vec<_> = registry_tools
                    .into_iter()
                    .filter(|t| !crate::mcp::is_mcp_name(&t.name))
                    .collect();

                // Pre-allocate capacity to avoid reallocations
                let capacity =
                    builtin_tools.len() + prepared.immediate.len() + prepared.deferred.len();
                let mut tools = Vec::with_capacity(capacity);

                // 1. Built-in tools (non-MCP)
                tools.extend(builtin_tools);

                // 2. Immediate MCP tools (full schema, no defer_loading)
                tools.extend(prepared.immediate.iter().cloned());

                // 3. Deferred MCP tools (full schema, defer_loading: true)
                tools.extend(prepared.deferred.iter().cloned());

                if !tools.is_empty() {
                    request = request.with_tools(tools);
                }

                // Add ToolSearchTool when threshold exceeded
                if prepared.use_search {
                    let tool_search = match prepared.search_mode {
                        SearchMode::Regex => ToolSearchTool::regex(),
                        SearchMode::Bm25 => ToolSearchTool::bm25(),
                    };
                    request = request.with_tool_search(tool_search);
                }

                request
            }
            None => {
                // Standard mode: all tools from registry
                let tool_defs = self.tools.definitions();
                if !tool_defs.is_empty() {
                    request.with_tools(tool_defs)
                } else {
                    request
                }
            }
        };

        if self.tool_access.is_allowed("WebSearch") {
            let web_search = self.server_tools.web_search.clone().unwrap_or_default();
            request = request.with_web_search(web_search);
        }

        if self.tool_access.is_allowed("WebFetch") {
            let web_fetch = self.server_tools.web_fetch.clone().unwrap_or_default();
            request = request.with_web_fetch(web_fetch);
        }

        request
    }

    fn build_system_prompt_blocks(&self, dynamic_rules: &str) -> SystemPrompt {
        let mut blocks = Vec::new();

        let static_prompt = match self.system_prompt_mode {
            SystemPromptMode::Replace => self
                .custom_system_prompt
                .clone()
                .unwrap_or_else(|| self.base_system_prompt.clone()),
            SystemPromptMode::Append => {
                let mut base = self.base_system_prompt.clone();
                if let Some(custom) = &self.custom_system_prompt {
                    base.push_str("\n\n");
                    base.push_str(custom);
                }
                base
            }
        };

        // Per Anthropic best practices: static content with longer TTL first
        if !static_prompt.is_empty() {
            blocks.push(if self.cache_config.strategy.cache_system() {
                SystemBlock::cached_with_ttl(&static_prompt, self.cache_config.static_ttl)
            } else {
                SystemBlock::uncached(&static_prompt)
            });
        }

        // Dynamic rules are never cached (they change frequently)
        if !dynamic_rules.is_empty() {
            blocks.push(SystemBlock::uncached(dynamic_rules));
        }

        if blocks.is_empty() {
            SystemPrompt::Text(String::new())
        } else {
            SystemPrompt::Blocks(blocks)
        }
    }

    fn generate_base_prompt(
        model: &str,
        working_dir: Option<&PathBuf>,
        output_style: Option<&OutputStyle>,
    ) -> String {
        let mut generator = SystemPromptGenerator::new().with_model(model);

        if let Some(dir) = working_dir {
            generator = generator.with_working_dir(dir);
        }

        if let Some(style) = output_style {
            generator = generator.with_style(style.clone());
        }

        generator.generate()
    }
}
