pub mod lower;
pub mod runtime;

use crate::ast::Module;
use crate::codegen::{self, object};
use crate::diagnostics::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticList};
use crate::diagnostics::emitter::Emitter;
use crate::diagnostics::span::{SourceMap, Span};
use crate::lexer::Lexer;
use crate::mir::body::Body;
use crate::parser::parse;
use crate::resolve::Defs;
use crate::symbol::FileId;
use crate::types::intern::TyCtxt;
use std::path::{Path, PathBuf};

pub const EXIT_OK: u8 = 0;
pub const EXIT_USER: u8 = 1;

pub fn check(inputs: &[PathBuf]) -> Result<(), ()> {
    let (map, mut diags, modules) = load_and_parse(inputs);
    report(&map, &diags);
    if diags.has_errors() {
        return Err(());
    }
    let mut cx = TyCtxt::new();
    let mut defs = Defs::new();
    for (i, (_, module)) in modules.iter().enumerate() {
        let fid = FileId::new(i as u32);
        let resolved = crate::resolve::resolve_module(&mut defs, fid, module);
        diags.extend(resolved.diags);
    }
    if diags.has_errors() {
        report(&map, &diags);
        return Err(());
    }
    let mut bodies = Vec::new();
    for (path, module) in &modules {
        match lower::lower_module(&mut cx, &defs, module) {
            Ok(b) => bodies.extend(b),
            Err(msg) => {
                let is_internal = msg.starts_with("internal compiler error");
                diags.push(Diagnostic::error(
                    if is_internal {
                        DiagnosticKind::EInternal
                    } else {
                        DiagnosticKind::EDuplicate
                    },
                    Span::point(module.span.file, 0),
                    format!("{}: {}", path.display(), msg),
                ));
            }
        }
    }
    for b in &bodies {
        crate::check::validate_mir(b, &mut diags);
        crate::check::validate_return_consistency(b, &cx, &mut diags);
        crate::check::check_ownership(b, &mut diags);
        crate::check::check_borrow(b, &mut diags);
        crate::check::check_magnet(b, &mut diags);
    }
    report(&map, &diags);
    if diags.has_errors() {
        Err(())
    } else {
        Ok(())
    }
}

pub fn build(inputs: &[PathBuf], _opt: u32, emit: Option<crate::cli::EmitKind>) -> Result<(), ()> {
    let (map, mut diags, modules) = load_and_parse(inputs);
    report(&map, &diags);
    if diags.has_errors() {
        return Err(());
    }
    let mut cx = TyCtxt::new();
    let mut defs = Defs::new();
    for (i, (_, module)) in modules.iter().enumerate() {
        let fid = FileId::new(i as u32);
        let resolved = crate::resolve::resolve_module(&mut defs, fid, module);
        diags.extend(resolved.diags);
    }
    if diags.has_errors() {
        report(&map, &diags);
        return Err(());
    }
    let mut bodies = Vec::new();
    for (path, module) in &modules {
        match lower::lower_module(&mut cx, &defs, module) {
            Ok(b) => bodies.extend(b),
            Err(msg) => {
                let is_internal = msg.starts_with("internal compiler error");
                diags.push(Diagnostic::error(
                    if is_internal {
                        DiagnosticKind::EInternal
                    } else {
                        DiagnosticKind::EDuplicate
                    },
                    Span::point(module.span.file, 0),
                    format!("{}: {}", path.display(), msg),
                ));
            }
        }
    }
    if std::env::var("AVERA_DUMP_AST").is_ok() {
        for (_, m) in &modules {
            eprintln!("--- AST ---\n{:#?}", m);
        }
    }
    if std::env::var("AVERA_DUMP_MIR").is_ok() {
        for b in &bodies {
            eprintln!("--- MIR: {} ---\n{:#?}", b.name, b);
        }
    }
    for b in &bodies {
        crate::check::validate_mir(b, &mut diags);
        crate::check::validate_return_consistency(b, &cx, &mut diags);
        crate::check::check_ownership(b, &mut diags);
        crate::check::check_borrow(b, &mut diags);
        crate::check::check_magnet(b, &mut diags);
    }
    if diags.has_errors() {
        report(&map, &diags);
        return Err(());
    }
    let mut bodies = bodies;
    for b in &mut bodies {
        crate::check::elaborate_drops(b);
    }
    let name = inputs
        .first()
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("avera_out")
        .to_string();
    let build_dir = PathBuf::from("build/debug");
    if let Err(e) = std::fs::create_dir_all(&build_dir) {
        eprintln!(
            "cannot create build directory `{}`: {e}",
            build_dir.display()
        );
        return Err(());
    }
    match emit {
        Some(crate::cli::EmitKind::Ast) => {
            for (_, m) in &modules {
                println!("{:#?}", m);
            }
            return Ok(());
        }
        Some(crate::cli::EmitKind::Mir) => {
            for b in &bodies {
                println!("--- {} ---\n{:#?}", b.name, b);
            }
            return Ok(());
        }
        _ => {}
    }
    let obj_path = match codegen_obj(&bodies, &cx, &defs, &build_dir, &name) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("codegen error: {}", e);
            return Err(());
        }
    };
    if matches!(emit, Some(crate::cli::EmitKind::Obj)) {
        println!("{}", obj_path.display());
        return Ok(());
    }
    let exe_path = build_dir.join(&name);
    let rt_obj = runtime::build_runtime_object(&build_dir);
    if let Err(e) = object::link_executable_with_rt(&obj_path, &rt_obj, &exe_path) {
        eprintln!("link error: {}", e);
        return Err(());
    }
    println!("built {}", exe_path.display());
    Ok(())
}

