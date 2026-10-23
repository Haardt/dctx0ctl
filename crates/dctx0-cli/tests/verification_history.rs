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
    let cli = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .output()
            .unwrap()
    };
    let sql = "SELECT c.subject,c.outcome,COUNT(*) FROM dctx0_cases c JOIN dctx0_runs r USING(run_id) WHERE c.run_id=:run_id GROUP BY c.subject,c.outcome ORDER BY c.subject,c.outcome";
    let parameter = format!("run_id=\"{}\"", saved.id);
    for json in [false, true] {
        for args in [
            vec!["query", "schema", root.to_str().unwrap()],
            vec![
                "query",
                "sql",
                root.to_str().unwrap(),
                "--sql",
                sql,
                "--param",
                &parameter,
            ],
            vec!["query", "prepare", root.to_str().unwrap()],
        ] {
            let mut args = args;
            if json {
                args.push("--json");
            }
            let result = cli(&args);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
    let export = root.join("export.json");
    let result = cli(&[
        "verify",
        "export",
        root.to_str().unwrap(),
        &saved.id,
        "--output",
        export.to_str().unwrap(),
    ]);
    assert!(result.status.success());
    assert_eq!(
        serde_json::from_slice::<dctx0_store::StoredVerificationRun>(&fs::read(&export).unwrap())
            .unwrap(),
        saved
    );
    let result = cli(&[
        "verify",
        "export",
        root.to_str().unwrap(),
        &saved.id,
        "--case",
        "C3",
        "--output",
        export.to_str().unwrap(),
    ]);
    assert!(result.status.success());
    assert_eq!(
        serde_json::from_slice::<VerificationCaseRecord>(&fs::read(&export).unwrap()).unwrap(),
        run.cases[2]
    );
    let queryfile = root.join("investigation.sql");
    fs::write(
        &queryfile,
        "SELECT task_id,title,ready FROM dctx0_tasks WHERE work=:work ORDER BY wave,task_id",
    )
    .unwrap();
    assert!(
        cli(&[
            "query",
            "sql",
            root.to_str().unwrap(),
            "--file",
            queryfile.to_str().unwrap(),
            "--param",
            "work=\"Open\"",
            "--json"
        ])
        .status
        .success()
    );
    for extra in [
        vec!["--param", "x=[]"],
        vec!["--param", "work=null", "--param", "work=true"],
        vec!["--param", "!bad=1"],
        vec!["--param", "wrong"],
        vec!["--max-output-bytes", "1"],
        vec!["--sql", "SELECT 1"],
        vec!["--plan", "999999"],
        vec!["--as-of", "-1"],
    ] {
        let mut args = vec![
            "query",
            "sql",
            root.to_str().unwrap(),
            "--file",
            queryfile.to_str().unwrap(),
        ];
        args.extend(extra);
        assert!(!cli(&args).status.success());
    }
    {
        use std::io::Write;
        let mut child = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args([
                "query",
                "sql",
                root.to_str().unwrap(),
                "--file",
                "-",
                "--json",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"SELECT COUNT(*) FROM dctx0_tasks")
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        assert!(
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()["rows"][0][0]
                .as_i64()
                .unwrap()
                > 0
        );
    }
    for unsafe_sql in [
        "SELECT * FROM query_rows",
        "SELECT * FROM verification_runs",
        "DELETE FROM dctx0_tasks",
        "PRAGMA user_version",
        "SELECT 1;SELECT 2",
    ] {
        assert!(
            !cli(&["query", "sql", root.to_str().unwrap(), "--sql", unsafe_sql])
                .status
                .success()
        );
    }
    let database_before_show = fs::read(&p.database).unwrap();
    fs::write(root.join("w.dctx0"), "invalid now").unwrap();
    for json in [false, true] {
        let mut args = vec![
            "verify",
            "show",
            root.to_str().unwrap(),
            saved.id.as_str(),
            "--full",
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
    assert_eq!(compact["summary"]["case_count"], 3);
    assert_eq!(compact["summary"]["result"]["status"], "FAIL");
    assert!(compact.get("cases").is_none());
    let filtered_summary = show(&["--summary", "--outcome", "PASS"]);
    assert!(filtered_summary.status.success());
    let filtered_summary: serde_json::Value =
        serde_json::from_slice(&filtered_summary.stdout).unwrap();
    assert_eq!(filtered_summary["matching_cases"], 1);
    assert_eq!(filtered_summary["summary"]["result"]["status"], "FAIL");
    assert!(filtered_summary["cases"].as_array().unwrap().is_empty());
    let filtered = show(&["--outcome", "FAIL,UNKNOWN", "--limit", "1"]);
    assert!(filtered.status.success());
    let filtered: serde_json::Value = serde_json::from_slice(&filtered.stdout).unwrap();
    assert_eq!(filtered["matching_cases"], 2);
    assert_eq!(filtered["cases"][0]["id"], "C1");
    for subject in ["W", "W.Number"] {
        let result = show(&["--subject", subject]);
        assert!(result.status.success());
        let result: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(result["matching_cases"], 3);
        assert_eq!(result["summary"]["result"]["status"], "FAIL");
    }
    let unrelated = show(&["--subject", "W.Numb"]);
    assert!(unrelated.status.success());
    let unrelated: serde_json::Value = serde_json::from_slice(&unrelated.stdout).unwrap();
    assert_eq!(unrelated["matching_cases"], 0);
    assert_eq!(unrelated["summary"]["result"]["status"], "FAIL");
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

#[test]
fn storage_commands_build_verified_copies_preserve_task_queries_and_keep_a_backup() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-compact-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("w.dctx0"), "workspace W { type A = Int }").unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let call = |args: Vec<String>| -> serde_json::Value {
        let out = Command::new(binary)
            .args(&args)
            .arg("--json")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let spec = root.to_str().unwrap().to_owned();
    call(vec!["check".into(), spec.clone()]);
    call(vec!["tasks".into(), "plan".into(), spec.clone()]);
    let before = call(vec!["storage".into(), "inspect".into(), spec.clone()]);
    assert_eq!(before["schema_version"], 7);
    let preview = call(vec![
        "storage".into(),
        "compact".into(),
        spec.clone(),
        "--dry-run".into(),
    ]);
    assert_eq!(preview["dry_run"], true);
    let destination = root.join("copy.sqlite3");
    let copy = call(vec![
        "storage".into(),
        "compact".into(),
        spec.clone(),
        "--output".into(),
        destination.to_str().unwrap().into(),
    ]);
    assert_eq!(copy["installed"], false);
    let query = call(vec![
        "--db".into(),
        destination.to_str().unwrap().into(),
        "query".into(),
        "sql".into(),
        spec.clone(),
        "--sql".into(),
        "SELECT COUNT(*) FROM dctx0_tasks".into(),
    ]);
    assert_eq!(query["rows"][0][0], 1);
    let prepared = call(vec!["storage".into(), "compact".into(), spec.clone()]);
    assert_eq!(prepared["installed"], false);
    let candidate = prepared["destination"].as_str().unwrap();
    use sha2::{Digest, Sha256};
    let evidence = root.join("comparison.json");
    let source = root.join(".dctx0ctl/state.sqlite3");
    let mut record = serde_json::json!({"status":"complete","source":source,"copy":candidate,"copy_sha256":format!("{:x}",Sha256::digest(fs::read(candidate).unwrap())),"binary_hashes":{"new":format!("{:x}",Sha256::digest(fs::read(binary).unwrap()))}});
    fs::write(&evidence, serde_json::to_vec(&record).unwrap()).unwrap();
    let verified = call(vec![
        "storage".into(),
        "verify-copy".into(),
        spec.clone(),
        "--copy".into(),
        candidate.into(),
    ]);
    assert_eq!(verified["integrity_ok"], true);
    record["status"] = serde_json::json!("failed-regression");
    fs::write(&evidence, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(
        !Command::new(binary)
            .args([
                "storage",
                "install-copy",
                &spec,
                "--copy",
                candidate,
                "--evidence",
                evidence.to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success()
    );
    record["status"] = serde_json::json!("complete");
    fs::write(&evidence, serde_json::to_vec(&record).unwrap()).unwrap();
    let original = fs::read(candidate).unwrap();
    let mut damaged = original.clone();
    damaged[0] ^= 1;
    fs::write(candidate, damaged).unwrap();
    assert!(
        !Command::new(binary)
            .args([
                "storage",
                "install-copy",
                &spec,
                "--copy",
                candidate,
                "--evidence",
                evidence.to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success()
    );
    fs::write(candidate, original).unwrap();
    let installed = call(vec![
        "storage".into(),
        "install-copy".into(),
        spec.clone(),
        "--copy".into(),
        candidate.into(),
        "--evidence".into(),
        evidence.to_str().unwrap().into(),
    ]);
    assert_eq!(installed["installed"], true);
    assert!(std::path::Path::new(installed["backup"].as_str().unwrap()).exists());
    let again = call(vec![
        "query".into(),
        "sql".into(),
        spec,
        "--sql".into(),
        "SELECT COUNT(*) FROM dctx0_tasks".into(),
    ]);
    assert_eq!(again["rows"], query["rows"]);
    let mut recovered = call(vec![
        "storage".into(),
        "recover".into(),
        root.to_str().unwrap().into(),
    ]);
    assert_eq!(recovered["recovered"], false);
    let journal = source.with_extension("storage-install.json");
    fs::write(
        &journal,
        serde_json::to_vec(&serde_json::json!({"source":source,"backup":installed["backup"]}))
            .unwrap(),
    )
    .unwrap();
    recovered = call(vec![
        "storage".into(),
        "recover".into(),
        root.to_str().unwrap().into(),
    ]);
    assert_eq!(recovered["restored_backup"], false);
    fs::write(
        &journal,
        serde_json::to_vec(&serde_json::json!({"source":source,"backup":installed["backup"]}))
            .unwrap(),
    )
    .unwrap();
    fs::remove_file(&source).unwrap();
    recovered = call(vec![
        "storage".into(),
        "recover".into(),
        root.to_str().unwrap().into(),
    ]);
    assert_eq!(recovered["restored_backup"], true);
    fs::remove_dir_all(root).unwrap();
}
