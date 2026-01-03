# Output Styles

Output styles customize Claude's behavior and response format.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    System Prompt Generation                  │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Base Identifier (always included)                    │   │
│  │  "You are Claude Code, Anthropic's official CLI..."  │   │
│  └──────────────────────────────────────────────────────┘   │
│                          │                                   │
│                          ▼                                   │
│              ┌───────────────────────┐                       │
│              │ keep-coding-instructions│                      │
│              └───────────┬───────────┘                       │
│                          │                                   │
│          ┌───────────────┴───────────────┐                   │
│          ▼                               ▼                   │
│  ┌───────────────┐              ┌───────────────┐            │
│  │     true      │              │     false     │            │
│  │               │              │               │            │
│  │ + Coding      │              │ Custom prompt │            │
│  │   instructions│              │ only          │            │
│  │ + Custom      │              │               │            │
│  │   prompt      │              │               │            │
│  └───────────────┘              └───────────────┘            │
│                          │                                   │
│                          ▼                                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Environment Block (always appended)                  │   │
│  │  Working directory, platform, date, model info       │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Built-in Styles

### default

Standard coding assistant behavior.

```rust
let style = default_style();
// name: "default"
// keep_coding_instructions: true
// prompt: "" (empty, uses standard instructions)
```

### explanatory

Detailed explanations for each step.

```rust
let style = explanatory_style();
// Includes additional instructions for verbose explanations
```

### learning

Tutorial-oriented responses.

```rust
let style = learning_style();
// Focuses on teaching and explaining concepts
```

## Custom Styles

### Programmatic

```rust
use claude_agent::OutputStyle;

let style = OutputStyle::new(
    "concise",
    "Brief, to-the-point responses",
    r#"
Keep all responses short and focused:
- Maximum 3 sentences per explanation
- Use bullet points
- Skip pleasantries
- Code over prose
    "#,
)
.with_keep_coding_instructions(false);
```

### File-based

Create `.claude/output-styles/concise.md`:

```markdown
---
name: concise
description: Brief, to-the-point responses
keep-coding-instructions: false
---

Keep all responses short and focused:
- Maximum 3 sentences per explanation
- Use bullet points
- Skip pleasantries
- Code over prose
```

## Frontmatter Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | filename | Style identifier |
| `description` | string | - | Brief description |
| `keep-coding-instructions` | boolean | false | Include standard coding sections |

## keep-coding-instructions

Controls what's included in the system prompt.

### Sections Controlled by keep-coding-instructions

When `keep-coding-instructions: true`, these 4 sections from `prompts/sections.rs` are included:

| Section | Content |
|---------|---------|
| **Core Principles** | Be helpful, safe, transparent, efficient |
| **Tool Usage Guidelines** | Read, Write, Edit, Glob, Grep, Bash, TodoWrite usage |
| **Important Behaviors** | Read before edit, use absolute paths, batch changes |
| **Safety Rules** | Confirm destructive ops, be cautious with recursion |

### true (default style)

```
1. BASE_IDENTIFIER (always)
   "You are Claude Code, Anthropic's official CLI..."

2. CORE_PRINCIPLES
   ## Core Principles
   1. Be helpful and thorough...
   2. Be safe...
   3. Be transparent...
   4. Be efficient...

3. TOOL_USAGE
   ## Tool Usage Guidelines
   - Read: Use to read file contents...
   - Write: Use to create or overwrite files...
   - Edit: Use for surgical string replacements...
   ...

4. IMPORTANT_BEHAVIORS
   ## Important Behaviors
   - Always read a file before attempting to edit it
   - Use absolute paths when possible
   ...

5. SAFETY_RULES
   ## Safety Rules
   - Never execute commands that could cause data loss...
   ...

6. Custom Prompt (if set)

7. Environment Block (always)
   <env>Working directory: ...</env>
```

### false (custom replacement)

```
1. BASE_IDENTIFIER (always)
   "You are Claude Code, Anthropic's official CLI..."

2. Custom Prompt (your content replaces coding instructions)

3. Environment Block (always)
   <env>Working directory: ...</env>
```

### Code Reference

```rust
// src/output_style/generator.rs:131-145
pub fn generate(&self) -> String {
    let mut parts = Vec::new();

    // 1. Base Identifier (always)
    parts.push(sections::BASE_IDENTIFIER.to_string());

    // 2. Coding Instructions (conditional)
    if self.style.keep_coding_instructions {
        parts.push(sections::coding_instructions());
    }

    // 3. Custom Prompt (if present)
    if !self.style.prompt.is_empty() {
        parts.push(self.style.prompt.clone());
    }

    // 4. Environment Block (always)
    parts.push(sections::environment_block(...));

    parts.join("\n\n")
}
```

## Style Sources

| Source | Location | Priority |
|--------|----------|----------|
| Builtin | SDK | Lowest |
| User | `~/.claude/output-styles/` | Medium |
| Project | `.claude/output-styles/` | Highest |

## Loading Styles

```rust
use claude_agent::output_style::OutputStyleLoader;

let loader = OutputStyleLoader::new();

// Load all available styles
let styles = loader.load_all(&project_dir).await?;

// Find specific style
if let Some(style) = loader.find("concise").await? {
    // Use style
}
```

## Using with Agent

```rust
let agent = Agent::builder()
    .from_claude_code()
    .output_style(style)
    .build()
    .await?;
```

## System Prompt Generation

```rust
use claude_agent::output_style::SystemPromptGenerator;

let generator = SystemPromptGenerator::new();

// Generate with default style
let prompt = generator.generate(
    &default_style(),
    &environment_info,
    &tools,
)?;

// Generate with custom style
let prompt = generator.generate(
    &custom_style,
    &environment_info,
    &tools,
)?;
```

## Environment Block

Always appended, contains:

```
<env>
Working directory: /path/to/project
Is directory a git repo: Yes
Platform: darwin
OS Version: Darwin 25.1.0
Today's date: 2025-01-15
Model: claude-sonnet-4-5
</env>
```

## Example: Research Style

`.claude/output-styles/research.md`:

```markdown
---
name: research
description: Academic research assistant
keep-coding-instructions: false
---

# Research Assistant Mode

You are a research assistant helping with academic work.

## Guidelines
- Cite sources when making claims
- Distinguish between facts and opinions
- Use formal academic language
- Provide balanced perspectives
- Ask clarifying questions before proceeding

## Response Format
1. Summary of understanding
2. Analysis with citations
3. Recommendations
4. Further questions
```

## Example: Minimalist Style

`.claude/output-styles/minimal.md`:

```markdown
---
name: minimal
description: Absolute minimum output
keep-coding-instructions: false
---

Rules:
- No explanations unless asked
- Code only, no comments
- One-word answers when possible
- Skip all pleasantries
```

## Best Practices

1. **Use keep-coding-instructions: true** for coding tasks
2. **Use keep-coding-instructions: false** for specialized roles
3. **Project styles override user styles** - customize per-project
4. **Keep prompts concise** - they're included in every request
5. **Test styles** - different behaviors may affect tool usage
