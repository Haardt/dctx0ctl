use dctx0_core::VerificationResult;
use dctx0_engine::*;
use dctx0_target_typescript::BunHarness;
use dctx0_verification::application_plan::TestForm;
use dctx0_verification::application_session::ApplicationValueBoundary;
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
static RUN: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    app: PathBuf,
    project: Project,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-incremental-hybrid-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let app = root.join("app");
        let spec = root.join("spec");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir(&spec).unwrap();
        fs::write(
            spec.join("w.dctx0"),
            "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } }",
        )
        .unwrap();
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
        let config_path = app.join(".dctx0/testing/verification.json");
        let mut config: serde_json::Value =
            serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        config["history_retention"] = "preserve-all".into();
        fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
        fs::create_dir_all(app.join(".dctx0ctl")).unwrap();
        let legacy =
            serde_json::to_string(&app.join(".dctx0ctl/legacy-calls").to_string_lossy()).unwrap();
        let native =
            serde_json::to_string(&app.join(".dctx0ctl/native-calls").to_string_lossy()).unwrap();
        fs::write(app.join("app.ts"), format!(r#"import {{appendFileSync}} from 'node:fs';
export function factory(value){{appendFileSync({legacy},'a\n');if(!Number.isSafeInteger(value)||value<1||value>2)throw TypeError('A rejected');return value;}}
export function decodeB(raw){{appendFileSync({native},'b\n');const value=JSON.parse(new TextDecoder('utf-8',{{fatal:true}}).decode(raw));if(!Number.isSafeInteger(value)||value<10||value>11)throw TypeError('B rejected');return value;}}"#)).unwrap();
        fs::write(app.join(".dctx0/testing/typescript/adapter.ts"), r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){if(subject!=='W.A')throw Error('native B reached legacy');let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#).unwrap();
        publish_application_test_library(
            &app,
            &dctx0_target_typescript::application_test_library(),
        )
        .unwrap();
        let dir = app.join(".dctx0/application-tests");
        fs::write(dir.join("native-b.test.ts"), r#"import {test} from 'bun:test';
import {AssessmentClient,decoderTest} from './library/index.ts';
import {decodeB} from '../../app.ts';
test('NativeB',async()=>{const client=await AssessmentClient.connect(process.env.DCTX0_APPLICATION_SOCKET!);try{await decoderTest(client,'NativeB',(raw)=>{try{return {kind:'accepted',value:decodeB(raw)};}catch(error){if(error instanceof TypeError||error instanceof SyntaxError)return {kind:'rejected',reason:error.message};throw error;}},value=>({kind:'integer',value:String(value)}));await client.finish();}finally{client.close();}});"#).unwrap();
        let recipe = serde_json::json!({"schema_version":1,"runtime":"bun","root_test":"NativeB","test_directory":"root","entrypoint":".dctx0/application-tests/native-b.test.ts","prepare":[],"build":[{"program":"bun","arguments":["build","--target=bun","--external=bun:test","--outdir","{artifacts}/build",".dctx0/application-tests/native-b.test.ts"]}],"catalog":{"program":"bun","arguments":[".dctx0/application-tests/library/catalog.ts",".dctx0/application-tests/native-b.test.ts"]},"run":{"program":"bun","arguments":["test","--reporter","junit","--reporter-outfile","{artifacts}/native.junit.xml","./.dctx0/application-tests/native-b.test.ts"]},"junit":".dctx0ctl/application-tests/native.junit.xml"});
        fs::write(
            dir.join("recipe.json"),
            serde_json::to_vec(&recipe).unwrap(),
        )
        .unwrap();
        fs::write(app.join(".dctx0/reuse.json"), r#"{"schema_version":1,"deterministic_units":["contract:W.A"],"files":[".dctx0ctl/revision"]}"#).unwrap();
        fs::write(app.join(".dctx0ctl/revision"), "one").unwrap();
        project
            .prepare_application_tests(
                &app,
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
        Self { root, app, project }
    }
    async fn run(&self, parent: Option<String>) -> NativeVerificationReport {
        let report = format!(
            ".dctx0ctl/application-tests/native-{}.json",
            RUN.fetch_add(1, Ordering::Relaxed)
        );
        self.project
            .run_hybrid_verification_incremental(
                &self.app,
                &HybridVerificationOptions {
                    plan: ".dctx0/application-tests/hybrid.json",
                    application_group: "native",
                    recipe: ".dctx0/application-tests/recipe.json",
                    native_report: &report,
                },
                &BunHarness,
                &NativeCancellation::default(),
                &NativeReuseOptions {
                    mode: if parent.is_some() {
                        NativeReuseMode::Auto
                    } else {
                        NativeReuseMode::Off
                    },
                    inputs: ".dctx0/reuse.json".into(),
                    parent,
                },
            )
            .await
            .unwrap()
    }
    async fn adopt(&self, parent: Option<String>) -> NativeVerificationReport {
        let report = format!(
            ".dctx0ctl/application-tests/adopt-{}.json",
            RUN.fetch_add(1, Ordering::Relaxed)
        );
        self.project
            .run_hybrid_verification_incremental_with_adoption(
                &self.app,
                &HybridVerificationOptions {
                    plan: ".dctx0/application-tests/hybrid.json",
                    application_group: "native",
                    recipe: ".dctx0/application-tests/recipe.json",
                    native_report: &report,
                },
                &BunHarness,
                &NativeCancellation::default(),
                &NativeReuseOptions {
                    mode: if parent.is_some() {
                        NativeReuseMode::Auto
                    } else {
                        NativeReuseMode::Off
                    },
                    inputs: ".dctx0/reuse.json".into(),
                    parent,
                },
                false,
            )
            .await
            .unwrap()
    }
    fn plan_tasks(&self) -> (i64, i64) {
        create_profiles(
            &self.app,
            &dctx0_target_typescript::Provider,
            true,
            false,
            false,
        )
        .unwrap();
        self.project.task_plan("latest").unwrap();
        let session = self.project.task_session(None, None).unwrap();
        let id = |subject: &str| {
            session
                .plan
                .members
                .iter()
                .find(|member| {
                    member.candidate.kind == dctx0_tasks::TaskKind::Contract
                        && member.candidate.subjects.contains(&subject.to_string())
                })
                .unwrap()
                .task_id
        };
        (id("W.A"), id("W.B"))
    }
    fn count(&self, name: &str) -> usize {
        fs::read_to_string(self.app.join(".dctx0ctl").join(name))
            .unwrap()
            .lines()
            .count()
    }
}

#[tokio::test]
async fn hybrid_incremental_delivers_guarded_tasks_with_reused_adapter_and_fresh_application() {
    let f = Fixture::new();
    let config_path = f.app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["environment"] =
        serde_json::json!({"mode":"controlled","inherit":["PATH","HOME","TMPDIR"]});
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let (a, b) = f.plan_tasks();
    let first = f.run(None).await;
    assert_eq!(first.evidence.run.result, VerificationResult::Pass);
    let calls = f.count("legacy-calls");
    let report = f.adopt(Some(first.evidence.id)).await;
    assert_eq!(
        report.evidence.run.result,
        VerificationResult::Pass,
        "{report:#?}"
    );
    assert!(report.incremental.as_ref().unwrap().reused_cases > 0);
    assert_eq!(f.count("legacy-calls"), calls);
    assert!(
        report.adopted_tasks.contains(&a) && report.adopted_tasks.contains(&b),
        "{report:#?}"
    );
    assert!(report.incremental.as_ref().unwrap().adoption_authority);
    let session = f.project.task_session(None, None).unwrap();
    for id in [a, b] {
        assert_eq!(session.states[&id].check, dctx0_tasks::CheckStatus::Pass);
    }
}

#[tokio::test]
async fn hybrid_incremental_bootstrap_delivers_from_a_fresh_complete_cohort() {
    let f = Fixture::new();
    let (a, b) = f.plan_tasks();
    let report = f.adopt(None).await;
    assert_eq!(
        report.evidence.run.result,
        VerificationResult::Pass,
        "{report:#?}"
    );
    assert_eq!(report.incremental.as_ref().unwrap().reused_cases, 0);
    assert!(
        report.adopted_tasks.contains(&a) && report.adopted_tasks.contains(&b),
        "{report:#?}"
    );
}

#[tokio::test]
async fn hybrid_incremental_adds_pass_to_authored_done_without_replacing_result() {
    let f = Fixture::new();
    let (a, b) = f.plan_tasks();
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
    let result = dctx0_tasks::TaskResult {
        summary: "Authored implementation".into(),
        artifacts: vec!["app.ts".into()],
        decisions: vec![],
        implementation: None,
    };
    f.project
        .task_action(dctx0_tasks::Action::Complete {
            task: a,
            actor: "author".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            result: result.clone(),
        })
        .unwrap();
    let first = f.run(None).await;
    let report = f.adopt(Some(first.evidence.id)).await;
    assert!(
        report.adopted_tasks.contains(&a) && report.adopted_tasks.contains(&b),
        "{report:#?}"
    );
    let session = f.project.task_session(None, None).unwrap();
    assert_eq!(session.states[&a].work, dctx0_tasks::WorkState::Done);
    assert_eq!(session.states[&a].check, dctx0_tasks::CheckStatus::Pass);
    assert_eq!(session.states[&a].result.as_ref(), Some(&result));
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn hybrid_reuse_off_executes_both_backends_with_project_wide_declarations() {
    let f = Fixture::new();
    fs::write(
        f.app.join(".dctx0/reuse.json"),
        r#"{"schema_version":1,"deterministic_units":["contract:W.A","contract:W.B"],"files":[".dctx0ctl/revision"]}"#,
    )
    .unwrap();
    let mut previous = (0, 0);
    for attempt in 0..2 {
        let report = format!(".dctx0ctl/application-tests/fresh-{attempt}.json");
        let result = f
            .project
            .run_hybrid_verification_incremental(
                &f.app,
                &HybridVerificationOptions {
                    plan: ".dctx0/application-tests/hybrid.json",
                    application_group: "native",
                    recipe: ".dctx0/application-tests/recipe.json",
                    native_report: &report,
                },
                &BunHarness,
                &NativeCancellation::default(),
                &NativeReuseOptions {
                    mode: NativeReuseMode::Off,
                    inputs: ".dctx0/reuse.json".into(),
                    parent: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.evidence.run.result, VerificationResult::Pass);
        let summary = result.incremental.unwrap();
        assert_eq!(summary.reused_cases, 0);
        assert_eq!(summary.executed_cases, summary.required_cases);
        let calls = (f.count("legacy-calls"), f.count("native-calls"));
        assert!(calls.0 > previous.0 && calls.1 > previous.1);
        previous = calls;
    }
}

#[tokio::test]
async fn hybrid_reuses_legacy_cases_but_always_runs_native_and_invalidates_changes() {
    let f = Fixture::new();
    let first = f.run(None).await;
    assert_eq!(
        first.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        first.evidence.run.provenance.incomplete
    );
    let legacy = f.count("legacy-calls");
    let native = f.count("native-calls");
    assert!(legacy > 0 && native > 0);
    let second = f.run(Some(first.evidence.id.clone())).await;
    assert_eq!(
        second.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        second.evidence.run.provenance.incomplete
    );
    assert_eq!(
        f.count("legacy-calls"),
        legacy,
        "legacy callbacks should be skipped"
    );
    assert!(
        f.count("native-calls") > native,
        "native root must still execute"
    );
    let cases = &second.evidence.run.cases;
    let unique: BTreeSet<_> = cases.iter().map(|c| &c.id).collect();
    assert_eq!(unique.len(), cases.len());
    assert_eq!(cases, &first.evidence.run.cases);
    let summary = second.incremental.as_ref().unwrap();
    assert!(summary.reused_cases > 0 && summary.executed_cases > 0);
    assert_eq!(summary.reused_cases + summary.executed_cases, cases.len());
    assert_eq!(summary.required_cases, cases.len());
    assert!(!summary.adoption_authority && second.adopted_tasks.is_empty());
    fs::write(f.app.join(".dctx0ctl/revision"), "two").unwrap();
    let third = f.run(Some(second.evidence.id)).await;
    assert_eq!(third.evidence.run.result, VerificationResult::Pass);
    assert!(third.incremental.unwrap().reused_cases > 0);
    assert_eq!(f.count("legacy-calls"), legacy);
    let after_revision = f.count("legacy-calls");
    fs::write(
        f.app.join("app.ts"),
        format!(
            "{}\n// changed implementation",
            fs::read_to_string(f.app.join("app.ts")).unwrap()
        ),
    )
    .unwrap();
    let fourth = f.run(None).await;
    assert_eq!(fourth.evidence.run.result, VerificationResult::Pass);
    assert_eq!(fourth.incremental.unwrap().reused_cases, 0);
    assert!(f.count("legacy-calls") > after_revision);
    let after_source = f.count("legacy-calls");
    let native_test = f.app.join(".dctx0/application-tests/native-b.test.ts");
    fs::write(
        &native_test,
        format!(
            "{}\n// changed native test",
            fs::read_to_string(&native_test).unwrap()
        ),
    )
    .unwrap();
    let fifth = f.run(None).await;
    assert_eq!(fifth.evidence.run.result, VerificationResult::Pass);
    assert_eq!(fifth.incremental.unwrap().reused_cases, 0);
    assert!(f.count("legacy-calls") > after_source);
}
