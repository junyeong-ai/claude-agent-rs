# Model Registry

Runtime-extensible model management with alias resolution and pricing tiers.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Model Registry                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │
│  │  ModelSpec    │  │  ModelFamily  │  │  PricingTier  │    │
│  │               │  │               │  │               │    │
│  │ - id          │  │ - Opus        │  │ - Standard    │    │
│  │ - family      │  │ - Sonnet      │  │   (≤200K)     │    │
│  │ - capabilities│  │ - Haiku       │  │ - Extended    │    │
│  │ - provider_ids│  │               │  │   (>200K, 2x) │    │
│  └───────────────┘  └───────────────┘  └───────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Built-in Models

| Family | Model ID | Context Window | Extended |
|--------|----------|----------------|----------|
| Opus | `claude-opus-4-5-20251101` | 200K | 1M |
| Sonnet | `claude-sonnet-4-5-20250929` | 200K | 1M |
| Haiku | `claude-haiku-4-5-20251001` | 200K | 1M |

## Alias Resolution

The registry supports multiple alias formats:

```rust
use claude_agent::models::read_registry;

let registry = read_registry();

// All of these resolve to the same model
registry.resolve("sonnet");                      // Family alias
registry.resolve("claude-sonnet-4-5-20250929");  // Full ID
registry.resolve("my-sonnet-variant");           // Substring fallback
```

### Resolution Order

1. **Direct ID lookup**: Exact match in registry
2. **Alias lookup**: Registered aliases (e.g., "sonnet" → latest Sonnet)
3. **Substring fallback**: Family detection from name (contains "opus", "sonnet", "haiku")

## ModelSpec

```rust
pub struct ModelSpec {
    pub id: ModelId,
    pub family: ModelFamily,
    pub capabilities: Capabilities,
    pub provider_ids: ProviderIds,
}

pub struct Capabilities {
    pub context_window: u64,      // Standard limit (200K)
    pub extended_context: u64,    // Extended limit (1M)
    pub max_output: u64,
    pub supports_vision: bool,
    pub supports_extended_thinking: bool,
}
```

### Effective Context Window

```rust
impl Capabilities {
    pub fn effective_context(&self, extended_enabled: bool) -> u64 {
        if extended_enabled {
            self.extended_context
        } else {
            self.context_window
        }
    }
}
```

## Pricing Tiers

Two pricing tiers based on context window usage:

| Tier | Token Range | Multiplier |
|------|-------------|------------|
| Standard | ≤ 200,000 | 1.0x |
| Extended | > 200,000 | 2.0x |

```rust
use claude_agent::tokens::PricingTier;

let tier = PricingTier::for_context(150_000);  // Standard
let tier = PricingTier::for_context(250_000);  // Extended

assert_eq!(PricingTier::Standard.multiplier(), 1.0);
assert_eq!(PricingTier::Extended.multiplier(), 2.0);
```

## Extended Context (1M)

Enable 1M context window via AgentBuilder:

```rust
use claude_agent::Agent;

let agent = Agent::builder()
    .auth(auth).await?
    .extended_context(true)  // Enable 1M context
    .build()
    .await?;
```

This automatically adds the `context-1m-2025-08-07` beta feature.

## Runtime Registration

Register custom models at runtime:

```rust
use claude_agent::models::{registry, ModelSpec, ModelFamily, Capabilities};

let mut reg = registry().write().unwrap();
reg.register(ModelSpec {
    id: "my-custom-model".into(),
    family: ModelFamily::Sonnet,
    capabilities: Capabilities {
        context_window: 200_000,
        extended_context: 1_000_000,
        max_output: 16_384,
        supports_vision: true,
        supports_extended_thinking: false,
    },
    provider_ids: Default::default(),
});

// Add alias
reg.add_alias("custom", "my-custom-model".into());
```

## Provider IDs

Models have different IDs across cloud providers:

```rust
pub struct ProviderIds {
    pub anthropic: Option<String>,
    pub bedrock: Option<String>,
    pub vertex: Option<String>,
    pub foundry: Option<String>,
}

// Lookup by provider
let spec = registry.for_provider(CloudProvider::Bedrock, "anthropic.claude-3-5-sonnet-20241022-v2:0");
```

## ModelRole

Default models by role:

| Role | Default | Purpose |
|------|---------|---------|
| Primary | Sonnet | Main agent model |
| Small | Haiku | Subagents, fast tasks |
| Reasoning | Sonnet | Complex reasoning |

```rust
use claude_agent::models::{read_registry, ModelRole};

let registry = read_registry();
let primary = registry.default_for_role(ModelRole::Primary);
let small = registry.default_for_role(ModelRole::Small);
```

## See Also

- [Token Tracking](tokens.md) - Context window management
- [Budget Management](budget.md) - Cost tracking and limits
