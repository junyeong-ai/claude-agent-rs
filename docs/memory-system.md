# Memory System

The memory system provides project context through CLAUDE.md files and rules.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Resource Levels                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Enterprise  /Library/Application Support/ClaudeCode/        │
│      ↓                                                       │
│  User        ~/.claude/                                      │
│      ↓                                                       │
│  Project     {project}/.claude/                              │
│      ↓                                                       │
│  Local       {project}/CLAUDE.local.md (gitignored)         │
│                                                              │
│  Later levels override earlier levels.                       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Usage

```rust
Agent::builder()
    .from_claude_code(path).await?    // Auth + working_dir
    .with_enterprise_resources()      // Enable enterprise level
    .with_user_resources()            // Enable user level
    .with_project_resources()         // Enable project level
    .with_local_resources()           // Enable local level
    .build()
    .await?;
```

**Important**: The `with_*_resources()` methods only enable loading from specific levels.
Actual loading happens during `build()` in a **fixed order** regardless of chaining order:

```
Enterprise → User → Project → Local
```

This ensures consistent override behavior:
- Chaining order does not affect override priority
- Later levels always override earlier levels
- Safe to call methods in any order

## CLAUDE.md

Project-level instructions loaded automatically.

### Locations per Level

| Level | CLAUDE.md Path |
|-------|----------------|
| Enterprise | `/Library/Application Support/ClaudeCode/CLAUDE.md` |
| User | `~/.claude/CLAUDE.md` |
| Project | `{project}/CLAUDE.md`, `{project}/.claude/CLAUDE.md` |
| Local | `{project}/CLAUDE.local.md`, `{project}/.claude/CLAUDE.local.md` |

### Example

```markdown
# Project Guidelines

## Architecture
- Use clean architecture pattern
- Keep services stateless

## Coding Standards
- All public functions must be documented
- Use `Result<T, E>` for fallible operations

@import ./docs/api-guidelines.md
```

## CLAUDE.local.md

Personal settings (should be gitignored).

```markdown
# Local Settings

## My Preferences
- Use vim keybindings
- Enable verbose logging

## Local Paths
- Test DB: localhost:5432
```

## @import Syntax

Include content from other files:

```markdown
# Main Document

@./relative/path.md
@/absolute/path.md
@~/home/relative/path.md

## More Content
```

**Limits**:
- Maximum 5 import hops (prevents cycles)
- Circular imports are detected and skipped
- Missing files log warning, don't fail

## Rules System

Rules provide path-specific context loaded on demand.

### Directory Structure

Rules are loaded recursively from `.claude/rules/`:

```
.claude/
└── rules/
    ├── rust.md
    ├── security.md
    ├── api/
    │   └── endpoints.md
    └── frontend/
        ├── react.md
        └── styles.md
```

### Rule Format

`.claude/rules/rust.md`:

```markdown
---
paths:
  - "**/*.rs"
priority: 10
---

# Rust Guidelines

- Use `snake_case` for functions
- Prefer `impl Trait` over generics when possible
- Add `#[must_use]` to functions returning important values
```

### Frontmatter Options

| Field | Type | Description |
|-------|------|-------------|
| `paths` | array | Glob patterns for matching files (YAML list) |
| `priority` | number | Higher = loaded first |
| `description` | string | Rule description (optional) |

### Progressive Disclosure

Rules implement progressive disclosure:

1. **Index only**: Rule names and patterns in memory
2. **On match**: Full content loaded when path matches
3. **Context efficiency**: Only relevant rules consume tokens

## MemoryLoader API

```rust
use claude_agent::context::MemoryLoader;

let loader = MemoryLoader::new();

// Load everything (CLAUDE.md + CLAUDE.local.md + rules)
let content = loader.load(&project_dir).await?;

// Or load selectively:
// load_shared(): CLAUDE.md + rules (for any level: enterprise/user/project)
// load_local(): CLAUDE.local.md only (project-level private config)
let shared = loader.load_shared(&project_dir).await?;
let local = loader.load_local(&project_dir).await?;

// Access components
let claude_md = content.combined_claude_md();
let rules = content.rule_indices;
```

### Method Summary

| Method | Loads | Use Case |
|--------|-------|----------|
| `load()` | All (shared + local + rules) | Full content from single directory |
| `load_shared()` | CLAUDE.md + rules | Any level (enterprise/user/project) |
| `load_local()` | CLAUDE.local.md | Project-level private config |

## LeveledMemoryProvider

For multi-level resource aggregation:

```rust
use claude_agent::LeveledMemoryProvider;

let mut provider = LeveledMemoryProvider::new();
provider.add_content("# Enterprise Rules");
provider.add_content("# User Preferences");
provider.add_content("# Project Guidelines");

// Content is merged in order added (later overrides earlier)
let content = provider.load().await?;
```

## MemoryContent Structure

```rust
pub struct MemoryContent {
    pub claude_md: Vec<String>,       // CLAUDE.md contents
    pub local_md: Vec<String>,        // CLAUDE.local.md contents
    pub rule_indices: Vec<RuleIndex>, // Rule metadata
}
```

## Settings Cascade

Settings follow the same level order:

```
Enterprise settings.json
    ↓ (overridden by)
User ~/.claude/settings.json
    ↓ (overridden by)
Project .claude/settings.json
    ↓ (overridden by)
Local .claude/settings.local.json
    ↓ (overridden by)
Explicit code configuration
```

## Best Practices

### CLAUDE.md

1. Keep concise - loaded every conversation
2. Use @import for detailed docs
3. Focus on project-specific guidance
4. Include architecture overview

### CLAUDE.local.md

1. Personal preferences only
2. Add to .gitignore
3. Local environment settings
4. Debug configurations

### Rules

1. Use specific path patterns
2. Set appropriate priorities
3. Keep rules focused
4. Use for file-type specific guidance
5. Organize in subdirectories for complex projects
