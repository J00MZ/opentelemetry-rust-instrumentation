# OpenTelemetry Auto-Instrumentation for Rust

This project adds [OpenTelemetry instrumentation](https://opentelemetry.io/docs/concepts/instrumenting/#automatic-instrumentation) to Rust applications without having to modify their source code.

Our goal is to provide the same level of automatic instrumentation for Rust as exists for languages such as Java, Python, and Go.

This automatic instrumentation is based on [eBPF](https://ebpf.io/) uprobes.

## Features

- **No code changes required** - Instrument any Rust application without modifying source code
- **Wide Rust version support** - Works with Rust 1.70+ compiled binaries
- **Stripped binary support** - Works on release builds with debug symbols stripped
- **Low overhead** - eBPF-based instrumentation with minimal performance impact
- **OpenTelemetry compliant** - Produces standard OpenTelemetry traces

## Current Instrumentations

| Library/Framework | Type |
| ----------------- | ---- |
| hyper             | HTTP Server |
| axum              | HTTP Server (via hyper) |
| tonic             | gRPC Client/Server |
| reqwest           | HTTP Client |

## Quick Start

### Prerequisites

- Linux kernel 5.8+ (for BPF features)
- CAP_SYS_PTRACE capability
- Target application running

### Running with Docker

```bash
docker run --privileged \
  -e OTEL_TARGET_EXE=/path/to/your/rust-app \
  -e OTEL_SERVICE_NAME=my-rust-service \
  -e OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4317 \
  -v /sys/kernel/debug:/sys/kernel/debug \
  --pid=host \
  otel/rust-instrumentation:latest
```

### Running on Kubernetes

See the [Getting Started Guide](docs/getting-started/README.md) for Kubernetes deployment examples.

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_TARGET_EXE` | Path to target executable | Required (or `OTEL_TARGET_PID`) |
| `OTEL_TARGET_PID` | PID of target process | Required (or `OTEL_TARGET_EXE`) |
| `OTEL_SERVICE_NAME` | Service name for traces | Required |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP collector endpoint | `http://localhost:4317` |
| `OTEL_STDOUT` | Output traces to stdout | `false` |

## How It Works

This instrumentation works by:

1. **Binary Analysis** - Parsing the target Rust binary to find instrumentation points
2. **Symbol Resolution** - Demangling Rust symbols to identify library functions
3. **eBPF Probes** - Attaching uprobes to function entry/exit points
4. **Event Collection** - Collecting timing and context data via perf events
5. **Span Export** - Converting events to OpenTelemetry spans and exporting via OTLP

For more details, see [How It Works](docs/how-it-works.md).

## Building from Source

### Prerequisites

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install build dependencies (Fedora/RHEL)
sudo dnf install clang llvm libbpf-devel

# Install build dependencies (Ubuntu/Debian)
sudo apt install clang llvm libelf-dev libbpf-dev
```

### Build

```bash
make build
```

### Docker Build

```bash
make docker-build IMG=my-registry/rust-instrumentation:latest
```

## Project Status

This project is in early development. We welcome contributions and feedback!

## Contributing

Please refer to [CONTRIBUTING.md](CONTRIBUTING.md) for information about how to get involved.

## License

This project is licensed under the Apache 2.0 License. See [LICENSE](LICENSE) for details.

## Related Projects

- [opentelemetry-rust](https://github.com/open-telemetry/opentelemetry-rust) - OpenTelemetry Rust SDK
- [opentelemetry-go-instrumentation](https://github.com/open-telemetry/opentelemetry-go-instrumentation) - Go auto-instrumentation (inspiration for this project)
- [aya](https://github.com/aya-rs/aya) - eBPF library for Rust

