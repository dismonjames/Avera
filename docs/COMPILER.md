# Compiler Architecture

`avera` is a single Rust crate that compiles Avera source to a native
executable. This document describes the compilation pipeline, the crate
structure, and where each phase lives.

> Crate: `Cargo.toml` defines one binary (`avera`, `src/main.rs`) and one
> library (`avera_compiler`, `src/lib.rs`).

---

## 1. The pipeline

```text
source → lexer → parser → resolver → type lowering → MIR lowering
       → MIR validation → ownership/borrow/magnet checks → drop elaboration
       → Cranelift codegen → object file → system linker → executable
```

Each stage is a folder under `src/`, exposing its pieces through a
`mod.rs`. The driver wires them together in
[`src/driver_pipeline/mod.rs`](../src/driver_pipeline/mod.rs).

### 1.1 Stage map

| Stage | Folder | Key types | Output |
|-------|--------|-----------|--------|
| Lex | `src/lexer/` | `Lexer`, `Token`, `TokenKind`, `Keyword` | `Vec<Token>` |
| Parse | `src/parser/` | `Parser`, `parse` | `Parsed` (`Module`) |
| AST | `src/ast/` | `Module`, `Item`, `Stmt`, `Expr`, `Ty`, `Pat` | AST nodes |
| Resolve | `src/resolve/` | `Defs`, `DefInfo`, `ScopeTable` | def table |
| Types | `src/types/` | `TyCtxt`, `TyData`, `TyId`, `Abilities` | interned types |
| MIR | `src/mir/` | `Body`, `BasicBlock`, `Stmt`, `Rvalue`, `Terminator` | MIR bodies |
| Check | `src/check/` | `validate_mir`, `check_ownership`, `check_borrow`, `check_magnet`, `elaborate_drops` | diagnostics |
| Lower (AST→MIR) | `src/driver_pipeline/lower.rs` | `Lowerer` | `Vec<Body>` |
| Codegen | `src/codegen/` | `compile_bodies`, `write_object`, `link_executable_with_rt` | `.o`, executable |
| Runtime | `src/driver_pipeline/runtime.rs` | `avera_putc`, `avera_print_i64`, … | `avera_runtime.o` |
| Diagnostics | `src/diagnostics/` | `Diagnostic`, `DiagnosticList`, `Emitter`, `Span`, `SourceMap` | error report |
| Fmt | `src/fmt/` | `format_module` | canonical source |
| Module | `src/module/` | `ModuleDescriptor`, `ModuleGraph` | module graph |
| Project | `src/project/` | `Manifest` | `avera.toml` parse |
| Symbols | `src/symbol.rs` | `NodeId`, `LocalId`, `BlockId`, …, `Arena` | typed IDs |

### 1.2 End-to-end flow of `avera build`

`build` in [`src/driver_pipeline/mod.rs`](../src/driver_pipeline/mod.rs):

1. **`load_and_parse`** — for each input file: read, `map.load`, lex,
   parse. Accumulate diagnostics and modules.
2. If any errors, report and stop.
3. **Lower** — for each module, `lower::lower_module` produces MIR bodies
   for every action with a body. AST/MIR dumps honour `AVERA_DUMP_AST` /
   `AVERA_DUMP_MIR`.
4. **Check** — for each body: `validate_mir`, `validate_return_consistency`,
   `check_ownership`, `check_borrow`, `check_magnet`. If errors, report
   and stop.
5. **Drop elaboration** — `elaborate_drops` inserts `StorageDead` at
   `Return` points.
6. **`--emit` shortcuts** — `--emit ast` / `--emit mir` print and return;
   `--emit obj` stops after writing the object file.
7. **Codegen** — `codegen_obj` builds an `ObjectModule`, compiles all
   bodies, writes `<name>.o` to `build/debug/`.
8. **Runtime** — `runtime::build_runtime_object` writes
   `avera_runtime.c` and compiles it with `cc -c -O2` to `avera_runtime.o`.
9. **Link** — `object::link_executable_with_rt` links `<name>.o` +
   `avera_runtime.o` with `cc -o build/debug/<name>`.

### 1.3 End-to-end flow of `avera run`

