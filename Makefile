REPODIR := $(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))

BPF_INCLUDE += -I${REPODIR}/include/libbpf
BPF_INCLUDE += -I${REPODIR}/include

TARGET ?= $(shell uname -m)
ifeq ($(TARGET),x86_64)
	TARGET := x86_64
else ifeq ($(TARGET),aarch64)
	TARGET := aarch64
endif

CLANG ?= clang
LLC ?= llc
CARGO ?= cargo

BPF_CFLAGS := -O2 -g -target bpf -D__TARGET_ARCH_$(TARGET) $(BPF_INCLUDE)

BPF_SOURCES := $(shell find pkg/instrumentors/bpf -name '*.bpf.c')
BPF_OBJECTS := $(BPF_SOURCES:.bpf.c=.bpf.o)

.PHONY: all
all: build

.PHONY: bpf
bpf: $(BPF_OBJECTS)

%.bpf.o: %.bpf.c
	$(CLANG) $(BPF_CFLAGS) -c $< -o $@

.PHONY: build
build: bpf
	$(CARGO) build --release

.PHONY: build-debug
build-debug: bpf
	$(CARGO) build

.PHONY: test
test:
	$(CARGO) test

.PHONY: clippy
clippy:
	$(CARGO) clippy -- -D warnings

.PHONY: fmt
fmt:
	$(CARGO) fmt

.PHONY: fmt-check
fmt-check:
	$(CARGO) fmt -- --check

.PHONY: clean
clean:
	$(CARGO) clean
	find pkg/instrumentors/bpf -name '*.bpf.o' -delete

.PHONY: docker-build
docker-build:
	docker build -t $(IMG) .

.PHONY: docker-push
docker-push:
	docker push $(IMG)

.PHONY: install-deps
install-deps:
	@echo "Installing build dependencies..."
	@if command -v dnf >/dev/null 2>&1; then \
		sudo dnf install -y clang llvm libbpf-devel elfutils-libelf-devel; \
	elif command -v apt-get >/dev/null 2>&1; then \
		sudo apt-get update && sudo apt-get install -y clang llvm libelf-dev libbpf-dev; \
	else \
		echo "Unknown package manager. Please install clang, llvm, libelf, and libbpf manually."; \
		exit 1; \
	fi

.PHONY: help
help:
	@echo "OpenTelemetry Rust Instrumentation"
	@echo ""
	@echo "Targets:"
	@echo "  all          - Build everything (default)"
	@echo "  build        - Build release binary"
	@echo "  build-debug  - Build debug binary"
	@echo "  bpf          - Compile BPF programs only"
	@echo "  test         - Run tests"
	@echo "  clippy       - Run clippy lints"
	@echo "  fmt          - Format code"
	@echo "  fmt-check    - Check code formatting"
	@echo "  clean        - Clean build artifacts"
	@echo "  docker-build - Build Docker image (requires IMG=...)"
	@echo "  docker-push  - Push Docker image (requires IMG=...)"
	@echo "  install-deps - Install build dependencies"
	@echo "  help         - Show this help"

