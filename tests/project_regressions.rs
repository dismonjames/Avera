use avera_compiler::project::parse_manifest;
use std::path::Path;

#[test]
fn invalid_opt_is_an_error_instead_of_falling_back_to_o0() {
    let (_, diags) = parse_manifest(
        Path::new("avera.toml"),
        "[project]\nname = \"demo\"\n\n[build]\nopt = banana\n",
    );
    assert!(diags.has_errors());
    assert!(diags
        .iter()
        .any(|diag| diag.message.contains("invalid optimization level")));
}

#[test]
fn out_of_range_opt_is_rejected() {
    let (_, diags) = parse_manifest(
        Path::new("avera.toml"),
        "[project]\nname = \"demo\"\n\n[build]\nopt = 9\n",
    );
    assert!(diags.has_errors());
}

#[test]
fn unknown_key_and_section_are_not_silently_ignored() {
    let (_, diags) = parse_manifest(
        Path::new("avera.toml"),
        "[project]\nname = \"demo\"\ntyop = \"oops\"\n\n[magic]\nthing = 1\n",
    );
    assert!(diags.has_errors());
    assert!(diags
        .iter()
        .any(|diag| diag.message.contains("unknown `[project]` key")));
    assert!(diags
        .iter()
        .any(|diag| diag.message.contains("unknown manifest section")));
}

#[test]
fn dependency_without_path_is_rejected_and_not_added() {
    let (manifest, diags) = parse_manifest(
        Path::new("avera.toml"),
        "[project]\nname = \"demo\"\n\n[dependencies]\nfoo = {}\n",
    );
    assert!(diags.has_errors());
    assert!(manifest.dependencies.is_empty());
}