pub fn run(input: &Path, args: &[String]) -> Result<u8, ()> {
    let build_dir = PathBuf::from("build/debug");
    if let Err(e) = std::fs::create_dir_all(&build_dir) {
        eprintln!(
            "cannot create build directory `{}`: {e}",
            build_dir.display()
        );
        return Err(());
    }
    let name = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("avera_out")
        .to_string();
    let exe_path = build_dir.join(&name);
    build(&[input.to_path_buf()], 0, None)?;
    let mut cmd = std::process::Command::new(&exe_path);
    cmd.args(args);
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to run `{}`: {}", exe_path.display(), e);
            return Err(());
        }
    };
    Ok(status.code().unwrap_or(1) as u8)
}

pub fn test(inputs: &[PathBuf]) -> Result<(), ()> {
    if inputs.is_empty() {
        eprintln!("`avera test` needs at least one input");
        return Err(());
    }
    let mut ok = 0usize;
    let mut fail = 0usize;
    for input in inputs {
        if input.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(input)
                .map_err(|e| {
                    eprintln!("cannot read test directory `{}`: {e}", input.display());
                })?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("av"))
                .collect();
            entries.sort();
            for e in entries {
                run_test_file(&e, &mut ok, &mut fail);
            }
        } else {
            run_test_file(input, &mut ok, &mut fail);
        }
    }
    println!("test result: {} passed, {} failed", ok, fail);
    if fail == 0 {
        Ok(())
    } else {
        Err(())
    }
}

fn run_test_file(p: &Path, ok: &mut usize, fail: &mut usize) {
    print!("test {} ... ", p.display());
    match run(p, &[]) {
        Ok(0) => {
            println!("ok");
            *ok += 1;
        }
        Ok(code) => {
            println!("FAILED (exit {})", code);
            *fail += 1;
        }
        Err(()) => {
            println!("FAILED (compile error)");
            *fail += 1;
        }
    }
}