`run` calls `build` then executes `build/debug/<name>` and returns the
process exit code.

### 1.4 `avera check`

Lex + parse only (no lower, no MIR, no codegen). Verifies the source
parses cleanly. Stage-0 type-checking is a stub that succeeds on parse.

### 1.5 `avera fmt`

For each input: read, lex, parse, `format_module`, and either write back
or compare (`--check`). It round-trips through the AST, so anything that
does not parse cannot be formatted.

### 1.6 `avera test`

For each input (file or directory of `.av` files): `avera run` it; a test
passes if it compiles and exits `0`. Prints a `passed/failed` summary.

---

## 2. Lexing

[`src/lexer/lexer.rs`](../src/lexer/lexer.rs) is a hand-written byte scanner.

- Skips whitespace and comments (`//` line, `/* */` block — non-nesting).
- Tracks `bracket_depth` so newlines inside `()` / `[]` are *not*
  significant. A pending newline is flushed as a `Newline` token only at
  depth 0, after a "significant" token that does not continue a line
  (operator/punct whose `continues_line()` is true suppresses the newline).
- Lexes identifiers/keywords, decimal/hex/binary integers with `_`
  separators, floats with optional exponent, string and char literals
  with escapes (`\n \t \r \\ \" \' \0 \xHH \u{...}`).
- Two-char tokens (`::`, `&!`, `:>`, `->`, `=>`, `==`, `!=`, `<=`, `>=`,
  `&&`, `||`, `<<`, `>>`, `+=`, `-=`, `*=`, `/=`, `%=`, `|=`, `&=`, `^=`,
  `~!`, `..`) are matched before single-char tokens.
- Emits `Eof` at the end.

Tokens are `Token { kind: TokenKind, span: Span }` where `Span` is a
half-open byte range plus a `FileId`.

---

## 3. Parsing

[`src/parser/`](../src/parser/) is a recursive-descent parser with a Pratt
sub-parser for expressions.

- `mod.rs` — `Parser` cursor helpers (`at`, `eat`, `expect`, `eat_separator`),
  top-level loop, and recovery (`synchronize_top` skips to the next
  declaration boundary).
- `directive.rs` — `#…` import and module directives, including `as`
  aliases and wildcard `*`.
- `item.rs` — items: `shape`, `choice`, `ability`, `implement`,
  `foreign`, and `action` (with optional `export` and `@attr`s). Action
  signatures support `Type.method` receiver syntax and parameter modes.
- `stmt.rs` — statements: bindings (`:`, `::`, `~`, `~!`), `if`/`else
  if`/`else`, `while`, `loop`, `for`, `match`, `return`, `break`,
  `continue`, `raw`, assignment/compound-assignment, magnet retarget
  `->`, early drop `!`, and expression statements.
- `expr.rs` — Pratt binary parser using `BinOp::precedence`; unary prefix
  `! - & &! ^ ~`; postfix `.field`/`.method()`/`[i]`/`[a..b]`/`(args)`/
  `:> Ty`/`?`; primary literals/paths/`self`/`.variant`/arrays/parens.
- `pat.rs` — patterns for `match` and `for`: variant `.name(binds)`,
  no-payload `.name`, bind `name`, wildcard `_`, integer literal `42`.
- `ty.rs` — type expressions: named (with generic args), `&T`/`&!T`,
  `Magnet<T>`, `Address<T>`, `[T; N]`, `Span<T>`, `Maybe<T>`,
  `Outcome<T, E>`, `Self`, `Unit`, `_`, tuples. Specialises the common
  constructors so `Magnet<User>` etc. become structured `TyKind`s.

---

## 4. AST

[`src/ast/`](../src/ast/) holds the parse tree. Highlights:

- `Module { span, directives, items }`.
- `Item { span, kind: ItemKind, attrs, exported }` with `ItemKind` =
  `Shape | Choice | Ability | Impl | Action | Foreign`.
- `Stmt { span, kind: StmtKind }` covering every statement form above.
- `Expr { span, kind: ExprKind }` with `BinOp`/`UnOp` and the precedence
  table (`BinOp::precedence`).
