# Diagnostics

Every diagnostic `avera` emits carries a stable code (`E0001`–`E9999` for
errors, `W0001` for warnings). This document catalogues the codes, gives
the explanation printed by `avera explain <code>`, lists where each is
produced, and suggests a fix.

> Codes and explanations are defined in two places that must stay in sync:
> [`src/diagnostics/diagnostic.rs`](../src/diagnostics/diagnostic.rs)
> (`DiagnosticKind::code()`) maps each internal kind to its code, and
> [`src/lib.rs`](../src/lib.rs) (`diagnostics_explanations`) maps each
> code to its human text for `avera explain`.

## Using the diagnostics

### Reading an error

```
error: expected `=` but found `self`
  --> examples/shape.av:9:11
   |
 9 |     :self = User(name, age)
   |           ^ expected `=` but found `self`
```

- `error` / `warning` — severity.
- `--> file:line:col` — 1-based location.
- the source line and an underline of the span.
- optional `note`/`suggestion` lines and `-->` related-location lines.

### `avera explain`

```bash
avera explain E4001
```

```
E4001: A value was used after its ownership was moved. Create another value,
borrow it, or change the action so it does not take ownership.
```

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | success |
| `1` | one or more errors were emitted |
| `2` | compiler-internal error (`E9999`) |

---

## Code catalogue

Codes are grouped by the phase/category that defines them.

### Lexing and parsing

| Code | Kind | Explanation |
|------|------|-------------|
| `E0001` | `Lex` | The lexer could not tokenize this character. Check for stray punctuation or invalid UTF-8. |
| `E0100` | `Parse` | The parser expected a different token. This is a syntax error; the snippet shows where parsing failed. |

### MIR validation

These are emitted by `validate_mir` / `validate_return_consistency`
(`src/check/validate.rs`). They indicate a malformed MIR body; with a
correct front end they should be unreachable from user code, but they
guard against internal inconsistencies.

| Code | Kind | Explanation |
|------|------|-------------|
| `E1100` | `MirValidation` | Generic MIR validation failure (e.g. a non-`Unit` action `return`s without a value). |
| `E1101` | `MirEmptyBody` | A MIR body has no basic blocks. |
| `E1102` | `MirInvalidEntry` | The entry block is not block 0. |
| `E1103` | `MirBlockIdMismatch` | A block's id does not match its index in the blocks vector. |
| `E1104` | `MirInvalidLocal` | A `Place` references a local id out of range. |
| `E1105` | `MirInvalidTarget` | A `Goto`/`Switch`/`SwitchInt` target is out of range. |

> The `E1100`–`E1105` codes do not have an entry in
> `diagnostics_explanations`, so `avera explain` reports "no explanation
> available" for them; the table above is the reference.

### Names and declarations

| Code | Kind | Explanation | Produced by |
|------|------|-------------|-------------|
| `E2001` | `EUndefinedName` | A name was used that is not defined in this scope. Make sure it is bound, declared, or imported. | resolver, type lowerer |
| `E2002` | `EDuplicate` | A declaration or binding with the same name already exists in this scope. | resolver (`resolve_item`) |

### Types

| Code | Kind | Explanation |
|------|------|-------------|
| `E3001` | `ETypeMismatch` | Two types that should match do not. Add a conversion (`:>`) or change one of the types. |
| `E5003` | `ENarrowing` | An integer literal does not fit in the target type, or an implicit narrowing conversion was attempted. Use `:>` to convert explicitly. |

### Ownership, borrows, magnets

These are the core dataflow errors from `src/check/`. The lifetime
lifecycle is described in [OWNERSHIP.md](OWNERSHIP.md); magnets in
[MAGNET.md](MAGNET.md).

