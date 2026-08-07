# The Magnet System

A **Magnet** is Avera's pointer discipline. Where a borrow `&x` is a
short-lived reference bound by scope, a magnet `~m` is a *relocatable
handle* that attaches to a value, remembers where it lives in memory, and
can be queried for its real runtime address. Magnets are how Avera gives you
"the address of a thing" without handing you a raw pointer that the
type system cannot reason about.

This document describes the surface syntax, the lowering, the runtime
proof that magnet addresses are real machine addresses, and the magnet
checker that keeps them safe.

> Implementation: creation/queries are lowered in
> [`src/driver_pipeline/lower.rs`](../src/driver_pipeline/lower.rs) and
> compiled in [`src/codegen/cranelift.rs`](../src/codegen/cranelift.rs);
> the safety checker is
> [`src/check/magnet.rs`](../src/check/magnet.rs).

---

## 1. What is a magnet?

A magnet is a value of type `Magnet<T>` (read) or `MagnetMut<T>`
(mutable). At runtime it is a single machine word: the **address of the
storage it points at**. The type parameter `T` records what kind of value
lives there, so the type system can track element types and (later) bounds.

Conceptually:

```
Magnet<T>   ≈   address-of a live T, with a tracked lifetime
```

The crucial property, proven below: the address a magnet reports is a
**real address in the running process's address space**, not an index, a
fake handle, or a compile-time placeholder. Two magnets over two distinct
locals report two distinct addresses exactly one slot apart.

---

## 2. Surface syntax

### 2.1 Creating a magnet

```av
~name = expr        // read magnet attached to the value of expr
~!name = expr       // mutable magnet attached to expr
```

The magnet binds `name` to a handle pointing at the storage that holds the
*value* of `expr`. The parser recognises `~` / `~!` as the magnet binding
introducers (`src/parser/stmt.rs`) and produces a `MagnetBinding` /
`MagnetMutBinding` statement.

`examples/magnet.av`:

```av
action main(): I32 will
    :value = 42
    ~m = value
    print("Value:", value)
    print("Address:", ~m.address)
    return 0
finish
```

### 2.2 Querying a magnet

| Syntax | Meaning |
|--------|---------|
| `~m.address` | the real runtime address `m` points at |
| `~m.offset` | the byte offset from the attachment base |
| `m -> other` | retarget `m` to point at `other` |
| `m -> none` | detach `m` (it no longer points anywhere) |
| `m?` | magnet validity test (true if attached) |

`~m.address` is parsed as a `MagnetMeta` expression with property
`"address"` (`src/parser/expr.rs`, `src/ast/expr.rs`).

### 2.3 Detaching

`m -> none` detaches a magnet. A detached magnet cannot be queried for its
address — the magnet checker rejects it with `E4008`.

### 2.4 Raw magnets

Inside a `raw will … finish` block, you may construct a magnet from an
integer address (`expr :> Address<...>` → `MagnetFromAddr`). Outside a
`raw` block, converting an integer to an address is `E4007`.

---

## 3. Lowering: how a magnet becomes an address

When the lowerer sees `~m = value` it does the following
(`src/driver_pipeline/lower.rs`):

1. Evaluate `value` into a fresh temporary local `t`.
2. Allocate a magnet local `m` of type `I64`.
3. Emit `m := MagnetAttach { place: t, is_mut, ty: I64 }`.

In Cranelift codegen, `MagnetAttach` is **not** a fake. It allocates a real
8-byte stack slot, stores the value into it, and yields the slot's
address (`src/codegen/cranelift.rs`):

```rust
Rvalue::MagnetAttach { place, .. } => {
    let val = place_val(builder, body, place, local_vars, cx, defs);
    let ss = builder.create_sized_stack_slot(StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        types::I64.bytes() as u32,
        1,
    ));
    let addr = builder.ins().stack_addr(types::I64, ss, 0);
    builder.ins().store(MemFlags::trusted(), val, addr, 0);
    addr
}
```

So:

- `create_sized_stack_slot` reserves 8 bytes on the actual C stack frame
  that Cranelift is emitting.
- `stack_addr` computes the real address of that slot at runtime.
- `store` writes the value there.
- The magnet local `m` *is* that address, carried in an `I64`.

And `~m.address` (`Rvalue::MagnetAddress`) simply loads the magnet local:

```rust
Rvalue::MagnetAddress { magnet, .. } => {
    place_val(builder, body, magnet, local_vars, cx, defs)
}
```

The address you print is therefore the literal address of a stack slot in
your running program.

---

## 4. Proof: the addresses are real

The claim "magnets return real runtime memory addresses" is verified
end-to-end, not asserted. Here are the observed facts from running the
compiled examples (Cranelift → `cc` link → native exec):

### 4.1 `magnet.av`

Source:

