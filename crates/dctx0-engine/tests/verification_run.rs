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
    assert_eq!(changed.adoption_issues[0].code, "ADOPTION_UNAVAILABLE");
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
    let mut f = Fixture::new("workspace W { type A = Int }", "W.A");
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
    f.app = reference.canonicalize().unwrap();
    let report = f.run(true).await;
    let run = &report.evidence.run;
    assert!(
        matches!(run.result, VerificationResult::Unknown { .. }),
        "{:?}",
        run.result
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
    assert_eq!(operations.len(), 100);
    let reviewed = run
        .cases
        .iter()
        .filter(|c| c.kind == VerificationCaseKind::Operation && c.id.contains(":contract:"))
        .collect::<Vec<_>>();
    assert_eq!(reviewed.len(), 6);
    assert!(reviewed.iter().all(|c|matches!(&c.result,VerificationResult::Unknown{issue} if matches!(issue.code.as_str(), "OPERATION_EFFECT_PENDING" | "STATE_RESULT_PROJECTION_REQUIRED"))),"{reviewed:?}");
    assert!(
        reviewed
            .iter()
            .all(|c| !c.states.is_empty() && !c.observations.is_empty())
    );
    assert!(
        operations
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
            .any(|i| i.code == "OPERATION_CLOSURE_INCOMPLETE")
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
