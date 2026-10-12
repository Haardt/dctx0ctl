use dctx0_core::*;
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
    project: Project,
    app: PathBuf,
}
impl Fixture {
    fn new(spec: &str, subject: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-run-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();
        let specs = root.join("spec");
        fs::create_dir(&specs).unwrap();
        fs::write(specs.join("w.dctx0"), spec).unwrap();
        let project = Project::new(&specs, None).unwrap();
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
        Self { root, project, app }
    }
    fn adapter(&self, text: &str) {
        fs::write(self.app.join(".dctx0/testing/typescript/adapter.ts"), text).unwrap();
    }
    async fn run(&self, no_adopt: bool) -> NativeVerificationReport {
        self.project
            .run_verification(
                Some(&self.app),
                Default::default(),
                &BunHarness,
                &NativeCancellation::default(),
                no_adopt,
            )
            .await
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
const VALUE_ADAPTER: &str = r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#;

#[tokio::test]
async fn real_session_initialization_and_cleanup_failures_retain_attempts_and_cannot_adopt() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap().event_seq;
    for adapter in [
        VALUE_ADAPTER.replace(
            "async close(){}",
            "async close(){throw new Error('Actual cleanup failed');}",
        ),
        VALUE_ADAPTER.replace(
            "async begin(){return",
            "async begin(){throw new Error('Actual initialization failed');return",
        ),
    ] {
        f.adapter(&adapter);
        let report = f.run(false).await;
        let run = &report.evidence.run;
        assert!(matches!(run.result, VerificationResult::Unknown { .. }));
        assert!(!run.required_cases.is_empty());
        assert_eq!(
            run.cases.len(),
            1,
            "Fatal session errors must stop subsequent mandatory cases"
        );
        assert!(matches!(
            run.cases[0].result,
            VerificationResult::Unknown { .. }
        ));
        assert!(
            run.open_obligations
                .iter()
                .any(|issue| issue.code == "NATIVE_RUN_ABORTED")
        );
        assert!(
            run.artifacts
                .iter()
                .any(|artifact| artifact.media_type == "application/vnd.dctx0.session+json")
        );
        assert!(report.adopted_tasks.is_empty());
        assert_eq!(
            f.project.task_session(None, None).unwrap().event_seq,
            before
        );
        assert_eq!(
            f.project
                .saved_verification_run(&report.evidence.id)
                .unwrap(),
            report.evidence
        );
    }
}

#[tokio::test]
async fn event_examples_require_a_real_advertised_observer_and_preserve_missing_cases() {
    let f = Fixture::new(
        "workspace W { event Changed { number:Int } surface Api { operation next() -> Int { emit Changed(number: 5) } }\nexample \"event\" { when result = Api.next()\nexpect result == 5\nexpect emits Changed { number = 5 } } }",
        "W.event",
    );
    fs::write(
        f.app.join("app.ts"),
        "export function next(emit:(value:number)=>void){emit(5);return 5;}",
    )
    .unwrap();
    let adapter = r#"import {next} from '../../../app.ts';
import {invokeApplication} from './contract.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'next'}],artifacts:['app.ts'],capabilities:['sessions','invocation'],representations:{Int:'safe integer'}};},async begin(_caseId,context){return {async invoke(){return invokeApplication(()=>next(number=>context.emit('W.Changed',{kind:'record',value:{number:{kind:'integer',value:String(number)}}})),value=>({kind:'integer',value:String(value)}));},async close(){}};}};"#;
    f.adapter(adapter);
    let missing = f.run(true).await;
    assert!(
        matches!(
            missing.evidence.run.result,
            VerificationResult::Unknown { .. }
        ),
        "{:?}; cases: {:?}",
        missing.evidence.run.result,
        missing
            .evidence
            .run
            .cases
            .iter()
            .map(|case| (&case.id, &case.result))
            .collect::<Vec<_>>()
    );
    assert!(
        missing
            .evidence
            .run
            .open_obligations
            .iter()
            .any(|issue| issue.code == "EXAMPLE_EVENT_OBSERVER"),
        "{:?}; required: {:?}",
        missing.evidence.run.open_obligations,
        missing.evidence.run.required_cases
    );
    assert!(missing.evidence.run.required_cases.contains("W.event"));
    assert!(
        !missing
            .evidence
            .run
            .cases
            .iter()
            .any(|case| case.id == "W.event")
    );
    assert!(missing.adopted_tasks.is_empty());
    f.adapter(&adapter.replace(
        "'sessions','invocation'",
        "'sessions','invocation','events'",
    ));
    let actual = f.run(true).await;
    let case = actual
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "W.event")
        .unwrap();
    assert_eq!(case.result, VerificationResult::Pass);
    assert_eq!(case.observations.len(), 1);
    assert_eq!(case.events.as_ref().unwrap().len(), 1);
    assert_eq!(case.events.as_ref().unwrap()[0].subject, "W.Changed");
    assert!(actual.adopted_tasks.is_empty());
    fs::write(
        f.app.join("app.ts"),
        "export function next(emit:(value:number)=>void){emit(6);return 5;}",
    )
    .unwrap();
    let mutated = f.run(true).await;
    let case = mutated
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "W.event")
        .unwrap();
    assert!(matches!(case.result, VerificationResult::Fail { .. }));
    assert_eq!(case.observations.len(), 1);
    assert_eq!(
        case.events.as_ref().unwrap()[0].payload,
        VerificationValue::Record(
            [("number".into(), VerificationValue::Integer("6".into()))].into()
        )
    );
    assert!(mutated.adopted_tasks.is_empty());
}

#[tokio::test]
async fn escaped_custom_loaders_prevent_adoption_even_when_all_factory_cases_pass() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"),
        "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;} export function unresolved(loader:any){return loader.require;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let report = f.run(false).await;
    assert!(!report.evidence.run.cases.is_empty());
    assert!(
        report
            .evidence
            .run
            .cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    assert!(
        report
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "DYNAMIC_DEPENDENCY_UNRESOLVED"
                && issue.message.starts_with("app.ts:"))
    );
    assert!(report.adopted_tasks.is_empty());
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(before.event_seq, after.event_seq);
    assert!(
        after
            .states
            .values()
            .all(|state| state.work == dctx0_tasks::WorkState::Open
                && state.check == dctx0_tasks::CheckStatus::Unverified)
    );
}

#[tokio::test]
async fn invalid_native_adapter_descriptions_retain_required_cases_and_never_adopt() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap();
    for adapter in [
        VALUE_ADAPTER.replace("protocol_version:1", "protocol_version:2"),
        VALUE_ADAPTER.replace(
            "async describe(){return",
            "async describe(){throw new Error('Initialization failed');return",
        ),
        VALUE_ADAPTER.replace("artifacts:['app.ts']", "artifacts:'app.ts'"),
    ] {
        f.adapter(&adapter);
        let report = f.run(false).await;
        assert!(matches!(
            report.evidence.run.result,
            VerificationResult::Unknown { .. }
        ));
        assert!(
            report
                .evidence
                .run
                .open_obligations
                .iter()
                .any(|issue| issue.code == "ADAPTER_DESCRIPTION")
        );
        assert!(!report.evidence.run.required_cases.is_empty());
        assert!(report.evidence.run.cases.is_empty());
        assert!(report.adopted_tasks.is_empty());
        assert_eq!(
            f.project
                .saved_verification_run(&report.evidence.id)
                .unwrap(),
            report.evidence
        );
        let after = f.project.task_session(None, None).unwrap();
        assert_eq!(after.event_seq, before.event_seq);
        assert!(
            after
                .states
                .values()
                .all(|state| state.work == dctx0_tasks::WorkState::Open
                    && state.check == dctx0_tasks::CheckStatus::Unverified)
        );
    }
}

fn copy_project_fixture(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if matches!(
            entry.file_name().to_str(),
            Some(".git" | ".dctx0ctl" | ".runatron" | ".bin")
        ) {
            continue;
        }
        let path = destination.join(entry.file_name());
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            copy_project_fixture(&entry.path(), &path);
        } else {
            assert!(
                kind.is_file(),
                "Fixture must contain regular captured files"
            );
            fs::copy(entry.path(), path).unwrap();
        }
    }
}

#[tokio::test]
async fn setup_preconditions_are_checked_before_actual_preparation() {
    let f = Fixture::new(
        "workspace W { port Store { operation prepare(value:Int) -> Unit { requires value > 0 } } surface Api { operation next() -> Int { ensures result == 2 } } }",
        "W.Api.next",
    );
    fs::write(f.app.join("app.ts"), "export function prepare(value:number){if(value<=0)throw Error('invalid setup was invoked');} export function next(){return 2;}").unwrap();
    f.adapter(r#"import {prepare,next} from '../../../app.ts';
import {invokeApplication} from './contract.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'next'},{subject:'W.Store.prepare',role:'port',artifact:'app.ts',entry_point:'prepare'}],artifacts:['app.ts'],capabilities:['sessions','events','invocation'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(invocation){return invokeApplication(()=>invocation.subject==='W.Store.prepare'?prepare(Number(invocation.arguments[0].value)):next(),value=>value===undefined?{kind:'unit'}:{kind:'integer',value:String(value)});},async close(){}};}};"#);
    let mut fixtures = dctx0_verification::ProjectionFixtures::default();
    fixtures.operations.insert(
        "W.Api.next".into(),
        [0, 1]
            .into_iter()
            .map(|value| dctx0_verification::OperationCaseInput {
                id: format!("setup-{value}"),
                invocation: Invocation {
                    subject: "W.Api.next".into(),
                    arguments: vec![],
                },
                setup: vec![Invocation {
                    subject: "W.Store.prepare".into(),
                    arguments: vec![VerificationValue::Integer(value.to_string())],
                }],
                mocks: vec![],
            })
            .collect(),
    );
    fs::write(
        f.app.join(".dctx0/testing/typescript/examples.json"),
        serde_json::to_vec(&fixtures).unwrap(),
    )
    .unwrap();
    let report = f.run(true).await;
    let bad = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "setup-0")
        .unwrap();
    assert!(
        matches!(&bad.result, VerificationResult::Unknown { issue } if issue.code == "OPERATION_PRECONDITION_UNMET")
    );
    assert!(bad.observations.is_empty());
    let good = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "setup-1")
        .unwrap();
    assert_eq!(good.result, VerificationResult::Pass);
    assert_eq!(
        good.observations
            .iter()
            .map(|observation| observation.invocation.subject.as_str())
            .collect::<Vec<_>>(),
        ["W.Store.prepare", "W.Api.next"]
    );
}

#[tokio::test]
async fn explicitly_historical_current_check_never_adopts_current_tasks() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    dctx0_engine::create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let plan = f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let result = f
        .project
        .run_verification(
            Some(&f.app),
            VerificationSelection {
                subjects: ["W.A".into()].into(),
                historical_check: Some(plan.check_run_id),
                ..Default::default()
            },
            &dctx0_target_typescript::BunHarness,
            &NativeCancellation::default(),
            false,
        )
        .await
        .unwrap();
    assert_eq!(result.evidence.run.result, VerificationResult::Pass);
    assert!(result.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    fs::write(
        f.project.root.join("w.dctx0"),
        "workspace W { value A = Int { range 1..3 } }",
    )
    .unwrap();
    assert!(f.project.check().unwrap().ok);
    let current = f.project.task_plan("latest").unwrap();
    assert_ne!(current.check_run_id, plan.check_run_id);
    let before_historical = f.project.task_session(None, None).unwrap();
    let historical = f
        .project
        .run_verification(
            Some(&f.app),
            VerificationSelection {
                subjects: ["W.A".into()].into(),
                historical_check: Some(plan.check_run_id),
                ..Default::default()
            },
            &BunHarness,
            &NativeCancellation::default(),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        historical.evidence.run.provenance.snapshot,
        plan.snapshot_id
    );
    assert_eq!(
        historical.evidence.run.provenance.check_occurrence,
        plan.check_run_id
    );
    assert!(historical.evidence.run.provenance.task_pin.is_none());
    assert!(
        historical
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "ADOPTION_HISTORICAL_SCOPE")
    );
    assert!(
        historical
            .evidence
            .run
            .cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    assert!(historical.adopted_tasks.is_empty());
    let after_historical = f.project.task_session(None, None).unwrap();
    assert_eq!(after_historical.plan.id, current.id);
    assert_eq!(after_historical.event_seq, before_historical.event_seq);
    assert_eq!(
        f.project
            .saved_verification_run(&result.evidence.id)
            .unwrap(),
        result.evidence
    );
    assert_eq!(
        f.project
            .saved_verification_run(&historical.evidence.id)
            .unwrap(),
        historical.evidence
    );
}

#[tokio::test]
async fn reviewed_invalid_preconditions_remain_mandatory_without_invoking_production() {
    let f = Fixture::new(
        "workspace W { surface Api { operation next(input:Int) -> Int { requires input > 0\nensures result == input } } }",
        "W.Api.next",
    );
    fs::write(f.app.join("app.ts"), "export function next(input:number){if(input<=0)throw Error('Invalid reviewed input was invoked');return input;}").unwrap();
    f.adapter(r#"import {next} from '../../../app.ts';
import {invokeApplication} from './contract.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'next'}],artifacts:['app.ts'],capabilities:['sessions','events','invocation'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(i){return invokeApplication(()=>next(Number(i.arguments[0].value)),value=>({kind:'integer',value:String(value)}));},async close(){}};}};"#);
    let mut fixtures = dctx0_verification::ProjectionFixtures::default();
    fixtures.operations.insert(
        "W.Api.next".into(),
        vec![dctx0_verification::OperationCaseInput {
            id: "reviewed-invalid-input".into(),
            invocation: Invocation {
                subject: "W.Api.next".into(),
                arguments: vec![VerificationValue::Integer("0".into())],
            },
            setup: vec![],
            mocks: vec![],
        }],
    );
    fs::write(
        f.app.join(".dctx0/testing/typescript/examples.json"),
        serde_json::to_vec(&fixtures).unwrap(),
    )
    .unwrap();
    let report = f.run(true).await;
    assert!(
        report
            .evidence
            .run
            .required_cases
            .contains("reviewed-invalid-input")
    );
    let case = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "reviewed-invalid-input")
        .unwrap();
    assert!(
        matches!(&case.result, VerificationResult::Unknown { issue } if issue.code == "OPERATION_PRECONDITION_UNMET")
    );
    assert!(case.observations.is_empty());
    assert_eq!(
        case.arguments,
        vec![vec![VerificationValue::Integer("0".into())]]
    );
    assert!(report.adopted_tasks.is_empty());
    assert_eq!(
        f.project
            .saved_verification_run(&report.evidence.id)
            .unwrap(),
        report.evidence
    );
}

