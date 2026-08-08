# Standard Library

Avera's stage-0 standard library is intentionally tiny. The `#std.io` import
brings four I/O built-ins into scope, and the compiled program is linked
against a small C runtime (`avera_runtime.o`) that provides the foreign
functions the backend calls. There is no GC, no allocator-backed
collections, and no prelude of macros — the runtime is just enough to
print, read, allocate, and exit.

> Runtime source: [`src/driver_pipeline/runtime.rs`](../src/driver_pipeline/runtime.rs).
> Lowering of the built-ins: [`src/driver_pipeline/lower.rs`](../src/driver_pipeline/lower.rs).

---

## 1. Importing the standard library

```av
#std.io
```

This directive (parsed by `src/parser/directive.rs`) imports the
`std.io` module, which makes the I/O built-ins available as plain
function calls. Every example in `examples/` begins with this line.

The built-in module set recognised by the module system
(`src/module/mod.rs`, `STD_MODULES`) is:

```
std.core, std.memory, std.collections, std.io, std.fs,
std.process, std.os, std.text, std.hash, std.format
```

In stage-0 only `std.io` is wired to actual functions; the others are
declared for the module graph and dependency-cycle detection.

---

## 2. I/O built-ins

These are recognised by name in the lowerer (`lower_expr_into`'s `Call`
case) and translated directly to runtime calls. They are not user-defined
actions and have no first-class function value.

### 2.1 `print`

```av
print(arg, arg, ...)
```

- Prints each argument, separated by a single space, followed by a
  trailing newline (`\n`).
- Writes to **stdout**.
- Arguments may be any mix of strings and integers; booleans print as
  `true`/`false`; chars print as one byte; floats print via `%g`.

Lowering (`lower_print`):

- For each argument after the first, a space character is printed via
  `avera_putc(' ')`.
- Each argument is printed by `lower_print_arg` (see §2.5).
- A final newline is printed via `avera_putc('\n')`.

`examples/hello.av`:

```av
print("Hello from Avera")
```

```
Hello from Avera
```

`examples/arithmetic.av`:

```av
print("sum:", z)        // z = 30
```

```
sum: 30
```

### 2.2 `write`

```av
write(arg, arg, ...)
```

- Same as `print` but **without** the trailing newline.
- Writes to stdout.

`write` is useful when you want to compose output on one line across
several statements.

### 2.3 `eprint`

```av
eprint(arg, arg, ...)
```

- Prints each argument (no space separator between args in stage-0)
  followed by a trailing newline.
- Writes to **stderr**.

Lowering (`lower_print_eprint`): each arg via `lower_print_arg`, then a
final `avera_putc('\n')`. (The stage-0 eprint path calls `avera_putc`; the
runtime also provides `avera_putc_err` for writing the byte to fd 2.)

### 2.4 `input`

```av
:n = input()
```

- Reads **one integer** from **stdin** and returns it.
- Implemented by a call to the runtime `avera_read_i64`, which reads a line
  into a buffer and parses it with `strtol`.

Lowering: a `Stmt::Call { callee: "avera_read_i64", args: [], ret_ty: I64 }`.

`examples/input.av`:

```av
action main(): I32 will
    print("Enter a number:")
    :n = input()
    print("You entered:", n)
    print("Doubled:", n + n)
    return 0
finish
```

With `42` typed on stdin:

```
Enter a number:
You entered: 42
Doubled: 84
```

### 2.5 How each argument is printed

`lower_print_arg` (`src/driver_pipeline/lower.rs`) dispatches on the
literal kind:

| Argument | Lowering | Runtime call |
|----------|----------|--------------|
| string literal `"…"` | emit one `avera_putc` per byte | `avera_putc` |
| integer literal `42` | `Const(Int)` then print | `avera_print_i64` |
| float literal `3.14` | `Const(Float)` then print | `avera_print_f64` |
| bool literal `true`/`false` | print the string `"true"`/`"false"` | `avera_putc` |
| char literal `'a'` | `Const(U64)` | `avera_putc` |
| any other expression | evaluate into a temp, then print | `avera_print_i64` |

So `print("add(3,4)=", r)` prints the string `add(3,4)=` byte-by-byte,
then a space, then `r` via `avera_print_i64`, then a newline.

### 2.6 `panic`

```av
panic("message")
```

