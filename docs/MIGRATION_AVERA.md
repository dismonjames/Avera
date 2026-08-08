# Migration: C>> / CGreater → Avera

This document records the identity migration from the old `C>>` / `CGreater`
language to **Avera**. It is a permanent rename, not a temporary alias.

## Identity changes

| Old | New |
|-----|-----|
| `C>>` / `CGreater` | `Avera` |
| `.cgr` (source) | `.av` |
| `.mgr` (module descriptor) | `.mav` |
| `.hgr` / `.hav` (header) | removed — no separate header file type |
| `cgreater.toml` | `avera.toml` |
| `cgr` (CLI) | `avera` |
| `cgr_*` runtime symbols | `avera_*` |
| `CGR_DUMP_*` env vars | `AVERA_DUMP_*` |

## The canonical Avera file types

Avera has exactly **two** canonical source-facing file types:

- `.av` — source files: implementations, private declarations, action bodies,
  `main`, and tests.
- `.mav` — module contracts: module identity, public interface, source
  membership, dependencies, platform conditions, and native link metadata.

There is **no separate header file type**. `.mav` replaces *both* the old
`.mgr` module descriptor *and* the separate `.hgr`/`.hav` public
header/interface.

## Why `.mav` is not "a header with a new extension"

The old model had three files per module:

```
user.mgr    (module descriptor)
user.hgr    (public header / interface)
user.cgr    (implementation)
```

Avera collapses this into two:

```
user.mav    (module contract: identity + public interface + source list)
user.av     (implementation)
```

`.mav` is the *module definition* of Avera — it declares what the module is,
what it exports, which source files make it up, and what it depends on. It is
not a "header"; it is the authoritative public contract and the dependency
graph entry point.

## Module contract example

`math.mav`:

```av
#module app.math

#source "math.av"

export action add(a: I32, b: I32): I32
export action sub(a: I32, b: I32): I32
```

`math.av`:

```av
action add(a: I32, b: I32): I32 will
    return a + b
finish

action sub(a: I32, b: I32): I32 will
    return a - b
finish
```

Caller (`main.av`):

```av
#std.io
#app.math

action main(): I32 will
    print("sum:", add(10, 20))
    return 0
finish
```

## Public / private

Anything declared `export` in a `.mav` is public. Implementation-only symbols
in `.av` remain private unless the module contract exports them. Do **not**
duplicate `export` in both `.mav` and `.av` — the contract alone makes a symbol
public.

## Legacy extensions

If you run the old extensions, the compiler gives a clear, actionable
diagnostic instead of a generic "unknown file":

```
error: old.cgr: `.cgr` is no longer an Avera source extension
  → use `.av`

error: old.mgr: `.mgr` is no longer an Avera module extension
  → use `.mav`

error: old.hgr: Avera no longer uses separate header files
  → move the public interface into the module's `.mav` file
```