#[tokio::test]
async fn invalid_reviewed_signatures_are_rejected_before_native_execution_or_task_changes() {
    let f = Fixture::new(
        "workspace W { surface Api { operation next(input:Int) -> Int { ensures result == input } } }",
        "W.Api.next",
    );
    let production = "export function next(input:number){return input;}";
    fs::write(f.app.join("app.ts"), production).unwrap();
    f.adapter(r#"import {next} from '../../../app.ts';
export const adapter={async describe(){throw new Error('Invalid plans must not execute this adapter');},async begin(){throw new Error('Invalid plans must not begin');}};"#);
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap().event_seq;
    let snapshot = f.project.load("latest").unwrap().id;
    for bad_subject in [false, true] {
        let mut fixtures = dctx0_verification::ProjectionFixtures::default();
        fixtures.operations.insert(
            "W.Api.next".into(),
            vec![dctx0_verification::OperationCaseInput {
                id: "invalid-signature".into(),
                invocation: Invocation {
                    subject: if bad_subject {
                        "W.Missing.next".into()
                    } else {
                        "W.Api.next".into()
                    },
                    arguments: if bad_subject {
                        vec![VerificationValue::Integer("1".into())]
                    } else {
                        vec![]
                    },
                },
                setup: vec![],
                mocks: vec![],
            }],
        );
        fs::write(
            f.app.join(".dctx0/testing/typescript/examples.json"),
            serde_json::to_vec(&fixtures).unwrap(),
        )
        .unwrap();
        let rejected = f
            .project
            .run_verification(
                Some(&f.app),
                Default::default(),
                &BunHarness,
                &NativeCancellation::default(),
                false,
            )
            .await
            .unwrap_err();
        assert!(
            rejected
                .to_string()
                .contains("Invalid reviewed operation cases"),
            "{rejected}"
        );
        assert_eq!(
            f.project.task_session(None, None).unwrap().event_seq,
            before
        );
        assert_eq!(f.project.load("latest").unwrap().id, snapshot);
        assert_eq!(
            fs::read_to_string(f.app.join("app.ts")).unwrap(),
            production
        );
    }
}

#[tokio::test]
async fn unavailable_original_or_captured_inputs_preserve_cases_but_never_adopt() {
    for (mode, code) in [
        ("application", "APPLICATION_INPUTS_UNAVAILABLE"),
        ("spec", "SPEC_INPUTS_UNAVAILABLE"),
        ("capture", "CAPTURED_INPUTS_UNAVAILABLE"),
    ] {
        let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
        let application = serde_json::to_string(&f.app.to_string_lossy()).unwrap();
        let spec =
            serde_json::to_string(&f.project.root.join("w.dctx0").to_string_lossy()).unwrap();
        let mutation = match mode {
            "application" => format!("renameSync({application},{application}+'.moved');"),
            "spec" => format!("writeFileSync({spec},new Uint8Array([255]));"),
            _ => "renameSync(import.meta.path,import.meta.path+'.saved');symlinkSync('missing-source.ts',import.meta.path);".into(),
        };
        fs::write(f.app.join("app.ts"), format!("import {{renameSync,writeFileSync,symlinkSync}} from 'node:fs';let changed=false; export function factory(value:number){{if(!changed){{changed=true;{mutation}}}if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}}")).unwrap();
        f.adapter(VALUE_ADAPTER);
        create_profiles(
            &f.app,
            &dctx0_target_typescript::Provider,
            true,
            false,
            false,
        )
        .unwrap();
        f.project.task_plan("latest").unwrap();
        let before = f.project.task_session(None, None).unwrap().event_seq;
        let snapshot = f.project.load("latest").unwrap().id;
        let report = f.run(false).await;
        assert_eq!(
            report.evidence.run.required_cases.len(),
            report.evidence.run.cases.len()
        );
        assert!(
            report
                .evidence
                .run
                .cases
                .iter()
                .all(|case| case.result == VerificationResult::Pass)
        );
        assert!(
            report
                .evidence
                .run
                .provenance
                .incomplete
                .iter()
                .any(|issue| issue.code == code),
            "{mode}: {:?}",
            report.evidence.run.provenance.incomplete
        );
        assert!(report.adopted_tasks.is_empty());
        assert_eq!(
            f.project.task_session(None, None).unwrap().event_seq,
            before
        );
        assert_eq!(f.project.load("latest").unwrap().id, snapshot);
        assert_eq!(
            f.project
                .saved_verification_run(&report.evidence.id)
                .unwrap(),
            report.evidence
        );
    }
}

#[tokio::test]
async fn declared_setup_failure_is_preserved_and_never_establishes_operation_prerequisites() {
    let f = Fixture::new(
        "workspace W { failure Unavailable { reason:String } port Store { operation prepare() -> Unit ! Unavailable } surface Api { operation next() -> Int { ensures result == 2 } } }",
        "W.Api.next",
    );
    fs::write(f.app.join("app.ts"), "export function prepare(){return {ok:false,reason:'Actual storage unavailable'};} export function next(){return 2;}").unwrap();
    f.adapter(r#"import {prepare,next} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'next'},{subject:'W.Store.prepare',role:'port',artifact:'app.ts',entry_point:'prepare'}],artifacts:['app.ts'],capabilities:['sessions','events','invocation'],representations:{Int:'safe integer',Unavailable:'tagged error result'}};},async begin(){return {async invoke(i){if(i.subject==='W.Store.prepare'){const actual=prepare();return {kind:'returned',value:{kind:'failure',value:{declaration:'W.Unavailable',payload:{kind:'record',value:{reason:{kind:'string',value:actual.reason}}}}}};}return {kind:'returned',value:{kind:'integer',value:String(next())}};},async close(){}};}};"#);
    let mut fixtures = dctx0_verification::ProjectionFixtures::default();
    fixtures.operations.insert(
        "W.Api.next".into(),
        vec![dctx0_verification::OperationCaseInput {
            id: "setup-declared-failure".into(),
            invocation: Invocation {
                subject: "W.Api.next".into(),
                arguments: vec![],
            },
            setup: vec![Invocation {
                subject: "W.Store.prepare".into(),
                arguments: vec![],
            }],
            mocks: vec![],
        }],
    );
    fs::write(
        f.app.join(".dctx0/testing/typescript/examples.json"),
        serde_json::to_vec(&fixtures).unwrap(),
    )
    .unwrap();
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap().event_seq;
    let report = f.run(false).await;
    let attempted = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "setup-declared-failure")
        .unwrap();
    assert!(
        matches!(&attempted.result, VerificationResult::Unknown { issue } if issue.code == "SETUP_REQUIREMENT_UNDELIVERED")
    );
    assert_eq!(attempted.observations.len(), 1);
    assert_eq!(
        attempted.observations[0].invocation.subject,
        "W.Store.prepare"
    );
    assert!(
        matches!(&attempted.observations[0].observation, Observation::Returned { value: VerificationValue::Failure { declaration, payload: Some(payload) } } if declaration == "W.Unavailable" && matches!(payload.as_ref(), VerificationValue::Record(fields) if fields.get("reason") == Some(&VerificationValue::String("Actual storage unavailable".into()))))
    );
    assert!(report.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before
    );
    assert_eq!(
        f.project
            .saved_verification_run(&report.evidence.id)
            .unwrap(),
        report.evidence
    );
}

#[tokio::test]
async fn spec_edits_during_actual_execution_preserve_evidence_and_prevent_adoption() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    let spec = serde_json::to_string(&f.project.root.join("w.dctx0").to_string_lossy()).unwrap();
    fs::write(f.app.join("app.ts"), format!("import {{appendFileSync}} from 'node:fs'; export function factory(value:number){{appendFileSync({spec},'\\n// edited while verification executes\\n');if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}}")).unwrap();
    f.adapter(VALUE_ADAPTER);
    dctx0_engine::create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let result = f.run(false).await;
    assert_eq!(result.evidence.run.result, VerificationResult::Pass);
    assert!(
        result
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "SPEC_INPUTS_CHANGED")
    );
    assert!(result.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    assert!(
        fs::read_to_string(f.project.root.join("w.dctx0"))
            .unwrap()
            .contains("edited while verification executes")
    );
    assert_eq!(
        f.project
            .saved_verification_run(&result.evidence.id)
            .unwrap(),
        result.evidence
    );
}

#[tokio::test]
async fn profiles_changed_during_native_execution_preserve_evidence_without_partial_delivery() {
    for corrupt_manifest in [false, true] {
        let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
        create_profiles(
            &f.app,
            &dctx0_target_typescript::Provider,
            true,
            false,
            false,
        )
        .unwrap();
        let relative = if corrupt_manifest {
            ".dctx0/profiles/typescript/manifest.json"
        } else {
            ".dctx0/profiles/typescript/common.md"
        };
        let profile = f.app.join(relative);
        assert!(profile.is_file());
        let quoted = serde_json::to_string(&profile.to_string_lossy()).unwrap();
        fs::write(f.app.join("app.ts"), format!("import {{appendFileSync}} from 'node:fs'; export function factory(value:number){{appendFileSync({quoted},'\\nchanged during native verification\\n');if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}}")).unwrap();
        f.adapter(VALUE_ADAPTER);
        f.project.task_plan("latest").unwrap();
        let before = f.project.task_session(None, None).unwrap().event_seq;
        let report = f.run(false).await;
        assert_eq!(report.evidence.run.result, VerificationResult::Pass);
        assert!(report.adopted_tasks.is_empty());
        assert!(
            report
                .evidence
                .run
                .provenance
                .incomplete
                .iter()
                .any(|issue| issue.code == "APPLICATION_INPUTS_CHANGED"),
            "Profile edits must invalidate captured input freshness: {:?}",
            report.evidence.run.provenance.incomplete
        );
        let after = f.project.task_session(None, None).unwrap();
        assert_eq!(after.event_seq, before);
        assert!(
            after
                .states
                .values()
                .all(|state| state.work == dctx0_tasks::WorkState::Open
                    && state.check == dctx0_tasks::CheckStatus::Unverified)
        );
        assert_eq!(
            f.project
                .saved_verification_run(&report.evidence.id)
                .unwrap(),
            report.evidence
        );
        assert!(
            fs::read_to_string(profile)
                .unwrap()
                .contains("changed during native verification")
        );
    }
}

#[tokio::test]
async fn missing_required_profile_documents_block_adoption_until_a_new_complete_run() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap().event_seq;
    let common = f.app.join(".dctx0/profiles/typescript/common.md");
    let retained = common.with_extension("saved");
    fs::rename(&common, &retained).unwrap();
    let incomplete = f.run(false).await;
    assert!(
        incomplete
            .evidence
            .run
            .cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    assert!(
        incomplete
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "ADOPTION_PROFILE_INCOMPLETE")
    );
    let pin = incomplete
        .evidence
        .run
        .provenance
        .task_pin
        .as_ref()
        .unwrap();
    assert!(!pin.requirements.is_empty());
    assert!(pin.profile_bundles.is_empty());
    assert!(incomplete.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before
    );
    fs::rename(retained, common).unwrap();
    let complete = f.run(false).await;
    assert_eq!(complete.evidence.run.result, VerificationResult::Pass);
    assert!(complete.evidence.run.provenance.incomplete.is_empty());
    assert!(!complete.adopted_tasks.is_empty());
    assert_ne!(complete.evidence.id, incomplete.evidence.id);
    assert_eq!(
        f.project
            .saved_verification_run(&incomplete.evidence.id)
            .unwrap(),
        incomplete.evidence
    );
}

