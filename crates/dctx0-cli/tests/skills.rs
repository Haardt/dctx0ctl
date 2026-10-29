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
        dctx0_engine::SKILL_NAMES.len()
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
    assert_eq!(
        report["installations"].as_array().unwrap().len(),
        dctx0_engine::SKILL_NAMES.len() * 7
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn copied_binary_installs_config_skill_and_its_references_without_repository_access() {
    let root = std::env::temp_dir().join(format!("dctx0-config-skill-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let binary = root.join(format!("dctx0{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_dctx0"), &binary).unwrap();
    let output = Command::new(&binary)
        .current_dir(&root)
        .args([
            "--json",
            "skills",
            "install",
            "--project",
            root.to_str().unwrap(),
            "--agent",
            "codex",
            "--skill",
            "dctx0-config",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["installations"].as_array().unwrap().len(), 1);
    let destination = root.join(".agents/skills/dctx0-config");
    for name in [
        "SKILL.md",
        "references/configuration.md",
        "references/query.md",
        "references/workflow-measurement.md",
    ] {
        assert!(
            destination.join(name).is_file(),
            "Missing installed resource: {name}"
        );
    }
    assert!(!root.join(".dctx0ctl").exists());
    assert!(!root.join(".agents/skills/dctx0-language").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn standalone_delivery_bundle_is_complete_and_installation_is_idempotent() {
    let root = std::env::temp_dir().join(format!("dctx0-delivery-skill-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let binary = root.join(format!("dctx0{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_dctx0"), &binary).unwrap();
    let install = || {
        Command::new(&binary)
            .current_dir(&root)
            .args([
                "--json",
                "install",
                "--project",
                root.to_str().unwrap(),
                "--agent",
                "codex",
                "--skill",
                "dctx0-delivery",
            ])
            .output()
            .unwrap()
    };
    let first = install();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let destination = root.join(".agents/skills/dctx0-delivery");
    for (name, expected) in [
        (
            "SKILL.md",
            include_str!("../../../skills/dctx0-delivery/SKILL.md"),
        ),
        (
            "references/delivery-measures.md",
            include_str!("../../../skills/dctx0-delivery/references/delivery-measures.md"),
        ),
        (
            "references/query.md",
            include_str!("../../../skills/query-guide.md"),
        ),
        (
            "references/workflow-measurement.md",
            include_str!("../../../skills/workflow-measurement.md"),
        ),
    ] {
        assert_eq!(
            fs::read_to_string(destination.join(name)).unwrap(),
            expected
        );
    }
    let second = install();
    assert!(second.status.success());
    let report: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(report["installations"][0]["status"], "unchanged");
    assert!(!root.join(".dctx0ctl").exists());
    fs::remove_dir_all(root).unwrap();
}
