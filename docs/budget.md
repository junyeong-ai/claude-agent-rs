# Budget Management

Cost control for agent sessions with automatic tracking and limits.

> **Note**: This document covers **cost budget** (USD). For **token/context window tracking**, see [Token Tracking](tokens.md).

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Budget System                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │
│  │ BudgetTracker │  │ PricingTable  │  │TenantBudget   │    │
│  │               │  │               │  │ Manager       │    │
│  │ - Per session │  │ - Model costs │  │               │    │
│  │ - Auto record │  │ - Per token   │  │ - Multi-tenant│    │
│  │ - OnExceed    │  │ - Per request │  │ - Shared pool │    │
│  └───────────────┘  └───────────────┘  └───────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## BudgetTracker

Per-session cost tracking with configurable limits.

### Basic Usage

```rust
use claude_agent::{BudgetTracker, OnExceed};
use rust_decimal_macros::dec;

// Set $10 limit (uses Decimal for precision)
let tracker = BudgetTracker::new(dec!(10));

// Unlimited (no limit)
let tracker = BudgetTracker::unlimited();
```

### OnExceed Behavior

| Behavior | Description |
|----------|-------------|
| `StopBeforeNext` | Stop before next API call (default) |
| `WarnAndContinue` | Log warning, continue execution |
| `FallbackModel(model)` | Switch to cheaper model |

```rust
use rust_decimal_macros::dec;

// Stop when exceeded (default)
let tracker = BudgetTracker::new(dec!(5))
    .on_exceed(OnExceed::StopBeforeNext);

// Warn but continue
let tracker = BudgetTracker::new(dec!(5))
    .on_exceed(OnExceed::WarnAndContinue);

// Fall back to cheaper model
let tracker = BudgetTracker::new(dec!(5))
    .on_exceed(OnExceed::fallback("claude-haiku-4-5-20251001"));
```

### Checking Status

```rust
let status = tracker.check();

match status {
    BudgetStatus::Unlimited { used } => {
        println!("Used: ${:.4}", used);
    }
    BudgetStatus::WithinBudget { used, limit, remaining } => {
        println!("${:.4} of ${:.2} (${:.4} remaining)", used, limit, remaining);
    }
    BudgetStatus::Exceeded { used, limit, overage } => {
        println!("Exceeded by ${:.4}", overage);
    }
}

// Helper methods
if tracker.should_stop() { /* stop execution */ }
if let Some(model) = tracker.should_fallback() { /* switch model */ }
```

## Agent Integration

```rust
use claude_agent::{Agent, BudgetTracker, OnExceed};
use rust_decimal_macros::dec;

let tracker = BudgetTracker::new(dec!(10))
    .on_exceed(OnExceed::StopBeforeNext);

let agent = Agent::builder()
    .auth(Auth::from_env()).await?
    .budget(tracker)
    .build()
    .await?;
```

## PricingTable

Model-specific pricing for cost calculation.

### Default Pricing (per 1M tokens)

| Model | Input | Output |
|-------|-------|--------|
| claude-opus-4-6 | $15.00 | $75.00 |
| claude-sonnet-4-5 | $3.00 | $15.00 |
| claude-haiku-4-5 | $0.80 | $4.00 |

### Custom Pricing

```rust
use claude_agent::{PricingTableBuilder, ModelPricing};
use rust_decimal_macros::dec;

// Full pricing with all 5 fields
let pricing = PricingTableBuilder::new()
    .model("my-model", ModelPricing::new(
        dec!(2),    // input_per_mtok
        dec!(10),   // output_per_mtok
        dec!(0.2),  // cache_read_per_mtok
        dec!(2.5),  // cache_write_per_mtok
        dec!(2),    // long_context_multiplier
    ))
    .build();

// Or use from_base() to auto-derive cache pricing
let pricing = PricingTableBuilder::new()
    .model_base("my-model", dec!(2), dec!(10))
    .build();
```

## TenantBudgetManager

Multi-tenant budget management with per-tenant cost tracking.

```rust
use claude_agent::TenantBudgetManager;
use rust_decimal_macros::dec;

let manager = TenantBudgetManager::new();

// Set per-tenant budgets
manager.set_budget("tenant-a", dec!(100));
manager.set_budget("tenant-b", dec!(50));

// Record usage against tenant
let cost = manager.record("tenant-a", "claude-sonnet-4-5", &usage);

// Check tenant budget
if let Some(budget) = manager.get("tenant-a") {
    println!("Used: {}, Remaining: {}", budget.used_cost_usd(), budget.remaining());
    if budget.is_exceeded() { /* handle */ }
}

// Check if tenant should stop
if manager.should_stop("tenant-a") { /* stop execution */ }

// Get summary of all tenants
let summaries = manager.summary();
```

## Cost Calculation

Costs are calculated from API response usage using `rust_decimal` for precise monetary calculations:

```rust
// ModelPricing fields (per million tokens)
pub struct ModelPricing {
    pub input_per_mtok: Decimal,
    pub output_per_mtok: Decimal,
    pub cache_read_per_mtok: Decimal,    // Default: 10% of input
    pub cache_write_per_mtok: Decimal,   // Default: 125% of input
    pub long_context_multiplier: Decimal, // Default: 2x for >200K tokens
}

// Long context threshold: 200,000 tokens
// When total context > 200K, input/cache costs are multiplied
```

## See Also

- [Token Tracking](tokens.md) - Context window management and pre-flight validation
- [Models](models.md) - Model registry and pricing tiers