#[tokio::test]
async fn complete_native_contract_evidence_adopts_open_work_and_preserves_historical_pass() {
    let f = Fixture::new(
        "workspace W {\nintent \"Explain the numeric contract without adding an assertion\"\nvalue A = Int { range 1..2 }\n}",
        "W.A",
    );
    fs::write(f.app.join("app.ts"), "import bounds from './bounds.cjs'; export function factory(value:number){if(!Number.isInteger(value)||value<bounds.min||value>bounds.max)throw TypeError('invalid');return value;}").unwrap();
    fs::write(
        f.app.join("bounds.cjs"),
        "module.exports = module.require('./limits.cjs');",
    )
    .unwrap();
    fs::write(
        f.app.join("limits.cjs"),
        "module.exports = require('bounds-package');",
    )
    .unwrap();
    fs::create_dir_all(f.app.join("node_modules/bounds-package")).unwrap();
    fs::write(
        f.app.join("node_modules/bounds-package/package.json"),
        r#"{"name":"bounds-package","version":"1.0.0","main":"index.cjs"}"#,
    )
    .unwrap();
    fs::write(
        f.app.join("node_modules/bounds-package/index.cjs"),
        "module.exports = require('./implementation.cjs');",
    )
    .unwrap();
    fs::write(
        f.app.join("node_modules/bounds-package/implementation.cjs"),
        "const path = require('path'); const api = require('bun'); const sqlite = require('bun:sqlite'); if(path.basename('x') !== 'x' || typeof api.plugin !== 'function' || typeof sqlite.Database !== 'function')throw Error('builtin unavailable'); module.exports = {min:1,max:2};",
    )
    .unwrap();
    f.adapter(VALUE_ADAPTER);
    // Historical tooling outside the adapter's import closure is still
    // captured, but cannot create a fictitious production dependency.
    fs::write(f.app.join("legacy.ts"), "import 'missing-legacy-package';").unwrap();
    fs::create_dir_all(f.app.join("node_modules/unused-package")).unwrap();
    fs::write(
        f.app.join("node_modules/unused-package/broken.js"),
        "import broken {",
    )
    .unwrap();
    dctx0_engine::create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let plan = f.project.task_plan("latest").unwrap();
    let id = plan
        .members
        .iter()
        .find(|member| member.candidate.subjects == ["W.A"])
        .unwrap()
        .task_id;
    let run = f.run(false).await;
    assert_eq!(run.evidence.run.result, VerificationResult::Pass);
    assert!(
        run.evidence.run.provenance.incomplete.is_empty(),
        "{:?}",
        run.evidence.run.provenance.incomplete
    );
    assert_eq!(run.adopted_tasks, vec![id], "{:?}", run.adoption_issues);
    let scope = run
        .evidence
        .run
        .artifacts
        .iter()
        .find(|artifact| artifact.media_type == "application/vnd.dctx0.dependency-scope+json")
        .unwrap();
    let store = dctx0_store::Store::open(
        &f.root.join("spec/.dctx0ctl/state.sqlite3"),
        AccessMode::ReadOnly,
    )
    .unwrap();
    let scope = store
        .verification_artifact(&run.evidence.id, &scope.hash)
        .unwrap();
    let scope: serde_json::Value = serde_json::from_slice(&scope.contents).unwrap();
    for dependency in [
        "bounds.cjs",
        "limits.cjs",
        "node_modules/bounds-package/index.cjs",
        "node_modules/bounds-package/implementation.cjs",
    ] {
        assert!(
            scope["visited"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(dependency)),
            "Missing actual CommonJS dependency {dependency}: {scope}"
        );
    }
    assert!(
        scope["visited"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("app.ts"))
    );
    assert!(
        scope["not_selected"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("legacy.ts"))
    );
    let delivered = f.project.task_session(None, None).unwrap();
    assert_eq!(delivered.states[&id].work, dctx0_tasks::WorkState::Done);
    assert_eq!(delivered.states[&id].check, dctx0_tasks::CheckStatus::Pass);
    let repeated = f.run(false).await;
    assert!(repeated.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        delivered.event_seq
    );
    fs::remove_file(f.app.join("node_modules/bounds-package/package.json")).unwrap();
    fs::write(
        f.app.join("limits.cjs"),
        "module.exports = require('./node_modules/bounds-package/index.cjs');",
    )
    .unwrap();
    let missing_manifest = f.run(false).await;
    assert_eq!(
        missing_manifest.evidence.run.result,
        VerificationResult::Pass
    );
    assert!(
        missing_manifest
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "PACKAGE_MANIFEST_NOT_CAPTURED")
    );
    assert!(missing_manifest.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        delivered.event_seq
    );
    fs::write(
        f.app.join("node_modules/bounds-package/package.json"),
        r#"{"name":"bounds-package","version":"1.0.0","main":"index.cjs"}"#,
    )
    .unwrap();
    fs::write(
        f.app.join("limits.cjs"),
        "module.exports = require('bounds-package');",
    )
    .unwrap();
    fs::write(
        f.app.join("bounds.cjs"),
        "const load = module.require.bind(module); module.exports = load('./limits.cjs');",
    )
    .unwrap();
    let escaped_loader = f.run(false).await;
    assert_eq!(escaped_loader.evidence.run.result, VerificationResult::Pass);
    assert!(
        escaped_loader
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "DYNAMIC_DEPENDENCY_UNRESOLVED")
    );
    assert!(escaped_loader.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        delivered.event_seq
    );
    fs::write(
        f.app.join("bounds.cjs"),
        "module.exports = module.require('./limits.cjs');",
    )
    .unwrap();
    fs::write(
        f.app.join("legacy.ts"),
        "export function unresolved(name:string){return import(name);}",
    )
    .unwrap();
    fs::write(f.app.join("app.ts"), "import './legacy.ts'; export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    let incomplete = f.run(false).await;
    assert!(
        incomplete
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "DYNAMIC_DEPENDENCY_UNRESOLVED")
    );
    assert!(incomplete.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        delivered.event_seq
    );
    fs::write(
        f.app.join("app.ts"),
        "export function factory(){return 999;}",
    )
    .unwrap();
    let failure = f.run(false).await;
    assert!(matches!(
        failure.evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    assert!(failure.adopted_tasks.is_empty());
    assert!(
        failure
            .adoption_issues
            .iter()
            .any(|issue| issue.code == "ADOPTION_COMPLETE_RUN_REQUIRED")
    );
    assert!(failure.adoption_proposal.is_none());
    let retained = f.project.task_session(None, None).unwrap();
    assert_eq!(retained.event_seq, delivered.event_seq);
    assert_eq!(retained.states[&id].check, dctx0_tasks::CheckStatus::Pass);
}

#[tokio::test]
async fn cinema_existing_domain_contracts_adopt_without_loading_unselected_frameworks() {
    let f = Fixture::new("workspace W { type A = Int }", "W.A");
    let reference = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema");
    fs::remove_file(f.project.root.join("w.dctx0")).unwrap();
    for name in ["workspace.dctx0", "movie-night.dctx0"] {
        fs::copy(
            reference.join("dctx0").join(name),
            f.project.root.join(name),
        )
        .unwrap();
    }
    assert!(f.project.check().unwrap().ok);
    fs::create_dir_all(f.app.join("src/domain")).unwrap();
    for name in ["film-id.ts", "release-year.ts"] {
        let original = fs::read(reference.join("src/domain").join(name)).unwrap();
        fs::write(f.app.join("src/domain").join(name), &original).unwrap();
        assert_eq!(
            fs::read(f.app.join("src/domain").join(name)).unwrap(),
            original
        );
    }
    f.adapter(
        &fs::read_to_string(reference.join(".dctx0/testing/typescript/domain-adapter.ts")).unwrap(),
    );
    let subjects = ["FilmId", "MemberId", "CommentId", "PollId", "ReleaseYear"]
        .map(|name| format!("HomeCinema.MovieNight.{name}"));
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["scope"]["subjects"] = serde_json::json!(subjects);
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    dctx0_engine::create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let plan = f.project.task_plan("latest").unwrap();
    let ids = subjects
        .iter()
        .map(|subject| {
            plan.members
                .iter()
                .find(|member| member.candidate.subjects == [subject.as_str()])
                .unwrap()
                .task_id
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut compiler = NativeCommand::spawn(
        &dctx0_target_sdk::ExecutionRecipe {
            program: "bun".into(),
            arguments: vec![
                reference
                    .join("node_modules/typescript/bin/tsc")
                    .to_string_lossy()
                    .into_owned(),
                "--strict".into(),
                "--noEmit".into(),
                "--target".into(),
                "ES2022".into(),
                "--module".into(),
                "ESNext".into(),
                "--moduleResolution".into(),
                "Bundler".into(),
                "--allowImportingTsExtensions".into(),
                "--skipLibCheck".into(),
                ".dctx0/testing/typescript/adapter.ts".into(),
            ],
        },
        &f.app,
        NativeLimits::default(),
    )
    .await
    .unwrap();
    compiler.wait(&NativeCancellation::default()).await.unwrap();
    let report = f.run(false).await;
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    assert!(
        report.evidence.run.provenance.incomplete.is_empty(),
        "{:?}",
        report.evidence.run.provenance.incomplete
    );
    assert_eq!(
        report
            .adopted_tasks
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        ids,
        "{:?}",
        report.adoption_issues
    );
    let delivered = f.project.task_session(None, None).unwrap();
    for id in &ids {
        assert_eq!(delivered.states[id].work, dctx0_tasks::WorkState::Done);
        assert_eq!(delivered.states[id].check, dctx0_tasks::CheckStatus::Pass);
    }
    assert!(
        delivered
            .states
            .iter()
            .filter(|(id, _)| !ids.contains(id))
            .all(|(_, state)| state.work == dctx0_tasks::WorkState::Open)
    );
    let film_id = f.app.join("src/domain/film-id.ts");
    let original = fs::read_to_string(&film_id).unwrap();
    fs::write(
        &film_id,
        original.replace(
            " || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(input)",
            "",
        ),
    )
    .unwrap();
    let changed = f.run(false).await;
    assert!(matches!(
        changed.evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    assert!(changed.adopted_tasks.is_empty());
    assert_ne!(changed.evidence.id, report.evidence.id);
    let retained = f.project.task_session(None, None).unwrap();
    assert_eq!(retained.event_seq, delivered.event_seq);
    for id in &ids {
        assert_eq!(retained.states[id].work, dctx0_tasks::WorkState::Done);
        assert_eq!(retained.states[id].check, dctx0_tasks::CheckStatus::Pass);
    }
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/cinema-domain-adoption-validation.json");
    let summary = serde_json::json!({"schema_version":1,"test":"cinema_existing_domain_contracts_adopt_without_loading_unselected_frameworks","status":"pass","subjects":subjects,"adopted_task_ids":ids,"positive_run_id":report.evidence.id,"positive_cases":report.evidence.run.cases.len(),"positive_result":report.evidence.run.result,"positive_provenance":report.evidence.run.provenance,"mutation_run_id":changed.evidence.id,"mutation_result":changed.evidence.run.result,"mutation_cases":changed.evidence.run.cases.len(),"mutation_adopted_tasks":changed.adopted_tasks,"historical_task_event_sequence_preserved":retained.event_seq==delivered.event_seq,"scope":"Five existing Cinema domain contracts; original full spec and byte-identical production validators copied to an isolated application/history. No framework or whole-application proof.","typescript_typecheck":"pass","performance_status":"deferred"});
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    let temporary = output.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temporary, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
    fs::rename(temporary, output).unwrap();
}

#[tokio::test]
async fn task_selected_runs_pin_requirements_profiles_and_build_inputs_without_claiming_work() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    fs::write(f.app.join("package.json"), "{\"private\":true}").unwrap();
    f.adapter(VALUE_ADAPTER);
    let snapshot = f.project.load("latest").unwrap().id;
    let tasks = f.project.task_plan(&snapshot).unwrap();
    let task = tasks
        .members
        .iter()
        .find(|m| m.candidate.subjects == ["W.A"])
        .unwrap();
    let selection = VerificationSelection {
        tasks: [task.task_id].into(),
        ..Default::default()
    };
    let before = f.project.task_session(None, None).unwrap();
    let missing = f
        .project
        .run_verification(
            Some(&f.app),
            selection.clone(),
            &BunHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    let pin = missing.evidence.run.provenance.task_pin.as_ref().unwrap();
    assert_eq!(pin.plan_id, tasks.id);
    assert_eq!(pin.event_sequence, before.event_seq);
    assert_eq!(
        pin.requirements,
        [(task.task_id, task.candidate.requirements_hash.clone())].into()
    );
    assert!(pin.profile_bundles.is_empty());
    assert!(
        missing
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|i| i.code == "ADOPTION_PROFILE_UNAVAILABLE")
    );
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let loaded = f
        .project
        .profiles(Some(&f.app), Some("typescript"))
        .unwrap()
        .unwrap();
    let input = f.project.load(&snapshot).unwrap();
    let expected =
        dctx0_tasks::select_profiles(&input, &task.candidate, &loaded.manifest, &loaded.documents);
    expected.require_complete().unwrap();
    let pinned = f
        .project
        .run_verification(
            Some(&f.app),
            selection,
            &BunHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        pinned
            .evidence
            .run
            .provenance
            .task_pin
            .as_ref()
            .unwrap()
            .profile_bundles,
        [(task.task_id, expected.bundle.hash)].into()
    );
    assert_eq!(
        pinned.evidence.run.provenance.build_configuration["package.json"],
        pinned.evidence.run.provenance.implementation["package.json"]
    );
    assert!(pinned.adopted_tasks.is_empty());
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

#[tokio::test]
async fn wrong_native_value_shapes_resource_limits_and_empty_plans_remain_incomplete() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(
        f.app.join("app.ts"),
        "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}",
    )
    .unwrap();
    f.adapter(&VALUE_ADAPTER.replace(
        "{kind:'integer',value:String(result)}",
        "{kind:'boolean',value:true}",
    ));
    assert!(
        matches!(f.run(true).await.evidence.run.result,VerificationResult::Fail{issue} if issue.code=="NATIVE_VALUE_CONTRACT")
    );
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: VerificationConfiguration =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config.budgets.value_bytes = 64;
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    f.adapter(
        &VALUE_ADAPTER
            .replace("String(result)", "'1'.repeat(128)")
            .replace("['app.ts']", "['../outside.ts']"),
    );
    let limited = f.run(true).await;
    assert!(matches!(
        limited.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    assert!(
        limited
            .evidence
            .run
            .cases
            .iter()
            .filter(|c| c.bindings["expect_rejection"] == VerificationValue::Boolean(false))
            .all(|c| matches!(c.result, VerificationResult::Unknown { .. }))
    );
    assert!(
        limited
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|i| i.code == "IMPLEMENTATION_ARTIFACT_OUTSIDE_CAPTURE")
    );
    let empty = Fixture::new("workspace W {}", "W");
    empty.adapter("export const adapter={async describe(){return {protocol_version:1,subjects:[],artifacts:[],capabilities:['events'],representations:{}};},async begin(){throw Error('Must not begin');}};");
    let report = empty.run(true).await;
    assert!(report.evidence.run.cases.is_empty());
    assert!(
        matches!(report.evidence.run.result,VerificationResult::Unknown{issue} if issue.code=="INCOMPLETE_RUN")
    );
}

struct MutatedGenerator {
    mode: &'static str,
}

struct WorkerFault(&'static str);
impl dctx0_target_sdk::HarnessGenerator for WorkerFault {
    fn scaffold(&self) -> dctx0_target_sdk::Artifact {
        dctx0_target_sdk::HarnessGenerator::scaffold(&BunHarness)
    }
    fn inspect_dependencies(
        &self,
        sources: &[dctx0_target_sdk::InspectionSource],
    ) -> std::result::Result<dctx0_target_sdk::DependencyInspection, dctx0_target_sdk::Error> {
        dctx0_target_sdk::HarnessGenerator::inspect_dependencies(&BunHarness, sources)
    }
    fn module_observer_artifacts(
        &self,
        sources: &[dctx0_target_sdk::InspectionSource],
        root: &str,
    ) -> std::result::Result<Vec<dctx0_target_sdk::Artifact>, dctx0_target_sdk::Error> {
        dctx0_target_sdk::HarnessGenerator::module_observer_artifacts(&BunHarness, sources, root)
    }
    fn generate(
        &self,
        request: &dctx0_target_sdk::HarnessRequest,
    ) -> std::result::Result<dctx0_target_sdk::HarnessBundle, dctx0_target_sdk::Error> {
        let mut bundle = dctx0_target_sdk::HarnessGenerator::generate(&BunHarness, request)?;
        let worker = bundle
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.path == "worker.ts")
            .unwrap();
        let (before, after) = match self.0 {
            "initial-metrics" => (
                "case 'metrics':{",
                "case 'metrics':{throw new Error('Runtime metrics failed');",
            ),
            "final-metrics" => (
                "case 'metrics':{",
                "case 'metrics':{if(++metricsCount>1)throw new Error('Final metrics failed');",
            ),
            "observer" => (
                "case 'observe-dependencies':{",
                "case 'observe-dependencies':{throw new Error('Module observer failed');",
            ),
            "shutdown" => (
                "case 'shutdown':",
                "case 'shutdown':throw new Error('Shutdown failed');",
            ),
            "exit" => (
                "reply({kind:'bye'});",
                "process.exitCode=7;reply({kind:'bye'});",
            ),
            _ => unreachable!("Unknown test fault"),
        };
        assert!(worker.contents.contains(before));
        worker.contents = worker.contents.replace(before, after);
        if self.0 == "final-metrics" {
            worker.contents.insert_str(0, "let metricsCount=0;\n");
        }
        bundle
            .generator_version
            .push_str(&format!("-test-fault-{}", self.0));
        Ok(bundle)
    }
}

#[tokio::test]
async fn runtime_metrics_module_observer_and_shutdown_faults_never_authorize_adoption() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"), "import {identity} from './loader.mjs'; export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return identity(value);}").unwrap();
    fs::write(f.app.join("loader.mjs"), "export function identity(value){return value;} export function lazyModule(specifier){return import(specifier);}").unwrap();
    f.adapter(VALUE_ADAPTER);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap().event_seq;
    let baseline = f.run(true).await;
    assert_eq!(baseline.evidence.run.result, VerificationResult::Pass);
    assert!(
        baseline.evidence.run.provenance.incomplete.is_empty(),
        "{:?}",
        baseline.evidence.run.provenance.incomplete
    );
    for (mode, code) in [
        ("initial-metrics", "RUNTIME_PROVENANCE"),
        ("final-metrics", "RUNTIME_FINAL_METRICS"),
        ("observer", "MODULE_OBSERVER_UNAVAILABLE"),
        ("shutdown", "NATIVE_SHUTDOWN"),
        ("exit", "NATIVE_CLEANUP"),
    ] {
        let report = f
            .project
            .run_verification(
                Some(&f.app),
                Default::default(),
                &WorkerFault(mode),
                &NativeCancellation::default(),
                false,
            )
            .await
            .unwrap();
        let run = &report.evidence.run;
        assert!(!run.required_cases.is_empty());
        assert!(
            matches!(run.result, VerificationResult::Unknown { .. }),
            "{mode}: {:?}",
            run.result
        );
        assert!(
            run.open_obligations
                .iter()
                .chain(&run.provenance.incomplete)
                .any(|issue| issue.code == code),
            "{mode}: {:?}; {:?}",
            run.open_obligations,
            run.provenance.incomplete
        );
        if mode == "initial-metrics" {
            assert!(run.cases.is_empty());
        } else {
            assert_eq!(run.required_cases.len(), run.cases.len());
            assert!(
                run.cases
                    .iter()
                    .all(|case| case.result == VerificationResult::Pass)
            );
        }
        assert!(report.adopted_tasks.is_empty());
        assert_eq!(
            f.project.task_session(None, None).unwrap().event_seq,
            before
        );
        assert_eq!(
            f.project
                .saved_verification_run(&report.evidence.id)
                .unwrap(),
            report.evidence
        );
    }
}
impl dctx0_target_sdk::HarnessGenerator for MutatedGenerator {
    fn scaffold(&self) -> dctx0_target_sdk::Artifact {
        dctx0_target_sdk::HarnessGenerator::scaffold(&BunHarness)
    }
    fn generate(
        &self,
        request: &dctx0_target_sdk::HarnessRequest,
    ) -> std::result::Result<dctx0_target_sdk::HarnessBundle, dctx0_target_sdk::Error> {
        let mut bundle = dctx0_target_sdk::HarnessGenerator::generate(&BunHarness, request)?;
        match self.mode {
            "version" => bundle.sdk_version = 2,
            "duplicate" => bundle.artifacts.push(bundle.artifacts[0].clone()),
            "adapter" => bundle.artifacts[0].path = "adapter.ts".into(),
            "missing-runtime" => bundle.recipe.program = "/dctx0-missing-bun-runtime".into(),
            path => bundle.artifacts[0].path = path.into(),
        }
        Ok(bundle)
    }
}
#[tokio::test]
async fn incompatible_harnesses_escaping_artifacts_and_missing_runtimes_never_execute_or_adopt() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    let adapter = f.app.join(".dctx0/testing/typescript/adapter.ts");
    let original = fs::read(&adapter).unwrap();
    for mode in ["version", "duplicate", "adapter", "../escape.ts", ".", "/"] {
        assert!(
            f.project
                .run_verification(
                    Some(&f.app),
                    Default::default(),
                    &MutatedGenerator { mode },
                    &NativeCancellation::default(),
                    true
                )
                .await
                .is_err()
        );
        assert_eq!(fs::read(&adapter).unwrap(), original);
    }
    let path = f.app.join(".dctx0/testing/verification.json");
    let mut config: VerificationConfiguration =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    config.runtime = "/dctx0-missing-bun-runtime".into();
    fs::write(path, serde_json::to_vec(&config).unwrap()).unwrap();
    let report = f
        .project
        .run_verification(
            Some(&f.app),
            Default::default(),
            &MutatedGenerator {
                mode: "missing-runtime",
            },
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    assert!(report.evidence.run.cases.is_empty());
    assert!(
        report
            .evidence
            .run
            .open_obligations
            .iter()
            .any(|i| i.code == "NATIVE_INITIALIZATION")
    );
}
#[tokio::test]
async fn actual_factory_results_are_stored_immutably_mutations_fail_and_tasks_do_not_change() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"),"export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    let tasks = f.project.task_plan("latest").unwrap();
    let first = f.run(true).await;
    let reference = first
        .evidence
        .run
        .artifacts
        .iter()
        .find(|a| a.media_type == "application/vnd.dctx0.case-manifest+json")
        .unwrap();
    let store = dctx0_store::Store::open(
        &f.root.join("spec/.dctx0ctl/state.sqlite3"),
        AccessMode::ReadOnly,
    )
    .unwrap();
    let artifact = store
        .verification_artifact(&first.evidence.id, &reference.hash)
        .unwrap();
    let manifest: dctx0_verification::CaseManifest =
        serde_json::from_slice(&artifact.contents).unwrap();
    assert_eq!(manifest.required_cases, first.evidence.run.required_cases);
    assert_eq!(
        manifest.required_cases,
        manifest.case_requirements.keys().cloned().collect()
    );
    assert_eq!(first.evidence.run.result, VerificationResult::Pass);
    assert!(first.evidence.run.required_cases.len() > 2);
    assert_eq!(
        first.evidence.run.cases.len(),
        first.evidence.run.required_cases.len()
    );
    assert!(first.adoption_issues.is_empty());
    assert!(first.adopted_tasks.is_empty());
    assert!(!first.evidence.run.provenance.runtime_version.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().plan.id,
        tasks.id
    );
    assert_eq!(
        f.project
            .saved_verification_run(&first.evidence.id)
            .unwrap(),
        first.evidence
    );
    fs::write(f.app.join("app.ts"),"export function factory(value:number){if(!Number.isInteger(value))throw TypeError('invalid');return value;}").unwrap();
    let unchecked = f.run(true).await;
    assert!(
        matches!(unchecked.evidence.run.result,VerificationResult::Fail{issue} if issue.code=="NATIVE_INVALID_VALUE_ACCEPTED")
    );
    assert!(
        unchecked
            .evidence
            .run
            .cases
            .iter()
            .filter(|c| c.bindings["expect_rejection"] == VerificationValue::Boolean(false))
            .all(|c| c.result == VerificationResult::Pass)
    );
    fs::write(
        f.app.join("app.ts"),
        "export function factory(_value:number){return 1;}",
    )
    .unwrap();
    let changed = f.run(false).await;
    assert!(
        matches!(changed.evidence.run.result,VerificationResult::Fail{issue} if issue.code=="NATIVE_VALUE_CHANGED")
    );
    assert_ne!(
        first.evidence.run.provenance.implementation["app.ts"],
        changed.evidence.run.provenance.implementation["app.ts"]
    );
    assert_eq!(
        f.project
            .saved_verification_run(&first.evidence.id)
            .unwrap()
            .run
            .result,
        VerificationResult::Pass
    );
    assert!(
        changed
            .adoption_issues
            .iter()
            .any(|issue| issue.code == "TASK_PROFILE_UNPINNED")
    );
    assert!(
        f.project
            .task_session(None, None)
            .unwrap()
            .states
            .values()
            .all(|state| state.work == dctx0_tasks::WorkState::Open)
    );
    fs::write(
        f.app.join("app.ts"),
        "export function factory(_value:number){throw TypeError('mutated');}",
    )
    .unwrap();
    assert!(
        matches!(f.run(true).await.evidence.run.result,VerificationResult::Fail{issue} if issue.code=="NATIVE_VALUE_REJECTED")
    );
}

#[tokio::test]
async fn native_suite_policy_artifacts_aggregate_real_examples_and_detect_production_mutation() {
    let f = Fixture::new(
        "workspace W { surface Api { operation next() -> Int { pure\nensures result == 5 } }\nexample \"five\" { when result = Api.next()\nexpect result == 5 }\ntest Examples\npolicy Release { require Examples == PASS } }",
        "W",
    );
    fs::write(f.app.join("app.ts"), "export function next(){return 5;}").unwrap();
    f.adapter(r#"import {next} from '../../../app.ts';
import {invokeApplication} from './contract.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'next'}],artifacts:['app.ts'],capabilities:['sessions','invocation','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){return invokeApplication(()=>next(),value=>({kind:'integer',value:String(value)}));},async close(){}};}};"#);
    for mutated in [false, true] {
        if mutated {
            fs::write(f.app.join("app.ts"), "export function next(){return 6;}").unwrap();
        }
        let run = f.run(true).await;
        let reference = run
            .evidence
            .run
            .artifacts
            .iter()
            .find(|reference| {
                reference.media_type == "application/vnd.dctx0.suite-policy-assessment+json"
            })
            .unwrap();
        let store = dctx0_store::Store::open(
            &f.root.join("spec/.dctx0ctl/state.sqlite3"),
            AccessMode::ReadOnly,
        )
        .unwrap();
        let artifact = store
            .verification_artifact(&run.evidence.id, &reference.hash)
            .unwrap();
        let assessment: serde_json::Value = serde_json::from_slice(&artifact.contents).unwrap();
        let expected_status = if mutated { "FAIL" } else { "PASS" };
        assert_eq!(assessment["suites"][0]["result"]["status"], expected_status);
        assert_eq!(
            assessment["policies"][0]["result"]["status"],
            expected_status
        );
        if !mutated {
            assert!(
                !run.evidence
                    .run
                    .open_obligations
                    .iter()
                    .any(|issue| issue.code == "SUITE_INSPECTION_REQUIRED")
            );
            assert!(
                !run.evidence
                    .run
                    .open_obligations
                    .iter()
                    .any(|issue| issue.message
                        == "W.Release: Policy has no automatically asserted meaning")
            );
        }
        assert!(run.adopted_tasks.is_empty());
    }
    fs::write(f.app.join("app.ts"), "export function next(){return 5;}").unwrap();
    let spec = fs::read_to_string(f.root.join("spec/w.dctx0"))
        .unwrap()
        .replace("require Examples == PASS", "require Examples == FAIL");
    fs::write(f.root.join("spec/w.dctx0"), spec).unwrap();
    assert!(f.project.check().unwrap().ok);
    let contradicted = f.run(true).await;
    assert!(
        contradicted
            .evidence
            .run
            .cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    assert!(
        matches!(contradicted.evidence.run.result, VerificationResult::Fail { issue } if issue.code == "POLICY_REQUIREMENT_VIOLATED")
    );
}

