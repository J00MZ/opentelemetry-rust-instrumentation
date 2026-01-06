#ifndef __ARGUMENTS_H__
#define __ARGUMENTS_H__

#include "common.h"

static __always_inline void* get_argument(struct pt_regs *ctx, int pos) {
#if defined(__x86_64__)
    switch (pos) {
        case 1: return (void*)PT_REGS_PARM1(ctx);
        case 2: return (void*)PT_REGS_PARM2(ctx);
        case 3: return (void*)PT_REGS_PARM3(ctx);
        case 4: return (void*)PT_REGS_PARM4(ctx);
        case 5: return (void*)PT_REGS_PARM5(ctx);
        case 6: return (void*)PT_REGS_PARM6(ctx);
        default: return NULL;
    }
#elif defined(__aarch64__)
    switch (pos) {
        case 1: return (void*)ctx->regs[0];
        case 2: return (void*)ctx->regs[1];
        case 3: return (void*)ctx->regs[2];
        case 4: return (void*)ctx->regs[3];
        case 5: return (void*)ctx->regs[4];
        case 6: return (void*)ctx->regs[5];
        case 7: return (void*)ctx->regs[6];
        case 8: return (void*)ctx->regs[7];
        default: return NULL;
    }
#else
#error "Unsupported architecture"
#endif
}

static __always_inline void* get_argument_by_stack(struct pt_regs *ctx, int pos) {
    void* ptr = NULL;
    u64 sp = PT_REGS_SP(ctx);
    bpf_probe_read(&ptr, sizeof(ptr), (void*)(sp + (pos * 8)));
    return ptr;
}

#endif /* __ARGUMENTS_H__ */

