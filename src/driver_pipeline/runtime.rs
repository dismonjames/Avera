use std::path::{Path, PathBuf};

const RUNTIME_C: &str = r#"
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <limits.h>

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
    int neg = n < 0;
    unsigned long u;
    if (neg) {
        /* Avoid signed overflow for LONG_MIN. */
        u = (unsigned long)(-(n + 1));
        u += 1;
    } else {
        u = (unsigned long)n;
    }
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
    char buf[64];
    snprintf(buf, sizeof(buf), "%g", f);
    for (char *p = buf; *p; p++) avera_putc(*p);
}

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

long avera_read_i64(void) {
    char buf[64];
    avera_read_line(buf, sizeof(buf));
    return strtol(buf, NULL, 10);
}

typedef struct AveraAllocHeader {
    uint64_t magic;
    void *aux;
} AveraAllocHeader;

#define AVERA_ALLOC_MAGIC UINT64_C(0x41564552414D454D)

static AveraAllocHeader *avera_alloc_header(void *ptr) {
    if (!ptr) return NULL;
    return ((AveraAllocHeader *)ptr) - 1;
}

static void avera_set_aux(void *ptr, void *aux) {
    AveraAllocHeader *header = avera_alloc_header(ptr);
    if (header && header->magic == AVERA_ALLOC_MAGIC) header->aux = aux;
}

void *avera_alloc(long size) {
    if (size < 0) return NULL;
    size_t n = (size_t)size;
    if (n > SIZE_MAX - sizeof(AveraAllocHeader)) return NULL;
    AveraAllocHeader *header =
        (AveraAllocHeader *)calloc(1, sizeof(AveraAllocHeader) + n);
    if (!header) return NULL;
    header->magic = AVERA_ALLOC_MAGIC;
    header->aux = NULL;
    return (void *)(header + 1);
}

void avera_free(void *ptr) {
    if (!ptr) return;
    AveraAllocHeader *header = avera_alloc_header(ptr);
    if (header->magic != AVERA_ALLOC_MAGIC) return;
    free(header->aux);
    header->aux = NULL;
    header->magic = 0;
    free(header);
}

void avera_exit(int code) {
    exit(code);
}

typedef struct AveraArray {
    long len;
    long cap;
    long elem_size;
    unsigned char *data;
} AveraArray;

static int avera_array_bytes(long cap, long elem_size, size_t *out) {
    if (cap < 0 || elem_size <= 0) return 0;
    if ((unsigned long)cap > SIZE_MAX / (unsigned long)elem_size) return 0;
    *out = (size_t)cap * (size_t)elem_size;
    return 1;
}

void *avera_alloc_array(long n, long elem_size) {
    if (n < 0 || elem_size <= 0) return NULL;
    if (n > (LONG_MAX - 8) / 2) return NULL;
    long cap = n * 2 + 8;
    size_t bytes = 0;
    if (!avera_array_bytes(cap, elem_size, &bytes)) return NULL;
    AveraArray *arr = (AveraArray *)avera_alloc((long)sizeof(AveraArray));
    if (!arr) return NULL;
    arr->data = (unsigned char *)calloc(1, bytes);
    if (!arr->data) {
        avera_free(arr);
        return NULL;
    }
    arr->len = n;
    arr->cap = cap;
    arr->elem_size = elem_size;
    avera_set_aux(arr, arr->data);
    return arr;
}

long avera_array_len(void *arr_ptr) {
    AveraArray *arr = (AveraArray *)arr_ptr;
    return arr ? arr->len : 0;
}

void *avera_array_get(void *arr_ptr, long index, long elem_size) {
    AveraArray *arr = (AveraArray *)arr_ptr;
    if (!arr) return NULL;
    if (elem_size != arr->elem_size) {
        const char *msg = "avera: array element size mismatch\n";
        for (const char *p = msg; *p; p++) avera_putc((long)*p);
        avera_exit(1);
    }
    if (index < 0 || index >= arr->len) {
        const char *msg = "avera: array index out of bounds\n";
        for (const char *p = msg; *p; p++) avera_putc((long)*p);
        avera_exit(1);
    }
    return arr->data + (size_t)index * (size_t)arr->elem_size;
}

long avera_array_set(void *arr_ptr, long index, long value, long elem_size) {
    AveraArray *arr = (AveraArray *)arr_ptr;
    if (!arr) return 0;
    if (elem_size != arr->elem_size || elem_size != 8) {
        const char *msg = "avera: unsupported array element layout\n";
        for (const char *p = msg; *p; p++) avera_putc((long)*p);
        avera_exit(1);
    }
    if (index < 0 || index >= arr->len) {
        const char *msg = "avera: array index out of bounds\n";
        for (const char *p = msg; *p; p++) avera_putc((long)*p);
        avera_exit(1);
    }
    long *slot = (long *)(arr->data + (size_t)index * 8);
    *slot = value;
    return (long)(intptr_t)arr_ptr;
}

long avera_array_cap(void *arr_ptr) {
    AveraArray *arr = (AveraArray *)arr_ptr;
    return arr ? arr->cap : 0;
}

