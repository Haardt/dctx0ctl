#![cfg(feature = "typescript")]
use dctx0_core::*;
use std::{fs, process::Command};

#[test]
fn cli_lifecycle_replay_runs_real_sessions_detects_mutations_and_preserves_saved_history() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-lifecycle-{}", std::process::id()));
    let spec = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir(&app).unwrap();
    fs::write(
        spec.join("w.dctx0"),
        r#"workspace W {
type Status = Open | Closed
entity Item { id:String
status:Status
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
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let call = |args: &[&str]| Command::new(binary).args(args).output().unwrap();
    let input = spec.to_str().unwrap();
    let application = app.to_str().unwrap();
    assert!(call(&["check", input]).status.success());
    let initialized = call(&[
        "verify",
        "init",
        input,
        "--project",
        application,
        "--target",
        "typescript",
        "--subject",
        "W.Flow",
        "--json",
    ]);
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let planned = call(&["verify", "plan", input, "--json"]);
    let plan: dctx0_verification::ProgramPlan = serde_json::from_slice(&planned.stdout).unwrap();
    let path = plan.lifecycles[0].paths[0].id.clone();
    let production = "export function prepare(item:any,id:string){item.id=id;item.status='Open';item.present=true;return 1;} export function finish(item:any,note:string){item.status='Closed';return note.length;}";
    fs::write(app.join("app.ts"), production).unwrap();
    fs::write(app.join(".dctx0/testing/typescript/adapter.ts"), r#"import {prepare,finish} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.Api.prepare',role:'operation',artifact:'app.ts',entry_point:'prepare'},{subject:'W.Api.finish',role:'operation',artifact:'app.ts',entry_point:'finish'}],artifacts:['app.ts'],capabilities:['sessions','invocation','state','events'],representations:{String:'string',Int:'safe integer'}};},async begin(){const item={id:'',status:'Open',present:false};return {async invoke(i){return {kind:'returned',value:{kind:'integer',value:String(i.subject==='W.Api.prepare'?prepare(item,i.arguments[0].value):finish(item,i.arguments[1].value))}};},async observe(){if(!item.present)return {'W.Item':[]};return {'W.Item':[{kind:'record',value:{id:{kind:'string',value:item.id},status:{kind:'variant',value:{declaration:'W.Status',name:item.status,payload:null}}}}]};},async close(){}};}};"#).unwrap();
    let fixtures = dctx0_engine::VerificationLifecycleFixtures {
        paths: [(
            path.clone(),
            dctx0_verification::LifecycleCaseInput {
                keys: vec![VerificationValue::String("chosen".into())],
                setup: vec![Invocation {
                    subject: "W.Api.prepare".into(),
                    arguments: vec![VerificationValue::String("chosen".into())],
                }],
                arguments: vec![vec![
                    VerificationValue::String("chosen".into()),
                    VerificationValue::String("long note".into()),
                ]],
            },
        )]
        .into(),
    };
    fs::write(
        app.join(".dctx0/testing/typescript/lifecycles.json"),
        serde_json::to_vec(&fixtures).unwrap(),
    )
    .unwrap();
    let adapter_path = app.join(".dctx0/testing/typescript/adapter.ts");
    let original_adapter = fs::read_to_string(&adapter_path).unwrap();
    let project = dctx0_engine::Project::new(&spec, None).unwrap();
    for missing in ["fixture", "capability", "observer-error"] {
        let adapted = match missing {
            "capability" => {
                original_adapter.replace("'invocation','state','events'", "'invocation','events'")
            }
            "observer-error" => original_adapter.replace(
                "async observe(){",
                "async observe(){throw new Error('Actual observer failed');",
            ),
            _ => original_adapter.clone(),
        };
        fs::write(&adapter_path, adapted).unwrap();
        let inputs = if missing == "fixture" {
            dctx0_engine::VerificationLifecycleFixtures::default()
        } else {
            fixtures.clone()
        };
        fs::write(
            app.join(".dctx0/testing/typescript/lifecycles.json"),
            serde_json::to_vec(&inputs).unwrap(),
        )
        .unwrap();
        let unavailable = call(&["verify", "run", input, "--no-adopt", "--json", "--full"]);
        assert!(!unavailable.status.success());
        let report: dctx0_engine::NativeVerificationReport =
            serde_json::from_slice(&unavailable.stdout).unwrap();
        assert!(report.evidence.run.required_cases.contains(&path));
        assert!(matches!(
            report.evidence.run.result,
            VerificationResult::Unknown { .. }
        ));
        let code = match missing {
            "fixture" => "LIFECYCLE_FIXTURE_REQUIRED",
            "capability" => "LIFECYCLE_OBSERVER_REQUIRED",
            _ => "NATIVE_RUN_ABORTED",
        };
        assert!(
            report
                .evidence
                .run
                .open_obligations
                .iter()
                .any(|issue| issue.code == code),
            "{missing}: {:?}",
            report.evidence.run.open_obligations
        );
        let attempted = report
            .evidence
            .run
            .cases
            .iter()
            .find(|case| case.id == path);
        if missing == "observer-error" {
            assert!(matches!(
                attempted.unwrap().result,
                VerificationResult::Unknown { .. }
            ));
        } else {
            assert!(attempted.is_none());
        }
        assert!(report.adopted_tasks.is_empty());
        assert_eq!(
            project.saved_verification_run(&report.evidence.id).unwrap(),
            report.evidence
        );
    }
    fs::write(adapter_path, original_adapter).unwrap();
    fs::write(
        app.join(".dctx0/testing/typescript/lifecycles.json"),
        serde_json::to_vec(&fixtures).unwrap(),
    )
    .unwrap();
    let executed = call(&["verify", "run", input, "--no-adopt", "--json", "--full"]);
    let report: dctx0_engine::NativeVerificationReport =
        serde_json::from_slice(&executed.stdout).unwrap();
    let lifecycle = report
        .evidence
        .run
        .cases
        .iter()
        .find(|case| case.id == path)
        .unwrap();
    assert_eq!(lifecycle.result, VerificationResult::Pass);
    assert!(
        lifecycle
            .states
            .iter()
            .all(|state| state.phase_recorded == Some(true))
    );
    assert!(report.adopted_tasks.is_empty());
    let original = project.saved_verification_run(&report.evidence.id).unwrap();
    for json in [true, false] {
        let mut args = vec![
            "verify",
            "replay",
            input,
            &report.evidence.id,
            "--case",
            &path,
        ];
        if json {
            args.extend(["--json", "--full"]);
        }
        let replay = call(&args);
        assert!(
            replay.status.success(),
            "{}",
            String::from_utf8_lossy(&replay.stderr)
        );
        if json {
            let replay: dctx0_engine::VerificationReplayReport =
                serde_json::from_slice(&replay.stdout).unwrap();
            assert_eq!(replay.case_result, Some(VerificationResult::Pass));
            assert!(replay.evidence.adopted_tasks.is_empty());
        } else {
            assert!(
                String::from_utf8_lossy(&replay.stdout)
                    .contains("no task adoption or whole-application claim")
            );
        }
    }
    fs::write(
        app.join("app.ts"),
        production.replace("item.status='Closed'", "item.status='Open'"),
    )
    .unwrap();
    let failed = call(&[
        "verify",
        "replay",
        input,
        &report.evidence.id,
        "--case",
        &path,
        "--json",
        "--full",
    ]);
    assert!(!failed.status.success());
    let failed: dctx0_engine::VerificationReplayReport =
        serde_json::from_slice(&failed.stdout).unwrap();
    assert!(matches!(
        failed.case_result,
        Some(VerificationResult::Fail { .. })
    ));
    assert!(
        failed
            .changes
            .iter()
            .any(|change| change.component == "implementation:app.ts")
    );
    assert!(failed.evidence.adopted_tasks.is_empty());
    let minimized = call(&[
        "verify",
        "replay",
        input,
        &failed.evidence.evidence.id,
        "--case",
        &path,
        "--minimized",
        "--json",
        "--full",
    ]);
    assert!(!minimized.status.success());
    let minimized: dctx0_engine::VerificationReplayReport =
        serde_json::from_slice(&minimized.stdout).unwrap();
    assert!(minimized.minimized_input);
    assert!(matches!(
        minimized.case_result,
        Some(VerificationResult::Fail { .. })
    ));
    let summary = project.verification_summary(&report.evidence.id).unwrap();
    assert!(!summary.details_available);
    assert_eq!(summary.result, original.run.result);
    assert_eq!(summary.case_count, original.run.cases.len());
    assert!(
        project
            .saved_verification_run(&report.evidence.id)
            .unwrap_err()
            .to_string()
            .contains("pruned")
    );
    assert_eq!(project.history().unwrap().len(), 1);
    fs::remove_dir_all(root).unwrap();
}