#[tokio::test]
async fn explicit_agent_delivery_binds_actual_sources_and_retains_automatic_unknown_history() {
    let f = Fixture::new(
        "workspace W { constraint \"Keep validation at the application boundary.\"\nvalue A = Int { range 1..2 } }",
        "W",
    );
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let plan = f.project.task_plan("latest").unwrap();
    let member = plan
        .members
        .iter()
        .find(|member| member.candidate.subjects == ["W.A"])
        .unwrap();
    let id = member.task_id;
    let run = f.run(true).await;
    assert!(matches!(
        run.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    let report = TaskAttestation { task: id, review: None, inspection: VerificationAttestation {
        schema_version: 1, actor: "agent-reviewer".into(), subject: format!("T{id}"),
        snapshot: run.evidence.run.provenance.snapshot.clone(), plan_hash: run.evidence.run.provenance.plan_hash.clone(),
        requirements: member.candidate.subjects.iter().chain(&member.candidate.types).chain(&member.candidate.facts).cloned().collect(),
        implementation_artifacts: run.evidence.run.provenance.implementation.clone(),
        report: "Inspected app.ts: factory performs integer and range rejection at the exported boundary. Native mutation evidence remains separate.".into(),
        limitations: vec!["Inspection is an agent judgement, not an exhaustive proof.".into()], result: VerificationResult::Pass,
    }};
    let sequence = f.project.task_session(None, None).unwrap().event_seq;
    let mut stale = report.clone();
    stale
        .inspection
        .implementation_artifacts
        .insert("app.ts".into(), "old-code".into());
    assert!(
        f.project
            .publish_task_attestations(Some(&f.app), &run.evidence.id, &[stale])
            .is_err()
    );
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        sequence
    );
    let original = fs::read(f.app.join("app.ts")).unwrap();
    fs::write(
        f.app.join("app.ts"),
        "export function factory(value:number){return value;}",
    )
    .unwrap();
    assert!(
        f.project
            .publish_task_attestations(
                Some(&f.app),
                &run.evidence.id,
                std::slice::from_ref(&report)
            )
            .is_err()
    );
    fs::write(f.app.join("app.ts"), original).unwrap();
    let receipt = f
        .project
        .publish_task_attestations(Some(&f.app), &run.evidence.id, &[report])
        .unwrap();
    assert_eq!(receipt.deliveries.len(), 1);
    let session = f.project.task_session(None, None).unwrap();
    assert_eq!(session.states[&id].work, dctx0_tasks::WorkState::Done);
    assert_eq!(session.states[&id].check, dctx0_tasks::CheckStatus::Pass);
    let evidence = session.states[&id].evidence.last().unwrap();
    assert_eq!(evidence.verifier, "agent-reviewer");
    let inspected: serde_json::Value = serde_json::from_str(&evidence.report).unwrap();
    assert_eq!(inspected["basis"], "agent");
    assert_eq!(inspected["native_run"], run.evidence.id);
    assert_eq!(
        f.project.saved_verification_run(&run.evidence.id).unwrap(),
        run.evidence
    );
}

#[tokio::test]
async fn inspected_review_batches_roll_back_invalid_findings_and_deliver_through_normal_guards() {
    let f = Fixture::new(
        "workspace W { value A = Int { range 1..2 }\nreview Boundary { intent \"Inspect exported validation.\"\ninspect { implementation contracts }\ncriteria { \"Reject inputs outside the declared range.\" } } }",
        "W",
    );
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let plan = f.project.task_plan("latest").unwrap();
    let run = f.run(true).await;
    let reports = plan.members.iter().map(|member| TaskAttestation {
        task: member.task_id,
        inspection: VerificationAttestation {
            schema_version: 1, actor: "agent-reviewer".into(), subject: format!("T{}", member.task_id),
            snapshot: run.evidence.run.provenance.snapshot.clone(), plan_hash: run.evidence.run.provenance.plan_hash.clone(),
            requirements: member.candidate.subjects.iter().chain(&member.candidate.types).chain(&member.candidate.facts).cloned().collect(),
            implementation_artifacts: run.evidence.run.provenance.implementation.clone(),
            report: "Inspected exported factory and native boundary cases: ordinary application code rejects nonintegers and values outside 1..2.".into(),
            limitations: vec!["Agent inspection, not exhaustive automatic proof.".into()], result: VerificationResult::Pass,
        },
        review: (member.candidate.kind == dctx0_tasks::TaskKind::Review).then(|| dctx0_tasks::ReviewReport {
            conformance: None,
            reviewer: "agent-reviewer".into(), judgement: "approved".into(), summary: "The exported factory owns the integer/range rejection; no unresolved findings.".into(), findings: vec![],
        }),
    }).collect::<Vec<_>>();
    let mut invalid = reports.clone();
    invalid
        .iter_mut()
        .find_map(|report| report.review.as_mut())
        .unwrap()
        .findings
        .push(dctx0_tasks::Finding {
            id: "invalid".into(),
            severity: "unsupported".into(),
            description: "Must reject unknown severity.".into(),
            task_ids: vec![],
            action_required: false,
        });
    let before = f.project.task_session(None, None).unwrap();
    assert!(
        f.project
            .publish_task_attestations(Some(&f.app), &run.evidence.id, &invalid)
            .is_err()
    );
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(after.event_seq, before.event_seq);
    assert_eq!(after.states, before.states);
    let receipt = f
        .project
        .publish_task_attestations(Some(&f.app), &run.evidence.id, &reports)
        .unwrap();
    assert_eq!(receipt.deliveries.len(), plan.members.len());
    let session = f.project.task_session(None, None).unwrap();
    assert!(
        session
            .states
            .values()
            .all(|state| state.work == dctx0_tasks::WorkState::Done
                && state.check == dctx0_tasks::CheckStatus::Pass)
    );
    let review = plan
        .members
        .iter()
        .find(|member| member.candidate.kind == dctx0_tasks::TaskKind::Review)
        .unwrap();
    assert_eq!(session.states[&review.task_id].reviews.len(), 1);
    assert_eq!(
        f.project.saved_verification_run(&run.evidence.id).unwrap(),
        run.evidence
    );
    let store = dctx0_store::Store::open(&f.project.database, AccessMode::ReadOnly).unwrap();
    let reference = run
        .evidence
        .run
        .artifacts
        .iter()
        .find(|reference| reference.media_type == "application/vnd.dctx0.plan+json")
        .unwrap();
    let artifact = store
        .verification_artifact(&run.evidence.id, &reference.hash)
        .unwrap();
    let projected: dctx0_verification::ProgramPlan =
        serde_json::from_slice(&artifact.contents).unwrap();
    drop(store);
    let inspection = dctx0_verification::program_inspection::ProgramInspection {
        inspection: VerificationAttestation {
            schema_version:1,actor:"agent-reviewer".into(),subject:"W.Boundary".into(),snapshot:run.evidence.run.provenance.snapshot.clone(),plan_hash:run.evidence.run.provenance.plan_hash.clone(),
            requirements:dctx0_verification::program_inspection::inspection_requirements(&projected,"W.Boundary").unwrap(),implementation_artifacts:run.evidence.run.provenance.implementation.clone(),
            report:"Inspected app.ts exported integer/range checks and actual native input/rejection evidence; boundary criteria satisfied.".into(),limitations:vec!["Agent inspection, not exhaustive proof".into()],result:VerificationResult::Pass,
        },
        review:Some(dctx0_verification::program_inspection::InspectedReview { approved:true,findings:vec![] }),
    };
    let inspected = f
        .project
        .publish_program_inspections(
            Some(&f.app),
            &run.evidence.id,
            std::slice::from_ref(&inspection),
        )
        .unwrap();
    assert_eq!(inspected.assessment.result, VerificationResult::Pass);
    assert_eq!(
        inspected.evidence.run.origin,
        VerificationRunOrigin::Imported
    );
    assert_eq!(inspected.evidence.run.cases, run.evidence.run.cases);
    assert_eq!(inspected.evidence.run.result, VerificationResult::Pass);
    assert!(inspected.evidence.run.open_obligations.is_empty());
    assert_eq!(
        f.project.saved_verification_run(&run.evidence.id).unwrap(),
        run.evidence
    );
    assert_eq!(
        f.project
            .publish_program_inspections(
                Some(&f.app),
                &run.evidence.id,
                std::slice::from_ref(&inspection)
            )
            .unwrap()
            .evidence,
        inspected.evidence
    );
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(after.event_seq, session.event_seq);
    assert_eq!(after.states, session.states);
    fs::write(
        f.app.join("app.ts"),
        "export function factory(value:number){return value;}",
    )
    .unwrap();
    assert!(
        f.project
            .publish_program_inspections(Some(&f.app), &run.evidence.id, &[inspection])
            .is_err()
    );
}

#[tokio::test]
async fn replay_uses_saved_inputs_despite_changed_specs_fixtures_and_implementation_without_adoption()
 {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"),"export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    let original = f.run(true).await;
    let case = original
        .evidence
        .run
        .cases
        .iter()
        .find(|c| {
            c.bindings["value"] == VerificationValue::Integer("2".into())
                && c.bindings["expect_rejection"] == VerificationValue::Boolean(false)
        })
        .unwrap();
    fs::remove_file(f.app.join(".dctx0/testing/typescript/examples.json")).unwrap();
    fs::write(f.project.root.join("w.dctx0"), "invalid current spec").unwrap();
    let same = f
        .project
        .replay_verification(
            Some(&f.app),
            &original.evidence.id,
            &case.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(same.case_result, Some(VerificationResult::Pass));
    assert_eq!(
        same.evidence.evidence.run.provenance.snapshot,
        original.evidence.run.provenance.snapshot
    );
    assert_eq!(same.evidence.evidence.run.cases.len(), 1);
    assert_eq!(
        same.evidence.evidence.run.cases[0].bindings["value"],
        case.bindings["value"]
    );
    assert!(same.evidence.adopted_tasks.is_empty());
    assert!(same.evidence.adoption_issues.is_empty());
    assert_ne!(same.evidence.evidence.id, original.evidence.id);
    fs::write(
        f.app.join("app.ts"),
        "export function factory(_value:number){return 1;}",
    )
    .unwrap();
    let changed = f
        .project
        .replay_verification(
            Some(&f.app),
            &original.evidence.id,
            &case.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert!(
        matches!(changed.case_result,Some(VerificationResult::Fail{issue}) if issue.code=="NATIVE_VALUE_CHANGED")
    );
    assert!(
        changed
            .changes
            .iter()
            .any(|c| c.component == "implementation:app.ts")
    );
    assert_eq!(
        f.project
            .saved_verification_run(&original.evidence.id)
            .unwrap(),
        original.evidence
    );
    assert!(
        f.project
            .replay_verification(
                Some(&f.app),
                &original.evidence.id,
                "missing",
                &BunHarness,
                &NativeCancellation::default()
            )
            .await
            .is_err()
    );
}
#[tokio::test]
async fn scaffold_and_missing_factories_store_unknown_without_replacing_compiler_head() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    let history = f.project.history().unwrap();
    let scaffold = f.run(true).await;
    assert!(matches!(
        scaffold.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    assert_eq!(scaffold.evidence.run.cases.len(), 1);
    assert!(scaffold.evidence.run.cases[0].observations.is_empty());
    assert_eq!(f.project.history().unwrap().len(), history.len());
    f.adapter("export const adapter={async describe(){return {protocol_version:1,subjects:[],artifacts:[],capabilities:['events'],representations:{}};},async begin(){return {async invoke(){return {kind:'unavailable',reason:'unwired'};},async close(){}};}};");
    let missing = f.run(true).await;
    assert!(missing.evidence.run.cases.len() > 2);
    assert!(missing.evidence.run.cases.iter().all(|c| matches!(&c.result,VerificationResult::Unknown{issue} if issue.code=="NATIVE_VALUE_UNAVAILABLE")));
    fs::write(f.project.root.join("w.dctx0"), "invalid modified source").unwrap();
    assert!(
        f.project
            .run_verification(
                Some(&f.app),
                Default::default(),
                &BunHarness,
                &NativeCancellation::default(),
                true
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("run check first")
    );
    assert_eq!(
        f.project
            .saved_verification_run(&missing.evidence.id)
            .unwrap(),
        missing.evidence
    );
}

#[tokio::test]
async fn complete_cinema_plan_runs_real_pure_examples_and_lifecycle_cases_and_retains_open_requirements()
 {
    let f = Fixture::new("workspace W { type A = Int }", "W.A");
    let reference = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema");
    fs::remove_file(f.project.root.join("w.dctx0")).unwrap();
    for name in ["workspace.dctx0", "movie-night.dctx0"] {
        fs::copy(
            reference.join("dctx0").join(name),
            f.project.root.join(name),
        )
        .unwrap();
    }
    assert!(f.project.check().unwrap().ok);
    copy_project_fixture(&reference, &f.app);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let report = f.run(false).await;
    assert!(
        !report.adopted_tasks.is_empty(),
        "Complete task proofs must be adopted: {:?}",
        report.adoption_issues
    );
    assert!(
        report
            .adoption_issues
            .iter()
            .all(|issue| issue.code != "ADOPTION_PUBLICATION_REJECTED"),
        "{:?}",
        report.adoption_issues
    );
    let delivered = f.project.task_session(None, None).unwrap();
    for member in &delivered.plan.members {
        let state = &delivered.states[&member.task_id];
        if report.adopted_tasks.contains(&member.task_id) {
            assert!(matches!(
                member.candidate.kind,
                dctx0_tasks::TaskKind::Contract | dctx0_tasks::TaskKind::Operation
            ));
            assert_eq!(state.work, dctx0_tasks::WorkState::Done);
            assert_eq!(state.check, dctx0_tasks::CheckStatus::Pass);
        } else {
            assert_eq!(state.work, dctx0_tasks::WorkState::Open);
            assert_eq!(state.check, dctx0_tasks::CheckStatus::Unverified);
        }
    }
    eprintln!(
        "Cinema native adoption: {} of {} tasks delivered; remaining blockers: {:?}",
        report.adopted_tasks.len(),
        delivered.plan.members.len(),
        report.adoption_issues
    );
    let run = &report.evidence.run;
    let identities = run
        .cases
        .iter()
        .filter(|case| case.kind == VerificationCaseKind::EntityIdentity)
        .collect::<Vec<_>>();
    assert_eq!(identities.len(), 5);
    assert!(
        identities
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    assert!(
        report
            .adoption_issues
            .iter()
            .all(|issue| !issue.message.contains("#key:"))
    );
    let contracts = run
        .cases
        .iter()
        .filter(|case| case.kind == VerificationCaseKind::Contract)
        .collect::<Vec<_>>();
    assert_eq!(
        contracts
            .iter()
            .map(|case| &case.subject)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        46
    );
    assert!(
        contracts
            .iter()
            .all(|case| case.result == VerificationResult::Pass),
        "Every complete Cinema data contract must pass against real validators"
    );
    assert!(
        run.provenance.incomplete.is_empty(),
        "Actual captured Cinema dependency provenance must be complete: {:?}",
        run.provenance.incomplete
    );
    assert!(run.artifacts.iter().any(
        |artifact| artifact.media_type == "application/vnd.dctx0.module-observer-evidence+json"
    ));
    assert_eq!(
        run.cases.len(),
        run.required_cases.len(),
        "Every mandatory Cinema case must produce a record"
    );
    assert_eq!(
        run.cases
            .iter()
            .map(|case| case.id.clone())
            .collect::<std::collections::BTreeSet<_>>(),
        run.required_cases
    );
    assert!(
        matches!(&run.result, VerificationResult::Unknown { issue } if issue.code == "MANUAL_REQUIREMENT"),
        "{:?}",
        run.result
    );
    assert!(
        run.cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass),
        "All complete Cinema mandatory cases must pass: {:?}",
        run.cases
            .iter()
            .filter(|case| case.result != VerificationResult::Pass)
            .collect::<Vec<_>>()
    );
    assert!(run.open_obligations.iter().all(|issue| matches!(
        issue.code.as_str(),
        "MANUAL_REQUIREMENT" | "SUITE_INSPECTION_REQUIRED"
    )));
    let comments = run
        .cases
        .iter()
        .filter(|case| case.subject == "HomeCinema.MovieNight.AddComment")
        .collect::<Vec<_>>();
    assert!(!comments.is_empty());
    assert!(
        comments
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    let empty_comments = comments.iter().filter(|case| {
        matches!(&case.bindings["value"], VerificationValue::Record(fields) if fields.get("text") == Some(&VerificationValue::String(String::new())))
            && case.bindings["expect_rejection"] == VerificationValue::Boolean(false)
    }).count();
    assert_eq!(
        empty_comments, 0,
        "Visible-text refinement excludes empty text from valid comments"
    );
    let examples = run
        .cases
        .iter()
        .filter(|c| c.kind == VerificationCaseKind::Example)
        .collect::<Vec<_>>();
    assert_eq!(examples.len(), 12);
    assert!(
        examples
            .iter()
            .all(|c| c.result == VerificationResult::Pass),
        "{examples:?}"
    );
    let lifecycles = run
        .cases
        .iter()
        .filter(|c| c.kind == VerificationCaseKind::Lifecycle)
        .collect::<Vec<_>>();
    assert_eq!(lifecycles.len(), 2);
    assert!(
        lifecycles
            .iter()
            .all(|c| c.result == VerificationResult::Pass),
        "{lifecycles:?}"
    );
    assert!(
        lifecycles
            .iter()
            .all(|c| !c.setup.is_empty() && !c.states.is_empty())
    );
    let assertions = run
        .cases
        .iter()
        .filter(|c| c.kind == VerificationCaseKind::Assertion)
        .collect::<Vec<_>>();
    assert_eq!(assertions.len(), 101);
    assert!(
        assertions
            .iter()
            .all(|c| c.result == VerificationResult::Pass),
        "{assertions:?}"
    );
    let operations = run
        .cases
        .iter()
        .filter(|c| c.kind == VerificationCaseKind::Operation && c.id.contains(":operation:"))
        .collect::<Vec<_>>();
    assert!(operations.len() >= 100);
    let pure_operations = operations
        .iter()
        .filter(|case| case.subject.ends_with("VotingRules.candidateOrderWinner"))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(pure_operations.len(), 100);
    let reviewed = run
        .cases
        .iter()
        .filter(|c| c.kind == VerificationCaseKind::Operation && c.id.contains(":contract:"))
        .collect::<Vec<_>>();
    assert_eq!(reviewed.len(), 42);
    assert_eq!(
        reviewed
            .iter()
            .map(|case| case.subject.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        25
    );
    let original_reviewed = reviewed
        .iter()
        .filter(|case| !case.id.ends_with(":real-entry") && !case.id.contains(":http-"))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(original_reviewed.len(), 6);
    assert!(
        original_reviewed
            .iter()
            .all(|case| case.result == VerificationResult::Pass),
        "{original_reviewed:?}"
    );
    assert!(
        reviewed
            .iter()
            .all(|c| !c.states.is_empty() && !c.observations.is_empty())
    );
    assert!(
        pure_operations
            .iter()
            .all(|c| c.result == VerificationResult::Pass)
    );
    assert!(
        run.cases
            .iter()
            .any(|c| c.subject == "HomeCinema.MovieNight.ReleaseYear"
                && c.result == VerificationResult::Pass)
    );
    assert!(run.cases.iter().any(
        |c| c.subject == "HomeCinema.MovieNight.FilmId" && c.result == VerificationResult::Pass
    ));
    assert!(
        run.open_obligations
            .iter()
            .all(|i| i.code != "OPERATION_CLOSURE_INCOMPLETE")
    );
    let before_replay = f
        .project
        .task_session(None, None)
        .ok()
        .map(|session| session.event_seq);
    let mut replay_cases = Vec::new();
    for kind in [
        VerificationCaseKind::Assertion,
        VerificationCaseKind::Example,
        VerificationCaseKind::Lifecycle,
        VerificationCaseKind::EntityIdentity,
    ] {
        replay_cases.push(run.cases.iter().find(|case| case.kind == kind).unwrap());
    }
    replay_cases.push(
        run.cases
            .iter()
            .find(|case| case.id.ends_with("FilmCatalogStore.save:contract:insert"))
            .unwrap(),
    );
    replay_cases.push(
        run.cases
            .iter()
            .find(|case| {
                case.subject.ends_with("FilmCatalogStore.find") && case.id.contains(":operation:")
            })
            .unwrap(),
    );
    for case in replay_cases {
        let replayed = f
            .project
            .replay_verification(
                Some(&f.app),
                &report.evidence.id,
                &case.id,
                &dctx0_target_typescript::BunHarness,
                &NativeCancellation::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            replayed.case_result.as_ref(),
            Some(&case.result),
            "Replay {}",
            case.id
        );
        if case.kind == VerificationCaseKind::EntityIdentity {
            assert!(replayed.evidence.evidence.run.cases.len() > 1);
        } else {
            assert_eq!(replayed.evidence.evidence.run.cases.len(), 1);
        }
        assert!(replayed.evidence.adopted_tasks.is_empty());
    }
    assert_eq!(
        f.project
            .task_session(None, None)
            .ok()
            .map(|session| session.event_seq),
        before_replay
    );

    let http_cases = run
        .cases
        .iter()
        .filter(|case| case.id.contains(":http-"))
        .collect::<Vec<_>>();
    assert_eq!(http_cases.len(), 14);
    assert!(
        http_cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    let http_case = http_cases
        .iter()
        .find(|case| case.subject.ends_with("FilmCatalogApi.searchFilms"))
        .unwrap();
    let http_source = f.app.join("src/http/app.ts");
    let original_http = fs::read_to_string(&http_source).unwrap();
    fs::write(
        &http_source,
        original_http.replace("app.get('/films',", "app.get('/broken-films',"),
    )
    .unwrap();
    let mutated = f
        .project
        .replay_verification(
            Some(&f.app),
            &report.evidence.id,
            &http_case.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        mutated.case_result,
        Some(VerificationResult::Fail { .. })
    ));
    assert!(mutated.evidence.evidence.run.cases.iter().flat_map(|case| &case.observations).any(|observation| {
        observation.invocation.subject == http_case.subject && matches!(&observation.observation, Observation::Threw { message } if message.contains("HTTP response 404"))
    }));
    assert!(mutated.evidence.adopted_tasks.is_empty());
    fs::write(&http_source, original_http).unwrap();
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before_replay.unwrap()
    );

    assert!(!run.provenance.dependencies.is_empty());
    assert_eq!(
        f.project
            .saved_verification_run(&report.evidence.id)
            .unwrap(),
        report.evidence
    );
    let output =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-executor-conformance");
    fs::create_dir_all(&output).unwrap();
    fs::write(
        output.join(format!("{}-cinema-whole-plan.json", std::process::id())),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn changes_to_original_application_during_captured_execution_preserve_evidence_and_prevent_adoption()
 {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    let production = "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}";
    fs::write(f.app.join("app.ts"), production).unwrap();
    let original = f.app.join("app.ts");
    let adapter = VALUE_ADAPTER.replace("async begin(){return", &format!("async begin(){{await Bun.write({:?}, 'export function factory(){{return 999;}}');return", original.to_str().unwrap()));
    f.adapter(&adapter);
    let report = f.run(false).await;
    // Actual factory calls still execute the immutable captured production code.
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    assert!(
        report
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|i| i.code == "APPLICATION_INPUTS_CHANGED")
    );
    assert!(report.adopted_tasks.is_empty());
    assert_eq!(
        fs::read_to_string(&original).unwrap(),
        "export function factory(){return 999;}"
    );
    assert_eq!(
        report.evidence.run.provenance.implementation["app.ts"],
        dctx0_store::VerificationArtifact::new("text/typescript", production.as_bytes().to_vec())
            .reference
            .hash
    );
    assert_eq!(
        f.project
            .saved_verification_run(&report.evidence.id)
            .unwrap(),
        report.evidence
    );
}

#[tokio::test]
async fn centrally_selected_preconditions_run_actual_operations_only_for_eligible_generated_inputs()
{
    let f = Fixture::new(
        "workspace W { value A = Int { range 0..2 } surface Api { operation next(a:A) -> Int {\n pure\n requires a > 0\n ensures result == a + 1\n } } }",
        "W.Api.next",
    );
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<0||value>2)throw TypeError('invalid');return value;} export function nextValue(value:number){if(value<=0)throw Error('precondition violated');return value+1;}").unwrap();
    let adapter = VALUE_ADAPTER.replace("import {factory}", "import {factory,nextValue}")
        .replace("capabilities:['sessions','values','events']", "capabilities:['sessions','values','events','invocation']")
        .replace("export const adapter=", "import {invokeApplication} from './contract.ts'; export const adapter=")
        .replace("async invoke(){throw Error('No operation');}", "async invoke(invocation){return invokeApplication(()=>nextValue(Number(invocation.arguments[0].value)),value=>({kind:'integer',value:String(value)}));}");
    f.adapter(&adapter);
    let report = f.run(true).await;
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    let operations = report
        .evidence
        .run
        .cases
        .iter()
        .filter(|c| c.kind == VerificationCaseKind::Operation)
        .collect::<Vec<_>>();
    assert_eq!(operations.len(), 2);
    assert_eq!(
        operations
            .iter()
            .map(|c| c.arguments[0][0].clone())
            .collect::<Vec<_>>(),
        [
            VerificationValue::Integer("1".into()),
            VerificationValue::Integer("2".into())
        ]
    );
    for case in operations {
        assert_eq!(case.observations.len(), 1);
        assert_eq!(case.observations[0].invocation.subject, "W.Api.next");
        assert_eq!(case.result, VerificationResult::Pass);
    }
}

#[tokio::test]
async fn failing_real_operation_is_shrunk_and_minimized_input_replays_against_new_code() {
    let f = Fixture::new(
        "workspace W { value A = Int { range 3..10 } surface Api { operation next(a:A) -> Int {\n pure\n ensures result == a + 1\n } } }",
        "W.Api.next",
    );
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<3||value>10)throw TypeError('invalid');return value;} export function nextValue(value:number){return value+2;}").unwrap();
    let adapter = VALUE_ADAPTER.replace("import {factory}", "import {factory,nextValue}")
        .replace("capabilities:['sessions','values','events']", "capabilities:['sessions','values','events','invocation']")
        .replace("export const adapter=", "import {invokeApplication} from './contract.ts'; export const adapter=")
        .replace("async invoke(){throw Error('No operation');}", "async invoke(invocation){return invokeApplication(()=>nextValue(Number(invocation.arguments[0].value)),value=>({kind:'integer',value:String(value)}));}");
    f.adapter(&adapter);
    let report = f.run(true).await;
    assert!(matches!(
        report.evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    let case = report
        .evidence
        .run
        .cases
        .iter()
        .find(|c| {
            c.kind == VerificationCaseKind::Operation
                && c.arguments == vec![vec![VerificationValue::Integer("10".into())]]
        })
        .unwrap();
    let replay = f
        .project
        .replay_minimized_verification(
            Some(&f.app),
            &report.evidence.id,
            &case.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert!(replay.minimized_input);
    assert!(matches!(
        replay.case_result,
        Some(VerificationResult::Fail { .. })
    ));
    assert_eq!(
        replay.evidence.evidence.run.cases[0].arguments,
        vec![vec![VerificationValue::Integer("5".into())]]
    );
    assert!(replay.evidence.adopted_tasks.is_empty());
    // Version 1 recorded candidates remain executable inputs, even though new
    // runs use the bounded version 2 candidate generator.
    let mut store = dctx0_store::Store::open(
        &f.root.join("spec/.dctx0ctl/state.sqlite3"),
        AccessMode::ReadWrite,
    )
    .unwrap();
    let mut artifacts = Vec::new();
    for reference in &report.evidence.run.artifacts {
        let artifact = store
            .verification_artifact(&report.evidence.id, &reference.hash)
            .unwrap();
        if reference.media_type == "application/vnd.dctx0.shrink+json" {
            let mut value: serde_json::Value = serde_json::from_slice(&artifact.contents).unwrap();
            value["report"]["version"] = serde_json::json!("dctx0-shrink-1");
            artifacts.push(dctx0_store::VerificationArtifact::new(
                &reference.media_type,
                serde_json::to_vec(&value).unwrap(),
            ));
        } else {
            artifacts.push(artifact);
        }
    }
    let mut legacy = report.evidence.run.clone();
    legacy.origin = VerificationRunOrigin::Imported;
    legacy.artifacts = artifacts
        .iter()
        .map(|artifact| artifact.reference.clone())
        .collect();
    let legacy = store
        .record_verification_run(&f.project.key(), &legacy, &artifacts)
        .unwrap();
    for corruption in [
        "missing-shrink",
        "wrong-failure",
        "unknown-version",
        "ambiguous-shrink",
        "wrong-plan",
    ] {
        let mut altered = Vec::new();
        for artifact in &artifacts {
            if artifact.reference.media_type == "application/vnd.dctx0.shrink+json" {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&artifact.contents).unwrap();
                if value["case_id"].as_str() == Some(case.id.as_str()) {
                    match corruption {
                        "missing-shrink" => continue,
                        "wrong-failure" => {
                            for attempt in value["report"]["attempts"].as_array_mut().unwrap() {
                                attempt["result"] = serde_json::json!({"status":"PASS"});
                            }
                        }
                        "unknown-version" => {
                            value["report"]["version"] =
                                serde_json::json!("unsupported-shrink-version")
                        }
                        "ambiguous-shrink" => {
                            altered.push(artifact.clone());
                            value["duplicate_wrapper"] = serde_json::json!(true);
                        }
                        _ => {}
                    }
                    altered.push(dctx0_store::VerificationArtifact::new(
                        &artifact.reference.media_type,
                        serde_json::to_vec(&value).unwrap(),
                    ));
                    continue;
                }
            }
            if corruption == "wrong-plan"
                && artifact.reference.media_type == "application/vnd.dctx0.plan+json"
            {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&artifact.contents).unwrap();
                value["check_occurrence"] =
                    serde_json::json!(report.evidence.run.provenance.check_occurrence + 1);
                altered.push(dctx0_store::VerificationArtifact::new(
                    &artifact.reference.media_type,
                    serde_json::to_vec(&value).unwrap(),
                ));
            } else {
                altered.push(artifact.clone());
            }
        }
        // Imported copies exercise reader rejection; they claim no native authority.
        let mut invalid = legacy.run.clone();
        invalid.artifacts = altered
            .iter()
            .map(|artifact| artifact.reference.clone())
            .collect();
        invalid.provenance.plan_hash = altered
            .iter()
            .find(|artifact| artifact.reference.media_type == "application/vnd.dctx0.plan+json")
            .unwrap()
            .reference
            .hash
            .clone();
        let invalid = store
            .record_verification_run(&f.project.key(), &invalid, &altered)
            .unwrap();
        let error = f
            .project
            .replay_minimized_verification(
                Some(&f.app),
                &invalid.id,
                &case.id,
                &BunHarness,
                &NativeCancellation::default(),
            )
            .await
            .unwrap_err()
            .to_string();
        let expected = match corruption {
            "missing-shrink" => "No saved shrink artifact",
            "ambiguous-shrink" => "Ambiguous saved shrink artifacts",
            "wrong-plan" => "plan provenance does not match",
            _ => "does not match the recorded failing operation",
        };
        assert!(error.contains(expected), "{corruption}: {error}");
        assert_eq!(
            f.project.saved_verification_run(&invalid.id).unwrap(),
            invalid
        );
    }
    drop(store);
    let contract = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.kind == VerificationCaseKind::Contract)
        .unwrap();
    let error = f
        .project
        .replay_minimized_verification(
            Some(&f.app),
            &report.evidence.id,
            &contract.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("pure generated operation case"), "{error}");
    assert_eq!(
        f.project
            .saved_verification_run(&report.evidence.id)
            .unwrap(),
        report.evidence
    );
    let old = f
        .project
        .replay_minimized_verification(
            Some(&f.app),
            &legacy.id,
            &case.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        old.case_result,
        Some(VerificationResult::Fail { .. })
    ));
    assert_eq!(
        old.evidence.evidence.run.cases[0].arguments,
        replay.evidence.evidence.run.cases[0].arguments
    );
    assert!(old.evidence.adopted_tasks.is_empty());
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<3||value>10)throw TypeError('invalid');return value;} export function nextValue(value:number){return value+1;}").unwrap();
    let fixed = f
        .project
        .replay_minimized_verification(
            Some(&f.app),
            &report.evidence.id,
            &case.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(fixed.case_result, Some(VerificationResult::Pass));
    assert!(
        fixed
            .changes
            .iter()
            .any(|change| change.component == "implementation:app.ts")
    );
}

#[tokio::test]
async fn subject_selection_pins_existing_matching_tasks_without_changing_work_state() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    let snapshot = f.project.load("latest").unwrap().id;
    let tasks = f.project.task_plan(&snapshot).unwrap();
    let task = tasks
        .members
        .iter()
        .find(|member| member.candidate.subjects == ["W.A"])
        .unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let report = f.run(true).await;
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    let pin = report.evidence.run.provenance.task_pin.as_ref().unwrap();
    assert_eq!(pin.plan_id, tasks.id);
    assert_eq!(pin.event_sequence, before.event_seq);
    assert_eq!(
        pin.requirements.get(&task.task_id),
        Some(&task.candidate.requirements_hash)
    );
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    assert!(report.adopted_tasks.is_empty());
}

