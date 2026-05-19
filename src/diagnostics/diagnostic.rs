use crate::diagnostics::span::Span;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
    pub fn ansi(self) -> &'static str {
        match self {
            Severity::Error => "\u{1b}[31m",
            Severity::Warning => "\u{1b}[33m",
            Severity::Note => "\u{1b}[36m",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DiagnosticKind {
    Lex,
    Parse,
    EUndefinedName,
    EDuplicate,
    ETypeMismatch,
    EUseAfterMove,
    EUseAfterDrop,
    EDoubleDrop,
    EMoveWhileBorrowed,
    EMutableAlias,
    EImmutableMutate,
    EAddrFromInt,
    EMagnetOob,
    ERelocation,
    ENonExhaustive,
    EInfiniteSize,
    ENarrowing,
    EPrivateAccess,
    EHeaderMismatch,
    EAmbiguousImport,
    EMissingModule,
    EDepCycle,
    EBorrowEscape,
    EImplicitOwnership,
    EInternal,
    Warning,
    MirEmptyBody,
    MirInvalidEntry,
    MirBlockIdMismatch,
    MirInvalidLocal,
    MirInvalidTarget,
    MirValidation,
    EMagnetDetached,
    EMagnetUseAfterMove,
    ELegacyExtension,
}

impl DiagnosticKind {
    pub fn code(self) -> &'static str {
        match self {
            DiagnosticKind::Lex => "E0001",
            DiagnosticKind::Parse => "E0100",
            DiagnosticKind::EUndefinedName => "E2001",
            DiagnosticKind::EDuplicate => "E2002",
            DiagnosticKind::ETypeMismatch => "E3001",
            DiagnosticKind::EUseAfterMove => "E4001",
            DiagnosticKind::EUseAfterDrop => "E4002",
            DiagnosticKind::EDoubleDrop => "E4003",
            DiagnosticKind::EMoveWhileBorrowed => "E4004",
            DiagnosticKind::EMutableAlias => "E4005",
            DiagnosticKind::EImmutableMutate => "E4006",
            DiagnosticKind::EAddrFromInt => "E4007",
            DiagnosticKind::EMagnetOob => "E4008",
            DiagnosticKind::ERelocation => "E4009",
            DiagnosticKind::ENonExhaustive => "E5001",
            DiagnosticKind::EInfiniteSize => "E5002",
            DiagnosticKind::ENarrowing => "E5003",
            DiagnosticKind::EPrivateAccess => "E6001",
            DiagnosticKind::EHeaderMismatch => "E6002",
            DiagnosticKind::EAmbiguousImport => "E6003",
            DiagnosticKind::EMissingModule => "E6004",
            DiagnosticKind::EDepCycle => "E6005",
            DiagnosticKind::EBorrowEscape => "E4006",
            DiagnosticKind::EImplicitOwnership => "E7001",
            DiagnosticKind::EInternal => "E9999",
            DiagnosticKind::MirEmptyBody => "E1101",
            DiagnosticKind::MirInvalidEntry => "E1102",
            DiagnosticKind::MirBlockIdMismatch => "E1103",
            DiagnosticKind::MirInvalidLocal => "E1104",
            DiagnosticKind::MirInvalidTarget => "E1105",
            DiagnosticKind::MirValidation => "E1100",
            DiagnosticKind::EMagnetDetached => "E4008",
            DiagnosticKind::EMagnetUseAfterMove => "E4009",
            DiagnosticKind::ELegacyExtension => "E0101",
            DiagnosticKind::Warning => "W0001",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Related {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagnosticKind,
    pub message: String,
    pub span: Span,
    pub related: Vec<Related>,
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Self {
        let severity = match kind {
            DiagnosticKind::Warning => Severity::Warning,
            _ => Severity::Error,
        };
        Self {
            severity,
            kind,
            message: message.into(),
            span,
            related: Vec::new(),
            suggestion: None,
        }
    }
    pub fn error(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            kind,
            message: message.into(),
            span,
            related: Vec::new(),
            suggestion: None,
        }
    }
    pub fn with_related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.related.push(Related {
            span,
            message: message.into(),
        });
        self
    }
    pub fn with_suggestion(mut self, s: impl Into<String>) -> Self {
        self.suggestion = Some(s.into());
        self
    }
}

#[derive(Default, Clone)]
pub struct DiagnosticList {
    pub items: Vec<Diagnostic>,
}

impl DiagnosticList {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, d: Diagnostic) {
        self.items.push(d);
    }
    pub fn extend(&mut self, other: DiagnosticList) {
        self.items.extend(other.items);
    }
    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }
    pub fn error_count(&self) -> usize {
        self.items
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
    pub fn warn_count(&self) -> usize {
        self.items
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }
    pub fn sort(&mut self) {
        self.items.sort_by_key(|a| (a.span.file, a.span.start));
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl std::ops::Deref for DiagnosticList {
    type Target = [Diagnostic];
    fn deref(&self) -> &[Diagnostic] {
        &self.items
    }
}
