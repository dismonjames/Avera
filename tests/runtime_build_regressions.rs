use avera_compiler::driver_pipeline::runtime::build_runtime_object;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn runtime_builder_reports_invalid_build_directory() {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "avera_runtime_build_file_{}_{}",
        std::process::id(),
        id
    ));
    std::fs::write(&path, "this is a file, not a directory").unwrap();

    let err = build_runtime_object(&path).expect_err("invalid build directory must fail");
    let _ = std::fs::remove_file(&path);

    assert!(err.contains("runtime build directory"), "{err}");
}