- `Ty { span, kind: TyKind }`.
- `Pat { span, kind: PatKind }`.
- `visit.rs` — a generic `Visit` trait with `walk_*` functions for
  traversing modules/items/stmts/exprs/pats/types.

Every node carries its source `Span`.

---

## 5. Resolution and types

- `src/resolve/` — `Defs` is the global definition table
  (`Vec<DefInfo>` + `by_name` map). `resolve_module` populates it from a
  module's items and collects imports. `ScopeTable` is the lexical
  value-scope stack (`enter`/`leave`/`define`/`lookup`).
- `src/types/intern.rs` — `TyCtxt` interns `TyData` and pre-interns all
  primitives (`cx.i32`, `cx.bool`, …). Caches per-type `Abilities`.
- `src/types/lower.rs` — `TyLowerer` turns AST `Ty` into `TyId`s, mapping
  named primitives and user shapes/choices, and evaluating `[T; N]`
  constant sizes.
- `src/types/ty.rs` — `TyData`, `Abilities`, `DefKind` (shape/choice/
  ability), and `Defs` (resolved field/variant info).

---

## 6. MIR lowering

[`src/driver_pipeline/lower.rs`](../src/driver_pipeline/lower.rs) —
`Lowerer` walks an action's body and builds a `Body`:

- Allocates params as locals (in order), then lowers the block.
- `new_block_id` pre-allocates placeholder blocks so jump targets are
  known before they are filled (ids == indices).
- `lower_stmt` handles each statement kind, emitting `Assign`s and
  `Call`s and creating the right terminators (`SwitchInt` for `if`,
  `Goto` loops for `while`/`for`/`loop`, `SwitchInt`/`Switch` for
  `match`).
- `lower_expr_into` evaluates an expression into a destination local:
  literals → `Const`; paths → `Use`; binaries → `BinOp`; `~m.address` →
  `MagnetAddress`; `~m = v` → `MagnetAttach`; calls to `print`/`write`/
  `eprint`/`input`/`panic` are special-cased; other calls become `Call`.
- `lower_print` emits per-character `avera_putc` calls for strings, with
  space separators between args and a trailing newline for `print`/
  `eprint` (not `write`).
- A `loop_ctx` stack of `(header, exit)` block ids implements `break`/
  `continue`.
- `finish` rewrites any leftover `Unreachable` terminator to `Return`.

### Type guessing

The lowerer uses a small `guess_ty` heuristic (int literals → `I32`,
floats → `F64`, comparisons → `Bool`, else `I32`) to pick `BinOp` result
types; the Cranelift backend represents all scalar locals as `I64` (or
`F64` for floats), so the exact guessed type only affects signedness of
comparisons and shifts.

---

## 7. Checking

See [MIR.md](MIR.md) for the checkers' dataflow details. The order, run
once per body before drop elaboration, is:

1. `validate_mir` — structural invariants (block ids, local range,
   target range).
2. `validate_return_consistency` — non-`Unit` bodies return a value.
3. `check_ownership` — full inter-block fixpoint over `Uninit/Init/
   Moved/Dropped` ([OWNERSHIP.md](OWNERSHIP.md)).
4. `check_borrow` — per-block shared/exclusive borrow tracking.
5. `check_magnet` — per-block magnet attachment tracking
   ([MAGNET.md](MAGNET.md)).

Then `elaborate_drops` inserts `StorageDead` at `Return` points.

---

## 8. Code generation

[`src/codegen/`](../src/codegen/) lowers MIR to a native object file via
Cranelift.

### 8.1 ABI

`src/codegen/abi.rs` maps a `TyId` to its Cranelift scalar type: `I8`/
`I16`/`I32`/`I64`/`F32`/`F64`, with borrows/magnets/spans/addresses as
`I64`. Aggregates return `None` (passed by stack slot).

### 8.2 Layout

`src/codegen/layout.rs` computes `Layout { size, align, scalar }`:
natural alignment, scalar types in a register, aggregates by summing
field layouts with `align_up`, choices as `max_payload + tag`. This is
what shape/choice codegen uses once those land.

### 8.3 Cranelift compilation

`src/codegen/cranelift.rs`:

1. **Declare** every user action (`Linkage::Export`) and every runtime
   function (`avera_putc`, `avera_print_i64`, `avera_print_f64`, `avera_print_u64`,
   `avera_print_addr`, `avera_read_i64`, `avera_alloc`, `avera_free`, `avera_exit`)
   with `Linkage::Import`. This two-pass scheme is what makes **mutual
   recursion** work: all signatures are known before any body compiles.
2. **Compile** each body:
   - One Cranelift `Variable` per MIR local, all `I64` (or `F64`).
   - One Cranelift `Block` per MIR `BlockId`, created up front.
   - Block 0 is sealed immediately so its entry params can be bound to
     the function's incoming argument values; all other blocks are
     sealed at the end once every predecessor is known.
   - `lower_stmt` emits `def_var`/`use_var` for assignments; `lower_rvalue`
     handles `Use`/`Const`/`BinOp`/`UnOp`/`Cast`/`MagnetAttach`/
     `MagnetAddress`/`Call`; `lower_terminator` emits `jump`/`brif`/
     `return_`/`trap`.
   - `MagnetAttach` allocates a real stack slot (`create_sized_stack_slot`
     + `stack_addr` + `store`) — see [MAGNET.md](MAGNET.md).
   - `binop` picks signed/unsigned `IntCC` or `FloatCC` from the operand
     type and normalises comparison results to `I64` via `select`.
3. `module.define_function` finalises; on failure it dumps the Cranelift
   IR to stderr for debugging.

### 8.4 Object emission and linking

`src/codegen/object.rs`:

- `make_object_module` builds a host-target `ObjectModule`.
- `write_object` calls `module.finish()` and writes `<name>.o`.
- `link_executable_with_rt` links `<name>.o` + `avera_runtime.o` with `cc`
  (or `clang`) to `build/debug/<name>`.

---

## 9. Runtime

[`src/driver_pipeline/runtime.rs`](../src/driver_pipeline/runtime.rs)
embeds a small C runtime (`RUNTIME_C`) and compiles it to
`build/debug/avera_runtime.o` with `cc -c -O2`. It provides the functions
the backend imports: `avera_putc`, `avera_putc_err`, `avera_print_i64`,
`avera_print_u64`, `avera_print_f64`, `avera_print_addr`, `avera_read_line`,
`avera_read_i64`, `avera_alloc`, `avera_free`, `avera_exit`. Full detail is in
[STDLIB.md](STDLIB.md).

---

## 10. Diagnostics

`src/diagnostics/`:

- `span.rs` — `Span` (file + byte range), `SourceFile` (text +
  line_starts), `SourceMap` (arena of files), `LineCol`.
- `diagnostic.rs` — `Diagnostic { severity, kind, message, span, related,
  suggestion }`, `DiagnosticKind` (the stable `E*`/`W*` codes),
  `DiagnosticList` sink.
- `emitter.rs` — renders a diagnostic as a header line, `--> file:line:col`,
  the source line, and an underline of the span, with optional ANSI
  colour and a summary `N error(s)` / `N warning(s)` line.

`avera explain <code>` looks up the human explanation in
`diagnostics_explanations` ([`src/lib.rs`](../src/lib.rs)); see
[DIAGNOSTICS.md](DIAGNOSTICS.md).

---

## 11. Exit codes

Defined in [`src/cli.rs`](../src/cli.rs) and reused by the driver:

| Code | Meaning |
|------|---------|
| `0` | success |
| `1` | user/compile error (lex/parse/check/codegen failure) |
| `2` | compiler-internal error (invariant violation, `E9999`) |

`avera run` returns the *program's* exit code on success.

---

## 12. Typed IDs

[`src/symbol.rs`](../src/symbol.rs) defines index newtypes via a
`define_id!` macro: `NodeId`, `TypeId`, `SymbolId`, `LocalId`,
`BlockId`, `PlaceId`, `FieldId`, `VariantId`, `AbilityId`, `ModuleId`,
`FileId`, `TempId`. Each is a distinct `u32`-backed type so two phases
cannot accidentally swap, e.g., a `LocalId` for a `BlockId`. The same
file provides `Arena<I, T>` — a `Vec<T>` indexed by a typed `Idx` — and
the `Idx` trait. `placeholder()` (`u32::MAX`) marks "no value".
