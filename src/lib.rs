// Lib root cho avera compiler

#![allow(clippy::needless_return)]
#![allow(clippy::module_inception)]
#![allow(unknown_lints)]
#![allow(rust_2024_compatibility)]
#![allow(edition_2024_expr_fragment_specifier)]
// Tat canh bao clippy ko can thiet
#![allow(clippy::result_unit_err)]
#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::manual_map)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::unused_self)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::manual_repeat)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::manual_slice_fill)]
#![allow(clippy::string_slice)]
#![allow(clippy::iter_overeager_cloned)]
#![allow(clippy::map_values)]
#![allow(clippy::doc_overindented_list_items)]
#![allow(clippy::result_large_err)]
#![allow(clippy::unnecessary_map_or)]

pub mod ast;
pub mod check;
pub mod cli;
pub mod codegen;
pub mod diagnostics;
pub mod driver;
pub mod driver_pipeline;
pub mod fmt;
pub mod lexer;
pub mod mir;
pub mod module;
pub mod parser;
pub mod project;
pub mod resolve;
pub mod symbol;
pub mod types;

pub use diagnostics::{Diagnostic, SourceMap, Span};

pub const AVERA_VERSION: &str = env!("AVERA_BUILD_VERSION");

pub fn diagnostics_explanations(code: &str) -> Option<String> {
    let text = match code {
        "E0001" => "The lexer could not tokenize this character. Check for stray punctuation or invalid UTF-8.",
        "E0100" => "The parser expected a different token. This is a syntax error; the snippet shows where parsing failed.",
        "E0101" => "A legacy Avera file extension was used. Use `.av` for sources and `.mav` for module contracts.",
        "E2001" => "A name was used that is not defined in this scope. Make sure it is bound, declared, or imported.",
        "E2002" => "A declaration or binding with the same name already exists in this scope.",
        "E3001" => "Two types that should match do not. Add a conversion (`:>`) or change one of the types.",
        "E4001" => "A value was used after its ownership was moved. Create another value, borrow it, or change the action so it does not take ownership.",
        "E4002" => "A value was used after being dropped (explicitly or at scope end). Drop points are deterministic in Avera.",
        "E4003" => "A value would be dropped twice. The compiler inserts drops automatically; do not call `!` on an already-dropped binding.",
        "E4004" => "A value was moved while a borrow or Magnet still depended on it. Detach the Magnet or let the borrow expire first.",
        "E4005" => "Two mutable accesses to the same place overlap. Mutable access is exclusive in Avera.",
        "E4006" => "An immutable binding was mutated, or a borrow escaped its owner's lifetime.",
        "E4007" => "An integer was converted to an address in safe code. Use a `raw` block for this operation.",
        "E4008" => "Magnet arithmetic went out of the span. Use `raw` to bypass the check or adjust the offset.",
        "E4009" => "A container was resized while a Magnet pointed inside it. End the Magnet lifetime first.",
        "E5001" => "The match did not cover all variants. Add a wildcard arm or handle every variant of the choice.",
        "E5002" => "A shape contains itself by value, leading to infinite size. Use `Box`, `Magnet`, or `Maybe`.",
        "E5003" => "An integer literal does not fit in the target type, or an implicit narrowing conversion was attempted. Use `:>` to convert explicitly.",
        "E6001" => "A private symbol was accessed. Export it in the module's `.mav` file or use a different symbol.",
        "E6002" => "The implementation in `.av` does not match the signature declared in the `.mav` module contract.",
        "E6003" => "An imported symbol is ambiguous. Use an alias (`as`) to disambiguate.",
        "E6004" => "A module referenced by an import could not be found.",
        "E6005" => "A dependency cycle was detected in the module graph.",
        "E7001" => "A resource parameter was declared without an ownership modifier. Use `^`, `&`, or `&!`.",
        "E9999" => "An internal compiler invariant was violated. This is a bug in `avera`, not your program.",
        "W0001" => "A warning was emitted (e.g. wildcard import). Your program still compiles.",
        _ => return None,
    };
    Some(format!("{}: {}", code, text))
}
