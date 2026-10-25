use serde_json::Value;
use std::{fs, process::Command};

#[test]
fn binding_requirements_cli_needs_no_implementation_and_keeps_history_read_only() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-bindings-{}", std::process::id()));
    let spec = root.join("spec");
    fs::create_dir_all(&spec).unwrap();
    fs::write(spec.join("w.dctx0"), "workspace W { value Id = Int { range 1..3 } surface Api { operation next(id:Id) -> Id { pure } operation future() -> String } }").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap()
    };
    let report = run(&[
        "verify",
        "bindings",
        "plan",
        "spec",
        "--subject",
        "W.Api.next",
        "--json",
    ]);
    assert!(!report.status.success());
    assert!(!spec.join(".dctx0ctl/state.sqlite3").exists());
    assert!(run(&["check", "spec"]).status.success());
    let before = fs::read(spec.join(".dctx0ctl/state.sqlite3")).unwrap();
    for json in [false, true] {
        let mut args = vec![
            "verify",
            "bindings",
            "plan",
            "spec",
            "--subject",
            "W.Api.next",
        ];
        if json {
            args.push("--json");
        }
        let report = run(&args);
        assert!(
            report.status.success(),
            "{}",
            String::from_utf8_lossy(&report.stderr)
        );
        if json {
            let value: Value = serde_json::from_slice(&report.stdout).unwrap();
            assert!(
                value["requirements"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["subject"] == "W.Api.next" && r["role"] == "operation")
            );
            assert!(
                !value["requirements"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["subject"] == "W.Api.future")
            );
            assert!(value["declarations"].get("W.Id").is_some());
        } else {
            assert!(
                String::from_utf8_lossy(&report.stdout)
                    .contains("Planning does not confirm bindings")
            );
        }
    }
    assert_eq!(
        before,
        fs::read(spec.join(".dctx0ctl/state.sqlite3")).unwrap()
    );
    assert!(!root.join(".dctx0").exists());
    fs::create_dir_all(root.join("app/.dctx0/testing")).unwrap();
    let configuration = dctx0_engine::VerificationConfiguration {
        schema_version: 1,
        target: "typescript".into(),
        runtime: "bun".into(),
        adapter: ".dctx0/testing/adapter.ts".into(),
        fixtures: ".dctx0/testing/examples.json".into(),
        dependency_resolution: None,
        execution: None,
        scope: dctx0_engine::ConfiguredVerificationScope {
            subjects: ["W.Id".into()].into(),
            tasks: Default::default(),
        },
        budgets: dctx0_core::VerificationBudgets {
            generated_cases: 2,
            ..Default::default()
        },
    };
    fs::write(
        root.join("app/.dctx0/testing/verification.json"),
        serde_json::to_vec(&configuration).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("app/.dctx0/testing/examples.json"),
        "{\"examples\":{}}",
    )
    .unwrap();
    for subject in [None, Some("W.Api.next")] {
        let mut args = vec![
            "verify",
            "bindings",
            "plan",
            "spec",
            "--project",
            "app",
            "--json",
        ];
        if let Some(subject) = subject {
            args.extend(["--subject", subject]);
        }
        let report = run(&args);
        assert!(
            report.status.success(),
            "{}",
            String::from_utf8_lossy(&report.stderr)
        );
        let value: Value = serde_json::from_slice(&report.stdout).unwrap();
        assert_eq!(value["selected_subjects"][0], subject.unwrap_or("W.Id"));
    }
    fs::write(root.join("fixtures.json"), "{\"examples\":{}}").unwrap();
    assert!(
        run(&[
            "verify",
            "bindings",
            "plan",
            "spec",
            "--subject",
            "W.Id",
            "--fixtures",
            "fixtures.json"
        ])
        .status
        .success()
    );
    assert!(
        !run(&[
            "verify",
            "bindings",
            "plan",
            "spec",
            "--subject",
            "W.Unknown"
        ])
        .status
        .success()
    );
    assert_eq!(
        before,
        fs::read(spec.join(".dctx0ctl/state.sqlite3")).unwrap()
    );
    for args in [
        vec!["verify", "bindings", "plan", "spec", "--task", "0"],
        vec!["verify", "bindings", "plan", "spec", "--task", "not-a-task"],
        vec!["verify", "bindings", "plan", "spec", "--dry-run"],
        vec!["verify", "bindings", "plan", "spec"],
        vec!["verify", "bindings", "plan", "spec", "--task", "T999"],
    ] {
        assert!(!run(&args).status.success());
    }
    let benchmark = run(&["bench", "spec", "verification-bindings-plan", "1"]);
    assert!(
        benchmark.status.success(),
        "{}",
        String::from_utf8_lossy(&benchmark.stderr)
    );
    let measured: Value = serde_json::from_slice(&benchmark.stdout).unwrap();
    assert!(measured["elapsed_ms"].as_f64().unwrap() > 0.0);
    #[cfg(target_os = "linux")]
    assert!(measured["peak_rss_bytes"].as_u64().unwrap() > 0);
    assert_eq!(
        before,
        fs::read(spec.join(".dctx0ctl/state.sqlite3")).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}
