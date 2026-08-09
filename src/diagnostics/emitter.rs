use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticList, Severity};
use crate::diagnostics::span::{SourceMap, Span};

pub struct Emitter<'a> {
    pub map: &'a SourceMap,
    pub color: bool,
}

impl<'a> Emitter<'a> {
    pub fn new(map: &'a SourceMap) -> Self {
        Self {
            map,
            color: std::io::IsTerminal::is_terminal(&std::io::stderr()),
        }
    }
    pub fn no_color(map: &'a SourceMap) -> Self {
        Self { map, color: false }
    }
    pub fn render(&self, list: &DiagnosticList) -> String {
        let mut out = String::new();
        for d in &list.items {
            self.render_one(d, &mut out);
            out.push('\n');
        }
        if !list.items.is_empty() {
            let errors = list.error_count();
            let warns = list.warn_count();
            let reset = if self.color { "\u{1b}[0m" } else { "" };
            if errors > 0 {
                let col = if self.color {
                    Severity::Error.ansi()
                } else {
                    ""
                };
                out.push_str(&format!("{}{} error(s){}\n", col, errors, reset));
            }
            if warns > 0 {
                let col = if self.color {
                    Severity::Warning.ansi()
                } else {
                    ""
                };
                out.push_str(&format!("{}{} warning(s){}\n", col, warns, reset));
            }
        }
        out
    }

    fn render_one(&self, d: &Diagnostic, out: &mut String) {
        let file = self.map.file(d.span.file);
        let lc = file.line_col(d.span.start);
        let sev = d.severity.label();
        let reset = if self.color { "\u{1b}[0m" } else { "" };
        let sev_color = if self.color { d.severity.ansi() } else { "" };
        out.push_str(&format!("{}{}{}: {}\n", sev_color, sev, reset, d.message));
        out.push_str(&format!(
            "  {} {}:{}:{}\n",
            "-->", file.name, lc.line, lc.col
        ));
        self.render_snippet(d.span, &d.message, out, true);
        for r in &d.related {
            let related_file = self.map.file(r.span.file);
            let rlc = related_file.line_col(r.span.start);
            out.push_str(&format!(
                "  {} {}:{}:{}\n",
                "-->", related_file.name, rlc.line, rlc.col
            ));
            self.render_snippet(r.span, &r.message, out, false);
        }
        if let Some(s) = &d.suggestion {
            out.push_str(&format!("  {} suggestion: {}\n", "=", s));
        }
    }

    fn render_snippet(&self, span: Span, msg: &str, out: &mut String, primary: bool) {
        let file = self.map.file(span.file);
        let lc = file.line_col(span.start);
        let line_no = lc.line;
        let line_text = file.line_text(line_no);
        let gutter_width = line_no.to_string().len().max(3);
        let pad = " ".repeat(gutter_width);
        out.push_str(&format!("{}{} {}\n", pad, "|", line_text));
        let col0 = (lc.col as usize).saturating_sub(1);
        let line_start =
            (file.line_starts[(line_no as usize).saturating_sub(1)] as usize).min(file.src.len());
        let line_end = if (line_no as usize) < file.line_starts.len() {
            (file.line_starts[line_no as usize] as usize)
                .saturating_sub(1)
                .min(file.src.len())
                .max(line_start)
        } else {
            file.src.len()
        };
        let span_end_in_line = (span.end as usize).clamp(line_start, line_end);
        let span_start_in_line = (span.start as usize).clamp(line_start, line_end);
        let width = span_end_in_line.saturating_sub(span_start_in_line).max(1);
        let accent = if primary { "^" } else { "-" };
        let underline: String = accent.repeat(width);
        let marker_color = if self.color {
            if primary {
                "\u{1b}[31m"
            } else {
                "\u{1b}[36m"
            }
        } else {
            ""
        };
        let reset = if self.color { "\u{1b}[0m" } else { "" };
        out.push_str(&format!(
            "{}{} {}{}{}{} {}\n",
            pad,
            "|",
            " ".repeat(col0),
            marker_color,
            underline,
            reset,
            msg
        ));
    }
}

pub fn render_plain(map: &SourceMap, list: &DiagnosticList) -> String {
    Emitter::no_color(map).render(list)
}
