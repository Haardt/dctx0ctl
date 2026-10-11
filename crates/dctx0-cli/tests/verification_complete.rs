#![cfg(feature = "typescript")]
use dctx0_core::*;
use dctx0_engine::*;
use dctx0_target_typescript::BunHarness;
use dctx0_verification::program_inspection::InspectedReview;
use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};

struct Fixture {
    root: PathBuf,
    app: PathBuf,
    spec: PathBuf,
    project: Project,
    notes: AuthoredInspections,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("dctx0-cli-complete-{}", std::process::id()));
        let app = root.join("app");
        let spec = root.join("spec");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&spec).unwrap();
        fs::write(spec.join("w.dctx0"), "workspace W { value A = Int { range 1..2 }\nproof TypeConformance\nreview Boundary { inspect { implementation }\ncriteria { \"Reject inputs outside the range.\" } } }").unwrap();
        let project = Project::new(&spec, None).unwrap();
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
        fs::write(app.join("app.ts"), "export function factory(value:unknown){if(typeof value!=='number'||!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
        fs::write(app.join(".dctx0/testing/typescript/adapter.ts"), r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'bounded exact number'}};},async begin(){return {async invoke(){throw Error('unavailable');},async validate(_subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#).unwrap();
        let reviewed_hashes = ["app.ts", ".dctx0/testing/typescript/adapter.ts"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    dctx0_store::VerificationArtifact::new(
                        "application/octet-stream",
                        fs::read(app.join(name)).unwrap(),
                    )
                    .reference
                    .hash,
                )
            })
            .collect();
        let mut notes = AuthoredInspections {
            schema_version: 1, actor: "acceptance-reviewer".into(), scope: "W: bounded factory and explicit boundary review".into(), reviewed_hashes,
            reviewed_spec_hashes: BTreeMap::from([("w.dctx0".into(), dctx0_store::VerificationArtifact::new("application/octet-stream", fs::read(spec.join("w.dctx0")).unwrap()).reference.hash)]),
            subjects: BTreeMap::from([("W.Boundary".into(), AuthoredInspectionNote {
                report: "Inspected the real factory: integer and range rejection agree with the contract.".into(), limitations: vec!["Explicit source review, not an automatic proof.".into()], review: Some(InspectedReview { approved: true, findings: vec![] }),
            })]),
        };
        notes.subjects.insert(
            "W.A".into(),
            AuthoredInspectionNote {
                report: "Reviewed the actual bounded integer representation and native factory."
                    .into(),
                limitations: vec!["Explicit inspection".into()],
                review: None,
            },
        );
        notes.subjects.insert(
            "W.TypeConformance".into(),
            AuthoredInspectionNote {
                report:
                    "Reviewed type conformance against actual accepted and rejected factory values."
                        .into(),
                limitations: vec!["Explicit proof-suite inspection".into()],
                review: None,
            },
        );
        notes.subjects.get_mut("W.Boundary").unwrap().review.as_mut().unwrap().findings.push(dctx0_verification::program_inspection::InspectionFinding {
            id: "finite-boundary".into(), severity: dctx0_verification::program_inspection::FindingSeverity::Low,
            description: "The corpus is finite; exhaustive claims remain limited to the declared bounded range.".into(),
        });
        Self {
            root,
            app,
            spec,
            project,
            notes,
        }
    }
    fn run(&self, json: bool) -> std::process::Output {
        let file = self.root.join("notes.json");
        fs::write(&file, serde_json::to_vec(&self.notes).unwrap()).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_dctx0"));
        command
            .args(["verify", "complete"])
            .arg(&self.spec)
            .arg("--project")
            .arg(&self.app)
            .arg("--inspections")
            .arg(file)
            // The validation process already owns the collective memory cap.
            .env("DCTX0_COMPLETE_MEMORY_GUARDED", "1");
        if json {
            command.arg("--json");
        }
        command.output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn complete_cli_executes_real_cases_delivers_reviews_and_rejects_stale_notes_and_native_failures() {
    let mut fixture = Fixture::new();
    let review = fixture.notes.subjects.remove("W.Boundary").unwrap();
    let missing = fixture.run(true);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("Effective assessment incomplete"));
    assert!(
        fixture
            .project
            .task_session(None, None)
            .unwrap()
            .states
            .values()
            .any(|state| state.work != dctx0_tasks::WorkState::Done)
    );
    fixture.notes.subjects.insert("W.Boundary".into(), review);
    // No current head or task plan is needed: complete publishes them explicitly.
    let first = fixture.run(true);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let report: CompleteVerificationReport = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report.result, VerificationResult::Pass);
    assert!(matches!(
        report.native_result,
        VerificationResult::Unknown { .. }
    ));
    assert!(report.tasks > 0 && report.mandatory_cases > 0);
    assert_eq!((report.open_tasks, report.non_pass_tasks), (0, 0));
    assert!(!report.inspected_tasks.is_empty());
    let native = fixture
        .project
        .saved_verification_run(&report.native_run)
        .unwrap();
    assert_eq!(native.run.origin, VerificationRunOrigin::Native);
    let effective = fixture
        .project
        .saved_verification_run(&report.inspected_run)
        .unwrap();
    assert_eq!(effective.run.origin, VerificationRunOrigin::Imported);
    assert_eq!(native.run.cases, effective.run.cases);
    assert_eq!(
        fixture
            .project
            .task_session(None, None)
            .unwrap()
            .states
            .values()
            .filter(|s| s.work == dctx0_tasks::WorkState::Done
                && s.check == dctx0_tasks::CheckStatus::Pass)
            .count(),
        report.tasks
    );
    let repeated = fixture.run(false);
    assert!(
        repeated.status.success(),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    assert!(String::from_utf8_lossy(&repeated.stdout).contains("unfinished: 0"));
    assert_eq!(
        fixture
            .project
            .saved_verification_run(&report.native_run)
            .unwrap(),
        native
    );
    let history = fixture.project.history().unwrap().len();
    let original_spec = fs::read(fixture.spec.join("w.dctx0")).unwrap();
    fs::write(
        fixture.spec.join("w.dctx0"),
        "workspace W { value A = Int { range 1..3 } }",
    )
    .unwrap();
    let changed_spec = fixture.run(true);
    assert!(!changed_spec.status.success());
    assert!(
        String::from_utf8_lossy(&changed_spec.stderr)
            .contains("New specification inspection required")
    );
    assert_eq!(fixture.project.history().unwrap().len(), history);
    fs::write(fixture.spec.join("w.dctx0"), original_spec).unwrap();
    fs::write(
        fixture.app.join("app.ts"),
        "export function factory(value:unknown){return value;}",
    )
    .unwrap();
    let stale = fixture.run(true);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("New source inspection required"));
    assert_eq!(fixture.project.history().unwrap().len(), history);
    // Even a new explicit approval cannot overwrite a real native violation.
    fixture.notes.reviewed_hashes.insert(
        "app.ts".into(),
        dctx0_store::VerificationArtifact::new(
            "application/octet-stream",
            fs::read(fixture.app.join("app.ts")).unwrap(),
        )
        .reference
        .hash,
    );
    let violated = fixture.run(true);
    assert!(!violated.status.success());
    assert!(
        String::from_utf8_lossy(&violated.stderr).contains("Complete native evidence required")
    );
    assert_eq!(
        fixture
            .project
            .saved_verification_run(&report.native_run)
            .unwrap(),
        native
    );
}
