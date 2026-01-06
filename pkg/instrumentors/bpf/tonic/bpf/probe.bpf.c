#include "arguments.h"
#include "span_context.h"
#include "rust_context.h"

char __license[] SEC("license") = "Dual MIT/GPL";

#define MAX_CONCURRENT 50

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __type(key, void*);
    __type(value, struct grpc_request_t);
    __uint(max_entries, MAX_CONCURRENT);
} context_to_grpc_events SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
} grpc_events SEC(".maps");

volatile const u64 service_ptr_pos;
volatile const u64 method_ptr_pos;

SEC("uprobe/tonic_server_serve")
int uprobe_tonic_server_serve(struct pt_regs *ctx) {
    struct grpc_request_t grpcReq = {};
    grpcReq.start_time = bpf_ktime_get_ns();

    void* self_ptr = get_argument(ctx, 1);
    if (!self_ptr) {
        return 0;
    }

    grpcReq.sc = generate_span_context();
    bpf_map_update_elem(&context_to_grpc_events, &self_ptr, &grpcReq, 0);
    bpf_map_update_elem(&spans_in_progress, &self_ptr, &grpcReq.sc, 0);

    return 0;
}

SEC("uprobe/tonic_server_serve_return")
int uprobe_tonic_server_serve_return(struct pt_regs *ctx) {
    void* self_ptr = get_argument_by_stack(ctx, 1);
    if (!self_ptr) {
        return 0;
    }

    void* grpcReq_ptr = bpf_map_lookup_elem(&context_to_grpc_events, &self_ptr);
    if (!grpcReq_ptr) {
        return 0;
    }

    struct grpc_request_t grpcReq = {};
    bpf_probe_read(&grpcReq, sizeof(grpcReq), grpcReq_ptr);
    grpcReq.end_time = bpf_ktime_get_ns();

    bpf_perf_event_output(ctx, &grpc_events, BPF_F_CURRENT_CPU, &grpcReq, sizeof(grpcReq));
    bpf_map_delete_elem(&context_to_grpc_events, &self_ptr);
    bpf_map_delete_elem(&spans_in_progress, &self_ptr);

    return 0;
}

SEC("uprobe/tonic_client_call")
int uprobe_tonic_client_call(struct pt_regs *ctx) {
    struct grpc_request_t grpcReq = {};
    grpcReq.start_time = bpf_ktime_get_ns();

    void* self_ptr = get_argument(ctx, 1);
    if (!self_ptr) {
        return 0;
    }

    void* service_ptr = NULL;
    bpf_probe_read(&service_ptr, sizeof(service_ptr), (void*)(self_ptr + service_ptr_pos));
    if (service_ptr) {
        u64 service_len = 0;
        bpf_probe_read(&service_len, sizeof(service_len), (void*)(self_ptr + service_ptr_pos + 8));
        u64 service_size = sizeof(grpcReq.service);
        service_size = service_size < service_len ? service_size : service_len;
        bpf_probe_read(&grpcReq.service, service_size, service_ptr);
    }

    void* method_ptr = NULL;
    bpf_probe_read(&method_ptr, sizeof(method_ptr), (void*)(self_ptr + method_ptr_pos));
    if (method_ptr) {
        u64 method_len = 0;
        bpf_probe_read(&method_len, sizeof(method_len), (void*)(self_ptr + method_ptr_pos + 8));
        u64 method_size = sizeof(grpcReq.method);
        method_size = method_size < method_len ? method_size : method_len;
        bpf_probe_read(&grpcReq.method, method_size, method_ptr);
    }

    grpcReq.sc = generate_span_context();
    bpf_map_update_elem(&context_to_grpc_events, &self_ptr, &grpcReq, 0);
    bpf_map_update_elem(&spans_in_progress, &self_ptr, &grpcReq.sc, 0);

    return 0;
}

SEC("uprobe/tonic_client_call_return")
int uprobe_tonic_client_call_return(struct pt_regs *ctx) {
    void* self_ptr = get_argument_by_stack(ctx, 1);
    if (!self_ptr) {
        return 0;
    }

    void* grpcReq_ptr = bpf_map_lookup_elem(&context_to_grpc_events, &self_ptr);
    if (!grpcReq_ptr) {
        return 0;
    }

    struct grpc_request_t grpcReq = {};
    bpf_probe_read(&grpcReq, sizeof(grpcReq), grpcReq_ptr);
    grpcReq.end_time = bpf_ktime_get_ns();

    bpf_perf_event_output(ctx, &grpc_events, BPF_F_CURRENT_CPU, &grpcReq, sizeof(grpcReq));
    bpf_map_delete_elem(&context_to_grpc_events, &self_ptr);
    bpf_map_delete_elem(&spans_in_progress, &self_ptr);

    return 0;
}