pub fn fmt(inputs: &[PathBuf], check: bool) -> Result<(), ()> {
    let mut any_diff = false;
    for input in inputs {
        let src = match std::fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot read `{}`: {}", input.display(), e);
                return Err(());
            }
        };
        let mut map = SourceMap::new();
        let fid = map.load(input, &src);
        let lexed = Lexer::new(fid, &src).lex();
        let mut diags = lexed.diags;
        let parsed = parse(fid, lexed.tokens);
        diags.extend(parsed.diags.clone());
        if diags.has_errors() {
            report(&map, &diags);
            eprintln!("refusing to format malformed source `{}`", input.display());
            return Err(());
        }
        let formatted = crate::fmt::format_module(&parsed.module);
        if check {
            if formatted != src {
                any_diff = true;
                eprintln!("would reformat {}", input.display());
            }
        } else if let Err(e) = std::fs::write(input, formatted) {
            eprintln!("cannot write `{}`: {}", input.display(), e);
            return Err(());
        }
    }
    if check && any_diff {
        Err(())
    } else {
        Ok(())
    }
}

fn load_and_parse(inputs: &[PathBuf]) -> (SourceMap, DiagnosticList, Vec<(PathBuf, Module)>) {
    let mut map = SourceMap::new();
    let mut diags = DiagnosticList::new();
    let mut modules: Vec<(PathBuf, Module)> = Vec::new();
    let mut loaded_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut loaded_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    let mut queue: Vec<(PathBuf, PathBuf)> = Vec::new();
    for input in inputs {
        queue.push((
            input.clone(),
            input.parent().unwrap_or(Path::new(".")).to_path_buf(),
        ));
    }

    while let Some((path, search_dir)) = queue.pop() {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let msg = match ext {
                "cgr" => Some(("`.cgr` is no longer an Avera source extension", "use `.av`")),
                "mgr" => Some((
                    "`.mgr` is no longer an Avera module extension",
                    "use `.mav`",
                )),
                "hgr" | "hav" => Some((
                    "Avera no longer uses separate header files",
                    "move the public interface into the module's `.mav` file",
                )),
                _ => None,
            };
            if let Some((m, sugg)) = msg {
                let fid = match std::fs::read_to_string(&path) {
                    Ok(src) => map.load(&path, &src),
                    Err(_) => map.load(&path, ""),
                };
                diags.push(
                    Diagnostic::error(
                        DiagnosticKind::ELegacyExtension,
                        Span::point(fid, 0),
                        format!("{}: {}", path.display(), m),
                    )
                    .with_suggestion(sugg.to_string()),
                );
                continue;
            }
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if loaded_files.contains(&canonical) {
            continue;
        }
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                let fid = map.load(&path, "");
                diags.push(Diagnostic::error(
                    DiagnosticKind::EMissingModule,
                    Span::point(fid, 0),
                    format!("cannot read `{}`: {}", path.display(), e),
                ));
                continue;
            }
        };
        loaded_files.insert(canonical);
        let fid = map.load(&path, &src);
        let lexed = Lexer::new(fid, &src).lex();
        diags.extend(lexed.diags);
        let parsed = parse(fid, lexed.tokens);
        diags.extend(parsed.diags.clone());
        for d in &parsed.module.directives {
            if let crate::ast::DirectiveKind::Import { path: ipath, .. } = &d.kind {
                let module_name = crate::module::import_to_module_name(&d.kind).unwrap_or_default();
                if module_name.is_empty() || loaded_names.contains(&module_name) {
                    continue;
                }
                loaded_names.insert(module_name.clone());
                let mut resolved = false;
                if let Some(stdlib_path) = resolve_stdlib_import(&ipath.segments) {
                    if stdlib_path.exists() {
                        let parent = stdlib_path.parent().unwrap_or(Path::new(".")).to_path_buf();
                        queue.push((stdlib_path, parent));
                        resolved = true;
                    }
                }
                if !resolved {
                    if let Some(mav_path) = resolve_user_module(&ipath.segments, &search_dir) {
                        load_module_contract(
                            &mav_path,
                            &mut map,
                            &mut diags,
                            &mut modules,
                            &mut queue,
                            &mut loaded_names,
                            &mut loaded_files,
                        );
                        resolved = true;
                    }
                }
                if !resolved {
                    diags.push(Diagnostic::error(
                        DiagnosticKind::EMissingModule,
                        d.span,
                        format!("cannot resolve imported module `{module_name}`"),
                    ));
                }
            }
        }
        modules.push((path.clone(), parsed.module));
    }
    (map, diags, modules)
}

