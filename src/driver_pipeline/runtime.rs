use std::path::{Path, PathBuf};

const RUNTIME_C: &str = r#"
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void avera_putc(long c) {
    char ch = (char)c;
    write(1, &ch, 1);
}

void avera_putc_err(int c) {
    char ch = (char)c;
    write(2, &ch, 1);
}

void avera_print_i64(long n) {
    char buf[32];
    int i = 0;
    if (n == 0) { avera_putc('0'); return; }
    int neg = 0;
    unsigned long u;
    if (n < 0) { neg = 1; u = (unsigned long)(-n); }
    else { u = (unsigned long)n; }
    while (u > 0) { buf[i++] = '0' + (char)(u % 10); u /= 10; }
    if (neg) buf[i++] = '-';
    while (i > 0) avera_putc(buf[--i]);
}

void avera_print_u64(unsigned long n) {
    char buf[32];
    int i = 0;
    if (n == 0) { avera_putc('0'); return; }
    while (n > 0) { buf[i++] = '0' + (char)(n % 10); n /= 10; }
    while (i > 0) avera_putc(buf[--i]);
}

void avera_print_f64(double f) {
    /* simple: use snprintf */
    char buf[64];
    snprintf(buf, sizeof(buf), "%g", f);
    for (char *p = buf; *p; p++) avera_putc(*p);
}

/* Print an address in hexadecimal with 0x prefix. */
void avera_print_addr(unsigned long addr) {
    avera_putc('0');
    avera_putc('x');
    if (addr == 0) { avera_putc('0'); return; }
    char buf[32];
    int i = 0;
    while (addr > 0) {
        unsigned long d = addr & 0xF;
        buf[i++] = (char)(d < 10 ? '0' + d : 'a' + d - 10);
        addr >>= 4;
    }
    while (i > 0) avera_putc(buf[--i]);
}

/* Read one line from stdin into a buffer. Returns the length. */
long avera_read_line(char *buf, long max) {
    long n = 0;
    while (n < max - 1) {
        char c;
        ssize_t r = read(0, &c, 1);
        if (r <= 0) break;
        if (c == '\n') break;
        buf[n++] = c;
    }
    buf[n] = 0;
    return n;
}

/* Read one integer from stdin. */
long avera_read_i64(void) {
    char buf[64];
    avera_read_line(buf, sizeof(buf));
    return strtol(buf, NULL, 10);
}

/* Allocate memory (for heap-allocated shapes/lists). */
void *avera_alloc(long size) {
    return calloc(1, (size_t)size);
}

/* Free memory. */
void avera_free(void *ptr) {
    free(ptr);
}

void avera_exit(int code) {
    exit(code);
}

/* Allocate an array of n elements, each `elem_size` bytes. Returns a pointer. */
void *avera_alloc_array(long n, long elem_size) {
    /* Layout: [length: 8 bytes][capacity: 8 bytes][elem0...elemN] */
    /* Give extra capacity so push() works without immediate reallocation. */
    long cap = n * 2 + 8;
    long total = 16 + cap * elem_size;
    char *p = (char *)avera_alloc(total);
    if (p) {
        *(long *)p = n;         /* length */
        *(long *)(p + 8) = cap; /* capacity */
    }
    return p;
}

/* Get array length. */
long avera_array_len(void *arr) {
    if (!arr) return 0;
    return *(long *)arr;
}

/* Get element at index (0-based). Returns pointer to the element, or NULL
 * if the index is out of bounds. Callers should check for NULL; the language
 * bounds-checks by treating a NULL result as index 0 of a sentinel. */
void *avera_array_get(void *arr, long index, long elem_size) {
    if (!arr) return 0;
    long len = *(long *)arr;
    if (index < 0 || index >= len) {
        /* Bounds violation: print a diagnostic and abort. */
        const char *msg = "avera: array index out of bounds\n";
        for (const char *p = msg; *p; p++) avera_putc((long)*p);
        avera_exit(1);
    }
    return (char *)arr + 16 + index * elem_size;
}

/* Set element at index (0-based). Bounds-checked: aborts on violation. */
void avera_array_set(void *arr, long index, long value, long elem_size) {
    if (!arr) return;
    long len = *(long *)arr;
    if (index < 0 || index >= len) {
        const char *msg = "avera: array index out of bounds\n";
        for (const char *p = msg; *p; p++) avera_putc((long)*p);
        avera_exit(1);
    }
    long *slot = (long *)((char *)arr + 16 + index * elem_size);
    *slot = value;
}

/* Get array capacity. */
long avera_array_cap(void *arr) {
    if (!arr) return 0;
    return *(long *)((char *)arr + 8);
}

/* Push a value onto the end of the array, growing if needed. Returns new len. */
long avera_array_push(void *arr_ptr, long value) {
    char *arr = (char *)arr_ptr;
    if (!arr) return 0;
    long len = *(long *)arr;
    long cap = *(long *)(arr + 8);
    if (len >= cap) {
        /* Grow by reallocating. */
        long new_cap = cap * 2 + 8;
        long total = 16 + new_cap * 8;
        char *new_arr = (char *)realloc(arr, total);
        if (!new_arr) return len; /* can't grow */
        *(long *)(new_arr + 8) = new_cap;
        arr = new_arr;
    }
    long *slot = (long *)(arr + 16 + len * 8);
    *slot = value;
    len++;
    *(long *)arr = len;
    return len;
}

