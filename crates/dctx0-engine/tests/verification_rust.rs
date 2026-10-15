//! Actual Cargo build and Rust execution, preserving failed evidence.
use dctx0_core::VerificationResult;
use dctx0_engine::*;
use dctx0_target_rust::RustHarness;
use std::{fs, process::Command};
#[tokio::test]
async fn rust_builds_actual_code_detects_mutations_and_preserves_failed_build_cases() {
    let root = std::env::temp_dir().join(format!("dctx0-engine-rust-{}", std::process::id()));
    let app = root.join("app");
    let spec = root.join("spec");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(&spec).unwrap();
    fs::write(spec.join("w.dctx0"),"workspace W { surface Api { operation next(input:Int) -> Int {\n pure\n ensures result == input + 1\n } } }").unwrap();
    fs::write(app.join("Cargo.toml"),"[package]\nname=\"fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\nrust-version=\"1.85\"\n[workspace]\n[dependencies]\nnum-bigint=\"=0.4.6\"\n").unwrap();
    let actual = "use num_bigint::BigInt; pub fn next(value:BigInt)->BigInt {value+1}";
    fs::write(app.join("src/lib.rs"), actual).unwrap();
    let project = Project::new(&spec, None).unwrap();
    assert!(project.check().unwrap().ok);
    project
        .initialize_verification(
            &app,
            &RustHarness,
            ConfiguredVerificationScope {
                subjects: ["W.Api.next".into()].into(),
                tasks: Default::default(),
            },
        )
        .unwrap();
    let adapter_dir = app.join(".dctx0/testing/rust");
    fs::write(
        adapter_dir.join("Cargo.toml"),
        format!(
            "{}\nfixture={{path=\"../../..\"}}\nnum-bigint=\"=0.4.6\"\n",
            include_str!("../../../targets/rust/harness/Cargo.toml")
        ),
    )
    .unwrap();
    fs::write(adapter_dir.join("adapter.rs"),r#"
pub mod contract;
use contract::*;use serde_json::json;
pub fn describe()->Result<Value,String>{Ok(json!({"protocol_version":1,"subjects":[{"subject":"W.Api.next","role":"operation","artifact":"src/lib.rs","entry_point":"fixture::next"}],"artifacts":["src/lib.rs"],"capabilities":["sessions","invocation","events"],"representations":{"Int":"num_bigint::BigInt"}}))}
pub fn begin(case_id: &str,_:Context)->Result<Box<dyn Session>,String>{Ok(Box::new(Actual{case_id:case_id.into()}))}
struct Actual{case_id:String}
impl Session for Actual {
fn invoke(&mut self,invocation:Value)->Result<Value,String>{if self.case_id=="adapter-error"{return Err("Fixture adapter error".into())}if self.case_id=="adapter-panic"{panic!("Fixture adapter panic")}let input=invocation["arguments"][0]["value"].as_str().ok_or("Missing input")?.parse::<num_bigint::BigInt>().map_err(|e|e.to_string())?;let result=fixture::next(input);Ok(json!({"kind":"returned","value":{"kind":"integer","value":result.to_string()}}))}
fn close(&mut self)->Result<(),String>{
if std::path::Path::new("cleanup.probe").exists(){use std::io::Write;let mut file=std::fs::OpenOptions::new().create(true).append(true).open("closed.sessions").map_err(|e|e.to_string())?;writeln!(file,"{}",self.case_id).map_err(|e|e.to_string())?;}
if self.case_id=="cleanup-error"{return Err("Fixture cleanup error".into())}
if self.case_id=="cleanup-panic"{panic!("Fixture cleanup panic")}
Ok(())}}
"#).unwrap();
    let output = Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(&adapter_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = project
        .run_verification_with_native_limits(
            Some(&app),
            Default::default(),
            &RustHarness,
            &NativeCancellation::default(),
            true,
            NativeLimits {
                supervision: NativeSupervision::Child,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        report.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        report.evidence.run.provenance.incomplete
    );
    assert!(!report.evidence.run.cases.is_empty());
    assert!(report.evidence.run.provenance.incomplete.is_empty());
    assert_rust_transport_closes_sessions(&root, &app).await;
    fs::write(app.join("src/lib.rs"), actual.replace("value+1", "value+2")).unwrap();
    let broken = project
        .run_verification(
            Some(&app),
            Default::default(),
            &RustHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    assert!(matches!(
        broken.evidence.run.result,
        VerificationResult::Fail { .. }
    ));
    assert!(broken.adopted_tasks.is_empty());
    let case = broken
        .evidence
        .run
        .cases
        .iter()
        .find(|case| matches!(case.result, VerificationResult::Fail { .. }))
        .unwrap();
    let saved = project.saved_verification_run(&broken.evidence.id).unwrap();
    let replay = project
        .replay_verification(
            Some(&app),
            &broken.evidence.id,
            &case.id,
            &RustHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        replay.case_result,
        Some(VerificationResult::Fail { .. })
    ));
    fs::write(app.join("src/lib.rs"), actual).unwrap();
    let repaired = project
        .replay_verification(
            Some(&app),
            &broken.evidence.id,
            &case.id,
            &RustHarness,
            &NativeCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(repaired.case_result, Some(VerificationResult::Pass));
    assert!(!repaired.changes.is_empty());
    assert!(repaired.evidence.adopted_tasks.is_empty());
    assert_eq!(
        project.saved_verification_run(&broken.evidence.id).unwrap(),
        saved
    );
    fs::write(app.join("src/lib.rs"), "pub fn broken(").unwrap();
    let unbuilt = project
        .run_verification(
            Some(&app),
            Default::default(),
            &RustHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    assert!(matches!(
        unbuilt.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    assert!(unbuilt.evidence.run.cases.is_empty());
    assert!(!unbuilt.evidence.run.required_cases.is_empty());
    fs::remove_dir_all(root).unwrap();
}

// A close marker proves that the adapter ran its cleanup hook before exit;
// process exit alone would also release OS handles and would hide this defect.
async fn assert_rust_transport_closes_sessions(root: &std::path::Path, app: &std::path::Path) {
    use dctx0_target_sdk::ExecutionRecipe;
    use serde_json::json;
    let workers = fs::read_dir(app.join(".dctx0ctl/native/rust"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(workers.len(), 1, "one built worker before mutation");
    let worker = workers[0].join(format!("worker{}", std::env::consts::EXE_SUFFIX));
    let envelope =
        |id, request| json!({"protocol_version":1,"id":id,"request":request}).to_string() + "\n";
    let begin = |id, case_id| envelope(id, json!({"kind":"begin","case_id":case_id}));
    let shutdown = |id| envelope(id, json!({"kind":"shutdown"}));
    let invoke = |id| {
        envelope(
            id,
            json!({"kind":"invoke","invocation":{"subject":"W.Api.next","arguments":[]}}),
        )
    };
    let scenarios = [
        ("eof", begin(1, "eof"), false, vec!["eof"]),
        (
            "malformed",
            begin(1, "malformed") + "{broken}\n",
            false,
            vec!["malformed"],
        ),
        (
            "duplicate",
            begin(1, "duplicate") + &shutdown(1),
            false,
            vec!["duplicate"],
        ),
        (
            "normal",
            begin(1, "normal") + &envelope(2, json!({"kind":"end"})) + &shutdown(3),
            true,
            vec!["normal"],
        ),
        (
            "shutdown",
            begin(1, "shutdown") + &shutdown(2),
            true,
            vec!["shutdown"],
        ),
        (
            "adapter-error",
            begin(1, "adapter-error") + &invoke(2) + &begin(3, "after-error") + &shutdown(4),
            true,
            vec!["adapter-error", "after-error"],
        ),
        (
            "adapter-panic",
            begin(1, "adapter-panic") + &invoke(2) + &begin(3, "after-panic") + &shutdown(4),
            true,
            vec!["adapter-panic", "after-panic"],
        ),
        (
            "cleanup-error",
            begin(1, "cleanup-error") + &shutdown(2),
            true,
            vec!["cleanup-error"],
        ),
        (
            "cleanup-panic",
            begin(1, "cleanup-panic") + &shutdown(2),
            true,
            vec!["cleanup-panic"],
        ),
    ];
    for (name, input, exits_cleanly, closed) in scenarios {
        let directory = root.join("cleanup").join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("cleanup.probe"), "").unwrap();
        let mut command = NativeCommand::spawn(
            &ExecutionRecipe {
                program: worker.to_string_lossy().into_owned(),
                arguments: vec![],
            },
            &directory,
            NativeLimits {
                supervision: NativeSupervision::Child,
                run: std::time::Duration::from_secs(10),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        command.write_input(input.as_bytes()).await.unwrap();
        let audit = command
            .wait_status(&NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(
            audit.exit_code == Some(0),
            exits_cleanly,
            "{name}: {audit:?}"
        );
        assert_eq!(
            fs::read_to_string(directory.join("closed.sessions"))
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            closed,
            "{name}"
        );
        let replies = audit
            .stdout
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        if name.starts_with("adapter-") {
            assert!(
                replies.iter().any(|reply| reply["message"]["id"] == 3
                    && reply["message"]["reply"]["kind"] == "begun"),
                "{name}: {}",
                audit.stdout
            );
        }
        if name.starts_with("cleanup-") {
            assert!(
                replies
                    .iter()
                    .any(|reply| reply["message"]["kind"] == "technical-error"),
                "{name}: {}",
                audit.stdout
            );
            assert!(
                !replies
                    .iter()
                    .any(|reply| reply["message"]["reply"]["kind"] == "bye"),
                "{name}: {}",
                audit.stdout
            );
        }
    }
}
