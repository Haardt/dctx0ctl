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
}
const VALUE_ADAPTER: &str = r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#;

#[tokio::test]
async fn pinned_hybrid_rejects_wrong_provider_and_empty_scope_without_publication() {
    let f = Fixture::new("workspace W { value A = Int { range 1..2 } }", "W");
    let report = ".dctx0ctl/application-tests/rejected.json";
    let options = HybridVerificationOptions {
        plan: "missing-plan.json",
        application_group: "native",
        recipe: "missing-recipe.json",
        native_report: report,
    };
    let cancellation = NativeCancellation::default();
    let notes = f.app.join("missing-notes.json");
    let wrong_provider = f
        .project
        .complete_hybrid_verification_with_plan_hash(
            &f.app,
            &options,
            &notes,
            &dctx0_target_go::Provider,
            &BunHarness,
            &cancellation,
            &"0".repeat(64),
        )
        .await
        .unwrap_err();
    assert!(
        wrong_provider
            .to_string()
            .contains("target and profile provider differ")
    );
    let wrong_application = f
        .project
        .complete_application_tests(
            &f.app,
            options.plan,
            options.application_group,
            options.recipe,
            report,
            &notes,
            &dctx0_target_go::Provider,
            &cancellation,
        )
        .await
        .unwrap_err();
    let wrong_native = f
        .project
        .complete_verification(
            &f.app,
            &notes,
            &dctx0_target_go::Provider,
            &BunHarness,
            &cancellation,
        )
        .await
        .unwrap_err();
    for error in [wrong_application, wrong_native] {
        assert!(
            error
                .to_string()
                .contains("target and profile provider differ")
        );
    }
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["scope"] = serde_json::json!({"subjects": [], "tasks": []});
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let empty_scope = f
        .project
        .complete_hybrid_verification_with_plan_hash(
            &f.app,
            &options,
            &notes,
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &cancellation,
            &"0".repeat(64),
        )
        .await
        .unwrap_err();
    assert!(
        empty_scope
            .to_string()
            .contains("explicitly configured scope")
    );
    let empty_application = f
        .project
        .complete_application_tests(
            &f.app,
            options.plan,
            options.application_group,
            options.recipe,
            report,
            &notes,
            &dctx0_target_typescript::Provider,
            &cancellation,
        )
        .await
        .unwrap_err();
    let empty_native = f
        .project
        .complete_verification(
            &f.app,
            &notes,
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &cancellation,
        )
        .await
        .unwrap_err();
    for error in [empty_application, empty_native] {
        assert!(error.to_string().contains("explicitly configured scope"));
    }
    assert!(!f.app.join(report).exists());
    assert!(f.project.task_session(None, None).is_err());
}

#[tokio::test]
async fn hybrid_complete_refuses_swapped_pinned_plan_before_publication_or_execution() {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    use sha2::{Digest, Sha256};
    let f = Fixture::new(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } }",
        "W",
    );
    let path = ".dctx0/application-tests/hybrid.json";
    let mut bundle = f
        .project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: path,
                group: "native",
                test: "NativeB",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    let expected = format!("{:x}", Sha256::digest(fs::read(f.app.join(path)).unwrap()));
    bundle.plan.configuration.groups[0]
        .resources
        .timeout_seconds += 1;
    fs::write(f.app.join(path), serde_json::to_vec(&bundle).unwrap()).unwrap();
    assert!(
        f.project
            .read_application_testing_bundle(&f.app, path)
            .is_ok()
    );
    let report = ".dctx0ctl/application-tests/rejected.json";
    let error = f
        .project
        .complete_hybrid_verification_with_plan_hash(
            &f.app,
            &HybridVerificationOptions {
                plan: path,
                application_group: "native",
                recipe: "missing-recipe.json",
                native_report: report,
            },
            &f.app.join("missing-notes.json"),
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &NativeCancellation::default(),
            &expected,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("SHA-256 changed"), "{error}");
    assert!(!f.app.join(report).exists());
    assert!(f.project.task_session(None, None).is_err());
}

#[tokio::test]
async fn hybrid_complete_requires_reconciled_plan_without_publishing_it() {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    use sha2::{Digest, Sha256};
    let f = Fixture::new(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } }",
        "W",
    );
    let path = ".dctx0/application-tests/hybrid.json";
    let _bundle = f
        .project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: path,
                group: "native",
                test: "NativeB",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    let expected = format!("{:x}", Sha256::digest(fs::read(f.app.join(path)).unwrap()));
    let report = ".dctx0ctl/application-tests/rejected.json";
    let error = f
        .project
        .complete_hybrid_verification_with_plan_hash(
            &f.app,
            &HybridVerificationOptions {
                plan: path,
                application_group: "native",
                recipe: "missing-recipe.json",
                native_report: report,
            },
            &f.app.join("missing-notes.json"),
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &NativeCancellation::default(),
            &expected,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("No task plan"), "{error}");
    assert!(!f.app.join(report).exists());
    assert!(f.project.task_session(None, None).is_err());
}

#[tokio::test]
async fn hybrid_complete_rejects_old_task_check_without_reconciliation() {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    use sha2::{Digest, Sha256};
    let f = Fixture::new(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } }",
        "W",
    );
    f.project.task_plan("latest").unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let path = ".dctx0/application-tests/hybrid.json";
    let bundle = f
        .project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: path,
                group: "native",
                test: "NativeB",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    assert_eq!(bundle.plan.program.snapshot, before.plan.snapshot_id);
    assert_ne!(
        bundle.plan.program.check_occurrence,
        u64::try_from(before.plan.check_run_id).unwrap()
    );
    let expected = format!("{:x}", Sha256::digest(fs::read(f.app.join(path)).unwrap()));
    let report = ".dctx0ctl/application-tests/rejected.json";
    let error = f
        .project
        .complete_hybrid_verification_with_plan_hash(
            &f.app,
            &HybridVerificationOptions {
                plan: path,
                application_group: "native",
                recipe: "missing-recipe.json",
                native_report: report,
            },
            &f.app.join("missing-notes.json"),
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &NativeCancellation::default(),
            &expected,
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("requires a reconciled task plan for its exact check"),
        "{error}"
    );
    assert!(!f.app.join(report).exists());
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(after.plan.id, before.plan.id);
    assert_eq!(after.plan.check_run_id, before.plan.check_run_id);
    assert_eq!(after.plan.snapshot_id, before.plan.snapshot_id);
    assert_eq!(after.event_seq, before.event_seq);
}

