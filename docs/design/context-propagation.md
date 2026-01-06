# Design Proposal: Context Propagation

## Motivation

Context propagation is a mechanism that allows tracing to be propagated across process boundaries. Propagation is typically done by passing `traceId` and `spanId` of the current span to the next process via headers in requests and responses.

The examples in this proposal describe context propagation over HTTP/gRPC formatted as [W3C Trace Context](https://www.w3.org/TR/trace-context/). However, the implementation should support other transportation methods and header formats.

## Overview

The context propagation implementation should support:

1. **Reading headers** - If the current transaction is part of an existing distributed trace, the request should contain headers according to the chosen format.

2. **Storing the current span context** - Data about the current span is stored in an eBPF map. The suggested data structure is a map from task identifier to span context. Entries can be written by:
   - Header propagator (for remote spans)
   - Automatic instrumentation (for automatically created spans)
   - Manual instrumentation (for user-created spans)

3. **Writing headers** - The implementation should get the current span from the eBPF map and propagate it to the next process by adding headers to outgoing requests.

## Task Identification

Unlike Go's goroutines, Rust async tasks don't have a single consistent identifier. We use a combination of:

- **Thread ID** - For synchronous code
- **Tokio task ID** - When using Tokio runtime (requires `tokio_unstable` cfg)
- **Stack pointer heuristics** - Fallback for identifying unique execution contexts

```c
static __always_inline u64 get_task_key() {
    u64 pid_tgid = bpf_get_current_pid_tgid();
    u64 stack_ptr = PT_REGS_SP(ctx);
    
    // Combine thread ID with stack region for unique key
    return pid_tgid ^ (stack_ptr & 0xFFFF000000000000);
}
```

## Example Walkthrough

Consider this scenario:

```
┌──────────────┐     HTTP      ┌──────────────┐     gRPC      ┌──────────────┐
│ Application A│ ─────────────▶│Target (Rust) │ ─────────────▶│ Application B│
│  (external)  │◀─────────────│   axum+tonic │◀─────────────│  (external)  │
└──────────────┘               └──────────────┘               └──────────────┘
```

The target application is an axum HTTP server that makes gRPC calls via tonic. We assume Applications A and B are already instrumented.

### Step 1: Read HTTP Request Headers

The hyper/axum instrumentation reads headers from `http::Request`:

```rust
// Target function in hyper
impl<T> Request<T> {
    pub fn headers(&self) -> &HeaderMap { ... }
}
```

The eBPF probe reads the `traceparent` header value according to W3C Trace Context specification.

### Step 2: Store as Current Span

Update the span context map:

```c
struct span_context incoming_ctx;
w3c_string_to_span_context(traceparent_header, &incoming_ctx);

u64 key = get_task_key();
bpf_map_update_elem(&spans_in_progress, &key, &incoming_ctx, 0);
```

### Step 3: Create HTTP Span

The HTTP server instrumentor creates a span with the incoming context as parent:

```c
struct http_request_t httpReq = {};
httpReq.start_time = bpf_ktime_get_ns();

// Copy parent trace ID, generate new span ID
bpf_probe_read(&httpReq.sc.TraceID, TRACE_ID_SIZE, &incoming_ctx.TraceID);
generate_random_bytes(httpReq.sc.SpanID, SPAN_ID_SIZE);
```

### Step 4: Update Span Context Map

Replace the incoming context with the newly created HTTP span:

```c
bpf_map_update_elem(&spans_in_progress, &key, &httpReq.sc, 0);
```

### Step 5: Add Headers to gRPC Request

When tonic makes an outgoing gRPC call, we inject the current span context:

```c
SEC("uprobe/tonic_send_request")
int uprobe_tonic_send_request(struct pt_regs *ctx) {
    u64 key = get_task_key();
    struct span_context* current = bpf_map_lookup_elem(&spans_in_progress, &key);
    
    if (current != NULL) {
        // Build traceparent header
        char traceparent[SPAN_CONTEXT_STRING_SIZE];
        span_context_to_w3c_string(current, traceparent);
        
        // Inject into request metadata
        inject_grpc_metadata(ctx, "traceparent", traceparent);
    }
    
    return 0;
}
```

### Step 6: Read gRPC Response Headers

If the downstream service returns trace state, read and update accordingly.

### Step 7: Write HTTP Response Headers

Before responding to the original HTTP request, inject trace headers:

```c
SEC("uprobe/hyper_response_send")
int uprobe_hyper_response_send(struct pt_regs *ctx) {
    u64 key = get_task_key();
    struct span_context* current = bpf_map_lookup_elem(&spans_in_progress, &key);
    
    if (current != NULL) {
        inject_http_header(ctx, "traceparent", current);
    }
    
    return 0;
}
```

## Header Injection Techniques

### HTTP Headers (hyper)

Target the header map manipulation functions:

```rust
// hyper/http
impl HeaderMap {
    pub fn insert<K>(&mut self, key: K, val: HeaderValue) -> Option<HeaderValue>
}
```

Use `bpf_probe_write_user()` to modify the header map, adding our trace context header.

### gRPC Metadata (tonic)

Target tonic's metadata handling:

```rust
// tonic
impl<T> Request<T> {
    pub fn metadata_mut(&mut self) -> &mut MetadataMap
}
```

## Rust-Specific Considerations

### Async Task Context

When `.await` is called, the current task may be suspended and resumed later, potentially on a different thread. We handle this by:

1. **Instrumenting poll functions** - Track when futures are polled
2. **Using wake context** - The waker contains task identity information
3. **Stack-based heuristics** - Use stack regions to correlate across polls

### Zero-Copy String Handling

Rust strings are `(ptr, len)` pairs without null terminators. Our eBPF probes must:

1. Read the pointer and length separately
2. Use bounded reads to prevent overflows
3. Not assume null termination

```c
struct rust_string {
    char* ptr;
    u64 len;
};

static __always_inline int read_rust_string(void* rust_str, char* out, u64 max_len) {
    struct rust_string str;
    bpf_probe_read(&str, sizeof(str), rust_str);
    
    u64 read_len = str.len < max_len ? str.len : max_len;
    return bpf_probe_read(out, read_len, str.ptr);
}
```

## Supported Propagation Formats

| Format | Status | Header Names |
|--------|--------|--------------|
| W3C Trace Context | Implemented | `traceparent`, `tracestate` |
| B3 Single | Planned | `b3` |
| B3 Multi | Planned | `X-B3-TraceId`, `X-B3-SpanId`, `X-B3-Sampled` |
| Jaeger | Planned | `uber-trace-id` |

## Configuration

Propagation format is configured via environment variable:

```bash
OTEL_PROPAGATORS=tracecontext,baggage   # default
OTEL_PROPAGATORS=b3multi
OTEL_PROPAGATORS=jaeger
```

## Safety Considerations

Modifying request/response data requires care:

1. **Buffer bounds** - Never write beyond allocated memory
2. **Atomicity** - Header injection should be atomic where possible
3. **Fallback** - If injection fails, continue without breaking the request

## Future Work

- Support for OpenTelemetry Baggage propagation
- Custom propagator support via configuration
- Correlation with browser/mobile client traces

