# Skills System

Skills are specialized workflows that activate on-demand for context optimization.

> **Related**: [Subagents Guide](subagents.md) for independent agent execution with similar configuration options

## Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Skill Sources                         │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐           │
│  │ Enterprise │ │    User    │ │  Project   │           │
│  │ /Library/..│ │ ~/.claude/ │ │ .claude/   │           │
│  │ skills/    │ │  skills/   │ │  skills/   │           │
│  └─────┬──────┘ └─────┬──────┘ └─────┬──────┘           │
│        │              │              │                   │
│        └──────────────┼──────────────┘                   │
│                       ▼                                  │
│              ┌────────────────┐                          │
│              │ SkillRegistry  │                          │
│              └───────┬────────┘                          │
│                      │                                   │
│          ┌───────────┼───────────┐                       │
│          ▼           ▼           ▼                       │
│     Explicit    Trigger      Slash                       │
│      Call       Match       Command                      │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Skill Definition

### Programmatic

```rust
use claude_agent::{SkillIndex, ContentSource};

let skill = SkillIndex::new("deploy", "Production deployment workflow")
    .with_source(ContentSource::in_memory(r#"
Deploy the application to $ARGUMENTS environment:
1. Run tests
2. Build artifacts
3. Deploy to server
4. Verify health checks
    "#))
    .with_triggers(["deploy", "release"])
    .with_allowed_tools(["Bash", "Read"])
    .with_model("claude-sonnet-4-5-20250929");
```

### File-based

Create `.claude/skills/deploy.md`:

```markdown
---
name: deploy
description: Production deployment workflow
allowed-tools:
  - Bash
  - Read
model: claude-sonnet-4-5-20250929
---

Deploy the application to $ARGUMENTS environment:
1. Run tests
2. Build artifacts
3. Deploy to server
4. Verify health checks
```

## Frontmatter Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | (required) | Skill identifier |
| `description` | string | (required) | Brief description |
| `allowed-tools` | array | `[]` | Tool restrictions |
| `model` | string | — | Model override |
| `triggers` | array | `[]` | Keywords for auto-activation |
| `argument-hint` | string | — | Usage hint (e.g., `"[file or PR]"`) |
| `disable-model-invocation` | bool | `false` | Prevent model from invoking this skill |
| `user-invocable` | bool | `true` | Whether users can invoke via slash command |
| `context` | string | — | Additional context identifier |
| `agent` | string | — | Target agent for execution |
| `hooks` | object | — | Lifecycle hooks (map of event → `HookRule[]`) |

## File Patterns

Skills can be defined as:
- **File**: `.claude/skills/deploy.skill.md`
- **Directory**: `.claude/skills/deploy/SKILL.md` (for skills with supporting files)

## Activation Methods

### 1. Explicit Invocation

Use the Skill tool directly:

```json
{
  "skill": "deploy",
  "args": "production"
}
```

### 2. Trigger-based

Skills can auto-activate based on keywords:

```rust
let skill = SkillIndex::new("deploy", "Deploy")
    .with_source(ContentSource::in_memory("..."))
    .with_triggers(["deploy", "release", "ship"]);

// "deploy to production" → activates deploy skill
// "ship it" → also activates deploy skill
```

### 3. Slash Commands

User-defined commands in `.claude/commands/`:

```
.claude/commands/
├── deploy.md
├── review.md
└── db/
    ├── migrate.md
    └── seed.md
```

Command namespace: `db:migrate`, `db:seed`

## Slash Command Format

`.claude/commands/review.md`:

```markdown
---
description: Code review workflow
allowed-tools:
  - Read
  - Grep
  - Glob
argument-hint: "[file or PR number]"
model: claude-haiku-4-5-20251001
---

Review the code in $ARGUMENTS:
1. Check for bugs and security issues
2. Verify code style
3. Suggest improvements

Use $1 for the first argument, $2 for second, etc.
```

## Variable Substitution

| Variable | Description |
|----------|-------------|
| `$ARGUMENTS` | Full argument string |
| `$1`, `$2`... | Positional arguments (up to $9) |

### File References

```markdown
# Include another file
@./guidelines.md
@~/global-rules.md
```

### Bash Execution

```markdown
# Execute command and include output
Current branch: !`git branch --show-current`
```

## Tool Restrictions

Skills can limit available tools:

```rust
let skill = SkillIndex::new("reader", "Read-only")
    .with_source(ContentSource::in_memory("..."))
    .with_allowed_tools(["Read", "Glob", "Grep"]);

// Only Read, Glob, Grep are available during this skill
```

Pattern-based restrictions:

```rust
.with_allowed_tools(["Bash(git:*)", "Read"])
// Bash only for git commands, Read always allowed
```

## Model Override

Skills can specify a different model:

```rust
// Use faster model for simple tasks
let skill = SkillIndex::new("quick-check", "Fast check")
    .with_source(ContentSource::in_memory("..."))
    .with_model("claude-haiku-4-5-20251001");

// Use stronger model for complex tasks
let skill = SkillIndex::new("architect", "Design")
    .with_source(ContentSource::in_memory("..."))
    .with_model("claude-opus-4-5-20251101");
```

## Registration

```rust
let agent = Agent::builder()
    .from_claude_code(path).await?
    .with_project_resources()         // Loads skills from .claude/skills/
    .skill(deploy_skill)              // Add programmatic skills
    .skill(review_skill)
    .build()
    .await?;
```

Or load from directory:

```rust
let loader = SkillIndexLoader::new();
let skills = loader.scan_directory(&skills_dir).await?;

for skill in skills {
    agent_builder = agent_builder.skill(skill);
}
```

## Progressive Disclosure

Skills implement progressive disclosure:

1. **Index only**: Only skill names/descriptions in system prompt
2. **On activation**: Full skill content loaded when triggered
3. **Context efficiency**: Unused skills don't consume tokens

```
System Prompt:
  Skills: deploy, review, migrate (descriptions only)

User: "deploy to production"
  → Load full deploy skill content
  → Execute with skill-specific tools/model
```

## Skill Result

```rust
pub struct SkillResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub allowed_tools: Vec<String>,  // Tool restrictions
    pub model: Option<String>,       // Model override
    pub base_dir: Option<PathBuf>,   // For path resolution
}
```