Lowers to a `Terminator::Abort`, which the backend compiles to a Cranelift
`trap` (user trap code `0`). There is no unwinding; the process aborts.

---

## 3. The C runtime (`avera_runtime.o`)

The runtime is a single C file written to `build/debug/avera_runtime.c` and
compiled with `cc -c -O2` to `build/debug/avera_runtime.o`, then linked
alongside the Cranelift-generated object. It uses only POSIX
`unistd.h`, `stdio.h`, `stdlib.h`, `string.h`.

### 3.1 Function reference

| Function | Signature | Behaviour |
|----------|-----------|-----------|
| `avera_putc` | `void avera_putc(long c)` | write one byte `(char)c` to fd 1 (stdout) |
| `avera_putc_err` | `void avera_putc_err(int c)` | write one byte to fd 2 (stderr) |
| `avera_print_i64` | `void avera_print_i64(long n)` | print a signed 64-bit integer in decimal |
| `avera_print_u64` | `void avera_print_u64(unsigned long n)` | print an unsigned 64-bit integer in decimal |
| `avera_print_f64` | `void avera_print_f64(double f)` | print a double via `snprintf("%g", …)` |
| `avera_print_addr` | `void avera_print_addr(unsigned long addr)` | print an address in hex with `0x` prefix |
| `avera_read_line` | `long avera_read_line(char *buf, long max)` | read one line from fd 0 into `buf`; return length |
| `avera_read_i64` | `long avera_read_i64(void)` | read a line and parse it with `strtol` |
| `avera_alloc` | `void *avera_alloc(long size)` | `calloc(1, size)` — zeroed heap allocation |
| `avera_free` | `void avera_free(void *ptr)` | `free(ptr)` |
| `avera_exit` | `void avera_exit(int code)` | `exit(code)` |

### 3.2 Declaration in the backend

`src/codegen/cranelift.rs` declares all of these with
`Linkage::Import` before compiling any user body, so calls resolve to
`FuncRef`s. The signatures (`runtime_sig`) match the C prototypes:

```rust
"avera_putc"       => I64 -> ()
"avera_print_i64"  => I64 -> ()
"avera_print_f64"  => F64 -> ()
"avera_print_u64"  => I64 -> ()
"avera_print_addr" => I64 -> ()
"avera_read_i64"   () -> I64
"avera_alloc"      => I64 -> I64
"avera_free"       => I64 -> ()
"avera_exit"       => I64 -> ()
```

### 3.3 Integer printing algorithm

`avera_print_i64` (from `RUNTIME_C`) handles the sign and the digits
least-significant-first into a stack buffer, then writes them in reverse:

```c
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
```

`avera_print_addr` is the same idea in base 16 with a `0x` prefix.

---

## 4. The module graph

`src/module/mod.rs` defines:

- `ModuleDescriptor` — a `.mav` file's `#module`, `#header`,
  `#source`, `#depends`, `#cfg`, `#link` directives.
- `ModuleGraph` — logical module name → `LoadedModule`.
- `detect_cycles` — a DFS over the `#depends` edges that returns the
  first cycle found (reported as `E6005`).
- `STD_MODULES` — the list of built-in modules above.
- `import_to_module_name` — turns an import directive path into a
  module name (`std.fs.File` → `std.fs`).

A dependency cycle in the module graph is a compile error.

---

## 5. Project manifests

`src/project/mod.rs` parses a `avera.toml`:

```toml
[project]
name = "myapp"
version = "0.1.0"
entry = "src/main.av"

[build]
target = "native"
opt = 2

[dependencies]
libc = { path = "../libc" }
```

`find_project_root` walks up from a starting path looking for
`avera.toml`. This is the scaffolding for multi-file builds beyond the
single-file stage-0 `avera build <file>` form.

---

## 6. Extending the standard library

Because the backend imports any external symbol you declare, adding a
new runtime function is a three-step change:

1. Add the C implementation to `RUNTIME_C` in
   `src/driver_pipeline/runtime.rs`.
2. Add a `runtime_sig` arm for it in `src/codegen/cranelift.rs` and
   include it in the import-declaration list.
3. Lower the call to it (either as a special-case name in the lowerer, or
   via a `foreign` block once that path is wired end-to-end).

The function is then available to compiled programs with no other
plumbing.
