#![allow(dead_code)]

use std::process::Command;

pub fn build_source(src: &str, prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let name = format!("{}_{}", prefix, id);

    let dir = std::env::temp_dir().join("avera_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join(format!("{}.av", name));
    std::fs::write(&src_path, src).unwrap();

    let exe = std::path::PathBuf::from(format!("build/debug/{}", name));

    let build = Command::new(env!("CARGO_BIN_EXE_avera"))
        .arg("build")
        .arg(&src_path)
        .output()
        .expect("failed to run avera build");
    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr);
        let _ = std::fs::remove_file(&src_path);
        panic!("build failed:\n{}", stderr);
    }

    let _ = std::fs::remove_file(&src_path);
    exe
}

pub fn cleanup_binary(exe: &std::path::Path) {
    let _ = std::fs::remove_file(exe);
    let mut obj = exe.to_path_buf();
    obj.set_extension("o");
    let _ = std::fs::remove_file(&obj);
}

pub fn build_and_run_full(src: &str) -> (String, String) {
    let exe = build_source(src, "test_input");
    let run = Command::new(&exe).output().expect("failed to run binary");
    cleanup_binary(&exe);
    (
        String::from_utf8_lossy(&run.stdout).to_string(),
        String::from_utf8_lossy(&run.stderr).to_string(),
    )
}

pub fn build_and_run(src: &str) -> String {
    build_and_run_full(src).0
}

pub fn build_and_run_with_stdin(src: &str, stdin: &str) -> String {
    let exe = build_source(src, "stdin_input");

    use std::io::Write;
    let run = Command::new(&exe)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn binary");
    {
        let mut child_stdin = run.stdin.as_ref().expect("failed to open stdin");
        child_stdin
            .write_all(stdin.as_bytes())
            .expect("failed to write stdin");
    }
    let output = run.wait_with_output().expect("failed to wait on binary");
    cleanup_binary(&exe);
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn assert_compile_fails(src: &str, needle: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static FAIL_COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = FAIL_COUNTER.fetch_add(1, Ordering::SeqCst);
    let name = format!("fail_input_{}", id);

    let dir = std::env::temp_dir().join("avera_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join(format!("{}.av", name));
    std::fs::write(&src_path, src).unwrap();

    let build = Command::new(env!("CARGO_BIN_EXE_avera"))
        .arg("build")
        .arg(&src_path)
        .output()
        .expect("failed to run avera build");
    let _ = std::fs::remove_file(&src_path);
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&build.stderr),
        String::from_utf8_lossy(&build.stdout)
    );
    assert!(
        !build.status.success(),
        "expected compilation to FAIL, but it succeeded. Output:\n{}",
        combined
    );
    assert!(
        !needle.is_empty(),
        "compile-fail tests must assert a concrete diagnostic"
    );
    assert!(
        combined.contains(needle),
        "expected diagnostic containing `{}` but got:\n{}",
        needle,
        combined
    );
    combined
}

pub fn assert_run(src: &str, expected: &str) {
    let out = build_and_run(src);
    assert_eq!(out, expected, "stdout mismatch");
}

pub fn assert_run_contains(src: &str, needle: &str) {
    let out = build_and_run(src);
    assert!(
        out.contains(needle),
        "expected stdout containing `{}` but got: {}",
        needle,
        out
    );
}
