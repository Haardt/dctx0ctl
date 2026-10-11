#![cfg(feature = "typescript")]
use dctx0_core::*;
use dctx0_engine::*;
use dctx0_target_typescript::BunHarness;
use std::{fs, process::Command};

#[tokio::test]
async fn inspection_cli_retains_automatic_unknown_and_rejects_changed_implementation() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-inspection-{}", std::process::id()));
    let app = root.join("app");
    let spec = root.join("spec");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&spec).unwrap();
    fs::write(spec.join("w.dctx0"),"workspace W { value A = Int { range 1..2 }\nreview Boundary { inspect { implementation }\ncriteria { \"Reject inputs outside the range.\" } } }").unwrap();
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
    fs::write(app.join("app.ts"),"export function factory(value:unknown){if(typeof value!=='number'||!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}").unwrap();
    fs::write(app.join(".dctx0/testing/typescript/adapter.ts"),r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'bounded exact number'}};},async begin(){return {async invoke(){throw Error('unavailable');},async validate(_subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#).unwrap();
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
    assert!(matches!(
        native.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    let plan = project
        .verification_plan(
            VerificationSelection {
                subjects: ["W".into()].into(),
                ..Default::default()
            },
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    let inspection = dctx0_verification::program_inspection::ProgramInspection {
        inspection: VerificationAttestation {
            schema_version: 1,
            actor: "cli-reviewer".into(),
            subject: "W.Boundary".into(),
            snapshot: native.evidence.run.provenance.snapshot.clone(),
            plan_hash: native.evidence.run.provenance.plan_hash.clone(),
            requirements: dctx0_verification::program_inspection::inspection_requirements(
                &plan,
                "W.Boundary",
            )
            .unwrap(),
            implementation_artifacts: native.evidence.run.provenance.implementation.clone(),
            report: "Inspected the exported factory and its actual native rejection cases.".into(),
            limitations: vec!["Agent judgement".into()],
            result: VerificationResult::Pass,
        },
        review: Some(dctx0_verification::program_inspection::InspectedReview {
            approved: true,
            findings: vec![],
        }),
    };
    let file = root.join("inspections.json");
    fs::write(&file, serde_json::to_vec(&[inspection]).unwrap()).unwrap();
    for json in [true, false] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dctx0"));
        command
            .args(["verify", "inspect"])
            .arg(&spec)
            .arg(&native.evidence.id)
            .arg("--project")
            .arg(&app)
            .arg("--file")
            .arg(&file);
        if json {
            command.arg("--json");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if json {
            let report: InspectedVerificationReport =
                serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(report.assessment.result, VerificationResult::Pass);
            assert_eq!(report.evidence.run.origin, VerificationRunOrigin::Imported);
        } else {
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains("No new execution or task adoption")
            );
        }
    }
    assert_eq!(
        project.saved_verification_run(&native.evidence.id).unwrap(),
        native.evidence
    );
    fs::write(
        app.join("app.ts"),
        "export function factory(value:unknown){return value;}",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(["verify", "inspect"])
        .arg(&spec)
        .arg(&native.evidence.id)
        .arg("--project")
        .arg(&app)
        .arg("--file")
        .arg(&file)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("changed since execution"));
    fs::remove_dir_all(root).unwrap();
}