| Code | Kind | Explanation | Produced by |
|------|------|-------------|-------------|
| `E4001` | `EUseAfterMove` | A value was used after its ownership was moved. Create another value, borrow it, or change the action so it does not take ownership. | ownership checker |
| `E4002` | `EUseAfterDrop` | A value was used after being dropped (explicitly or at scope end). Drop points are deterministic in Avera. | ownership checker |
| `E4003` | `EDoubleDrop` | A value would be dropped twice. The compiler inserts drops automatically; do not call `!` on an already-dropped binding. | ownership checker |
| `E4004` | `EMoveWhileBorrowed` | A value was moved while a borrow or Magnet still depended on it. Detach the Magnet or let the borrow expire first. | ownership/borrow/magnet checkers |
| `E4005` | `EMutableAlias` | Two mutable accesses to the same place overlap. Mutable access is exclusive in Avera. | borrow checker |
| `E4006` | `EImmutableMutate` | An immutable binding was mutated, or a borrow escaped its owner's lifetime. (Also used for `EBorrowEscape`.) | borrow checker |
| `E4007` | `EAddrFromInt` | An integer was converted to an address in safe code. Use a `raw` block for this operation. | magnet/raw check |
| `E4008` | `EMagnetOob` / `EMagnetDetached` | Magnet arithmetic went out of the span, or a detached/non-attached magnet was queried. Use `raw` to bypass the check or adjust the offset. | magnet checker |
| `E4009` | `ERelocation` / `EMagnetUseAfterMove` | A container was resized while a Magnet pointed inside it, or a magnet's target moved/dropped. End the Magnet lifetime first. | magnet checker |

### Pattern matching and shapes

| Code | Kind | Explanation |
|------|------|-------------|
| `E5001` | `ENonExhaustive` | The match did not cover all variants. Add a wildcard arm or handle every variant of the choice. |
| `E5002` | `EInfiniteSize` | A shape contains itself by value, leading to infinite size. Use `Box`, `Magnet`, or `Maybe`. |

### Modules and imports

| Code | Kind | Explanation | Produced by |
|------|------|-------------|-------------|
| `E6001` | `EPrivateAccess` | A private symbol was accessed. Export it in the `.mav` header or use a different symbol. | module system |
| `E6002` | `EHeaderMismatch` | The implementation in `.av` does not match the signature declared in `.mav`. | module system |
| `E6003` | `EAmbiguousImport` | An imported symbol is ambiguous. Use an alias (`as`) to disambiguate. | module system |
| `E6004` | `EMissingModule` | A module referenced by an import could not be found. (Also used when a source file cannot be read.) | module system / driver |
| `E6005` | `EDepCycle` | A dependency cycle was detected in the module graph. | `detect_cycles` |

### Resources and ABI

| Code | Kind | Explanation |
|------|------|-------------|
| `E7001` | `EImplicitOwnership` | A resource parameter was declared without an ownership modifier. Use `^`, `&`, or `&!`. |

### Internal

| Code | Kind | Explanation |
|------|------|-------------|
| `E9999` | `EInternal` | An internal compiler invariant was violated. This is a bug in `avera`, not your program. |

### Warnings

| Code | Kind | Explanation |
|------|------|-------------|
| `W0001` | `Warning` | A warning was emitted (e.g. wildcard import, malformed manifest line). Your program still compiles. |

---

## Severity and ordering

- `Severity::Error` for everything except `Warning`; `Severity::Warning`
  for `W0001`; `Severity::Note` for secondary `related` spans.
- `DiagnosticList::has_errors()` is what stops the pipeline after a
  phase (parse, check, codegen). Warnings do not stop compilation.
- Diagnostics are emitted in source order; `DiagnosticList::sort()` orders
  by `(file, span.start)`.

## Suggested fixes by symptom

| Symptom | Likely code | Fix |
|---------|-----------|-----|
| "expected … but found …" | `E0100` | fix the syntax at the caret |
| "use after move" / "use after drop" | `E4001` / `E4002` | don't use a moved/dropped value; re-bind it |
| "double drop" | `E4003` | remove the explicit `x!`; drops are automatic |
| "cannot drop a place that is currently borrowed" | `E4004` | end the borrow/magnet first |
| "cannot mutably borrow: a … borrow is already active" | `E4005` | split borrows or scope them |
| "cannot assign to a place that is mutably borrowed" | `E4006` | release the borrow before assigning |
| "cannot read address of a detached magnet" | `E4008` | re-attach with `~m = v` or use `raw` |
| "unknown type `X`" | `E2001` | define/import `X`, or check spelling |
| "`X` is already defined" | `E2002` | rename one of the declarations |

For the full lifecycle model behind the `E40xx` codes, see
[OWNERSHIP.md](OWNERSHIP.md).
