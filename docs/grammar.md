# Avera Grammar (EBNF)

This is the context-free grammar accepted by the `avera` parser for the
stage-0 language. It is derived directly from the recursive-descent
parser in [`src/parser/`](../src/parser/) and the token set in
[`src/lexer/token.rs`](../src/lexer/token.rs). Productions are written in
ISO EBNF (`{ x }` = zero or more, `[ x ]` = optional, `x | y` =
alternative). Terminals are quoted literally or named in `UPPER_CASE`.

> For the meaning of each construct, see [LANGUAGE.md](LANGUAGE.md).

---

## 1. Lexical grammar

```ebnf
Source        = { Item | Directive | Newline } .

Newline       = "significant newline at bracket-depth 0" | ";" .
Whitespace    = " " | "\t" | "\r" .                    (* ignored *)
Comment       = "//" { any - "\n" }
              | "/*" { any - "*/" } "*/" .              (* non-nesting *)
```

### 1.1 Tokens

```ebnf
Token        = Ident | Keyword
             | IntLit | FloatLit | StringLit | CharLit
             | Punct | Newline | Eof .

Ident        = ( "_" | Letter ) { "_" | Letter | Digit } .
Keyword      = "if" | "else" | "while" | "loop" | "for" | "in" | "match"
             | "action" | "shape" | "choice" | "ability" | "implement"
             | "foreign" | "export" | "return" | "break" | "continue"
             | "will" | "finish" | "raw" | "self" | "true" | "false"
             | "none" .

IntLit       = ( "0x" HexDigit { HexDigit | "_" }
              | "0b" Bit { Bit | "_" }
              | Digit { Digit | "_" } ) .
FloatLit     = Digit { Digit | "_" }
                 [ "." Digit { Digit | "_" } ]
                 [ ("e"|"E") ["+"|"-"] Digit { Digit | "_" } ] .
StringLit    = '"' { Char | Escape | RawByte } '"' .
CharLit      = "'" ( Char | Escape ) "'" .
Escape       = "\" ( "n" | "t" | "r" | "\" | '"' | "'" | "0"
                     | "x" HexDigit HexDigit
                     | "u" "{" HexDigit { HexDigit } "}" ) .
```

### 1.2 Punctuation and operators

```ebnf
Punct        = "::" | ":>" | "->" | "=>" | "==" | "!=" | "<=" | ">="
             | "&&" | "||" | "<<" | ">>" | "+=" | "-=" | "*=" | "/="
             | "%=" | "|=" | "&=" | "^=" | "~!" | ".." | "&!"
             | ":"  | ";"  | ","  | "."  | "("  | ")"  | "["  | "]"
             | "<"  | ">"  | "="  | "+"  | "-"  | "*"  | "/"  | "%"
             | "&"  | "|"  | "^"  | "!"  | "~"  | "?"  | "@"  | "#" | "$" .
```

Multi-character operators are matched before single-character operators.
Newlines inside `(`…`)` or `[`…`]` are **insignificant** (the lexer tracks
bracket depth); newlines at depth 0 act as statement separators unless
suppressed by a line-continuation operator.

---

## 2. Module structure

```ebnf
Module       = Directive* Item* .
Directive    = "#" ( Import | "module" Path | "header" StringLit
                   | "source" StringLit | "depends" Path
                   | "cfg" Ident | "target" Ident | "link" StringLit ) .

Import       = PathSeg { "." PathSeg } [ "as" Ident ] .     (* first seg is the
                                                            directive name; a
                                                            trailing "*" is a
                                                            wildcard import *)
PathSeg      = Ident | "*" .
Path         = Ident { "." Ident } .
```

`#std.io`, `#std.fs as fs`, `#std.collections.*` are imports. Directives
must precede all items.

---

## 3. Items

```ebnf
Item         = Attr* [ "export" ] ItemKind .
Attr         = "@" Ident [ "(" AttrArg { "," AttrArg } ")" ] .
AttrArg      = Ident | StringLit | IntLit .
ItemKind     = Shape | Choice | Ability | Impl | Action | Foreign .
```

### 3.1 Shapes, choices, abilities

```ebnf
Shape        = "shape" Ident GenericParams? "will"
                 { Ident ":" Type }+
                 "finish" .
Choice       = "choice" Ident GenericParams? "will"
                 { VariantDecl }+
                 "finish" .
VariantDecl  = Ident [ "(" PayloadField { "," PayloadField } ")" ] .
PayloadField = Ident ":" Type .
Ability      = "ability" Ident GenericParams? "will"
                 { "action" ActionSig }+
                 "finish" .
GenericParams = "<" Ident { "," Ident [ ":" Type ] } ">" .
```

### 3.2 Implementations and foreign blocks

```ebnf
Impl         = "implement" Type [ "for" Type ] GenericParams? "will"
                 { "action" ActionSig [ "will" Block "finish" ] }+
                 "finish" .
Foreign      = "foreign" Ident [ "(" StringLit ")" ] "will"
                 { [ "@" "link" "(" StringLit ")" ]
                   [ "raw" ] "action" ActionSig }+
                 "finish" .
```

### 3.3 Actions

