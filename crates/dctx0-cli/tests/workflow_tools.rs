use serde_json::{Value, json};
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
        let root = std::env::temp_dir().join(format!(
            "dctx0-workflow-tools-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("spec")).unwrap();
        fs::create_dir(root.join("app")).unwrap();
        fs::write(root.join("spec/w.dctx0"),"workspace W { value Id = Int\nreview Boundary { inspect { implementation }\ncriteria { \"Review the actual factory\" } } }").unwrap();
        Self(root)
    }
    fn cmd(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let output = self.cmd(args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn package_apply_accepts_direct_and_enveloped_inputs_without_a_spec_or_history() {
    let t = Temp::new();
    t.ok(&["check", "spec", "--json"]);
    t.ok(&["tasks", "plan", "spec", "--json"]);
    let listed = t.ok(&["tasks", "list", "spec", "--json"]);
    let task = format!("T{}", listed["body"]["data"]["items"][0]["task_id"]);
    let package = t.ok(&["tasks", "package", "spec", &task, "--json"]);
    fs::write(t.0.join("package.json"), package.to_string()).unwrap();
    let delta = t.ok(&[
        "tasks",
        "package-delta",
        "spec",
        &task,
        "--file",
        "package.json",
        "--json",
    ]);
    fs::write(t.0.join("delta.json"), delta.to_string()).unwrap();
    let run = || {
        t.ok(&[
            "tasks",
            "package-apply",
            "--file",
            "package.json",
            "--delta",
            "delta.json",
            "--json",
        ])
    };
    assert_eq!(run(), package["body"]["data"]);
    fs::write(
        t.0.join("package.json"),
        package["body"]["data"].to_string(),
    )
    .unwrap();
    fs::write(t.0.join("delta.json"), delta["body"]["data"].to_string()).unwrap();
    // A non-existent positional spec demonstrates no Project or Store is opened.
    assert_eq!(
        t.ok(&[
            "tasks",
            "package-apply",
            "missing-spec",
            "--file",
            "package.json",
            "--delta",
            "delta.json",
            "--json"
        ]),
        run()
    );
    for format in [vec![], vec!["--format", "markdown"]] {
        let mut arguments = vec![
            "tasks",
            "package-apply",
            "--file",
            "package.json",
            "--delta",
            "delta.json",
        ];
        arguments.extend(format);
        let output = t.cmd(&arguments);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("## Work package"));
    }
    assert_eq!(
        t.ok(&[
            "tasks",
            "package-apply",
            "--file",
            "package.json",
            "--delta",
            "delta.json",
            "--format",
            "json"
        ]),
        run()
    );
    let missing_delta = t.cmd(&["tasks", "package-apply", "--file", "package.json"]);
    assert!(!missing_delta.status.success());
    assert!(String::from_utf8_lossy(&missing_delta.stderr).contains("--delta is required"));
    assert!(
        !t.cmd(&["tasks", "list", "spec", "--delta", "delta.json"])
            .status
            .success()
    );
    let mut corrupt = delta["body"]["data"].clone();
    corrupt["after"] = json!("bad");
    fs::write(t.0.join("delta.json"), corrupt.to_string()).unwrap();
    assert!(
        !t.cmd(&[
            "tasks",
            "package-apply",
            "--file",
            "package.json",
            "--delta",
            "delta.json"
        ])
        .status
        .success()
    );
}
#[test]
fn codec_cli_reports_explicit_feature_availability() {
    let t = Temp::new();
    let output = t.cmd(&[
        "verify",
        "scaffold-codecs",
        "spec",
        "--project",
        "app",
        "--target",
        "go",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("only typescript"));
    #[cfg(not(feature = "typescript"))]
    assert!(
        String::from_utf8_lossy(
            &t.cmd(&["verify", "scaffold-codecs", "spec", "--project", "app"])
                .stderr
        )
        .contains("TypeScript target feature")
    );
    #[cfg(not(feature = "typescript"))]
    assert!(
        String::from_utf8_lossy(&t.cmd(&["bench", "spec", "verification-codecs"]).stderr)
            .contains("TypeScript target feature")
    );
}
#[cfg(feature = "typescript")]
#[test]
fn codec_cli_merges_config_explicit_overrides_preserves_edits_and_requires_a_check() {
    let t = Temp::new();
    let args = [
        "verify",
        "scaffold-codecs",
        "spec",
        "--project",
        "app",
        "--dry-run",
        "--json",
    ];
    assert!(!t.cmd(&args).status.success());
    t.ok(&["check", "spec", "--json"]);
    let config = json!({"schema_version":1,"integer":"bigint","option":"undefined","enum":"tagged","types":{"W.Id":"bigint"}});
    fs::write(t.0.join("config.json"), config.to_string()).unwrap();
    let report = t.ok(&[
        "verify",
        "scaffold-codecs",
        "spec",
        "--project",
        "app",
        "--config",
        "config.json",
        "--integer",
        "number",
        "--type",
        "W.Id=number",
        "--subject",
        "W.Id",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(report["configuration"]["integer"], "number");
    assert_eq!(report["configuration"]["option"], "undefined");
    assert_eq!(report["configuration"]["types"]["W.Id"], "number");
    assert_eq!(report["status"], "proposed");
    assert!(
        report["artifact"]["contents"]
            .as_str()
            .unwrap()
            .contains("export function encode")
    );
    assert!(!t.0.join("app/.dctx0").exists());
    let create = [
        "verify",
        "scaffold-codecs",
        "spec",
        "--project",
        "app",
        "--json",
    ];
    assert_eq!(t.ok(&create)["status"], "created");
    fs::write(
        t.0.join("app/.dctx0/testing/typescript/wire-codecs.ts"),
        "// custom",
    )
    .unwrap();
    assert_eq!(t.ok(&create)["status"], "preserved");
    for extra in [
        vec!["--type", "W.Id=number", "--type", "W.Id=bigint"],
        vec!["--type", "invalid"],
        vec!["--type", "W.Id=tagged"],
        vec!["--type", "W.Id=nope"],
        vec!["--type", "Missing=number"],
        vec!["--output", "../escape.ts"],
    ] {
        let mut invalid = create.to_vec();
        invalid.extend(extra);
        assert!(!t.cmd(&invalid).status.success());
    }
    fs::write(
        t.0.join("spec/w.dctx0"),
        "workspace W { value Id = String }",
    )
    .unwrap();
    assert!(!t.cmd(&create).status.success());
}
#[cfg(feature = "typescript")]
#[test]
fn review_changes_produces_an_empty_template_and_reports_spec_drift_without_events() {
    let t = Temp::new();
    t.ok(&["check", "spec", "--json"]);
    t.ok(&["tasks", "plan", "spec", "--json"]);
    t.ok(&[
        "verify",
        "init",
        "spec",
        "--project",
        "app",
        "--target",
        "typescript",
        "--subject",
        "W",
        "--json",
    ]);
    let list = t.ok(&["tasks", "list", "spec", "--kind", "review", "--json"]);
    let id = format!("T{}", list["body"]["data"]["items"][0]["task_id"]);
    let args = [
        "verify",
        "review-changes",
        "spec",
        "--project",
        "app",
        "--task",
        &id,
        "--json",
    ];
    let report = t.ok(&args);
    assert_eq!(report["changes"]["comparable"], false);
    assert_eq!(report["template"]["reviewer"], "");
    assert_eq!(report["template"]["judgement"], "request-changes");
    let event = report["event_seq"].clone();
    fs::write(
        t.0.join("spec/w.dctx0"),
        "workspace W { value Id = String }",
    )
    .unwrap();
    let drift = t.ok(&args);
    assert!(drift["template"].is_null());
    assert!(!drift["blockers"].as_array().unwrap().is_empty());
    assert_eq!(drift["event_seq"], event);
}
