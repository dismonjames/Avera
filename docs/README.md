# Avera Compiler

> **Dự án cá nhân - Trình biên dịch Avera**  
> **Tác giả:** Lê Hùng Quang Minh

>[!CAUTION]
>Phục vụ mục đích nghiên cứu, không sử dụng thực tế nếu không chấp nhận rủi ro

> [!NOTE]  
> **Ghi chú về tài liệu (Tiếng Việt & Tiếng Anh):**  
> Vì mình là học sinh lớp 10 bận đi học hàng ngày nên không có nhiều thời gian để dịch và đồng bộ lại toàn bộ tài liệu. Nhiều phần đặc tả kỹ thuật, kiến trúc và bảng giải thích lỗi mình có dùng AI (ChatGPT / Claude) hỗ trợ soạn thảo hộ nên tài liệu sẽ hơi "thập cẩm" Anh - Việt một chút. Nếu có đoạn nào khó hiểu hoặc chưa nhất quán thì mong mọi người thông cảm giúp mình nhé!

Avera là trình biên dịch AOT native cho ngôn ngữ Avera, viết bằng Rust và sử dụng Cranelift backend.

Avera is a small systems language with explicit **ownership**, a **Magnet**
pointer discipline, and a deterministic **MIR** that is checked by dataflow
passes before it is lowered to native machine code. There is no garbage
collector, reference counting, virtual machine, or exception unwinding. Native
executables are linked with a small C runtime and your system linker.

The compiler is a **single Rust crate** (`avera`, library `avera_compiler`)
organized by folder-per-functionality. This repository is the stage-0
compiler: it implements the proven-working core subset described below and
is the foundation the later stages build on.

---

## Highlights

- **AOT native, no VM/JIT/interpreter.** Cranelift produces a real `.o`, which
  is linked with the small C support runtime (`avera_runtime.o`) and your
  system linker.
- **Ownership is checked explicitly.** Forward MIR dataflow tracks locals
  through `Uninit → Init → Moved/Dropped`. Separate MIR borrow and Magnet
  passes enforce the invariants that are actually represented in current MIR.
- **Magnets are real addresses.** `~m = value` attaches a magnet that points
  at a real runtime memory location; `~m.address` yields the live machine
  address.
- **Idempotent formatter.** `avera fmt` parses your source and re-emits it in a
  canonical form; running it again changes nothing (`fmt(fmt(x)) == fmt(x)`).
- **Stable diagnostics.** Errors use stable diagnostic codes and can be
  explained with `avera explain <code>` when an explanation exists.

---

## Quick start

### Prerequisites

- A Rust toolchain (stable, edition 2021; `rust-toolchain.toml` pins it).
- A system C compiler/linker (`cc` or `clang`) on your `PATH` — used to build
  the C runtime object and link the final executable.

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

### Check without codegen

```bash
avera check examples/hello.av
```

`avera check` currently parses source, resolves the stage-0 top-level symbols,
lowers to MIR, validates MIR, and runs the ownership/borrow/Magnet checks. It
**is not yet a complete source-language type checker**; the current lowering
still contains heuristic type guessing for parts of the stage-0 subset.

---

## The `avera` command-line interface

```
USAGE:
    avera <COMMAND> [OPTIONS] [INPUTS...]

COMMANDS:
    check <files...>        Parse, lower, and validate without linking
    build <files...>        Compile and link to a native executable
    run   <file> [args...]  Build then run the program
    test  <dirs...>         Compile and run the test suite
    fmt   <files...>        Format source (use --check to verify)
    version                 Print compiler version
    explain <code>          Explain a diagnostic code
    clean                   Remove build artifacts

OPTIONS:
    -O0/-O1/-O2 | --opt N   Optimization level
    --emit ast|mir|obj       Emit an implemented intermediate form
    --check (with fmt)       Exit non-zero if formatting would change

ENV:
    AVERA_DUMP_AST=1   dump the parsed AST
    AVERA_DUMP_MIR=1   dump the built MIR
    AVERA_DUMP_CLIF=1  dump Cranelift IR

EXIT CODES:
    0  success
    1  user/compile error
    2  compiler-internal error
```

`--emit typed-ast` and `--emit clif` are intentionally rejected by the
stage-0 CLI until those output paths are implemented instead of silently
building a normal executable.

### Intermediate emission

```bash
avera build --emit mir examples/fib.av     # print the MIR for every action
avera build --emit obj examples/fib.av     # print the .o path, no linking
avera build --emit ast examples/fib.av     # pretty-print the parsed AST
```

### Explaining a diagnostic

```bash
avera explain E4001
```

Unknown diagnostic codes return a user error instead of pretending an
explanation exists.

---

## Proven-working feature set

The stage-0 compiler is verified end-to-end against the examples in
`examples/` and the integration/regression tests. The **proven-working**
features are deliberately narrower than the full grammar:

