use std::process::Command;

#[test]
fn js_search_tests_pass() {
    let probe = Command::new("node").arg("--version").output();
    let Ok(out) = probe else {
        eprintln!("node not found on PATH — skipping JS search tests");
        return;
    };
    if !out.status.success() {
        eprintln!("`node --version` failed — skipping JS search tests");
        return;
    }

    let manifest = env!("CARGO_MANIFEST_DIR");
    let status = Command::new("node")
        .args(["--test", "tests/search.test.mjs"])
        .current_dir(manifest)
        .status()
        .expect("failed to spawn node for JS tests");

    assert!(
        status.success(),
        "JS search tests failed (run `node --test tests/search.test.mjs` to see details)",
    );
}