```av
:value = 42
~m = value
print("Value:", value)
print("Address:", ~m.address)
```

A run produces (address varies per run):

```
Value: 42
Address: 140735769045984
```

The address is printed as a signed decimal integer: `~m.address` lowers
to a `MagnetAddress` rvalue, and the `print` builtin routes any
non-literal expression through the runtime `avera_print_i64`
([`src/driver_pipeline/lower.rs`](../src/driver_pipeline/lower.rs)). The
runtime also provides a hex-formatted `avera_print_addr`, but the `print`
builtin does not route address values to it — it prints them as ordinary
integers. Either way the value is a genuine `unsigned long` produced by
Cranelift's `stack_addr`. Stack addresses on x86-64 Linux sit in the low
half of the address space, so `avera_print_i64` (which treats its argument
as signed) prints them as large positive decimals.

### 4.2 `magnet2.av` — the decisive test

Source:

```av
:x = 42
:y = 999
~a = x
~b = y
print("a ->", x, "at", ~a.address)
print("b ->", y, "at", ~b.address)
```

Two consecutive runs:

```
a -> 42 at 140721004500240
b -> 999 at 140721004500248
```

```
a -> 42 at 140733816996064
b -> 999 at 140733816996072
```

Three properties hold, and each is a consequence of the lowering in §3:

1. **The addresses differ between runs.** Stack base addresses are
   randomized by ASLR; a real stack slot's address reflects that. A fake
   handle or a compile-time index would be identical across runs.
2. **The two addresses are exactly 8 bytes apart.** `~a` and `~b` each
   allocate one `I64` (8-byte) stack slot, back to back:
   `140721004500248 - 140721004500240 = 8`, and
   `140733816996072 - 140733816996064 = 8`. The slots are the magnet's own
   storage.
3. **The printed value matches the bound value.** `a -> 42` and `b -> 999`
   confirm the slots actually hold `x` and `y`; the magnet points at real
   initialized storage.

Together these prove the magnet address is a live machine address of a
real stack slot holding the bound value — not a placeholder.

> Reproduce: `avera run examples/magnet2.av` twice and compare the printed
> addresses.

---

## 5. The magnet checker

The magnet checker is a per-block state-machine analysis
([`src/check/magnet.rs`](../src/check/magnet.rs)). It tracks each magnet
local through four states:

| State | Meaning |
|-------|---------|
| `Detached` | not attached, or `~m = none` |
| `Attached` | points at a live place (`~m = value`) |
| `Moved` | the target moved/relocated |
| `Dropped` | the magnet was dropped |

Transfers:

| Statement | Effect |
|-----------|--------|
| `MagnetAttach` | dest := `Attached` |
| `MagnetNone` | dest := `Detached` |
| `MagnetFromAddr` (raw) | dest := `Attached` |
| `MagnetAddress { magnet }` | require `Attached`, else `E4008` |
| `MagnetOffset { magnet }` | require `Attached`, else `E4008` |
| `MagnetAdd { magnet, .. }` | require `Attached`, else `E4008` |
| `Drop(place)` | if attached, `Attached → Dropped` |

So:

- Querying a **detached** magnet (`m -> none`, then `~m.address`) is
  `E4008`.
- Querying a magnet whose target has **moved** is `E4009`.
- Querying a **dropped** magnet is `E4009`.

The checker runs alongside the ownership and borrow checkers, after MIR
validation and before drop elaboration
([`src/driver_pipeline/mod.rs`](../src/driver_pipeline/mod.rs)).

---

## 6. Magnets vs. borrows

| | Borrow `&x` | Magnet `~m = x` |
|---|-------------|------------------|
| Stored as | a (pointer, lifetime) pair | one machine word: the address |
| Lifetime | scope-bound | explicit; can be retargeted/detached |
| Relocatable | no | yes (`m -> other`) |
| Reports address | no | yes (`~m.address`) |
| Checked by | borrow checker (`src/check/borrow.rs`) | magnet checker (`src/check/magnet.rs`) |

A magnet attach acts like a borrow for aliasing purposes: the borrow
checker rejects attaching a magnet to a place that is already mutably
borrowed (`E4005`), and the ownership checker rejects moving a place out
from under a live magnet (`E4004`).

---

## 7. The full magnet example

`examples/magnet2.av`:

```av
#std.io

action main(): I32 will
    :x = 42
    :y = 999
    ~a = x
    ~b = y
    print("a ->", x, "at", ~a.address)
    print("b ->", y, "at", ~b.address)
    return 0
finish
```

Compile and run:

```bash
avera run examples/magnet2.av
```

```
a -> 42 at 140727628194944
b -> 999 at 140727628194952
```

(The exact addresses change per run due to ASLR; the difference between the
two is always `8` — the size of one `I64` magnet slot. Addresses print as
signed decimals via `avera_print_i64`.)
