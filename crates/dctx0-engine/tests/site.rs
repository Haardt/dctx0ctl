use dctx0_engine::{Project, site_html};
use dctx0_store::SiteSelection;
use dctx0_tasks::{Action, TaskResult};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-site-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap()
    }
}
const SPEC: &str = "workspace W { intent \"Text </script><script>unsafe()</script>\"; value A = Int { range 1..10 }; surface Api { operation get(a: A) -> A { pure } } }";
#[test]
fn export_is_deterministic_preserves_unmanaged_files_and_escapes_sources() {
    let temp = Temp::new();
    fs::write(temp.0.join("spec.dctx0"), SPEC).unwrap();
    let p = temp.project();
    assert!(p.site_snapshot(&SiteSelection::default()).is_err());
    assert!(!p.database.exists());
    assert!(p.check().unwrap().ok);
    let data = p.site_snapshot(&SiteSelection::default()).unwrap();
    assert!(data.records.tasks.is_none());
    assert!(data.records.runs.is_empty());
    let html = site_html(&data, false).unwrap();
    assert!(!html.contains("<script>unsafe()"));
    assert!(html.contains("\\u003c/script\\u003e"));
    assert!(html.contains("window.__SITE_LIVE__=false"));
    let before = fs::read(&p.database).unwrap();
    let out = temp.0.join("export");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("notes.txt"), "keep").unwrap();
    p.site_export(&SiteSelection::default(), &out).unwrap();
    let first = fs::read(out.join("index.html")).unwrap();
    let collision = temp.0.join("collision");
    fs::create_dir(&collision).unwrap();
    fs::write(collision.join("index.html"), "authored page").unwrap();
    assert!(
        p.site_export(&SiteSelection::default(), &collision)
            .unwrap_err()
            .to_string()
            .contains("Preserving")
    );
    assert_eq!(
        fs::read_to_string(collision.join("index.html")).unwrap(),
        "authored page"
    );

    p.site_export(&SiteSelection::default(), &out).unwrap();
    assert_eq!(first, fs::read(out.join("index.html")).unwrap());
    assert_eq!(fs::read_to_string(out.join("notes.txt")).unwrap(), "keep");
    assert_eq!(before, fs::read(&p.database).unwrap());
    let wire: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("snapshot.json")).unwrap()).unwrap();
    assert!(wire["records"]["check"]["run_id"].is_string());
    fs::write(temp.0.join("blocked"), "file").unwrap();
    assert!(
        p.site_export(&SiteSelection::default(), &temp.0.join("blocked"))
            .is_err()
    );
}
#[test]
fn pinned_plans_history_changes_and_done_without_pass_are_projected() {
    let temp = Temp::new();
    fs::write(temp.0.join("spec.dctx0"), SPEC).unwrap();
    let p = temp.project();
    assert!(p.check().unwrap().ok);
    let plan = p.task_plan("latest").unwrap();
    let initial = p.task_session(None, None).unwrap();
    let ready = initial.overviews().into_iter().find(|t| t.ready).unwrap();
    let started = p
        .task_action(Action::Start {
            task: ready.task_id,
            actor: "site-test".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    p.task_action(Action::Complete {
        task: ready.task_id,
        actor: "site-test".into(),
        claim: started.claim.unwrap(),
        version: started.version,
        result: TaskResult {
            summary: "Delivered".into(),
            artifacts: vec![],
            decisions: vec![],
            implementation: None,
        },
    })
    .unwrap();
    let frozen = p
        .site_snapshot(&SiteSelection {
            plan: Some(plan.id),
            as_of: Some(initial.event_seq),
            snapshot: None,
        })
        .unwrap();
    assert_eq!(
        frozen
            .tasks
            .iter()
            .find(|t| t.task_id == ready.task_id)
            .unwrap()
            .work,
        dctx0_tasks::WorkState::Open
    );
    let live = p.site_snapshot(&SiteSelection::default()).unwrap();
    let row = live
        .tasks
        .iter()
        .find(|t| t.task_id == ready.task_id)
        .unwrap();
    assert_eq!(row.work, dctx0_tasks::WorkState::Done);
    assert_eq!(row.check, dctx0_tasks::CheckStatus::Unverified);
    assert!(!live.dependencies.is_empty());
    fs::write(temp.0.join("spec.dctx0"), SPEC.replace("1..10", "1..20")).unwrap();
    assert!(p.check().unwrap().ok);
    let mismatch = p.site_snapshot(&SiteSelection::default()).unwrap();
    assert_ne!(
        mismatch.records.snapshot.id,
        mismatch.records.tasks.as_ref().unwrap().plan.snapshot_id
    );
    let new = p.task_plan("latest").unwrap();
    let change = p.site_snapshot(&SiteSelection::default()).unwrap();
    assert_ne!(new.id, plan.id);
    assert!(change.records.plan_diff.is_some());
    assert!(!change.records.changes.unwrap().changes.is_empty());
    let historic = p
        .site_snapshot(&SiteSelection {
            snapshot: Some(plan.snapshot_id),
            plan: Some(plan.id),
            as_of: Some(initial.event_seq),
        })
        .unwrap();
    assert!(historic.records.before.is_none());
    assert!(
        p.site_snapshot(&SiteSelection {
            plan: Some(i64::MAX),
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        p.site_snapshot(&SiteSelection {
            as_of: Some(i64::MAX),
            ..Default::default()
        })
        .is_err()
    );
    let renamed = p.rename("W.A", "Input", false).unwrap();
    assert_eq!(renamed.new_symbol, "W.Input");
    assert!(
        !p.site_snapshot(&SiteSelection::default())
            .unwrap()
            .records
            .renames
            .is_empty()
    );
}
#[test]
fn old_schemas_are_read_without_migration_and_missing_heads_are_explicit() {
    let temp = Temp::new();
    fs::write(temp.0.join("spec.dctx0"), SPEC).unwrap();
    let p = temp.project();
    p.check().unwrap();
    let conn = rusqlite::Connection::open(&p.database).unwrap();
    conn.execute_batch("DROP TABLE task_heads; PRAGMA user_version=1;")
        .unwrap();
    drop(conn);
    let before = fs::read(&p.database).unwrap();
    let data = p.site_snapshot(&SiteSelection::default()).unwrap();
    assert!(data.records.tasks.is_none());
    assert!(data.records.runs.is_empty());
    assert_eq!(before, fs::read(&p.database).unwrap());
    assert!(
        p.site_snapshot(&SiteSelection {
            as_of: Some(0),
            ..Default::default()
        })
        .is_err()
    );
    let conn = rusqlite::Connection::open(&p.database).unwrap();
    conn.execute_batch("PRAGMA user_version=5;").unwrap();
    drop(conn);
    assert!(
        p.site_snapshot(&SiteSelection::default())
            .unwrap_err()
            .to_string()
            .contains("schema")
    );
}
#[test]
fn stored_evidence_and_large_occurrence_ids_are_lossless() {
    use dctx0_store::{AccessMode, Store, VerificationArtifact};
    let temp = Temp::new();
    fs::write(temp.0.join("spec.dctx0"), SPEC).unwrap();
    let p = temp.project();
    p.check().unwrap();
    let conn = rusqlite::Connection::open(&p.database).unwrap();
    conn.execute("UPDATE checks SET id=?1", [9_007_199_254_740_993_i64])
        .unwrap();
    drop(conn);
    let snapshot = p.site_snapshot(&SiteSelection::default()).unwrap();
    let artifact = VerificationArtifact::new("application/json", b"{}".to_vec());
    let issue = serde_json::json!({"code":"TEST_IMPORTED","message":"Explicit imported test evidence, not a native run","span":null});
    let run:dctx0_core::VerificationRun=serde_json::from_value(serde_json::json!({"schema_version":1,"origin":"imported","started_at_unix_ns":"0","elapsed_us":0,"provenance":{"snapshot":snapshot.records.snapshot.id,"check_occurrence":snapshot.records.check.run_id,"plan_hash":artifact.reference.hash,"implementation":{},"dependencies":{},"adapter_hash":"","configuration_hash":"","harness_hash":"","interpreter_version":"test","generator_version":"test","protocol_version":1,"target":"test","runtime_version":"not-executed","build_configuration":{},"task_pin":null,"incomplete":[issue]},"budgets":dctx0_core::VerificationBudgets::default(),"selected_subjects":["W.A"],"required_subjects":["W.A"],"not_selected":[],"required_cases":["unexecuted"],"cases":[],"open_obligations":[issue],"bounded_obligations":[],"artifacts":[artifact.reference],"result":{"status":"UNKNOWN","issue":issue}})).unwrap();
    let mut run = run;
    run.cases.push(dctx0_core::VerificationCaseRecord {
        id: "unexecuted".into(),
        subject: "W.A".into(),
        kind: dctx0_core::VerificationCaseKind::Contract,
        bindings: std::collections::BTreeMap::from([(
            "value".into(),
            dctx0_core::VerificationValue::Integer("9007199254740993".into()),
        )]),
        setup: vec![],
        arguments: vec![],
        observations: vec![],
        events: None,
        resources: None,
        states: vec![],
        result: run.result.clone(),
        elapsed_us: u64::MAX,
    });
    let mut store = Store::open(&p.database, AccessMode::ReadWrite).unwrap();
    store
        .record_verification_run(&p.key(), &run, &[artifact])
        .unwrap();
    drop(store);
    let records = p.site_snapshot(&SiteSelection::default()).unwrap();
    assert_eq!(records.records.runs.len(), 1);
    let wire = serde_json::to_value(records).unwrap();
    assert_eq!(
        wire["records"]["runs"][0]["run"]["provenance"]["check_occurrence"],
        "9007199254740993"
    );
    assert_eq!(wire["records"]["check"]["run_id"], "9007199254740993");
    use base64::Engine;
    use std::io::Read;
    let blob = wire["records"]["runs"][0]["run"]["case_details"]
        .as_str()
        .unwrap();
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(blob)
        .unwrap();
    let mut json = String::new();
    flate2::read::GzDecoder::new(compressed.as_slice())
        .read_to_string(&mut json)
        .unwrap();
    let cases: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(cases["cases"][0]["elapsed_us"], "18446744073709551615");
    assert_eq!(
        cases["cases"][0]["bindings"]["value"]["value"],
        "9007199254740993"
    );
    assert_eq!(wire["records"]["runs"][0]["run"]["case_count"], "1");
}
#[test]
fn reserved_work_stays_open_and_exports_preserve_authored_edits() {
    let temp = Temp::new();
    fs::write(temp.0.join("spec.dctx0"), SPEC).unwrap();
    let p = temp.project();
    p.check().unwrap();
    let plan = p.task_plan("latest").unwrap();
    let anchor = plan
        .members
        .iter()
        .find(|m| m.candidate.kind == dctx0_tasks::TaskKind::Operation)
        .unwrap()
        .task_id;
    p.task_action(Action::ClaimSlice {
        anchor,
        actor: "reserved-worker".into(),
        domain: None,
    })
    .unwrap();
    let data = p.site_snapshot(&SiteSelection::default()).unwrap();
    let reserved = data.tasks.iter().find(|t| t.task_id == anchor).unwrap();
    assert!(reserved.reservation.is_some());
    assert_eq!(reserved.work, dctx0_tasks::WorkState::Open);
    assert!(!reserved.delivered);
    assert_eq!(reserved.actor.as_deref(), Some("reserved-worker"));
    let output = temp.0.join("export");
    p.site_export(&SiteSelection::default(), &output).unwrap();
    fs::write(output.join("index.html"), "authored edit").unwrap();
    assert!(p.site_export(&SiteSelection::default(), &output).is_err());
    assert_eq!(
        fs::read_to_string(output.join("index.html")).unwrap(),
        "authored edit"
    );
    fs::write(
        output.join(".dctx0-site.json"),
        "{\"schema_version\":2,\"files\":{}}",
    )
    .unwrap();
    assert!(
        p.site_export(&SiteSelection::default(), &output)
            .unwrap_err()
            .to_string()
            .contains("Unsupported")
    );
}
