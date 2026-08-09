# Avera Compiler

> **Dự án cá nhân - Trình biên dịch Avera**  
> **Người thực hiện:** Lê Hùng Quang Minh (Học sinh lớp 10) — `lhquangmink@gmail.com`

> [!NOTE]  
> **Ghi chú về tài liệu (Tiếng Việt & Tiếng Anh):**  
> Vì mình là học sinh lớp 10 bận đi học hàng ngày nên không có nhiều thời gian để dịch và đồng bộ lại toàn bộ tài liệu. Nhiều phần đặc tả kỹ thuật, kiến trúc và bảng giải thích lỗi mình có dùng AI (ChatGPT / Claude) hỗ trợ soạn thảo hộ nên tài liệu sẽ hơi "thập cẩm" Anh - Việt một chút. Nếu có đoạn nào khó hiểu hoặc chưa nhất quán thì mong mọi người thông cảm giúp mình nhé!

Avera là trình biên dịch AOT native cho ngôn ngữ Avera, viết bằng Rust và sử dụng Cranelift backend.


Avera is a small systems
language with first-class **ownership**, a **Magnet** pointer discipline,
and a deterministic **MIR** that is checked by dataflow passes before it is
lowered to native machine code. There is no garbage collector, no reference
counting, no virtual machine, and no exception unwinding: every program
compiles to a standalone native executable linked by your system `cc`.

The compiler is a **single Rust crate** (`avera`, library `avera_compiler`)
organized by folder-per-functionality. This repository is the stage-0
compiler: it implements the proven-working core subset described below and
is the foundation the later stages build on.

---

## Highlights

- **AOT, no runtime.** Cranelift codegen produces a real `.o` that is linked
  with a tiny C runtime (`avera_runtime.o`) and your system linker. No VM, no
  JIT, no interpreter.
- **Ownership is checked, not inferred.** A forward dataflow analysis tracks
  every local through `Uninit → Init → Moved/Dropped` and rejects
  use-after-move, use-after-drop, double-drop, and move-while-borrowed.
- **Magnets are real addresses.** `~m = value` attaches a magnet that points
  at a *real runtime memory location*; `~m.address` yields the live machine
  address. This is verified end-to-end, not faked.
- **Idempotent formatter.** `avera fmt` parses your source and re-emits it in a
  canonical form; running it again changes nothing (`fmt(fmt(x)) == fmt(x)`).
- **Stable diagnostics.** Every error has a code (`E0001`–`E9999`) and a
  human explanation via `avera explain <code>`.

---

## Quick start

### Prerequisites

- A Rust toolchain (stable, edition 2021; `rust-toolchain.toml` pins it).
- A system C linker (`cc` or `clang`) on your `PATH` — used to link the
  final executable and to compile the C runtime object.

### Build the compiler

```bash
cargo build            # produces target/debug/avera
cargo build --release  # optimized build (opt-level 2, thin LTO, panic=abort)
```

The binary is `target/debug/avera` (or `target/release/avera`).

### Hello, Avera

`examples/hello.av`:

```av
#std.io

action main(): I32 will
    print("Hello from Avera")
    return 0
finish
```

Compile and run it:

```bash
avera run examples/hello.av
```

Output:

```
Hello from Avera
```

`avera run` builds the program to `build/debug/<name>` and then executes it,
forwarding its exit code. The program `return`s `0`, so the process exits `0`.

### Build only

```bash
avera build examples/hello.av     # links build/debug/hello
./build/debug/hello
```

### Type-check without codegen

```bash
avera check examples/hello.av
```

---

## The `avera` command-line interface

```
USAGE:
    avera <COMMAND> [OPTIONS] [INPUTS...]

COMMANDS:
    check <files...>        Type-check and validate without linking
    build <files...>        Compile and link to a native executable
    run   <file> [args...]  Build then run the program
    test  <dirs...>         Compile and run the test suite
    fmt   <files...>        Format source (use --check to verify)
    version                 Print compiler version
    explain <code>          Explain a diagnostic code
    clean                   Remove build artifacts

OPTIONS:
    -O0/-O1/-O2 | --opt N   Optimization level
    --emit ast|typed-ast|mir|clif|obj   Emit an intermediate form
    --check (with fmt)      Exit non-zero if formatting would change

ENV:
    AVERA_DUMP_AST=1   dump the parsed AST
    AVERA_DUMP_MIR=1   dump the built MIR
    AVERA_DUMP_CLIF=1  dump Cranelift IR

EXIT CODES:
    0  success
    1  user/compile error
    2  compiler-internal error
```

