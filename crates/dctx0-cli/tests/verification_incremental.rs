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
        "--reuse",
        "off",
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
    let blocked = cli(&base);
    assert!(
        String::from_utf8_lossy(&blocked.stderr).contains("FULL_EXECUTION_REQUIRES_EXPLICIT_OFF")
    );
    assert!(!counter_path.exists());
    let mut fresh_args = base.to_vec();
    fresh_args.extend(["--reuse", "off"]);
    let first = cli(&fresh_args);
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
    let same_code = cli(&required);
    assert!(
        same_code.status.success(),
        "{}",
        String::from_utf8_lossy(&same_code.stderr)
    );
    assert_eq!(
        calls(),
        fresh_calls,
        "environment metadata must not invalidate code-only reuse"
    );
    let source = fs::read_to_string(app.join("app.ts")).unwrap();
    fs::write(app.join("app.ts"), format!("{source}\n// source changed")).unwrap();
    let blocked = cli(&required);
    assert!(!blocked.status.success());
    let message = String::from_utf8_lossy(&blocked.stderr);
    assert!(message.contains("REUSE_REQUIRED"), "{message}");
    assert!(
        message.contains("CAPTURED_SOURCE_DISCOVERY_CHANGED_OR_MISSING"),
        "{message}"
    );
    assert_eq!(calls(), fresh_calls);

    let changed = cli(&forced);
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    assert!(calls() > fresh_calls);
    let changed: serde_json::Value = serde_json::from_slice(&changed.stdout).unwrap();
    assert_eq!(changed["extra"]["incremental"]["reused_cases"], 0);
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

