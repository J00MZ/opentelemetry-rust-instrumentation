# Design Proposal: Integration with Manual Instrumentation

## Motivation

Users may want to enrich the traces produced by automatic instrumentation with additional spans created manually via the [OpenTelemetry Rust SDK](https://github.com/open-telemetry/opentelemetry-rust).

## Overview

Integration with manual instrumentation happens in two steps:

1. **Modify spans created manually** - Attach a uprobe to the function that creates the span, override the trace ID and parent span ID with the current active span (according to the eBPF map).

2. **Update active span map** - After the span is created, update the eBPF map with the new span as the current span. This step is needed to create traces that combine spans created manually, automatically, and remotely (via context propagation).

This implementation depends on changes described in the [Context Propagation design document](context-propagation.md).

## Instrumenting OpenTelemetry Rust SDK

The following function in the OpenTelemetry SDK may be a good candidate for instrumenting manual span creation:

```rust
// From opentelemetry-sdk/src/trace/tracer.rs
impl Tracer {
    pub fn start_with_context<T>(&self, name: T, parent_cx: &Context) -> Span
    where
        T: Into<Cow<'static, str>>,
    {
        // ...
    }
}
```

By overriding the parent context's span ID and trace ID to match the current span from the eBPF map, manually created spans become children of the automatically instrumented spans.

## Target Functions

### Span Creation

```rust
// opentelemetry::trace::Tracer::start
// opentelemetry::trace::Tracer::start_with_context  
// opentelemetry_sdk::trace::Tracer::build_recording_span
```

### Span Ending

```rust
// opentelemetry::trace::Span::end
// opentelemetry::trace::Span::end_with_timestamp
```

## Implementation Approach

### 1. Detect SDK Usage

Analyze the target binary for OpenTelemetry SDK symbols:

```rust
let sdk_symbols = [
    "opentelemetry_sdk::trace::tracer::Tracer::start",
    "opentelemetry::trace::Tracer::start",
];

for sym in &target.functions {
    if sdk_symbols.iter().any(|s| sym.demangled_name.contains(s)) {
        // Enable SDK integration
    }
}
```

### 2. Share Span Context

The eBPF map `spans_in_progress` is pinned to the BPF filesystem, allowing:
- Automatic instrumentors to write current span context
- SDK integration probes to read and modify span context
- Both to maintain a consistent view of the active span

### 3. Modify Span Builder

When `Tracer::start` is called:

```c
SEC("uprobe/otel_tracer_start")
int uprobe_otel_tracer_start(struct pt_regs *ctx) {
    // Get current active span from map
    void* key = get_current_task_key();
    struct span_context* parent = bpf_map_lookup_elem(&spans_in_progress, &key);
    
    if (parent != NULL) {
        // Modify the SpanBuilder to use this parent
        void* builder_ptr = get_argument(ctx, 2);
        inject_parent_context(builder_ptr, parent);
    }
    
    return 0;
}
```

## Example Integration

```rust
use opentelemetry::trace::{Tracer, TracerProvider};
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;

async fn handle_request(req: Request) -> Response {
    // Automatic span: hyper::server handles this
    
    let tracer = global::tracer("my-service");
    
    // Manual span: will be child of automatic span
    let span = tracer.start("process_business_logic");
    
    // Do work...
    do_processing().await;
    
    span.end();
    
    Response::ok()
}
```

With SDK integration enabled, the `process_business_logic` span becomes a child of the automatically created HTTP span.

## Trace Visualization

```
HTTP POST /api/users (auto)
└── process_business_logic (manual)
    └── validate_input (manual)
    └── save_to_database (auto - future sqlx instrumentation)
```

## Future Work

### Use Single Exporter

Applications instrumented both manually and automatically will export spans via two different exporters:
1. User's configured exporter (for manual spans)
2. Agent's OTLP exporter (for automatic spans)

This works but isn't ideal. Future improvements may include:
- Unix domain socket communication between SDK and agent
- Shared exporter configuration
- Agent-side aggregation of all spans

### Baggage Propagation

Beyond span context, propagate OpenTelemetry Baggage through the eBPF maps for cross-cutting concerns like tenant ID, feature flags, etc.

## Safety Considerations

Modifying SDK function arguments requires care:

- **Thread safety** - Use per-thread/per-task keys in eBPF maps
- **Version compatibility** - Track struct layouts across SDK versions
- **Error handling** - Gracefully handle cases where modification fails

The implementation will include thorough testing across SDK versions to ensure stability.

