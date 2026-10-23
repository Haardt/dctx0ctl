#![cfg(all(feature = "rust", target_os = "linux"))]
//! Orchestration conformance against a real tiny Rust application, never a full-product claim.
use std::{fs, path::Path, process::Command};
fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn lock(directory: &Path) {
    let output = Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn run(root: &Path) -> (bool, serde_json::Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(["verify", "self"])
        .arg(root)
        .args(["--subject", "W.Api.answer", "--json"])
        .output()
        .unwrap();
    assert!(
        !output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        output.status.success(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}
#[test]
fn self_command_builds_captured_sources_keeps_development_history_untouched_and_detects_real_mutation()
 {
    let root = std::env::temp_dir().join(format!("dctx0-self-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    for relative in [
        "targets", "skills", "editors", "fixtures", "scripts", ".github", "ui/site",
    ] {
        fs::create_dir_all(root.join(relative)).unwrap();
    }
    write(&root, "install.sh", "# fixture installer");
    write(&root, "install.ps1", "# fixture installer");
    write(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers=[\"crates/dctx0-cli\"]\nresolver=\"3\"\n",
    );
    write(
        &root,
        "crates/dctx0-cli/Cargo.toml",
        "[package]\nname=\"dctx0-cli\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[[bin]]\nname=\"dctx0\"\npath=\"src/main.rs\"\n",
    );
    write(
        &root,
        "crates/dctx0-cli/src/main.rs",
        "fn main() { println!(\"actual fixture CLI\"); }",
    );
    write(
        &root,
        "crates/dctx0-cli/src/lib.rs",
        "pub fn answer() -> i64 { 7 }",
    );
    write(
        &root,
        "dctx0/main.dctx0",
        "workspace W {\n constraint \"Calls the actual captured Rust function\"\n surface Api { operation answer() -> Int {\n pure\n ensures result == 7\n } } }",
    );
    let adapter = root.join(".dctx0/testing/rust");
    write(
        &root,
        ".dctx0/testing/rust/Cargo.toml",
        "[package]\nname=\"dctx0-project-adapter\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n[lib]\npath=\"adapter.rs\"\n[[bin]]\nname=\"dctx0-worker\"\npath=\"worker.rs\"\n[dependencies]\nserde_json={version=\"=1.0.151\",features=[\"arbitrary_precision\"]}\nfixture={package=\"dctx0-cli\",path=\"../../../crates/dctx0-cli\"}\n",
    );
    write(
        &root,
        ".dctx0/testing/rust/adapter.rs",
        r#"
pub mod contract; use contract::*; use serde_json::json;
pub fn describe()->Result<Value,String>{Ok(json!({"protocol_version":1,"subjects":[{"subject":"W.Api.answer","role":"operation","artifact":"crates/dctx0-cli/src/lib.rs","entry_point":"answer"}],"artifacts":["crates/dctx0-cli/src/lib.rs"],"capabilities":["sessions","invocation","events"],"representations":{"Int":"i64"}}))}
pub fn begin(_: &str,_:Context)->Result<Box<dyn Session>,String>{Ok(Box::new(Actual))}
struct Actual; impl Session for Actual {
fn invoke(&mut self,_:Value)->Result<Value,String>{Ok(json!({"kind":"returned","value":{"kind":"integer","value":fixture::answer().to_string()}}))}
fn close(&mut self)->Result<(),String>{Ok(())}}
"#,
    );
    write(
        &root,
        ".dctx0/testing/rust/examples.json",
        "{\"examples\":{}}",
    );
    write(
        &root,
        ".dctx0/testing/verification.json",
        r#"{"schema_version":1,"target":"rust","runtime":"cargo","adapter":".dctx0/testing/rust/adapter.rs","fixtures":".dctx0/testing/rust/examples.json","scope":{"subjects":["W.Api.answer"],"tasks":[]},"budgets":{"seed":42,"generated_cases":2,"collection_size":2,"recursion_depth":8,"value_bytes":65536,"shrink_steps":0,"evaluation_steps":100000}}"#,
    );
    lock(&root);
    lock(&adapter);
    let (pass, report) = run(&root);
    assert!(pass, "{report}");
    assert_eq!(report["partial"], true);
    assert_eq!(report["machine_complete"], false);
    assert_eq!(report["selected_machine_complete"], true);
    assert_eq!(report["source_current"], true);
    assert!(!root.join("dctx0/.dctx0ctl").exists());
    assert_eq!(report["native"]["adopted_tasks"], serde_json::json!([]));
    let capture = Path::new(report["captured_root"].as_str().unwrap());
    let helper = capture.join(".dctx0/testing/rust/executables/dctx0");
    assert!(Command::new(helper).output().unwrap().status.success());
    assert!(
        capture
            .join(".dctx0/testing/rust/cli-build-receipt.json")
            .exists()
    );
    assert!(Path::new(report["report_path"].as_str().unwrap()).is_file());
    assert!(!capture.join("self-verification.json").exists());
    assert_eq!(
        report["native"]["evidence"]["run"]["result"]["status"],
        "UNKNOWN"
    );
    let inspections = root.join("source-inspections.json");
    let provenance = &report["native"]["evidence"]["run"]["provenance"];
    let project = dctx0_engine::Project::new(&capture.join("dctx0"), None).unwrap();
    let store = project.store(dctx0_store::AccessMode::ReadOnly).unwrap();
    let plan_artifact = store
        .verification_artifact(
            report["native"]["evidence"]["id"].as_str().unwrap(),
            provenance["plan_hash"].as_str().unwrap(),
        )
        .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&plan_artifact.contents).unwrap();
    let reports = plan["manual_checks"].as_array().unwrap().iter()
        .filter(|manual| manual["kind"] != "intent")
        .map(|manual| {
            let mut requirements = std::collections::BTreeSet::from([manual["subject"].as_str().unwrap().to_owned()]);
            requirements.extend(manual["nodes"].as_array().unwrap().iter().map(|node| node["id"].as_str().unwrap().to_owned()));
            serde_json::json!({"inspection":{
                "schema_version":1, "actor":"fixture-reviewer", "subject":manual["subject"],
                "snapshot":provenance["snapshot"], "plan_hash":provenance["plan_hash"],
                "requirements":requirements, "implementation_artifacts":provenance["implementation"],
                "report":"Inspected the fixture adapter invoke method and the captured production answer function. It calls fixture::answer directly; the return is encoded as the actual integer, rather than a stored expected answer.",
                "limitations":["Tiny fixture scope; this is not a full product review."],
                "result":{"status":"PASS"}
            }})
        }).collect::<Vec<_>>();
    assert_eq!(reports.len(), 1);
    fs::write(&inspections, serde_json::to_vec(&reports).unwrap()).unwrap();
    let published = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(["verify", "inspect"])
        .arg(capture.join("dctx0"))
        .arg(report["native"]["evidence"]["id"].as_str().unwrap())
        .arg("--project")
        .arg(capture)
        .arg("--file")
        .arg(inspections)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        published.status.success(),
        "{}",
        String::from_utf8_lossy(&published.stderr)
    );
    let effective: serde_json::Value = serde_json::from_slice(&published.stdout).unwrap();
    assert_eq!(effective["summary"]["result"]["status"], "PASS");
    write(
        &root,
        "crates/dctx0-cli/src/lib.rs",
        "pub fn answer() -> i64 { 9 }",
    );
    let (pass, broken) = run(&root);
    assert!(!pass);
    assert!(
        broken["native"]["evidence"]["run"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|case| case["result"]["status"] == "FAIL")
    );
    assert!(!root.join("dctx0/.dctx0ctl").exists());
    write(
        &root,
        "crates/dctx0-cli/src/lib.rs",
        "pub fn answer() -> i64 { 7 }",
    );
    let capture_directory = root.join("target/self-verification");
    let previous = fs::read_dir(&capture_directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<std::collections::BTreeSet<_>>();
    let drift_output = root.join("drift-report.json");
    let mut drifting = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(["verify", "self"])
        .arg(&root)
        .args(["--subject", "W.Api.answer", "--json"])
        .stdout(fs::File::create(&drift_output).unwrap())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    loop {
        // Sorted capture copies production crates before the specification.
        // Change the original only once its immutable source copy exists.
        if fs::read_dir(&capture_directory).unwrap().any(|entry| {
            let path = entry.unwrap().path();
            !previous.contains(&path) && path.join("dctx0/main.dctx0").is_file()
        }) {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
        assert!(drifting.try_wait().unwrap().is_none());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    write(
        &root,
        "crates/dctx0-cli/src/lib.rs",
        "pub fn answer() -> i64 { 9 }",
    );
    let finished = drifting.wait_with_output().unwrap();
    assert!(!finished.status.success());
    let drift: serde_json::Value =
        serde_json::from_slice(&fs::read(drift_output).unwrap()).unwrap();
    assert_eq!(drift["source_current"], false);
    assert_eq!(drift["selected_machine_complete"], false);
    assert!(
        drift["native"]["evidence"]["run"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case["result"]["status"] == "PASS")
    );
    assert!(!root.join("dctx0/.dctx0ctl").exists());
    write(&root, "dctx0/main.dctx0", "workspace W {");
    let rejected = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(["verify", "self"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("Self specification rejected"));
    assert!(!root.join("dctx0/.dctx0ctl").exists());
    fs::remove_dir_all(root).unwrap();
}
