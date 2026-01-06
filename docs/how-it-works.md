# OpenTelemetry Rust Instrumentation - How It Works

We aim to bring the automatic instrumentation experience found in languages like [Java](https://github.com/open-telemetry/opentelemetry-java-instrumentation), [Python](https://github.com/open-telemetry/opentelemetry-python-contrib), and [Go](https://github.com/open-telemetry/opentelemetry-go-instrumentation) to Rust applications.

## Design Goals

- **No code changes required** - Any Rust application can be instrumented without modifying the source code
- **Wide Rust version support** - Instrumentation works with Rust 1.70+ compiled binaries, including release builds with symbols stripped
- **OpenTelemetry compliant** - Configuration via `OTEL_*` environment variables following the [OpenTelemetry Environment Variable Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/sdk-environment-variables.md)
- **Standard telemetry** - Instrumented libraries follow OpenTelemetry specification and semantic conventions

## Why eBPF

Rust is a compiled language that produces native machine code. Unlike interpreted languages like Python or JVM-based languages like Java, Rust has no runtime that can be hooked for instrumentation.

Fortunately, the Linux kernel provides [eBPF](https://ebpf.io/) - a mechanism to attach user-defined code to process execution. This same technology powers other Cloud Native projects like Cilium and Falco.

## Main Challenges and Solutions

### 1. Symbol Name Mangling

Rust mangles function names to encode type information and prevent symbol conflicts. For example:

```
_ZN5hyper6server4conn4Http16serve_connection17h8a9b2c3d4e5f6g7hE
```

We use `rustc-demangle` to decode these into readable names:

```
hyper::server::conn::Http::serve_connection
```

This allows us to match against known instrumentation targets regardless of the exact mangled form.

### 2. Finding Return Points

Unlike Go's approach with uretprobes (which have issues with Go), we analyze the binary to find all `ret` instructions within instrumented functions. We place uprobes at each return point to capture end timestamps.

For Rust, this is more straightforward than Go because Rust follows the System V AMD64 ABI calling convention.

### 3. Struct Layout and Field Offsets

eBPF programs need to read fields from Rust structs, but these layouts can change between versions. We handle this by:

1. **Tracking offsets by version** - Maintaining a JSON file with known field offsets for each library version
2. **DWARF parsing** - When debug info is available, extracting offsets directly from DWARF data
3. **Heuristics** - For common patterns, using structural analysis to find fields

### 4. Async Runtime Considerations

Rust's async ecosystem (primarily Tokio) presents unique challenges:

- **Task polling** - Async functions may be polled multiple times before completion
- **Task migration** - Tasks can move between threads
- **Future composition** - Spans need to track through `.await` chains

We instrument at the executor level and track task contexts to maintain proper span hierarchies.

### 5. Timestamp Conversion

eBPF's `bpf_ktime_get_ns()` returns monotonic time since boot. We convert to wall-clock timestamps by:

1. Reading the boot time offset at agent startup
2. Adding the offset to monotonic timestamps from eBPF events

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Target Rust Application                       │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │    hyper     │  │    tonic     │  │   reqwest    │          │
│  │  HTTP Server │  │ gRPC Client  │  │ HTTP Client  │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                    │
│         ▼                 ▼                 ▼                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   eBPF Uprobes                           │   │
│  │  (attached to function entry/exit points)                │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Perf Events
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   otel-rust-agent                                │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Process    │  │ Instrumentor │  │ OpenTelemetry│          │
│  │   Analyzer   │  │   Manager    │  │  Controller  │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
│                              │                                   │
│                              ▼                                   │
│                    ┌──────────────┐                             │
│                    │ OTLP Exporter│                             │
│                    └──────┬───────┘                             │
└───────────────────────────┼─────────────────────────────────────┘
                            │
                            ▼
                 ┌──────────────────┐
                 │ OpenTelemetry    │
                 │ Collector/Backend│
                 └──────────────────┘
```

## Instrumentation Flow

1. **Discovery** - Agent finds target process by executable path or PID
2. **Analysis** - Binary is parsed to find symbols matching known instrumentation targets
3. **Attachment** - eBPF uprobes are attached to function entry/exit points
4. **Collection** - Events are collected via perf buffers as requests flow through
5. **Conversion** - Raw events are converted to OpenTelemetry spans
6. **Export** - Spans are batched and exported via OTLP

## Currently Supported Libraries

| Library | Version Range | Instrumented Functions |
|---------|---------------|----------------------|
| hyper   | 0.14+, 1.0+   | HTTP server handlers |
| axum    | 0.6+, 0.7+    | Via hyper integration |
| tonic   | 0.10+, 0.11+  | gRPC client/server |
| reqwest | 0.11+, 0.12+  | HTTP client requests |

## Future Work

- **Context propagation** - Automatically inject/extract trace context from HTTP/gRPC headers
- **Manual instrumentation integration** - Combine with user-created spans
- **Additional frameworks** - actix-web, warp, tower services
- **Database instrumentation** - sqlx, diesel, sea-orm

