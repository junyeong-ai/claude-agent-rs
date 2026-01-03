# Memory System

The memory system provides project context through CLAUDE.md files and rules.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Memory Loading                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ~/.claude/CLAUDE.md     (global)                            │
│       │                                                      │
│       ▼                                                      │
│  /project/CLAUDE.md      (project root)                      │
│       │                                                      │
│       ▼                                                      │
│  /project/src/CLAUDE.md  (subdirectory)                      │
│       │                                                      │
│       ▼                                                      │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                  Combined Context                       │ │
│  │                                                         │ │
│  │  + CLAUDE.local.md (same hierarchy, gitignored)        │ │
│  │  + .claude/rules/ (indexed, loaded on demand)          │ │
│  │  + @import files (max 5 hops)                          │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## CLAUDE.md

Project-level instructions loaded automatically.

### Search Locations

Priority order (all loaded and combined):

1. `~/.claude/CLAUDE.md` - Global settings
2. `~/.claude/.claude/CLAUDE.md` - Nested global
3. `/project/CLAUDE.md` - Project root
4. `/project/.claude/CLAUDE.md` - Hidden project
5. `/project/subdir/CLAUDE.md` - Subdirectories

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

```
.claude/
└── rules/
    ├── rust.md
    ├── security.md
    └── api/
        └── endpoints.md
```

### Rule Format

`.claude/rules/rust.md`:

```markdown
---
paths: **/*.rs
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
| `paths` | glob | Files this rule applies to |
| `priority` | number | Higher = loaded first |

### Progressive Disclosure

Rules implement progressive disclosure:

1. **Index only**: Rule names and patterns in memory
2. **On match**: Full content loaded when path matches
3. **Context efficiency**: Only relevant rules consume tokens

```
System has rules: rust.md, security.md, api/endpoints.md

User: "Edit src/main.rs"
  → Match: **/*.rs
  → Load: rust.md content

User: "Edit src/api/handlers.rs"
  → Match: **/*.rs, api/**
  → Load: rust.md, api/endpoints.md
```

## MemoryLoader API

```rust
use claude_agent::context::MemoryLoader;

let mut loader = MemoryLoader::new();
let content = loader.load_all(&project_dir).await?;

// Access components
let claude_md = content.combined_claude_md();
let rules = content.rule_indices;
```

## MemoryContent Structure

```rust
pub struct MemoryContent {
    pub claude_md: Vec<String>,      // All CLAUDE.md contents
    pub local_md: Vec<String>,       // All CLAUDE.local.md contents
    pub rule_indices: Vec<RuleIndex>, // Rule metadata (not full content)
}
```

## RuleIndex Structure

```rust
pub struct RuleIndex {
    pub name: String,           // Filename without extension
    pub path: PathBuf,          // Full path to rule file
    pub paths: Option<String>,  // Glob pattern
    pub priority: u32,          // Loading priority
}
```

## Context Orchestrator

Coordinates context assembly:

```rust
use claude_agent::context::ContextOrchestrator;

let orchestrator = ContextOrchestrator::new(project_dir);
let context = orchestrator.build_context(&current_file).await?;

// Context includes:
// - Combined CLAUDE.md
// - Matching rules for current_file
// - Any @imported content
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
