# Contributing to OpenTelemetry Rust Instrumentation

Thank you for your interest in contributing to this project! This document provides guidelines and information for contributors.

## Code of Conduct

This project follows the [CNCF Code of Conduct](https://github.com/cncf/foundation/blob/main/code-of-conduct.md). By participating, you are expected to uphold this code.

## Getting Started

### Prerequisites

- Rust 1.70+ (`rustup update stable`)
- Linux kernel 5.8+ (for eBPF)
- clang and LLVM (for BPF compilation)
- libbpf development headers

### Setting Up Development Environment

```bash
# Clone the repository
git clone https://github.com/open-telemetry/opentelemetry-rust-instrumentation.git
cd opentelemetry-rust-instrumentation

# Install system dependencies
make install-deps

# Build the project
make build

# Run tests
make test
```

## How to Contribute

### Reporting Bugs

1. Check existing issues to avoid duplicates
2. Use the bug report template
3. Include:
   - Rust version
   - Kernel version
   - Target application details
   - Steps to reproduce
   - Expected vs actual behavior

### Suggesting Features

1. Open a GitHub issue with the feature request template
2. Describe the use case
3. Explain why this would be valuable

### Pull Requests

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `make test`
5. Run lints: `make clippy`
6. Format code: `make fmt`
7. Commit with a clear message
8. Push and create a PR

## Development Guidelines

### Code Style

- Follow Rust conventions (use `rustfmt`)
- Run `make clippy` before submitting
- Write tests for new functionality
- Update documentation as needed

### Commit Messages

Use conventional commits:

```
feat: add reqwest HTTP client instrumentation
fix: handle stripped binaries correctly
docs: update getting-started guide
test: add integration tests for tonic
```

### Adding New Instrumentations

1. Create a new directory under `pkg/instrumentors/bpf/<library>/`
2. Add the BPF probe in `bpf/probe.bpf.c`
3. Implement the Rust instrumentor in `probe.rs`
4. Register in `pkg/instrumentors/manager.rs`
5. Add to the offsets tracker in `pkg/inject/offset_results.json`
6. Update documentation

### Testing

- Unit tests: `cargo test`
- Integration tests require a Linux environment with eBPF support
- Test with both debug and release builds of target applications

## Architecture Overview

```
opentelemetry-rust-instrumentation/
├── cli/src/main.rs          # CLI entry point
├── pkg/
│   ├── instrumentors/       # Instrumentation logic
│   │   ├── bpf/            # Per-library BPF probes
│   │   └── manager.rs      # Instrumentor orchestration
│   ├── process/            # Binary analysis
│   ├── inject/             # Offset tracking
│   └── opentelemetry/      # OTel SDK integration
└── include/                 # BPF header files
```

## Release Process

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Create a release tag: `git tag v0.x.0`
4. Push the tag: `git push origin v0.x.0`
5. GitHub Actions will build and publish

## Getting Help

- Open a GitHub issue for bugs/features
- Join the [CNCF Slack](https://slack.cncf.io/) #opentelemetry-rust channel
- Attend OpenTelemetry community meetings

## License

By contributing, you agree that your contributions will be licensed under the Apache 2.0 License.

