use serde_json::{Value, json};
use std::{fs, process::Command};
#[test]
fn experimental_coverage_collects_imports_and_queries_without_spec_checks() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-coverage-{}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname=\"coverage-cli-probe\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n").unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn one()->u32 {1}\npub fn two()->u32 {2}\n#[test] fn works(){assert_eq!(one(),1);}\n#[test] fn unrelated(){assert_eq!(two(),2);}\n",
    )
    .unwrap();
    assert!(
        Command::new("cargo")
            .arg("generate-lockfile")
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["experimental", "coverage"])
            .args(args)
            .output()
            .unwrap()
    };
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Coverage test",
                "-c",
                "user.email=coverage@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    fs::write(root.join(".gitignore"), "/.dctx0ctl/\n").unwrap();
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-qm", "baseline"]);
    let path = root.to_str().unwrap();
    assert!(invoke(&["affected", path]).status.success());
    assert!(!root.join(".dctx0ctl/state.sqlite3").exists());
    assert!(
        !invoke(&["affected", path, "--base", "missing-revision"])
            .status
            .success()
    );
    let result = invoke(&["collect", path, "--timeout-secs", "60"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = invoke(&["list", path]);
    assert!(result.status.success());
    let records: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 2);
    let execution = records
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["execution"]["test_id"]
                .as_str()
                .unwrap()
                .ends_with("::works")
        })
        .unwrap()["execution"]
        .clone();
    let manifest = std::path::Path::new(execution["provenance"]["artifacts"].as_str().unwrap())
        .join("manifest.json");
    assert!(
        invoke(&["import", path, manifest.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(execution["outcome"], "pass");
    let source = fs::canonicalize(root.join("src/lib.rs")).unwrap();
    let result = invoke(&[
        "list",
        path,
        "--file",
        source.to_str().unwrap(),
        "--line",
        "1",
    ]);
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let report = root.join("import.json");
    fs::write(
        &report,
        serde_json::to_vec(&json!({"executions":[execution]})).unwrap(),
    )
    .unwrap();
    assert!(
        invoke(&["import", path, report.to_str().unwrap()])
            .status
            .success()
    );
    assert!(
        !invoke(&[
            "collect",
            path,
            "--filter",
            "missing-test",
            "--timeout-secs",
            "60"
        ])
        .status
        .success()
    );
    assert!(!invoke(&["list", path, "--line", "1"]).status.success());
    assert!(
        !invoke(&["collect", path, "--timeout-secs", "0"])
            .status
            .success()
    );
    let original = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        original.replace("one()->u32 {1}", "one()->u32 {2}"),
    )
    .unwrap();
    let affected = invoke(&["affected", path, "--base", "HEAD"]);
    assert!(
        affected.status.success(),
        "{}",
        String::from_utf8_lossy(&affected.stderr)
    );
    let affected: Value = serde_json::from_slice(&affected.stdout).unwrap();
    assert_eq!(affected["candidates"].as_array().unwrap().len(), 1);
    assert!(
        affected["candidates"][0]["test_id"]
            .as_str()
            .unwrap()
            .ends_with("::works")
    );
    assert_eq!(
        affected["candidates"][0]["reasons"][0]["file"],
        "src/lib.rs"
    );
    assert_eq!(affected["can_skip_other_tests"], false);
    assert_eq!(
        affected["candidates"][0]["command"],
        json!([
            "cargo",
            "test",
            "--locked",
            "--package",
            "coverage-cli-probe",
            "--lib",
            "--",
            "--exact",
            "works"
        ])
    );
    let truth = Command::new("cargo")
        .args(["test", "--lib", "--locked"])
        .env("CARGO_TARGET_DIR", root.join(".dctx0ctl/ground-truth"))
        .env("RUSTFLAGS", "")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("LLVM_PROFILE_FILE")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(!truth.status.success());
    let truth = String::from_utf8_lossy(&truth.stdout);
    assert!(truth.contains("works ... FAILED"));
    assert!(truth.contains("unrelated ... ok"));
    fs::write(&source, format!("// inserted comment\n{original}")).unwrap();
    fs::write(root.join("new.rs"), "// no baseline\n").unwrap();
    let affected: Value = serde_json::from_slice(&invoke(&["affected", path]).stdout).unwrap();
    let codes: Vec<_> = affected["unknowns"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["code"].as_str())
        .collect();
    assert!(codes.contains(&"inserted_lines_not_observed_in_base"));
    assert!(codes.contains(&"new_file_has_no_baseline_coverage"));
    assert_eq!(
        affected["candidates"][0]["reasons"][0]["insertion_anchor"],
        true
    );
    let connection = dctx0_store::Store::open(
        &root.join(".dctx0ctl/state.sqlite3"),
        dctx0_store::AccessMode::ReadOnly,
    )
    .unwrap();
    use dctx0_store::HistoryReader;
    assert!(connection.history(path).unwrap().is_empty());
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}