/* Pop a value from the end of the array. Returns the popped value (0 if empty). */
long avera_array_pop(void *arr) {
    if (!arr) return 0;
    long len = *(long *)arr;
    if (len == 0) return 0;
    long *slot = (long *)((char *)arr + 16 + (len - 1) * 8);
    long val = *slot;
    *(long *)arr = len - 1;
    return val;
}

/* =========================================================================
 * Text operations. A Text value is represented as a pointer to a byte buffer
 * with the same layout as an array: [len:8][cap:8][byte0...byteN]. Each byte
 * is stored in 8-byte slots for uniformity with arrays (wasteful but simple).
 * ========================================================================= */

/* Allocate a Text value from a length. Returns a pointer. */
void *avera_text_new(long n) {
    return avera_alloc_array(n, 8);
}

/* Get Text length (number of byte slots). */
long avera_text_len(void *t) {
    return avera_array_len(t);
}

/* Get one byte at index (0-based). Returns 0 if out of bounds. */
long avera_text_get(void *t, long index) {
    void *slot = avera_array_get(t, index, 8);
    if (!slot) return 0;
    return *(long *)slot;
}

/* Set one byte at index. */
void avera_text_set(void *t, long index, long value) {
    avera_array_set(t, index, value, 8);
}

/* Concatenate two Text values into a new Text. */
void *avera_text_concat(void *a, void *b) {
    long la = a ? *(long *)a : 0;
    long lb = b ? *(long *)b : 0;
    long total = la + lb;
    void *out = avera_text_new(total);
    if (!out) return 0;
    for (long i = 0; i < la; i++) {
        long v = avera_text_get(a, i);
        avera_text_set(out, i, v);
    }
    for (long i = 0; i < lb; i++) {
        long v = avera_text_get(b, i);
        avera_text_set(out, la + i, v);
    }
    return out;
}

/* Compare two Text values. Returns 1 if equal, 0 otherwise. */
long avera_text_eq(void *a, void *b) {
    long la = a ? *(long *)a : 0;
    long lb = b ? *(long *)b : 0;
    if (la != lb) return 0;
    for (long i = 0; i < la; i++) {
        if (avera_text_get(a, i) != avera_text_get(b, i)) return 0;
    }
    return 1;
}

/* Push one byte onto a Text value. Returns new length. */
long avera_text_push(void *t_ptr, long value) {
    return avera_array_push(t_ptr, value);
}

/* Print a Text value (the bytes) to stdout. */
void avera_text_print(void *t) {
    long len = t ? *(long *)t : 0;
    for (long i = 0; i < len; i++) {
        avera_putc(avera_text_get(t, i));
    }
}

/* Read one line from stdin into a NEW Text value. Returns the Text.
 * `input()` is typed Text (not I64), so it must allocate a real Text
 * object whose bytes can be inspected/printed/concatenated, rather
 * than returning a bare integer. Reads until newline or EOF; the
 * trailing newline is NOT included in the result. */
void *avera_text_read_line(void) {
    /* Read into a growable local buffer first (we don't know the length
     * ahead of time, so we can't pre-size the Text). */
    char stack[256];
    long n = 0;
    while (n < (long)sizeof(stack) - 1) {
        char c;
        ssize_t r = read(0, &c, 1);
        if (r <= 0) break;
        if (c == '\n') break;
        stack[n++] = c;
    }
    /* Build the Text and copy the bytes in. */
    void *t = avera_text_new(n);
    for (long i = 0; i < n; i++) {
        avera_text_set(t, i, (long)(unsigned char)stack[i]);
    }
    return t;
}
"#;

pub fn build_runtime_object(build_dir: &Path) -> PathBuf {
    let rt_path = build_dir.join("avera_runtime.c");
    let obj_path = build_dir.join("avera_runtime.o");
    std::fs::create_dir_all(build_dir).ok();
    // Only (re)write the source if it differs from the embedded RUNTIME_C.
    // Rewriting unconditionally would bump the mtime on every call and force
    // a recompile under parallel test runs, racing the linker.
    let need_write = match std::fs::read_to_string(&rt_path) {
        Ok(existing) => existing != RUNTIME_C,
        Err(_) => true,
    };
    if need_write {
        std::fs::write(&rt_path, RUNTIME_C).ok();
    }
    // Reuse an existing object if it's newer than the source (no recompile).
    if let (Ok(src_meta), Ok(obj_meta)) =
        (std::fs::metadata(&rt_path), std::fs::metadata(&obj_path))
    {
        if let (Ok(src_time), Ok(obj_time)) = (src_meta.modified(), obj_meta.modified()) {
            if obj_time >= src_time {
                return obj_path;
            }
        }
    }
    // Compile to a unique temp file, then atomically rename to the final path.
    // This avoids a race where a parallel build reads a half-written object.
    let pid = std::process::id();
    let tmp_obj = build_dir.join(format!(
        "avera_runtime.{}.{}.o",
        pid,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::process::Command::new("cc")
        .args(["-c", "-O2", "-o"])
        .arg(&tmp_obj)
        .arg(&rt_path)
        .output();
    if !tmp_obj.exists() {
        // Compilation failed; fall back to any existing object.
        return obj_path;
    }
    // Try to atomically install the freshly built object. If a parallel build
    // already installed one (newer than ours, or simply present), keep the
    // existing object and link against our temp copy instead. Either way the
    // caller links a valid, complete object — never a half-written file.
    match std::fs::rename(&tmp_obj, &obj_path) {
        Ok(()) => obj_path,
        Err(_) => tmp_obj, // rename failed (e.g. cross-device); use the temp.
    }
}
