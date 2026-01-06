#include "arguments.h"
#include "span_context.h"
#include "rust_context.h"

char __license[] SEC("license") = "Dual MIT/GPL";

#define MAX_CONCURRENT 50

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __type(key, void*);
    __type(value, struct http_request_t);
    __uint(max_entries, MAX_CONCURRENT);
} context_to_http_events SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
} events SEC(".maps");

volatile const u64 method_ptr_pos;
volatile const u64 uri_ptr_pos;
volatile const u64 path_ptr_pos;

SEC("uprobe/hyper_serve_connection")
int uprobe_hyper_serve_connection(struct pt_regs *ctx) {
    struct http_request_t httpReq = {};
    httpReq.start_time = bpf_ktime_get_ns();

    void* self_ptr = get_argument(ctx, 1);
    if (!self_ptr) {
        return 0;
    }

    httpReq.sc = generate_span_context();
    bpf_map_update_elem(&context_to_http_events, &self_ptr, &httpReq, 0);
    bpf_map_update_elem(&spans_in_progress, &self_ptr, &httpReq.sc, 0);

    return 0;
}

SEC("uprobe/hyper_serve_connection_return")
int uprobe_hyper_serve_connection_return(struct pt_regs *ctx) {
    void* self_ptr = get_argument_by_stack(ctx, 1);
    if (!self_ptr) {
        return 0;
    }

    void* httpReq_ptr = bpf_map_lookup_elem(&context_to_http_events, &self_ptr);
    if (!httpReq_ptr) {
        return 0;
    }

    struct http_request_t httpReq = {};
    bpf_probe_read(&httpReq, sizeof(httpReq), httpReq_ptr);
    httpReq.end_time = bpf_ktime_get_ns();

    bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, &httpReq, sizeof(httpReq));
    bpf_map_delete_elem(&context_to_http_events, &self_ptr);
    bpf_map_delete_elem(&spans_in_progress, &self_ptr);

    return 0;
}

SEC("uprobe/hyper_request_method")
int uprobe_hyper_request_method(struct pt_regs *ctx) {
    void* request_ptr = get_argument(ctx, 1);
    if (!request_ptr) {
        return 0;
    }

    void* method_ptr = NULL;
    bpf_probe_read(&method_ptr, sizeof(method_ptr), (void*)(request_ptr + method_ptr_pos));
    if (!method_ptr) {
        return 0;
    }

    void* httpReq_ptr = bpf_map_lookup_elem(&context_to_http_events, &request_ptr);
    if (!httpReq_ptr) {
        return 0;
    }

    struct http_request_t httpReq = {};
    bpf_probe_read(&httpReq, sizeof(httpReq), httpReq_ptr);

    u64 method_len = 0;
    bpf_probe_read(&method_len, sizeof(method_len), (void*)(request_ptr + method_ptr_pos + 8));
    u64 method_size = sizeof(httpReq.method);
    method_size = method_size < method_len ? method_size : method_len;
    bpf_probe_read(&httpReq.method, method_size, method_ptr);

    bpf_map_update_elem(&context_to_http_events, &request_ptr, &httpReq, 0);

    return 0;
}

SEC("uprobe/hyper_request_uri")
int uprobe_hyper_request_uri(struct pt_regs *ctx) {
    void* request_ptr = get_argument(ctx, 1);
    if (!request_ptr) {
        return 0;
    }

    void* uri_ptr = NULL;
    bpf_probe_read(&uri_ptr, sizeof(uri_ptr), (void*)(request_ptr + uri_ptr_pos));
    if (!uri_ptr) {
        return 0;
    }

    void* httpReq_ptr = bpf_map_lookup_elem(&context_to_http_events, &request_ptr);
    if (!httpReq_ptr) {
        return 0;
    }

    struct http_request_t httpReq = {};
    bpf_probe_read(&httpReq, sizeof(httpReq), httpReq_ptr);

    void* path_ptr = NULL;
    bpf_probe_read(&path_ptr, sizeof(path_ptr), (void*)(uri_ptr + path_ptr_pos));

    u64 path_len = 0;
    bpf_probe_read(&path_len, sizeof(path_len), (void*)(uri_ptr + path_ptr_pos + 8));
    u64 path_size = sizeof(httpReq.path);
    path_size = path_size < path_len ? path_size : path_len;
    bpf_probe_read(&httpReq.path, path_size, path_ptr);

    bpf_map_update_elem(&context_to_http_events, &request_ptr, &httpReq, 0);

    return 0;
}

