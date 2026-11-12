use dctx0_core::*;
use dctx0_engine::*;
use dctx0_target_typescript::BunHarness;
use dctx0_tasks::{Action, CheckEvidence};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    app: PathBuf,
    project: Project,
    task: i64,
    attestation: TaskAttestation,
    native: NativeVerificationReport,
}
impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-agent-freshness-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let app = root.join("app");
        let spec = root.join("spec");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir(&spec).unwrap();
        fs::write(spec.join("w.dctx0"),"workspace W { constraint \"Validate at the actual exported boundary.\"\nvalue A = Int { range 1..2 } }").unwrap();
        let project = Project::new(&spec, None).unwrap();
        assert!(project.check().unwrap().ok);
        project
            .initialize_verification(
                &app,
                &BunHarness,
                ConfiguredVerificationScope {
                    subjects: ["W".into()].into(),
                    tasks: Default::default(),
                },
            )
            .unwrap();
        fs::write(app.join("app.ts"),"export function factory(n){if(!Number.isInteger(n)||n<1||n>2)throw Error('invalid');return n;}").unwrap();
        fs::write(app.join(".dctx0/testing/typescript/adapter.ts"),r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#).unwrap();
        create_profiles(&app, &dctx0_target_typescript::Provider, true, false, false).unwrap();
        let plan = project.task_plan("latest").unwrap();
        let m = plan
            .members
            .iter()
            .find(|m| m.candidate.subjects == ["W.A"])
            .unwrap();
        let task = m.task_id;
        let native = project
            .run_verification(
                Some(&app),
                Default::default(),
                &BunHarness,
                &NativeCancellation::default(),
                true,
            )
            .await
            .unwrap();
        assert!(
            native
                .evidence
                .run
                .cases
                .iter()
                .all(|c| c.result == VerificationResult::Pass)
        );
        let attestation=TaskAttestation{task,review:None,inspection:VerificationAttestation{schema_version:1,actor:"agent-author".into(),subject:format!("T{task}"),snapshot:native.evidence.run.provenance.snapshot.clone(),plan_hash:native.evidence.run.provenance.plan_hash.clone(),requirements:m.candidate.subjects.iter().chain(&m.candidate.types).chain(&m.candidate.facts).cloned().collect(),implementation_artifacts:native.evidence.run.provenance.implementation.clone(),report:"Reviewed app.ts: exported factory rejects noninteger and out-of-range inputs; the full actual native corpus passed.".into(),limitations:vec!["Agent inspection supplements finite native observations; not automatic native proof.".into()],result:VerificationResult::Pass}};
        project
            .publish_task_attestations(
                Some(&app),
                &native.evidence.id,
                std::slice::from_ref(&attestation),
            )
            .unwrap();
        Self {
            root,
            app,
            project,
            task,
            attestation,
            native,
        }
    }
    fn evidence(&self) -> CheckEvidence {
        self.project.task_session(None, None).unwrap().states[&self.task]
            .evidence
            .last()
            .unwrap()
            .clone()
    }
    fn record(&self, evidence: CheckEvidence) {
        self.project
            .task_action(Action::RecordCheck {
                task: self.task,
                evidence,
            })
            .unwrap();
    }
    fn counts(&self) -> StatusTaskCounts {
        self.project.status(Some(&self.app), None).unwrap().tasks
    }
    fn unknown(&self) {
        let c = self.counts();
        assert_eq!(c.done_pass, 1);
        assert_eq!(c.evidence_current, 0);
        assert_eq!(c.evidence_unknown, 1);
    }
    fn current(&self) {
        let before = fs::read(&self.project.database).unwrap();
        let c = self.counts();
        assert_eq!(c.evidence_current, 1);
        assert_eq!(c.agent_current, 1);
        assert_eq!(c.inspected_current, 0);
        assert_eq!(c.evidence_unknown, 0);
        assert_eq!(fs::read(&self.project.database).unwrap(), before);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[tokio::test]
async fn embedded_agent_freshness_checks_real_native_bindings_and_staleness() {
    let f = Fixture::new().await;
    let original = f.evidence();
    f.current();
    let original_report: serde_json::Value = serde_json::from_str(&original.report).unwrap();
    for (field, value) in [
        ("actor", serde_json::json!("another-agent")),
        ("subject", serde_json::json!("T999999")),
        ("snapshot", serde_json::json!("old")),
        ("plan_hash", serde_json::json!("other")),
        ("requirements", serde_json::json!(["wrong"])),
        (
            "implementation_artifacts",
            serde_json::json!({"app.ts":"wrong"}),
        ),
        (
            "result",
            serde_json::json!({"status":"UNKNOWN","issue":{"code":"NO_PROOF","message":"not inspected","span":null}}),
        ),
    ] {
        let mut report = original_report.clone();
        report["inspection"][field] = value;
        let mut evidence = original.clone();
        evidence.report = report.to_string();
        f.record(evidence);
        f.unknown();
    }
    let mut report = original_report.clone();
    report["native_run"] = serde_json::json!("missing-run");
    let mut evidence = original.clone();
    evidence.report = report.to_string();
    f.record(evidence);
    f.unknown();
    let mut evidence = original.clone();
    evidence.implementation = "wrong-provenance".into();
    assert!(
        f.project
            .task_action(Action::RecordCheck {
                task: f.task,
                evidence
            })
            .is_err()
    );
    f.record(original.clone());
    f.current();
    let source = f.app.join("app.ts");
    let saved = fs::read(&source).unwrap();
    fs::write(&source, "export function factory(n){return n;}").unwrap();
    assert_eq!(f.counts().agent_stale, 1);
    fs::write(&source, saved).unwrap();
    f.current();
    let profiles = f
        .project
        .profiles(Some(&f.app), Some("typescript"))
        .unwrap()
        .unwrap();
    let profile = profiles.project_root.join(&profiles.documents[0].path);
    let saved = fs::read(&profile).unwrap();
    let mut changed = saved.clone();
    changed.extend_from_slice(b"\nAdditional reviewed requirement.\n");
    fs::write(&profile, changed).unwrap();
    assert_eq!(f.counts().agent_stale, 1);
    fs::write(&profile, saved).unwrap();
    f.current();
    let spec = f.root.join("spec/w.dctx0");
    let saved = fs::read(&spec).unwrap();
    let mut changed = saved.clone();
    changed.push(b'\n');
    fs::write(&spec, changed).unwrap();
    assert_eq!(f.counts().agent_stale, 1);
    fs::write(&spec, saved).unwrap();
    f.current();
    let db = rusqlite::Connection::open(&f.project.database).unwrap();
    let summary: String = db
        .query_row(
            "SELECT summary FROM verification_runs WHERE id=?1",
            [&f.native.evidence.id],
            |r| r.get(0),
        )
        .unwrap();
    for field in ["outcomes", "case_inventory_complete", "origin"] {
        let mut bad: serde_json::Value = serde_json::from_str(&summary).unwrap();
        bad[field] = match field {
            "outcomes" => serde_json::json!([1, 1, 0]),
            "origin" => serde_json::json!("imported"),
            _ => serde_json::json!(false),
        };
        db.execute(
            "UPDATE verification_runs SET summary=?1 WHERE id=?2",
            rusqlite::params![bad.to_string(), f.native.evidence.id],
        )
        .unwrap();
        f.unknown();
        db.execute(
            "UPDATE verification_runs SET summary=?1 WHERE id=?2",
            rusqlite::params![summary, f.native.evidence.id],
        )
        .unwrap();
    }
    f.current();
    db.execute(
        "UPDATE verification_runs SET pruned_at='test-pruned' WHERE id=?1",
        [&f.native.evidence.id],
    )
    .unwrap();
    f.unknown();
    db.execute(
        "UPDATE verification_runs SET pruned_at=NULL WHERE id=?1",
        [&f.native.evidence.id],
    )
    .unwrap();
    let header: String = db
        .query_row(
            "SELECT data FROM verification_runs WHERE id=?1",
            [&f.native.evidence.id],
            |r| r.get(0),
        )
        .unwrap();
    let mut bad: serde_json::Value = serde_json::from_str(&header).unwrap();
    bad["open_obligations"].as_array_mut().unwrap().push(serde_json::json!({"code":"HYBRID_COHORT_REQUIRED","message":"missing required group","span":null}));
    db.execute(
        "UPDATE verification_runs SET data=?1 WHERE id=?2",
        rusqlite::params![bad.to_string(), f.native.evidence.id],
    )
    .unwrap();
    f.unknown();
    db.execute(
        "UPDATE verification_runs SET data=?1 WHERE id=?2",
        rusqlite::params![header, f.native.evidence.id],
    )
    .unwrap();
    f.current();
    fs::write(
        f.root.join("spec/w.dctx0"),
        "workspace W { value A = Int { range 1..3 } }",
    )
    .unwrap();
    assert!(f.project.check().unwrap().ok);
    assert_eq!(f.counts().agent_stale, 1);
}

#[tokio::test]
async fn archived_agent_freshness_requires_digest_exact_scope_and_safe_paths() {
    let f = Fixture::new().await;
    let original = f.evidence();
    let run = &f.native.evidence.run;
    let pin = run.provenance.task_pin.as_ref().unwrap();
    fs::create_dir_all(f.app.join(".dctx0ctl")).unwrap();
    let archive = f.app.join(".dctx0ctl/agent-inspections.json");
    let base = serde_json::json!({"schema_version":1,"basis":"agent inspection with complete actual native corpus","native_run":f.native.evidence.id,"task":f.task,"snapshot":run.provenance.snapshot,"check_occurrence":run.provenance.check_occurrence,"requirements_hash":original.requirements_hash,"profile_bundle_hash":pin.profile_bundles[&f.task],"source_inventory_artifact":run.artifacts.iter().find(|a|a.media_type=="application/vnd.dctx0.captured-input-inventory+json").unwrap().hash,"full_task_inspection_file":".dctx0ctl/agent-inspections.json","inspection_selector":{"task":f.task}});
    let record = |items: &[TaskAttestation], mut report: serde_json::Value| {
        let bytes = serde_json::to_vec_pretty(items).unwrap();
        fs::write(&archive, &bytes).unwrap();
        report["full_task_inspection_file_sha256"] =
            serde_json::json!(format!("{:x}", Sha256::digest(&bytes)));
        let mut e = original.clone();
        e.report = report.to_string();
        f.record(e);
    };
    record(std::slice::from_ref(&f.attestation), base.clone());
    f.current();
    let evidence = f.evidence();
    fs::write(&archive, b"[]").unwrap();
    f.unknown();
    f.record(evidence);
    for field in [
        "task",
        "snapshot",
        "check_occurrence",
        "requirements_hash",
        "profile_bundle_hash",
        "source_inventory_artifact",
        "inspection_selector",
        "full_task_inspection_file",
    ] {
        let mut bad = base.clone();
        bad[field] = match field {
            "task" | "check_occurrence" => serde_json::json!(999999),
            "inspection_selector" => serde_json::json!({"task":999999}),
            "full_task_inspection_file" => serde_json::json!("../escape.json"),
            _ => serde_json::json!("wrong"),
        };
        record(std::slice::from_ref(&f.attestation), bad);
        f.unknown();
    }
    let mut bad = f.attestation.clone();
    bad.inspection.actor = "another-agent".into();
    record(&[bad], base.clone());
    f.unknown();
    let mut bad = f.attestation.clone();
    bad.inspection
        .implementation_artifacts
        .insert("app.ts".into(), "wrong".into());
    record(&[bad], base.clone());
    f.unknown();
    record(
        &[f.attestation.clone(), f.attestation.clone()],
        base.clone(),
    );
    f.unknown();
    record(std::slice::from_ref(&f.attestation), base.clone());
    f.current();
    for bytes in [b"{}".as_slice(), b"[", b"[] {}"] {
        fs::write(&archive, bytes).unwrap();
        let mut report = base.clone();
        report["full_task_inspection_file_sha256"] =
            serde_json::json!(format!("{:x}", Sha256::digest(bytes)));
        let mut evidence = original.clone();
        evidence.report = report.to_string();
        f.record(evidence);
        f.unknown();
    }
    let mut invalid_digest = base.clone();
    invalid_digest["full_task_inspection_file_sha256"] = serde_json::json!("not-a-sha256");
    let mut evidence = original.clone();
    evidence.report = invalid_digest.to_string();
    f.record(evidence);
    f.unknown();
    record(std::slice::from_ref(&f.attestation), base.clone());
    let saved = fs::read(&archive).unwrap();
    let file = fs::OpenOptions::new().write(true).open(&archive).unwrap();
    file.set_len((1 << 30) + 1).unwrap();
    f.unknown();
    drop(file);
    fs::write(&archive, &saved).unwrap();
    f.current();
    fs::remove_file(&archive).unwrap();
    f.unknown();
    fs::write(&archive, &saved).unwrap();
    #[cfg(unix)]
    {
        let target = f.root.join("outside-archive.json");
        fs::write(&target, &saved).unwrap();
        fs::remove_file(&archive).unwrap();
        std::os::unix::fs::symlink(&target, &archive).unwrap();
        f.unknown();
        fs::remove_file(&archive).unwrap();
        fs::write(&archive, &saved).unwrap();
    }
    f.current();
}

#[tokio::test]
async fn agent_revalidation_preserves_delivered_identity_and_checks_new_native_binding() {
    let f = Fixture::new().await;
    let original = f.evidence();
    let delivered = f.project.task_session(None, None).unwrap().states[&f.task]
        .result
        .clone();
    let native = f
        .project
        .run_verification(
            Some(&f.app),
            Default::default(),
            &BunHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    let mut inspection = f.attestation.inspection.clone();
    inspection.snapshot = native.evidence.run.provenance.snapshot.clone();
    inspection.plan_hash = native.evidence.run.provenance.plan_hash.clone();
    inspection.implementation_artifacts = native.evidence.run.provenance.implementation.clone();
    let report = serde_json::json!({
        "schema_version":1, "basis":"agent-revalidation", "native_run":native.evidence.id,
        "delivered_implementation":original.implementation,
        "tested_implementation":dctx0_changes::hash(&native.evidence.run.provenance),
        "inspection":inspection
    });
    let mut renewed = original.clone();
    renewed.verifier_version = "dctx0-agent-revalidation-1".into();
    renewed.report = report.to_string();
    f.record(renewed.clone());
    f.current();
    assert_eq!(
        f.project.task_session(None, None).unwrap().states[&f.task].result,
        delivered
    );
    for (field, value) in [
        ("delivered_implementation", serde_json::json!("wrong")),
        ("tested_implementation", serde_json::json!("wrong")),
        ("native_run", serde_json::json!("missing")),
        ("basis", serde_json::json!("agent")),
    ] {
        let mut invalid = report.clone();
        invalid[field] = value;
        let mut evidence = renewed.clone();
        evidence.report = invalid.to_string();
        f.record(evidence);
        f.unknown();
    }
    let mut invalid = renewed.clone();
    invalid.verifier_version = "dctx0-attestation-1".into();
    f.record(invalid);
    f.unknown();
    f.record(renewed);
    f.current();
}
