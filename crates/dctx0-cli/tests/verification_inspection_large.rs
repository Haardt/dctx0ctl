#![cfg(feature = "typescript")]
use dctx0_core::*;
use dctx0_engine::*;
use dctx0_target_typescript::BunHarness;
use std::{fs, process::Command};

#[tokio::test]
async fn large_inspection_groups_are_bound_complete_and_atomically_rejected() {
    let root =
        std::env::temp_dir().join(format!("dctx0-cli-large-inspection-{}", std::process::id()));
    let app = root.join("app");
    let spec = root.join("spec");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&spec).unwrap();
    let mut source = "workspace W { value A = Int { range 1..2 }\n".to_owned();
    for index in 0..48 {
        source.push_str(&format!("review Boundary{index} {{ inspect {{ implementation }}\ncriteria {{ \"Reject inputs outside the range.\" }} }}\n"));
    }
    source.push('}');
    fs::write(spec.join("w.dctx0"), source).unwrap();
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
    let mut production = fs::read_to_string(app.join("app.ts")).unwrap();
    fs::create_dir(app.join("sources")).unwrap();
    for index in 0..543 {
        let path = format!(
            "sources/module-{index:04}-descriptive-production-source-for-bound-inspection.ts"
        );
        fs::write(
            app.join(&path),
            format!("export const value{index} = {index};"),
        )
        .unwrap();
        production.push_str(&format!("\nimport './{path}';"));
    }
    fs::write(app.join("app.ts"), production).unwrap();
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
    assert!(native.evidence.run.provenance.implementation.len() >= 543);
    let native_artifacts = {
        let store =
            dctx0_store::Store::open(&project.database, dctx0_store::AccessMode::ReadOnly).unwrap();
        native
            .evidence
            .run
            .artifacts
            .iter()
            .map(|reference| {
                store
                    .verification_artifact(&native.evidence.id, &reference.hash)
                    .unwrap()
            })
            .collect::<Vec<_>>()
    };
    let reports = (0..47).map(|index| {
        let subject = format!("W.Boundary{index}");
        dctx0_verification::program_inspection::ProgramInspection {
            inspection: VerificationAttestation {
                schema_version:1,actor:"input-acceptance-fixture".into(),subject:subject.clone(),
                snapshot:native.evidence.run.provenance.snapshot.clone(),
                plan_hash:native.evidence.run.provenance.plan_hash.clone(),
                requirements:dctx0_verification::program_inspection::inspection_requirements(&plan,&subject).unwrap(),
                implementation_artifacts:native.evidence.run.provenance.implementation.clone(),
                report:"Fixture review of actual integer/range checks; no claim about Company Manager.".into(),
                limitations:vec!["Agent judgement in a transport acceptance fixture".into()],result:VerificationResult::Pass,
            },
            review:Some(dctx0_verification::program_inspection::InspectedReview { approved:true, findings:vec![] }),
        }
    }).collect::<Vec<_>>();
    let file = root.join("group.json");
    let invoke = |label: &str, action: &str, extra: &[&str]| {
        let binary = std::env::var_os("DCTX0_INSPECTION_BINARY")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_dctx0").into());
        let mut command = if let Some(metrics) = std::env::var_os("DCTX0_INSPECTION_METRICS") {
            let python = std::env::var_os("DCTX0_TEST_PYTHON").unwrap_or_else(|| "python3".into());
            let mut command = Command::new(python);
            command
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/inspection_cli_measure.py"
                ))
                .arg(metrics)
                .arg(label)
                .arg(binary);
            command
        } else {
            Command::new(binary)
        };
        command
            .args(["verify", action])
            .arg(&spec)
            .arg(&native.evidence.id)
            .arg("--project")
            .arg(&app)
            .arg("--file")
            .arg(&file)
            .args(["--json", "--full"])
            .args(extra)
            .output()
            .unwrap()
    };
    for count in [19, 47] {
        let bytes = serde_json::to_vec(&reports[..count]).unwrap();
        assert!(
            bytes.len() > 1_048_576,
            "{count} reports: {} bytes",
            bytes.len()
        );
        fs::write(&file, &bytes).unwrap();
        let output = invoke(&format!("inspect-{count}"), "inspect", &[]);
        // An unreviewed obligation intentionally keeps the total UNKNOWN.
        let report: InspectedVerificationReport = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| {
                panic!(
                    "{error}: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        assert!(matches!(
            report.assessment.result,
            VerificationResult::Unknown { .. }
        ));
        assert_eq!(report.evidence.run.cases, native.evidence.run.cases);
        for index in 0..count {
            let assessed = report
                .assessment
                .subjects
                .iter()
                .find(|subject| subject.subject == format!("W.Boundary{index}"))
                .unwrap();
            assert_eq!(assessed.result.effective, VerificationResult::Pass);
            assert!(matches!(
                assessed.result.automatic,
                VerificationResult::Unknown { .. }
            ));
        }
        assert!(matches!(
            report
                .assessment
                .subjects
                .iter()
                .find(|s| s.subject == "W.Boundary47")
                .unwrap()
                .result
                .effective,
            VerificationResult::Unknown { .. }
        ));
        let before = project.verification_history(0, 200).unwrap();
        let too_small = (bytes.len() - 1).to_string();
        let rejected = invoke("input-cap", "inspect", &["--max-input-bytes", &too_small]);
        assert!(!rejected.status.success());
        assert!(
            String::from_utf8_lossy(&rejected.stderr).contains("input exceeds")
                || String::from_utf8_lossy(&rejected.stdout).contains("input exceeds")
        );
        assert_eq!(project.verification_history(0, 200).unwrap(), before);
        let exact = bytes.len().to_string();
        let exact = invoke(
            &format!("inspect-{count}-exact-cap"),
            "inspect",
            &["--max-input-bytes", &exact],
        );
        let exact: InspectedVerificationReport = serde_json::from_slice(&exact.stdout).unwrap();
        assert_eq!(exact.evidence.id, report.evidence.id);
    }
    let before = project.verification_history(0, 200).unwrap();
    for mutation in ["source", "snapshot", "plan", "unknown", "duplicate"] {
        let mut invalid = reports.clone();
        match mutation {
            "source" => {
                invalid[0]
                    .inspection
                    .implementation_artifacts
                    .insert("app.ts".into(), "wrong".into());
            }
            "snapshot" => invalid[0].inspection.snapshot = "wrong".into(),
            "plan" => invalid[0].inspection.plan_hash = "wrong".into(),
            "unknown" => invalid[0].inspection.subject = "W.UnknownObligation".into(),
            _ => {
                let mut duplicate = invalid[0].clone();
                duplicate.inspection.result = VerificationResult::Fail {
                    issue: VerificationIssue {
                        code: "CONTRADICTORY".into(),
                        message: "Conflicting duplicate".into(),
                        span: None,
                    },
                };
                invalid.push(duplicate);
            }
        }
        fs::write(&file, serde_json::to_vec(&invalid).unwrap()).unwrap();
        let rejected = invoke(mutation, "inspect", &[]);
        assert!(!rejected.status.success());
        assert_eq!(project.verification_history(0, 200).unwrap(), before);
    }
    fs::write(&file, b"[invalid JSON").unwrap();
    let invalid = invoke("invalid-json", "inspect", &[]);
    assert!(!invalid.status.success());
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("Invalid Inspection JSON")
            || String::from_utf8_lossy(&invalid.stdout).contains("Invalid Inspection JSON")
    );
    let mut padded = vec![b' '; 1_048_576];
    padded.extend_from_slice(b"[]");
    fs::write(&file, &padded).unwrap();
    let attest = invoke("attest-over-old-cap", "attest", &[]);
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&attest.stdout),
        String::from_utf8_lossy(&attest.stderr)
    );
    assert!(
        message.contains("Nonempty inspected task reports"),
        "{message}"
    );
    assert_eq!(project.verification_history(0, 200).unwrap(), before);
    fs::File::create(&file)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    let huge = invoke("default-cap", "inspect", &[]);
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&huge.stdout),
        String::from_utf8_lossy(&huge.stderr)
    );
    assert!(message.contains("67108864 bytes"), "{message}");
    assert_eq!(project.verification_history(0, 200).unwrap(), before);
    let production = fs::read_to_string(app.join("app.ts")).unwrap();
    fs::write(
        app.join("app.ts"),
        production.replace("return value;", "return 0;"),
    )
    .unwrap();
    let failed = project
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
        failed
            .evidence
            .run
            .cases
            .iter()
            .any(|case| matches!(case.result, VerificationResult::Fail { .. }))
    );
    assert!(
        project
            .publish_program_inspections(Some(&app), &failed.evidence.id, &reports)
            .unwrap_err()
            .to_string()
            .contains("without failures")
    );
    for (index, kind) in ["missing-case", "duplicate-case", "incomplete-provenance"]
        .into_iter()
        .enumerate()
    {
        let mut corrupt = native.evidence.run.clone();
        corrupt.started_at_unix_ns = format!("900{index}");
        match kind {
            "missing-case" => {
                corrupt.cases.pop();
            }
            "duplicate-case" => corrupt.cases.push(corrupt.cases[0].clone()),
            _ => corrupt.provenance.incomplete.push(VerificationIssue {
                code: "INCOMPLETE_FIXTURE".into(),
                message: "Simulated incomplete stored provenance".into(),
                span: None,
            }),
        }
        // Deliberately corrupted history fixtures reuse the actual captured
        // observations; they never represent new executed evidence.
        let saved = dctx0_store::Store::open(&project.database, dctx0_store::AccessMode::ReadWrite)
            .unwrap()
            .record_verification_run(&project.key(), &corrupt, &native_artifacts);
        if kind == "duplicate-case" {
            assert!(saved.unwrap_err().to_string().contains("Duplicate"));
            continue;
        }
        let saved = saved.unwrap();
        let before = project.verification_history(0, 200).unwrap();
        assert!(
            project
                .publish_program_inspections(Some(&app), &saved.id, &reports)
                .unwrap_err()
                .to_string()
                .contains("complete actual native evidence"),
            "{kind}"
        );
        assert_eq!(project.verification_history(0, 200).unwrap(), before);
    }
    fs::remove_dir_all(root).unwrap();
}
