// Minimal freestanding libc for lwext4 that the linker wants.
#include <stdint.h>
#include <stddef.h>

#define HEAP_START 0x08000000u   // 128MB: above the vmlinuz buffer (1MB+), below initramfs (256MB)
#define HEAP_SIZE  0x01000000u   // 16MB

static uintptr_t heap_ptr = HEAP_START;

void* memcpy(void *dest, const void *src, size_t n);
void* memset(void *s, int c, size_t n);

void* malloc(size_t n) {
    n = (n + 15) & ~(size_t)15;
    // 16-byte header keeps returned pointers 16-byte aligned and stores the size.
    if (heap_ptr + 16 + n > HEAP_START + HEAP_SIZE) return NULL;
    uint32_t *hdr = (uint32_t*)heap_ptr;
    hdr[0] = (uint32_t)n;
    heap_ptr += 16 + n;
    return hdr + 4;
}

void free(void *p) { (void)p; }

void* calloc(size_t cnt, size_t sz) {
    size_t total = cnt * sz;
    void *p = malloc(total);
    if (p) memset(p, 0, total);
    return p;
}

void* realloc(void *p, size_t n) {
    if (!p) return malloc(n);
    uint32_t old = ((uint32_t*)p)[-4];
    if (n <= old) return p;
    void *q = malloc(n);
    if (q) memcpy(q, p, old);
    return q;
}

int memcmp(const void *a, const void *b, size_t n) {
    const uint8_t *x = a, *y = b;
    for (size_t i = 0; i < n; i++)
        if (x[i] != y[i]) return x[i] - y[i];
    return 0;
}

void* memmove(void *dest, const void *src, size_t n) {
    uint8_t *d = dest;
    const uint8_t *s = src;
    if (d < s) { for (size_t i = 0; i < n; i++) d[i] = s[i]; }
    else       { for (size_t i = n; i > 0; i--) d[i-1] = s[i-1]; }
    return dest;
}

int strcmp(const char *a, const char *b) {
    while (*a && *a == *b) { a++; b++; }
    return (uint8_t)*a - (uint8_t)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (a[i] != b[i]) return (uint8_t)a[i] - (uint8_t)b[i];
        if (!a[i]) return 0;
    }
    return 0;
}

char* strcpy(char *d, const char *s) {
    char *r = d;
    while ((*d++ = *s++)) {}
    return r;
}

char* strncpy(char *d, const char *s, size_t n) {
    size_t i = 0;
    for (; i < n && s[i]; i++) d[i] = s[i];
    for (; i < n; i++) d[i] = 0;
    return d;
}
