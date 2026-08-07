# MIR — Mid-level Intermediate Representation

MIR is the contract between the front end and the back end of `avera`. The
AST lowers to a small, explicit MIR; all semantic checks run on MIR; and
MIR is what the Cranelift backend consumes. This document describes the
MIR data structures and the dataflow checkers that validate them.

> Source: [`src/mir/`](../src/mir/) and [`src/check/`](../src/check/).

---

## 1. Shape of a MIR body

A MIR **body** is one compiled action
([`src/mir/body.rs`](../src/mir/body.rs)):

```rust
struct Body {
    name: String,        // action name
    locals: Vec<Local>,  // all locals (params first)
    blocks: Vec<BasicBlock>,
    entry: BlockId,      // always block 0
    ret_ty: TyId,
    params: Vec<LocalId>,
}
```

- `locals` is a flat vector indexed by `LocalId`. Parameters come first and
  are also listed in `params`.
- `blocks` is a flat vector indexed by `BlockId`. The invariant
  **`blocks[i].id == BlockId::new(i)`** is enforced by the validator; this
  lets jump targets use a block id directly as an index.
- `entry` is always `BlockId::new(0)`.

A **local** carries its name, type, and mutability:

```rust
struct Local { id: LocalId, name: String, ty: TyId, mutable: bool }
```

A **basic block** is a list of statements followed by exactly one
terminator:

```rust
struct BasicBlock { id: BlockId, stmts: Vec<Stmt>, term: Terminator }
```

---

## 2. Places

A [`Place`](../src/mir/place.rs) is an addressable location within a
local — the left-hand side of an assignment or an operand of an rvalue.

```rust
struct Place { local: LocalId, elems: Vec<PlaceElem> }

enum PlaceElem {
    Field(u32),       // .field   — field index inside a shape
    Index(LocalId),   // [index] — array index (index is itself a local)
    Deref,            // .deref  — magnet/borrow target dereference
}
```

`Place::local(id)` is a place with no projections — just the local itself.
`Place::field(id, idx)` and `Place::deref(id)` are convenience
constructors. In the v0.1 backend, the Cranelift lowering currently reads
and writes whole locals (the place's `elems` are tracked structurally by
the validator but scalars live in Cranelift `Variable`s).

---

## 3. Rvalues

An [`Rvalue`](../src/mir/rvalue.rs) is the right-hand side of an
assignment — a pure computation that produces a value into a place.

```rust
enum Rvalue {
    Use(Place),
    Const(Const),
    BinOp { op, lhs, rhs, ty },
    UnOp { op, operand, ty },
    Borrow { place, is_mut, ty },
    MagnetAttach { place, is_mut, ty },
    MagnetNone { ty },
    MagnetFromAddr { addr, ty },          // raw only
    MagnetAddress { magnet, ty },
    MagnetOffset { magnet, ty },
    MagnetAdd { magnet, delta, ty },
    Cast { operand, from, to },
    ChoiceCtor { def, variant, args, ty },
    ShapeCtor { def, fields, ty },
    ArrayCtor { elems, ty },
    Call { callee, args, ret_ty },
    Try { operand, ok_ty, err_ty },
}
```

`Const` values:

```rust
enum Const {
    Int(i64), U64(u64), Float(f64), Bool(bool),
    Str(String), Char(char), Unit, Addr(u64),
}
```

`BinOp` mirrors the source operators:

```rust
enum BinOp { Add, Sub, Mul, Div, Mod,
             Eq, Ne, Lt, Le, Gt, Ge,
             BitAnd, BitOr, BitXor, Shl, Shr }
enum UnOp { Not, Neg }
```

Each `BinOp`/`UnOp` carries its result `TyId` so the backend can choose
signed vs. unsigned comparisons and float vs. int arithmetic
(`src/codegen/cranelift.rs`).

---

## 4. Statements

A MIR statement is one of
([`src/mir/body.rs`](../src/mir/body.rs)):

