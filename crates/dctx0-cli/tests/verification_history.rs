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
        b"{\"required_cases\":[\"C1\"]}".to_vec(),
    );
    let issue = serde_json::json!({"code":"IMPORTED_UNEXECUTED","message":"This report has no native execution authority","span":null});
    let unknown = serde_json::json!({"status":"UNKNOWN","issue":issue});
    let run:VerificationRun=serde_json::from_value(serde_json::json!({"schema_version":1,"origin":"imported","started_at_unix_ns":"1234567890000000000","elapsed_us":0,"provenance":{"snapshot":snapshot.id,"check_occurrence":occurrence,"plan_hash":plan.reference.hash,"implementation":{},"dependencies":{},"adapter_hash":"","configuration_hash":"","harness_hash":"","interpreter_version":"1","generator_version":"import","protocol_version":1,"target":"typescript","runtime_version":"not-executed","build_configuration":{},"task_pin":null,"incomplete":[issue]},"budgets":VerificationBudgets::default(),"selected_subjects":["W.Number"],"required_subjects":["W.Number"],"not_selected":[],"required_cases":["C1"],"cases":[{"id":"C1","subject":"W.Number","kind":"contract","bindings":{},"setup":[],"arguments":[],"observations":[],"events":null,"states":[],"result":unknown,"elapsed_us":0}],"open_obligations":[issue],"bounded_obligations":[],"artifacts":[plan.reference],"result":unknown})).unwrap();
    let saved = store
        .record_verification_run(&p.key(), &run, &[plan])
        .unwrap();
    drop(store);
    fs::write(root.join("w.dctx0"), "invalid now").unwrap();
    for json in [false, true] {
        let mut args = vec!["verify", "show", root.to_str().unwrap(), saved.id.as_str()];
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
        }
    }
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["verify", "show", root.to_str().unwrap(), "missing"])
            .status()
            .unwrap()
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}
