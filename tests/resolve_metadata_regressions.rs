use avera_compiler::diagnostics::span::SourceMap;
use avera_compiler::lexer::Lexer;
use avera_compiler::parser::parse;
use avera_compiler::resolve::defs::{DefClass, Defs};
use avera_compiler::resolve::resolve_module;
use avera_compiler::types::ty::DefKind;

#[test]
fn action_definition_has_action_metadata() {
    let source = "action answer(): I32 will\n    return 42\nfinish\n";
    let mut map = SourceMap::new();
    let file = map.load_str("action.av", source);
    let lexed = Lexer::new(file, source).lex();
    assert!(!lexed.diags.has_errors());
    let parsed = parse(file, lexed.tokens);
    assert!(!parsed.diags.has_errors());

    let mut defs = Defs::new();
    let resolved = resolve_module(&mut defs, file, &parsed.module);
    assert!(!resolved.diags.has_errors());

    let info = defs.find("answer").expect("action definition missing");
    assert!(matches!(info.class, DefClass::Action));
    assert!(matches!(info.kind, DefKind::Action { .. }));
}
