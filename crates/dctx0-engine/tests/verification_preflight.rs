use dctx0_core::VerificationResult;
use dctx0_engine::*;
use dctx0_target_typescript::BunHarness;
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
}
impl Fixture {
    fn new(spec: &str, subject: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-preflight-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir(root.join("spec")).unwrap();
        fs::write(root.join("spec/w.dctx0"), spec).unwrap();
        let project = Project::new(&root.join("spec"), None).unwrap();
        assert!(project.check().unwrap().ok);
        project
            .initialize_verification(
                &app,
                &BunHarness,
                ConfiguredVerificationScope {
                    subjects: [subject.into()].into(),
                    tasks: Default::default(),
                },
            )
            .unwrap();
        Self { root, app, project }
    }
    fn prepare(&self) {
        create_profiles(
            &self.app,
            &dctx0_target_typescript::Provider,
            true,
            false,
            false,
        )
        .unwrap();
        self.project.task_plan("latest").unwrap();
    }
    fn preflight(&self, intent: VerificationIntent) -> VerificationPreflight {
        self.project
            .verification_preflight(Some(&self.app), Default::default(), intent, None)
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

const SPEC: &str = "workspace W { value A = Int { range 1..2 } }";
const ADAPTER: &str = r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#;

#[tokio::test]
async fn delivery_preflight_pins_actual_native_results_and_status_never_expands_case_bodies() {
    let f = Fixture::new(SPEC, "W.A");
    fs::write(f.app.join("app.ts"),"export function factory(n:unknown){if(typeof n!=='number'||!Number.isInteger(n)||n<1||n>2)throw Error('invalid');return n;}").unwrap();
    fs::write(f.app.join(".dctx0/testing/typescript/adapter.ts"), ADAPTER).unwrap();
    let bytes = fs::read(&f.project.database).unwrap();
    let unpinned = f.preflight(VerificationIntent::Delivery);
    assert!(!unpinned.permits_execution());
    assert!(
        unpinned
            .blockers
            .iter()
            .any(|i| i.code == "ADOPTION_TASK_PIN_REQUIRED")
    );
    assert!(
        f.preflight(VerificationIntent::Diagnostic)
            .permits_execution()
    );
    assert_eq!(bytes, fs::read(&f.project.database).unwrap());
    assert!(
        f.project
            .run_verification_preflighted(
                Some(&f.app),
                Default::default(),
                &BunHarness,
                &NativeCancellation::default(),
                unpinned
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("PREFLIGHT_BLOCKED")
    );
    f.prepare();
    let before = f.project.task_metadata_session(None, None).unwrap();
    let good = f.preflight(VerificationIntent::Delivery);
    assert!(good.permits_execution(), "{:?}", good.blockers);
    assert_eq!(good.potential_tasks.len(), 1);
    assert!(!good.bindings_checked);
    assert!(
        good.warnings
            .iter()
            .any(|i| i.code == "PREFLIGHT_FIXTURE_UNKNOWN")
    );
    let report = f
        .project
        .run_verification_preflighted(
            Some(&f.app),
            Default::default(),
            &BunHarness,
            &NativeCancellation::default(),
            good,
        )
        .await
        .unwrap();
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    assert_eq!(report.adopted_tasks.len(), 1);
    let baseline = Some((before.plan.id, before.event_seq));
    let status = f.project.status(Some(&f.app), baseline).unwrap();
    assert_eq!(status.tasks.done_pass, 1);
    assert_eq!(status.tasks.evidence_current, 1);
    assert_eq!(status.tasks.evidence_unknown, 0);
    assert_eq!(status.progress.unwrap().newly_pass, 1);
    assert_eq!(
        status.latest_run.as_ref().unwrap().sources_current,
        Some(true)
    );
    assert_eq!(
        status.latest_run.as_ref().unwrap().task_event_pin_current,
        Some(false)
    );
    assert_eq!(status.latest_run.unwrap().requirements_current, Some(true));
    let pointer = f.project.root.join(".dctx0ctl/verification.json");
    let pointer_bytes = fs::read(&pointer).unwrap();
    fs::remove_file(&pointer).unwrap();
    assert_eq!(
        f.project
            .status(None, baseline)
            .unwrap()
            .tasks
            .evidence_unknown,
        1
    );
    fs::write(pointer, pointer_bytes).unwrap();
    // Deliberately corrupt the compressed case payload. Status must neither
    // decode it nor invent an execution result from the corruption.
    let connection = rusqlite::Connection::open(&f.project.database).unwrap();
    connection
        .execute("UPDATE verification_case_bodies SET bytes=X'00'", [])
        .unwrap();
    assert_eq!(
        f.project
            .status(Some(&f.app), baseline)
            .unwrap()
            .tasks
            .evidence_current,
        1
    );
    assert!(
        f.project
            .saved_verification_run(&report.evidence.id)
            .is_err()
    );
    fs::write(
        f.app.join("app.ts"),
        "export function factory(n:unknown){return n;}",
    )
    .unwrap();
    let stale = f.project.status(Some(&f.app), baseline).unwrap();
    assert_eq!(stale.tasks.done_pass, 1);
    assert_eq!(stale.tasks.evidence_stale, 1);
    assert_eq!(stale.latest_run.unwrap().sources_current, Some(false));
    assert!(
        stale
            .next_causes
            .iter()
            .any(|c| c.code == "IMPLEMENTATION_CHANGED")
    );
    let header: String = connection
        .query_row(
            "SELECT data FROM verification_runs WHERE id=?1",
            [&report.evidence.id],
            |r| r.get(0),
        )
        .unwrap();
    connection
        .execute(
            "UPDATE verification_runs SET pruned_at='test-retention' WHERE id=?1",
            [&report.evidence.id],
        )
        .unwrap();
    let pruned = f.project.status(Some(&f.app), baseline).unwrap();
    assert_eq!(pruned.tasks.done_pass, 1);
    assert_eq!(pruned.tasks.evidence_unknown, 1);
    assert_eq!(pruned.latest_run.as_ref().unwrap().sources_current, None);
    assert_eq!(pruned.latest_run.unwrap().task_pin_present, Some(true));
    assert!(
        pruned
            .next_causes
            .iter()
            .any(|c| c.code == "EVIDENCE_DETAILS_PRUNED")
    );
    connection
        .execute(
            "UPDATE verification_runs SET data='' WHERE id=?1",
            [&report.evidence.id],
        )
        .unwrap();
    assert_eq!(
        f.project
            .status(Some(&f.app), baseline)
            .unwrap()
            .latest_run
            .unwrap()
            .task_pin_present,
        None
    );
    connection.execute("UPDATE verification_runs SET pruned_at=NULL,data=json_set(?2,'$.provenance.adapter_hash','corrupt') WHERE id=?1",rusqlite::params![&report.evidence.id,header]).unwrap();
    assert!(
        f.project
            .status(Some(&f.app), baseline)
            .unwrap_err()
            .to_string()
            .contains("provenance digest")
    );
}

#[test]
fn authored_pass_and_done_without_pass_remain_distinct_from_current_native_evidence() {
    use dctx0_tasks::{Action, CheckEvidence, CheckStatus, TaskResult};
    let f = Fixture::new(SPEC, "W.A");
    f.prepare();
    let before = f.project.task_metadata_session(None, None).unwrap();
    let member = &before.plan.members[0];
    let id = member.task_id;
    f.project
        .task_action(Action::Start {
            task: id,
            actor: "author".into(),
            reservation: None,
        })
        .unwrap();
    assert_eq!(
        f.project
            .status(Some(&f.app), None)
            .unwrap()
            .tasks
            .in_progress,
        1
    );
    let started = f.project.task_metadata_session(None, None).unwrap();
    f.project
        .task_action(Action::Complete {
            task: id,
            actor: "author".into(),
            claim: started.states[&id].claim.unwrap(),
            version: started.states[&id].version,
            result: TaskResult {
                summary: "authored work".into(),
                artifacts: vec!["app.ts".into()],
                decisions: vec![],
                implementation: Some("authored-revision".into()),
            },
        })
        .unwrap();
    assert_eq!(
        f.project
            .status(Some(&f.app), None)
            .unwrap()
            .tasks
            .done_without_pass,
        1
    );
    f.project
        .task_action(Action::RecordCheck {
            task: id,
            evidence: CheckEvidence {
                requirements_hash: member.candidate.requirements_hash.clone(),
                implementation: "authored-revision".into(),
                verifier: "external-checker".into(),
                verifier_version: "1".into(),
                command: "independent check".into(),
                started_at: "1".into(),
                finished_at: "2".into(),
                report: "authored-report".into(),
                status: CheckStatus::Pass,
            },
        })
        .unwrap();
    let status = f
        .project
        .status(Some(&f.app), Some((before.plan.id, before.event_seq)))
        .unwrap();
    assert_eq!(status.tasks.done_pass, 1);
    assert_eq!(status.tasks.evidence_current, 0);
    assert_eq!(status.tasks.evidence_unknown, 1);
    assert_eq!(status.progress.unwrap().newly_done, 1);
}

#[tokio::test]
async fn compact_status_keeps_adapter_abort_missing_cases_and_task_progress_separate() {
    let f = Fixture::new(SPEC, "W.A");
    f.prepare();
    fs::write(f.app.join("app.ts"),"export function factory(n:unknown){if(typeof n!=='number'||!Number.isInteger(n)||n<1||n>2)throw Error('invalid');return n;}").unwrap();
    let adapter=format!("import {{AdapterError}} from './contract.ts';\n{}",ADAPTER.replace("async validate(subject,value){let result;","async validate(subject,value){throw new AdapterError('intentional adapter fault');let result;"));
    fs::write(f.app.join(".dctx0/testing/typescript/adapter.ts"), adapter).unwrap();
    let preflight = f.preflight(VerificationIntent::Delivery);
    let run = f
        .project
        .run_verification_preflighted(
            Some(&f.app),
            Default::default(),
            &BunHarness,
            &NativeCancellation::default(),
            preflight,
        )
        .await
        .unwrap();
    assert!(matches!(
        run.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    let status = f.project.status(Some(&f.app), None).unwrap();
    let summary = &status.latest_run.unwrap().summary;
    assert!(summary.missing_case_count > 0);
    assert_eq!(summary.outcomes[2], 1);
    assert_eq!(status.tasks.done_pass, 0);
    assert_eq!(status.tasks.open, 1);
    assert!(
        status
            .next_causes
            .iter()
            .any(|c| c.code == "NATIVE_RUN_ABORTED")
    );
}

#[tokio::test]
async fn latest_relevant_run_uses_configured_subjects_instead_of_newest_unrelated_execution() {
    let f = Fixture::new(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 1..2 } }",
        "W.A",
    );
    f.prepare();
    fs::write(f.app.join("app.ts"),"export function factory(n:unknown){if(typeof n!=='number'||!Number.isInteger(n)||n<1||n>2)throw Error('invalid');return n;}").unwrap();
    fs::write(f.app.join(".dctx0/testing/typescript/adapter.ts"), ADAPTER).unwrap();
    let run = f
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
    let other = f
        .project
        .run_verification(
            Some(&f.app),
            VerificationSelection {
                subjects: ["W.B".into()].into(),
                ..Default::default()
            },
            &BunHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    assert_ne!(run.evidence.id, other.evidence.id);
    let status = f.project.status(Some(&f.app), None).unwrap();
    assert_eq!(status.latest_run.unwrap().summary.id, run.evidence.id);
    assert!(status.progress.is_none());
    f.project
        .run_verification(
            Some(&f.app),
            Default::default(),
            &BunHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    let status = f.project.status(Some(&f.app), None).unwrap();
    assert_eq!(status.progress.unwrap().newly_done, 0);
    assert_eq!(status.tasks.done_pass, 0);
}

#[tokio::test]
async fn source_or_task_changes_between_preflight_and_execution_start_no_native_worker() {
    let f = Fixture::new(SPEC, "W.A");
    f.prepare();
    assert!(
        f.preflight(VerificationIntent::Delivery)
            .blockers
            .iter()
            .any(|i| i.code == "PREFLIGHT_BINDINGS_UNAVAILABLE")
    );
    fs::write(f.app.join("app.ts"),"export function factory(n:unknown){if(typeof n!=='number'||!Number.isInteger(n)||n<1||n>2)throw Error('invalid');return n;}").unwrap();
    fs::write(f.app.join(".dctx0/testing/typescript/adapter.ts"), ADAPTER).unwrap();
    let report = f.preflight(VerificationIntent::Delivery);
    assert!(report.permits_execution());
    fs::write(f.app.join("new-production.ts"), "export const changed=true").unwrap();
    let error = f
        .project
        .run_verification_preflighted(
            Some(&f.app),
            Default::default(),
            &BunHarness,
            &NativeCancellation::default(),
            report,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("PREFLIGHT_INPUTS_CHANGED"));
    assert!(f.project.verification_history(0, 20).unwrap().is_empty());
    let report = f.preflight(VerificationIntent::Delivery);
    f.project.task_plan("latest").unwrap();
    // An idempotent plan does not create a false mismatch.
    assert_eq!(
        report.pin.task_pin,
        f.preflight(VerificationIntent::Delivery).pin.task_pin
    );
    let id = report
        .pin
        .task_pin
        .as_ref()
        .unwrap()
        .requirements
        .keys()
        .next()
        .copied()
        .unwrap();
    f.project
        .task_action(dctx0_tasks::Action::Start {
            task: id,
            actor: "other".into(),
            reservation: None,
        })
        .unwrap();
    let error = f
        .project
        .run_verification_preflighted(
            Some(&f.app),
            Default::default(),
            &BunHarness,
            &NativeCancellation::default(),
            report,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("PREFLIGHT_TASK_PIN_CHANGED"));
    assert!(f.project.verification_history(0, 20).unwrap().is_empty());
}

#[test]
fn historical_scope_profile_gaps_and_wrong_binding_target_cannot_offer_delivery() {
    let f = Fixture::new(SPEC, "W.A");
    f.project.task_plan("latest").unwrap();
    let missing = f.preflight(VerificationIntent::Delivery);
    assert!(
        missing
            .blockers
            .iter()
            .any(|i| i.code == "PREFLIGHT_PROFILE_REQUIRED")
    );
    let historical = f
        .project
        .verification_preflight(
            Some(&f.app),
            VerificationSelection {
                historical_check: Some(missing.pin.check_occurrence as i64),
                ..Default::default()
            },
            VerificationIntent::Diagnostic,
            None,
        )
        .unwrap();
    assert!(historical.permits_execution());
    assert!(
        historical
            .blockers
            .iter()
            .any(|i| i.code == "PREFLIGHT_HISTORICAL_SCOPE")
    );
    let entry = serde_json::json!({"source_root":null,"source":"app.ts","symbol":"actual"});
    fs::write(f.app.join("bindings.json"),serde_json::json!({"schema_version":1,"target":"go","session":{"factory":entry,"cleanup":entry,"native_type":"Session"},"operations":[],"validators":[],"entities":[],"effects":{"events":null,"calls":null,"resources":null},"types":[]}).to_string()).unwrap();
    let wrong = f
        .project
        .verification_preflight(
            Some(&f.app),
            Default::default(),
            VerificationIntent::Delivery,
            Some("bindings.json"),
        )
        .unwrap();
    assert!(
        wrong
            .blockers
            .iter()
            .any(|i| i.code == "PREFLIGHT_BINDING_TARGET")
    );
}

#[test]
fn planning_without_before_state_does_not_claim_the_runtime_observer_is_absent() {
    let f = Fixture::new(
        "workspace W { value Id = Int { range 0..0 }\nentity Item { id:Id\nkey id }\nsurface Api { operation load() -> Int { requires exists Item(0)\nensures result == 0 }\neffects { read Item } } }",
        "W.Api.load",
    );
    f.prepare();
    let report = f.preflight(VerificationIntent::Delivery);
    assert!(
        report
            .warnings
            .iter()
            .any(|i| i.code == "STATE_OBSERVER_REQUIRED")
    );
    assert!(
        !report
            .blockers
            .iter()
            .any(|i| i.code == "STATE_OBSERVER_REQUIRED")
    );
    assert!(report.required_capabilities.contains("state"));
    assert!(report.required_cases > 0);
    // An authored declaration that actually lacks the observer remains blocked.
    let entry = serde_json::json!({"source_root":null,"source":"app.ts","symbol":"actual"});
    fs::write(f.app.join("bindings.json"),serde_json::json!({"schema_version":1,"target":"typescript","session":{"factory":entry,"cleanup":entry,"native_type":"Session"},"operations":[],"validators":[],"entities":[],"effects":{"events":null,"calls":null,"resources":null},"types":[]}).to_string()).unwrap();
    let missing = f
        .project
        .verification_preflight(
            Some(&f.app),
            Default::default(),
            VerificationIntent::Delivery,
            Some("bindings.json"),
        )
        .unwrap();
    assert!(
        missing
            .blockers
            .iter()
            .any(|i| i.code == "MISSING_BINDING" && i.message.contains("EntityObserver"))
    );
    assert_eq!(report.required_cases, missing.required_cases);
}

#[test]
fn manual_review_stays_a_warning_but_missing_symbolic_inputs_and_event_observers_block_delivery() {
    let f = Fixture::new(
        "workspace W { intent \"Context for delivery.\"\nconstraint \"Keep validation at the application boundary.\"\nvalue A = Int { range 1..2 }\nevent Changed { number:Int }\nsurface Api { operation next() -> Int { emit Changed(number: 5) } operation echo(input:Int) -> Int { pure } }\nexample \"symbolic\" { when result = Api.echo(symbolic)\nexpect result == 5 }\ntest Examples\npolicy Release { require Examples == PASS } }",
        "W",
    );
    f.prepare();
    let entry = serde_json::json!({"source_root":null,"source":"app.ts","symbol":"actual"});
    fs::write(f.app.join("bindings.json"),serde_json::json!({"schema_version":1,"target":"typescript","session":{"factory":entry,"cleanup":entry,"native_type":"Session"},"operations":[],"validators":[],"entities":[],"effects":{"events":null,"calls":null,"resources":null},"types":[]}).to_string()).unwrap();
    let report = f
        .project
        .verification_preflight(
            Some(&f.app),
            Default::default(),
            VerificationIntent::Delivery,
            Some("bindings.json"),
        )
        .unwrap();
    assert!(
        report
            .warnings
            .iter()
            .any(|i| i.code == "MANUAL_REQUIREMENT")
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|i| i.code == "SUITE_INSPECTION_REQUIRED")
    );
    assert!(
        report
            .blockers
            .iter()
            .any(|i| i.code == "EXAMPLE_BINDINGS_REQUIRED")
    );
    assert!(
        report
            .blockers
            .iter()
            .any(|i| i.code == "PREFLIGHT_OBSERVER_REQUIRED")
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|i| i.code == "PREFLIGHT_TASK_REQUIREMENTS_UNMAPPED")
    );
    assert!(report.potential_tasks.is_empty());
}

#[test]
fn lifecycle_inputs_and_explicit_missing_bindings_are_reported_without_runtime_execution() {
    let f = Fixture::new(
        "workspace W { type Phase = Open | Closed\nentity Item { id:String\nphase:Phase\nkey id } surface Api { operation close(id:String) -> Unit } lifecycle Flow for Item using phase { state Open initial\nstate Closed terminal\ntransition Api.close from Open to Closed when true } }",
        "W.Flow",
    );
    f.prepare();
    let before = fs::read(&f.project.database).unwrap();
    let report = f.preflight(VerificationIntent::Delivery);
    assert!(!report.permits_execution());
    assert!(
        report
            .blockers
            .iter()
            .any(|i| i.code == "LIFECYCLE_INPUT_REQUIRED")
    );
    let entry = serde_json::json!({"source_root":null,"source":"app.ts","symbol":"actual"});
    fs::write(f.app.join("bindings.json"),serde_json::json!({"schema_version":1,"target":"typescript","session":{"factory":entry,"cleanup":entry,"native_type":"Session"},"operations":[],"validators":[],"entities":[],"effects":{"events":null,"calls":null,"resources":null},"types":[]}).to_string()).unwrap();
    let bindings = f
        .project
        .verification_preflight(
            Some(&f.app),
            Default::default(),
            VerificationIntent::Delivery,
            Some("bindings.json"),
        )
        .unwrap();
    assert!(bindings.bindings_checked);
    assert!(
        bindings
            .blockers
            .iter()
            .any(|i| i.code == "MISSING_BINDING")
    );
    assert!(
        f.project
            .verification_preflight(
                Some(&f.app),
                Default::default(),
                VerificationIntent::Diagnostic,
                Some("../escape.json")
            )
            .is_err()
    );
    assert_eq!(before, fs::read(&f.project.database).unwrap());
    assert!(f.project.verification_history(0, 20).unwrap().is_empty());
}
