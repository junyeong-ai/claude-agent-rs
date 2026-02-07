# Output Styles

Output styles customize Claude's behavior and response format.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    System Prompt Generation                  │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  CLI Identity (conditional: require_cli_identity)     │   │
│  │  "You are Claude Code, Anthropic's official CLI..."  │   │
│  └──────────────────────────────────────────────────────┘   │
│                          │                                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Base System Prompt (always included)                 │   │
│  │  Tone, style, professional objectivity               │   │
│  └──────────────────────────────────────────────────────┘   │
│                          │                                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Tool Usage Policy (always included)                  │   │
│  │  Tool-specific guidelines                            │   │
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
.keep_coding_instructions(false);
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
| `keep-coding-instructions` | boolean | true | Include standard coding sections |

## keep-coding-instructions

Controls what's included in the system prompt.

### What keep-coding-instructions Controls

When `keep-coding-instructions: true`, the coding instructions section from `prompts/coding.rs` is included. This contains software engineering instructions and git commit/PR protocols.

The **Base System Prompt** (tone, style, task management) and **Tool Usage Policy** (tool-specific guidelines) are always included regardless of this setting.

### true (default style)

```
1. CLI_IDENTITY (only if require_cli_identity is true)
   "You are Claude Code, Anthropic's official CLI..."

2. BASE_SYSTEM_PROMPT (always)
   Tone, style, professional objectivity, task management

3. TOOL_USAGE_POLICY (always)
   Tool-specific guidelines

4. coding_instructions() (conditional on keep-coding-instructions)
   Software engineering instructions, git protocols

5. Custom Prompt (if set)

6. Environment Block (always)
   <env>Working directory: ...</env>
```

### false (custom replacement)

```
1. CLI_IDENTITY (only if require_cli_identity is true)
   "You are Claude Code, Anthropic's official CLI..."

2. BASE_SYSTEM_PROMPT (always)
   Tone, style, professional objectivity, task management

3. TOOL_USAGE_POLICY (always)
   Tool-specific guidelines

4. Custom Prompt (your content replaces coding instructions)

5. Environment Block (always)
   <env>Working directory: ...</env>
```

### Code Reference

```rust
// src/output_style/generator.rs — SystemPromptGenerator::generate()
pub fn generate(&self) -> String {
    let mut parts = Vec::new();

    // 1. CLI Identity (required for CLI OAuth, cannot be replaced)
    if self.require_cli_identity {
        parts.push(CLI_IDENTITY.to_string());
    }

    // 2. Base System Prompt (always)
    parts.push(BASE_SYSTEM_PROMPT.to_string());

    // 3. Tool Usage Policy (always)
    parts.push(TOOL_USAGE_POLICY.to_string());

    // 4. Coding Instructions (conditional)
    if self.style.keep_coding_instructions {
        parts.push(coding::coding_instructions(&self.model_name));
    }

    // 5. Custom Prompt (if present)
    if !self.style.prompt.is_empty() {
        parts.push(self.style.prompt.clone());
    }

    // 6. Environment Block (always)
    parts.push(environment_block(...));

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

> **Note**: `OutputStyleLoader` requires the `cli-integration` feature flag.

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
    .from_claude_code(".").await?
    .output_style(style)
    .build()
    .await?;
```

## System Prompt Generation

> **Note**: `SystemPromptGenerator` requires the `cli-integration` feature flag.

```rust
use claude_agent::output_style::SystemPromptGenerator;

// Generate with default style (no CLI identity)
let prompt = SystemPromptGenerator::new()
    .working_dir("/path/to/project")
    .model("claude-sonnet-4-5-20250929")
    .generate();

// Generate with CLI identity (required for CLI OAuth)
let prompt = SystemPromptGenerator::cli_identity()
    .working_dir("/path/to/project")
    .model("claude-sonnet-4-5-20250929")
    .generate();

// Generate with custom style
let prompt = SystemPromptGenerator::new()
    .output_style(custom_style)
    .working_dir("/path/to/project")
    .generate();
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
