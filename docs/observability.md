# Observability

Structured tracing, metrics collection, and OpenTelemetry integration.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Observability Stack                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │
│  │   Tracing     │  │   Metrics     │  │ OpenTelemetry │    │
│  │               │  │               │  │   (optional)  │    │
│  │ - SpanContext │  │ - Counter     │  │               │    │
│  │ - ApiCallSpan │  │ - Gauge       │  │ - OTLP export │    │
│  │ - TracingLevel│  │ - Histogram   │  │ - Semantic    │    │
│  └───────────────┘  └───────────────┘  └───────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Enable Feature

```toml
[dependencies]
claude-agent = { version = "0.2", features = ["otel"] }
```

## Built-in Metrics

Local metrics without external dependencies.

### Counter

```rust
use claude_agent::observability::{Counter, MetricsRegistry};

let registry = MetricsRegistry::default();
let requests = registry.counter("api_requests_total");

requests.inc();
requests.add(5);
```

### Gauge

```rust
let active_sessions = registry.gauge("active_sessions");

active_sessions.set(10);
active_sessions.inc();
active_sessions.dec();
```

### Histogram

```rust
let latency = registry.histogram("request_latency_ms");

latency.observe(150.0);
latency.observe(200.0);

// Get statistics
let stats = latency.stats();
println!("p50: {} p99: {}", stats.p50, stats.p99);
```

## Agent Metrics

Built-in metrics for agent operations.

```rust
use claude_agent::observability::AgentMetrics;

let metrics = AgentMetrics::new(&registry);

// Auto-tracked during execution:
// - api_requests_total
// - api_errors_total
// - tool_executions_total
// - tokens_used_total
// - request_latency_ms
```

## Tracing

### SpanContext

```rust
use claude_agent::observability::{SpanContext, TracingLevel};

let ctx = SpanContext::new("my-agent")
    .with_level(TracingLevel::Debug);

ctx.span("operation", || {
    // ... operation
});
```

### TracingLevel

| Level | Description |
|-------|-------------|
| `Off` | No tracing |
| `Error` | Errors only |
| `Info` | Info and above |
| `Debug` | All events |

## OpenTelemetry Integration

Requires `otel` feature.

### Setup

```rust
use claude_agent::observability::{OtelConfig, OtelRuntime};

let config = OtelConfig::new("my-agent")
    .with_endpoint("http://localhost:4317")
    .with_service_version("1.0.0");

let runtime = OtelRuntime::init(&config)?;

// ... use the agent ...

runtime.shutdown(); // Flush before exit
```

### Configuration

```rust
let config = OtelConfig::new("my-agent")
    .with_endpoint("http://otel-collector:4317")
    .with_service_version(env!("CARGO_PKG_VERSION"))
    .with_resource("deployment.environment", "production")
    .with_batch_size(512)
    .with_export_timeout_secs(30);
```

### Semantic Conventions

```rust
use claude_agent::observability::semantic;

// Pre-defined attribute keys
semantic::MODEL_NAME       // "gen_ai.model.name"
semantic::TOKEN_COUNT      // "gen_ai.usage.tokens"
semantic::TOOL_NAME        // "claude.tool.name"
semantic::SESSION_ID       // "claude.session.id"
```

## ObservabilityConfig

Combined configuration for all observability features.

```rust
use claude_agent::observability::{
    ObservabilityConfig, TracingConfig, MetricsConfig
};

let config = ObservabilityConfig::new()
    .with_service_name("my-agent")
    .with_tracing(TracingConfig {
        level: TracingLevel::Info,
        ..Default::default()
    })
    .with_metrics(MetricsConfig::default());

let registry = config.build_registry();
```

## Agent Integration

```rust
let agent = Agent::builder()
    .observability(ObservabilityConfig::new()
        .with_service_name("my-agent"))
    .build()
    .await?;
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP endpoint URL |
| `OTEL_SERVICE_NAME` | Service name |
| `OTEL_LOG_LEVEL` | Logging level |