```ebnf
Action       = "action" ActionSig [ "will" Block "finish" ] .
ActionSig    = [ Ident "." ] Ident                     (* Type.method receiver *)
                 [ "(" [ Param { "," Param } ] ")" ]
                 [ ":" Type ] .
Param        = ( "&" "!"? | "^" )? ( "self" | Ident ) ":" Type .
```

The optional receiver `Ident "."` makes `User.greet(...)` a method of
type `User`. Parameter modes: plain (value/take), `&` (read borrow), `&!`
(mutable borrow), `^` (move); `self` receiver forms are recognised but
not in the v0.1 proven-working set (see [README.md](README.md)).

---

## 4. Statements and blocks

```ebnf
Block        = Stmt* .                                  (* a "will … finish" body *)
Stmt         = BindingStmt | ControlStmt | JumpStmt
             | RawStmt | AssignStmt | DropStmt
             | RetargetStmt | ExprStmt .

BindingStmt  = OwnerBinding | ConstBinding | MagnetBinding .
OwnerBinding = ":"  Ident [ ":" Type ] "=" Expr .
ConstBinding = "::" Ident [ ":" Type ] "=" Expr .
MagnetBinding= ( "~" | "~!" ) Ident [ ":" Type ] "=" Expr .

ControlStmt  = If | While | Loop | For | Match .
JumpStmt     = "return" Expr? | "break" | "continue" .
RawStmt      = "raw" "will" Block "finish" .

If           = "if" Expr "will" Block
                 { "else" "if" Expr "will" Block }
                 [ "else" [ "will" ] ( Block | Stmt ) ]
                 "finish" .
While        = "while" Expr "will" Block "finish" .
Loop         = "loop" "will" Block "finish" .
For          = "for" ( "&" "!"? | "^" )? Pat "in" Iter "will" Block "finish" .
Iter         = Expr [ ".." Expr ] .                     (* Range, exclusive upper *)
Match        = "match" Expr "will" { MatchArm } "finish" .
MatchArm     = Pat [ "if" Expr ] "=>" ArmBody .
ArmBody      = "will" Block "finish"?
             | Stmt .

AssignStmt   = Expr AssignOp Expr .
AssignOp     = "=" | "+=" | "-=" | "*=" | "/=" | "%="
             | "&=" | "|=" | "^=" | "<<=" | ">>=" .
DropStmt     = Expr "!" .
RetargetStmt = Expr "->" Expr .                         (* magnet retarget *)
ExprStmt     = Expr .
```

Notes:

- `for` accepts an optional mode prefix `&!` (mutable borrow iteration) or
  `^` (move iteration); the default is read iteration.
- The match arm body is either a `will … [finish]` block or a single
  statement. The `finish` of a `will` arm is optional because arm blocks
  end at the next arm or the enclosing `finish`.
- `<<=` / `>>=` appear in the formatter but v0.1 only lexes `<<` and `>>`
  (not `<<=` / `>>=`), so they are reserved in the parser.

---

## 5. Patterns

```ebnf
Pat          = VariantPat | BindPat | WildPat | IntPat .
VariantPat   = "." Ident [ "(" PatBind { "," PatBind } ")" ] .  (* payload *)
             | "." Ident .                                      (* no payload *)
PatBind      = Ident .
BindPat      = Ident .                                    (* not "_" *)
WildPat      = "_" .                                      (* lexed as Ident "_" *)
IntPat       = IntLit .                                   (* hex/binary/decimal *)
```

A `BindPat` acts as the catch-all arm of a `match`; the lowerer uses it as
the `SwitchInt` `otherwise` target when present. Integer patterns become
`SwitchInt` cases; variant patterns map to a stable name-hash discriminant.

---

## 6. Types

```ebnf
Type         = BorrowTy | ArrayTy | TupleTy | SelfTy | NamedTy .

BorrowTy     = "&" "!"? Type .                            (* &T  or  &!T *)
ArrayTy      = "[" Type ";" Expr "]" .                     (* [T; N] *)
TupleTy      = "(" [ Type { "," Type } ] ")" .             (* () is Unit *)
SelfTy       = "self" .                                   (* inside an impl *)
NamedTy      = Path TypeArgs? .
TypeArgs     = "<" Type { "," Type } ">" .
```

The parser specialises the common constructors: `Magnet<T>`,
`MagnetMut<T>` (note: written as `Magnet<T>` with a `~!`-created handle —
the mutability comes from the binding form, not the type name),
`Address<T>`, `Span<T>`, `Maybe<T>`, and `Outcome<T, E>` are recognised by
their last path segment and arity and produce structured `TyKind`s
directly. `Unit` is a named type. Primitive names (`Bool`, `I32`, `F64`,
`Size`, `Text`, …) are recognised during type lowering.

---

## 7. Expressions

Expressions are parsed by a Pratt parser with the precedence table below
(higher = binds tighter). Binary operators are left-associative; unary
prefix operators bind tighter than any binary operator.

