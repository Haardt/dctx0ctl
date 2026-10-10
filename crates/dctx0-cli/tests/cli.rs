use std::{fs, path::PathBuf, process::Command};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("dctx0ctl-{name}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn cli_parses_files_and_recursive_workspaces_and_reports_failures() {
    let sandbox = Sandbox::new("cli");
    fs::create_dir(sandbox.0.join("nested")).unwrap();
    fs::write(sandbox.0.join("a.dctx0"), "workspace W {}").unwrap();
    fs::write(
        sandbox.0.join("nested/b.dctx0"),
        "domain D { type T = Int }",
    )
    .unwrap();
    fs::write(sandbox.0.join("ignored.ts"), "invalid").unwrap();
    let path = sandbox.0.to_str().unwrap();
    let output = cli(&["parse", path]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().lines().count(), 2);
    let file = sandbox.0.join("a.dctx0");
    assert!(cli(&["parse", file.to_str().unwrap()]).status.success());
    let output = cli(&["bench-parse", path, "2"]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["files"], 2);
    assert_eq!(report["iterations"], 2);
    assert!(report["elapsed_ms"].as_f64().unwrap() > 0.0);
    assert!(cli(&["bench-parse", path]).status.success());
    fs::write(&file, "workspace W {").unwrap();
    let output = cli(&["parse", path]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("DCTX0_PARSE_BLOCK_UNCLOSED")
    );
    assert!(!cli(&["bench-parse", path]).status.success());
    for iterations in ["0", "wrong"] {
        assert!(!cli(&["bench-parse", path, iterations]).status.success());
    }
    fs::write(&file, [0xff]).unwrap();
    assert!(!cli(&["parse", path]).status.success());
    for args in [
        vec![],
        vec!["invalid"],
        vec!["parse"],
        vec!["invalid", path],
        vec!["parse", "/does-not-exist/dctx0ctl"],
    ] {
        assert!(!cli(&args).status.success());
    }
    let empty = Sandbox::new("empty");
    assert!(!cli(&["parse", empty.0.to_str().unwrap()]).status.success());
    assert!(
        !cli(&["bench-parse", empty.0.to_str().unwrap()])
            .status
            .success()
    );
}

#[test]
fn cli_serializes_complete_typed_reference_asts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in ["examples/cinema/dctx0", "examples/runatron-next/dctx0"] {
        assert!(
            cli(&["parse", root.join(path).to_str().unwrap()])
                .status
                .success()
        );
    }
}

#[test]
fn commands_publish_inspect_rename_and_benchmark_with_machine_readable_output() {
    let sandbox = Sandbox::new("commands");
    let file = sandbox.0.join("spec.dctx0");
    fs::write(
        &file,
        "workspace W { type A = Int; surface Api { operation get(a: A) -> A { pure } } }",
    )
    .unwrap();
    let path = sandbox.0.to_str().unwrap();
    let first = cli(&["--json", "check", path]);
    assert!(first.status.success(), "{:?}", first);
    let first: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    let id = first["snapshot_id"].as_str().unwrap();
    for args in [
        vec!["check", path],
        vec!["history", path],
        vec!["--json", "history", path],
        vec!["impact", path, "W.A"],
        vec!["--json", "impact", path, "W.A"],
        vec!["diff", path, id, "latest"],
        vec!["--json", "diff", path, id, "latest"],
        vec!["rename", path, "W.A", "B", "--dry-run"],
        vec!["--json", "rename", path, "W.A", "B", "--dry-run"],
    ] {
        assert!(cli(&args).status.success(), "{args:?}");
    }
    assert!(cli(&["rename", path, "W.A", "B"]).status.success());
    assert!(cli(&["diff", path, id, "latest"]).status.success());
    assert!(!cli(&["impact", path, "Missing"]).status.success());
    let database = sandbox.0.join("benchmark.sqlite3");
    for phase in [
        "compile",
        "graph",
        "snapshot",
        "diff",
        "diff-changed",
        "persist",
        "check",
        "check-changed",
    ] {
        let output = cli(&[
            "--db",
            database.to_str().unwrap(),
            "bench",
            path,
            phase,
            "2",
        ]);
        assert!(output.status.success(), "{phase}: {:?}", output);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["valid"], true);
        assert!(report["peak_rss_bytes"].as_u64().unwrap() > 0);
    }
    assert!(cli(&["bench-compile", path]).status.success());
    assert!(!cli(&["bench", path, "check"]).status.success());
    assert!(!cli(&["bench", path, "compile", "0"]).status.success());
    assert!(cli(&["--json", "bench-parse", path]).status.success());
    fs::write(&file, "workspace W { type A = Missing }").unwrap();
    assert!(!cli(&["check", path]).status.success());
    assert!(!cli(&["bench-compile", path]).status.success());
    assert!(!cli(&["--json", "check", path]).status.success());
}

