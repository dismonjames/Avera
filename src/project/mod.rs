use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Manifest {
    pub project: ProjectSection,
    pub build: BuildSection,
    pub dependencies: Vec<Dependency>,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    pub entry: String,
}

#[derive(Clone, Debug)]
pub struct BuildSection {
    pub target: String,
    pub opt: u32,
}

impl Default for BuildSection {
    fn default() -> Self {
        Self {
            target: "native".to_string(),
            opt: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Dependency {
    pub name: String,
    pub path: PathBuf,
}

pub fn parse_manifest(path: &Path, src: &str) -> (Manifest, DiagnosticList) {
    let mut diags = DiagnosticList::new();
    let mut m = Manifest {
        project: ProjectSection::default(),
        build: BuildSection::default(),
        dependencies: Vec::new(),
        path: path.to_path_buf(),
    };
    let mut section = String::new();
    for line in src.lines() {
        let raw = line.trim();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        if raw.starts_with('[') && raw.ends_with(']') {
            section = raw[1..raw.len() - 1].to_string();
            continue;
        }
        if let Some((k, v)) = raw.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"');
            match section.as_str() {
                "project" => match k {
                    "name" => m.project.name = v.to_string(),
                    "version" => m.project.version = v.to_string(),
                    "entry" => m.project.entry = v.to_string(),
                    _ => {}
                },
                "build" => match k {
                    "target" => m.build.target = v.to_string(),
                    "opt" => m.build.opt = v.parse().unwrap_or(0),
                    _ => {}
                },
                "dependencies" => {
                    // `name = { path = "../foo" }`
                    if let Some(rest) = v.strip_prefix('{') {
                        let rest = rest.trim_end_matches('}');
                        let mut dep_path = PathBuf::new();
                        for pair in rest.split(',') {
                            let pair = pair.trim();
                            if let Some((pk, pv)) = pair.split_once('=') {
                                let pk = pk.trim();
                                let pv = pv.trim().trim_matches('"');
                                if pk == "path" {
                                    dep_path = PathBuf::from(pv);
                                }
                            }
                        }
                        m.dependencies.push(Dependency {
                            name: k.to_string(),
                            path: dep_path,
                        });
                    }
                }
                _ => {}
            }
        } else {
            diags.push(Diagnostic::new(
                DiagnosticKind::Warning,
                crate::diagnostics::span::Span::point(crate::symbol::FileId::new(0), 0),
                format!("ignoring malformed manifest line: `{}`", raw),
            ));
        }
    }
    if m.project.name.is_empty() {
        diags.push(Diagnostic::error(
            DiagnosticKind::EMissingModule,
            crate::diagnostics::span::Span::point(crate::symbol::FileId::new(0), 0),
            "manifest is missing `[project] name`",
        ));
    }
    (m, diags)
}

pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("avera.toml");
        if candidate.is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}
