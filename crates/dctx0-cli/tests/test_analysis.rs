use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
impl Fixture {
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args([
                "-c",
                "user.name=Research fixture",
                "-c",
                "user.email=research@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    fn cli(&self, args: &[&str]) -> (bool, Value) {
        let out = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .arg("experimental")
            .args(args)
            .output()
            .unwrap();
        let data = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| json!({"stderr":String::from_utf8_lossy(&out.stderr)}));
        (out.status.success(), data)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
    fn commit(&self, name: &str) -> String {
        self.git(&["add", "."]);
        self.git(&["commit", "-qm", name]);
        self.git(&["rev-parse", "HEAD"])
    }
}
fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "dctx0-research-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir(root.join("tests")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname=\"research-probe\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n[features]\nextra=[]\n").unwrap();
    fs::write(root.join("src/lib.rs"),"pub fn first()->u32 {1}\npub fn second()->u32 {2}\n#[test] fn same(){assert_eq!(first(),1);}\n#[test] fn other(){assert_eq!(second(),2);}\n#[test] #[ignore] fn ignored(){panic!(\"ignored\");}\n#[cfg(feature=\"extra\")] #[test] fn extra(){assert!(true);}\n").unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main(){}\n#[test] fn same(){assert_eq!(research_probe::first(),1);}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/integration.rs"),
        "#[test] fn same(){assert_eq!(research_probe::second(),2);}\n",
    )
    .unwrap();
    fs::write(root.join(".gitignore"), "/.dctx0ctl/\n/target/\n").unwrap();
    let result = Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(result.status.success());
    let f = Fixture(root);
    f.git(&["init", "-q"]);
    f.commit("clean baseline");
    f
}
#[test]
fn catalogs_complete_isolated_reference_and_shadow_freezes_before_labels() {
    let f = fixture();
    let root = f.path();
    assert!(!f.cli(&["test-analysis", "show", root, "absent"]).0);
    assert!(!f.0.join(".dctx0ctl/state.sqlite3").exists());
    let (ok, catalog) = f.cli(&["coverage", "catalog", root, "--all-features"]);
    assert!(ok, "{catalog}");
    assert_eq!(catalog["tests"].as_array().unwrap().len(), 6);
    assert!(
        !catalog["tests"][0]["version"]["key"]["project"]
            .as_str()
            .unwrap()
            .contains(f.path())
    );
    let clone = f.0.join(".dctx0ctl/clone");
    let cloned = Command::new("git")
        .args([
            "clone",
            "--local",
            "--quiet",
            f.path(),
            clone.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(cloned.status.success());
    let (ok, other) = f.cli(&[
        "coverage",
        "catalog",
        clone.to_str().unwrap(),
        "--all-features",
    ]);
    assert!(ok, "{other}");
    assert_eq!(
        catalog["tests"][0]["version"]["key"],
        other["tests"][0]["version"]["key"]
    );

    assert_eq!(
        catalog["tests"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["version"]["key"]["test_name"] == "same")
            .count(),
        3
    );
    assert!(
        catalog["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["kind"] == "doctest" && t["supported"] == false)
    );
    let (ok, report) = f.cli(&["coverage", "collect", root, "--all-features"]);
    assert!(ok, "{report}");
    assert_eq!(report["completeness"]["reference_evaluable"], true);
    let baseline = report["collection_id"].as_str().unwrap();
    let original = fs::read_to_string(f.0.join("src/lib.rs")).unwrap();
    fs::write(
        f.0.join("src/lib.rs"),
        original.replace("first()->u32 {1}", "first()->u32 {9}"),
    )
    .unwrap();
    let (ok, report) = f.cli(&[
        "test-analysis",
        "shadow",
        root,
        "--base",
        "HEAD",
        "--baseline",
        baseline,
    ]);
    assert!(ok, "{report}");
    assert_eq!(report["missed_failures"].as_array().unwrap().len(), 0);
    assert_eq!(report["detected_failures"].as_array().unwrap().len(), 2);
    assert_eq!(report["failure_recall"], 1.0);
    assert_eq!(
        fs::read_to_string(f.0.join("src/lib.rs")).unwrap(),
        original.replace("first()->u32 {1}", "first()->u32 {9}")
    );
    let prediction = report["prediction"].as_str().unwrap();
    let (ok, show) = f.cli(&["test-analysis", "show", root, prediction]);
    assert!(ok, "{show}");
    assert_eq!(show["events"][0]["kind"], "prediction_frozen");
    assert_eq!(show["events"][1]["kind"], "evaluation_published");
    let frozen = show["events"][0]["at"]
        .as_str()
        .unwrap()
        .parse::<u128>()
        .unwrap();
    let reference = report["reference_collection"].as_str().unwrap();
    for observation in show["records"].as_array().unwrap().iter().filter(|r| {
        r["kind"] == "Observation"
            && r["data"]["collection"] == reference
            && r["data"]["outcome"] != "Ignored"
    }) {
        assert!(
            frozen
                <= observation["data"]["started_at"]
                    .as_str()
                    .unwrap()
                    .parse::<u128>()
                    .unwrap()
        );
    }

    assert_eq!(show["record"]["data"]["can_skip_other_tests"], false);
    let (ok, repeated) = f.cli(&[
        "test-analysis",
        "shadow",
        root,
        "--base",
        "HEAD",
        "--baseline",
        baseline,
    ]);
    assert!(ok, "{repeated}");
    assert_ne!(repeated["prediction"], report["prediction"]);
    assert_ne!(
        repeated["reference_collection"],
        report["reference_collection"]
    );

    assert!(
        !f.cli(&[
            "test-analysis",
            "shadow",
            root,
            "--base",
            "HEAD",
            "--baseline",
            "legacy-id"
        ])
        .0
    );
    let (_, dirty) = f.cli(&["coverage", "collect", root, "--all-features"]);
    assert_eq!(dirty["completeness"]["reference_evaluable"], true);
    assert!(
        !f.cli(&[
            "test-analysis",
            "shadow",
            root,
            "--base",
            "HEAD",
            "--baseline",
            dirty["collection_id"].as_str().unwrap()
        ])
        .0
    );
    f.commit("new source revision");
    assert!(
        !f.cli(&[
            "test-analysis",
            "shadow",
            root,
            "--base",
            "HEAD",
            "--baseline",
            baseline
        ])
        .0
    );
}
#[test]
fn chronological_replay_resumes_without_rewriting_records_and_exports_labels() {
    let f = fixture();
    let base = f.git(&["rev-parse", "HEAD"]);
    let original = fs::read_to_string(f.0.join("src/lib.rs")).unwrap();
    fs::write(
        f.0.join("src/lib.rs"),
        original.replace("first()->u32 {1}", "first()->u32 {9}"),
    )
    .unwrap();
    let broken = f.commit("production failure");
    fs::write(f.0.join("src/lib.rs"), &original).unwrap();
    let repaired = f.commit("repair");
    fs::write(
        f.0.join("tests/new.rs"),
        "#[test] fn added(){assert_eq!(research_probe::first(),1);}\n",
    )
    .unwrap();
    let added = f.commit("new test");
    let configuration = fs::read_to_string(f.0.join("Cargo.toml")).unwrap();
    fs::write(
        f.0.join("Cargo.toml"),
        format!("{configuration}\n[profile.test]\nopt-level=1\n"),
    )
    .unwrap();
    let configured = f.commit("test configuration");
    let manifest = json!({"schema_version":1,"scope":{"package":null,"filter":null,"all_features":false,"no_default_features":false,"features":[]},"timeout_secs":60,"strategy":"coverage","pairs":[{"base":base,"head":broken},{"base":broken,"head":repaired},{"base":repaired,"head":added},{"base":added,"head":configured}]});
    let path = f.0.join(".dctx0ctl/manifest.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let (ok, replay) = f.cli(&[
        "test-analysis",
        "replay",
        f.path(),
        "--manifest",
        path.to_str().unwrap(),
    ]);
    assert!(ok, "{replay}");
    let id = replay["id"].as_str().unwrap();
    let (ok, before) = f.cli(&["test-analysis", "show", f.path(), id]);
    assert!(ok);
    assert_eq!(
        before["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "Collection")
            .count(),
        5
    );
    let (ok, resumed) = f.cli(&[
        "test-analysis",
        "replay",
        f.path(),
        "--manifest",
        path.to_str().unwrap(),
        "--resume",
        id,
    ]);
    assert!(ok, "{resumed}");
    let (ok, after) = f.cli(&["test-analysis", "show", f.path(), id]);
    assert!(ok);
    assert_eq!(before, after);
    let output = f.0.join(".dctx0ctl/dataset.ndjson");
    assert!(
        f.cli(&[
            "test-analysis",
            "export",
            f.path(),
            id,
            "--output",
            output.to_str().unwrap()
        ])
        .0
    );
    let rows: Vec<Value> = fs::read_to_string(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(rows.iter().any(|r| r["kind"] == "Prediction"));
    assert!(rows.iter().any(|r| r["kind"] == "Evaluation"));
    let (ok, recomputed) = f.cli(&[
        "test-analysis",
        "recompute",
        f.0.join(".dctx0ctl/dataset.ndjson").to_str().unwrap(),
    ]);
    assert!(ok, "{recomputed}");
    for evaluation in recomputed.as_array().unwrap() {
        assert!(
            rows.iter()
                .any(|r| r["kind"] == "Evaluation" && r["data"] == *evaluation)
        );
    }

    assert_eq!(f.git(&["rev-parse", "HEAD"]), configured);
    let (ok, predictions) = f.cli(&[
        "test-analysis",
        "recompute",
        f.0.join(".dctx0ctl/dataset.ndjson").to_str().unwrap(),
        "--stage",
        "prediction",
    ]);
    assert!(ok, "{predictions}");
    for prediction in predictions.as_array().unwrap() {
        assert!(
            rows.iter()
                .any(|r| r["kind"] == "Prediction" && r["data"]["id"] == prediction["id"])
        );
    }
    assert!(
        !f.cli(&[
            "test-analysis",
            "export",
            f.path(),
            id,
            "--output",
            f.0.join("missing/dataset.ndjson").to_str().unwrap()
        ])
        .0
    );
    let frozen_prediction =
        rows.iter().find(|row| row["kind"] == "Prediction").unwrap()["data"].clone();
    for (kind, key) in [
        ("Catalog", "base_catalog"),
        ("Collection", "baseline_collection"),
    ] {
        let partial = f.0.join(format!(".dctx0ctl/missing-{kind}.ndjson"));
        let lines: Vec<_> = rows
            .iter()
            .filter(|row| !(row["kind"] == kind && row["data"]["id"] == frozen_prediction[key]))
            .map(|row| serde_json::to_string(row).unwrap())
            .collect();
        fs::write(&partial, lines.join("\n") + "\n").unwrap();
        assert!(
            !f.cli(&[
                "test-analysis",
                "recompute",
                partial.to_str().unwrap(),
                "--stage",
                "prediction"
            ])
            .0
        );
    }
    let mut backwards = manifest;
    backwards["pairs"] = json!([{"base":repaired,"head":added},{"base":base,"head":broken}]);
    fs::write(&path, serde_json::to_vec(&backwards).unwrap()).unwrap();
    assert!(
        !f.cli(&[
            "test-analysis",
            "replay",
            f.path(),
            "--manifest",
            path.to_str().unwrap()
        ])
        .0
    );
}

#[test]
fn scopes_unsupported_harness_timeout_crash_and_build_errors_remain_explicit() {
    let f = fixture();
    let (ok, catalog) = f.cli(&[
        "coverage",
        "catalog",
        f.path(),
        "--package",
        "research-probe",
        "--filter",
        "same",
        "--no-default-features",
        "--features",
        "extra",
    ]);
    assert!(ok, "{catalog}");
    assert_eq!(catalog["tests"].as_array().unwrap().len(), 3);
    assert_eq!(catalog["scope"]["features"], json!(["extra"]));
    let manifest = fs::read_to_string(f.0.join("Cargo.toml")).unwrap();
    fs::write(
        f.0.join("Cargo.toml"),
        format!("{manifest}\n[[test]]\nname=\"custom\"\npath=\"tests/custom.rs\"\nharness=false\n"),
    )
    .unwrap();
    fs::write(
        f.0.join("tests/custom.rs"),
        "fn main(){panic!(\"custom harness must not execute\");}\n",
    )
    .unwrap();
    fs::write(f.0.join("tests/abnormal.rs"),"#[test] fn timeout(){std::thread::sleep(std::time::Duration::from_secs(5));}\n#[test] fn crash(){std::process::abort();}\n#[test] fn failure(){panic!(\"ordinary test failure\");}\n").unwrap();
    f.commit("abnormal and unsupported tests");
    let (ok, catalog) = f.cli(&["coverage", "catalog", f.path()]);
    assert!(ok, "{catalog}");
    assert!(
        catalog["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "custom" && t["supported"] == false)
    );
    let (ok, report) = f.cli(&[
        "coverage",
        "collect",
        f.path(),
        "--filter",
        "abnormal",
        "--timeout-secs",
        "2",
    ]);
    assert!(!ok, "{report}");
    assert_eq!(
        report["completeness"]["all_executable_tests_observed"],
        true
    );
    assert_eq!(report["completeness"]["reference_evaluable"], false);
    let (_, show) = f.cli(&[
        "coverage",
        "collection",
        f.path(),
        report["collection_id"].as_str().unwrap(),
    ]);
    let outcomes: Vec<_> = show["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "Observation")
        .map(|r| r["data"]["outcome"].as_str().unwrap())
        .collect();
    assert!(outcomes.contains(&"Timeout"));
    assert!(outcomes.contains(&"Crash"));
    assert!(outcomes.contains(&"TestFailure"));
    fs::write(f.0.join("src/lib.rs"), "not valid Rust").unwrap();
    assert!(!f.cli(&["coverage", "collect", f.path()]).0);
}

#[test]
fn killed_replay_resumes_missing_steps_and_preserves_completed_execution_bytes() {
    let f = fixture();
    let state = f.0.join(".dctx0ctl");
    fs::create_dir_all(&state).unwrap();
    let marker = state.join("counter");
    let release = state.join("release");
    let mut source = fs::read_to_string(f.0.join("src/lib.rs")).unwrap();
    source.push_str(&format!(r#"
#[test] fn a_checkpoint(){{
use std::io::Write;
let mut f=std::fs::OpenOptions::new().append(true).create(true).open({:?}).unwrap();
f.write_all(b"once\n").unwrap();
}}
#[test] fn z_wait(){{while std::fs::read_to_string({:?}).unwrap().lines().count()>1 && !std::path::Path::new({:?}).exists(){{std::thread::sleep(std::time::Duration::from_millis(20));}}}}
"#,marker,marker,release));
    for index in 0..12 {
        source.push_str(&format!(
            "#[test] fn middle_{index}(){{assert_eq!(first(),1);}}\n"
        ));
    }
    fs::write(f.0.join("src/lib.rs"), &source).unwrap();
    let base = f.commit("checkpoint base");
    source.push_str("// head snapshot\n");
    fs::write(f.0.join("src/lib.rs"), source).unwrap();
    let head = f.commit("head snapshot");
    let manifest = state.join("manifest.json");
    fs::write(&manifest,serde_json::to_vec(&json!({"schema_version":1,"scope":{"package":null,"filter":null,"all_features":false,"no_default_features":false,"features":[]},"timeout_secs":60,"strategy":"coverage","pairs":[{"base":base,"head":head}]})).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args([
            "experimental",
            "test-analysis",
            "replay",
            f.path(),
            "--manifest",
            manifest.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    let mut checkpoint = None;
    while started.elapsed().as_secs() < 30 && checkpoint.is_none() {
        if let Ok(runs) = fs::read_dir(state.join("analysis/runs")) {
            for run in runs.flatten() {
                if let Ok(executions) = fs::read_dir(run.path()) {
                    for execution in executions.flatten() {
                        let file = execution.path().join("execution.json");
                        if let Ok(bytes) = fs::read(&file) {
                            let raw: Value = serde_json::from_slice(&bytes).unwrap();
                            if raw["provenance"]["test_name"] == "a_checkpoint"
                                && raw["provenance"]["revision"] == head
                            {
                                checkpoint = Some((file, bytes));
                            }
                        }
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let (file, bytes) = checkpoint.expect("checkpoint should complete before interruption");
    fs::write(&release, "").unwrap();
    let store = dctx0_store::Store::open(
        &state.join("state.sqlite3"),
        dctx0_store::AccessMode::ReadOnly,
    )
    .unwrap();
    let id = store
        .analysis_records(f.path())
        .unwrap()
        .into_iter()
        .find_map(|r| match r {
            dctx0_store::AnalysisRecord::Replay(r) => Some(r.id),
            _ => None,
        })
        .unwrap();
    drop(store);
    let (ok, report) = f.cli(&[
        "test-analysis",
        "replay",
        f.path(),
        "--manifest",
        manifest.to_str().unwrap(),
        "--resume",
        &id,
    ]);
    assert!(ok, "{report}");
    assert_eq!(fs::read(file).unwrap(), bytes);
    let (_, show) = f.cli(&["test-analysis", "show", f.path(), &id]);
    assert_eq!(
        show["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "Prediction")
            .count(),
        1
    );

    assert_eq!(fs::read_to_string(marker).unwrap(), "once\nonce\n");
}

#[test]
fn blocked_tools_invalid_inputs_and_unavailable_outputs_do_not_invent_results() {
    let f = fixture();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let missing = Command::new(binary)
        .args(["experimental", "coverage", "catalog", f.path()])
        .env("RUSTUP_TOOLCHAIN", "dctx0-research-missing-toolchain")
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let error = String::from_utf8_lossy(&missing.stderr);
    assert!(error.contains("not installed"), "{error}");
    let owner = error
        .split("COLLECTION_BLOCKED ")
        .nth(1)
        .unwrap()
        .split(':')
        .next()
        .unwrap();
    let (ok, show) = f.cli(&["test-analysis", "show", f.path(), owner]);
    assert!(ok, "{show}");
    assert_eq!(show["events"][1]["kind"], "collection_blocked");
    assert!(
        show["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["kind"] != "Collection")
    );
    assert!(
        !f.cli(&[
            "test-analysis",
            "shadow",
            f.path(),
            "--base",
            "missing",
            "--baseline",
            "missing"
        ])
        .0
    );
    assert!(
        !f.cli(&[
            "test-analysis",
            "export",
            f.path(),
            "missing",
            "--output",
            f.0.join("data.ndjson").to_str().unwrap()
        ])
        .0
    );
    let dataset = f.0.join(".dctx0ctl/invalid.ndjson");
    fs::write(&dataset, "{\"schema_version\":99}\n").unwrap();
    assert!(
        !f.cli(&["test-analysis", "recompute", dataset.to_str().unwrap()])
            .0
    );
    assert!(
        !f.cli(&[
            "test-analysis",
            "recompute",
            f.0.join("missing.ndjson").to_str().unwrap()
        ])
        .0
    );
    fs::write(&dataset, "invalid json").unwrap();
    assert!(
        !f.cli(&["test-analysis", "recompute", dataset.to_str().unwrap()])
            .0
    );
    let manifest = f.0.join(".dctx0ctl/invalid.json");
    fs::write(&manifest, "{}").unwrap();
    assert!(
        !f.cli(&[
            "test-analysis",
            "replay",
            f.path(),
            "--manifest",
            manifest.to_str().unwrap()
        ])
        .0
    );
}

#[test]
fn replay_preserves_blocked_pair_and_continues_later_independent_pairs() {
    let f = fixture();
    let base = f.git(&["rev-parse", "HEAD"]);
    let original = fs::read(f.0.join("src/lib.rs")).unwrap();
    fs::write(f.0.join("src/lib.rs"), "invalid Rust input\n").unwrap();
    let broken = f.commit("build failure");
    fs::write(f.0.join("src/lib.rs"), original).unwrap();
    let repaired = f.commit("restored build");
    fs::write(f.0.join("tests/new.rs"), "#[test] fn added(){}\n").unwrap();
    let head = f.commit("later independent comparison");
    let path = f.0.join(".dctx0ctl/manifest.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path,serde_json::to_vec(&json!({"schema_version":1,"scope":{"package":null,"filter":null,"all_features":false,"no_default_features":false,"features":[]},"timeout_secs":60,"strategy":"coverage","pairs":[{"base":base,"head":broken},{"base":repaired,"head":head}]})).unwrap()).unwrap();
    let (ok, report) = f.cli(&[
        "test-analysis",
        "replay",
        f.path(),
        "--manifest",
        path.to_str().unwrap(),
    ]);
    assert!(!ok, "{report}");
    let events = report["events"].as_array().unwrap();
    assert!(events.iter().any(|e| e["kind"] == "pair_0_blocked"));
    assert!(events.iter().any(|e| e["kind"] == "pair_1_complete"));
}

#[test]
fn explicit_database_inside_repository_is_derived_state_in_collect_and_shadow() {
    let f = fixture();
    let database = f.0.join("research.sqlite3");
    let cli = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["--db", database.to_str().unwrap(), "experimental"])
            .args(args)
            .output()
            .unwrap();
        let report: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| json!({"stderr":String::from_utf8_lossy(&output.stderr)}));
        (output.status.success(), report)
    };
    let (ok, report) = cli(&["coverage", "collect", f.path()]);
    assert!(ok, "{report}");
    assert_eq!(report["completeness"]["reference_evaluable"], true);
    let baseline = report["collection_id"].as_str().unwrap();
    let source = fs::read_to_string(f.0.join("src/lib.rs")).unwrap();
    fs::write(
        f.0.join("src/lib.rs"),
        source.replace("first()->u32 {1}", "first()->u32 {9}"),
    )
    .unwrap();
    let (ok, evaluation) = cli(&[
        "test-analysis",
        "shadow",
        f.path(),
        "--base",
        "HEAD",
        "--baseline",
        baseline,
    ]);
    assert!(ok, "{evaluation}");
    assert_eq!(evaluation["failure_recall"], 1.0);
    let (_, show) = cli(&["test-analysis", "show", f.path(), baseline]);
    for snapshot in show["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "Snapshot")
    {
        assert!(
            snapshot["data"]["files"]
                .as_object()
                .unwrap()
                .keys()
                .all(|path| !path.starts_with("research.sqlite3"))
        );
    }
}

#[test]
fn replay_resumes_after_transaction_failure_without_reexecuting_delivered_reference() {
    for stage in ["evaluation", "completion"] {
        let f = fixture();
        let base = f.git(&["rev-parse", "HEAD"]);
        let mut source = fs::read_to_string(f.0.join("src/lib.rs")).unwrap();
        source.push_str("// comparison head\n");
        fs::write(f.0.join("src/lib.rs"), source).unwrap();
        let head = f.commit("comparison head");
        assert!(f.cli(&["coverage", "catalog", f.path()]).0);
        let state = f.0.join(".dctx0ctl");
        let database = state.join("state.sqlite3");
        let sql = |statement: &str| {
            let output=Command::new("python3").args(["-c","import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.executescript(sys.argv[2]); c.close()",database.to_str().unwrap(),statement]).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        sql(if stage == "evaluation" {
            "CREATE TRIGGER injected_failure BEFORE INSERT ON experimental_analysis_evaluations_v1 BEGIN SELECT RAISE(ABORT,'injected evaluation publication failure'); END;"
        } else {
            "CREATE TRIGGER injected_failure BEFORE INSERT ON experimental_analysis_events_v1 WHEN json_extract(NEW.data,'$.kind')='pair_0_complete' BEGIN SELECT RAISE(ABORT,'injected progress publication failure'); END;"
        });
        let manifest = state.join("manifest.json");
        fs::write(&manifest,serde_json::to_vec(&json!({"schema_version":1,"scope":{"package":null,"filter":null,"all_features":false,"no_default_features":false,"features":[]},"timeout_secs":60,"strategy":"coverage","pairs":[{"base":base,"head":head}]})).unwrap()).unwrap();
        assert!(
            !f.cli(&[
                "test-analysis",
                "replay",
                f.path(),
                "--manifest",
                manifest.to_str().unwrap()
            ])
            .0
        );
        let records = dctx0_store::Store::open(&database, dctx0_store::AccessMode::ReadOnly)
            .unwrap()
            .analysis_records(f.path())
            .unwrap();
        let id = records
            .iter()
            .find_map(|r| match r {
                dctx0_store::AnalysisRecord::Replay(r) => Some(r.id.clone()),
                _ => None,
            })
            .unwrap();
        let before: Vec<_> = records
            .iter()
            .filter(|r| matches!(r, dctx0_store::AnalysisRecord::Observation(_)))
            .map(|r| serde_json::to_value(r).unwrap())
            .collect();
        assert!(!before.is_empty());
        sql("DROP TRIGGER injected_failure;");
        let (ok, report) = f.cli(&[
            "test-analysis",
            "replay",
            f.path(),
            "--manifest",
            manifest.to_str().unwrap(),
            "--resume",
            &id,
        ]);
        assert!(ok, "{stage}: {report}");
        let (_, show) = f.cli(&["test-analysis", "show", f.path(), &id]);
        let after: Vec<_> = show["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "Observation")
            .cloned()
            .collect();
        assert_eq!(before, after);
        assert_eq!(
            show["records"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "Evaluation")
                .count(),
            1
        );
    }
}

#[test]
fn export_keeps_only_the_selected_replay_and_its_explicit_dependencies() {
    let f = fixture();
    let base = f.git(&["rev-parse", "HEAD"]);
    let mut source = fs::read_to_string(f.0.join("src/lib.rs")).unwrap();
    source.push_str("// selected head\n");
    fs::write(f.0.join("src/lib.rs"), source).unwrap();
    let head = f.commit("selected head");
    let state = f.0.join(".dctx0ctl");
    fs::create_dir_all(&state).unwrap();
    let manifest = state.join("manifest.json");
    fs::write(&manifest,serde_json::to_vec(&json!({"schema_version":1,"scope":{"package":null,"filter":null,"all_features":false,"no_default_features":false,"features":[]},"timeout_secs":60,"strategy":"coverage","pairs":[{"base":base,"head":head}]})).unwrap()).unwrap();
    let (ok, first) = f.cli(&[
        "test-analysis",
        "replay",
        f.path(),
        "--manifest",
        manifest.to_str().unwrap(),
    ]);
    assert!(ok, "{first}");
    let (ok, second) = f.cli(&[
        "test-analysis",
        "replay",
        f.path(),
        "--manifest",
        manifest.to_str().unwrap(),
    ]);
    assert!(ok, "{second}");
    assert_ne!(first["id"], second["id"]);
    let id = first["id"].as_str().unwrap();
    let output = state.join("selected.ndjson");
    assert!(
        f.cli(&[
            "test-analysis",
            "export",
            f.path(),
            id,
            "--output",
            output.to_str().unwrap()
        ])
        .0
    );
    let rows: Vec<Value> = fs::read_to_string(&output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.iter().filter(|r| r["kind"] == "Replay").count(), 1);
    assert_eq!(rows.iter().filter(|r| r["kind"] == "Prediction").count(), 1);
    assert_eq!(rows.iter().filter(|r| r["kind"] == "Collection").count(), 2);
    assert!(
        rows.iter()
            .any(|r| r["kind"] == "ResearchEvent" && r["data"]["kind"] == "prediction_frozen")
    );
    let (ok, computed) = f.cli(&["test-analysis", "recompute", output.to_str().unwrap()]);
    assert!(ok, "{computed}");
    assert_eq!(computed.as_array().unwrap().len(), 1);
}

#[test]
fn a_large_complete_inventory_is_not_limited_by_process_argument_size() {
    let f = fixture();
    let mut source = fs::read_to_string(f.0.join("src/lib.rs")).unwrap();
    for index in 0..100 {
        source.push_str(&format!(
            "#[test] #[ignore] fn ignored_{index}_{}(){{panic!(\"must remain ignored\");}}\n",
            "x".repeat(1500)
        ));
    }
    fs::write(f.0.join("src/lib.rs"), source).unwrap();
    f.commit("large inventory");
    let (ok, report) = f.cli(&["coverage", "collect", f.path()]);
    assert!(ok, "{report}");
    assert_eq!(report["completeness"]["reference_evaluable"], true);
    let (_, show) = f.cli(&[
        "coverage",
        "collection",
        f.path(),
        report["collection_id"].as_str().unwrap(),
    ]);
    assert_eq!(
        show["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "Observation" && r["data"]["outcome"] == "Ignored")
            .count(),
        101
    );
}
