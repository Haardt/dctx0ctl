//! Real CLI, compact metadata and no implicit status/preflight publication.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-cli-status-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("spec")).unwrap();
        fs::write(
            root.join("spec/w.dctx0"),
            "workspace W { value A = Int { range 1..2 } }",
        )
        .unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str], success: bool) -> Value {
        let out = self.run(args);
        assert_eq!(
            out.status.success(),
            success,
            "{args:?}: {} / {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn configure(&self) {
        let directory = self.0.join("app/.dctx0/testing");
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("adapter.ts"),
            "throw Error('runtime must not start during preflight')",
        )
        .unwrap();
        fs::write(
            directory.join("fixtures.json"),
            json!({"examples":{}}).to_string(),
        )
        .unwrap();
        fs::write(directory.join("verification.json"),json!({"schema_version":1,"target":"typescript","runtime":"bun","adapter":".dctx0/testing/adapter.ts","fixtures":".dctx0/testing/fixtures.json","scope":{"subjects":["W.A"],"tasks":[]},"budgets":dctx0_core::VerificationBudgets::default()}).to_string()).unwrap();
    }
    fn db(&self) -> PathBuf {
        self.0.join("spec/.dctx0ctl/state.sqlite3")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn status_reports_unpublished_open_and_changed_sources_in_one_read_only_command() {
    let f = Fixture::new();
    let empty = f.json(&["status", "spec", "--json"], true);
    assert!(empty["snapshot"].is_null());
    assert!(!f.0.join("spec/.dctx0ctl").exists());
    assert!(
        !f.run(&["status", "spec", "--since-plan", "1", "--since-event", "0"])
            .status
            .success()
    );
    f.json(&["check", "spec", "--json"], true);
    f.json(&["tasks", "plan", "spec", "--json"], true);
    let bytes = fs::read(f.db()).unwrap();
    let status = f.json(&["status", "spec", "--json"], true);
    assert_eq!(status["spec_current"], true);
    assert_eq!(status["task_plan_current"], true);
    assert_eq!(status["tasks"]["open"], 1);
    assert_eq!(status["tasks"]["done_pass"], 0);
    assert!(status["latest_run"].is_null());
    assert!(status["progress"].is_null());
    assert!(status["next_causes"].as_array().unwrap().len() <= 3);
    let out = f.run(&["status", "spec"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Tasks:"));
    assert!(out.stdout.len() < 4096);
    let pin = &status["task_pin"];
    let plan = pin["plan_id"].to_string();
    let event = pin["event_seq"].to_string();
    let progress = f.json(
        &[
            "status",
            "spec",
            "--since-plan",
            &plan,
            "--since-event",
            &event,
            "--json",
        ],
        true,
    );
    assert_eq!(progress["progress"]["newly_done"], 0);
    assert!(
        !f.run(&["status", "spec", "--since-event", &event])
            .status
            .success()
    );
    fs::write(
        f.0.join("spec/w.dctx0"),
        "workspace W { value A = Int { range 1..3 } }",
    )
    .unwrap();
    let changed = f.json(&["status", "spec", "--json"], true);
    assert_eq!(changed["spec_current"], false);
    assert_eq!(changed["next_causes"][0]["code"], "SPEC_CHECK_REQUIRED");
    assert_eq!(bytes, fs::read(f.db()).unwrap());
}

#[test]
fn status_keeps_task_counts_when_configuration_or_source_freshness_cannot_be_read() {
    let f = Fixture::new();
    f.json(&["check", "spec", "--json"], true);
    f.json(&["tasks", "plan", "spec", "--json"], true);
    f.configure();
    let path = f.0.join("app/.dctx0/testing/verification.json");
    let mut config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    config["capture"] = json!({"max_file_bytes":1,"max_tree_bytes":536870912});
    fs::write(&path, config.to_string()).unwrap();
    let report = f.json(&["status", "spec", "--project", "app", "--json"], true);
    assert_eq!(report["tasks"]["open"], 1);
    assert_eq!(
        report["issues"][0]["code"],
        "STATUS_SOURCE_FRESHNESS_UNAVAILABLE"
    );
    fs::write(path, "invalid JSON").unwrap();
    let report = f.json(&["status", "spec", "--project", "app", "--json"], true);
    assert_eq!(report["tasks"]["open"], 1);
    assert_eq!(
        report["issues"][0]["code"],
        "STATUS_CONFIGURATION_UNAVAILABLE"
    );
    let human = f.run(&["status", "spec", "--project", "app"]);
    assert!(human.status.success());
    assert!(
        String::from_utf8_lossy(&human.stdout).contains("Unknown STATUS_CONFIGURATION_UNAVAILABLE")
    );
}

#[test]
fn preflight_pages_blockers_and_diagnostic_mode_does_not_invent_a_task_pin() {
    let f = Fixture::new();
    f.json(&["check", "spec", "--json"], true);
    f.configure();
    let bytes = fs::read(f.db()).unwrap();
    let base = ["verify", "preflight", "spec", "--project", "app", "--json"];
    let delivery = f.json(&base, false);
    assert_eq!(delivery["executed"], false);
    assert_eq!(delivery["intent"], "delivery");
    assert_eq!(delivery["blockers"]["ADOPTION_TASK_PIN_REQUIRED"], 1);
    assert_eq!(delivery["potential_task_count"], 0);
    assert!(delivery["task_pin"].is_null());
    let diagnostic = f.json(
        &[
            "verify",
            "preflight",
            "spec",
            "--project",
            "app",
            "--diagnostic",
            "--json",
        ],
        true,
    );
    assert_eq!(diagnostic["intent"], "diagnostic");
    assert_eq!(diagnostic["required_cases"], delivery["required_cases"]);
    assert_eq!(diagnostic["plan_hash"], delivery["plan_hash"]);
    assert_eq!(diagnostic["bindings_checked"], false);
    assert_eq!(diagnostic["warnings"]["PREFLIGHT_FIXTURE_UNKNOWN"], 1);
    let full = f.json(
        &[
            "verify",
            "preflight",
            "spec",
            "--project",
            "app",
            "--diagnostic",
            "--full",
            "--json",
        ],
        true,
    );
    assert!(full["pin"]["inputs_hash"].is_string());
    let human = f.run(&[
        "verify",
        "preflight",
        "spec",
        "--project",
        "app",
        "--diagnostic",
    ]);
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("Preflight Diagnostic"));
    let page = f.json(
        &[
            "verify",
            "preflight",
            "spec",
            "--project",
            "app",
            "--limit",
            "1",
            "--offset",
            "1",
            "--json",
        ],
        false,
    );
    assert_eq!(page["issues"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["issues"]["complete"], false);
    assert!(
        !f.run(&[
            "verify",
            "preflight",
            "spec",
            "--project",
            "app",
            "--limit",
            "0"
        ])
        .status
        .success()
    );
    assert_eq!(bytes, fs::read(f.db()).unwrap());
    assert!(!f.0.join("app/.dctx0ctl").exists());
}

#[test]
fn generated_placeholder_is_known_unavailable_while_edited_bindings_stay_unproved() {
    if !cfg!(feature = "typescript") {
        return;
    }
    let f = Fixture::new();
    f.json(&["check", "spec", "--json"], true);
    fs::create_dir_all(f.0.join("app")).unwrap();
    f.json(
        &[
            "verify",
            "init",
            "spec",
            "--project",
            "app",
            "--target",
            "typescript",
            "--subject",
            "W.A",
            "--json",
        ],
        true,
    );
    f.json(
        &[
            "profile",
            "typescript",
            "create",
            "--project",
            "app",
            "--default",
            "--json",
        ],
        true,
    );
    f.json(&["tasks", "plan", "spec", "--json"], true);
    let placeholder = f.json(
        &["verify", "preflight", "spec", "--project", "app", "--json"],
        false,
    );
    assert_eq!(placeholder["blockers"]["PREFLIGHT_BINDINGS_UNAVAILABLE"], 1);
    fs::write(
        f.0.join("app/.dctx0/testing/typescript/adapter.ts"),
        "// authored adapter under development\nthrow Error('unavailable')",
    )
    .unwrap();
    let edited = f.json(
        &["verify", "preflight", "spec", "--project", "app", "--json"],
        true,
    );
    assert_eq!(edited["bindings_checked"], false);
    assert_eq!(edited["warnings"]["PREFLIGHT_BINDINGS_UNKNOWN"], 1);
    assert_eq!(edited["potential_task_count"], 1);
    let human = f.run(&["verify", "preflight", "spec", "--project", "app"]);
    assert!(human.status.success());
    assert!(
        String::from_utf8_lossy(&human.stdout)
            .contains("Potential tasks if all proof/review/transaction guards pass")
    );
    let diagnostic = f.json(
        &[
            "verify",
            "run",
            "--reuse",
            "off",
            "spec",
            "--project",
            "app",
            "--diagnostic",
            "--json",
        ],
        false,
    );
    assert_eq!(diagnostic["summary"]["result"]["status"], "UNKNOWN");
    assert_eq!(diagnostic["extra"]["preflight"]["intent"], "diagnostic");
    let plan = edited["task_pin"]["plan_id"].to_string();
    let event = edited["task_pin"]["event_sequence"].to_string();
    let status = f.run(&[
        "status",
        "spec",
        "--project",
        "app",
        "--since-plan",
        &plan,
        "--since-event",
        &event,
    ]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(String::from_utf8_lossy(&status.stdout).contains("Latest run:"));
    assert!(String::from_utf8_lossy(&status.stdout).contains("Since P"));
}

#[test]
fn delivery_run_refuses_unpinned_scope_before_building_or_starting_adapter() {
    if !cfg!(feature = "typescript") {
        return;
    }
    let f = Fixture::new();
    f.json(&["check", "spec", "--json"], true);
    f.configure();
    let bytes = fs::read(f.db()).unwrap();
    let report = f.json(
        &[
            "verify",
            "run",
            "--reuse",
            "off",
            "spec",
            "--project",
            "app",
            "--json",
        ],
        false,
    );
    assert_eq!(report["executed"], false);
    assert_eq!(report["blockers"]["ADOPTION_TASK_PIN_REQUIRED"], 1);
    assert_eq!(bytes, fs::read(f.db()).unwrap());
    assert_eq!(
        f.json(&["verify", "history", "spec", "--json"], true),
        json!([])
    );
    assert!(!Path::new(&f.0.join("app/.dctx0ctl")).exists());
}