long avera_array_push(void *arr_ptr, long value) {
    AveraArray *arr = (AveraArray *)arr_ptr;
    if (!arr) return 0;
    if (arr->elem_size != 8) return arr->len;
    if (arr->len >= arr->cap) {
        if (arr->cap > (LONG_MAX - 8) / 2) return arr->len;
        long new_cap = arr->cap * 2 + 8;
        size_t bytes = 0;
        if (!avera_array_bytes(new_cap, arr->elem_size, &bytes)) return arr->len;
        unsigned char *new_data = (unsigned char *)realloc(arr->data, bytes);
        if (!new_data) return arr->len;
        if (new_cap > arr->cap) {
            size_t old_bytes = (size_t)arr->cap * (size_t)arr->elem_size;
            memset(new_data + old_bytes, 0, bytes - old_bytes);
        }
        arr->data = new_data;
        arr->cap = new_cap;
        avera_set_aux(arr, new_data);
    }
    long *slot = (long *)(arr->data + (size_t)arr->len * 8);
    *slot = value;
    arr->len++;
    return arr->len;
}

long avera_array_pop(void *arr_ptr) {
    AveraArray *arr = (AveraArray *)arr_ptr;
    if (!arr || arr->len == 0 || arr->elem_size != 8) return 0;
    long *slot = (long *)(arr->data + (size_t)(arr->len - 1) * 8);
    long value = *slot;
    *slot = 0;
    arr->len--;
    return value;
}

void *avera_text_new(long n) {
    return avera_alloc_array(n, 8);
}

long avera_text_len(void *t) {
    return avera_array_len(t);
}

long avera_text_get(void *t, long index) {
    void *slot = avera_array_get(t, index, 8);
    return slot ? *(long *)slot : 0;
}

long avera_text_set(void *t, long index, long value) {
    return avera_array_set(t, index, value, 8);
}

void *avera_text_concat(void *a, void *b) {
    long la = avera_text_len(a);
    long lb = avera_text_len(b);
    if (la > LONG_MAX - lb) return NULL;
    long total = la + lb;
    void *out = avera_text_new(total);
    if (!out) return NULL;
    for (long i = 0; i < la; i++) avera_text_set(out, i, avera_text_get(a, i));
    for (long i = 0; i < lb; i++) avera_text_set(out, la + i, avera_text_get(b, i));
    return out;
}

long avera_text_eq(void *a, void *b) {
    long la = avera_text_len(a);
    long lb = avera_text_len(b);
    if (la != lb) return 0;
    for (long i = 0; i < la; i++) {
        if (avera_text_get(a, i) != avera_text_get(b, i)) return 0;
    }
    return 1;
}

long avera_text_push(void *t_ptr, long value) {
    return avera_array_push(t_ptr, value);
}

long avera_text_print(void *t) {
    long len = avera_text_len(t);
    for (long i = 0; i < len; i++) avera_putc(avera_text_get(t, i));
    return 0;
}

void *avera_text_read_line(void) {
    void *t = avera_text_new(0);
    if (!t) return NULL;
    for (;;) {
        char c;
        ssize_t r = read(0, &c, 1);
        if (r <= 0 || c == '\n') break;
        long before = avera_text_len(t);
        long after = avera_text_push(t, (long)(unsigned char)c);
        if (after == before) break;
    }
    return t;
}
"#;

pub fn build_runtime_object(build_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(build_dir).map_err(|e| {
        format!(
            "cannot create runtime build directory `{}`: {e}",
            build_dir.display()
        )
    })?;

    let rt_path = build_dir.join("avera_runtime.c");
    let obj_path = build_dir.join("avera_runtime.o");
    let need_write = match std::fs::read_to_string(&rt_path) {
        Ok(existing) => existing != RUNTIME_C,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => return Err(format!("cannot read `{}`: {e}", rt_path.display())),
    };
    if need_write {
        std::fs::write(&rt_path, RUNTIME_C)
            .map_err(|e| format!("cannot write `{}`: {e}", rt_path.display()))?;
    }

    if let (Ok(src_meta), Ok(obj_meta)) =
        (std::fs::metadata(&rt_path), std::fs::metadata(&obj_path))
    {
        if let (Ok(src_time), Ok(obj_time)) = (src_meta.modified(), obj_meta.modified()) {
            if obj_time >= src_time && obj_meta.len() > 0 {
                return Ok(obj_path);
            }
        }
    }

    let pid = std::process::id();
    let tmp_obj = build_dir.join(format!(
        "avera_runtime.{}.{}.o",
        pid,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let cc = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let output = std::process::Command::new(&cc)
        .args(["-c", "-O2", "-o"])
        .arg(&tmp_obj)
        .arg(&rt_path)
        .output()
        .map_err(|e| format!("failed to execute C compiler `{:?}`: {e}", cc))?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&tmp_obj);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "C runtime compilation failed with {}:\n{}",
            output.status,
            stderr.trim()
        ));
    }
    let meta = std::fs::metadata(&tmp_obj).map_err(|e| {
        format!(
            "C compiler reported success but runtime object `{}` is missing: {e}",
            tmp_obj.display()
        )
    })?;
    if meta.len() == 0 {
        let _ = std::fs::remove_file(&tmp_obj);
        return Err("C compiler produced an empty Avera runtime object".to_string());
    }

    match std::fs::rename(&tmp_obj, &obj_path) {
        Ok(()) => Ok(obj_path),
        Err(rename_err) => {
            if tmp_obj.is_file() {
                // Another build may have won the race installing obj_path.
                // The unique temp object is complete and safe to link.
                Ok(tmp_obj)
            } else {
                Err(format!(
                    "cannot install runtime object `{}`: {rename_err}",
                    obj_path.display()
                ))
            }
        }
    }
}
