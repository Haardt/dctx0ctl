#![cfg(all(target_os = "linux", feature = "typescript"))]
use dctx0_core::*;
use dctx0_engine::*;
use dctx0_target_typescript::BunHarness;
use std::{
    fs,
    path::PathBuf,
    process::Command,
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
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
const VALUE_ADAPTER: &str = r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#;

fn hybrid_fixture(spec: &str) -> std::mem::ManuallyDrop<Fixture> {
    use dctx0_verification::application_plan::TestForm;
    use dctx0_verification::application_session::ApplicationValueBoundary;
    let f = std::mem::ManuallyDrop::new(Fixture::new(spec, "W"));
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
                subjects: ["W.B".into()].into(),
            },
        )
        .unwrap();

    f
}

#[test]
fn hybrid_cli_runs_both_backends_and_preserves_existing_native_reports() {
    let f = hybrid_fixture(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } }",
    );
    let invoke = |name: &str, json: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dctx0"));
        command
            .args(["verify", "application-tests", "hybrid"])
            .arg(f.root.join("spec"))
            .arg("--project")
            .arg(&f.app)
            .args([
                "--plan",
                ".dctx0/application-tests/hybrid.json",
                "--group",
                "native",
                "--recipe",
                ".dctx0/application-tests/recipe.json",
                "--report",
                name,
                "--no-adopt",
            ]);
        if json {
            command.arg("--json");
        }
        command.output().unwrap()
    };
    let report_path = ".dctx0ctl/application-tests/cli-native.json";
    let first = invoke(report_path, true);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let result: NativeVerificationReport = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(result.evidence.run.result, VerificationResult::Pass);
    assert!(result.adopted_tasks.is_empty());
    let saved = &result.evidence;
    let native_case = saved
        .run
        .cases
        .iter()
        .find(|case| case.subject == "W.B")
        .unwrap();
    let adapter_case = saved
        .run
        .cases
        .iter()
        .find(|case| case.subject == "W.A")
        .unwrap();
    let before_history = f.project.verification_history(0, 200).unwrap();
    let before_events = f
        .project
        .task_session(None, None)
        .ok()
        .map(|session| session.event_seq);
    let replay = |case: &str| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["verify", "replay"])
            .arg(f.root.join("spec"))
            .arg(&saved.id)
            .args(["--case", case, "--project"])
            .arg(&f.app)
            .output()
            .unwrap()
    };
    let wrong_backend = replay(&native_case.id);
    assert!(!wrong_backend.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_backend.stderr).contains("verify application-tests replay")
    );
    assert_eq!(
        f.project.verification_history(0, 200).unwrap().len(),
        before_history.len()
    );
    assert_eq!(
        f.project
            .task_session(None, None)
            .ok()
            .map(|session| session.event_seq),
        before_events
    );
    let adapter_replay = replay(&adapter_case.id);
    assert!(
        adapter_replay.status.success(),
        "{}",
        String::from_utf8_lossy(&adapter_replay.stderr)
    );
    let replay_text = String::from_utf8_lossy(&adapter_replay.stdout);
    assert!(
        replay_text.contains("Selected case result: Some(Pass)"),
        "{replay_text}"
    );
    assert!(replay_text.contains("no task adoption"));
    assert_eq!(
        f.project.verification_history(0, 200).unwrap().len(),
        before_history.len() + 1
    );
    assert_eq!(
        f.project
            .task_session(None, None)
            .ok()
            .map(|session| session.event_seq),
        before_events
    );
    let bytes = fs::read(f.app.join(report_path)).unwrap();
    let duplicate = invoke(report_path, false);
    assert!(!duplicate.status.success());
    assert_eq!(bytes, fs::read(f.app.join(report_path)).unwrap());
    let second = invoke(".dctx0ctl/application-tests/cli-native-text.json", false);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stdout).contains("Fresh hybrid assessment: Pass"));
}

