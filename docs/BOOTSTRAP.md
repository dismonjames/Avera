# Bootstrap and self-hosting roadmap

Avera is currently a **stage-0** compiler: the compiler itself is written in
Rust, and it compiles Avera source to native code via Cranelift. The
long-term goal is a **self-hosting** compiler — an Avera compiler written in
Avera itself. This document records the staged plan, the prerequisites, and
the concrete feature gaps that must close before each stage becomes
possible.

## Terminology

| Term | Meaning |
|------|---------|
| **Stage-0** | The Rust compiler (`src/`, this crate) that produces native Avera executables. Already complete for the v0.1 language. |
| **Stage-1** | A Avera compiler written in Avera that targets native code, but is itself compiled by stage-0. The first self-host proof. |
| **Stage-2** | A Avera compiler written in Avera and compiled by stage-1. This is the "second-generation" build; once it compiles itself, the language is self-hosting. |
| **Bootstrap** | The act of crossing from one stage to the next: compile stage-N+1 with stage-N. |
| **Self-check** | Stage-2 rebuilding itself byte-for-byte (or at least producing equivalent behaviour). This is the end-of-road property. |

## Why bootstrap matters

A self-hosting compiler is the strongest possible demonstration that the
language is usable for serious programs: the compiler is one of the most
demanding applications one can write, exercising every feature (parsing,
algebraic data types, pattern matching, heap allocation, I/O, FFI). Once
stage-2 can build itself, the language has paid its own bill.

## The pipeline today (stage-0)

```
.av source ─► [Rust: lex → parse → AST → resolve → MIR → check → Cranelift] ─► .o ─► executable
                                                                          ▲
                                                           avera_runtime.o (C) ─┘
```

Stage-0 is Rust + a small C runtime (`avera_runtime.c`) for I/O and memory.
It is the **seed** from which self-hosting grows.

## The pipeline we want (stage-2)

```
.av source ─► [Avera: lex → parse → AST → resolve → MIR → check → Cranelift] ─► .o ─► executable
                            (written in Avera itself, compiled by stage-1)
```

Stage-2 has no Rust and no C runtime beyond what Avera itself generates or
imports via FFI.

## Prerequisites (feature gaps to close before stage-1)

Stage-0 must be capable of compiling a realistic program before we can
write a compiler in Avera. The gaps below are the blockers, ordered by how
much they'd block a stage-1 attempt.

### P0 — must close before stage-1 can begin

1. **Algebraic data types with full destructuring.** Choice types exist
   (`.some`, `.none`) and pattern-match on the tag, but a real compiler's
   AST/MIR needs rich enums (e.g. `Expr = Num | Binop | Call | ...`) with
   payloads of mixed arity and recursive `match`. Close: generalise the
   choice variant payload layout and the `match` lowering to support
   multi-field, named-field variants.
2. **Recursive types via pointers/Magnet.** An AST node referencing itself
   (`Expr` contains `Box<Expr>`) needs indirection. Magnet is the Avera
   answer: a recursive `shape` is fine as long as the recursive field is a
   `~Magnet<...>` or heap pointer. Verify the resolver and ownership
   checker accept recursive shapes through magnets.
3. **Heap arrays of arbitrary types (not just I64).** The current array
   layout is `[len:8][cap:8][slot:8]...` — every element is 8 bytes. A
   compiler holds arrays of AST nodes, tokens, etc. Decide on a uniform
   boxed representation (every value is a heap pointer) for stage-1, or
   parameterise arrays by element size at the type level.
4. **Strings as first-class Text with an Avera-written `print`/`read`.** Today
   `print` is a built-in lowered directly to `avera_putc` calls. For
   self-hosting, the runtime's `avera_*` functions must remain, but the
   *front-end conveniences* (formatting integers, string building) must be
   writable in Avera itself. Text is already there; need `avera_text_from_i64`
   and similar in the runtime.
5. **A real type-checker.** Stage-0 type-checking is currently a stub
   (`check` succeeds on parse). Before writing a compiler in Avera we need
   the type-checker to reject bad programs and infer types, otherwise the
   stage-1 source won't be checkable by stage-0.

### P1 — strongly needed for stage-1 to be ergonomic

6. **Closures or function pointers.** A compiler's visitor pattern wants
   closures; without them, stage-1 must thread explicit context structs.
   Function pointers (via FFI or a first-class `Action` type) would
   suffice as a v0.1.
