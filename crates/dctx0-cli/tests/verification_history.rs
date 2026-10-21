use dctx0_core::*;
use dctx0_store::{AccessMode, Store, VerificationArtifact};
use std::{fs, process::Command};
#[test]
fn show_reads_saved_scope_outcomes_and_provenance_after_source_changes_without_migration() {
    let root = std::env::temp_dir().join(format!(
        "dctx0-cli-verification-history-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("w.dctx0"), "workspace W { type Number = Int }").unwrap();
    let p = dctx0_engine::Project::new(&root, None).unwrap();
    assert!(p.check().unwrap().ok);
    let mut store = Store::open(&p.database, AccessMode::ReadWrite).unwrap();
    let snapshot = dctx0_changes::HistoryReader::latest(&store, &p.key())
        .unwrap()
        .unwrap();
    let occurrence = dctx0_changes::HistoryReader::history(&store, &p.key()).unwrap()[0].run_id;
    let plan = VerificationArtifact::new(
        "application/json",
        b"{\"required_cases\":[\"C1\",\"C2\",\"C3\"]}".to_vec(),
    );
    let issue = serde_json::json!({"code":"IMPORTED_UNEXECUTED","message":"This report has no native execution authority","span":null});
    let unknown = serde_json::json!({"status":"UNKNOWN","issue":issue});
    let mut run:VerificationRun=serde_json::from_value(serde_json::json!({"schema_version":1,"origin":"imported","started_at_unix_ns":"1234567890000000000","elapsed_us":0,"provenance":{"snapshot":snapshot.id,"check_occurrence":occurrence,"plan_hash":plan.reference.hash,"implementation":{},"dependencies":{},"adapter_hash":"","configuration_hash":"","harness_hash":"","interpreter_version":"1","generator_version":"import","protocol_version":1,"target":"typescript","runtime_version":"not-executed","build_configuration":{},"task_pin":null,"incomplete":[issue]},"budgets":VerificationBudgets::default(),"selected_subjects":["W.Number"],"required_subjects":["W.Number"],"not_selected":[],"required_cases":["C1"],"cases":[{"id":"C1","subject":"W.Number","kind":"contract","bindings":{},"setup":[],"arguments":[],"observations":[],"events":null,"states":[],"result":unknown,"elapsed_us":0}],"open_obligations":[issue],"bounded_obligations":[],"artifacts":[plan.reference],"result":unknown})).unwrap();
    run.cases[0]
        .states
        .push(serde_json::from_value(serde_json::json!({"phase":"before","entities":{}})).unwrap());
    let mut passed = run.cases[0].clone();
    passed.states[0].phase_recorded = Some(true);
    passed.id = "C2".into();
    passed.result = VerificationResult::Pass;
    passed.observations.push(InvocationObservation {
        invocation: Invocation {
            subject: "W.Number".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("1".into()),
        },
    });
    let mut failed = passed.clone();
    failed.id = "C3".into();
    failed.result = VerificationResult::Fail {
        issue: VerificationIssue {
            code: "CONTRACT_VIOLATED".into(),
            message: "Actual result violated its contract".into(),
            span: None,
        },
    };
    failed.observations[0].observation = Observation::Threw {
        message: "Actual exception".into(),
    };
    failed.observations.push(InvocationObservation {
        invocation: failed.observations[0].invocation.clone(),
        observation: Observation::Unavailable {
            reason: "Observer missing".into(),
        },
    });
    run.result = failed.result.clone();
    run.required_cases.extend(["C2".into(), "C3".into()]);
    run.cases.extend([passed, failed]);
    let saved = store
        .record_verification_run(&p.key(), &run, &[plan])
        .unwrap();
    drop(store);
    p.task_plan("latest").unwrap();
    let database_before_show = fs::read(&p.database).unwrap();
    fs::write(root.join("w.dctx0"), "invalid now").unwrap();
    for json in [false, true] {
        let mut args = vec!["verify", "show", root.to_str().unwrap(), saved.id.as_str()];
        if json {
            args.push("--json");
        }
        let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if json {
            let got: dctx0_store::StoredVerificationRun =
                serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(got, saved);
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("no working-tree freshness"));
            assert!(text.contains("IMPORTED_UNEXECUTED"));
            assert!(text.contains("Recorded outcomes: 1 PASS, 1 FAIL, 1 UNKNOWN"));
            assert!(text.contains(
                "Recorded invocation observations: 1 returned, 1 exceptions, 1 unavailable"
            ));
            assert!(text.contains("Cases without invocation observations: 1"));
            assert!(text.contains(
                "State phase provenance: 2 recorded at observation, 1 without recorded timing"
            ));
            assert!(text.contains("| UNKNOWN | `IMPORTED_UNEXECUTED` | 1 |"));
            assert!(text.contains("| FAIL | `CONTRACT_VIOLATED` | 1 |"));
            assert!(text.contains("| `W.Number` | 1 | 1 | 1 |"));
            assert!(text.contains("Case counts do not establish complete subject verification"));
            assert!(text.contains("Incomplete provenance: 1"));
            assert!(text.contains("Seed: 42"));
            assert!(text.contains("`C1` / `W.Number`"));
        }
    }
    let show = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args([
                "verify",
                "show",
                root.to_str().unwrap(),
                &saved.id,
                "--json",
            ])
            .args(args)
            .output()
            .unwrap()
    };
    let compact = show(&["--summary"]);
    assert!(compact.status.success());
    let compact: serde_json::Value = serde_json::from_slice(&compact.stdout).unwrap();
    assert_eq!(compact["summary"]["recorded_cases"], 3);
    assert_eq!(compact["summary"]["result"]["status"], "FAIL");
    assert_eq!(compact["cases"].as_array().unwrap().len(), 0);
    let filtered = show(&["--outcome", "FAIL,UNKNOWN", "--limit", "1"]);
    assert!(filtered.status.success());
    let filtered: serde_json::Value = serde_json::from_slice(&filtered.stdout).unwrap();
    assert_eq!(filtered["matching_cases"], 2);
    assert_eq!(filtered["cases"][0]["id"], "C1");
    let cursor = serde_json::to_string(&filtered["next_cursor"]).unwrap();
    let next = show(&[
        "--outcome",
        "FAIL,UNKNOWN",
        "--limit",
        "1",
        "--cursor",
        &cursor,
    ]);
    assert!(next.status.success());
    let next: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_eq!(next["cases"][0]["id"], "C3");
    assert!(next["complete"].as_bool().unwrap());
    assert!(
        !show(&["--limit", "1", "--cursor", &cursor])
            .status
            .success()
    );
    assert!(!show(&["--limit", "0"]).status.success());
    assert!(!show(&["--case", "absent"]).status.success());
    let detail = show(&["--case", "C3"]);
    assert!(detail.status.success());
    let detail: serde_json::Value = serde_json::from_slice(&detail.stdout).unwrap();
    assert_eq!(
        detail["detail"]["observations"].as_array().unwrap().len(),
        2
    );
    for args in [
        &["--summary"][..],
        &["--outcome", "FAIL", "--limit", "1"][..],
        &["--case", "C3"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["verify", "show", root.to_str().unwrap(), &saved.id])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Result: Fail"));
    }
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["verify", "show", root.to_str().unwrap(), "missing"])
            .status()
            .unwrap()
            .success()
    );
    for json in [true, false] {
        let mut args = vec![
            "verify",
            "adoption-preview",
            root.to_str().unwrap(),
            &saved.id,
        ];
        if json {
            args.push("--json");
        }
        let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("ADOPTION_NATIVE_ORIGIN_REQUIRED")
        );
    }
    assert_eq!(database_before_show, fs::read(&p.database).unwrap());
    let mut store = Store::open(&p.database, AccessMode::ReadWrite).unwrap();
    let artifacts = saved
        .run
        .artifacts
        .iter()
        .map(|a| store.verification_artifact(&saved.id, &a.hash).unwrap())
        .collect::<Vec<_>>();
    let mut next = saved.run.clone();
    for n in 1..=4 {
        next.started_at_unix_ns = n.to_string();
        store
            .record_verification_run(&p.key(), &next, &artifacts)
            .unwrap();
    }
    drop(store);
    let preview = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args([
            "verify",
            "adoption-preview",
            root.to_str().unwrap(),
            &saved.id,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(preview.status.success());
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert!(
        preview["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "ADOPTION_DETAILS_PRUNED")
    );
    assert!(preview["tasks"].as_array().unwrap().iter().all(|t| {
        t["eligible"] == false
            && t["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .all(|i| i["code"] != "TASK_OUTSIDE_EVIDENCE_SCOPE")
    }));
    let exports = root.join("exports");
    fs::create_dir_all(exports.join("nested")).unwrap();
    let report = serde_json::json!({"evidence":saved,"authored":[null,true,false,-7,9,1.5,"escaped\ntext",{"keep":"unchanged"}]});
    fs::write(
        exports.join("nested/report.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    fs::write(exports.join("unknown.json"), b"{\"author\":true}").unwrap();
    fs::write(exports.join("malformed.json"), b"{unfinished").unwrap();
    for (args, expected) in [
        (
            vec!["verify", "history", root.to_str().unwrap()],
            "details pruned",
        ),
        (
            vec![
                "verify",
                "history",
                root.to_str().unwrap(),
                "--json",
                "--limit",
                "2",
            ],
            "details_available",
        ),
        (
            vec!["verify", "show", root.to_str().unwrap(), saved.id.as_str()],
            "Details pruned",
        ),
        (
            vec![
                "verify",
                "show",
                root.to_str().unwrap(),
                saved.id.as_str(),
                "--json",
            ],
            "pruned_at",
        ),
        (
            vec![
                "verify",
                "gc",
                root.to_str().unwrap(),
                "--dry-run",
                "--compact",
                "--exports",
                exports.to_str().unwrap(),
            ],
            "Preview",
        ),
        (
            vec![
                "verify",
                "gc",
                root.to_str().unwrap(),
                "--compact",
                "--json",
                "--exports",
                exports.to_str().unwrap(),
            ],
            "compacted",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
    }
    let reduced: serde_json::Value =
        serde_json::from_slice(&fs::read(exports.join("nested/report.json")).unwrap()).unwrap();
    assert_eq!(reduced["authored"], report["authored"]);
    assert!(reduced["evidence"].get("run").is_none());
    assert_eq!(reduced["evidence"]["summary"]["details_available"], false);
    assert_eq!(
        fs::read(exports.join("unknown.json")).unwrap(),
        b"{\"author\":true}"
    );
    assert_eq!(
        fs::read(exports.join("malformed.json")).unwrap(),
        b"{unfinished"
    );
    fs::remove_dir_all(root).unwrap();
}
