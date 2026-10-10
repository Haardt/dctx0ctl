use std::{fs, process::Command};
#[test]
fn run_stores_unknown_scaffold_evidence_and_show_reads_it_without_a_new_check() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-native-run-{}", std::process::id()));
    let specs = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&specs).unwrap();
    fs::create_dir(&app).unwrap();
    fs::write(
        specs.join("w.dctx0"),
        "workspace W { value A = Int { range 1..2 } }",
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let call = |args: &[&str]| Command::new(binary).args(args).output().unwrap();
    let output = call(&["check", specs.to_str().unwrap(), "--json"]);
    assert!(output.status.success());
    let init = call(&[
        "verify",
        "init",
        specs.to_str().unwrap(),
        "--project",
        app.to_str().unwrap(),
        "--target",
        "typescript",
        "--subject",
        "W.A",
        "--json",
    ]);
    #[cfg(feature = "typescript")]
    {
        assert!(
            init.status.success(),
            "{}",
            String::from_utf8_lossy(&init.stderr)
        );
        for json in [true, false] {
            let mut args = vec!["verify", "run", specs.to_str().unwrap(), "--no-adopt"];
            if json {
                args.push("--json");
            }
            let run = call(&args);
            assert!(!run.status.success());
            if json {
                let report: dctx0_engine::NativeVerificationReport =
                    serde_json::from_slice(&run.stdout).unwrap();
                assert!(matches!(
                    report.evidence.run.result,
                    dctx0_core::VerificationResult::Unknown { .. }
                ));
                assert!(report.adopted_tasks.is_empty());
                assert!(report.adoption_issues.is_empty());
                let replay = call(&[
                    "verify",
                    "replay",
                    specs.to_str().unwrap(),
                    &report.evidence.id,
                    "--case",
                    &report.evidence.run.cases[0].id,
                    "--json",
                ]);
                assert!(!replay.status.success());
                let repeated: dctx0_engine::VerificationReplayReport =
                    serde_json::from_slice(&replay.stdout).unwrap();
                assert_eq!(repeated.original_run, report.evidence.id);
                assert!(matches!(
                    repeated.case_result,
                    Some(dctx0_core::VerificationResult::Unknown { .. })
                ));
                assert!(repeated.evidence.adopted_tasks.is_empty());
                let show = call(&[
                    "verify",
                    "show",
                    specs.to_str().unwrap(),
                    &report.evidence.id,
                    "--json",
                ]);
                assert!(show.status.success());
                let saved: dctx0_store::StoredVerificationRun =
                    serde_json::from_slice(&show.stdout).unwrap();
                assert_eq!(saved, report.evidence);
            } else {
                assert!(String::from_utf8_lossy(&run.stdout).contains("Native verification"));
            }
        }
        let project = dctx0_engine::Project::new(&specs, None).unwrap();
        assert_eq!(project.history().unwrap().len(), 1);
    }
    #[cfg(not(feature = "typescript"))]
    {
        assert!(!init.status.success());
        let run = call(&["verify", "run", specs.to_str().unwrap()]);
        assert!(!run.status.success());
        assert!(String::from_utf8_lossy(&run.stderr).contains("No executable TypeScript"));
        let replay = call(&[
            "verify",
            "replay",
            specs.to_str().unwrap(),
            "missing",
            "--case",
            "missing",
        ]);
        assert!(!replay.status.success());
        assert!(String::from_utf8_lossy(&replay.stderr).contains("No executable TypeScript"));
    }
    fs::remove_dir_all(root).unwrap();
}