#[tokio::test]
async fn hybrid_done_evidence_preserves_historical_result_and_publishes_open_together() {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    let f = std::mem::ManuallyDrop::new(Fixture::new(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } }",
        "W",
    ));
    fs::write(f.app.join("app.ts"),"export function factory(value){if(!Number.isSafeInteger(value)||value<1||value>2)throw TypeError('A rejected');return value;} export function decodeB(raw){const value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(raw));if(!Number.isSafeInteger(value)||value<10||value>11)throw TypeError('B rejected');return value;}").unwrap();
    f.adapter(&VALUE_ADAPTER.replace("async validate(subject,value){","async validate(subject,value){if(subject!=='W.A')throw Error('native-owned B reached legacy');"));
    publish_application_test_library(&f.app, &dctx0_target_typescript::application_test_library())
        .unwrap();
    let directory = f.app.join(".dctx0/application-tests");
    fs::write(directory.join("native-b.test.ts"),r#"import {test} from 'bun:test';
import {AssessmentClient,decoderTest} from './library/index.ts';
import {decodeB} from '../../app.ts';
test('NativeB',async()=>{const client=await AssessmentClient.connect(process.env.DCTX0_APPLICATION_SOCKET!);try{await decoderTest(client,'NativeB',(raw)=>{try{return {kind:'accepted',value:decodeB(raw)};}catch(error){if(error instanceof TypeError||error instanceof SyntaxError)return {kind:'rejected',reason:error.message};throw error;}},value=>({kind:'integer',value:String(value)}));await client.finish();}finally{client.close();}});"#).unwrap();
    let recipe = serde_json::json!({"schema_version":1,"runtime":"bun","root_test":"NativeB","test_directory":"root","entrypoint":".dctx0/application-tests/native-b.test.ts","prepare":[],"build":[{"program":"bun","arguments":["build","--target=bun","--external=bun:test","--outdir","{artifacts}/build",".dctx0/application-tests/native-b.test.ts"]}],"catalog":{"program":"bun","arguments":[".dctx0/application-tests/library/catalog.ts",".dctx0/application-tests/native-b.test.ts"]},"run":{"program":"bun","arguments":["test","--reporter","junit","--reporter-outfile","{artifacts}/native.junit.xml","./.dctx0/application-tests/native-b.test.ts"]},"junit":".dctx0ctl/application-tests/native.junit.xml"});
    fs::write(
        directory.join("recipe.json"),
        serde_json::to_vec(&recipe).unwrap(),
    )
    .unwrap();
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["history_retention"] = serde_json::json!("preserve-all");
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let _bundle = f
        .project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/hybrid.json",
                group: "native",
                test: "NativeB",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
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
    let initial = f.project.task_session(None, None).unwrap();
    let a = initial
        .plan
        .members
        .iter()
        .find(|member| {
            member.candidate.subjects.contains(&"W.A".to_string())
                && member.candidate.kind == dctx0_tasks::TaskKind::Contract
        })
        .unwrap()
        .task_id;
    let b = initial
        .plan
        .members
        .iter()
        .find(|member| {
            member.candidate.subjects.contains(&"W.B".to_string())
                && member.candidate.kind == dctx0_tasks::TaskKind::Contract
        })
        .unwrap()
        .task_id;
    let started = f
        .project
        .task_action(dctx0_tasks::Action::Start {
            task: a,
            actor: "author".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    f.project
        .task_action(dctx0_tasks::Action::Complete {
            task: a,
            actor: "author".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            result: dctx0_tasks::TaskResult {
                summary: "Historical delivery".into(),
                artifacts: vec![],
                decisions: vec!["Preserve original".into()],
                implementation: None,
            },
        })
        .unwrap();
    assert!(f.root.join("spec/w.dctx0").is_file());
    let before = f.project.task_session(None, None).unwrap();
    assert_eq!(
        before.states[&a].check,
        dctx0_tasks::CheckStatus::Unverified
    );
    assert_eq!(before.states[&b].work, dctx0_tasks::WorkState::Open);
    let report = f
        .project
        .run_hybrid_verification(
            &f.app,
            &HybridVerificationOptions {
                plan: ".dctx0/application-tests/hybrid.json",
                application_group: "native",
                recipe: ".dctx0/application-tests/recipe.json",
                native_report: ".dctx0ctl/application-tests/fresh-done-native.json",
            },
            &BunHarness,
            &NativeCancellation::default(),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        report.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        report.evidence.run.provenance.incomplete
    );
    assert!(
        report.adopted_tasks.contains(&a),
        "{:?}",
        report.adoption_issues
    );
    assert!(
        report.adopted_tasks.contains(&b),
        "{:?}",
        report.adoption_issues
    );
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(after.states[&a].work, dctx0_tasks::WorkState::Done);
    assert_eq!(after.states[&a].check, dctx0_tasks::CheckStatus::Pass);
    assert_eq!(after.states[&b].check, dctx0_tasks::CheckStatus::Pass);
    assert_eq!(
        serde_json::to_vec(&before.states[&a].result).unwrap(),
        serde_json::to_vec(&after.states[&a].result).unwrap()
    );
    assert_eq!(before.states[&a].reviews, after.states[&a].reviews);
    assert_eq!(
        serde_json::to_vec(&before.definitions).unwrap(),
        serde_json::to_vec(&after.definitions).unwrap()
    );
    assert_eq!(
        after.states[&a].evidence.last().unwrap().report,
        report.evidence.id
    );
    assert_eq!(
        report
            .evidence
            .run
            .provenance
            .task_pin
            .as_ref()
            .unwrap()
            .event_sequence,
        before.event_seq
    );
    let historical = f
        .project
        .task_session(Some(before.plan.id), Some(before.event_seq))
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&historical.states[&a]).unwrap(),
        serde_json::to_vec(&before.states[&a]).unwrap()
    );
}

#[tokio::test]
async fn pinned_hybrid_authored_inspection_preserves_historical_result() {
    fresh_hybrid_authored_inspection(true).await;
}

#[tokio::test]
async fn unpinned_hybrid_authored_inspection_preserves_historical_result() {
    fresh_hybrid_authored_inspection(false).await;
}