#[tokio::test]
async fn operation_mock_cases_keep_the_consumer_real_and_detect_changed_call_arguments() {
    let f = Fixture::new(
        "workspace W { port Store { operation load(id:Int) -> Int } surface Api { operation next(id:Int) -> Int {\n call Store.load\n ensures result == 3\n } } }",
        "W.Api.next",
    );
    fs::write(f.app.join("app.ts"), "export async function nextValue(id:number,load:(id:number)=>Promise<number>){return (await load(id))+1;}").unwrap();
    f.adapter(r#"import {nextValue} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'nextValue'}],artifacts:['app.ts'],capabilities:['sessions','events','invocation','mock-injection','call-observation'],representations:{Int:'safe integer'}};},async begin(caseId,context){return {async invoke(invocation){const value=await nextValue(Number(invocation.arguments[0].value),async id=>{const actual={subject:'W.Store.load',arguments:[{kind:'integer',value:String(id)}]};const observed=await context.collaborator(actual);context.recordCall(actual,observed);if(observed.kind!=='returned')throw Error('No collaborator result');return Number(observed.value.value);});return {kind:'returned',value:{kind:'integer',value:String(value)}};},async close(){}};}};"#);
    let integer = |value: &str| VerificationValue::Integer(value.into());
    let input = dctx0_verification::OperationCaseInput {
        id: "reviewed-consumer".into(),
        invocation: Invocation {
            subject: "W.Api.next".into(),
            arguments: vec![integer("1")],
        },
        setup: vec![],
        mocks: vec![dctx0_verification::OperationMock {
            invocation: Invocation {
                subject: "W.Store.load".into(),
                arguments: vec![integer("1")],
            },
            outcome: Observation::Returned {
                value: integer("2"),
            },
        }],
    };
    let mut fixtures = dctx0_verification::ProjectionFixtures::default();
    fixtures.operations.insert("W.Api.next".into(), vec![input]);
    fs::write(
        f.app.join(".dctx0/testing/typescript/examples.json"),
        serde_json::to_vec(&fixtures).unwrap(),
    )
    .unwrap();
    let report = f.run(true).await;
    let case = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "reviewed-consumer")
        .unwrap();
    assert_eq!(case.result, VerificationResult::Pass);
    assert_eq!(case.observations[0].invocation.subject, "W.Api.next");
    let honest = fs::read_to_string(f.app.join(".dctx0/testing/typescript/adapter.ts")).unwrap();
    f.adapter(r#"import {nextValue} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.next',role:'operation',artifact:'app.ts',entry_point:'nextValue'}],artifacts:['app.ts'],capabilities:['sessions','events','invocation','mock-injection','call-observation'],representations:{Int:'safe integer'}};},async begin(caseId,context){return {async invoke(){context.recordCall({subject:'W.Store.load',arguments:[{kind:'integer',value:'1'}]},{kind:'returned',value:{kind:'integer',value:'2'}});return {kind:'returned',value:{kind:'integer',value:'3'}};},async close(){}};}};"#);
    let fabricated = f.run(true).await;
    let fabricated = fabricated
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "reviewed-consumer")
        .unwrap();
    assert!(
        matches!(&fabricated.result, VerificationResult::Fail { issue } if issue.code == "MOCK_OBSERVATION_MISMATCH")
    );
    f.adapter(&honest);
    fs::write(f.app.join("app.ts"), "export async function nextValue(id:number,load:(id:number)=>Promise<number>){return (await load(id+1))+1;}").unwrap();
    let mutation = f.run(true).await;
    let case = mutation
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "reviewed-consumer")
        .unwrap();
    assert!(
        matches!(&case.result, VerificationResult::Fail { issue } if issue.code == "UNEXPECTED_MOCK_CALL")
    );
    // The real consumer may recover from an unavailable collaborator. Its
    // apparently correct return cannot erase the centrally recorded wrong call.
    fs::write(f.app.join("app.ts"), "export async function nextValue(id:number,load:(id:number)=>Promise<number>){try{return (await load(id+1))+1;}catch{return 3;}}").unwrap();
    let recovered = f.run(true).await;
    let case = recovered
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "reviewed-consumer")
        .unwrap();
    assert!(
        matches!(&case.result, VerificationResult::Fail { issue } if issue.code == "UNEXPECTED_MOCK_CALL")
    );
    assert!(
        case.observations
            .iter()
            .any(|observed| observed.invocation.subject == "W.Api.next"
                && observed.observation
                    == Observation::Returned {
                        value: VerificationValue::Integer("3".into())
                    })
    );
    assert!(recovered.adopted_tasks.is_empty());
    assert_eq!(
        f.project
            .saved_verification_run(&recovered.evidence.id)
            .unwrap(),
        recovered.evidence
    );
}

