#ifndef __RUST_CONTEXT_H__
#define __RUST_CONTEXT_H__

#include "common.h"
#include "span_context.h"

#define MAX_CONCURRENT_REQUESTS 50

struct http_request_t {
    u64 start_time;
    u64 end_time;
    char method[MAX_METHOD_SIZE];
    char path[MAX_PATH_SIZE];
    u16 status_code;
    struct span_context sc;
};

struct grpc_request_t {
    u64 start_time;
    u64 end_time;
    char service[MAX_PATH_SIZE];
    char method[MAX_METHOD_SIZE];
    u32 status_code;
    struct span_context sc;
};

static __always_inline void* get_argument_system_v(struct pt_regs *ctx, int pos) {
    switch (pos) {
        case 1: return (void*)PT_REGS_PARM1(ctx);
        case 2: return (void*)PT_REGS_PARM2(ctx);
        case 3: return (void*)PT_REGS_PARM3(ctx);
        case 4: return (void*)PT_REGS_PARM4(ctx);
        case 5: return (void*)PT_REGS_PARM5(ctx);
        case 6: return (void*)PT_REGS_PARM6(ctx);
        default: return NULL;
    }
}

static __always_inline void* get_return_value(struct pt_regs *ctx) {
    return (void*)PT_REGS_RC(ctx);
}

#endif /* __RUST_CONTEXT_H__ */

