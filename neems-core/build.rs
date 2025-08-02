use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../migrations");
    println!("cargo:rustc-check-cfg=cfg(run_schema_tests)");

    // Check force flags
    let force_all = std::env::var("FORCE_ALL_TESTS").is_ok();
    let force_schema = std::env::var("FORCE_SCHEMA_TESTS").is_ok();

    // Only check git changes if not forcing tests
    let should_run_schema_tests = force_all
        || force_schema
        || Command::new("git")
            .args([
                "diff",
                "--quiet",
                "HEAD",
                "--",
                "../migrations/",
                "neems-core/src/schema.rs",
                "neems-core/src/models/",
            ])
            .status()
            .map(|status| !status.success())
            .unwrap_or(true);

    if should_run_schema_tests {
        println!("cargo:rustc-cfg=run_schema_tests");
    }
}