#[test]
fn isolated_parser_driver_reports_valid_invalid_and_unreadable_inputs() {
    let sandbox = Sandbox::new("driver");
    let file = sandbox.0.join("spec.dctx0");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_dctx0-parse-bench"))
            .args(args)
            .output()
            .unwrap()
    };
    let path = sandbox.0.to_str().unwrap();
    assert!(!run(&["bench-parse", path]).status.success());
    fs::write(&file, "workspace W {}").unwrap();
    fs::create_dir(sandbox.0.join("nested")).unwrap();
    fs::write(sandbox.0.join("nested/spec.dctx0"), "domain D {}").unwrap();
    fs::write(sandbox.0.join("ignored.txt"), "ignore").unwrap();
    let output = run(&["bench-parse", path, "2"]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["files"], 2);
    assert!(
        run(&["bench-parse", file.to_str().unwrap()])
            .status
            .success()
    );
    fs::write(&file, "workspace W {").unwrap();
    assert!(!run(&["bench-parse", path]).status.success());
    fs::write(&file, [0xff]).unwrap();
    assert!(!run(&["bench-parse", path]).status.success());
    for args in [
        vec![],
        vec!["invalid", path],
        vec!["bench-parse", path, "0"],
        vec!["bench-parse", path, "wrong"],
        vec!["bench-parse", "/missing-dctx0ctl"],
    ] {
        assert!(!run(&args).status.success());
    }
}

#[test]
fn benchmarks_reject_invalid_change_scenarios_and_skip_symlink_loops() {
    let sandbox = Sandbox::new("scenario");
    fs::write(
        sandbox.0.join("spec.dctx0"),
        "workspace W { domain BenchmarkDelta {} }",
    )
    .unwrap();
    let path = sandbox.0.to_str().unwrap();
    assert!(!cli(&["bench", path, "diff-changed"]).status.success());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&sandbox.0, sandbox.0.join("loop")).unwrap();
        assert!(cli(&["parse", path]).status.success());
    }
}

#[test]
fn human_readable_changes_preserve_all_facet_names() {
    let sandbox = Sandbox::new("human-facets");
    let original = sandbox.0.join("a.dctx0");
    let moved = sandbox.0.join("b.dctx0");
    fs::write(&original, "workspace W { type T { id: Int } surface Api { operation get() -> T { ensures success } } constraint \"One\" }").unwrap();
    let path = sandbox.0.to_str().unwrap();
    assert!(cli(&["check", path]).status.success());
    fs::rename(&original, &moved).unwrap();
    fs::write(&moved, "workspace W { type T { id: Int; extra: String } surface Api { operation get() -> T { ensures not success } } constraint \"Two\" }").unwrap();
    let output = cli(&["check", path]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "[contract, location]",
        "[behavior, location]",
        "[facts, location]",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(text.contains("PASS:"));
    assert!(text.contains("source changed: true"));
}

#[test]
fn verification_core_benchmarks_measure_both_compiled_examples_without_native_claims() {
    for example in ["cinema", "runatron-next"] {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../examples/{example}/dctx0"));
        for phase in [
            #[cfg(feature = "typescript")]
            "verification-harness",
            #[cfg(feature = "typescript")]
            "verification-init",
            "verification-history-publish",
            "verification-history-read",
            "verification-project",
            "verification-generate",
            "verification-evaluate-unbound",
            "verification-assess-unbound",
        ] {
            let output = cli(&["bench", input.to_str().unwrap(), phase, "1"]);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(result["valid"], true);
            assert!(result["elapsed_ms"].as_f64().unwrap() > 0.0);
            assert!(result["peak_rss_bytes"].as_u64().unwrap() > 0);
        }
    }
}

#[test]
fn verification_setup_and_configured_scope_preserve_files_and_never_execute_or_adopt() {
    let t = Sandbox::new("verification-setup-cli");
    let spec = t.0.join("spec");
    let app = t.0.join("app");
    fs::create_dir(&spec).unwrap();
    fs::create_dir(&app).unwrap();
    fs::write(spec.join("w.dctx0"), "workspace W { type Number = Int }").unwrap();
    let input = spec.to_str().unwrap();
    let project = app.to_str().unwrap();
    let unsupported = cli(&[
        "verify",
        "init",
        input,
        "--project",
        project,
        "--target",
        "java",
    ]);
    assert!(!unsupported.status.success());
    assert!(!app.join(".dctx0").exists());
    #[cfg(feature = "typescript")]
    {
        assert!(cli(&["check", input]).status.success());
        let result = cli(&[
            "verify",
            "init",
            input,
            "--project",
            project,
            "--target",
            "typescript",
            "--json",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["executed"], false);
        assert_eq!(report["created"].as_array().unwrap().len(), 5);
        let empty = cli(&["verify", "plan", input]);
        assert!(!empty.status.success());
        assert!(String::from_utf8_lossy(&empty.stderr).contains("empty scope"));
        let path = app.join(".dctx0/testing/verification.json");
        let mut config: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        config["scope"]["subjects"] = serde_json::json!(["W.Number"]);
        fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        let plan = cli(&["verify", "plan", input, "--json"]);
        assert!(
            plan.status.success(),
            "{}",
            String::from_utf8_lossy(&plan.stderr)
        );
        let plan: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
        assert_eq!(plan["scope"]["subjects"], serde_json::json!(["W.Number"]));
        let explicit = cli(&[
            "verify",
            "plan",
            input,
            "--project",
            project,
            "--subject",
            "W.Number",
        ]);
        assert!(explicit.status.success());
        let repeated = cli(&[
            "verify",
            "init",
            input,
            "--project",
            project,
            "--target",
            "typescript",
        ]);
        assert!(repeated.status.success());
        assert!(String::from_utf8_lossy(&repeated.stdout).contains("0 created"));
    }
    #[cfg(not(feature = "typescript"))]
    {
        assert!(
            !cli(&[
                "verify",
                "init",
                input,
                "--project",
                project,
                "--target",
                "typescript"
            ])
            .status
            .success()
        );
    }
}