### Intermediate emission

`--emit` lets you inspect each stage of the pipeline:

```bash
avera build --emit mir examples/fib.av     # print the MIR for every action
avera build --emit obj examples/fib.av     # print the .o path, no linking
avera build --emit ast examples/fib.av     # pretty-print the parsed AST
```

### Explaining a diagnostic

```bash
avera explain E4001
```

```
E4001: A value was used after its ownership was moved. Create another value,
borrow it, or change the action so it does not take ownership.
```

---

## Proven-working feature set

The stage-0 compiler is verified end-to-end against the examples in
`examples/` (see [examples/](#the-examples-directory) below). The
**proven-working** features are:

| Area | What works |
|------|-----------|
| Compilation model | AOT native via Cranelift; no GC, no RC, no VM, no exceptions |
| Crate | Single Rust crate `avera` (`avera_compiler` lib + `avera` bin), folder-per-functionality |
| Imports | `#std.io` import directive |
| Functions | `action name(params): Type will ... finish` definitions |
| Bindings | `:name = value` (mutable owner), `::name = value` (immutable) |
| Literals | integer, float, bool, char, string |
| Operators | arithmetic `+ - * / %`, comparison `== != < <= > >=`, logical `&& || !`, bitwise `& \| ^ << >>` |
| Control flow | `if`/`else if`/`else`/`finish`, `while`, `for i in lo..hi`, `loop`, `break`, `continue`, `return expr` |
| I/O | `print(...)`, `write(...)`, `eprint(...)`, `input()` |
| Calls | User function calls, including mutual recursion |
| Assignment | Compound assignment `+= -= *= /= %= &= |= ^= <<=` (and more) |
| Pattern matching | `match scrutinee will pat => ... finish` with integer literals, wildcards, and bindings |
| Magnets | `~m = value` creates a magnet; `~m.address` returns a real runtime memory address |
| Static checks | MIR validation, ownership checker (dataflow), borrow checker, magnet checker, drop elaboration |
| Tooling | `avera fmt` idempotent source formatter |

See [LANGUAGE.md](LANGUAGE.md) for the full syntax reference,
[OWNERSHIP.md](OWNERSHIP.md) for the ownership model, and
[MAGNET.md](MAGNET.md) for the magnet system.

---

## The examples directory

The `examples/` directory contains self-contained programs. Each one
`#std.io`-imports the standard I/O prelude and defines `action main(): I32`.

| Example | Demonstrates | Sample output |
|---------|-------------|---------------|
| `hello.av` | `print` of a string | `Hello from Avera` |
| `arithmetic.av` | a helper `action`, integer add, `print` mixing string + int | `sum: 30` |
| `calls.av` | nested calls `sum_of_squares(3,4)` → `square(3)+square(4)` | `sum_of_squares(3, 4) = 25` |
| `cond.av` | full `if`/`else if`/`else` chain, nested `if` | `5 cold` … `35 hot` |
| `ifelse.av` | `if`/`else if`/`else` with a score | `B` |
| `while.av` | `while` loop with `+=` | `sum 0..9: 45` |
| `forloop.av` | `for i in 0..10`, re-binding `:sum` | `sum 0..10: 45` |
| `sumto.av` | `for i in 1..101` (exclusive upper bound), `+=` | `sum 1..100 = 5050` |
| `fib.av` | `while` computing Fibonacci, multiple `+=`/`=` | `fib(10) = 55` |
| `power.av` | `while` computing `2^10` via `result += result` | `2^10 = 1024` |
| `fizzbuzz.av` | `for` + `if`/`else if`/`else`, `%`, `print` of int or string | `1` `2` `Fizz` … |
| `nested.av` | `for` containing `if`/`else if`/`else` | `zero` `small: 1` … `big: 4` |
| `primes.av` | nested `for` + `while`, factorization | `2 is prime` … `20 = 2 * 10` |
| `funcs.av` | two actions, call from `main` | `add(3,4)= 7` |
| `input.av` | `input()` reading an int from stdin, `print` of `n + n` | (reads `42`) `Doubled: 84` |
| `match_int.av` | `match` on an int with literal arms and `_` | `other` |
| `magnet.av` | `~m = value`, `~m.address` | `Value: 42` then `Address: 0x…` |
| `magnet2.av` | two magnets, real distinct addresses 8 bytes apart | `a -> 42 at 0x…` `b -> 999 at 0x…` |

> **Note on `shape.av`:** the `examples/` directory also contains
> `shape.av`, which exercises `shape`/method syntax. The `shape` *syntax*
> is parsed by the stage-0 front end (see [grammar.md](grammar.md)), but the
> v0.1 proven-working backend subset is the list above; `shape`-with-`self`
> methods are not yet end-to-end and `shape.av` is not part of the verified
> set.

### Running the whole suite

```bash
avera test examples/
```

This compiles and runs every `.av` file in the directory and prints a
pass/fail summary. A test "passes" when it compiles and exits `0`.

---

## Documentation index

| Document | Contents |
|----------|---------|
| [LANGUAGE.md](LANGUAGE.md) | Language reference: syntax, types, expressions, statements, operators |
| [OWNERSHIP.md](OWNERSHIP.md) | Ownership model: binding rules, move semantics, the ownership checker |
| [MAGNET.md](MAGNET.md) | Magnet system: `~m` syntax, `.address`/`.offset`, the proof addresses are real |
| [MIR.md](MIR.md) | MIR design: basic blocks, places, rvalues, terminators, dataflow checkers |
| [COMPILER.md](COMPILER.md) | Compiler architecture: pipeline and crate structure |
| [STDLIB.md](STDLIB.md) | Standard library: `print`/`write`/`eprint`/`input`, runtime functions |
| [DIAGNOSTICS.md](DIAGNOSTICS.md) | Error codes `E0001`–`E9999` with explanations |
| [grammar.md](grammar.md) | EBNF grammar for the full language |

---

## Project layout

```
Avera/
├── Cargo.toml          # single crate: bin `avera` + lib `avera_compiler`
├── build.rs            # embeds the version used by `avera version`
├── rust-toolchain.toml # stable Rust
├── examples/           # self-contained .av programs
├── docs/               # this documentation
└── src/
    ├── main.rs         # CLI entry point
    ├── cli.rs          # argument parsing and dispatch
    ├── lib.rs          # library root + diagnostic explanations
    ├── lexer/          # tokenizer + token definitions
    ├── parser/         # recursive-descent + Pratt expression parser
    ├── ast/            # AST node types and a visitor
    ├── resolve/        # name resolution, scopes, def table
    ├── types/          # type interner, lowering, layout
    ├── mir/            # MIR: bodies, places, rvalues, terminators
    ├── check/          # MIR validation + ownership/borrow/magnet/drop checkers
    ├── driver_pipeline/ # the real pipeline: lower, runtime, build/run/test/fmt
    ├── codegen/        # Cranelift backend + object emission + linking
    ├── diagnostics/    # spans, source maps, structured diagnostics, emitter
    ├── fmt/            # idempotent source formatter
    ├── module/         # module descriptors and dependency-graph cycle detection
    ├── project/        # avera.toml manifest parsing
    └── symbol.rs       # typed index newtypes and arenas
```

See [COMPILER.md](COMPILER.md) for how the pieces fit together.

---

## Design principles

1. **One crate, one pipeline.** No proc-macro scaffolding, no plugin system,
   no separate driver crate. Every phase lives in `src/` and is wired by
   `driver_pipeline/mod.rs`.
2. **MIR is the contract.** The AST lowers to a small, explicit MIR
   (`Body` of `BasicBlock`s of `Stmt`s ending in a `Terminator`). All
   semantic checks run on MIR, and MIR is what Cranelift consumes.
3. **Deterministic drops.** There is no unwinding, so drop points are
   statically known. The drop-elaboration pass inserts `StorageDead`
   markers at `Return` terminators; the ownership checker's state machine
   consumes them.
4. **Typed IDs, not raw `usize`.** `NodeId`, `LocalId`, `BlockId`,
   `TypeId`, … are distinct newtypes so two phases cannot accidentally swap
   a local index for a block index.

## License

Dual-licensed under `MIT OR Apache-2.0` (see `Cargo.toml`).
