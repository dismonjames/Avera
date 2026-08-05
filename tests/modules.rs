
use std::path::{Path, PathBuf};
use std::process::Command;

fn build_project(dir: &Path, entry_rel: &str) -> (bool, String) {
    let entry = dir.join(entry_rel);
    let out = Command::new(env!("CARGO_BIN_EXE_avera"))
        .current_dir(dir)
        .arg("build")
        .arg(&entry)
        .output()
        .expect("failed to run avera build");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );
    (out.status.success(), combined)
}

fn build_single(src: &str, ext: &str) -> (bool, String) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let id = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join("avera_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("modtest_{}.{}", id, ext));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_avera"))
        .arg("build")
        .arg(&path)
        .output()
        .expect("failed to run avera build");
    let _ = std::fs::remove_file(&path);
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );
    (out.status.success(), combined)
}

fn fresh_project(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let id = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir()
        .join("avera_mod_tests")
        .join(format!("{}_{}", name, id));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    dir
}

// 1. `.av` recognized (single-file program compiles and runs).
#[test]
fn av_single_file_recognized() {
    let dir = fresh_project("av_recognized");
    let main = dir.join("src/main.av");
    std::fs::write(
        &main,
        "#std.io\naction main(): I32 will\n    print(\"av-ok\")\n    return 0\nfinish\n",
    )
    .unwrap();
    let (ok, out) = build_project(&dir, "src/main.av");
    assert!(ok, "expected .av to compile: {out}");
    // Run the built binary.
    let exe = dir.join("build/debug/main");
    let run = Command::new(&exe).output().expect("failed to run binary");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "av-ok\n");
}

// 2. `.cgr` rejected with a legacy-extension diagnostic.
#[test]
fn cgr_legacy_rejected() {
    let (ok, out) = build_single("#std.io\naction main(): I32 will return 0 finish\n", "cgr");
    assert!(!ok, "expected .cgr to be rejected, but it compiled:\n{out}");
    assert!(
        out.contains("no longer an Avera source extension"),
        "expected legacy diagnostic, got:\n{out}"
    );
}

// 3. `.mav` recognized as a module contract driving compilation.
#[test]
fn mav_module_contract_drives_build() {
    let dir = fresh_project("mav_drives");
    std::fs::write(
        dir.join("src/math.mav"),
        "#module app.math\n#source \"math.av\"\nexport action add(a: I32, b: I32): I32\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/math.av"),
        "action add(a: I32, b: I32): I32 will\n    return a + b\nfinish\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.av"),
        "#std.io\n#app.math\naction main(): I32 will\n    print(\"Avera:\", add(20, 22))\n    return 0\nfinish\n",
    )
    .unwrap();
    let (ok, out) = build_project(&dir, "src/main.av");
    assert!(ok, "expected .mav-driven build to succeed:\n{out}");
    let run = Command::new(dir.join("build/debug/main"))
        .output()
        .expect("failed to run binary");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Avera: 42\n");
}

// 4. `.mgr` rejected with a legacy-extension diagnostic.
#[test]
fn mgr_legacy_rejected() {
    let (ok, out) = build_single("#module x\n", "mgr");
    assert!(!ok, "expected .mgr to be rejected, but it compiled:\n{out}");
    assert!(
        out.contains("no longer an Avera module extension"),
        "expected legacy diagnostic, got:\n{out}"
    );
}

// 5. `.hgr`/`.hav` rejected — no separate header file type.
#[test]
fn hgr_legacy_rejected() {
    let (ok, out) = build_single("#module x\n", "hgr");
    assert!(!ok, "expected .hgr to be rejected, but it compiled:\n{out}");
    assert!(
        out.contains("no longer uses separate header files"),
        "expected legacy diagnostic, got:\n{out}"
    );
    let (ok2, out2) = build_single("#module x\n", "hav");
    assert!(!ok2, "expected .hav to be rejected:\n{out2}");
    assert!(out2.contains("no longer uses separate header files"));
}

// 6. Multi-source `.mav` — contract lists several `#source` files compiled
// as one logical module.
#[test]
fn mav_multi_source() {
    let dir = fresh_project("mav_multi");
    std::fs::write(
        dir.join("src/m.mav"),
        "#module app.m\n#source \"a.av\"\n#source \"b.av\"\nexport action a(): I32\nexport action b(): I32\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/a.av"),
        "action a(): I32 will\n    return 11\nfinish\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/b.av"),
        "action b(): I32 will\n    return 22\nfinish\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.av"),
        "#std.io\n#app.m\naction main(): I32 will\n    print(a() + b())\n    return 0\nfinish\n",
    )
    .unwrap();
    let (ok, out) = build_project(&dir, "src/main.av");
    assert!(ok, "expected multi-source .mav build to succeed:\n{out}");
    let run = Command::new(dir.join("build/debug/main"))
        .output()
        .expect("failed to run binary");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "33\n");
}

