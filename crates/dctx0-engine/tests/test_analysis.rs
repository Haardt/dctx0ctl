use std::{fs, process::Command};
#[test]
fn legacy_collector_api_keeps_streamed_records_without_inventing_a_catalog() {
    if std::env::var_os("DCTX0_LEGACY_MISSING_PYTHON").is_some() {
        assert!(
            dctx0_engine::collect_experimental_coverage(&std::env::temp_dir(), None, None, 1)
                .is_err()
        );
        return;
    }
    let unavailable = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "legacy_collector_api_keeps_streamed_records_without_inventing_a_catalog",
        ])
        .env("DCTX0_LEGACY_MISSING_PYTHON", "1")
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        unavailable.status.success(),
        "{}",
        String::from_utf8_lossy(&unavailable.stderr)
    );

    let root = std::env::temp_dir().join(format!("dctx0-legacy-collector-{}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"legacy-collector\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "#[test] fn one(){assert_eq!(1+1,2);}\n#[test] fn two(){assert_eq!(2+2,4);}\n",
    )
    .unwrap();
    let locked = Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(locked.status.success());
    assert!(dctx0_engine::collect_experimental_coverage(&root, None, None, 0).is_err());
    let report = dctx0_engine::collect_experimental_coverage(&root, None, Some("one"), 30).unwrap();
    assert!(report.get("catalog").is_none());
    let paths = report["execution_files"].as_array().unwrap();
    assert_eq!(paths.len(), 1);
    let raw: dctx0_store::CoverageExecution =
        serde_json::from_slice(&fs::read(paths[0].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(raw.outcome, "pass");
    assert_eq!(raw.provenance["test_identity_version"], 2);
    assert!(!root.join(".dctx0ctl/state.sqlite3").exists());
    fs::remove_dir_all(root).unwrap();
}
