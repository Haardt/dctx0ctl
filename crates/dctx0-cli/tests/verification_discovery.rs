use std::{fs, process::Command};
#[test]
fn discovery_cli_reports_unconfirmed_candidates_without_executing_or_writing_application() {
    let root = std::env::temp_dir().join(format!("dctx0-cli-discovery-{}", std::process::id()));
    let spec = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir(&app).unwrap();
    fs::write(spec.join("w.dctx0"), "workspace W { value Item = Int }").unwrap();
    fs::write(
        app.join("one.ts"),
        "throw new Error('never execute'); export type Item = number;",
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    assert!(
        Command::new(binary)
            .args(["check", spec.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .success()
    );
    let database = fs::read(spec.join(".dctx0ctl/state.sqlite3")).unwrap();
    for json in [true, false] {
        let mut args = vec![
            "verify",
            "discover",
            spec.to_str().unwrap(),
            "--project",
            app.to_str().unwrap(),
        ];
        if json {
            args.push("--json");
        }
        let result = Command::new(binary).args(args).output().unwrap();
        #[cfg(feature = "typescript")]
        {
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            if json {
                let report: dctx0_engine::VerificationDiscovery =
                    serde_json::from_slice(&result.stdout).unwrap();
                assert!(
                    report
                        .subjects
                        .iter()
                        .any(|s| s.subject == "W.Item" && s.candidates.len() == 1)
                );
            } else {
                assert!(String::from_utf8_lossy(&result.stdout).contains("unconfirmed candidates"));
            }
        }
        #[cfg(not(feature = "typescript"))]
        {
            assert!(!result.status.success());
            assert!(String::from_utf8_lossy(&result.stderr).contains("inspector"));
        }
    }
    assert_eq!(
        database,
        fs::read(spec.join(".dctx0ctl/state.sqlite3")).unwrap()
    );
    assert!(!app.join(".dctx0").exists());
    let unsafe_benchmark = Command::new(binary)
        .args([
            "bench",
            spec.to_str().unwrap(),
            "verification-discover",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!unsafe_benchmark.status.success());
    let benchmark_database = root.join("benchmark.sqlite3");
    let benchmark = Command::new(binary)
        .args([
            "bench",
            spec.to_str().unwrap(),
            "verification-discover",
            "1",
            "--db",
            benchmark_database.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    #[cfg(feature = "typescript")]
    {
        assert!(
            benchmark.status.success(),
            "{}",
            String::from_utf8_lossy(&benchmark.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&benchmark.stdout).unwrap();
        assert_eq!(report["valid"], true);
        assert_eq!(
            report["application_source_hash"].as_str().unwrap().len(),
            64
        );
        assert!(report["elapsed_ms"].as_f64().unwrap() > 0.0);
        assert!(report["peak_rss_bytes"].as_u64().unwrap() > 0);
    }
    #[cfg(not(feature = "typescript"))]
    assert!(!benchmark.status.success());
    fs::remove_dir_all(root).unwrap();
}