// 7. Module export validation — missing implementation fails.
#[test]
fn missing_implementation_fails() {
    let dir = fresh_project("missing_impl");
    std::fs::write(
        dir.join("src/m.mav"),
        "#module app.m\n#source \"m.av\"\nexport action add(a: I32, b: I32): I32\n",
    )
    .unwrap();
    // m.av implements `sub` but NOT the declared `add`.
    std::fs::write(
        dir.join("src/m.av"),
        "action sub(a: I32, b: I32): I32 will\n    return a - b\nfinish\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.av"),
        "#std.io\n#app.m\naction main(): I32 will\n    print(add(1, 2))\n    return 0\nfinish\n",
    )
    .unwrap();
    let (ok, out) = build_project(&dir, "src/main.av");
    // Without the `add` implementation, calling it must fail (undefined name).
    assert!(
        !ok,
        "expected build to fail due to missing impl, but it succeeded:\n{out}"
    );
}

// 8. Duplicate export fails.
#[test]
fn duplicate_export_fails() {
    let dir = fresh_project("dup_export");
    std::fs::write(
        dir.join("src/m.mav"),
        "#module app.m\n#source \"m.av\"\nexport action dup(): I32\nexport action dup(): I32\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/m.av"),
        "action dup(): I32 will\n    return 0\nfinish\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.av"),
        "#std.io\n#app.m\naction main(): I32 will\n    print(dup())\n    return 0\nfinish\n",
    )
    .unwrap();
    let (ok, out) = build_project(&dir, "src/main.av");
    assert!(!ok, "expected duplicate export to fail:\n{out}");
    assert!(
        out.contains("already defined"),
        "expected 'already defined' diagnostic, got:\n{out}"
    );
}

// 9. Module import — a caller uses `#app.foo` to reach another module.
// (Covered by mav_module_contract_drives_build and mav_multi_source.)

// 10. Dependency cycle detection in the module graph.
#[test]
fn dependency_cycle_detected() {
    use avera_compiler::module::{detect_cycles, LoadedModule, ModuleDescriptor, ModuleGraph};
    let mk = |name: &str, deps: &[&str]| {
        let d = ModuleDescriptor {
            name: name.to_string(),
            depends: deps.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        LoadedModule {
            name: name.to_string(),
            descriptor: d,
            dir: PathBuf::from("."),
        }
    };
    let mut g = ModuleGraph::new();
    g.add(mk("app.a", &["app.b"]));
    g.add(mk("app.b", &["app.c"]));
    g.add(mk("app.c", &["app.a"]));
    let cycle = detect_cycles(&g).expect("expected a cycle");
    assert!(
        cycle.contains(&"app.a".to_string())
            && cycle.contains(&"app.b".to_string())
            && cycle.contains(&"app.c".to_string()),
        "cycle should list a, b, c; got {cycle:?}"
    );
}

// 11. Magnet regression — rename must not regress Magnet semantics.
#[test]
fn magnet_regression() {
    let dir = fresh_project("magnet_reg");
    std::fs::write(
        dir.join("src/main.av"),
        "#std.io\naction main(): I32 will\n    :value = 42\n    ~!m = value\n    m = 100\n    print(\"Value:\", value)\n    print(\"Address:\", ~m.address)\n    return 0\nfinish\n",
    )
    .unwrap();
    let (ok, out) = build_project(&dir, "src/main.av");
    assert!(ok, "expected magnet program to compile:\n{out}");
    let run = Command::new(dir.join("build/debug/main"))
        .output()
        .expect("failed to run binary");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    assert!(
        stdout.contains("Value: 100"),
        "expected Value: 100, got {stdout}"
    );
    assert!(
        stdout.contains("Address: "),
        "expected an address line, got {stdout}"
    );
    // The address is printed as a decimal integer; ensure it is a real,
    // non-zero native address.
    let addr_line = stdout.lines().find(|l| l.starts_with("Address: ")).unwrap();
    let addr_str = addr_line.trim_start_matches("Address: ").trim();
    let addr: i64 = addr_str
        .parse()
        .unwrap_or_else(|_| panic!("expected a numeric address, got {addr_str:?}"));
    assert!(addr != 0, "expected a non-zero native address, got {addr}");
}
