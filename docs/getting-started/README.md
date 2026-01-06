# Getting Started with Rust OpenTelemetry Automatic Instrumentation

This guide walks you through instrumenting a Rust application running on Kubernetes without modifying any application code.

## Before You Begin

Required tools:

- [Kind](https://kind.sigs.k8s.io/) - Run a local Kubernetes cluster
- [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/) - Kubernetes CLI
- [Docker](https://www.docker.com/) - Container runtime

## Creating the Kubernetes Cluster

Create a new local Kubernetes cluster:

```bash
kind create cluster
```

## Example Application

We'll use a simple Rust microservice architecture:

```
┌─────────────┐     HTTP      ┌─────────────┐     gRPC      ┌─────────────┐
│   Client    │ ─────────────▶│   Gateway   │ ─────────────▶│   Backend   │
│  (curl)     │               │   (axum)    │               │   (tonic)   │
└─────────────┘               └─────────────┘               └─────────────┘
```

## Deployment

### Deploy Jaeger for Trace Visualization

```bash
kubectl apply -f https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust-instrumentation/main/docs/getting-started/rust-microservice/jaeger.yaml
```

### Deploy the Rust Application

```bash
kubectl apply -f https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust-instrumentation/main/docs/getting-started/rust-microservice/app.yaml
```

### Apply Automatic Instrumentation

```bash
kubectl apply -f https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust-instrumentation/main/docs/getting-started/rust-microservice/instrumented.yaml
```

## Manual Deployment

If you prefer to deploy manually, here's what the instrumented deployment looks like:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rust-gateway
spec:
  replicas: 1
  selector:
    matchLabels:
      app: rust-gateway
  template:
    metadata:
      labels:
        app: rust-gateway
    spec:
      shareProcessNamespace: true  # Required for eBPF
      containers:
        # Your Rust application
        - name: gateway
          image: your-registry/rust-gateway:latest
          ports:
            - containerPort: 8080
          
        # OpenTelemetry auto-instrumentation sidecar
        - name: otel-rust-agent
          image: otel/rust-instrumentation:latest
          env:
            - name: OTEL_TARGET_EXE
              value: /app/gateway
            - name: OTEL_SERVICE_NAME
              value: rust-gateway
            - name: OTEL_EXPORTER_OTLP_ENDPOINT
              value: "http://jaeger:4317"
          securityContext:
            runAsUser: 0
            capabilities:
              add:
                - SYS_PTRACE
            privileged: true
          volumeMounts:
            - mountPath: /sys/kernel/debug
              name: kernel-debug
      volumes:
        - name: kernel-debug
          hostPath:
            path: /sys/kernel/debug
```

Key requirements:

| Setting | Purpose |
|---------|---------|
| `shareProcessNamespace: true` | Allows agent to access target process |
| `SYS_PTRACE` capability | Required for eBPF probe attachment |
| `privileged: true` | Full access for eBPF operations |
| `/sys/kernel/debug` mount | Access to kernel debug filesystem |

## Generate Traffic

Port forward to the gateway service:

```bash
kubectl port-forward svc/rust-gateway 8080:8080
```

Make some requests:

```bash
# Simple request
curl http://localhost:8080/api/hello

# Request that calls backend
curl http://localhost:8080/api/users/123

# Multiple requests for trace variety
for i in {1..10}; do
  curl -s http://localhost:8080/api/users/$i > /dev/null
  sleep 0.5
done
```

## View Traces

Port forward to Jaeger:

```bash
kubectl port-forward svc/jaeger 16686:16686
```

Open http://localhost:16686 in your browser.

### Understanding the Traces

Select the `rust-gateway` service to see traces:

![Jaeger Traces](jaeger_traces.png)

A typical trace shows:

1. **HTTP span** - The incoming HTTP request to the gateway
2. **gRPC client span** - The outgoing call to the backend
3. **gRPC server span** - The backend handling the request

Key observations:

- **Automatic context propagation** - The gateway and backend share the same trace ID
- **Low overhead** - Total added latency is typically < 1ms
- **Standard attributes** - HTTP method, status code, gRPC service/method

### Span Attributes

Click on a span to see its attributes:

```
http.method: GET
http.url: /api/users/123
http.status_code: 200
http.route: /api/users/:id
net.peer.ip: 10.0.0.5
```

## Troubleshooting

### Agent Not Starting

Check the agent logs:

```bash
kubectl logs deployment/rust-gateway -c otel-rust-agent
```

Common issues:

| Error | Solution |
|-------|----------|
| "Process not found" | Verify `OTEL_TARGET_EXE` matches the binary path |
| "Permission denied" | Ensure `privileged: true` is set |
| "BPF program failed to load" | Check kernel version (5.8+ required) |

### No Traces Appearing

1. Verify the OTLP endpoint is correct
2. Check that Jaeger is receiving data: `kubectl logs deployment/jaeger`
3. Ensure the application is receiving traffic

### Kernel Version

Check your kernel version:

```bash
kubectl get nodes -o jsonpath='{.items[*].status.nodeInfo.kernelVersion}'
```

eBPF features require Linux kernel 5.8 or later.

## Next Steps

- **Use with Odigos** - For automatic instrumentation across your entire cluster, consider [Odigos](https://odigos.io)
- **Add manual spans** - Combine with the OpenTelemetry Rust SDK for custom instrumentation
- **Configure sampling** - Set up head-based sampling for high-volume services

## Cleanup

Delete the Kubernetes cluster:

```bash
kind delete cluster
```

