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
pub fn begin(_: &str,_:Context)->Result<Box<dyn Session>,String>{Ok(Box::new(Actual))}
struct Actual;
impl Session for Actual {
fn invoke(&mut self,invocation:Value)->Result<Value,String>{let input=invocation["arguments"][0]["value"].as_str().ok_or("Missing input")?.parse::<num_bigint::BigInt>().map_err(|e|e.to_string())?;let result=fixture::next(input);Ok(json!({"kind":"returned","value":{"kind":"integer","value":result.to_string()}}))}
fn close(&mut self)->Result<(),String>{Ok(())}}
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
