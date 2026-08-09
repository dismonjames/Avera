use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::span::Span;
use crate::symbol::FileId;
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
    let mut manifest = Manifest {
        project: ProjectSection::default(),
        build: BuildSection::default(),
        dependencies: Vec::new(),
        path: path.to_path_buf(),
    };
    let mut section = String::new();

    for (line_index, line) in src.lines().enumerate() {
        let raw = line.trim();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        if raw.starts_with('[') && raw.ends_with(']') {
            section = raw[1..raw.len() - 1].trim().to_string();
            if !matches!(section.as_str(), "project" | "build" | "dependencies") {
                manifest_error(
                    &mut diags,
                    line_index,
                    format!("unknown manifest section `[{}]`", section),
                );
            }
            continue;
        }

        let Some((key, value)) = raw.split_once('=') else {
            manifest_error(
                &mut diags,
                line_index,
                format!("malformed manifest line `{raw}`; expected `key = value`"),
            );
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let plain_value = value.trim_matches('"');

        match section.as_str() {
            "project" => match key {
                "name" => manifest.project.name = plain_value.to_string(),
                "version" => manifest.project.version = plain_value.to_string(),
                "entry" => manifest.project.entry = plain_value.to_string(),
                _ => manifest_error(
                    &mut diags,
                    line_index,
                    format!("unknown `[project]` key `{key}`"),
                ),
            },
            "build" => match key {
                "target" => manifest.build.target = plain_value.to_string(),
                "opt" => match plain_value.parse::<u32>() {
                    Ok(opt @ 0..=2) => manifest.build.opt = opt,
                    Ok(other) => manifest_error(
                        &mut diags,
                        line_index,
                        format!("invalid optimization level `{other}`; expected 0, 1, or 2"),
                    ),
                    Err(_) => manifest_error(
                        &mut diags,
                        line_index,
                        format!("invalid optimization level `{plain_value}`; expected 0, 1, or 2"),
                    ),
                },
                _ => manifest_error(
                    &mut diags,
                    line_index,
                    format!("unknown `[build]` key `{key}`"),
                ),
            },
            "dependencies" => {
                let Some(rest) = value.strip_prefix('{') else {
                    manifest_error(
                        &mut diags,
                        line_index,
                        format!("dependency `{key}` must use `{{ path = \"...\" }}`"),
                    );
                    continue;
                };
                let Some(rest) = rest.strip_suffix('}') else {
                    manifest_error(
                        &mut diags,
                        line_index,
                        format!("dependency `{key}` is missing closing `}}`"),
                    );
                    continue;
                };
                let mut dep_path = None;
                for pair in rest.split(',') {
                    let pair = pair.trim();
                    if pair.is_empty() {
                        continue;
                    }
                    let Some((dep_key, dep_value)) = pair.split_once('=') else {
                        manifest_error(
                            &mut diags,
                            line_index,
                            format!("malformed dependency field `{pair}`"),
                        );
                        continue;
                    };
                    let dep_key = dep_key.trim();
                    let dep_value = dep_value.trim().trim_matches('"');
                    match dep_key {
                        "path" if !dep_value.is_empty() => {
                            dep_path = Some(PathBuf::from(dep_value));
                        }
                        "path" => manifest_error(
                            &mut diags,
                            line_index,
                            format!("dependency `{key}` has an empty path"),
                        ),
                        _ => manifest_error(
                            &mut diags,
                            line_index,
                            format!("unknown dependency field `{dep_key}`"),
                        ),
                    }
                }
                if let Some(dep_path) = dep_path {
                    manifest.dependencies.push(Dependency {
                        name: key.to_string(),
                        path: dep_path,
                    });
                } else {
                    manifest_error(
                        &mut diags,
                        line_index,
                        format!("dependency `{key}` is missing `path`"),
                    );
                }
            }
            "" => manifest_error(
                &mut diags,
                line_index,
                format!("manifest key `{key}` appears before any section"),
            ),
            _ => {
                // The unknown section itself is already diagnosed. Keep its
                // contents invalid instead of silently interpreting them.
                manifest_error(
                    &mut diags,
                    line_index,
                    format!("cannot use key `{key}` inside unknown section `[{}]`", section),
                );
            }
        }
    }

    if manifest.project.name.is_empty() {
        manifest_error(
            &mut diags,
            0,
            "manifest is missing `[project] name`".to_string(),
        );
    }
    (manifest, diags)
}

fn manifest_error(diags: &mut DiagnosticList, line_index: usize, message: String) {
    // The manifest parser is intentionally source-map independent today. Keep
    // a stable byte-ish position so callers can still sort diagnostics, but do
    // not downgrade malformed configuration to a warning.
    let pos = line_index.min(u32::MAX as usize) as u32;
    diags.push(Diagnostic::error(
        DiagnosticKind::Parse,
        Span::point(FileId::new(0), pos),
        message,
    ));
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