```rust
enum Stmt {
    Assign { place: Place, value: Rvalue },
    StorageLive(LocalId),
    StorageDead(LocalId),
    Drop(Place),
    Call { dest: Place, callee: String, args: Vec<Place>, ret_ty: TyId },
    Assert { cond: Place, msg: String },
}
```

- `Assign` evaluates `value` and stores it into `place`.
- `StorageLive` / `StorageDead` mark a local's storage lifetime. The drop
  elaborator inserts `StorageDead` at `Return` terminators.
- `Drop` is an explicit early drop (`x!`).
- `Call` is a statement-level call (used by the I/O built-ins and
  `input()`), distinct from the rvalue `Rvalue::Call`.
- `Assert` traps if `cond` is false (used for runtime invariants).

---

## 5. Terminators

Every block ends with exactly one [`Terminator`](../src/mir/terminator.rs):

```rust
enum Terminator {
    Goto(BlockId),
    Switch { discr: Place, targets: Vec<(u64, BlockId)>, otherwise: Option<BlockId> },
    SwitchInt { discr: Place, targets: Vec<(i128, BlockId)>, otherwise: BlockId },
    Return { value: Option<Place> },
    Abort,
    Unreachable,
}
```

- `Goto(b)` — unconditional jump.
- `SwitchInt` — integer switch used for `match` on ints and `if` chains.
  Each `(value, target)` is a case; `otherwise` is the fall-through (the
  `else` branch, or the catch-all match arm).
- `Switch` — a `u64`-keyed variant switch (for choice discriminants).
- `Return { value }` — return from the action; `None` for unit returns.
- `Abort` — trap (lowered from `panic`).
- `Unreachable` — placeholder used while building; replaced before
  validation.

### How control-flow constructs lower

| Source | MIR shape |
|--------|-----------|
| `if c will T else E` | `SwitchInt { discr=c, targets=[(1, then_blk)], otherwise=else_blk }` then both arms `Goto merge_blk` |
| `else if` chain | a cascade of `SwitchInt` blocks, each `else_blk` continuing the chain; all share `merge_blk` |
| `while c will B` | `header: SwitchInt{c}` → `body_blk` / `exit_blk`; `body` ends with `Goto header` |
| `loop will B` | `header` with `Goto header` back-edge; `exit_blk` for `break` |
| `for i in lo..hi` | counter `i := lo`; `header: i < hi` → `body_blk` / `exit_blk`; `body` ends with `i += 1; Goto header` |
| `break` / `continue` | `Goto exit_blk` / `Goto header` of innermost loop; a dead block follows |
| `match n will …` | `SwitchInt { discr=n, targets=[(lit, arm_blk)…], otherwise=catchall_blk }`; each arm `Goto merge_blk` |

The lowerer keeps a `loop_ctx` stack of `(header, exit)` block ids for
`break`/`continue` to target ([`src/driver_pipeline/lower.rs`](../src/driver_pipeline/lower.rs)).

---

## 6. Building a body

The lowerer (`Lowerer` in `src/driver_pipeline/lower.rs`) builds a body in
two phases:

1. **Pre-allocate blocks.** `new_block_id` pushes a placeholder block whose
   id equals its index, so jump targets can be named before they are
   filled. Blocks start with the `Unreachable` terminator.
2. **Fill blocks.** `flush(term)` writes the accumulated statements and
   the terminator into the current block; `start_block(id)` switches to a
   pre-allocated block.

On `finish`, any block still carrying the placeholder `Unreachable`
terminator is rewritten to `Return { value: None }` so the validator is
satisfied. `into_body` does a final pass replacing any remaining
`Unreachable` with a safe `Return`.

---

## 7. Validation

Before any semantic check runs, the validator asserts structural
invariants ([`src/check/validate.rs`](../src/check/validate.rs)):