#[tokio::test]
async fn failing_lifecycle_shrinks_real_session_and_replays_after_production_repair() {
    let f = Fixture::new(
        r#"workspace W {
 type Status = Open | Closed
 entity Item { id: String
 status: Status
 key id }
 surface Api { operation prepare(id:String) -> Int {
 requires true
 state lists Item
 }
 operation finish(id:String, note:String) -> Int }
 lifecycle Flow for Item using status {
 state Open initial
 state Closed terminal
 transition Api.finish from Open to Closed when true
 }
 }"#,
        "W.Flow",
    );
    fs::write(
        f.app.join("app.ts"),
        "export function prepare(item:any,id:string){item.id=id;item.status='Open';item.present=true;return 1;} export function finish(item:any,note:string){item.status='Open';return note.length;}",
    )
    .unwrap();
    f.adapter(r#"import {prepare,finish} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.prepare',role:'operation',artifact:'app.ts',entry_point:'prepare'},{subject:'W.Api.finish',role:'operation',artifact:'app.ts',entry_point:'finish'}],artifacts:['app.ts'],capabilities:['sessions','invocation','state','events'],representations:{String:'string',Int:'safe integer'}};},async begin(){const item={id:'',status:'Open',present:false};return {async invoke(i){return {kind:'returned',value:{kind:'integer',value:String(i.subject==='W.Api.prepare'?prepare(item,i.arguments[0].value):finish(item,i.arguments[1].value))}};},async observe(){if(!item.present)return {'W.Item':[]};return {'W.Item':[{kind:'record',value:{id:{kind:'string',value:item.id},status:{kind:'variant',value:{declaration:'W.Status',name:item.status,payload:null}}}}]};},async close(){}};}};"#);
    let plan = f
        .project
        .verification_plan(
            VerificationSelection {
                subjects: ["W.Flow".into()].into(),
                ..Default::default()
            },
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    let path = plan.lifecycles[0].paths[0].id.clone();
    let input = dctx0_verification::LifecycleCaseInput {
        keys: vec![VerificationValue::String("chosen".into())],
        setup: vec![Invocation {
            subject: "W.Api.prepare".into(),
            arguments: vec![VerificationValue::String("chosen".into())],
        }],
        arguments: vec![vec![
            VerificationValue::String("chosen".into()),
            VerificationValue::String("long note".into()),
        ]],
    };
    fs::write(
        f.app.join(".dctx0/testing/typescript/lifecycles.json"),
        serde_json::to_vec(&VerificationLifecycleFixtures {
            paths: [(path.clone(), input)].into(),
        })
        .unwrap(),
    )
    .unwrap();
    let report = f.run(true).await;
    let recorded = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == path)
        .unwrap();
    assert_eq!(
        recorded
            .states
            .iter()
            .map(|state| state.phase)
            .collect::<Vec<_>>(),
        vec![StatePhase::Before, StatePhase::Before, StatePhase::After]
    );
    assert!(
        recorded
            .states
            .iter()
            .all(|state| state.phase_recorded == Some(true))
    );
    assert!(recorded.states[0].entities["W.Item"].is_empty());
    assert_eq!(recorded.states[1].entities["W.Item"].len(), 1);

    assert!(
        !report
            .evidence
            .run
            .required_subjects
            .contains("W.Api.prepare")
    );
    assert!(
        !report
            .evidence
            .run
            .cases
            .iter()
            .any(|case| case.subject == "W.Api.prepare")
    );
    assert!(
        matches!(
            report
                .evidence
                .run
                .cases
                .iter()
                .find(|c| c.id == path)
                .unwrap()
                .result,
            VerificationResult::Fail { .. }
        ),
        "{:#?}",
        report.evidence.run
    );
    // Imported copies exercise the replay reader's rejection boundary. They
    // retain real original observations but have no native adoption authority.
    let mut store =
        dctx0_store::Store::open(&f.project.database, dctx0_store::AccessMode::ReadWrite).unwrap();
    let original_artifacts = report
        .evidence
        .run
        .artifacts
        .iter()
        .map(|reference| {
            store
                .verification_artifact(&report.evidence.id, &reference.hash)
                .unwrap()
        })
        .collect::<Vec<_>>();
    for corruption in [
        "missing",
        "duplicate",
        "version",
        "keys",
        "path-length",
        "original-pass",
        "attempt-pass",
        "original-input",
        "failure-code",
    ] {
        let mut artifacts = vec![];
        for artifact in &original_artifacts {
            if artifact.reference.media_type != "application/vnd.dctx0.lifecycle-shrink+json" {
                artifacts.push(artifact.clone());
                continue;
            }
            let mut value: serde_json::Value = serde_json::from_slice(&artifact.contents).unwrap();
            if value["case_id"].as_str() != Some(path.as_str()) {
                artifacts.push(artifact.clone());
                continue;
            }
            match corruption {
                "missing" => continue,
                "duplicate" => {
                    artifacts.push(artifact.clone());
                    value["duplicate_wrapper"] = serde_json::json!(true);
                }
                "version" => value["report"]["version"] = serde_json::json!("unsupported-version"),
                "keys" => value["report"]["minimized"]["keys"] = serde_json::json!([]),
                "path-length" => value["report"]["minimized"]["arguments"] = serde_json::json!([]),
                "original-pass" => {
                    value["report"]["original_assessment"]["result"] =
                        serde_json::json!({"status":"PASS"})
                }
                "original-input" => value["report"]["original"]["setup"] = serde_json::json!([]),
                "failure-code" => {
                    value["report"]["original_assessment"]["result"]["issue"]["code"] =
                        serde_json::json!("DIFFERENT_FAILURE")
                }
                _ => {
                    for attempt in value["report"]["attempts"].as_array_mut().unwrap() {
                        attempt["assessment"]["result"] = serde_json::json!({"status":"PASS"});
                    }
                }
            }
            artifacts.push(dctx0_store::VerificationArtifact::new(
                &artifact.reference.media_type,
                serde_json::to_vec(&value).unwrap(),
            ));
        }
        let mut imported = report.evidence.run.clone();
        imported.origin = VerificationRunOrigin::Imported;
        imported.artifacts = artifacts
            .iter()
            .map(|artifact| artifact.reference.clone())
            .collect();
        let imported = store
            .record_verification_run(&f.project.key(), &imported, &artifacts)
            .unwrap();
        let rejected = f
            .project
            .replay_minimized_verification(
                Some(&f.app),
                &imported.id,
                &path,
                &BunHarness,
                &NativeCancellation::default(),
            )
            .await
            .unwrap_err();
        assert!(
            rejected.to_string().to_lowercase().contains("shrink"),
            "{corruption}: {rejected}"
        );
        assert_eq!(
            f.project.saved_verification_run(&imported.id).unwrap(),
            imported
        );
    }
    drop(store);
    assert_eq!(
        f.project
            .saved_verification_run(&report.evidence.id)
            .unwrap(),
        report.evidence
    );
    let replay = f
        .project
        .replay_minimized_verification(
            Some(&f.app),
            &report.evidence.id,
            &path,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        replay.case_result,
        Some(VerificationResult::Fail { .. })
    ));
    assert_eq!(
        replay.evidence.evidence.run.cases[0].arguments[0][1],
        VerificationValue::String(String::new())
    );
    assert_eq!(replay.evidence.evidence.run.cases[0].setup.len(), 1);
    assert!(replay.evidence.adopted_tasks.is_empty());
    fs::write(
        f.app.join("app.ts"),
        "export function prepare(item:any,id:string){item.id=id;item.status='Open';item.present=true;return 1;} export function finish(item:any,note:string){item.status='Closed';return note.length;}",
    )
    .unwrap();
    let repaired = f
        .project
        .replay_minimized_verification(
            Some(&f.app),
            &report.evidence.id,
            &path,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(repaired.case_result, Some(VerificationResult::Pass));
}

