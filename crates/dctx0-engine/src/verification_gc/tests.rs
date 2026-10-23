use super::*;
use dctx0_changes::{HistoryReader, HistoryWriter};
use dctx0_core::*;
use dctx0_store::VerificationArtifact;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-storage-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("w.dctx0"), "workspace W { type A = Int }").unwrap();
        Self(root)
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn storage_recovery_rejects_invalid_journals_without_changing_live_history() {
    let t = Temp::new();
    let project = t.project();
    assert!(project.check().unwrap().ok);
    let original = fs::read(&project.database).unwrap();
    let journal = project.database.with_extension("storage-install.json");
    for value in [
        serde_json::json!({"source":project.database}),
        serde_json::json!({"source":"different-project","backup":t.0.join("outside.sqlite3")}),
    ] {
        fs::write(&journal, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(project.storage_recover().is_err());
        assert_eq!(fs::read(&project.database).unwrap(), original);
        assert!(journal.exists());
    }
}
fn run(p: &Project) -> dctx0_store::StoredVerificationRun {
    let mut store = p.store(AccessMode::ReadWrite).unwrap();
    let snapshot = store.latest(&p.key()).unwrap().unwrap();
    let plan = VerificationArtifact::new("application/json", b"{}".to_vec());
    let run=serde_json::from_value(serde_json::json!({
        "schema_version":1,"origin":"imported","started_at_unix_ns":"1","elapsed_us":0,
        "provenance":{"snapshot":snapshot.id,"check_occurrence":store.history(&p.key()).unwrap()[0].run_id,
        "plan_hash":plan.reference.hash,"implementation":{},"dependencies":{},"adapter_hash":"a","configuration_hash":"c","harness_hash":"h","interpreter_version":"1","generator_version":"1","protocol_version":1,"target":"typescript","runtime_version":"not-executed","build_configuration":{},"task_pin":null,"incomplete":[]},
        "budgets":VerificationBudgets::default(),"selected_subjects":["W.A"],"required_subjects":["W.A"],"not_selected":[],"required_cases":["C"],
        "cases":[{"id":"C","subject":"W.A","kind":"contract","bindings":{"value":{"kind":"string","value":"large".repeat(20000)}},"setup":[],"arguments":[],"observations":[],"events":null,"states":[],"result":{"status":"PASS"},"elapsed_us":0}],
        "open_obligations":[],"bounded_obligations":[],"artifacts":[plan.reference],"result":{"status":"PASS"}})).unwrap();
    store
        .record_verification_run_owned(&p.key(), run, &[plan])
        .unwrap()
}
#[test]
fn storage_engine_compaction_preview_exports_and_restart_keep_history_honest() {
    let t = Temp::new();
    let p = t.project();
    assert!(p.check().unwrap().ok);
    let saved = run(&p);
    let mut store = p.store(AccessMode::ReadWrite).unwrap();
    let artifact = store
        .verification_artifact(&saved.id, &saved.run.artifacts[0].hash)
        .unwrap();
    let mut next = saved.run.clone();
    for n in 2..=5 {
        next.started_at_unix_ns = n.to_string();
        store
            .record_verification_run(&p.key(), &next, std::slice::from_ref(&artifact))
            .unwrap();
    }
    drop(store);
    let exports = t.0.join("exports");
    fs::create_dir(&exports).unwrap();
    let body = serde_json::json!({"evidence":saved,"siblings":[null,true,false,1,-2,1.2,"string",{"unrelated":"keep"}]});
    fs::write(
        exports.join("report.json"),
        serde_json::to_vec(&body).unwrap(),
    )
    .unwrap();
    fs::write(exports.join("unknown.json"), "{\"authored\":true}").unwrap();
    fs::write(exports.join("invalid.json"), "{invalid}").unwrap();
    let before = fs::read(&p.database).unwrap();
    let export_before = fs::read(exports.join("report.json")).unwrap();
    let preview = p.verification_gc(true, true, Some(&exports)).unwrap();
    assert!(preview.storage.dry_run);
    assert_eq!(before, fs::read(&p.database).unwrap());
    assert_eq!(
        export_before,
        fs::read(exports.join("report.json")).unwrap()
    );
    assert!(preview.exports.iter().any(|e| e.status == "would-reduce"));
    // A stale partial destination is discarded only when our journal owns it.
    fs::write(p.database.with_extension("gc.sqlite3"), "partial").unwrap();
    assert!(p.verification_gc(false, true, None).is_err());
    fs::write(
        p.database.with_extension("gc.json"),
        serde_json::to_vec(&"another project").unwrap(),
    )
    .unwrap();
    assert!(p.verification_gc(false, true, None).is_err());
    fs::write(
        p.database.with_extension("gc.json"),
        serde_json::to_vec(&p.key()).unwrap(),
    )
    .unwrap();
    let report = p.verification_gc(false, true, Some(&exports)).unwrap();
    assert!(report.compacted);
    assert_eq!(report.storage.full_runs_after, 1);
    assert!(report.exports.iter().any(|e| e.status == "reduced"));
    assert!(
        report
            .exports
            .iter()
            .any(|e| e.status.starts_with("skipped:"))
    );
    let reduced_bytes = fs::read(exports.join("report.json")).unwrap();
    let reduced_text = std::str::from_utf8(&reduced_bytes).unwrap();
    assert!(reduced_text.ends_with('\n'));
    assert!(reduced_text.lines().count() > 1);
    let reduced: serde_json::Value = serde_json::from_slice(&reduced_bytes).unwrap();
    assert!(reduced["evidence"].get("run").is_none());
    assert_eq!(reduced["evidence"]["summary"]["details_available"], false);
    assert_eq!(reduced["siblings"], body["siblings"]);
    assert!(
        p.saved_verification_run(&saved.id)
            .unwrap_err()
            .to_string()
            .contains("pruned")
    );
    assert!(!p.verification_summary(&saved.id).unwrap().details_available);
    let reports = [
        saved.id.clone(),
        serde_json::json!({"native_run":saved.id}).to_string(),
        "external-report.json".into(),
        "f".repeat(64),
    ];
    let checks = reports
        .into_iter()
        .map(|report| dctx0_tasks::CheckEvidence {
            requirements_hash: "requirements".into(),
            implementation: "implementation".into(),
            verifier: "fixture".into(),
            verifier_version: "1".into(),
            command: "import".into(),
            started_at: "0".into(),
            finished_at: "1".into(),
            report,
            status: dctx0_tasks::CheckStatus::Pass,
        })
        .collect::<Vec<_>>();
    let references = p.verification_evidence_summaries(&checks).unwrap();
    assert_eq!(references.len(), 1);
    assert!(!references[0].details_available);
    assert_eq!(p.verification_history(0, 200).unwrap().len(), 5);
    let site = p.site_snapshot(&Default::default()).unwrap();
    let wire = serde_json::to_value(site).unwrap();
    assert_eq!(wire["records"]["runs"].as_array().unwrap().len(), 1);
    assert_eq!(
        wire["records"]["run_summaries"].as_array().unwrap().len(),
        5
    );
    assert_eq!(
        p.verification_gc(false, false, None)
            .unwrap()
            .storage
            .pruned_runs,
        0
    );
    assert!(
        p.verification_gc(true, false, Some(&exports.join("report.json")))
            .is_err()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&exports, t.0.join("linked")).unwrap();
        assert!(
            p.verification_gc(true, false, Some(&t.0.join("linked")))
                .is_err()
        );
        std::os::unix::fs::symlink(exports.join("report.json"), exports.join("linked.json"))
            .unwrap();
        assert!(
            !p.verification_gc(true, false, Some(&exports))
                .unwrap()
                .exports
                .iter()
                .any(|e| e.path.ends_with("linked.json"))
        );
    }
}
#[test]
fn storage_engine_rejects_missing_database_and_preserves_source_on_failed_rebuild() {
    let t = Temp::new();
    let p = t.project();
    assert!(p.verification_gc(true, true, None).is_err());
    assert!(p.check().unwrap().ok);
    let saved = run(&p);
    let mut store = p.store(AccessMode::ReadWrite).unwrap();
    // An additional check is not new execution evidence and must survive maintenance.
    store.failure(&p.key(), "invalid", &[]).unwrap();
    drop(store);
    assert!(p.verification_gc(false, true, None).is_ok());
    assert_eq!(
        p.store(AccessMode::ReadOnly)
            .unwrap()
            .history(&p.key())
            .unwrap()
            .len(),
        2
    );
    assert_eq!(p.saved_verification_run(&saved.id).unwrap(), saved);
}
