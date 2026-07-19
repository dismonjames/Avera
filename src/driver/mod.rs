use crate::ast::Module;
use crate::diagnostics::diagnostic::DiagnosticList;
use crate::diagnostics::span::SourceMap;
use crate::lexer::Lexer;
use crate::mir::body::Body;
use crate::parser::Parsed;
use crate::resolve::Defs;
use crate::types::intern::TyCtxt;

pub struct CompileOutput {
    pub diags: DiagnosticList,
    pub bodies: Vec<Body>,
    pub source_map: SourceMap,
    pub parsed: Option<Parsed>,
}

pub fn parse_source(
    file: crate::symbol::FileId,
    src: &str,
    diags: &mut DiagnosticList,
) -> Option<Parsed> {
    let lexed = Lexer::new(file, src).lex();
    diags.extend(lexed.diags);
    let parsed = crate::parser::parse(file, lexed.tokens);
    diags.extend(parsed.diags.clone());
    Some(parsed)
}

pub fn check_single_source(name: &str, src: &str) -> (SourceMap, DiagnosticList, Option<Module>) {
    let mut map = SourceMap::new();
    let fid = map.load_str(name, src);
    let mut diags = DiagnosticList::new();
    let parsed = parse_source(fid, src, &mut diags);
    let module = parsed.map(|p| p.module);
    (map, diags, module)
}

pub fn fresh_tyctxt() -> TyCtxt {
    TyCtxt::new()
}

pub fn fresh_defs() -> Defs {
    Defs::new()
}
