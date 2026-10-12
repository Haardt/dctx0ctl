#![cfg(feature = "rust")]
//! Rust-only CLI selection and unavailable-scaffold evidence.
use std::{fs, process::Command};
fn cli(args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .arg("--json")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn rust_cli_initializes_without_overwriting_the_adapter_and_discovers_public_items() {
    let root = std::env::temp_dir().join(format!("dctx0-rust-cli-{}", std::process::id()));
    fs::create_dir_all(root.join("dctx0")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("dctx0/w.dctx0"),
        "workspace W { scalar Year is Int }",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), "pub struct Year(pub i64);").unwrap();
    let spec = root.join("dctx0");
    cli(&["check", spec.to_str().unwrap()]);
    let setup = cli(&[
        "verify",
        "init",
        spec.to_str().unwrap(),
        "--project",
        root.to_str().unwrap(),
        "--target",
        "rust",
        "--subject",
        "W.Year",
    ]);
    assert_eq!(setup["configuration"]["target"], "rust");
    assert_eq!(setup["configuration"]["runtime"], "cargo");
    let adapter = root.join(".dctx0/testing/rust/adapter.rs");
    let scaffold = fs::read(&adapter).unwrap();
    cli(&[
        "verify",
        "init",
        spec.to_str().unwrap(),
        "--project",
        root.to_str().unwrap(),
        "--target",
        "rust",
        "--subject",
        "W.Year",
    ]);
    assert_eq!(fs::read(adapter).unwrap(), scaffold);
    let discovery = cli(&[
        "verify",
        "discover",
        spec.to_str().unwrap(),
        "--project",
        root.to_str().unwrap(),
        "--target",
        "rust",
    ]);
    assert_eq!(discovery["inspection"]["target"], "rust");
    let subject = discovery["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["subject"] == "W.Year")
        .unwrap();
    assert_eq!(subject["candidates"][0]["declaration"], "Year");
    fs::remove_dir_all(root).unwrap();
}
