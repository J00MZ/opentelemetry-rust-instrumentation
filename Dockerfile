FROM fedora:45 AS builder

ARG TARGETARCH

RUN dnf install -y \
    clang \
    llvm \
    libbpf-devel \
    elfutils-libelf-devel \
    make \
    gcc \
    pkg-config \
    && dnf clean all

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

COPY Cargo.toml Cargo.lock* ./
COPY cli/ cli/
COPY pkg/ pkg/
COPY include/ include/
COPY Makefile ./

RUN make build

FROM registry.fedoraproject.org/fedora-minimal:45

RUN microdnf install -y libbpf elfutils-libelf && microdnf clean all

COPY --from=builder /app/target/release/otel-rust-agent /otel-rust-agent

ENV OTEL_SERVICE_NAME=""
ENV OTEL_TARGET_EXE=""
ENV OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"

ENTRYPOINT ["/otel-rust-agent"]

