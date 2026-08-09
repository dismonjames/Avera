use avera_compiler::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use avera_compiler::diagnostics::emitter::render_plain;
use avera_compiler::diagnostics::span::{SourceMap, Span};

#[test]
fn related_span_uses_its_own_file() {
    let mut map = SourceMap::new();
    let first = map.load_str("first.av", "first\n");
    let second = map.load_str("second.av", "line one\nline two\n");

    let mut diags = DiagnosticList::new();
    diags.push(
        Diagnostic::error(
            DiagnosticKind::EDuplicate,
            Span::new(first, 0, 5),
            "duplicate",
        )
        .with_related(Span::new(second, 9, 13), "previous definition here"),
    );

    let rendered = render_plain(&map, &diags);
    assert!(rendered.contains("first.av:1:1"), "{rendered}");
    assert!(rendered.contains("second.av:2:1"), "{rendered}");
    assert!(rendered.contains("line two"), "{rendered}");
}
