#ifndef __COMMON_H__
#define __COMMON_H__

#include "libbpf/bpf_helpers.h"
#include "libbpf/bpf_tracing.h"
#include "libbpf/bpf_core_read.h"

#define TRACE_ID_SIZE 16
#define SPAN_ID_SIZE 8
#define TRACE_ID_STRING_SIZE 32
#define SPAN_ID_STRING_SIZE 16

#define MAX_PATH_SIZE 256
#define MAX_METHOD_SIZE 16
#define MAX_HEADER_SIZE 256

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef signed char s8;
typedef signed short s16;
typedef signed int s32;
typedef signed long long s64;

#endif /* __COMMON_H__ */

