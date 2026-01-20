# Token Tracking

Context window management and pre-flight validation for Claude API.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Token Tracking System                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │
│  │  TokenBudget  │  │ ContextWindow │  │ TokenTracker  │    │
│  │               │  │               │  │               │    │
│  │ - input       │  │ - limit       │  │ - preflight   │    │
│  │ - cache_read  │  │ - usage       │  │ - record      │    │
│  │ - cache_write │  │ - status()    │  │ - estimate    │    │
│  │ - output      │  │ - can_fit()   │  │               │    │
│  └───────────────┘  └───────────────┘  └───────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Key Concept: Billing vs Context

The API reports different token counts for billing vs actual context window usage:

| Metric | Description | Billing Impact |
|--------|-------------|----------------|
| `input_tokens` | Non-cached input tokens | Full price |
| `cache_read_tokens` | Tokens read from cache | 90% discount |
| `cache_write_tokens` | Tokens written to cache | 25% markup |
| `output_tokens` | Generated output tokens | Full price |

**Critical Formula:**
```
context_window_usage = input_tokens + cache_read_tokens + cache_write_tokens
```

This is why `input_tokens: 3` can appear while actual context usage is 219,821 tokens (most from cache).

## TokenBudget

Tracks all token types separately:

```rust
use claude_agent::tokens::TokenBudget;

let budget = TokenBudget {
    input_tokens: 100,
    cache_read_tokens: 200_000,
    cache_write_tokens: 0,
    output_tokens: 500,
};

// Context window consumption (input + cache)
assert_eq!(budget.context_usage(), 200_100);

// Total tokens (context + output)
assert_eq!(budget.total(), 200_600);
```

### Overflow Protection

All arithmetic uses saturating operations:

```rust
let mut a = TokenBudget::default();
let b = TokenBudget { input_tokens: u64::MAX, ..Default::default() };

a.add(&b);  // Saturates at u64::MAX, no overflow
```

## ContextWindow

Tracks usage against model limits with status thresholds:

```rust
use claude_agent::tokens::ContextWindow;
use claude_agent::models::read_registry;

let registry = read_registry();
let spec = registry.resolve("sonnet").unwrap();

let mut window = ContextWindow::new(spec, false);  // Standard context
// let mut window = ContextWindow::new(spec, true); // Extended (1M)

window.update(150_000);
println!("Limit: {}", window.limit());           // 200,000
println!("Usage: {}", window.usage());           // 150,000
println!("Remaining: {}", window.remaining());   // 50,000
println!("Utilization: {:.1}%", window.utilization() * 100.0);  // 75.0%
```

### WindowStatus

```rust
use claude_agent::tokens::WindowStatus;

match window.status() {
    WindowStatus::Ok { utilization, remaining } => {
        println!("OK: {:.1}% used, {} remaining", utilization * 100.0, remaining);
    }
    WindowStatus::Warning { utilization, remaining } => {
        // >= 80% threshold
        println!("WARNING: {:.1}% used", utilization * 100.0);
    }
    WindowStatus::Critical { utilization, remaining } => {
        // >= 95% threshold
        println!("CRITICAL: {:.1}% used", utilization * 100.0);
    }
    WindowStatus::Exceeded { overage } => {
        println!("EXCEEDED by {} tokens", overage);
    }
}

// Check if request can proceed
if window.status().should_proceed() {
    // Safe to make API call
}
```

### Threshold Configuration

```rust
let window = ContextWindow::new(spec, false)
    .with_thresholds(0.70, 0.90);  // Warning at 70%, Critical at 90%
```

Default thresholds:
- Warning: 80%
- Critical: 95%

## TokenTracker

Pre-flight validation using the `count_tokens` API:

```rust
use claude_agent::tokens::{TokenTracker, PreflightResult};
use claude_agent::models::read_registry;

let registry = read_registry();
let spec = registry.resolve("sonnet").unwrap();

let tracker = TokenTracker::new(spec, false);

// After API response, record the usage
tracker.record(&response.usage);

// Check current state
let status = tracker.status();
println!("Context usage: {}", tracker.context_usage());
```

### PreflightResult

```rust
pub enum PreflightResult {
    Ok {
        estimated_tokens: u64,
        remaining: u64,
    },
    Warning {
        estimated_tokens: u64,
        utilization: f64,
    },
    Exceeded {
        estimated_tokens: u64,
        limit: u64,
        overage: u64,
    },
}
```

## PricingTier

Determines pricing multiplier based on context usage:

```rust
use claude_agent::tokens::PricingTier;

let tier = PricingTier::for_context(150_000);  // Standard
assert_eq!(tier.multiplier(), 1.0);

let tier = PricingTier::for_context(250_000);  // Extended
assert_eq!(tier.multiplier(), 2.0);
assert!(tier.is_extended());
```

| Tier | Context Range | Multiplier |
|------|---------------|------------|
| Standard | ≤ 200,000 | 1.0x |
| Extended | > 200,000 | 2.0x |

## Agent Integration

Token tracking is automatic in Agent:

```rust
use claude_agent::{Agent, AgentEvent};

let agent = Agent::builder()
    .auth(auth).await?
    .extended_context(true)  // Enable 1M context
    .build()
    .await?;

let result = agent.execute("...").await?;

// Access token metrics
let metrics = result.metrics();
println!("Input tokens: {}", metrics.input_tokens);
println!("Cache read: {}", metrics.cache_read_input_tokens);
println!("Context usage: {}", metrics.context_usage());
```

## Best Practices

1. **Monitor context utilization**: Watch for Warning/Critical status
2. **Use extended context for large codebases**: Enable with `.extended_context(true)`
3. **Leverage prompt caching**: High cache_read + low input = optimal
4. **Pre-flight validation**: Check before expensive API calls

## Constants

```rust
// Context threshold for Standard/Extended pricing
pub const LONG_CONTEXT_THRESHOLD: u64 = 200_000;

// Default status thresholds
pub const DEFAULT_WARNING_THRESHOLD: f64 = 0.80;   // 80%
pub const DEFAULT_CRITICAL_THRESHOLD: f64 = 0.95;  // 95%
```

## See Also

- [Model Registry](models.md) - Model capabilities and extended context
- [Budget Management](budget.md) - Cost tracking (USD)
