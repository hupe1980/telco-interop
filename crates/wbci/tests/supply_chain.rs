//! The dependency set stays minimal and locked (wbci-switch/SR-009). `cargo deny check` runs
//! in CI's supply-chain job; this test fails when a dependency is added without review.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

// wbci-switch/SR-009
#[test]
fn runtime_dependencies_are_the_reviewed_ones() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let deps: Vec<&str> = manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap()
        .split("\n[")
        .next()
        .unwrap()
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim()))
        .filter(|k| !k.is_empty() && !k.starts_with('#'))
        .collect();
    // carrier-core is first-party (this workspace); the third-party set stays three.
    assert_eq!(
        deps,
        ["carrier-core", "quick-xml", "jiff", "thiserror"],
        "a new dependency needs review in plan.md and deny.toml"
    );
}

// wbci-switch/SR-009
#[test]
fn lockfile_and_deny_policy_exist() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    assert!(std::path::Path::new(&format!("{root}/Cargo.lock")).exists());
    let deny = std::fs::read_to_string(format!("{root}/deny.toml")).unwrap();
    assert!(deny.contains("unknown-registry = \"deny\"") && deny.contains("unknown-git = \"deny\""));
}