#[tokio::test]
async fn self_modified_captured_sources_preserve_evidence_but_cannot_adopt_tasks() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    let source = "import {writeFileSync} from 'node:fs'; export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');writeFileSync(new URL(import.meta.url),'export function factory(){return 999;}');return value;}";
    fs::write(f.app.join("app.ts"), source).unwrap();
    f.adapter(VALUE_ADAPTER);
    dctx0_engine::create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let report = f.run(false).await;
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    assert!(
        report
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "CAPTURED_INPUTS_CHANGED")
    );
    assert!(report.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    assert_eq!(fs::read_to_string(f.app.join("app.ts")).unwrap(), source);
    assert_eq!(
        report.evidence.run.provenance.implementation["app.ts"],
        dctx0_store::VerificationArtifact::new("text/typescript", source.as_bytes().to_vec())
            .reference
            .hash
    );
}

#[tokio::test]
async fn replay_reports_changed_or_missing_assessment_versions_without_rewriting_history() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    fs::write(f.app.join("app.ts"),"export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    f.project.task_plan("latest").unwrap();
    let original = f.run(true).await;
    let before = f.project.task_session(None, None).unwrap().event_seq;
    let case = original.evidence.run.cases[0].id.clone();
    for (media, component, legacy) in [
        (
            "application/vnd.dctx0.assessment-version+json",
            "assessment-version",
            Some("legacy-assessment"),
        ),
        (
            "application/vnd.dctx0.assessment-version+json",
            "assessment-version",
            None,
        ),
        (
            "application/vnd.dctx0.verifier-identity+json",
            "verifier-identity",
            Some("legacy-executable"),
        ),
        (
            "application/vnd.dctx0.verifier-identity+json",
            "verifier-identity",
            None,
        ),
        (
            "application/vnd.dctx0.runtime-identity+json",
            "runtime-identity",
            None,
        ),
        (
            "application/vnd.dctx0.native-limits+json",
            "native-limits",
            None,
        ),
    ] {
        let mut store = dctx0_store::Store::open(
            &f.root.join("spec/.dctx0ctl/state.sqlite3"),
            dctx0_store::AccessMode::ReadWrite,
        )
        .unwrap();
        let mut artifacts = Vec::new();
        for reference in &original.evidence.run.artifacts {
            if reference.media_type == media {
                if let Some(version) = legacy {
                    artifacts.push(dctx0_store::VerificationArtifact::new(
                        &reference.media_type,
                        serde_json::to_vec(&if component == "assessment-version" {serde_json::json!({"schema_version":1,"version":version})} else {serde_json::json!({"schema_version":1,"executable_hash":"0".repeat(64),"executable_bytes":1})})
                        .unwrap(),
                    ));
                }
            } else {
                artifacts.push(
                    store
                        .verification_artifact(&original.evidence.id, &reference.hash)
                        .unwrap(),
                );
            }
        }
        let mut imported = original.evidence.run.clone();
        imported.origin = VerificationRunOrigin::Imported;
        imported.artifacts = artifacts.iter().map(|a| a.reference.clone()).collect();
        let saved = store
            .record_verification_run(&f.project.key(), &imported, &artifacts)
            .unwrap();
        drop(store);
        let replay = f
            .project
            .replay_verification(
                Some(&f.app),
                &saved.id,
                &case,
                &BunHarness,
                &NativeCancellation::default(),
            )
            .await
            .unwrap();
        assert_eq!(replay.case_result, Some(VerificationResult::Pass));
        assert!(
            replay
                .changes
                .iter()
                .any(|change| change.component == component
                    && change.before.is_some() == legacy.is_some()
                    && change.after.is_some())
        );
        assert!(replay.evidence.adopted_tasks.is_empty());
        assert_eq!(f.project.saved_verification_run(&saved.id).unwrap(), saved);
    }
    assert_eq!(
        f.project
            .saved_verification_run(&original.evidence.id)
            .unwrap(),
        original.evidence
    );
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before
    );
}

