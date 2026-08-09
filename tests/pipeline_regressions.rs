use avera_compiler::driver_pipeline;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn temp_path(name: &str, ext: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join("avera_pipeline_regressions");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}_{}_{}.{}", std::process::id(), id, ext))
}

#[test]
fn formatter_refuses_to_overwrite_malformed_source() {
    let path = temp_path("malformed", "av");
    let original = "action main(: I32 will\n    return 0\nfinish\n";
    std::fs::write(&path, original).unwrap();

    let result = driver_pipeline::fmt(std::slice::from_ref(&path), false);
    let after = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        result.is_err(),
        "malformed source must not format successfully"
    );
    assert_eq!(after, original, "formatter must preserve malformed input");
}

#[test]
fn missing_import_is_an_error_even_when_unused() {
    let path = temp_path("missing_import", "av");
    std::fs::write(
        &path,
        "#app.this_module_does_not_exist\n\naction main(): I32 will\n    return 0\nfinish\n",
    )
    .unwrap();

    let result = driver_pipeline::check(std::slice::from_ref(&path));
    let _ = std::fs::remove_file(&path);

    assert!(
        result.is_err(),
        "unused dangling import must still be rejected"
    );
}

#[test]
fn missing_input_reports_error_instead_of_panicking_emitter() {
    let path = temp_path("definitely_missing", "av");
    let _ = std::fs::remove_file(&path);

    let result = std::panic::catch_unwind(|| driver_pipeline::check(std::slice::from_ref(&path)));
    assert!(
        result.is_ok(),
        "missing input should not panic diagnostics emitter"
    );
    assert!(
        result.unwrap().is_err(),
        "missing input should be a user error"
    );
}

#[test]
fn direct_test_api_rejects_empty_suite() {
    assert!(driver_pipeline::test(&[]).is_err());
}