```ebnf
Expr         = Binary .
Binary       = Unary ( BinOp Binary )* .                  (* left-assoc *)
BinOp        = "*" | "/" | "%"                            (* prec 9 *)
             | "+" | "-"                                 (* prec 8 *)
             | "<<" | ">>"                               (* prec 7 *)
             | "&"                                       (* prec 6 *)
             | "^"                                       (* prec 5 *)
             | "|"                                       (* prec 4 *)
             | "==" | "!=" | "<" | "<=" | ">" | ">="      (* prec 3 *)
             | "&&"                                      (* prec 2 *)
             | "||" .                                    (* prec 1 *)

Unary        = "!" Unary                                  (* logical/bitwise not *)
             | "-" Unary                                 (* negation *)
             | "&" "!"? Unary                            (* borrow / borrow-mut *)
             | "^" Unary                                 (* move *)
             | "~" Ident ( "." Ident )?                   (* ~m or ~m.address *)
             | Postfix .

Postfix      = Primary { PostfixOp } .
PostfixOp    = "." "~" Ident                              (* magnet meta *)
             | "." Ident "(" Args? ")"                   (* method call *)
             | "." Ident                                 (* field access *)
             | "[" [ Expr ] ".." [ Expr ] "]"             (* slice *)
             | "[" Expr "]"                              (* index *)
             | "(" Args? ")"                             (* call *)
             | ":>" Type                                 (* cast *)
             | "?" .                                     (* try / magnet validity *)

Primary      = Literal
             | "self"
             | "none"
             | "." Ident "(" Args? ")"                    (* choice ctor with args *)
             | "." Ident                                 (* no-payload choice ctor *)
             | "(" [ Expr ] ")"                           (* () is Unit *)
             | "[" [ Expr { "," Expr } ] "]"              (* array literal *)
             | Path .

Literal      = IntLit | FloatLit | StringLit | CharLit | "true" | "false" .
Path         = Ident { "." Ident } .
Args        = Arg { "," Arg } .
Arg          = Expr .                                     (* positional; name=expr
                                                            is detected for shape
                                                            ctors *)
```

### 7.1 Precedence and associativity

| Prec | Operators | Associativity |
|------|-----------|---------------|
| 9 | `*` `/` `%` | left |
| 8 | `+` `-` | left |
| 7 | `<<` `>>` | left |
| 6 | `&` | left |
| 5 | `^` | left |
| 4 | `\|` | left |
| 3 | `==` `!=` `<` `<=` `>` `>=` | left |
| 2 | `&&` | left |
| 1 | `\|\|` | left |

Unary prefix (`! - & &! ^ ~`) binds tighter than any binary operator.
Postfix (`.field`, `.method()`, `[i]`, `[a..b]`, `(args)`, `:> T`, `?`,
`~x.address`) binds tighter than unary prefix.

### 7.2 Choice construction

A path `P.variant(args)` or `P.variant` (no payload) is recognised in
`parse_primary` as a `ChoiceCtor` if the next token after the dot is an
identifier followed by `(` or a statement terminator. The leading-dot
form `.some(x)` constructs a choice with an elided type.

### 7.3 v0.1 lowering notes

- `&&` and `||` lower to the MIR `BitAnd`/`BitOr` ops in stage-0 (no
  short-circuit yet); see [MIR.md](MIR.md) and [LANGUAGE.md](LANGUAGE.md#63-logical).
- `expr?` is parsed as `Try`; when the receiver is a bare magnet name it
  is interpreted as a magnet validity test by later phases.
- `expr :> Type` lowers to a Cranelift `uextend` in the v0.1 backend.

---

## 8. Statement separation

Avera uses **significant newlines** (and `;`) as statement separators at
bracket depth 0. The lexer suppresses newlines inside `(`…`)` and
`[`…`]`, and suppresses a newline following a line-continuation operator
(an operator/punct whose `continues_line()` is true — essentially all
infix operators, `,`, `.`, `..`, `->`, `=>`, `:`, `::`, `~`, `~!`, `&!`,
`@`, `#`). The parser's `eat_separator` accepts a newline or `;`, and
permits no separator immediately before `finish`, `else`, or EOF.

---

## 9. Worked grammar trace

`examples/primes.av` exercises most of the surface:

```av
#std.io

action main(): I32 will
    for n in 2..21 will
        :d = 2
        while n % d != 0 will
            d += 1
        finish
        if d == n will
            print(n, "is prime")
        else
            print(n, "=", d, "*", n / d)
        finish
    finish
    return 0
finish
```

- `#std.io` — `Directive` → `Import`.
- `action main(): I32 will … finish` — `Item` → `Action` with
  `ActionSig` `main(): I32` and a `Block`.
- `for n in 2..21 will … finish` — `For` with a `Range` `Iter`
  (exclusive upper bound 21).
- `:d = 2` — `OwnerBinding`.
- `while n % d != 0 will … finish` — `While`; the condition is
  `Binary(%, n, d) != 0` (prec 3 `<` prec 9 `%`).
- `d += 1` — `AssignStmt` with `AssignOp` `+=`.
- `if d == n will … else … finish` — `If` with one branch and an `else`.
- `print(n, "is prime")` — `ExprStmt` → `Call` of the built-in `print`.
- `return 0` — `JumpStmt`.