fn load_module_contract(
    mav_path: &Path,
    map: &mut SourceMap,
    diags: &mut DiagnosticList,
    modules: &mut Vec<(PathBuf, Module)>,
    queue: &mut Vec<(PathBuf, PathBuf)>,
    loaded_names: &mut std::collections::HashSet<String>,
    loaded_files: &mut std::collections::HashSet<PathBuf>,
) {
    let canonical = mav_path
        .canonicalize()
        .unwrap_or_else(|_| mav_path.to_path_buf());
    if loaded_files.contains(&canonical) {
        return;
    }
    let src = match std::fs::read_to_string(mav_path) {
        Ok(s) => s,
        Err(e) => {
            let fid = map.load(mav_path, "");
            diags.push(Diagnostic::error(
                DiagnosticKind::EMissingModule,
                Span::point(fid, 0),
                format!("cannot read Avera module `{}`: {}", mav_path.display(), e),
            ));
            return;
        }
    };
    loaded_files.insert(canonical);
    let dir = mav_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let fid = map.load(mav_path, &src);
    let lexed = Lexer::new(fid, &src).lex();
    diags.extend(lexed.diags);
    let parsed = parse(fid, lexed.tokens);
    diags.extend(parsed.diags.clone());
    modules.push((mav_path.to_path_buf(), parsed.module.clone()));
    for d in &parsed.module.directives {
        match &d.kind {
            crate::ast::DirectiveKind::Module(p) => {
                let name = if p.segments.len() >= 2 {
                    format!("{}.{}", p.segments[0], p.segments[1])
                } else {
                    p.segments.first().cloned().unwrap_or_default()
                };
                loaded_names.insert(name);
            }
            crate::ast::DirectiveKind::Source(s) => {
                let src_path = normalize_source_path(&dir, s);
                queue.push((src_path, dir.clone()));
            }
            _ => {}
        }
    }
}

fn resolve_stdlib_import(segments: &[String]) -> Option<PathBuf> {
    if segments.is_empty() {
        return None;
    }
    let path = segments.join("/");
    let candidates = [
        PathBuf::from("stdlib").join(&path).with_extension("av"),
        PathBuf::from("std").join(&path).with_extension("av"),
        PathBuf::from(&path).with_extension("av"),
    ];
    for c in &candidates {
        if c.exists() {
            return Some(c.clone());
        }
    }
    None
}

fn resolve_user_module(segments: &[String], search_dir: &Path) -> Option<PathBuf> {
    let file_name = segments.last()?.clone();
    let mav = format!("{}.mav", file_name);
    let candidate = search_dir.join(&mav);
    if candidate.is_file() {
        return Some(candidate);
    }
    let src_candidate = PathBuf::from("src").join(&mav);
    if src_candidate.is_file() {
        return Some(src_candidate);
    }
    None
}

fn normalize_source_path(dir: &Path, source: &str) -> PathBuf {
    let p = Path::new(source);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let joined = dir.join(p);
    let cleaned: PathBuf = joined
        .components()
        .filter(|c| *c != std::path::Component::ParentDir)
        .collect();
    if cleaned.as_os_str().is_empty() {
        joined
    } else {
        cleaned
    }
}

fn report(map: &SourceMap, diags: &DiagnosticList) {
    if !diags.is_empty() {
        eprint!("{}", Emitter::no_color(map).render(diags));
    }
}

fn codegen_obj(
    bodies: &[Body],
    cx: &TyCtxt,
    defs: &Defs,
    build_dir: &Path,
    name: &str,
) -> Result<PathBuf, String> {
    let mut module = object::make_object_module()?;
    let _ = codegen::compile_bodies(&mut module, bodies, cx, defs)?;
    object::write_object(module, build_dir, name)
}
