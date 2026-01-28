# Plugin System

Plugins bundle skills, subagents, hooks, and MCP servers into self-contained, namespace-isolated packages.

> **Feature flag**: Requires the `plugins` feature (`cli-integration` + `dirs`).

## Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Plugin Architecture                    │
├─────────────────────────────────────────────────────────┤
│                                                          │
│   ┌────────────┐     ┌────────────┐                      │
│   │PluginDisc. │────▶│PluginLoader│                      │
│   │ (discover) │     │  (load)    │                      │
│   └────────────┘     └─────┬──────┘                      │
│                            │                             │
│                   ┌────────┼────────┐                    │
│                   ▼        ▼        ▼                    │
│              Skills   Subagents   Hooks   MCP            │
│              (namespaced: plugin-name:resource-name)      │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Directory Structure

```
~/.claude/plugins/
└── my-plugin/
    ├── .claude-plugin/
    │   └── plugin.json          # Manifest (required)
    ├── skills/
    │   └── commit/
    │       └── SKILL.md         # Skill definition
    ├── commands/
    │   └── hello.md             # Legacy skill (lower priority than skills/)
    ├── agents/
    │   └── reviewer.md          # Subagent definition
    ├── hooks/
    │   └── hooks.json           # Hook configurations
    └── .mcp.json                # MCP server configurations
```

## Manifest (`plugin.json`)

```json
{
  "name": "my-plugin",
  "description": "Plugin description",
  "version": "1.0.0",
  "author": {
    "name": "Alice",
    "email": "alice@example.com",
    "url": "https://example.com"
  },
  "homepage": "https://github.com/user/plugin",
  "repository": "https://github.com/user/plugin",
  "license": "MIT",
  "keywords": ["tooling", "automation"]
}
```

Required fields: `name`, `description`, `version`. All others are optional.

## Namespace System

All resources are namespaced as `plugin-name:resource-name` to avoid collisions:

```
my-plugin:commit       (skill)
my-plugin:reviewer     (subagent)
my-plugin:context7     (MCP server)
```

## Plugin Root Variable

Commands in hooks and MCP configs can reference `${CLAUDE_PLUGIN_ROOT}`, which resolves to the plugin's root directory at load time:

```json
{
  "PreToolUse": ["${CLAUDE_PLUGIN_ROOT}/scripts/check.sh"]
}
```

## Hooks Configuration

Plugins support two hooks.json formats:

### Official (Nested) Format

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          { "type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/fmt.sh" }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          { "type": "command", "command": "check.sh", "timeout": 10 }
        ]
      }
    ]
  }
}
```

### Legacy (Flat) Format

```json
{
  "PreToolUse": ["echo pre"],
  "SessionStart": ["echo start"]
}
```

## MCP Configuration (`.mcp.json`)

```json
{
  "mcpServers": {
    "context7": {
      "type": "stdio",
      "command": "${CLAUDE_PLUGIN_ROOT}/bin/server",
      "args": ["--config", "${CLAUDE_PLUGIN_ROOT}/config.json"],
      "env": { "DB_PATH": "${CLAUDE_PLUGIN_ROOT}/data" }
    }
  }
}
```

## Resource Precedence

When both `skills/` and `commands/` contain a definition with the same name, `skills/` takes precedence.

## Core Types

### PluginManager

```rust
use claude_agent::plugins::PluginManager;
use std::path::PathBuf;

let manager = PluginManager::load_from_dirs(&[
    PathBuf::from("/path/to/plugins"),
]).await?;

// Register into agent registries
manager.register_skills(&mut skill_registry);
manager.register_subagents(&mut subagent_registry);

// Access hooks and MCP configs
let hooks = manager.hooks();
let mcp_servers = manager.mcp_servers();
let count = manager.plugin_count();
```

### PluginDiscovery

```rust
use claude_agent::plugins::PluginDiscovery;

// Default: ~/.claude/plugins/
let default_dir = PluginDiscovery::default_plugins_dir();

// Discover plugins in directories
let plugins = PluginDiscovery::discover(&dirs)?;
```

### PluginDescriptor

Wraps a `PluginManifest` with path helpers:

```rust
descriptor.name()              // Plugin name
descriptor.root_dir()          // Root directory
descriptor.skills_dir()        // root/skills/
descriptor.commands_dir()      // root/commands/
descriptor.agents_dir()        // root/agents/
descriptor.hooks_config_path() // root/hooks/hooks.json
descriptor.mcp_config_path()   // root/.mcp.json
```

## PluginResources

Loaded resources from a single plugin:

```rust
pub struct PluginResources {
    pub skills: Vec<SkillIndex>,
    pub subagents: Vec<SubagentIndex>,
    pub hooks: Vec<PluginHookEntry>,
    pub mcp_servers: HashMap<String, McpServerConfig>,
}
```

## File Locations

| Type | File |
|------|------|
| `PluginManager` | plugins/manager.rs |
| `PluginLoader` | plugins/loader.rs |
| `PluginDiscovery` | plugins/discovery.rs |
| `PluginManifest` | plugins/manifest.rs |
| `PluginDescriptor` | plugins/manifest.rs |
| `PluginHookEntry` | plugins/loader.rs |
| `PluginError` | plugins/error.rs |
| `namespace` | plugins/namespace.rs |
