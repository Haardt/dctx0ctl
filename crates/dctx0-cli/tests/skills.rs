use std::{fs, process::Command};
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn standalone_binary_installs_without_a_spec_and_reports_preview_idempotence_and_conflict() {
    let root = std::env::temp_dir().join(format!("dctx0-skill-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let catalog = run(&["--json", "skills", "list"]);
    assert!(catalog.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&catalog.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let prefix = [
        "--json",
        "install",
        "--project",
        root.to_str().unwrap(),
        "--agent",
        "codex,claude",
        "--skill",
        "dctx0-language",
    ];
    let mut preview = prefix.to_vec();
    preview.push("--dry-run");
    assert!(run(&preview).status.success());
    assert!(!root.join(".agents").exists());
    assert!(run(&prefix).status.success());
    let again = run(&prefix);
    assert!(again.status.success());
    let report: serde_json::Value = serde_json::from_slice(&again.stdout).unwrap();
    assert_eq!(report["installations"][0]["status"], "unchanged");
    fs::write(
        root.join(".agents/skills/dctx0-language/SKILL.md"),
        "custom",
    )
    .unwrap();
    assert!(!run(&prefix).status.success());
    let mut force = prefix.to_vec();
    force.push("--force");
    assert!(run(&force).status.success());
    assert!(!root.join(".dctx0ctl").exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn defaults_all_agents_and_global_environment_fallbacks_are_explicit() {
    let root = std::env::temp_dir().join(format!("dctx0-skill-defaults-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let output = run(&["skills", "list"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("dctx0-implementation"));
    let output = run(&["install", "--project", root.to_str().unwrap(), "--dry-run"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("dctx0-implementation"));
    let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args([
            "--json",
            "install",
            "--agent",
            "all",
            "--global",
            "--dry-run",
        ])
        .env_remove("HOME")
        .env("USERPROFILE", &root)
        .env("CODEX_HOME", root.join("codex"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["installations"].as_array().unwrap().len(), 21);
    fs::remove_dir_all(root).unwrap();
}