7. **Module visibility and qualified imports.** Stage-1 is a multi-file
   project; `#std.io` style imports need to resolve across directories and
   respect privacy. The module graph (`src/module/`) exists but is mostly
   a header scanner.
8. **Better error recovery in the parser.** A self-host compiler is its
   own worst test case: when the stage-1 Avera source has a syntax error,
   stage-0 must report it clearly. Current parser recovery is minimal.
9. **A stdlib with real contents.** `std.core`, `std.io` are declarations
   only. Stage-1 will need `std.collections` (a `Vec`, a `HashMap`), a
   `std.fs` for reading files, and a `std.os` for `argv`/`exit`.

### P2 — nice to have, can defer

10. **Generics / type parameters.** A compiler loves `Vec<Token>` over
    `VecToken`-of-I64. v0.1 is monomorphic; generics can be added
    monomorphisation-style (like Rust's MIR) later.
11. **Trait-like dispatch.** The visitor pattern benefits from it, but
    explicit `match` on a choice type is enough for a first cut.
12. **Optimisation passes.** Stage-0 emits unoptimised Cranelift IR. A
    self-host compiler doesn't *need* optimisation to bootstrap; it needs
    correctness.

## The bootstrap procedure (concrete steps)

Once the P0 gaps are closed, self-hosting proceeds as follows:

### Step 1 — write `stage1.av`

Implement a minimal Avera compiler **in Avera** that can compile a tiny
subset of Avera (enough to compile `hello.av`). It will live under
`bootstrap/stage1/` and use:
- `#std.io` for `print`/`read`.
- FFI to Cranelift's C API (via `#link`/raw blocks) for codegen, OR emit
  assembly text directly and shell out to `cc`. Emitting assembly is far
  simpler and avoids linking against Cranelift from Avera.

The minimal stage-1 compiler needs: a lexer, a Pratt parser, a tiny AST,
a straight-line codegen (no optimisation), and `print` for diagnostics.

### Step 2 — stage-0 compiles stage-1

```
avera build bootstrap/stage1/stage1.av   # stage-0 builds stage-1
./build/debug/stage1 bootstrap/hello.av # stage-1 compiles hello
```

If `stage1` produces a working `hello`, **stage-1 exists**. This is the
"first light" milestone.

### Step 3 — write `stage2.av` = stage-1 + more features

Extend stage-1 to cover the full v0.1 grammar (loops, shapes, choices,
magnets, arrays, text, match, FFI). Call this `stage2.av`. It must be
written in the subset of Avera that stage-1 can compile.

### Step 4 — stage-1 compiles stage-2

```
./build/debug/stage1 bootstrap/stage2/stage2.av   # stage-1 builds stage-2
./stage2 bootstrap/stage2/stage2.av               # stage-2 builds itself
```

When stage-2 successfully compiles itself, **Avera is self-hosting**. The
Rust stage-0 compiler is no longer required to use the language.

### Step 5 — self-check (optional, ideal)

Freeze stage-2's source. Build it with stage-1, then build it again with
the freshly-built stage-2. The two resulting executables should be
byte-identical (deterministic codegen) or at least behave identically on
a fixed test corpus. This is the "standing on its own shoulders" proof.

## What stage-0 must keep until stage-2 lands

Stage-0 remains the reference until stage-2 self-checks. Do **not** delete
or stop maintaining stage-0 while stage-1/2 are incomplete — it is the
only thing that can compile them. Stage-0's role during the transition:

- Compile stage-1 and stage-2.
- Serve as the correctness oracle: any disagreement between stage-0 and
  stage-N on a test program is a bug in stage-N (stage-0 is trusted).
- Provide the Cranelift codegen until stage-N emits its own assembly.

## Current status (as of this commit)

- Stage-0 is complete for the v0.1 language surface: control flow,
  shapes, choices, magnets, arrays, text, FFI, modules.
- **No stage-1 exists yet.** The P0 blockers above (real type-checker,
  recursive types, richer ADTs, first-class strings) are the next work.
- The 37-test suite and 28 examples are the stage-0 correctness oracle.

## See also

- [COMPILER.md](COMPILER.md) — stage-0 architecture and stage map.
- [LANGUAGE.md](LANGUAGE.md) — the v0.1 language the compiler implements.
- [MIR.md](MIR.md) — the intermediate representation stage-1 must emit.
