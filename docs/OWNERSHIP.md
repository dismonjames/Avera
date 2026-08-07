# Ownership in Avera

Avera has explicit, statically-checked ownership. There is no garbage
collector, no reference counting, and no runtime borrow tag. Instead, a
**forward dataflow analysis** over the MIR tracks the lifecycle of every
local and rejects programs that use a value after it has moved or been
dropped. This document explains the model the programmer sees and the
checker that enforces it.

> The checker lives in [`src/check/ownership.rs`](../src/check/ownership.rs)
> and runs after MIR lowering and validation, before drop elaboration.

---

## 1. The mental model

Every binding in Avera is an **owner** of its value. Ownership has three
operations:

1. **Read** — use the value (e.g. pass it to a function, read it in an
   expression). For copy types (primitives), this copies; for move types,
   it transfers ownership.
2. **Move** — transfer ownership to another owner or into a function.
3. **Drop** — end the value's lifetime, releasing its storage.

At every program point, each local is in exactly one of four **states**,
and the checker verifies that every operation is legal for the current
state:

| State | Meaning | Allowed |
|-------|---------|---------|
| `Uninit` | declared but not yet assigned | none (cannot read) |
| `Init` | holds a live value | read, move, drop |
| `Moved` | value moved out | cannot read or drop until reassigned |
| `Dropped` | explicitly dropped or scope ended | cannot read or drop |

