# Avera Language Reference

This document is the reference for the Avera language as
implemented by the stage-0 `avera` compiler. It describes the syntax, types,
expressions, statements, and operators that the lexer, parser, and MIR
lowerer accept and the proven-working backend supports.

> The authoritative grammar is in [grammar.md](grammar.md); this document
> explains what each construct *means*. Examples are drawn from the
> programs in [`examples/`](../examples/), all of which compile and run
> (see [README.md](README.md#the-examples-directory)).

---

## 1. Lexical structure

### 1.1 Source format

A Avera source file is UTF-8 text. Statements are terminated by **significant
newlines** or `;`. Inside parentheses `( … )` and brackets `[ … ]` newlines
are insignificant (the lexer tracks bracket depth and suppresses newline
tokens there), which lets you wrap long argument lists and array literals
across lines.

Whitespace (space, tab, carriage return) is otherwise irrelevant.

### 1.2 Comments

```av
// line comment — runs to the end of the line

/* block comment — non-nesting in v0.1 */
```

A block comment that is not closed before end-of-file is a lexical error.

### 1.3 Identifiers and keywords

An identifier starts with `_` or an ASCII letter and continues with `_` or
alphanumeric characters. Identifiers are case-sensitive.

**Keywords** (reserved and cannot be used as identifiers):

```
if else while loop for in match
action shape choice ability implement foreign export
return break continue will finish raw
self true false none
```

### 1.4 Literals

| Literal | Examples | Notes |
|---------|----------|-------|
| Integer | `42`, `0`, `1_000`, `0xFF`, `0b1010` | decimal, hex (`0x`), binary (`0b`); `_` digit separators allowed |
| Float | `3.14`, `1.0`, `1e9`, `2.5e-3` | optional fractional and exponent parts |
| String | `"hello"`, `"with\nescapes"` | UTF-8; escapes below |
| Char | `'a'`, `'\n'`, `'\x41'`, `'字'` | a single Unicode scalar |
| Bool | `true`, `false` | |
| Unit | `()` | the unit value |

**String and char escapes:** `\n` `\t` `\r` `\\` `\"` `\'` `\0` `\xHH`
(two hex digits) `\u{HHHH…}` (a brace-delimited hex Unicode code point).

### 1.5 Operators and punctuation

```
:   ::  ;   ,   .   ..
(   )   [   ]   <   >
<=  >=  ==  !=  =   +   -   *   /   %
&   &!  &&  |   ||  ^   !   <<  >>
:>  ->  =>  +=  -=  *=  /=  %=
|=  &=  ^=  ~   ~!  ?   @   #   $
```

| Token | Meaning |
|-------|---------|
| `:`  | mutable binding introducer; type separator |
| `::` | immutable binding introducer |
| `..` | range operator (in `for`) |
| `:>` | conversion (cast) operator |
| `->` | magnet retarget |
| `=>` | match arm |
| `~` / `~!` | magnet bind (read / mutable) |
| `?` | error propagation / magnet validity test |
| `@` | attribute marker |
| `#` | directive / import marker |
| `$` | reserved |

### 1.6 Directives

A directive begins with `#` and must appear **before any item**. The
stage-0 prelude you will use is the standard-I/O import:

```av
#std.io
```

The general forms are:

```av
#std.io                 // import the std.io module
#std.fs as fs           // import with an alias
#std.collections.*      // wildcard import
#module app.user        // module identity (used in .mav / .mav files)
#header "fs.mav"        // header include
#source "file.av"      // additional source file
#depends std.core       // module dependency
#cfg linux              // cfg flag
#target ...             // target directive
#link "m"               // native link directive
```

In stage-0, `#std.io` is the import used by every example and brings the
I/O built-ins (`print`, `write`, `eprint`, `input`) into scope. See
[STDLIB.md](STDLIB.md).

---

## 2. Program structure

A Avera program is a single module consisting of directives followed by
top-level **items**.

```av
#std.io

action add(a: I32, b: I32): I32 will
    return a + b
finish

action main(): I32 will
    :r = add(3, 4)
    print("add(3,4)=", r)
    return 0
finish
```

### 2.1 Items

| Item | Syntax | Status |
|------|--------|--------|
| `action` | `action name(params): Ret will … finish` | proven working |
| `shape` | `shape Name will fields… finish` | parsed; not v0.1 proven |
| `choice` | `choice Name will variants… finish` | parsed; not v0.1 proven |
| `ability` | `ability Name will action-sigs… finish` | parsed; not v0.1 proven |
| `implement` | `implement Ability for Type will actions… finish` | parsed; not v0.1 proven |
| `foreign` | `foreign ABI ("lib") will @link("sym") raw action sig… finish` | parsed; not v0.1 proven |

Any item may be preceded by `export` and/or attributes `@name(args)`.

The **proven-working** backend subset centers on `action`. The other items
have full front-end support (lexer + parser + AST + formatter), and they
appear in the grammar, but the v0.1 lowering focuses on actions and the
features listed in [README.md](README.md#proven-working-feature-set).

### 2.2 Actions (functions)

```ebnf
ActionDecl ::= 'action' ActionSig 'will' Block 'finish'
             | 'action' ActionSig                        (* header decl, no body *)
ActionSig  ::= [TypePath '.'] name '(' [ParamList] ')' [':' Type]
ParamList  ::= Param (',' Param)*
Param     ::= ['&' ['!'] | '^'] name ':' Type            (* mode prefix optional *)
```

- The return type is optional; omitting it means `Unit`.
- A method signature is written `Type.method(params): Ret` — the part
  before the `.` is the receiver type path. (See the note on `shape.av` in
  the README.)
- Parameter modes: plain `name: T` (value/take), `&name: T` (read borrow),
  `&!name: T` (mutable borrow), `^name: T` (move). `self` receiver forms
  (`&self`, `&!self`, `^self`) are parsed but not in the v0.1 proven set.

Example — `examples/arithmetic.av`:

```av
action add(a: I32, b: I32): I32 will
    return a + b
finish

action main(): I32 will
    :x = 10
    :y = 20
    :z = add(x, y)
    print("sum:", z)
    return 0
finish
```

Actions may call each other freely, including **mutual recursion**, because
the codegen declares every function in a first pass before compiling any
body (`src/codegen/cranelift.rs`).

---

## 3. Types

### 3.1 Primitive types

| Type | Size | Description |
|------|------|-------------|
| `Bool` | 1 | boolean (`true`/`false`) |
| `Byte`, `U8` | 1 | unsigned byte |
| `I8` | 1 | signed byte |
| `Char`, `U16` | 2 | Unicode code unit / unsigned 16 |
| `I16` | 2 | signed 16 |
| `I32`, `U32`, `F32` | 4 | signed/unsigned int, float |
| `I64`, `U64`, `F64` | 8 | signed/unsigned int, float |
| `Size`, `Int`, `UInt` | 8 | pointer-sized integers (signed/unsigned) |
| `Text` | 24 | owned UTF-8 string (aggregate) |
| `Bytes` | — | raw byte buffer |
| `Unit` | 0 | the unit type |
| `Never` | 0 | diverging (no values) |

### 3.2 Constructed types

| Syntax | Meaning |
|--------|---------|
| `&T`, `&!T` | shared / mutable borrow |
| `Magnet<T>`, `MagnetMut<T>` | read / mutable magnet (see [MAGNET.md](MAGNET.md)) |
| `Address<T>` | machine address with element type |
| `[T; N]` | fixed-size array of `N` elements |
| `Span<T>` | borrowed contiguous region |
| `Maybe<T>` | optional value |
| `Outcome<T, E>` | success-or-error |
| `(A, B, …)` | tuple (reserved in v0.1) |
| `Self` | the implementing type (only inside an impl) |

The parser specialises the common constructors: writing `Magnet<User>`,
`Maybe<I32>`, `Outcome<T, E>`, `Address<U8>`, `Span<U8>` produces the
corresponding `TyKind` directly rather than a generic `Named` type
(`src/parser/ty.rs`).

### 3.3 Type abilities

A type carries an `Abilities` set (`src/types/ty.rs`): `copy`
(`Primitive` / `User` / `No`), `drop`, `display`, `equal`, `order`.
Primitives get `copy = Primitive, display, equal, order` and no `drop`.

---

## 4. Bindings

A binding introduces a name into the current lexical scope.

| Form | Meaning | Mutability |
|------|---------|-----------|
| `:name = expr` | owner binding — holds a value | mutable |
| `::name = expr` | const binding — holds a value | immutable |
| `:name: Type = expr` | owner binding with explicit type | mutable |
| `~name = expr` | magnet binding (read) | — |
| `~!name = expr` | magnet binding (mutable) | — |

Re-binding a name that already exists in scope **reuses the local slot**
rather than shadowing: the lowerer looks up the existing local before
allocating a new one (`src/driver_pipeline/lower.rs`), so this works:

```av
:sum = 0
for i in 1..101 will
    sum += i
finish
print("sum 1..100 =", sum)     // 5050
```

(From `examples/sumto.av`.)

See [OWNERSHIP.md](OWNERSHIP.md) for the ownership lifecycle that backs
these bindings.

---

## 5. Expressions

Expressions are parsed by a **Pratt parser** with the precedence table
below (`src/parser/expr.rs`). Binary operators associate left; unary
prefix operators bind tighter than any binary operator.

### 5.1 Precedence table

Higher number = tighter binding.

| Prec | Operators |
|------|-----------|
| 9 | `*` `/` `%` |
| 8 | `+` `-` |
| 7 | `<<` `>>` |
| 6 | `&` |
| 5 | `^` |
| 4 | `\|` |
| 3 | `==` `!=` `<` `<=` `>` `>=` |
| 2 | `&&` |
| 1 | `\|\|` |

Unary prefix: `!` (logical not), `-` (negation), `&`/`&!` (borrow read /
mutable), `^` (move), `~` (magnet reference).

Postfix: `.field`, `.method(args)`, `[index]`, `[lo..hi]` (slice),
`(args)` (call), `:> Type` (cast), `?` (try), and `~expr.property`
(magnet metadata such as `~m.address`).

### 5.2 Primary expressions

- Literals: `42`, `3.14`, `'a'`, `"text"`, `true`, `false`, `()`.
- Paths: `x`, `std.fs.File`, `User`.
- `self`.
- Choice construction: `.some(x)`, `.none`, `Maybe<I32>.some(42)`.
- Array literal: `[a, b, c]` (lowered to a `@array` constructor call).
- Parenthesised: `( expr )`.

### 5.3 Ranges

`lo .. hi` is a range expression, currently used as the iterator of a
`for` loop. The upper bound is **exclusive**:

```av
for i in 0..10 will ... finish    // i = 0, 1, …, 9
```

### 5.4 Casts

`expr :> Type` converts. In the v0.1 backend it lowers to a zero-extend
(`uextend`) in Cranelift (`src/codegen/cranelift.rs`). Narrowing
conversions that lose data are flagged (`E5003`).

---

## 6. Operators

### 6.1 Arithmetic

`+` `-` `*` `/` `%` over integers and floats. Integer `/` is unsigned
division and `%` is unsigned remainder at the Cranelift level in v0.1
(`udiv`/`urem`); signed comparisons use the type's signedness.

### 6.2 Comparison

`==` `!=` `<` `<=` `>` `>=`. For integers the comparison is signed or
unsigned according to the operand type (`src/codegen/cranelift.rs`); for
floats, `fcmp` with the corresponding `FloatCC`. Comparison results are
normalised to an integer `0`/`1` (the v0.1 backend represents booleans as
`I64` to keep all scalar locals a single width).

### 6.3 Logical

`&&` `||` `!`. Note: in the v0.1 MIR lowering `&&` and `||` map to the
bitwise `BitAnd`/`BitOr` MIR ops (the lowerer's `map_binop`), so evaluate
both operands; short-circuit is not yet implemented in stage-0.

### 6.4 Bitwise

`&` `|` `^` `<<` `>>`. `>>` is arithmetic (signed) for signed types and
logical (unsigned) for unsigned types.

### 6.5 Unary

`!x` (bitwise-not in the backend), `-x` (negation).

---

## 7. Statements

### 7.1 Bindings and assignment

```av
:x = 10            // owner binding (mutable)
::pi = 3.14        // const binding (immutable)
~m = value         // magnet binding
x = 5              // assignment
x += 1             // compound assignment
x!                 // explicit early drop (E4003 if already dropped)
```

Compound assignment operators (`src/ast/stmt.rs`):

```
=  +=  -=  *=  /=  %=
&=  |=  ^=  <<=  >>=
```

The v0.1 lowerer lowers `+=` and `-=` to a read-modify-write; other
compound forms fall back to a plain assignment in stage-0.

Example — `examples/fib.av`:

```av
while i < 10 will
    t = a + b
    a = b
    b = t
    i += 1
finish
```

### 7.2 `if` / `else if` / `else`

Full chains are supported. Each branch body is a `will … finish`-style
block; the whole construct is closed by a single `finish`.

```av
if cond will
    ...
else if cond2 will
    ...
else
    ...
finish
```

(From `examples/cond.av`.) An `else` may be followed directly by a single
statement (no `will`) or by `will … ` for a block.

### 7.3 `while`

```av
while cond will
    ...
finish
```

### 7.4 `for` (counted, exclusive upper bound)

```av
for i in lo..hi will
    ...
finish
```

`i` runs from `lo` to `hi - 1` inclusive. The loop variable is a fresh
mutable local. Optional mode prefixes `&!` (mutable borrow iteration) and
`^` (move iteration) are parsed.

`examples/sumto.av`:

```av
:sum = 0
for i in 1..101 will
    sum += i
finish
print("sum 1..100 =", sum)     // 5050
```

### 7.5 `loop` (infinite)

```av
loop will
    ...
finish
```

Exits only via `break` or `return`.

### 7.6 `break` and `continue`

Affect the innermost enclosing `while`, `for`, or `loop`. They are lowered
to jumps to the loop's exit or header block respectively; a dead block is
emitted after them so subsequent statements remain well-formed.

### 7.7 `return`

```av
return expr
return            // unit-returning action
```

Every non-`Unit` action must `return` a value (enforced by
`validate_return_consistency`).

### 7.8 `match`

```av
match scrutinee will
    pat [if guard] => body
    pat [if guard] => body
    ...
finish
```

- Integer literal patterns: `1 =>`, `42 =>` (lowered to a `SwitchInt`
  terminator with those values as targets).
- Wildcard `_` and binding `name` patterns act as the catch-all arm.
- Variant patterns `.some(x)` and `.none` are parsed and mapped to a
  stable integer discriminant (an FNV-style name hash) for `SwitchInt`.
- An optional `if guard` after the pattern is parsed.
- An arm body is either a single statement or a `will … [finish]` block.

`examples/match_int.av`:

```av
:n = 3
match n will
    1 => print("one")
    2 => print("two")
    _ => print("other")
finish
```

### 7.9 `raw` block

```av
raw will
    ...
finish
```

A `raw` block opts out of certain safety checks (e.g. integer-to-address
conversion, `E4007`). Reserved in the v0.1 backend.

### 7.10 Expression statements

A bare expression on its own line is an expression statement, e.g.
`print(...)`, a function call, or a magnet validity test `m?`.

---

## 8. Built-in I/O

These are provided by the `#std.io` prelude and lowered specially. Full
detail is in [STDLIB.md](STDLIB.md).

| Form | Effect |
|------|--------|
| `print(a, b, …)` | print args space-separated, then a trailing newline |
| `write(a, b, …)` | print args space-separated, **no** trailing newline |
| `eprint(a, b, …)` | print to stderr, then a trailing newline |
| `input()` | read one integer from stdin, return it |

`print`/`write`/`eprint` accept any mix of string and integer arguments;
booleans print as `true`/`false`; chars print as one byte; floats print
via `%g`. `print` of an integer calls the runtime `avera_print_i64`.

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

With `42` on stdin this prints:

```
Enter a number:
You entered: 42
Doubled: 84
```

---

## 9. Attributes

Attributes use `@` and may carry string/ident/int arguments:

```av
@test
@link("printf")
@layout(C)
```

They attach to items and to `foreign` declarations (the `@link("name")`
form overrides the symbol an imported function links against).

---

## 10. Formatting

`avera fmt` re-emits a parsed module in canonical form: 4-space indentation,
`will` / `finish` on their own lines, one statement per line, canonical
spacing around operators. The formatter is **idempotent**: applying it
twice produces the same output as applying it once. Run it with
`avera fmt <files>`; verify without writing with `avera fmt --check <files>`
(exits non-zero if any file would change).

See [COMPILER.md](COMPILER.md) for the formatter's place in the pipeline.
