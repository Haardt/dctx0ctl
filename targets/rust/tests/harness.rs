use dctx0_target_rust::RustHarness;
use dctx0_target_sdk::*;
#[test]
fn rust_harness_is_deterministic_keeps_the_manifest_editable_and_has_no_worker_verdict() {
    let request = HarnessRequest {
        adapter_import: "./adapter.rs".into(),
        subjects: ["W.Api.next".into()].into(),
    };
    let first = RustHarness.generate(&request).unwrap();
    assert_eq!(first, RustHarness.generate(&request).unwrap());
    assert_eq!(RustHarness.runtime(), "cargo");
    assert_eq!(first.target, "rust");
    assert!(first.build.unwrap().arguments.contains(&"--release".into()));
    assert!(
        RustHarness
            .setup_artifacts()
            .iter()
            .any(|a| a.path == "Cargo.toml")
    );
    assert!(RustHarness.scaffold().contents.contains("not implemented"));
    assert!(
        RustHarness
            .generate(&HarnessRequest {
                adapter_import: "../production.rs".into(),
                subjects: Default::default()
            })
            .is_err()
    );
}