async fn fresh_hybrid_authored_inspection(pinned: bool) {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    let f = std::mem::ManuallyDrop::new(Fixture::new(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } port Store { operation prepare(value:Int) -> Unit { requires value > 0 } } proof Approval }",
        "W",
    ));
    fs::write(f.app.join("app.ts"),"export function factory(value){if(!Number.isSafeInteger(value)||value<1||value>2)throw TypeError('A rejected');return value;} export function prepare(value){if(value<=0)throw TypeError('invalid');} export function decodeB(raw){const value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(raw));if(!Number.isSafeInteger(value)||value<10||value>11)throw TypeError('B rejected');return value;}").unwrap();
    let adapter = VALUE_ADAPTER.replace("import {factory}", "import {factory,prepare}")
        .replace("role:'contract',artifact:'app.ts',entry_point:'factory'}]", "role:'contract',artifact:'app.ts',entry_point:'factory'},{subject:'W.Store.prepare',role:'port',artifact:'app.ts',entry_point:'prepare'}]")
        .replace("'sessions','values','events'", "'sessions','values','events','invocation'")
        .replace("async close(){}", "async invoke(i){prepare(Number(i.arguments[0].value));return {kind:'returned',value:{kind:'unit'}};},async close(){}")
        .replace("async validate(subject,value){", "async validate(subject,value){if(subject!=='W.A')throw Error('native-owned B reached legacy');");
    f.adapter(&adapter);
    publish_application_test_library(&f.app, &dctx0_target_typescript::application_test_library())
        .unwrap();
    let directory = f.app.join(".dctx0/application-tests");
    fs::write(directory.join("native-b.test.ts"),r#"import {test} from 'bun:test';
import {AssessmentClient,decoderTest} from './library/index.ts';
import {decodeB} from '../../app.ts';
test('NativeB',async()=>{const client=await AssessmentClient.connect(process.env.DCTX0_APPLICATION_SOCKET!);try{await decoderTest(client,'NativeB',(raw)=>{try{return {kind:'accepted',value:decodeB(raw)};}catch(error){if(error instanceof TypeError||error instanceof SyntaxError)return {kind:'rejected',reason:error.message};throw error;}},value=>({kind:'integer',value:String(value)}));await client.finish();}finally{client.close();}});"#).unwrap();
    let recipe = serde_json::json!({"schema_version":1,"runtime":"bun","root_test":"NativeB","test_directory":"root","entrypoint":".dctx0/application-tests/native-b.test.ts","prepare":[],"build":[{"program":"bun","arguments":["build","--target=bun","--external=bun:test","--outdir","{artifacts}/build",".dctx0/application-tests/native-b.test.ts"]}],"catalog":{"program":"bun","arguments":[".dctx0/application-tests/library/catalog.ts",".dctx0/application-tests/native-b.test.ts"]},"run":{"program":"bun","arguments":["test","--reporter","junit","--reporter-outfile","{artifacts}/native.junit.xml","./.dctx0/application-tests/native-b.test.ts"]},"junit":".dctx0ctl/application-tests/native.junit.xml"});
    fs::write(
        directory.join("recipe.json"),
        serde_json::to_vec(&recipe).unwrap(),
    )
    .unwrap();
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["history_retention"] = serde_json::json!("preserve-all");
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let _bundle = f
        .project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/hybrid.json",
                group: "native",
                test: "NativeB",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
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
    let initial = f.project.task_session(None, None).unwrap();
    let a = initial
        .plan
        .members
        .iter()
        .find(|member| {
            member.candidate.subjects.contains(&"W.A".to_string())
                && member.candidate.kind == dctx0_tasks::TaskKind::Contract
        })
        .unwrap()
        .task_id;
    let started = f
        .project
        .task_action(dctx0_tasks::Action::Start {
            task: a,
            actor: "author".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    f.project
        .task_action(dctx0_tasks::Action::Complete {
            task: a,
            actor: "author".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            result: dctx0_tasks::TaskResult {
                summary: "Historical delivery".into(),
                artifacts: vec![],
                decisions: vec![],
                implementation: None,
            },
        })
        .unwrap();
    let adapter_task = initial
        .plan
        .members
        .iter()
        .find(|member| {
            member.candidate.kind == dctx0_tasks::TaskKind::Adapter
                && member
                    .candidate
                    .subjects
                    .contains(&"W.Store.prepare".into())
        })
        .unwrap()
        .task_id;
    let prereq_ids = initial
        .member(adapter_task)
        .unwrap()
        .candidate
        .prerequisites
        .iter()
        .map(|prerequisite| {
            initial
                .plan
                .members
                .iter()
                .find(|member| member.candidate.key == prerequisite.key)
                .unwrap()
                .task_id
        })
        .collect::<Vec<_>>();
    for id in &prereq_ids {
        let started = f
            .project
            .task_action(dctx0_tasks::Action::Start {
                task: *id,
                actor: "author".into(),
                reservation: None,
            })
            .unwrap()
            .state
            .unwrap();
        f.project
            .task_action(dctx0_tasks::Action::Complete {
                task: *id,
                actor: "author".into(),
                claim: started.claim.unwrap(),
                version: started.version,
                result: dctx0_tasks::TaskResult {
                    summary: "Historical prerequisite".into(),
                    artifacts: vec![],
                    decisions: vec![],
                    implementation: None,
                },
            })
            .unwrap();
    }
    let adapter_started = f
        .project
        .task_action(dctx0_tasks::Action::Start {
            task: adapter_task,
            actor: "author".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    f.project
        .task_action(dctx0_tasks::Action::Complete {
            task: adapter_task,
            actor: "author".into(),
            claim: adapter_started.claim.unwrap(),
            version: adapter_started.version,
            result: dctx0_tasks::TaskResult {
                summary: "Historical port binding".into(),
                artifacts: vec![],
                decisions: vec![],
                implementation: None,
            },
        })
        .unwrap();
    let before = f.project.task_session(None, None).unwrap();
    let uninspected = f
        .project
        .run_hybrid_verification(
            &f.app,
            &HybridVerificationOptions {
                plan: ".dctx0/application-tests/hybrid.json",
                application_group: "native",
                recipe: ".dctx0/application-tests/recipe.json",
                native_report: ".dctx0ctl/application-tests/uninspected.json",
            },
            &BunHarness,
            &NativeCancellation::default(),
            false,
        )
        .await
        .unwrap();
    assert!(matches!(
        uninspected.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    assert!(uninspected.adopted_tasks.is_empty());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    let hash = |path: &std::path::Path| {
        dctx0_store::VerificationArtifact::new("application/octet-stream", fs::read(path).unwrap())
            .reference
            .hash
    };
    let snapshot = f.project.load(&before.plan.snapshot_id).unwrap();
    let profiles = f
        .project
        .profiles(Some(&f.app), Some("typescript"))
        .unwrap()
        .unwrap();
    let mut pin = VerificationTaskPin {
        plan_id: before.plan.id,
        event_sequence: before.event_seq,
        requirements: Default::default(),
        profile_bundles: Default::default(),
    };
    for member in &before.plan.members {
        if !member.candidate.subjects.is_empty()
            && member
                .candidate
                .subjects
                .iter()
                .all(|subject| subject == "W" || subject.starts_with("W."))
        {
            pin.requirements
                .insert(member.task_id, member.candidate.requirements_hash.clone());
            let selection = dctx0_tasks::select_profiles(
                &snapshot,
                &member.candidate,
                &profiles.manifest,
                &profiles.documents,
            );
            if selection.require_complete().is_ok() {
                pin.profile_bundles
                    .insert(member.task_id, selection.bundle.hash);
            }
        }
    }
    let candidate = &before.member(a).unwrap().candidate;
    let app_hash = hash(&f.app.join("app.ts"));
    let facts = candidate
        .facts
        .iter()
        .map(|fact| {
            (
                fact.clone(),
                dctx0_tasks::SharedFactInspection {
                    fact: fact.clone(),
                    task_ids: [a].into(),
                    finding: "Reviewed this fact against app.ts".into(),
                    sources: ["app.ts".into()].into(),
                    limitations: vec!["Manual review".into()],
                },
            )
        })
        .collect();
    let task_inspection = dctx0_tasks::DoneTaskInspection {
        state_version: before.states[&a].version,
        requirements_hash: candidate.requirements_hash.clone(),
        profile_bundle: pin.profile_bundles[&a].clone(),
        subjects: candidate.subjects.iter().cloned().collect(),
        types: candidate.types.iter().cloned().collect(),
        facts: candidate.facts.iter().cloned().collect(),
        fact_reviews: candidate.facts.iter().cloned().collect(),
        implementation_finding: "Checked factory against W.A".into(),
        profile_finding: "Checked the selected TypeScript profile".into(),
        adapter_binding_finding: None,
        sources: ["app.ts".into()].into(),
        limitations: vec!["Manual review of facts".into()],
    };
    let adapter_candidate = &before.member(adapter_task).unwrap().candidate;
    let adapter_inspection = dctx0_tasks::DoneTaskInspection {
        state_version: before.states[&adapter_task].version,
        requirements_hash: adapter_candidate.requirements_hash.clone(),
        profile_bundle: pin.profile_bundles[&adapter_task].clone(),
        subjects: adapter_candidate.subjects.iter().cloned().collect(),
        types: adapter_candidate.types.iter().cloned().collect(),
        facts: adapter_candidate.facts.iter().cloned().collect(),
        fact_reviews: adapter_candidate.facts.iter().cloned().collect(),
        implementation_finding: "Reviewed prepare implementation".into(),
        profile_finding: "Reviewed port profile".into(),
        adapter_binding_finding: Some("Reviewed Store.prepare binding to prepare".into()),
        sources: ["app.ts".into()].into(),
        limitations: vec!["Manual review".into()],
    };
    let mut facts: std::collections::BTreeMap<String, dctx0_tasks::SharedFactInspection> = facts;
    for fact in &adapter_candidate.facts {
        facts
            .entry(fact.clone())
            .or_insert_with(|| dctx0_tasks::SharedFactInspection {
                fact: fact.clone(),
                task_ids: Default::default(),
                finding: "Reviewed fact against app.ts".into(),
                sources: ["app.ts".into()].into(),
                limitations: vec!["Manual review".into()],
            })
            .task_ids
            .insert(adapter_task);
    }
    let mut reviewed_tasks = std::collections::BTreeMap::from([
        (a, task_inspection),
        (adapter_task, adapter_inspection),
    ]);
    for id in &prereq_ids {
        let candidate = &before.member(*id).unwrap().candidate;
        for fact in &candidate.facts {
            facts
                .entry(fact.clone())
                .or_insert_with(|| dctx0_tasks::SharedFactInspection {
                    fact: fact.clone(),
                    task_ids: Default::default(),
                    finding: "Reviewed fact against app.ts".into(),
                    sources: ["app.ts".into()].into(),
                    limitations: vec!["Manual review".into()],
                })
                .task_ids
                .insert(*id);
        }
        reviewed_tasks.insert(
            *id,
            dctx0_tasks::DoneTaskInspection {
                state_version: before.states[id].version,
                requirements_hash: candidate.requirements_hash.clone(),
                profile_bundle: pin.profile_bundles[id].clone(),
                subjects: candidate.subjects.iter().cloned().collect(),
                types: candidate.types.iter().cloned().collect(),
                facts: candidate.facts.iter().cloned().collect(),
                fact_reviews: candidate.facts.iter().cloned().collect(),
                implementation_finding: "Reviewed prerequisite contract".into(),
                profile_finding: "Reviewed selected target profile".into(),
                adapter_binding_finding: None,
                sources: ["app.ts".into()].into(),
                limitations: vec!["Manual review".into()],
            },
        );
    }
    let inspected_count = reviewed_tasks.len();
    let done_inspections = dctx0_tasks::InspectedDoneBatch {
        schema_version: 1,
        actor: "reviewer".into(),
        snapshot: before.plan.snapshot_id.clone(),
        check_occurrence: before.plan.check_run_id,
        task_pin: pin,
        sources: [("app.ts".into(), app_hash)].into(),
        facts,
        tasks: reviewed_tasks,
    };
    let notes = serde_json::json!({"schema_version":1,"actor":"reviewer","scope":"Explicitly reviewed proof Approval", "reviewed_hashes":{"app.ts":hash(&f.app.join("app.ts"))}, "reviewed_spec_hashes":{"w.dctx0":hash(&f.root.join("spec/w.dctx0"))}, "subjects":{"W.Approval":{"report":"Inspected the modeled approval obligation against this source", "limitations":["Human judgement"], "review":{"approved":true,"findings":[]}}}, "done_inspections":done_inspections});
    let note_path = f.root.join("inspection.json");
    let mut stale_notes = notes.clone();
    stale_notes["reviewed_hashes"]["app.ts"] = "stale".into();
    fs::write(&note_path, serde_json::to_vec(&stale_notes).unwrap()).unwrap();
    let stale = f
        .project
        .complete_hybrid_verification(
            &f.app,
            &HybridVerificationOptions {
                plan: ".dctx0/application-tests/hybrid.json",
                application_group: "native",
                recipe: ".dctx0/application-tests/recipe.json",
                native_report: ".dctx0ctl/application-tests/stale-inspection.json",
            },
            &note_path,
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await;
    assert!(stale.is_err());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    fs::write(f.app.join("unreviewed.ts"), "export const value = 1;\n").unwrap();
    let mut laundered = notes.clone();
    laundered["done_inspections"]["sources"]["unreviewed.ts"] =
        hash(&f.app.join("unreviewed.ts")).into();
    fs::write(&note_path, serde_json::to_vec(&laundered).unwrap()).unwrap();
    let rejected = f
        .project
        .complete_hybrid_verification(
            &f.app,
            &HybridVerificationOptions {
                plan: ".dctx0/application-tests/hybrid.json",
                application_group: "native",
                recipe: ".dctx0/application-tests/recipe.json",
                native_report: ".dctx0ctl/application-tests/laundered.json",
            },
            &note_path,
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await;
    assert!(rejected.is_err());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    let mut wrong_binding = notes.clone();
    let other_hash = hash(&f.app.join("unreviewed.ts"));
    wrong_binding["reviewed_hashes"]["unreviewed.ts"] = other_hash.clone().into();
    wrong_binding["done_inspections"]["sources"]["unreviewed.ts"] = other_hash.into();
    wrong_binding["done_inspections"]["tasks"][adapter_task.to_string()]["sources"] =
        serde_json::json!(["unreviewed.ts"]);
    fs::write(&note_path, serde_json::to_vec(&wrong_binding).unwrap()).unwrap();
    let rejected_binding = f
        .project
        .complete_hybrid_verification(
            &f.app,
            &HybridVerificationOptions {
                plan: ".dctx0/application-tests/hybrid.json",
                application_group: "native",
                recipe: ".dctx0/application-tests/recipe.json",
                native_report: ".dctx0ctl/application-tests/wrong-binding.json",
            },
            &note_path,
            &dctx0_target_typescript::Provider,
            &BunHarness,
            &NativeCancellation::default(),
        )
        .await;
    assert!(rejected_binding.is_err());
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before.event_seq
    );
    fs::write(&note_path, serde_json::to_vec(&notes).unwrap()).unwrap();
    let expected_plan_sha256 = hash(&f.app.join(".dctx0/application-tests/hybrid.json"));
    let options = HybridVerificationOptions {
        plan: ".dctx0/application-tests/hybrid.json",
        application_group: "native",
        recipe: ".dctx0/application-tests/recipe.json",
        native_report: ".dctx0ctl/application-tests/fresh-inspected.json",
    };
    let cancellation = NativeCancellation::default();
    let report = if pinned {
        f.project
            .complete_hybrid_verification_with_plan_hash(
                &f.app,
                &options,
                &note_path,
                &dctx0_target_typescript::Provider,
                &BunHarness,
                &cancellation,
                &expected_plan_sha256,
            )
            .await
    } else {
        f.project
            .complete_hybrid_verification(
                &f.app,
                &options,
                &note_path,
                &dctx0_target_typescript::Provider,
                &BunHarness,
                &cancellation,
            )
            .await
    }
    .unwrap();
    assert_eq!(report.result, VerificationResult::Pass);
    assert!(report.adopted_tasks.contains(&a));
    assert!(report.adopted_tasks.contains(&adapter_task), "{:?}", report);
    let after = f.project.task_session(None, None).unwrap();
    assert_eq!(after.states[&a].check, dctx0_tasks::CheckStatus::Pass);
    assert_eq!(
        after.states[&adapter_task].check,
        dctx0_tasks::CheckStatus::Pass
    );
    assert_eq!(
        before.states[&adapter_task].result,
        after.states[&adapter_task].result
    );
    assert_eq!(
        before.states[&adapter_task].reviews,
        after.states[&adapter_task].reviews
    );
    assert!(
        after.states[&a]
            .evidence
            .last()
            .unwrap()
            .report
            .contains("INSPECTED")
    );
    assert_eq!(
        serde_json::to_vec(&before.states[&a].result).unwrap(),
        serde_json::to_vec(&after.states[&a].result).unwrap()
    );
    let native = dctx0_store::Store::open(&f.project.database, dctx0_store::AccessMode::ReadOnly)
        .unwrap()
        .verification_run(&f.project.root.to_string_lossy(), &report.native_run)
        .unwrap();
    assert_eq!(native.run.origin, VerificationRunOrigin::Native);
    assert_eq!(native.run.result, VerificationResult::Pass);
    assert!(
        native
            .run
            .artifacts
            .iter()
            .any(|a| a.media_type == "application/vnd.dctx0.program-inspections+json")
    );
    let status = f.project.status(Some(&f.app), None).unwrap();
    assert_eq!(status.tasks.inspected_current, inspected_count);
    assert_eq!(status.tasks.inspected_stale, 0);
    let reconciled = f
        .project
        .reconcile_native_tasks(&f.app, &report.native_run, &[])
        .unwrap();
    assert_eq!(reconciled.native_cases_executed, 0);
    assert_eq!(reconciled.published_checks, 0);
    assert_eq!(reconciled.done_pass, reconciled.tasks);
    let original_source = fs::read(f.app.join("app.ts")).unwrap();
    fs::write(f.app.join("app.ts"), "export const changed = true;\n").unwrap();
    assert!(
        f.project
            .reconcile_native_tasks(&f.app, &report.native_run, &[])
            .err()
            .unwrap()
            .to_string()
            .contains("Native code changed")
    );
    let stale = f.project.status(Some(&f.app), None).unwrap();
    assert_eq!(stale.tasks.inspected_current, 0);
    assert_eq!(stale.tasks.inspected_stale, inspected_count);
    fs::write(f.app.join("app.ts"), original_source).unwrap();
    let profile = f.app.join(".dctx0/profiles/typescript/common.md");
    let mut profile_bytes = fs::read(&profile).unwrap();
    profile_bytes.extend_from_slice(b"\nProfile changed after verification.\n");
    fs::write(profile, profile_bytes).unwrap();
    assert_eq!(
        f.project
            .status(Some(&f.app), None)
            .unwrap()
            .tasks
            .inspected_stale,
        inspected_count
    );
}

#[tokio::test]
async fn application_incremental_reuses_contract_cases_and_runs_native_tests() {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    let f = Fixture::new("workspace W { value B = Int { range 10..11 } }", "W.B");
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["history_retention"] = "preserve-all".into();
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    fs::write(f.app.join("app.ts"), "export function decode(raw){const v=JSON.parse(new TextDecoder().decode(raw));if(!Number.isSafeInteger(v)||v<10||v>11)throw new TypeError('invalid');return v;}").unwrap();
    publish_application_test_library(&f.app, &dctx0_target_typescript::application_test_library())
        .unwrap();
    let dir = f.app.join(".dctx0/application-tests");
    fs::write(dir.join("native.test.ts"), r#"import {test} from 'bun:test';
import {AssessmentClient,decoderTest} from './library/index.ts';
import {decode} from '../../app.ts';
import {existsSync} from 'node:fs';
test('Native',async()=>{const c=await AssessmentClient.connect(process.env.DCTX0_APPLICATION_SOCKET!);try{await decoderTest(c,'Native',(raw)=>{try{return {kind:'accepted',value:decode(raw)};}catch(e){if(e instanceof TypeError||e instanceof SyntaxError)return {kind:'rejected',reason:String(e)};throw e;}},v=>({kind:'integer',value:String(v)}));await c.finish();console.log('NATIVE_ASSERTION_EXECUTED');if(existsSync('__MARKER__'))throw Error('independent native assertion');}finally{c.close();}});"#.replace("__MARKER__", &f.root.join("fail-native").to_string_lossy())).unwrap();
    fs::write(dir.join("recipe.json"), serde_json::to_vec(&serde_json::json!({"schema_version":1,"runtime":"bun","root_test":"Native","test_directory":"root","entrypoint":".dctx0/application-tests/native.test.ts","prepare":[],"build":[{"program":"bun","arguments":["build","--target=bun","--external=bun:test","--outdir","{artifacts}/build",".dctx0/application-tests/native.test.ts"]}],"catalog":{"program":"bun","arguments":[".dctx0/application-tests/library/catalog.ts",".dctx0/application-tests/native.test.ts"]},"run":{"program":"bun","arguments":["test","--reporter","junit","--reporter-outfile","{artifacts}/native.junit.xml","./.dctx0/application-tests/native.test.ts"]},"junit":".dctx0ctl/application-tests/native.junit.xml"})).unwrap()).unwrap();
    fs::write(f.app.join("reuse.json"), r#"{"schema_version":1,"deterministic_units":["contract:W.B"],"files":[],"environment":[]}"#).unwrap();
    f.project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/plan.json",
                group: "native",
                test: "Native",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
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
    let cancel = NativeCancellation::default();
    let run = |options: NativeReuseOptions| {
        let f = &f;
        let cancel = &cancel;
        async move {
            f.project
                .verify_application_tests_incremental(
                    &f.app,
                    ".dctx0/application-tests/plan.json",
                    "native",
                    ".dctx0/application-tests/recipe.json",
                    ".dctx0ctl/application-tests/report.json",
                    cancel,
                    &options,
                )
                .await
        }
    };
    let first = run(NativeReuseOptions {
        mode: NativeReuseMode::Off,
        inputs: "reuse.json".into(),
        parent: None,
    })
    .await
    .unwrap();
    assert_eq!(first.result, VerificationResult::Pass, "{:?}", first.issues);
    let first_evidence = first.evidence.unwrap();
    let parent = first_evidence.id.clone();
    let second = run(NativeReuseOptions {
        mode: Default::default(),
        inputs: "reuse.json".into(),
        parent: Some(parent.clone()),
    })
    .await
    .unwrap();
    assert_eq!(
        second.result,
        VerificationResult::Pass,
        "{:?}",
        second.issues
    );
    let stats = second.incremental.as_ref().unwrap();
    assert!(stats.reused_cases > 0 && stats.executed_cases == 0 && !stats.adoption_authority);
    assert!(
        second
            .phases
            .iter()
            .find(|phase| phase.phase == "native-tests")
            .unwrap()
            .audit
            .stdout
            .contains("NATIVE_ASSERTION_EXECUTED")
    );
    assert!(
        second
            .evidence
            .unwrap()
            .run
            .artifacts
            .iter()
            .any(|a| a.media_type == dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE)
    );
    assert!(f.project.check().unwrap().ok);
    f.project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/plan.json",
                group: "native",
                test: "Native",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    let rechecked = run(NativeReuseOptions {
        mode: Default::default(),
        inputs: "reuse.json".into(),
        parent: Some(parent.clone()),
    })
    .await
    .unwrap();
    assert_eq!(
        rechecked.result,
        VerificationResult::Pass,
        "{:?}",
        rechecked.issues
    );
    assert!(
        rechecked.incremental.as_ref().unwrap().reused_cases > 0,
        "unchanged check occurrence must retain reuse"
    );
    f.project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/plan.json",
                group: "native",
                test: "Native",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(31),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    let changed_group = run(NativeReuseOptions {
        mode: Default::default(),
        inputs: "reuse.json".into(),
        parent: Some(parent.clone()),
    })
    .await
    .unwrap();
    assert_eq!(changed_group.result, VerificationResult::Pass);
    assert!(changed_group.incremental.unwrap().reused_cases > 0);
    f.project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/plan.json",
                group: "native",
                test: "Native",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(30),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    let recipe = dir.join("recipe.json");
    let original_recipe = fs::read(&recipe).unwrap();
    let mut revised: serde_json::Value = serde_json::from_slice(&original_recipe).unwrap();
    revised["run"]["arguments"]
        .as_array_mut()
        .unwrap()
        .push("--timeout=30000".into());
    fs::write(&recipe, serde_json::to_vec(&revised).unwrap()).unwrap();
    let changed_recipe = run(NativeReuseOptions {
        mode: Default::default(),
        inputs: "reuse.json".into(),
        parent: Some(parent.clone()),
    })
    .await
    .unwrap();
    assert_eq!(changed_recipe.result, VerificationResult::Pass);
    assert!(changed_recipe.incremental.unwrap().reused_cases > 0);
    fs::write(&recipe, original_recipe).unwrap();
    // Deliberately undeclared external state simulates a native assertion
    // changing independently of central inputs; it must never be hidden.
    fs::write(f.root.join("fail-native"), "fail").unwrap();
    let failed = run(NativeReuseOptions {
        mode: Default::default(),
        inputs: "reuse.json".into(),
        parent: Some(parent.clone()),
    })
    .await
    .unwrap();
    assert!(matches!(failed.result, VerificationResult::Fail { .. }));
    let failed_evidence = failed.evidence.unwrap();
    assert!(matches!(
        failed_evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    assert!(failed_evidence.run.cases.is_empty());
    assert!(!failed_evidence.run.required_cases.is_empty());
    assert!(
        !failed_evidence
            .run
            .artifacts
            .iter()
            .any(|artifact| artifact.media_type
                == dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE
                || artifact.media_type
                    == dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE)
    );
    let store =
        dctx0_store::Store::open(&f.project.database, dctx0_store::AccessMode::ReadOnly).unwrap();
    let saved = store
        .verification_run(&f.project.root.to_string_lossy(), &failed_evidence.id)
        .unwrap();
    assert!(matches!(saved.run.result, VerificationResult::Fail { .. }));
    assert_eq!(
        store
            .verification_runs(&f.project.root.to_string_lossy())
            .unwrap()[0]
            .id,
        failed_evidence.id
    );
    let latest = f
        .project
        .status(Some(&f.app), None)
        .unwrap()
        .latest_run
        .unwrap();
    assert_eq!(latest.summary.id, failed_evidence.id);
    assert!(matches!(
        latest.summary.result,
        VerificationResult::Fail { .. }
    ));
    assert_eq!(
        serde_json::to_value(
            store
                .verification_run(&f.project.root.to_string_lossy(), &parent)
                .unwrap()
                .run
        )
        .unwrap(),
        serde_json::to_value(first_evidence.run).unwrap()
    );
    let failed_report: serde_json::Value = serde_json::from_slice(
        &fs::read(f.app.join(".dctx0ctl/application-tests/report.json")).unwrap(),
    )
    .unwrap();
    assert!(
        failed_report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["message"]
                .as_str()
                .is_some_and(|message| message.contains("native-tests exited")))
    );
    fs::remove_file(f.root.join("fail-native")).unwrap();
    fs::write(f.app.join("app.ts"), "export function decode(raw){const v=JSON.parse(new TextDecoder().decode(raw));if(!Number.isSafeInteger(v)||v<10||v>11)throw new TypeError('invalid');return v;} // changed source").unwrap();
    let third = run(NativeReuseOptions {
        mode: NativeReuseMode::Off,
        inputs: "reuse.json".into(),
        parent: None,
    })
    .await
    .unwrap();
    assert_eq!(third.result, VerificationResult::Pass);
    assert_eq!(third.incremental.as_ref().unwrap().reused_cases, 0);
    assert!(
        !third
            .evidence
            .unwrap()
            .run
            .artifacts
            .iter()
            .any(|a| a.media_type == dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE)
    );
    fs::write(f.app.join("app.ts"), "export function decode(raw){const v=JSON.parse(new TextDecoder().decode(raw));if(!Number.isSafeInteger(v)||v<10||v>11)throw new TypeError('invalid');return v;}").unwrap();
    let library = dir.join("library/index.ts");
    let original_library = fs::read_to_string(&library).unwrap();
    fs::write(
        &library,
        format!("{}\n// changed library", original_library),
    )
    .unwrap();
    let changed_library = run(NativeReuseOptions {
        mode: NativeReuseMode::Off,
        inputs: "reuse.json".into(),
        parent: None,
    })
    .await
    .unwrap();
    assert_eq!(changed_library.result, VerificationResult::Pass);
    assert_eq!(changed_library.incremental.unwrap().reused_cases, 0);
    fs::write(&library, original_library).unwrap();
    f.project.task_plan("latest").unwrap();
    let task = f
        .project
        .task_session(None, None)
        .unwrap()
        .next(&Default::default(), None)
        .unwrap()
        .task_id;
    let started = f
        .project
        .task_action(dctx0_tasks::Action::Start {
            task,
            actor: "author".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    let authored = dctx0_tasks::TaskResult {
        summary: "Authored application implementation".into(),
        artifacts: vec!["app.ts".into()],
        decisions: vec![],
        implementation: None,
    };
    f.project
        .task_action(dctx0_tasks::Action::Complete {
            task,
            actor: "author".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            result: authored.clone(),
        })
        .unwrap();
    let fresh = run(NativeReuseOptions {
        mode: NativeReuseMode::Off,
        inputs: "reuse.json".into(),
        parent: None,
    })
    .await
    .unwrap();
    assert_eq!(fresh.result, VerificationResult::Pass);
    let parent = fresh.evidence.unwrap().id;
    let adopted = f
        .project
        .verify_application_tests_incremental_with_adoption(
            &f.app,
            ".dctx0/application-tests/plan.json",
            "native",
            ".dctx0/application-tests/recipe.json",
            ".dctx0ctl/application-tests/adopt.json",
            &cancel,
            &NativeReuseOptions {
                mode: Default::default(),
                inputs: "reuse.json".into(),
                parent: Some(parent),
            },
            false,
        )
        .await
        .unwrap();
    assert!(adopted.incremental.as_ref().unwrap().reused_cases > 0);
    assert!(
        !adopted.adopted_tasks.is_empty(),
        "issues: {:?}; proposal: {:?}",
        adopted.adoption_issues,
        adopted.adoption_proposal
    );
    assert!(adopted.incremental.as_ref().unwrap().adoption_authority);
    let session = f.project.task_session(None, None).unwrap();
    assert_eq!(session.states[&task].check, dctx0_tasks::CheckStatus::Pass);
    assert_eq!(session.states[&task].result.as_ref(), Some(&authored));
}

#[tokio::test]
async fn go_application_incremental_reuses_contract_cases() {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    let f = Fixture::new("workspace W { value B = Int { range 10..11 } }", "W.B");
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["target"] = "go".into();
    config["runtime"] = "go".into();
    config["history_retention"] = "preserve-all".into();
    fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    publish_application_test_library(&f.app, &dctx0_target_go::application_test_library()).unwrap();
    fs::write(f.app.join("go.mod"), "module example.test/fixture\n\ngo 1.25.0\n\nrequire github.com/Haardt/dctx0ctl/applicationtests/go v0.0.0\nreplace github.com/Haardt/dctx0ctl/applicationtests/go => ./.dctx0/application-tests/library\n").unwrap();
    fs::write(f.app.join("native_test.go"), r#"package fixture
import("encoding/json"; "fmt"; "os"; "testing"; "time"; app "github.com/Haardt/dctx0ctl/applicationtests/go")
func TestNative(t *testing.T) { c,e:=app.Dial(os.Getenv("DCTX0_APPLICATION_SOCKET"),1048576,30*time.Second);if e!=nil{t.Fatal(e)};defer c.Close();c.DecoderTest(t,func(raw []byte,_ app.TypeRef)(any,error){var v int;if json.Unmarshal(raw,&v)!=nil||v<10||v>11{return nil,app.Reject("invalid")};return v,nil},func(value any,_ app.TypeRef)(app.Value,error){b,_:=json.Marshal(fmt.Sprint(value));return app.Value{Kind:"integer",Value:b},nil});if _,e=c.Finish();e!=nil{t.Fatal(e)};t.Log("NATIVE_ASSERTION_EXECUTED")}
"#).unwrap();
    let dir = f.app.join(".dctx0/application-tests");
    fs::write(dir.join("recipe.json"),serde_json::to_vec(&serde_json::json!({"schema_version":1,"runtime":"go","root_test":"TestNative","test_directory":"root","prepare":[],"build":[{"program":"go","arguments":["test","-c","-trimpath","-buildvcs=false","-o","{artifacts}/native.test","."]}],"catalog":{"program":"{artifacts}/native.test","arguments":["-test.list=."]},"run":{"program":"go","arguments":["tool","test2json","-t","{artifacts}/native.test","-test.v","-test.run=^TestNative$"]},"junit":null})).unwrap()).unwrap();
    fs::write(f.app.join("reuse.json"),r#"{"schema_version":1,"deterministic_units":["contract:W.B"],"files":[],"environment":[]}"#).unwrap();
    f.project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/plan.json",
                group: "native",
                test: "TestNative",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(120),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    create_profiles(&f.app, &dctx0_target_go::Provider, true, false, false).unwrap();
    f.project.task_plan("latest").unwrap();
    let cancel = NativeCancellation::default();
    let run = |options: NativeReuseOptions| {
        let f = &f;
        let cancel = &cancel;
        async move {
            f.project
                .verify_application_tests_incremental(
                    &f.app,
                    ".dctx0/application-tests/plan.json",
                    "native",
                    ".dctx0/application-tests/recipe.json",
                    ".dctx0ctl/application-tests/report.json",
                    cancel,
                    &options,
                )
                .await
        }
    };
    let first = run(NativeReuseOptions {
        mode: NativeReuseMode::Off,
        inputs: "reuse.json".into(),
        parent: None,
    })
    .await
    .unwrap();
    assert_eq!(first.result, VerificationResult::Pass, "{:?}", first.issues);
    let parent = first.evidence.unwrap().id;
    let second = f
        .project
        .verify_application_tests_incremental_with_adoption(
            &f.app,
            ".dctx0/application-tests/plan.json",
            "native",
            ".dctx0/application-tests/recipe.json",
            ".dctx0ctl/application-tests/adopt.json",
            &cancel,
            &NativeReuseOptions {
                mode: Default::default(),
                inputs: "reuse.json".into(),
                parent: Some(parent.clone()),
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        second.result,
        VerificationResult::Pass,
        "{:?}",
        second.issues
    );
    assert!(second.incremental.as_ref().unwrap().reused_cases > 0);
    assert!(!second.adopted_tasks.is_empty(), "{second:#?}");
    assert!(second.incremental.as_ref().unwrap().adoption_authority);
    assert!(
        second
            .phases
            .iter()
            .find(|p| p.phase == "native-tests")
            .unwrap()
            .audit
            .stdout
            .contains("NATIVE_ASSERTION_EXECUTED")
    );
    assert!(
        second
            .evidence
            .unwrap()
            .run
            .artifacts
            .iter()
            .any(|a| a.media_type == dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE)
    );
    assert!(f.project.check().unwrap().ok);
    f.project
        .prepare_application_tests(
            &f.app,
            &ApplicationTestingPreparation {
                plan: ".dctx0/application-tests/plan.json",
                group: "native",
                test: "TestNative",
                form: TestForm::Decoder,
                boundary: ApplicationValueBoundary::JsonDecoder,
                group_seconds: Some(120),
                stateless_decoders: Default::default(),
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();
    let rechecked = run(NativeReuseOptions {
        mode: Default::default(),
        inputs: "reuse.json".into(),
        parent: Some(parent.clone()),
    })
    .await
    .unwrap();
    assert_eq!(rechecked.result, VerificationResult::Pass);
    assert!(rechecked.incremental.unwrap().reused_cases > 0);
    fs::write(
        f.app.join("native_test.go"),
        format!(
            "{}\n// source change",
            fs::read_to_string(f.app.join("native_test.go")).unwrap()
        ),
    )
    .unwrap();
    let changed = run(NativeReuseOptions {
        mode: NativeReuseMode::Off,
        inputs: "reuse.json".into(),
        parent: None,
    })
    .await
    .unwrap();
    assert_eq!(
        changed.result,
        VerificationResult::Pass,
        "{:?}",
        changed.issues
    );
    assert_eq!(changed.incremental.as_ref().unwrap().reused_cases, 0);
    assert!(
        !changed
            .evidence
            .unwrap()
            .run
            .artifacts
            .iter()
            .any(|a| a.media_type == dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE)
    );
}
