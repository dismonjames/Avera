// build.rs - version metadata
fn main() {
    // Re-run when manifest changes.
    println!("cargo:rerun-if-changed=Cargo.toml");
    // Expose the package version as a compile-time env var.
    println!(
        "cargo:rustc-env=AVERA_BUILD_VERSION={}",
        env!("CARGO_PKG_VERSION")
    );
}