| Check | Error |
|-------|-------|
| body has at least one block | `E1101` (MirEmptyBody) |
| `entry` is block 0 | `E1102` (MirInvalidEntry) |
| each `blocks[i].id == BlockId::new(i)` | `E1103` (MirBlockIdMismatch) |
| every `Place.local` is in range | `E1104` (MirInvalidLocal) |
| every `Goto`/`Switch`/`SwitchInt` target is in range | `E1105` (MirInvalidTarget) |
| every `Return` in a non-`Unit` body carries a value | `E1100` (MirValidation) |

`validate_return_consistency` separately walks every block ending in
`Return { value: None }` and, if the body's return type is not `Unit` or
`Never`, reports that the function returns without a value.

---

## 8. The dataflow checkers

After validation, three per-body dataflow analyses run
([`src/driver_pipeline/mod.rs`](../src/driver_pipeline/mod.rs)):

```
for b in &bodies {
    validate_mir(b, &mut diags);
    validate_return_consistency(b, &cx, &mut diags);
    check_ownership(b, &mut diags);
    check_borrow(b, &mut diags);
    check_magnet(b, &mut diags);
}
```

### 8.1 Ownership checker

A forward dataflow fixpoint over a four-state lattice
(`Uninit < Init < Moved < Dropped`). In-state of block 0: params `Init`,
others `Uninit`. Transfer walks statements, updating states and reporting
`E4001`/`E4002`/`E4003`/`E4004`. Meet at joins takes the most-restrictive
state. See [OWNERSHIP.md](OWNERSHIP.md).

### 8.2 Borrow checker

Per-block shared/exclusive borrow tracking (`src/check/borrow.rs`). Each
local is `None` / `Shared` / `Mut`. Reports `E4005` (mutable alias),
`E4006` (immutable mutate / borrow escape), `E4004` (move/drop while
borrowed). A magnet attach counts as a borrow for aliasing.

### 8.3 Magnet checker

Per-block magnet attachment tracking (`src/check/magnet.rs`). Each magnet
local is `Detached` / `Attached` / `Moved` / `Dropped`. Reports `E4008`
(querying a detached/moved magnet), `E4009` (target moved/dropped). See
[MAGNET.md](MAGNET.md).

> Stage-0 note: the borrow and magnet checkers are currently per-block
> (they reset state at each block). The ownership checker is the full
> inter-block fixpoint. This matches the v0.1 subset where borrows and
> magnet uses are local to a straight-line block, while moves and drops
> flow across the whole CFG.

---

## 9. Drop elaboration

After the checkers pass, `elaborate_drops`
([`src/check/drop_elaborate.rs`](../src/check/drop_elaborate.rs)) inserts
`StorageDead(local)` for every local, in reverse declaration order, into
each block that ends with `Return`. `Abort` and `Unreachable` terminators
get no cleanup (there is no unwinding in v0.1). The markers feed the
ownership checker's `Dropped` transition and are otherwise no-ops for the
scalar v0.1 types.

---

## 10. Reading a body

To dump the MIR for a program:

```bash
avera build --emit mir examples/fib.av
```

Or set `AVERA_DUMP_MIR=1` to print the MIR for every body during a normal
build. The MIR is printed with Rust's `Debug` format, so you see the
locals, blocks, statements, and terminators directly.

---

## 11. Worked example: `match_int.av`

Source:

```av
:n = 3
match n will
    1 => print("one")
    2 => print("two")
    _ => print("other")
finish
return 0
```

Conceptual MIR (block ids in brackets):

```
bb0:
    n := Const(3)
    SwitchInt { discr=n, targets=[(1, bb1), (2, bb2)], otherwise=bb3 }
bb1: print("one");  Goto bb4
bb2: print("two");  Goto bb4
bb3: print("other"); Goto bb4   // the `_` arm is the otherwise target
bb4:
    Return { value: Const(0) }
```

The catch-all arm `_` is chosen as the `otherwise` target when it exists;
without a catch-all, the lowerer synthesises an otherwise block that
falls through to the merge block.