#[tokio::test]
async fn exhausted_native_dependency_budget_cannot_adopt_otherwise_passing_contracts() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W.A");
    let source = "import './helper.ts';\n".repeat(1001)
        + "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}";
    fs::write(f.app.join("app.ts"), source).unwrap();
    fs::write(f.app.join("helper.ts"), "export const helper = 1;").unwrap();
    f.adapter(VALUE_ADAPTER);
    dctx0_engine::create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let run = f.run(false).await;
    assert!(
        run.evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "DEPENDENCY_RESOLUTION_BUDGET")
    );
    assert_eq!(run.evidence.run.result, VerificationResult::Pass);
    assert!(run.adopted_tasks.is_empty());
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(after.event_seq, before.event_seq);
    assert!(
        after
            .states
            .values()
            .all(|state| state.work == dctx0_tasks::WorkState::Open)
    );
}

#[tokio::test]
async fn complete_task_evidence_adopts_with_unrelated_manual_review_still_open() {
    let f = Fixture::new(
        "workspace W {\nvalue A = Int { range 1..2 }\nreview NeedsHuman { inspect { implementation } criteria { \"Inspect independently of the numeric contract.\" } }\n}",
        "W",
    );
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    f.adapter(VALUE_ADAPTER);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let plan = f.project.task_plan("latest").unwrap();
    let id = plan
        .members
        .iter()
        .find(|member| member.candidate.subjects == ["W.A"])
        .unwrap()
        .task_id;
    let report = f.run(false).await;
    assert!(
        matches!(&report.evidence.run.result, VerificationResult::Unknown { issue } if issue.code == "MANUAL_REQUIREMENT")
    );
    assert!(
        report
            .evidence
            .run
            .cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    assert_eq!(
        report.adopted_tasks,
        vec![id],
        "{:?}",
        report.adoption_issues
    );
    let delivered = f.project.task_session(None, None).unwrap();
    assert_eq!(delivered.states[&id].work, dctx0_tasks::WorkState::Done);
    assert_eq!(delivered.states[&id].check, dctx0_tasks::CheckStatus::Pass);
    assert!(
        delivered
            .states
            .iter()
            .filter(|(other, _)| **other != id)
            .all(|(_, state)| state.work == dctx0_tasks::WorkState::Open)
    );
    let repeated = f.run(false).await;
    assert!(repeated.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        delivered.event_seq
    );
    fs::write(f.app.join("app.ts"), "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return 2;}").unwrap();
    let changed = f.run(false).await;
    assert!(matches!(
        changed.evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    assert!(changed.adopted_tasks.is_empty());
    let retained = f.project.task_session(None, None).unwrap();
    assert_eq!(retained.event_seq, delivered.event_seq);
    assert_eq!(retained.states[&id].check, dctx0_tasks::CheckStatus::Pass);
}

#[tokio::test]
async fn checked_record_fields_deliver_after_their_scalar_producer_and_mutations_preserve_history()
{
    let f = Fixture::new(
        "workspace W { value A = Int { range 1..2 } message Box { value: A } }",
        "W",
    );
    let production = "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;} export function boxFactory(value:unknown){if(!value||typeof value!=='object'||Object.keys(value).length!==1||!Object.hasOwn(value,'value'))throw TypeError('invalid fields');return {value:factory((value as {value:number}).value)};}";
    fs::write(f.app.join("app.ts"), production).unwrap();
    f.adapter(r#"import {factory,boxFactory} from '../../../app.ts';
function decode(value){if(value.kind==='record')return Object.fromEntries(Object.entries(value.value).map(([name,field])=>[name,decode(field)]));if(value.kind==='integer'||value.kind==='float')return Number(value.value);return value.kind==='unit'?undefined:value.value;}
function encode(value){if(typeof value==='number')return {kind:'integer',value:String(value)};if(value&&typeof value==='object')return {kind:'record',value:Object.fromEntries(Object.entries(value).map(([name,field])=>[name,encode(field)]))};return {kind:'string',value:String(value)};}
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'},{subject:'W.Box',role:'contract',artifact:'app.ts',entry_point:'boxFactory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'bounded safe integer',Box:'application-owned record'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=(subject==='W.A'?factory:boxFactory)(decode(value));}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:encode(result)};},async close(){}};}};"#);
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    let plan = f.project.task_plan("latest").unwrap();
    let id = |subject: &str| {
        plan.members
            .iter()
            .find(|member| member.candidate.subjects == [subject])
            .unwrap()
            .task_id
    };
    let scalar = id("W.A");
    let record = id("W.Box");
    let report = f.run(false).await;
    assert_eq!(report.evidence.run.result, VerificationResult::Pass);
    assert_eq!(
        report.adopted_tasks,
        vec![scalar, record],
        "{:?}",
        report.adoption_issues
    );
    let delivered = f.project.task_session(None, None).unwrap();
    for id in [scalar, record] {
        assert_eq!(delivered.states[&id].work, dctx0_tasks::WorkState::Done);
        assert_eq!(delivered.states[&id].check, dctx0_tasks::CheckStatus::Pass);
    }
    fs::write(
        f.app.join("app.ts"),
        production.replace(
            "return {value:factory((value as {value:number}).value)}",
            "return {value:'wrong application field type'}",
        ),
    )
    .unwrap();
    let changed = f.run(false).await;
    assert!(matches!(
        changed.evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    assert!(changed.adopted_tasks.is_empty());
    assert!(
        changed
            .evidence
            .run
            .cases
            .iter()
            .any(|case| case.subject == "W.Box"
                && matches!(case.result, VerificationResult::Fail { .. }))
    );
    let retained = f.project.task_session(None, None).unwrap();
    assert_eq!(retained.event_seq, delivered.event_seq);
    assert_eq!(
        retained.states[&record].check,
        dctx0_tasks::CheckStatus::Pass
    );
}

#[tokio::test]
async fn actual_sqlite_identity_checks_adopt_keys_and_replay_detects_duplicate_persistence() {
    let f = Fixture::new(
        "workspace W { value Number = Int { range 1..2 } entity Row { id:Number\nvalue:Number\nkey id } port Store { operation put(row:Row) -> Row { state upserts Row from row\nensures result == row } } }",
        "W",
    );
    let production = r#"import {Database} from 'bun:sqlite';
export function number(value:unknown){if(typeof value!=='number'||!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid number');return value;}
export function row(value:unknown){if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).sort().join(',')!=='id,value')throw TypeError('invalid row');const r=value as any;return {id:number(r.id),value:number(r.value)};}
export class Store {
 db=new Database(':memory:');
 constructor(){this.db.run('CREATE TABLE entries(id INTEGER, value INTEGER)');}
 put(value:unknown){const r=row(value);this.db.query('DELETE FROM entries WHERE id = ?').run(r.id);this.db.query('INSERT INTO entries VALUES (?,?)').run(r.id,r.value);return r;}
 observe(){return this.db.query('SELECT id,value FROM entries ORDER BY id,value').all();}
 close(){this.db.close();}
}"#;
    fs::write(f.app.join("app.ts"), production).unwrap();
    f.adapter(r#"import {Store,number,row} from '../../../app.ts';
function decode(v){if(v.kind==='record')return Object.fromEntries(Object.entries(v.value).map(([k,v])=>[k,decode(v)]));return v.kind==='integer'||v.kind==='float'?Number(v.value):v.kind==='unit'?undefined:v.value;}
function encode(r){return {kind:'record',value:{id:{kind:'integer',value:String(r.id)},value:{kind:'integer',value:String(r.value)}}};}
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Number',role:'contract',artifact:'app.ts',entry_point:'number'},{subject:'W.Row',role:'contract',artifact:'app.ts',entry_point:'row'},{subject:'W.Store.put',role:'port',artifact:'app.ts',entry_point:'Store.put'}],artifacts:['app.ts'],capabilities:['sessions','values','invocation','state','events'],representations:{Int:'safe integer'}};},async begin(){const store=new Store();return {async validate(subject,value){let actual;try{actual=subject==='W.Number'?number(decode(value)):row(decode(value));}catch(error){if(error instanceof TypeError)return {kind:'rejected',reason:String(error)};throw error;}return {kind:'accepted',value:subject==='W.Number'?{kind:'integer',value:String(actual)}:encode(actual)};},async invoke(i){return {kind:'returned',value:encode(store.put(decode(i.arguments[0])))};},async observe(){return {'W.Row':store.observe().map(encode)};},async close(){store.close();}};}};"#);
    let row = |value: &str| {
        VerificationValue::Record(
            [
                ("id".into(), VerificationValue::Integer("1".into())),
                ("value".into(), VerificationValue::Integer(value.into())),
            ]
            .into(),
        )
    };
    let mut fixtures = dctx0_verification::ProjectionFixtures::default();
    fixtures.operations.insert(
        "W.Store.put".into(),
        vec![dctx0_verification::OperationCaseInput {
            id: "W.Store.put:contract:replace".into(),
            invocation: Invocation {
                subject: "W.Store.put".into(),
                arguments: vec![row("2")],
            },
            setup: vec![Invocation {
                subject: "W.Store.put".into(),
                arguments: vec![row("1")],
            }],
            mocks: vec![],
        }],
    );
    fs::write(
        f.app.join(".dctx0/testing/typescript/examples.json"),
        serde_json::to_vec(&fixtures).unwrap(),
    )
    .unwrap();
    create_profiles(
        &f.app,
        &dctx0_target_typescript::Provider,
        true,
        false,
        false,
    )
    .unwrap();
    f.project.task_plan("latest").unwrap();
    let original = f.run(false).await;
    assert_eq!(
        original.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        original.evidence.run
    );
    let session = f.project.task_session(None, None).unwrap();
    let row_task = session
        .plan
        .members
        .iter()
        .find(|member| member.candidate.subjects == ["W.Row"])
        .unwrap();
    assert!(
        original.adopted_tasks.contains(&row_task.task_id),
        "{:?}",
        original.adoption_issues
    );
    let identity = original
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "W.Row:identity")
        .unwrap();
    assert_eq!(identity.result, VerificationResult::Pass);
    let replay = f
        .project
        .replay_verification(
            Some(&f.app),
            &original.evidence.id,
            &identity.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(replay.case_result, Some(VerificationResult::Pass));
    fs::write(
        f.app.join("app.ts"),
        production.replace(
            "DELETE FROM entries WHERE id = ?",
            "DELETE FROM entries WHERE id = -1 AND id = ?",
        ),
    )
    .unwrap();
    let mutated = f.run(false).await;
    assert!(mutated.adopted_tasks.is_empty());
    let identity = mutated
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == "W.Row:identity")
        .unwrap();
    assert!(
        matches!(&identity.result,VerificationResult::Fail{issue} if issue.code=="DUPLICATE_ENTITY_IDENTITY"),
        "{:?}",
        identity.result
    );
    let replay = f
        .project
        .replay_verification(
            Some(&f.app),
            &original.evidence.id,
            &identity.id,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert!(
        matches!(replay.case_result,Some(VerificationResult::Fail{issue}) if issue.code=="DUPLICATE_ENTITY_IDENTITY")
    );
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        session.event_seq
    );
    assert_eq!(
        f.project
            .saved_verification_run(&original.evidence.id)
            .unwrap(),
        original.evidence
    );
}
