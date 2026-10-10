#![cfg(all(
    feature = "typescript",
    feature = "java",
    feature = "go",
    feature = "rust"
))]
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dctx0-profile-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("dctx0")).unwrap();
        fs::write(
            path.join("dctx0/spec.dctx0"),
            "workspace W { value Id = UUID }",
        )
        .unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn profile_cli_creates_selects_freezes_and_applies_reviewable_updates() {
    let t = Temp::new();
    let out = t.run(&["profile", "rust", "create", "--dry-run", "--agents"]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("PREVIEW"));
    assert!(!t.0.join(".dctx0").exists());
    t.ok(&["profile", "rust", "create", "--agents", "--json"]);
    let java = t.ok(&["profile", "java", "create", "--project", ".", "--json"]);
    assert_eq!(java["default_target"], "rust");
    let go = t.ok(&["profile", "go", "create", "--default", "--agents", "--json"]);
    assert_eq!(go["default_target"], "go");
    t.ok(&["profile", "rust", "create", "--default", "--json"]);
    assert!(!t.run(&["profile", "unknown", "create"]).status.success());
    t.ok(&["check", "dctx0", "--json"]);
    t.ok(&["tasks", "plan", "dctx0", "--json"]);
    let next = t.ok(&["tasks", "next", "dctx0", "--json"]);
    assert_eq!(next["schema_version"], 2);
    let id = next["body"]["data"]["task"]["task_id"]
        .as_i64()
        .unwrap()
        .to_string();
    let show = t.run(&["tasks", "show", "dctx0", &id, "--section", "implementation"]);
    assert!(show.status.success());
    let md = String::from_utf8(show.stdout).unwrap();
    assert!(md.contains("Rust implementation guidance"));
    assert!(md.contains("# Implement value"));
    assert!(
        !t.run(&["tasks", "next", "dctx0", "--target", "typescript"])
            .status
            .success()
    );
    let claim = t.ok(&[
        "tasks",
        "claim-next",
        "dctx0",
        "--actor",
        "a",
        "--project",
        ".",
        "--target",
        "rust",
        "--json",
    ]);
    let data = &claim["body"]["data"];
    let state = &data["state"];
    let claim = state["claim"].as_i64().unwrap().to_string();
    let version = state["version"].as_i64().unwrap().to_string();
    t.ok(&[
        "tasks",
        "complete",
        "dctx0",
        &id,
        "--actor",
        "a",
        "--claim",
        &claim,
        "--version",
        &version,
        "--summary",
        "Delivered",
        "--json",
    ]);
    let unchanged = t.run(&["tasks", "profile-changes", "dctx0"]);
    assert!(unchanged.status.success());
    assert!(
        !String::from_utf8(unchanged.stdout)
            .unwrap()
            .contains("selected for adaptation")
    );
    fs::write(
        t.0.join(".dctx0/profiles/rust/implement/value.md"),
        "Use the revised representation.",
    )
    .unwrap();
    let out = t.run(&[
        "tasks",
        "profile-changes",
        "dctx0",
        "--task",
        &id,
        "--format",
        "json",
    ]);
    assert!(out.status.success());
    fs::write(t.0.join("update.json"), out.stdout).unwrap();
    let preview = t.run(&["tasks", "profile-changes", "dctx0", "--all-affected"]);
    assert!(preview.status.success());
    assert!(
        String::from_utf8(preview.stdout)
            .unwrap()
            .contains("selected for adaptation")
    );
    let plan = t.ok(&[
        "tasks",
        "profile-update",
        "dctx0",
        "--file",
        "update.json",
        "--reason",
        "Adopt revised representation",
        "--json",
    ]);
    assert_ne!(
        plan["body"]["data"]["members"][0]["task_id"]
            .as_i64()
            .unwrap()
            .to_string(),
        id
    );
    assert_eq!(
        t.ok(&[
            "tasks",
            "profile-update",
            "dctx0",
            "--file",
            "update.json",
            "--reason",
            "Adopt revised representation",
            "--json"
        ])["body"]["data"]["id"],
        plan["body"]["data"]["id"]
    );
    fs::remove_dir_all(t.0.join(".dctx0/profiles/rust")).unwrap();
    assert!(
        t.run(&["tasks", "show", "dctx0", &id, "--section", "implementation"])
            .status
            .success()
    );
}
#[test]
fn profile_benchmarks_measure_all_targets_and_reject_unbounded_persistence() {
    let t = Temp::new();
    for target in ["typescript", "java", "go", "rust"] {
        for phase in ["create", "load", "context-md", "context-json", "claim"] {
            let db = t.0.join(format!("{target}-{phase}.sqlite3"));
            let data = t.ok(&[
                "bench-profiles",
                "dctx0",
                target,
                phase,
                "2",
                "--db",
                db.to_str().unwrap(),
            ]);
            assert!(data["valid"].as_bool().unwrap());
            assert!(data["peak_rss_bytes"].as_u64().unwrap() > 0);
            assert!(data["elapsed_ms"].as_f64().unwrap() > 0.0);
        }
    }
    assert!(
        !t.run(&["bench-profiles", "dctx0", "rust", "claim", "0"])
            .status
            .success()
    );
    assert!(
        !t.run(&["bench-profiles", "dctx0", "rust", "claim"])
            .status
            .success()
    );
}

#[test]
fn all_target_providers_share_the_same_conformance_contract() {
    use dctx0_core::TargetProfileProvider;
    let providers: Vec<Box<dyn TargetProfileProvider>> = vec![
        Box::new(dctx0_target_typescript::Provider),
        Box::new(dctx0_target_java::Provider),
        Box::new(dctx0_target_go::Provider),
        Box::new(dctx0_target_rust::Provider),
    ];
    for (provider, target) in providers.iter().zip(["typescript", "java", "go", "rust"]) {
        let manifest = provider.manifest();
        assert_eq!(manifest.target, target);
        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.standard_keys, dctx0_core::standard_profile_keys());
        let templates = provider.templates();
        assert_eq!(templates, provider.templates());
        let paths = templates
            .iter()
            .map(|t| &t.path)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(paths.len(), 37);
        assert!(templates.iter().all(|t| !t.text.is_empty()));
        for key in manifest.standard_keys {
            assert!(paths.contains(&key.path()));
        }
    }
}
#[test]
fn preview_markdown_explains_legacy_context_and_missing_required_files() {
    let t = Temp::new();
    t.ok(&["profile", "rust", "create", "--json"]);
    t.ok(&["check", "dctx0", "--json"]);
    t.ok(&["tasks", "plan", "dctx0", "--json"]);
    let out = t.run(&["tasks", "profile-changes", "dctx0"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("legacy context is unknown, not drift")
    );
    fs::remove_file(t.0.join(".dctx0/profiles/rust/common.md")).unwrap();
    let out = t.run(&["tasks", "profile-changes", "dctx0"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("Required profile missing")
    );
}
