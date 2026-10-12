use dctx0_engine::Project;
use dctx0_tasks::*;
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "dctx0-task-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("spec.dctx0"),"workspace W { intent \"Benchmark and task intent\"; value Id = Int { range 1..10 } entity Record { id: Id; key id } message Request { id: Id; records: List<Record> } module M { surface Api { operation first(id: Id) -> Int { pure } operation second() -> Int { pure } } review R { inspect { contracts }; criteria { \"Follow contract\" } } } }").unwrap();
        let this = Self(p);
        this.project().check().unwrap();
        this
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
    fn cmd(&self, verb: &str, args: &[&str], json: bool) -> std::process::Output {
        let mut c = Command::new(env!("CARGO_BIN_EXE_dctx0"));
        c.args(["tasks", verb, self.0.to_str().unwrap()]).args(args);
        if json {
            c.arg("--json");
        }
        c.output().unwrap()
    }
    fn ok(&self, verb: &str, args: &[&str]) -> Value {
        let o = self.cmd(verb, args, true);
        assert!(
            o.status.success(),
            "{verb}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        serde_json::from_slice(&o.stdout).unwrap()
    }
    fn md(&self, verb: &str, args: &[&str]) -> String {
        let o = self.cmd(verb, args, false);
        assert!(
            o.status.success(),
            "{verb}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8(o.stdout).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn done(p: &Project, id: i64) {
    let s = p
        .task_action(Action::Start {
            task: id,
            actor: "helper".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    p.task_action(Action::Complete {
        task: id,
        actor: "helper".into(),
        claim: s.claim.unwrap(),
        version: s.version,
        result: TaskResult {
            summary: "Delivered".into(),
            artifacts: vec![],
            decisions: vec![],
            implementation: Some("commit1".into()),
        },
    })
    .unwrap();
}
#[test]
fn all_read_views_share_pinned_models_and_markdown_json_formats() {
    let t = Temp::new();
    assert!(t.md("preview", &[]).contains("nothing stored"));
    assert!(
        !t.ok("plan", &["--dry-run"])["body"]["data"]["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let plan = t.ok("plan", &[])["body"]["data"]["id"].as_i64().unwrap();
    assert!(t.md("plan", &[]).contains("Published"));
    assert!(
        t.cmd("list", &["--plan", &plan.to_string()], true)
            .status
            .success()
    );
    assert!(
        !t.cmd("release", &["T1", "--actor", "x", "--claim", "1"], true)
            .status
            .success()
    );
    let listed = t.ok("list", &["--limit", "1"]);
    let id = listed["body"]["data"]["items"][0]["task_id"]
        .as_i64()
        .unwrap()
        .to_string();
    let cursor = listed["body"]["data"]["next_cursor"].to_string();
    assert!(t.md("list", &["--limit", "1"]).contains("--cursor"));
    assert!(
        t.ok("list", &["--limit", "1", "--cursor", &cursor])["body"]["data"]["items"].is_array()
    );
    assert!(t.md("list", &["--all-plans"]).contains("wave"));
    for verb in ["waves", "next"] {
        assert!(t.cmd(verb, &[], true).status.success());
        assert!(!t.md(verb, &[]).is_empty());
    }
    let before_next = t.project().task_session(None, None).unwrap().event_seq;
    let next = t.md("next", &[]);
    assert!(next.contains("Assignment:"));
    assert!(next.contains("Specification:"));
    assert!(next.contains("Benchmark and task intent"));
    assert!(next.contains("range 1..10"));
    let next_json = t.ok("next", &["--budget", "1"]);
    let next_data = &next_json["body"]["data"];
    assert!(
        next_data["context"]["missing_sections"]
            .as_array()
            .is_some_and(|s| !s.is_empty())
    );
    let next_cursor = next_data["context"]["page"]["next_cursor"].to_string();
    let continued = t.ok("next", &["--budget", "1", "--cursor", &next_cursor]);
    assert_eq!(
        continued["body"]["data"]["task"]["task_id"],
        next_data["task"]["task_id"]
    );
    assert_ne!(
        continued["body"]["data"]["context"]["page"]["items"],
        next_data["context"]["page"]["items"]
    );
    assert_eq!(
        t.project().task_session(None, None).unwrap().event_seq,
        before_next
    );
    let show = t.md("show", &[&id]);
    assert!(show.contains("Assignment"));
    assert!(show.contains("Benchmark and task intent"));
    assert!(show.contains("range 1..10"));
    assert!(!show.contains("Change request"));
    assert!(!show.contains("Load `tasks context`"));
    let show_page = t.ok("show", &[&id, "--budget", "1"]);
    let show_cursor = show_page["body"]["data"]["context"]["page"]["next_cursor"].to_string();
    t.ok("show", &[&id, "--budget", "1", "--cursor", &show_cursor]);
    let request_id = t
        .project()
        .task_session(None, None)
        .unwrap()
        .plan
        .members
        .iter()
        .find(|m| m.candidate.key == "contract:W.Request")
        .unwrap()
        .task_id
        .to_string();
    let request = t.md("show", &[&request_id]);
    assert!(request.contains("message Request { id: Id; records: List<Record> }"));
    assert!(request.contains("entity Record { id: Id; key id }"));
    assert_eq!(request.matches("message Request {").count(), 1);
    let typed = t.ok("show", &[&request_id]);
    assert!(
        typed["body"]["data"]["context"]["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type_definition"]["id"] == "W.Request")
    );
    assert!(
        t.md("context", &[&id, "--budget", "1"])
            .contains("Required content")
    );
    let c =
        t.ok("context", &[&id, "--budget", "1"])["body"]["data"]["page"]["next_cursor"].to_string();
    t.ok("context", &[&id, "--budget", "1", "--cursor", &c]);
    assert!(t.md("history", &[&id]).contains("Events"));
    t.ok("history", &[&id, "--direction", "ancestors"]);
    assert!(
        t.md(
            "dependencies",
            &[&id, "--direction", "dependents", "--transitive"]
        )
        .contains("needs")
    );
    assert!(t.md("slice", &[&id]).contains("dependency-closed"));
    assert!(
        t.md("slice", &[&id, "--domain", "absent"])
            .contains("Blocked")
    );
    assert!(!t.cmd("show", &["T0"], false).status.success());
    assert!(
        t.md("diff", &[&format!("P{plan}"), &format!("P{plan}")])
            .contains("Kept")
    );
    assert!(
        t.md("next", &["--subject", "absent"])
            .contains("No matching")
    );
    assert!(t.cmd("list", &["--format", "json"], false).status.success());
    for (verb, args) in [
        ("list", vec!["--format", "wrong"]),
        ("list", vec!["--format", "markdown", "--json"]),
        ("list", vec!["--cursor", "bad"]),
        ("show", vec![]),
        ("show", vec!["-1"]),
        ("dependencies", vec![&id, "--direction", "wrong"]),
        ("list", vec!["--state", "wrong"]),
        ("diff", vec!["P1"]),
        ("start", vec![&id]),
        ("release", vec![&id, "--actor", "one"]),
        ("complete", vec![&id, "--actor", "one", "--claim", "1"]),
        ("record-check", vec![&id]),
    ] {
        assert!(
            !t.cmd(verb, &args, false).status.success(),
            "{verb} {args:?}"
        );
    }
    let p = t.project();
    let before = p.task_session(None, None).unwrap();
    let id = before.next(&Filters::default(), None).unwrap().task_id;
    done(&p, id);
    let after = p.task_session(None, None).unwrap();
    let historical = p
        .task_session(Some(before.plan.id), Some(before.event_seq))
        .unwrap();
    assert_eq!(historical.states[&id].work, WorkState::Open);
    assert_eq!(after.states[&id].work, WorkState::Done);
    assert!(
        t.md("show", &[&id.to_string()])
            .contains("Delivered result")
    );
}
#[test]
fn cli_mutations_return_claim_guards_and_accept_structured_reports() {
    let t = Temp::new();
    t.ok("plan", &[]);
    let claim = t.ok("claim-next", &["--actor", "one"]);
    let waves = t.ok("waves", &[]);
    assert_eq!(
        waves["body"]["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["progress"].as_u64().unwrap())
            .sum::<u64>(),
        1
    );
    let data = &claim["body"]["data"];
    let id = data["task_id"].as_i64().unwrap().to_string();
    let token = data["state"]["claim"].as_i64().unwrap().to_string();
    let version = data["state"]["version"].as_i64().unwrap().to_string();
    assert!(t.md("show", &[&id]).contains("Claim"));
    assert!(
        t.md(
            "release",
            &[
                &id,
                "--actor",
                "one",
                "--claim",
                &token,
                "--version",
                &version,
                "--reason",
                "Reassign"
            ]
        )
        .contains("Released")
    );
    let started = t.ok("start", &[&id, "--actor", "one"]);
    let token = started["body"]["data"]["state"]["claim"]
        .as_i64()
        .unwrap()
        .to_string();
    let version = started["body"]["data"]["state"]["version"]
        .as_i64()
        .unwrap()
        .to_string();
    assert!(
        t.md(
            "complete",
            &[
                &id,
                "--actor",
                "one",
                "--claim",
                &token,
                "--version",
                &version,
                "--summary",
                "Implemented",
                "--implementation",
                "commit1",
                "--artifact",
                "src/type.rs"
            ]
        )
        .contains("no testing PASS")
    );
    let waves = t.ok("waves", &[]);
    assert!(
        waves["body"]["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["done"].as_u64().unwrap() > 0)
    );
    let p = t.project();
    let s = p.task_session(None, None).unwrap();
    let evidence = CheckEvidence {
        requirements_hash: s.definitions[&id.parse().unwrap()]
            .candidate
            .requirements_hash
            .clone(),
        implementation: "commit1".into(),
        verifier: "cargo-test".into(),
        verifier_version: "1".into(),
        command: "cargo test".into(),
        started_at: "start".into(),
        finished_at: "finish".into(),
        report: "Executed report".into(),
        status: CheckStatus::Fail,
    };
    let file = t.0.join("check.json");
    fs::write(&file, serde_json::to_vec(&evidence).unwrap()).unwrap();
    assert!(
        t.md("record-check", &[&id, "--file", file.to_str().unwrap()])
            .contains("External check")
    );
    assert!(
        t.md("follow-up", &[&id, "--reason", "Fix failing contract"])
            .contains("Published")
    );
    assert!(
        t.md(
            "add-note",
            &[&id, "--reason", "Historical implementation hint"]
        )
        .contains("Note recorded")
    );
    assert!(t.md("history", &[&id]).contains("follow-up"));
    let history = t.ok("list", &["--all-plans"]);
    assert!(
        history["body"]["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["task_id"].as_i64().unwrap().to_string() == id)
    );
    let current = p.task_session(None, None).unwrap().plan;
    assert!(
        t.md(
            "diff",
            &[
                &current.parent.unwrap().to_string(),
                &current.id.to_string()
            ]
        )
        .contains("→")
    );
    let anchor = p
        .task_session(None, None)
        .unwrap()
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Operation)
        .unwrap()
        .task_id
        .to_string();
    let group = t.ok("claim-slice", &[&anchor, "--actor", "two"])["body"]["data"]["group_id"]
        .as_i64()
        .unwrap()
        .to_string();
    assert!(
        t.md(
            "release-slice",
            &[&group, "--actor", "two", "--reason", "Free future work"]
        )
        .contains("released")
    );
    let review = p
        .task_session(None, None)
        .unwrap()
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Review)
        .unwrap()
        .task_id;
    loop {
        let s = p.task_session(None, None).unwrap();
        if s.overviews()
            .iter()
            .find(|r| r.task_id == review)
            .unwrap()
            .ready
        {
            break;
        }
        done(&p, s.next(&Filters::default(), None).unwrap().task_id);
    }
    let r = t.ok("start", &[&review.to_string(), "--actor", "reviewer"]);
    let token = r["body"]["data"]["state"]["claim"]
        .as_i64()
        .unwrap()
        .to_string();
    let version = r["body"]["data"]["state"]["version"]
        .as_i64()
        .unwrap()
        .to_string();
    let file = t.0.join("review.json");
    fs::write(
        &file,
        serde_json::to_vec(&ReviewReport {
            conformance: None,
            reviewer: "human".into(),
            judgement: "approved".into(),
            summary: "Contract matched".into(),
            findings: vec![],
        })
        .unwrap(),
    )
    .unwrap();
    let recorded = t.ok(
        "record-review",
        &[
            &review.to_string(),
            "--actor",
            "reviewer",
            "--claim",
            &token,
            "--version",
            &version,
            "--file",
            file.to_str().unwrap(),
        ],
    );
    let version = recorded["body"]["data"]["state"]["version"]
        .as_i64()
        .unwrap()
        .to_string();
    let file = t.0.join("result.json");
    fs::write(
        &file,
        serde_json::to_vec(&TaskResult {
            summary: "Reviewed".into(),
            artifacts: vec![],
            decisions: vec![],
            implementation: None,
        })
        .unwrap(),
    )
    .unwrap();
    t.ok(
        "complete",
        &[
            &review.to_string(),
            "--actor",
            "reviewer",
            "--claim",
            &token,
            "--version",
            &version,
            "--file",
            file.to_str().unwrap(),
        ],
    );
    assert!(
        !t.cmd("record-check", &[&id, "--file", "/does-not-exist"], false)
            .status
            .success()
    );
    fs::write(&file, "not-json").unwrap();
    assert!(
        !t.cmd(
            "record-check",
            &[&id, "--file", file.to_str().unwrap()],
            false
        )
        .status
        .success()
    );
}

#[test]
fn task_benchmarks_measure_every_phase_and_reject_project_state_and_empty_inputs() {
    let t = Temp::new();
    let path = t.0.to_str().unwrap();
    let db = t.0.join("bench.sqlite3");
    for phase in [
        "tasks-plan",
        "tasks-reconcile",
        "tasks-publish",
        "tasks-list",
        "tasks-context-md",
        "tasks-context-json",
        "tasks-slice",
        "tasks-claim",
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["bench", path, phase, "1", "--db", db.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let value: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert!(value["elapsed_ms"].as_f64().unwrap() > 0.0);
        assert!(value["peak_rss_bytes"].as_u64().unwrap() > 0);
    }
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["bench", path, "tasks-plan"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let empty_db = t.0.join("empty-bench.sqlite3");
    fs::write(t.0.join("spec.dctx0"), "workspace W {}").unwrap();
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args([
                "bench",
                path,
                "tasks-plan",
                "--db",
                empty_db.to_str().unwrap()
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::write(t.0.join("spec.dctx0"), "workspace W { type T = Int }").unwrap();
    assert!(
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["bench", path, "tasks-plan", "--db", db.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !t.cmd("start", &["T1", "--actor", "one", "--dry-run"], false)
            .status
            .success()
    );
}
