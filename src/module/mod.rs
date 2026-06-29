use crate::ast::DirectiveKind;
use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::symbol::FileId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct ModuleDescriptor {
    pub name: String,
    pub sources: Vec<String>,
    pub depends: Vec<String>,
    pub cfg: Vec<String>,
    pub native_links: Vec<String>,
}

impl ModuleDescriptor {
    pub fn parse(file: FileId, src: &str, _path: &Path) -> (Self, DiagnosticList) {
        let mut desc = ModuleDescriptor::default();
        let mut diags = DiagnosticList::new();
        for line in src.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix('#') {
                let mut parts = rest.split_whitespace();
                let kind = parts.next().unwrap_or("");
                match kind {
                    "module" => desc.name = parts.next().unwrap_or("").to_string(),
                    "source" => {
                        if let Some(s) = parts.next() {
                            desc.sources.push(strip_quotes(s));
                        }
                    }
                    "depends" => {
                        if let Some(s) = parts.next() {
                            desc.depends.push(s.to_string());
                        }
                    }
                    "cfg" => {
                        if let Some(s) = parts.next() {
                            desc.cfg.push(s.to_string());
                        }
                    }
                    "link" => {
                        if let Some(s) = parts.next() {
                            desc.native_links.push(strip_quotes(s));
                        }
                    }
                    _ => {}
                }
            }
        }
        if desc.name.is_empty() {
            diags.push(Diagnostic::error(
                DiagnosticKind::EMissingModule,
                crate::diagnostics::span::Span::point(file, 0),
                "module contract missing `#module`",
            ));
        }
        (desc, diags)
    }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

pub const STD_MODULES: &[&str] = &[
    "std.core",
    "std.memory",
    "std.collections",
    "std.io",
    "std.fs",
    "std.process",
    "std.os",
    "std.text",
    "std.hash",
    "std.format",
];

#[derive(Default)]
pub struct ModuleGraph {
    pub modules: HashMap<String, LoadedModule>,
}

#[derive(Clone, Debug)]
pub struct LoadedModule {
    pub name: String,
    pub descriptor: ModuleDescriptor,
    pub dir: PathBuf,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, m: LoadedModule) {
        self.modules.insert(m.name.clone(), m);
    }
}

pub fn detect_cycles(graph: &ModuleGraph) -> Option<Vec<String>> {
    fn visit(
        graph: &ModuleGraph,
        name: &str,
        stack: &mut Vec<String>,
        visited: &mut HashMap<String, bool>,
        on_stack: &mut HashMap<String, bool>,
    ) -> Option<Vec<String>> {
        if let Some(&true) = on_stack.get(name) {
            let pos = stack.iter().position(|n| n == name)?;
            return Some(stack[pos..].to_vec());
        }
        if let Some(&true) = visited.get(name) {
            return None;
        }
        visited.insert(name.to_string(), true);
        on_stack.insert(name.to_string(), true);
        stack.push(name.to_string());
        if let Some(m) = graph.modules.get(name) {
            for dep in &m.descriptor.depends {
                if (graph.modules.contains_key(dep) || STD_MODULES.contains(&dep.as_str()))
                    && visit(graph, dep, stack, visited, on_stack).is_some()
                {
                    return Some(stack.clone());
                }
            }
        }
        on_stack.insert(name.to_string(), false);
        stack.pop();
        None
    }

    let mut visited = HashMap::new();
    let mut on_stack = HashMap::new();
    let mut stack = Vec::new();
    for name in graph.modules.keys() {
        if let Some(cycle) = visit(graph, name, &mut stack, &mut visited, &mut on_stack) {
            return Some(cycle);
        }
    }
    None
}

pub fn import_to_module_name(d: &DirectiveKind) -> Option<String> {
    match d {
        DirectiveKind::Import { path, .. } => {
            let segs = &path.segments;
            if segs.len() >= 2 {
                Some(format!("{}.{}", segs[0], segs[1]))
            } else if !segs.is_empty() {
                Some(segs[0].clone())
            } else {
                None
            }
        }
        _ => None,
    }
}
