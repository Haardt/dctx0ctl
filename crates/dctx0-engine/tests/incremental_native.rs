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
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-incremental-native-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let app = root.join("app");
        let spec = root.join("spec");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir(&spec).unwrap();
        fs::write(
            spec.join("w.dctx0"),
            "workspace W { value A = Int { range 1..2 } }",
        )
        .unwrap();
        let project = Project::new(&spec, None).unwrap();
        assert!(project.check().unwrap().ok);
        project
            .initialize_verification(
                &app,
                &BunHarness,
                ConfiguredVerificationScope {
                    subjects: ["W.A".into()].into(),
                    tasks: Default::default(),
                },
            )
            .unwrap();
        fs::create_dir_all(app.join(".dctx0ctl")).unwrap();
        fs::write(app.join(".dctx0ctl/revision"), "initial").unwrap();
        fs::write(app.join(".dctx0/reuse.json"),r#"{"schema_version":1,"deterministic_units":["contract:W.A"],"files":[".dctx0ctl/revision"]}"#).unwrap();
        let counter =
            serde_json::to_string(&app.join(".dctx0ctl/calls").to_string_lossy()).unwrap();
        fs::write(app.join("app.ts"),format!(r#"import {{appendFileSync}} from 'node:fs'; export function factory(value:unknown){{appendFileSync({counter},'called\n');if(typeof value!=='number'||!Number.isSafeInteger(value)||value<1||value>2)throw Error('contract');return value;}}"#)).unwrap();
        fs::write(app.join(".dctx0/testing/typescript/adapter.ts"),r#"import {factory} from '../../../app.ts';export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#).unwrap();
        Self { root, app, project }
    }
    async fn run(&self, parent: Option<String>) -> NativeVerificationReport {
        self.project
            .run_verification_incremental(
                Some(&self.app),
                VerificationSelection::default(),
                &BunHarness,
                &NativeCancellation::default(),
                &NativeReuseOptions {
                    mode: Default::default(),
                    inputs: ".dctx0/reuse.json".into(),
                    parent,
                },
            )
            .await
            .unwrap()
    }
    fn calls(&self) -> usize {
        fs::read_to_string(self.app.join(".dctx0ctl/calls"))
            .unwrap()
            .lines()
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn plan_tasks(&self) -> i64 {
        create_profiles(
            &self.app,
            &dctx0_target_typescript::Provider,
            true,
            false,
            false,
        )
        .unwrap();
        self.project.task_plan("latest").unwrap();
        self.project
            .task_session(None, None)
            .unwrap()
            .next(&Default::default(), None)
            .unwrap()
            .task_id
    }

    async fn adopt(&self, parent: String) -> NativeVerificationReport {
        self.project
            .run_verification_incremental_with_adoption(
                Some(&self.app),
                VerificationSelection::default(),
                &BunHarness,
                &NativeCancellation::default(),
                &NativeReuseOptions {
                    mode: Default::default(),
                    inputs: ".dctx0/reuse.json".into(),
                    parent: Some(parent),
                },
                false,
            )
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn incremental_native_adopts_current_open_task_without_repeating_production_cases() {
    let f = Fixture::new();
    let id = f.plan_tasks();
    let baseline = f.run(None).await;
    assert!(baseline.adopted_tasks.is_empty());
    let calls = f.calls();
    let report = f.adopt(baseline.evidence.id).await;
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    assert_eq!(report.incremental.as_ref().unwrap().executed_cases, 0);
    assert_eq!(f.calls(), calls);
    assert!(report.adopted_tasks.contains(&id), "{report:#?}");
    let session = f.project.task_session(None, None).unwrap();
    assert_eq!(session.states[&id].work, dctx0_tasks::WorkState::Done);
    assert_eq!(session.states[&id].check, dctx0_tasks::CheckStatus::Pass);
}

#[tokio::test]
async fn incremental_native_preserves_claim_and_adopts_after_explicit_release() {
    let f = Fixture::new();
    let id = f.plan_tasks();
    let baseline = f.run(None).await;
    let claimed = f
        .project
        .task_action(dctx0_tasks::Action::Start {
            task: id,
            actor: "other-worker".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    let calls = f.calls();
    let blocked = f.adopt(baseline.evidence.id).await;
    assert_eq!(blocked.evidence.run.result, VerificationResult::Pass);
    assert!(blocked.adopted_tasks.is_empty());
    let session = f.project.task_session(None, None).unwrap();
    assert_eq!(session.states[&id], claimed);
    f.project
        .task_action(dctx0_tasks::Action::Release {
            task: id,
            actor: "other-worker".into(),
            claim: claimed.claim.unwrap(),
            version: claimed.version,
            reason: "Ready for guarded automatic delivery".into(),
        })
        .unwrap();
    let adopted = f.adopt(blocked.evidence.id).await;
    assert_eq!(
        f.calls(),
        calls,
        "task events must not invalidate unchanged production cases"
    );
    assert!(adopted.adopted_tasks.contains(&id), "{adopted:#?}");
    assert_eq!(
        f.project.task_session(None, None).unwrap().states[&id].check,
        dctx0_tasks::CheckStatus::Pass
    );
}

#[tokio::test]
async fn incremental_native_adds_pass_to_done_task_without_replacing_authored_result() {
    let f = Fixture::new();
    let id = f.plan_tasks();
    let started = f
        .project
        .task_action(dctx0_tasks::Action::Start {
            task: id,
            actor: "author".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    let authored = dctx0_tasks::TaskResult {
        summary: "Implemented by the project author".into(),
        artifacts: vec!["app.ts".into()],
        decisions: vec![],
        implementation: None,
    };
    f.project
        .task_action(dctx0_tasks::Action::Complete {
            task: id,
            actor: "author".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            result: authored.clone(),
        })
        .unwrap();
    let baseline = f.run(None).await;
    let calls = f.calls();
    let adopted = f.adopt(baseline.evidence.id).await;
    assert_eq!(f.calls(), calls);
    assert!(adopted.adopted_tasks.contains(&id), "{adopted:#?}");
    let state = &f.project.task_session(None, None).unwrap().states[&id];
    assert_eq!(state.work, dctx0_tasks::WorkState::Done);
    assert_eq!(state.check, dctx0_tasks::CheckStatus::Pass);
    assert_eq!(state.result.as_ref(), Some(&authored));
}

#[tokio::test]
async fn incremental_native_changed_failing_production_cannot_adopt_historical_pass() {
    let f = Fixture::new();
    let id = f.plan_tasks();
    let baseline = f.run(None).await;
    fs::write(
        f.app.join("app.ts"),
        "export function factory(value:unknown){return 9;}",
    )
    .unwrap();
    let report = f.adopt(baseline.evidence.id).await;
    assert!(matches!(
        report.evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    assert_eq!(report.incremental.as_ref().unwrap().reused_cases, 0);
    assert!(report.adopted_tasks.is_empty());
    let session = f.project.task_session(None, None).unwrap();
    assert_eq!(session.states[&id].work, dctx0_tasks::WorkState::Open);
    assert_eq!(
        session.states[&id].check,
        dctx0_tasks::CheckStatus::Unverified
    );
}

#[tokio::test]
async fn incremental_native_reconciled_current_requirements_execute_and_adopt_new_task_only() {
    let f = Fixture::new();
    let old_id = f.plan_tasks();
    let baseline = f.run(None).await;
    fs::write(
        f.root.join("spec/w.dctx0"),
        "workspace W { value A = Int { range 1..3 } }",
    )
    .unwrap();
    fs::write(f.app.join("app.ts"), "export function factory(value:unknown){if(typeof value!=='number'||!Number.isSafeInteger(value)||value<1||value>3)throw Error('contract');return value;}").unwrap();
    assert!(f.project.check().unwrap().ok);
    f.project.task_plan("latest").unwrap();
    let report = f.adopt(baseline.evidence.id).await;
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    assert_eq!(report.incremental.as_ref().unwrap().reused_cases, 0);
    assert!(report.incremental.as_ref().unwrap().executed_cases > 0);
    assert!(!report.adopted_tasks.contains(&old_id));
    assert!(!report.adopted_tasks.is_empty(), "{report:#?}");
    let session = f.project.task_session(None, None).unwrap();
    for id in report.adopted_tasks {
        assert_eq!(session.states[&id].check, dctx0_tasks::CheckStatus::Pass);
    }
}
#[tokio::test]
async fn actual_legacy_contract_cases_are_reused_and_revision_or_source_changes_execute_again() {
    let f = Fixture::new();
    let first = f.run(None).await;
    assert_eq!(
        first.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        first.evidence.run
    );
    let count = f.calls();
    assert!(count > 0);
    let second = f.run(Some(first.evidence.id.clone())).await;
    assert_eq!(second.evidence.run.result, VerificationResult::Pass);
    assert_eq!(
        f.calls(),
        count,
        "reuse must skip real production callbacks"
    );
    assert_eq!(second.evidence.run.cases, first.evidence.run.cases);
    assert!(second.adopted_tasks.is_empty());
    assert!(
        second
            .evidence
            .run
            .artifacts
            .iter()
            .any(|a| a.media_type == "application/vnd.dctx0.incremental-proof+json")
    );
    fs::write(f.app.join(".dctx0ctl/revision"), "changed").unwrap();
    let third = f.run(Some(second.evidence.id)).await;
    assert_eq!(third.evidence.run.result, VerificationResult::Pass);
    assert!(
        f.calls() > count,
        "changed fixture revision must execute actual callbacks"
    );
    fs::write(
        f.app.join("app.ts"),
        "export function factory(value:unknown){return 9;}",
    )
    .unwrap();
    let failed = f.run(Some(third.evidence.id)).await;
    assert!(
        matches!(failed.evidence.run.result, VerificationResult::Fail { .. }),
        "changed production must run and fail, never reuse old PASS"
    );
}

#[tokio::test]
async fn declared_live_revision_allows_whole_operation_unit_reuse_and_invalidates_on_change() {
    let f = Fixture::new();
    fs::write(f.root.join("spec/w.dctx0"),"workspace W { value A = Int { range 1..2 } surface Api { operation next(a:A) -> Int { pure\nensures result == a + 1 } } }").unwrap();
    assert!(f.project.check().unwrap().ok);
    let configpath = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&configpath).unwrap()).unwrap();
    config["scope"]["subjects"] = serde_json::json!(["W"]);
    fs::write(configpath, serde_json::to_vec(&config).unwrap()).unwrap();
    let counter = serde_json::to_string(&f.app.join(".dctx0ctl/calls").to_string_lossy()).unwrap();
    let mut source = fs::read_to_string(f.app.join("app.ts")).unwrap();
    source.push_str(&format!("\nexport function nextValue(value:number){{appendFileSync({counter},'operation\\n');return value+1;}}"));
    fs::write(f.app.join("app.ts"), source).unwrap();
    let path = f.app.join(".dctx0/testing/typescript/adapter.ts");
    let adapter=fs::read_to_string(&path).unwrap().replace("import {factory}","import {factory,nextValue}").replace("export const adapter=","import {invokeApplication} from './contract.ts';export const adapter=").replace("subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}]", "subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'},{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'nextValue'}]").replace("capabilities:['sessions','values','events']","capabilities:['sessions','values','events','invocation']").replace("async invoke(){throw Error('No operation');}","async invoke(invocation){return invokeApplication(()=>nextValue(Number(invocation.arguments[0].value)),value=>({kind:'integer',value:String(value)}));}");
    fs::write(path, adapter).unwrap();
    fs::write(f.app.join(".dctx0/reuse.json"),r#"{"schema_version":1,"deterministic_units":["contract:W.A","other-cases"],"revisions":{"database":{"executable":"cat","args":[".dctx0ctl/revision"],"timeout_ms":1000}}}"#).unwrap();
    f.plan_tasks();
    let first = f.run(None).await;
    assert_eq!(
        first.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        first.evidence.run
    );
    assert!(
        first
            .evidence
            .run
            .cases
            .iter()
            .any(|case| case.kind == dctx0_core::VerificationCaseKind::Operation)
    );
    let count = f.calls();
    let second = f.run(Some(first.evidence.id)).await;
    assert_eq!(second.evidence.run.result, VerificationResult::Pass);
    assert_eq!(f.calls(), count);
    assert_eq!(second.incremental.as_ref().unwrap().executed_cases, 0);
    let third = f.adopt(second.evidence.id).await;
    assert_eq!(third.evidence.run.result, VerificationResult::Pass);
    assert_eq!(third.incremental.as_ref().unwrap().executed_cases, 0);
    assert_eq!(f.calls(), count);
    let session = f.project.task_session(None, None).unwrap();
    let operation = session
        .plan
        .members
        .iter()
        .find(|member| member.candidate.kind == dctx0_tasks::TaskKind::Operation)
        .unwrap()
        .task_id;
    assert!(third.adopted_tasks.contains(&operation), "{third:#?}");
    fs::write(f.app.join(".dctx0ctl/revision"), "new-live-revision").unwrap();
    let changed = f.run(Some(third.evidence.id)).await;
    assert_eq!(changed.evidence.run.result, VerificationResult::Pass);
    assert!(f.calls() > count);
    assert_eq!(changed.incremental.as_ref().unwrap().reused_cases, 0);
}

#[tokio::test]
async fn incremental_execution_retains_preflight_input_drift_guard() {
    let f = Fixture::new();
    let selection = VerificationSelection::default();
    let preflight = f
        .project
        .verification_preflight(
            Some(&f.app),
            selection.clone(),
            VerificationIntent::Diagnostic,
            None,
        )
        .unwrap();
    assert!(preflight.permits_execution());
    fs::write(
        f.app.join("app.ts"),
        "export function factory(value){return value;}",
    )
    .unwrap();
    let error = f
        .project
        .run_verification_incremental_preflighted(
            Some(&f.app),
            selection,
            &BunHarness,
            &NativeCancellation::default(),
            &NativeReuseOptions {
                mode: Default::default(),
                inputs: ".dctx0/reuse.json".into(),
                parent: None,
            },
            preflight,
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("PREFLIGHT_INPUTS_CHANGED"),
        "{error}"
    );
    assert!(!f.app.join(".dctx0ctl/calls").exists());
}

#[tokio::test]
async fn unchanged_native_cases_reuse_without_reconciling_historical_task_occurrence() {
    let f = Fixture::new();
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let first = f.run(None).await;
    assert_eq!(first.evidence.run.result, VerificationResult::Pass);
    let count = f.calls();
    assert!(f.project.check().unwrap().ok);
    let second = f.run(Some(first.evidence.id.clone())).await;
    assert_eq!(
        second.incremental.as_ref().unwrap().executed_cases,
        0,
        "old={:?} new={:?}",
        first.evidence.run.provenance,
        second.evidence.run.provenance
    );
    assert_eq!(f.calls(), count);
    assert!(
        second
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|i| i.code == "ADOPTION_HISTORICAL_SCOPE")
    );
    assert!(second.adopted_tasks.is_empty());
}