A re-assignment (`x = expr`) returns a local to `Init`. A re-binding of an
existing name reuses the same local slot (see [LANGUAGE.md §4](LANGUAGE.md#4-bindings)).

---

## 2. Binding rules

| Construct | Initial state | Notes |
|-----------|---------------|-------|
| `:name = expr` | `Init` (mutable owner) | `name` may be reassigned |
| `::name = expr` | `Init` (immutable) | reassignment is `E4006` |
| function parameter | `Init` | the caller's value is available |
| anonymous temp | `Uninit` → `Init` on assign | used internally for expression results |
| `~name = expr` | magnet attachment | see [MAGNET.md](MAGNET.md) |

Parameters enter the entry block in `Init`; every other local starts
`Uninit` (`src/check/ownership.rs`).

```av
:x = 10          // x: Uninit -> Init
:y = x + 1       // y: Uninit -> Init  (x is read, primitive copy leaves x Init)
:x = x + 1       // x reassigned; still Init
```

---

## 3. Move semantics

For **copy types** (primitives: `Bool`, the integer types, `Float`,
`Char`, `Size`, `Unit`, `Never`), reading a value copies it — the source
stays `Init`. Stage-0 treats all v0.1 dataflow values as copy for the
purposes of the state machine, because the proven-working subset operates
on scalars; the move-vs-copy distinction is wired into the `Abilities::copy`
field (`src/types/ty.rs`) for when aggregate types come online.

A **move** transfers ownership: the source becomes `Moved` and may not be
used again until it is re-assigned. An explicit move is written `^x`; a
`move` parameter mode (`^name: T`) takes ownership on call.

```av
:a = build()     // a: Init
:b = ^a          // a: Moved, b: Init
print(a)         // ERROR E4001: use after move
```

The `Moved` state propagates through the CFG: at a join point, if a local
is `Moved` on any incoming edge, it is `Moved` at the join (the lattice
meet takes the most-restrictive state).

---

## 4. Drops and the lifecycle

Avera has **no unwinding**, so drop points are fully deterministic:

- An explicit early drop is written `x!`. It moves `x` from `Init` to
  `Dropped`.
- At a `Return` terminator, the **drop-elaboration pass**
  ([`src/check/drop_elaborate.rs`](../src/check/drop_elaborate.rs))
  inserts `StorageDead(local)` statements for every local still live, in
  reverse declaration order (LIFO). The ownership checker consumes these
  `StorageDead` markers as implicit drops.

Because drops are explicit and static, "use after drop" and "double drop"
are compile-time errors, not runtime faults:

```av
:x = make_thing()
x!              // x: Dropped
x!              // ERROR E4003: double drop
print(x)        // ERROR E4002: use after drop
```

> In the v0.1 core subset all types are scalars/POD, so a `StorageDead` is
  semantically a no-op, but the markers are emitted for completeness and so
  the ownership state machine has a well-defined `Dropped` transition.

---

## 5. The ownership checker

The checker is a **forward dataflow fixpoint** over the MIR control-flow
graph of each action body.

### 5.1 The lattice

```
Uninit  <  Init  <  Moved  <  Dropped
```

The `meet` of two states is the more-restrictive (higher) one
(`src/check/ownership.rs`):

```rust
fn meet(self, other: State) -> State {
    match (self, other) {
        (Dropped, _) | (_, Dropped) => Dropped,
        (Moved, _) | (_, Moved) => Moved,
        (Uninit, _) | (_, Uninit) => Uninit,
        (Init, Init) => Init,
    }
}
```

### 5.2 The transfer function

For each `BasicBlock` the checker starts from the block's in-state and
walks its statements, updating the per-local state map:

| Statement | Effect |
|-----------|--------|
| `Assign { place, value }` | use the rvalue's operands; then `place.local := Init` |
| `StorageLive(id)` | `id := Uninit` |
| `StorageDead(id)` | if `id` was `Init`, `id := Dropped` |
| `Drop(place)` | `Init → Dropped`; `Dropped` → `E4003`; `Moved` → `E4001`; `Uninit` → `E4002` |
| `Call { dest, args, .. }` | use each arg; `dest.local := Init` |
| `Assert { cond, .. }` | use `cond` |

`use_place` reads a local and reports:

- `Init` → OK (copy leaves it `Init`)
- `Moved` → **E4001** use after move
- `Dropped` → **E4002** use after drop
- `Uninit` → use of uninitialized value (reported as `E4002`)

### 5.3 The fixpoint

1. **Init:** block 0's in-state has every parameter `Init` and every other
   local `Uninit`.
2. **Worklist:** pop a block, compute its out-state by transferring its
   in-state, then meet the out-state into each successor's in-state. If a
   successor's in-state changes, push it.
3. **Termination:** the lattice is finite and monotone, so the fixpoint is
   reached. Successors are derived from each terminator (`Goto`,
   `SwitchInt`, `Switch`, `Return`, `Abort`, `Unreachable`).

This is exactly the structure of a textbook reaching-definitions /
available-expressions engine, applied to lifecycle states instead of
facts.

---

## 6. Interactions with borrows and magnets

Ownership is the foundation; the **borrow checker** and **magnet
checker** layer on top of it. They enforce:

- You cannot **move or drop** a value while a borrow or magnet depends on
  it (`E4004`).
- You cannot have two **mutable** accesses to the same place overlapping
  (`E4005`).
- You cannot **mutate** an immutable binding or a borrowed place (`E4006`).

These are described in their own documents:

- [MAGNET.md](MAGNET.md) — the magnet checker.
- [DIAGNOSTICS.md](DIAGNOSTICS.md) — the full error catalogue.

The borrow checker (`src/check/borrow.rs`) and magnet checker
(`src/check/magnet.rs`) run in the same phase as the ownership checker,
right after MIR validation and before drop elaboration
(`src/driver_pipeline/mod.rs`).

---

## 7. Worked example

`examples/fib.av` exercises the model with reassignment and re-binding
inside a loop:

```av
action main(): I32 will
    :a = 0
    :b = 1
    :t = 0
    :i = 0
    while i < 10 will
        t = a + b
        a = b
        b = t
        i += 1
    finish
    print("fib(10) =", a)
    return 0
finish
```

State trace for `a` and `b` across one iteration (all primitives, so reads
copy and leave the source `Init`):

| Point | `a` | `b` | `t` | `i` |
|-------|-----|-----|-----|-----|
| loop entry | `Init` | `Init` | `Init` | `Init` |
| `t = a + b` | `Init` | `Init` | `Init` | `Init` |
| `a = b` | `Init` | `Init` | `Init` | `Init` |
| `b = t` | `Init` | `Init` | `Init` | `Init` |
| `i += 1` | `Init` | `Init` | `Init` | `Init` |
| back-edge to header | `Init` | `Init` | `Init` | `Init` |

Because every read is of a copy type, no local ever leaves `Init`, and the
fixpoint is trivially stable. The exit block sees all locals `Init`; drop
elaboration inserts `StorageDead` for each, in reverse order.

---

## 8. Common errors

| Code | Meaning | Fix |
|------|---------|-----|
| `E4001` | use after move | create a new value, borrow it, or don't move it |
| `E4002` | use after drop / uninit | don't drop before use; ensure the binding has a value |
| `E4003` | double drop | the compiler inserts drops automatically; don't `x!` an already-dropped binding |
| `E4004` | move/drop while borrowed | end the borrow or detach the magnet first |
| `E4006` | immutable binding mutated | use `:` instead of `::`, or release the borrow |

See [DIAGNOSTICS.md](DIAGNOSTICS.md) for the full list.