#[test]
fn assigned_adapter_group_executes_only_its_cases_and_cannot_deliver_tasks() {
    let f = hybrid_fixture(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } }",
    );
    let invoke = |group: &str| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["verify", "application-tests", "adapter-group"])
            .arg(f.root.join("spec"))
            .arg("--project")
            .arg(&f.app)
            .args([
                "--plan",
                ".dctx0/application-tests/hybrid.json",
                "--group",
                group,
                "--json",
            ])
            .output()
            .unwrap()
    };
    let output = invoke("legacy");
    assert!(
        !output.status.success(),
        "A partial cohort cannot claim acceptance"
    );
    let report: NativeVerificationReport = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stderr)));
    assert!(report.adopted_tasks.is_empty());
    assert!(!report.evidence.run.cases.is_empty());
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
            .all(|case| case.subject == "W.A" && case.result == VerificationResult::Pass)
    );
    assert!(
        report
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "HYBRID_COHORT_REQUIRED")
    );
    assert!(!invoke("native").status.success());
    assert!(!invoke("absent").status.success());
}

#[test]
fn hybrid_complete_cli_requires_current_authored_notes_and_delivers_both_backends() {
    let f = hybrid_fixture(
        "workspace W { value A = Int { range 1..2 } value B = Int { range 10..11 } proof Approval }",
    );
    let hash = |path: &std::path::Path| {
        dctx0_store::VerificationArtifact::new("application/octet-stream", fs::read(path).unwrap())
            .reference
            .hash
    };
    let mut notes = serde_json::json!({
        "schema_version":1,"actor":"test-reviewer","scope":"Explicit fixture proof Approval inspection",
        "reviewed_hashes":{"app.ts":hash(&f.app.join("app.ts"))},
        "reviewed_spec_hashes":{"w.dctx0":hash(&f.root.join("spec/w.dctx0"))},
        "subjects":{"W.Approval":{"report":"Inspected this fixture approval obligation against its current source", "limitations":["Fixture-only authored judgement"],"review":{"approved":true,"findings":[]}}}
    });
    let note_path = f.root.join("authored-notes.json");
    fs::write(&note_path, serde_json::to_vec(&notes).unwrap()).unwrap();
    let invoke = |name: &str, json: bool, plan_hash: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dctx0"));
        command
            .args(["verify", "application-tests", "hybrid-complete"])
            .arg(f.root.join("spec"))
            .arg("--project")
            .arg(&f.app)
            .args([
                "--plan",
                ".dctx0/application-tests/hybrid.json",
                "--group",
                "native",
                "--recipe",
                ".dctx0/application-tests/recipe.json",
                "--report",
                name,
            ])
            .arg("--notes")
            .arg(&note_path);
        if let Some(plan_hash) = plan_hash {
            command.args(["--expected-plan-sha256", plan_hash]);
        }
        if json {
            command.arg("--json");
        }
        command.output().unwrap()
    };
    use sha2::{Digest, Sha256};
    let plan = fs::read(f.app.join(".dctx0/application-tests/hybrid.json")).unwrap();
    let plan_hash = format!("{:x}", Sha256::digest(&plan));
    let invalid_pin = invoke(
        ".dctx0ctl/application-tests/cli-bad-pin.json",
        false,
        Some(&"0".repeat(64)),
    );
    assert!(!invalid_pin.status.success());
    assert!(String::from_utf8_lossy(&invalid_pin.stderr).contains("SHA-256 changed"));
    assert!(
        !f.app
            .join(".dctx0ctl/application-tests/cli-bad-pin.json")
            .exists()
    );
    assert!(f.project.task_session(None, None).is_err());
    let first = invoke(".dctx0ctl/application-tests/cli-complete.json", true, None);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let report: CompleteVerificationReport = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report.result, VerificationResult::Pass);
    assert_eq!(report.open_tasks, 0);
    assert!(!report.adopted_tasks.is_empty());
    let second = invoke(
        ".dctx0ctl/application-tests/cli-complete-text.json",
        false,
        Some(&plan_hash),
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stdout).contains("Complete hybrid acceptance: Pass"));
    let before = f.project.task_session(None, None).unwrap().event_seq;
    notes["reviewed_hashes"]["app.ts"] = serde_json::json!("0".repeat(64));
    fs::write(&note_path, serde_json::to_vec(&notes).unwrap()).unwrap();
    let invalid = invoke(
        ".dctx0ctl/application-tests/cli-stale.json",
        false,
        Some(&plan_hash),
    );
    assert!(!invalid.status.success());
    assert!(
        !f.app
            .join(".dctx0ctl/application-tests/cli-stale.json")
            .exists()
    );
    assert_eq!(
        f.project.task_session(None, None).unwrap().event_seq,
        before
    );
}