#[test]
fn incremental_comparison_explains_task_impacts_and_logical_group_evidence() {
    use dctx0_core::{VerificationIssue, VerificationResult};
    use dctx0_engine::{
        IncrementalTestGroup, Project, TestGroupChange, TestGroupRunnerStatus,
        VerificationIncrementalReport,
    };
    use dctx0_store::{AccessMode, Store};

    let root = std::env::temp_dir().join(format!("dctx0-cli-impact-{}", std::process::id()));
    let spec = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir(&app).unwrap();
    let source = spec.join("w.dctx0");
    let original = r#"workspace W {
        domain D {
            constraint "First rule"
            constraint "Second rule"
            value One = Int { range 1..2 }
            value Two = Int { range 1..2 }
        }
        domain E {
            value Independent = Int { range 1..2 }
            value Drop = Int { range 1..2 }
        }
    }"#;
    fs::write(&source, original).unwrap();
    let spec_path = spec.to_str().unwrap();
    let app_path = app.to_str().unwrap();
    let cli = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .output()
            .unwrap()
    };
    let success = |args: &[&str]| {
        let result = cli(args);
        assert!(
            result.status.success(),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&result.stderr),
            String::from_utf8_lossy(&result.stdout)
        );
        result.stdout
    };
    success(&["check", spec_path]);
    success(&[
        "verify",
        "init",
        spec_path,
        "--project",
        app_path,
        "--target",
        "typescript",
        "--subject",
        "W",
    ]);
    let project = Project::new(&spec, None).unwrap();
    success(&[
        "profile",
        "typescript",
        "create",
        "--project",
        app_path,
        "--default",
    ]);
    let initial_plan = project.task_plan("latest").unwrap();
    let task = |subject: &str| {
        initial_plan
            .members
            .iter()
            .find(|member| member.candidate.subjects == [subject])
            .unwrap()
            .task_id
    };
    let one = task("W.D.One");
    let two = task("W.D.Two");
    let calls = app.join(".dctx0ctl/calls");
    fs::create_dir(calls.parent().unwrap()).unwrap();
    fs::write(
        app.join("app.ts"),
        format!(
            "import {{appendFileSync}} from 'node:fs'; export function markWorker(){{appendFileSync({},'x');}} export function factory(v:number){{if(!Number.isSafeInteger(v)||v<1||v>2)throw TypeError('invalid'); return v;}}",
            serde_json::to_string(calls.to_str().unwrap()).unwrap()
        ),
    ).unwrap();
    fs::write(app.join(".dctx0/testing/typescript/adapter.ts"), r#"
        import {factory,markWorker} from '../../../app.ts';
        const subjects = ['W.D.One', 'W.D.Two', 'W.E.Independent', 'W.E.Drop'];
        export const adapter = {
            async describe() {
                markWorker();
                return {protocol_version:1, subjects: subjects.map(subject => ({
                    subject, role:'contract', artifact:'app.ts', entry_point:'factory'
                })), artifacts:['app.ts'], capabilities:['sessions','values','events'],
                representations:{Int:'safe integer'}};
            },
            async begin() {
                return {async invoke(){throw Error('No operation');},
                    async validate(subject,value) {
                        try {
                            const result = factory(value.kind === 'integer' || value.kind === 'float'
                                ? Number(value.value) : value.kind === 'unit' ? undefined : value.value);
                            return {kind:'accepted', value:{kind:'integer', value:String(result)}};
                        } catch(error) { return {kind:'rejected', reason:String(error)}; }
                    },
                    async close(){}};
            },
        };
    "#).unwrap();
    fs::write(app.join(".dctx0/testing/reuse-inputs.json"), r#"{
        "schema_version":1,
        "deterministic_units":["contract:W.D.One","contract:W.D.Two","contract:W.E.Independent","contract:W.E.Drop"],
        "revisions":{"never-run":{"executable":"nonexistent-provider"}}
    }"#).unwrap();
    let run = cli(&[
        "verify",
        "run",
        spec_path,
        "--project",
        app_path,
        "--no-adopt",
        "--reuse",
        "off",
        "--json",
        "--full",
    ]);
    let parent: dctx0_engine::NativeVerificationReport = serde_json::from_slice(&run.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&run.stderr)));
    let parent = parent.evidence;
    assert!(!parent.run.cases.is_empty());
    let observed = fs::read(&calls).unwrap();
    let compare = |id: &str, options: &[&str]| {
        let database = fs::read(&project.database).unwrap();
        let wal = project.database.with_extension("sqlite3-wal");
        let before_wal = fs::read(&wal).unwrap_or_default();
        let mut args = vec![
            "verify",
            "incremental-plan",
            spec_path,
            id,
            "--project",
            app_path,
        ];
        args.extend_from_slice(options);
        let markdown = String::from_utf8(success(&args)).unwrap();
        assert!(markdown.contains("## Task impacts"));
        assert!(markdown.contains("## Test groups"));
        args.push("--json");
        let report: VerificationIncrementalReport =
            serde_json::from_slice(&success(&args)).unwrap();
        assert_eq!(fs::read(&project.database).unwrap(), database);
        assert_eq!(fs::read(&wal).unwrap_or_default(), before_wal);
        assert_eq!(fs::read(&calls).unwrap(), observed);
        (report, markdown)
    };
    fn group<'a>(report: &'a VerificationIncrementalReport, id: &str) -> &'a IncrementalTestGroup {
        report
            .test_groups
            .iter()
            .find(|group| group.id == id)
            .unwrap()
    }
    let (baseline, _) = compare(&parent.id, &[]);
    let independent = "contract:W.E.Independent";
    assert_eq!(
        group(&baseline, independent).runner_status,
        TestGroupRunnerStatus::StaticCompatible,
        "{baseline:?}"
    );

    let application_code = fs::read(app.join("app.ts")).unwrap();
    fs::write(app.join("app.ts"), b"export const changed = true;").unwrap();
    let (code_changed, _) = compare(&parent.id, &[]);
    assert!(
        group(&code_changed, independent)
            .reasons
            .iter()
            .any(|reason| reason.code == "LOCAL_CODE_INPUTS_CHANGED")
    );
    fs::write(app.join("app.ts"), application_code).unwrap();

    let store = Store::open(&project.database, AccessMode::ReadOnly).unwrap();
    let artifacts: Vec<_> = parent
        .run
        .artifacts
        .iter()
        .map(|artifact| {
            store
                .verification_artifact(&parent.id, &artifact.hash)
                .unwrap()
        })
        .collect();
    let saved_program: dctx0_verification::ProgramPlan = serde_json::from_slice(
        &artifacts
            .iter()
            .find(|artifact| artifact.reference.hash == parent.run.provenance.plan_hash)
            .unwrap()
            .contents,
    )
    .unwrap();
    drop(store);
    let mut unbound = parent.run.clone();
    unbound.started_at_unix_ns = "1".into();
    unbound.artifacts.retain(|artifact| {
        artifact.media_type != dctx0_verification::incremental::INCREMENTAL_INPUTS_MEDIA_TYPE
    });
    let unbound_artifacts = artifacts
        .iter()
        .filter(|artifact| unbound.artifacts.contains(&artifact.reference))
        .cloned()
        .collect::<Vec<_>>();
    let mut store = Store::open(&project.database, AccessMode::ReadWrite).unwrap();
    let unbound = store
        .record_verification_run_owned_from(
            &project.key(),
            unbound,
            &unbound_artifacts,
            Some(&parent.id),
        )
        .unwrap();
    drop(store);
    let (unbound, _) = compare(&unbound.id, &[]);
    assert_eq!(
        group(&unbound, independent).runner_status,
        TestGroupRunnerStatus::BindingCheckRequired
    );

    fs::write(&source, format!("\n\n{original}")).unwrap();
    success(&["check", spec_path]);
    let (position, _) = compare(&parent.id, &[]);
    assert!(position.task_impacts.is_empty());
    assert!(
        position
            .test_groups
            .iter()
            .all(|group| group.change == TestGroupChange::Unchanged)
    );

    let changed = original
        .replace("First rule", "Updated first rule")
        .replace("constraint \"Second rule\"", "")
        .replace("Two = Int { range 1..2 }", "Two = Int { range -3..-2 }")
        .replace(
            "value Drop = Int { range 1..2 }",
            "value New = Int { range 1..2 }",
        );
    fs::write(&source, changed).unwrap();
    success(&["check", spec_path]);
    let current_program = project
        .verification_plan(
            dctx0_engine::VerificationSelection {
                subjects: ["W".into()].into(),
                ..Default::default()
            },
            &Default::default(),
            saved_program.budgets.clone(),
        )
        .unwrap();
    let case_ids = |plan: &dctx0_verification::ProgramPlan| {
        dctx0_verification::unit_planner::catalog::logical_unit_catalog(plan)
            .unwrap()
            .into_iter()
            .find(|unit| unit.id == "contract:W.D.Two")
            .unwrap()
            .cases
    };
    assert_eq!(case_ids(&saved_program), case_ids(&current_program));
    let (comparison, markdown) = compare(&parent.id, &[]);
    assert_eq!(comparison.historical_task_plan_id, Some(initial_plan.id));
    assert!(comparison.current_task_plan_id.is_none());
    assert!(
        comparison
            .reasons
            .iter()
            .any(|reason| reason.code == "CURRENT_TASK_PLAN_MISSING")
    );
    let affected = |id| {
        comparison
            .task_impacts
            .iter()
            .find(|task| task.task_id == id)
            .unwrap()
    };
    let inherited = affected(one);
    assert_eq!(
        inherited.classification,
        dctx0_engine::TaskImpactClassification::InheritedConstraintOnly
    );
    assert_eq!(inherited.causes.len(), 2, "{inherited:?}");
    assert!(
        inherited
            .causes
            .iter()
            .any(|cause| cause.kind == dctx0_changes::ChangeKind::Removed)
    );
    assert!(inherited.causes.iter().all(|cause| {
        cause
            .path
            .path
            .starts_with(&["W.D.One".into(), "W.D".into()])
    }));
    let mixed = affected(two);
    assert_eq!(
        mixed.classification,
        dctx0_engine::TaskImpactClassification::Mixed
    );
    assert!(
        mixed
            .causes
            .iter()
            .any(|cause| cause.new.as_deref() == Some("W.D.Two"))
    );
    assert!(
        mixed
            .causes
            .iter()
            .any(|cause| cause.path.inherited_constraint)
    );
    let retained = group(&comparison, independent);
    assert_eq!(retained.change, TestGroupChange::Unchanged);
    assert!(retained.historical_pass);
    assert_eq!(
        retained.runner_status,
        TestGroupRunnerStatus::ExecutionRequired
    );
    assert!(
        retained
            .reasons
            .iter()
            .any(|reason| reason.code == "SHARED_ADAPTER_PROGRAM_CHANGED")
    );
    assert_eq!(
        group(&comparison, "contract:W.D.One").change,
        TestGroupChange::Changed
    );
    assert!(
        group(&comparison, "contract:W.D.Two")
            .reasons
            .iter()
            .any(|reason| reason.code == "LOGICAL_CASES_CHANGED")
    );
    assert_eq!(
        group(&comparison, "contract:W.E.New").change,
        TestGroupChange::New
    );
    assert_eq!(
        group(&comparison, "contract:W.E.Drop").change,
        TestGroupChange::Removed
    );
    assert!(markdown.contains("inherited-constraint-only"));
    assert!(markdown.contains("SHARED_ADAPTER_PROGRAM_CHANGED"));

    let (scoped, _) = compare(&parent.id, &["--task", &format!("T{one}")]);
    assert_eq!(
        scoped
            .task_impacts
            .iter()
            .map(|task| task.task_id)
            .collect::<Vec<_>>(),
        [one]
    );
    assert_eq!(scoped.test_groups.len(), 1);
    let current_plan = project.task_plan("latest").unwrap();
    let (published, _) = compare(&parent.id, &["--subject", "W.D.Two"]);
    assert_eq!(published.current_task_plan_id, Some(current_plan.id));
    assert!(
        published
            .task_impacts
            .iter()
            .any(|task| task.current_member)
    );
    assert!(
        published
            .task_impacts
            .iter()
            .all(|task| task.task_id != one)
    );

    let renamed = project.rename("W.D.One", "Renamed", false).unwrap();
    assert_eq!(renamed.new_symbol, "W.D.Renamed");
    let (rename_impact, _) = compare(&parent.id, &["--subject", "W.D.Renamed"]);
    assert!(
        rename_impact
            .task_impacts
            .iter()
            .any(|task| task.task_id == one
                && task.causes.iter().any(|cause| {
                    cause.kind == dctx0_changes::ChangeKind::Renamed
                        && cause.old.as_deref() == Some("W.D.One")
                        && cause.new.as_deref() == Some("W.D.Renamed")
                }))
    );

    let mut partial = parent.run.clone();
    partial.started_at_unix_ns = "2".into();
    partial
        .cases
        .retain(|case| !case.id.starts_with("W.E.Independent:"));
    partial.result = VerificationResult::Unknown {
        issue: VerificationIssue {
            code: "INCOMPLETE_RUN".into(),
            message: "No independent cases".into(),
            span: None,
        },
    };
    let mut store = Store::open(&project.database, AccessMode::ReadWrite).unwrap();
    let mut imported = partial.clone();
    imported.origin = dctx0_core::VerificationRunOrigin::Imported;
    imported.started_at_unix_ns = "3".into();
    let pruned = store
        .record_verification_run(&project.key(), &imported, &artifacts)
        .unwrap()
        .id;
    let partial = store
        .record_verification_run_owned_from(&project.key(), partial, &artifacts, Some(&parent.id))
        .unwrap();
    drop(store);
    let (incomplete, _) = compare(&partial.id, &["--subject", "W.E.Independent"]);
    assert_eq!(
        group(&incomplete, independent).runner_status,
        TestGroupRunnerStatus::EvidenceMissing
    );
    assert!(!group(&incomplete, independent).historical_pass);
    let (contradicted, _) = compare(&parent.id, &[]);
    assert!(
        group(&contradicted, independent)
            .reasons
            .iter()
            .any(|reason| reason.code == "LATER_NONPASS_OBSERVATION")
    );
    let pruned = cli(&[
        "verify",
        "incremental-plan",
        spec_path,
        &pruned,
        "--project",
        app_path,
    ]);
    assert!(!pruned.status.success());
    assert!(String::from_utf8_lossy(&pruned.stderr).contains("DETAILS_PRUNED"));
    fs::remove_dir_all(root).unwrap();
}
