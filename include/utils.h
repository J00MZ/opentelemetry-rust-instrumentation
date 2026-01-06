#ifndef __UTILS_H__
#define __UTILS_H__

#include "common.h"

static __always_inline void generate_random_bytes(unsigned char *buff, u32 size) {
    for (u32 i = 0; i < size; i++) {
        buff[i] = bpf_get_prandom_u32() & 0xFF;
    }
}

static __always_inline char hex_char(u8 value) {
    if (value < 10) {
        return '0' + value;
    }
    return 'a' + (value - 10);
}

static __always_inline void bytes_to_hex_string(unsigned char *bytes, u32 size, char *out) {
    for (u32 i = 0; i < size; i++) {
        out[i * 2] = hex_char((bytes[i] >> 4) & 0x0F);
        out[i * 2 + 1] = hex_char(bytes[i] & 0x0F);
    }
}

static __always_inline u8 hex_to_byte(char c) {
    if (c >= '0' && c <= '9') {
        return c - '0';
    }
    if (c >= 'a' && c <= 'f') {
        return c - 'a' + 10;
    }
    if (c >= 'A' && c <= 'F') {
        return c - 'A' + 10;
    }
    return 0;
}

static __always_inline void hex_string_to_bytes(char *hex, u32 hex_len, unsigned char *out) {
    for (u32 i = 0; i < hex_len / 2; i++) {
        out[i] = (hex_to_byte(hex[i * 2]) << 4) | hex_to_byte(hex[i * 2 + 1]);
    }
}

#endif /* __UTILS_H__ */

