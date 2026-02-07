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

Local metrics without external dependencies. `MetricsRegistry` has predefined atomic counters, gauges, and histograms.

### MetricsRegistry Fields

```rust
pub struct MetricsRegistry {
    pub requests_total: Counter,
    pub requests_success: Counter,
    pub requests_error: Counter,
    pub tokens_input: Counter,
    pub tokens_output: Counter,
    pub cache_read_tokens: Counter,
    pub cache_creation_tokens: Counter,
    pub tool_calls_total: Counter,
    pub tool_errors: Counter,
    pub active_sessions: Gauge,
    pub request_latency_ms: Histogram,
    pub cost_total_micros: Counter,
}
```

### Counter

```rust
use claude_agent::observability::Counter;

let counter = Counter::new();
counter.inc();
counter.add(5);
assert_eq!(counter.get(), 6);
```

### Gauge

```rust
use claude_agent::observability::Gauge;

let gauge = Gauge::new();
gauge.set(10);
gauge.inc();
gauge.dec();
assert_eq!(gauge.get(), 10);
```

### Histogram

```rust
use claude_agent::observability::Histogram;

let hist = Histogram::default_latency();
hist.observe(150.0);
hist.observe(200.0);
println!("Count: {}, Sum: {:.1}ms", hist.count(), hist.sum_ms());
```

## Metrics Summary

Get a snapshot summary from the metrics registry.

```rust
use claude_agent::observability::{MetricsRegistry, MetricsSummary};

let registry = MetricsRegistry::default();

// Record some operations...
registry.record_request_start();
registry.record_tokens(100, 50);
registry.record_request_end(true, 150.0);

// Get summary
let summary = MetricsSummary::from_registry(&registry);
println!("Requests: {}", summary.total_requests);
println!("Tokens: {} in / {} out", summary.total_input_tokens, summary.total_output_tokens);
```

## Tracing

### SpanContext

```rust
use claude_agent::observability::{SpanContext, TracingLevel};

let ctx = SpanContext::new("my-agent")
    .level(TracingLevel::Debug);

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
    .endpoint("http://localhost:4317")
    .service_version("1.0.0");

let runtime = OtelRuntime::init(&config)?;

// ... use the agent ...

runtime.shutdown(); // Flush before exit
```

### Configuration

```rust
let config = OtelConfig::new("my-agent")
    .endpoint("http://otel-collector:4317")
    .service_version(env!("CARGO_PKG_VERSION"))
    .sample_ratio(1.0)
    .metrics_interval(Duration::from_secs(60));
```

### Semantic Conventions

```rust
use claude_agent::observability::semantic;

// Pre-defined attribute keys
semantic::AGENT_MODEL           // "agent.model"
semantic::AGENT_SESSION_ID      // "agent.session.id"
semantic::AGENT_TOOL_NAME       // "agent.tool.name"
semantic::AGENT_INPUT_TOKENS    // "agent.tokens.input"
semantic::AGENT_OUTPUT_TOKENS   // "agent.tokens.output"
semantic::AGENT_CACHE_READ_TOKENS    // "agent.tokens.cache_read"
semantic::AGENT_CACHE_CREATION_TOKENS // "agent.tokens.cache_creation"
semantic::AGENT_COST_USD        // "agent.cost.usd"
semantic::AGENT_REQUEST_ID      // "agent.request.id"
semantic::AGENT_TOOL_USE_ID     // "agent.tool.use_id"
```

## ObservabilityConfig

Combined configuration for all observability features.

```rust
use claude_agent::observability::{
    ObservabilityConfig, TracingConfig, MetricsConfig
};

let config = ObservabilityConfig::new()
    .service_name("my-agent")
    .tracing(TracingConfig {
        level: TracingLevel::Info,
        ..Default::default()
    })
    .metrics(MetricsConfig::default());

let registry = config.build_registry();
```

## Agent Integration

```rust
let agent = Agent::builder()
    .auth(Auth::from_env()).await?
    .observability(ObservabilityConfig::new()
        .service_name("my-agent"))
    .build()
    .await?;
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP endpoint URL |
| `OTEL_SERVICE_NAME` | Service name |
| `OTEL_TRACES_SAMPLER_ARG` | Trace sampling ratio (0.0-1.0) |