| Area | What works |
|------|-----------|
| Compilation model | AOT native via Cranelift + small C support runtime; no GC, RC, VM, or interpreter |
| Crate | Single Rust crate `avera` (`avera_compiler` lib + `avera` bin) |
| Imports | `#std.io` import directive in the tested stage-0 subset |
| Functions | `action name(params): Type will ... finish` definitions |
| Bindings | `:name = value` (mutable owner), `::name = value` (immutable) |
| Literals | integer, float, bool, char, string |
| Operators | arithmetic `+ - * / %`, comparison `== != < <= > >=`, logical `&& || !`, bitwise `& \| ^ << >>` |
| Control flow | `if`/`else if`/`else`/`finish`, `while`, range `for i in lo..hi`, `loop`, `break`, `continue`, `return expr` |
| I/O | `print(...)`, `write(...)`, `input()` returning `Text` |
| Calls | Stage-0 user function calls, including tested recursion cases |
| Assignment | Direct and compound assignment covered by the regression suite |
| Pattern matching | Integer-literal/wildcard/binding `match` cases in the tested subset |
| Magnets | Real-address attach/read operations plus MIR alias/mutability checks |
| Static checks | MIR validation, ownership dataflow, Magnet dataflow, drop elaboration; borrow checker is MIR infrastructure while source borrow lowering is not yet proven end-to-end |
| Tooling | `avera fmt` canonical formatter; implemented `--emit ast|mir|obj` modes |

`eprint(...)` is parsed, but stderr lowering is **not** currently part of the
proven-working set: stage-0 still routes that path through the stdout print
lowering and it must be fixed before being advertised as stderr output.

See [LANGUAGE.md](LANGUAGE.md) for the full syntax reference,
[OWNERSHIP.md](OWNERSHIP.md) for the ownership model, and
[MAGNET.md](MAGNET.md) for the Magnet system.

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
| `fib.av` | `while` computing Fibonacci, multiple assignment | `fib(10) = 55` |
| `power.av` | `while` computing `2^10` | `2^10 = 1024` |
| `fizzbuzz.av` | `for` + `if`/`else if`/`else`, `%` | `1` `2` `Fizz` … |
| `nested.av` | `for` containing `if`/`else if`/`else` | `zero` `small: 1` … `big: 4` |
| `primes.av` | nested loops and factorization | `2 is prime` … |
| `funcs.av` | two actions, call from `main` | `add(3,4)= 7` |
| `input.av` | `input()` as `Text`, echo and `.len()` | `You entered: ...`, `Length: ...` |
| `match_int.av` | `match` on an int with literal arms and `_` | `other` |
| `magnet.av` | `~m = value`, `~m.address` | `Value: 42` then `Address: 0x…` |
| `magnet2.av` | two magnets with distinct real addresses | `a -> 42 at 0x…` `b -> 999 at 0x…` |

> **Note on `shape.av`:** the `examples/` directory also contains
> `shape.av`, which exercises `shape`/method syntax. The `shape` syntax is
> parsed by the stage-0 front end, but shape methods are not yet claimed as a
> fully verified end-to-end feature.

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
| [STDLIB.md](STDLIB.md) | Standard-library/runtime surface and current implementation notes |
| [DIAGNOSTICS.md](DIAGNOSTICS.md) | Error codes and explanations |
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
    ├── resolve/        # stage-0 top-level definition collection/resolution
    ├── types/          # type interner plus stage-0/legacy type helpers
    ├── mir/            # MIR: bodies, places, rvalues, terminators
    ├── check/          # MIR validation + ownership/borrow/Magnet/drop passes
    ├── driver_pipeline/ # active lowering/runtime/build/run/test/fmt pipeline
    ├── codegen/        # Cranelift backend + object emission + linking
    ├── diagnostics/    # spans, source maps, structured diagnostics, emitter
    ├── fmt/            # canonical source formatter
    ├── module/         # module descriptors and dependency-graph cycle detection
    ├── project/        # avera.toml manifest parsing
    └── symbol.rs       # typed index newtypes and arenas
```

See [COMPILER.md](COMPILER.md) for how the pieces fit together.

---

## Design principles

1. **One crate, one pipeline.** The active compiler pipeline is wired from
   `driver_pipeline/mod.rs`; unused or legacy helper code should not be treated
   as proof that a feature is implemented.
2. **MIR is the current semantic contract.** The AST lowers to explicit basic
   blocks and MIR is validated before Cranelift consumes it.
3. **Deterministic drops.** There is no unwinding; drop elaboration inserts
   deterministic storage/drop operations for the stage-0 model.
4. **Typed IDs, not raw `usize`.** `NodeId`, `LocalId`, `BlockId`, `TypeId`, …
   are distinct newtypes to reduce accidental cross-domain index mistakes.

## License

Licensed under `GPL-3.0-or-later` (see `Cargo.toml`).
