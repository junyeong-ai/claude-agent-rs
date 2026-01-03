# Budget Management

Cost control for agent sessions with automatic tracking and limits.

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

// Set $10 limit
let tracker = BudgetTracker::new(10.0);

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
// Stop when exceeded (default)
let tracker = BudgetTracker::new(5.0)
    .with_on_exceed(OnExceed::StopBeforeNext);

// Warn but continue
let tracker = BudgetTracker::new(5.0)
    .with_on_exceed(OnExceed::WarnAndContinue);

// Fall back to cheaper model
let tracker = BudgetTracker::new(5.0)
    .with_on_exceed(OnExceed::fallback("claude-haiku-4-5-20251001"));
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
use claude_agent::{Agent, BudgetConfig};

let agent = Agent::builder()
    .budget(BudgetConfig {
        max_cost_usd: Some(10.0),
        on_exceed: OnExceed::StopBeforeNext,
    })
    .build()
    .await?;
```

## PricingTable

Model-specific pricing for cost calculation.

### Default Pricing (per 1M tokens)

| Model | Input | Output |
|-------|-------|--------|
| claude-opus-4-5 | $15.00 | $75.00 |
| claude-sonnet-4-5 | $3.00 | $15.00 |
| claude-haiku-4-5 | $0.80 | $4.00 |

### Custom Pricing

```rust
use claude_agent::{PricingTableBuilder, ModelPricing};

let pricing = PricingTableBuilder::new()
    .add_model("my-model", ModelPricing {
        input_per_million: 2.0,
        output_per_million: 10.0,
    })
    .build();
```

## TenantBudgetManager

Multi-tenant budget allocation with shared pools.

```rust
use claude_agent::{TenantBudgetManager, TenantBudget};

let manager = TenantBudgetManager::new(1000.0); // $1000 total pool

// Allocate to tenant
manager.create_tenant("tenant-a", TenantBudget {
    max_daily: 50.0,
    max_monthly: 500.0,
})?;

// Check tenant budget
let budget = manager.get_budget("tenant-a")?;
if budget.can_spend(5.0) {
    // proceed
}
```

## Cost Calculation

Costs are calculated from API response usage:

```rust
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

// Cost = (input * input_rate + output * output_rate) / 1_000_000
// Cache creation: 25% more than input
// Cache read: 90% discount
```
