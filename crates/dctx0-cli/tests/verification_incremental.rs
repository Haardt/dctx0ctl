#![cfg(feature = "typescript")]

use std::{fs, process::Command};

#[test]
fn incremental_comparison_is_read_only_and_does_not_claim_test_pass() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-incremental-{}", std::process::id()));
    let spec = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir(&app).unwrap();
    fs::write(
        spec.join("w.dctx0"),
        "workspace W { value A = Int { range 1..2 } }",
    )
    .unwrap();
    let cli = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .output()
            .unwrap()
    };
    assert!(cli(&["check", spec.to_str().unwrap()]).status.success());
    let init = cli(&[
        "verify",
        "init",
        spec.to_str().unwrap(),
        "--project",
        app.to_str().unwrap(),
        "--target",
        "typescript",
        "--subject",
        "W.A",
    ]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let run = cli(&[
        "verify",
        "run",
        spec.to_str().unwrap(),
        "--project",
        app.to_str().unwrap(),
        "--no-adopt",
        "--json",
        "--full",
    ]);
    assert!(!run.status.success());
    let saved: dctx0_engine::NativeVerificationReport =
        serde_json::from_slice(&run.stdout).unwrap();
    let project = dctx0_engine::Project::new(&spec, None).unwrap();
    let before = fs::read(&project.database).unwrap();
    for json in [false, true] {
        let mut args = vec![
            "verify",
            "incremental-plan",
            spec.to_str().unwrap(),
            &saved.evidence.id,
            "--project",
            app.to_str().unwrap(),
        ];
        if json {
            args.push("--json");
        }
        let comparison = cli(&args);
        assert!(
            comparison.status.success(),
            "{}",
            String::from_utf8_lossy(&comparison.stderr)
        );
        if json {
            let report: serde_json::Value = serde_json::from_slice(&comparison.stdout).unwrap();
            assert_eq!(report["actual_reused_cases"], 0);
            assert_eq!(report["rerun_cases"], report["required_cases"]);
            assert!(
                report["reasons"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["code"] == "INCREMENTAL_EXTERNAL_INPUTS_UNAVAILABLE")
            );
        } else {
            assert!(
                String::from_utf8_lossy(&comparison.stdout)
                    .contains("Incremental verification comparison")
            );
        }
        assert_eq!(before, fs::read(&project.database).unwrap());
    }
    let missing = cli(&[
        "verify",
        "incremental-plan",
        spec.to_str().unwrap(),
        "missing",
        "--project",
        app.to_str().unwrap(),
    ]);
    assert!(!missing.status.success());
    assert_eq!(before, fs::read(&project.database).unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn incremental_execution_flags_record_and_reuse_actual_native_cases() {
    let root = std::env::temp_dir().join(format!(
        "dctx0-cli-incremental-execute-{}",
        std::process::id()
    ));
    let spec = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir_all(&app).unwrap();
    fs::write(
        spec.join("w.dctx0"),
        "workspace W { value A = Int { range 1..2 } }",
    )
    .unwrap();
    let tool_calls = std::cell::Cell::new(0u64);
    let relevant = std::cell::Cell::new("revision-one");
    let cli = |args: &[&str]| {
        tool_calls.set(tool_calls.get() + 1);
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .env("CODEX_TOOL_CALL_ID", format!("tool-{}", tool_calls.get()))
            .env("REUSE_RELEVANT", relevant.get())
            .output()
            .unwrap()
    };
    assert!(cli(&["check", spec.to_str().unwrap()]).status.success());
    assert!(
        cli(&[
            "verify",
            "init",
            spec.to_str().unwrap(),
            "--project",
            app.to_str().unwrap(),
            "--target",
            "typescript",
            "--subject",
            "W.A"
        ])
        .status
        .success()
    );
    let configuration_path = app.join(".dctx0/testing/verification.json");
    let mut configuration: serde_json::Value =
        serde_json::from_slice(&fs::read(&configuration_path).unwrap()).unwrap();
    configuration["environment"] = serde_json::json!({"mode":"controlled","inherit":["PATH","HOME","TMPDIR","REUSE_RELEVANT"]});
    fs::write(
        &configuration_path,
        serde_json::to_vec(&configuration).unwrap(),
    )
    .unwrap();
    fs::create_dir_all(app.join(".dctx0ctl")).unwrap();
    let counter_path = app.join(".dctx0ctl/calls");
    let counter = serde_json::to_string(&counter_path.to_string_lossy()).unwrap();
    fs::write(app.join("app.ts"),format!("import {{appendFileSync}} from 'node:fs';export function factory(value){{appendFileSync({counter},'call\\n');if(!Number.isSafeInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}}")).unwrap();
    let calls = || fs::read_to_string(&counter_path).unwrap().lines().count();
    fs::write(app.join(".dctx0/testing/typescript/adapter.ts"),r#"import {factory} from '../../../app.ts';export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"#).unwrap();
    fs::write(
        app.join(".dctx0/testing/reuse-inputs.json"),
        r#"{"schema_version":1,"deterministic_units":["contract:W.A"]}"#,
    )
    .unwrap();
    assert!(
        cli(&[
            "profile",
            "typescript",
            "create",
            "--project",
            app.to_str().unwrap(),
            "--default"
        ])
        .status
        .success()
    );
    assert!(
        cli(&["tasks", "plan", spec.to_str().unwrap()])
            .status
            .success()
    );
    let base = [
        "verify",
        "run",
        spec.to_str().unwrap(),
        "--project",
        app.to_str().unwrap(),
        "--no-adopt",
        "--json",
    ];
    let first = cli(&base);
    assert!(
        first.status.success(),
        "{} {}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let first: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    let parent = first["summary"]["id"].as_str().unwrap();
    assert_eq!(first["extra"]["incremental"]["reused_cases"], 0);
    let first_calls = calls();
    assert!(first_calls > 0);
    let second = cli(&base);
    assert_eq!(
        calls(),
        first_calls,
        "a different tool call must not repeat production cases"
    );
    assert!(
        second.status.success(),
        "{} {}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    let second: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert!(
        second["extra"]["incremental"]["reused_cases"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(second["extra"]["incremental"]["executed_cases"], 0);
    assert_eq!(second["extra"]["incremental"]["adoption_authority"], false);
    // The same current cases can authorize delivery when explicitly leaving
    // diagnostic mode. The CLI must not require --no-adopt for reuse inputs.
    let mut adopting = base
        .iter()
        .copied()
        .filter(|arg| *arg != "--no-adopt")
        .collect::<Vec<_>>();
    adopting.extend(["--reuse-from", parent]);
    let adopted = cli(&adopting);
    assert!(
        adopted.status.success(),
        "{} {}",
        String::from_utf8_lossy(&adopted.stdout),
        String::from_utf8_lossy(&adopted.stderr)
    );
    let adopted: serde_json::Value = serde_json::from_slice(&adopted.stdout).unwrap();
    assert_eq!(adopted["extra"]["incremental"]["executed_cases"], 0);
    assert_eq!(adopted["extra"]["incremental"]["adoption_authority"], true);
    assert!(
        !adopted["extra"]["adopted_tasks"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{adopted:#}"
    );
    let mut forced = base.to_vec();
    forced.extend(["--reuse", "off"]);
    let fresh = cli(&forced);
    assert!(
        fresh.status.success(),
        "{}",
        String::from_utf8_lossy(&fresh.stderr)
    );
    let fresh: serde_json::Value = serde_json::from_slice(&fresh.stdout).unwrap();
    assert_eq!(fresh["extra"]["incremental"]["reused_cases"], 0);
    assert!(calls() > first_calls);
    let fresh_calls = calls();
    relevant.set("revision-two");
    let mut required = base.to_vec();
    required.extend(["--reuse", "require"]);
    let blocked = cli(&required);
    assert!(!blocked.status.success());
    let message = String::from_utf8_lossy(&blocked.stderr);
    assert!(message.contains("REUSE_REQUIRED"), "{message}");
    assert!(message.contains("REUSE_RELEVANT"), "{message}");
    assert!(!message.contains("revision-one"));
    assert!(!message.contains("revision-two"));
    assert_eq!(calls(), fresh_calls);
    let changed = cli(&base);
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    assert!(calls() > fresh_calls);
    let changed: serde_json::Value = serde_json::from_slice(&changed.stdout).unwrap();
    assert_eq!(changed["extra"]["incremental"]["reused_cases"], 0);
    assert!(
        changed["extra"]["incremental"]["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|decision| decision["reasons"].as_array().unwrap())
            .any(|reason| reason["code"] == "ENVIRONMENT_CHANGED"
                && reason["input"] == "REUSE_RELEVANT")
    );
    let conflicting = cli(&[
        "verify",
        "run",
        spec.to_str().unwrap(),
        "--reuse=off",
        "--reuse-from",
        parent,
    ]);
    assert!(!conflicting.status.success());
    assert!(String::from_utf8_lossy(&conflicting.stderr).contains("conflicts"));
    fs::remove_dir_all(root).unwrap();
}
