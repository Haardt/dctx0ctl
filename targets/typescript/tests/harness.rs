use dctx0_target_sdk::*;
use dctx0_target_typescript::BunHarness;
use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};
#[test]
fn generated_worker_is_deterministic_scaffolds_are_unavailable_and_imports_cannot_escape() {
    let request = HarnessRequest {
        adapter_import: "./adapter.ts".into(),
        subjects: BTreeSet::from([
            "operation".into(),
            "value".into(),
            "Entity".into(),
            "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner".into(),
        ]),
    };
    let bundle = BunHarness.generate(&request).unwrap();
    assert_eq!(bundle, BunHarness.generate(&request).unwrap());
    assert_eq!(bundle.recipe.program, "bun");
    assert_eq!(bundle.sdk_version, 1);
    assert_eq!(bundle.artifacts.len(), 2);
    assert!(!bundle.artifacts[1].contents.contains("__ADAPTER_IMPORT__"));
    let scaffold = BunHarness.scaffold();
    assert!(scaffold.contents.contains("not implemented"));
    for path in [
        "adapter.ts",
        "/absolute.ts",
        "./../escape.ts",
        "./folder/../../escape.ts",
        "./a\\b.ts",
        "./a\nb.ts",
        "./folder//adapter.ts",
        "./adapter.js",
    ] {
        let mut invalid = request.clone();
        invalid.adapter_import = path.into();
        assert!(
            BunHarness
                .generate(&invalid)
                .unwrap_err()
                .to_string()
                .contains("Harness input")
        );
    }
    let mut slot = request.clone();
    slot.adapter_import = "./__SUBJECTS__.ts".into();
    slot.subjects.insert("__ADAPTER_IMPORT__".into());
    let generated = BunHarness.generate(&slot).unwrap();
    assert!(
        generated.artifacts[1]
            .contents
            .contains("\"./__SUBJECTS__.ts\"")
    );
    assert!(
        generated.artifacts[1]
            .contents
            .contains("\"__ADAPTER_IMPORT__\"")
    );
    let mut escaped = request.clone();
    escaped.adapter_import = "./quote\"adapter.ts".into();
    assert!(
        BunHarness.generate(&escaped).unwrap().artifacts[1]
            .contents
            .contains("quote\\\"adapter.ts")
    );
}
#[test]
fn real_bun_worker_conforms_to_sessions_observation_mock_transport_and_failures() {
    let root = std::env::temp_dir().join(format!("dctx0-bun-worker-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let bundle = BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./adapter.ts".into(),
            subjects: BTreeSet::from([
                "operation".into(),
                "value".into(),
                "Entity".into(),
                "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner".into(),
            ]),
        })
        .unwrap();
    for artifact in &bundle.artifacts {
        fs::write(root.join(&artifact.path), &artifact.contents).unwrap();
    }
    fs::write(root.join("counter.ts"), include_str!("fixtures/counter.ts")).unwrap();
    fs::write(root.join("adapter.ts"), include_str!("fixtures/adapter.ts")).unwrap();
    fs::write(root.join("scaffold.ts"), BunHarness.scaffold().contents).unwrap();
    let scaffold = BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./scaffold.ts".into(),
            subjects: BTreeSet::new(),
        })
        .unwrap();
    fs::write(
        root.join("scaffold-worker.ts"),
        &scaffold.artifacts[1].contents,
    )
    .unwrap();
    fs::write(
        root.join("transport.test.ts"),
        include_str!("fixtures/transport.test.ts"),
    )
    .unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema/src/domain");
    fs::create_dir_all(root.join("src/domain")).unwrap();
    for file in ["candidate-order-winner.ts", "film-id.ts", "voting-rules.ts"] {
        fs::copy(source.join(file), root.join("src/domain").join(file)).unwrap();
    }
    let output = Command::new("bun")
        .args(["test", "transport.test.ts"])
        .current_dir(&root)
        .output()
        .expect("Bun is required for executable target conformance");
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-harness-conformance");
    fs::create_dir_all(&evidence).unwrap();
    if output.status.success() {
        let mut raw: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("native-performance.json")).unwrap())
                .unwrap();
        for message in raw["protocolSamples"].as_array().unwrap() {
            decode_message(&serde_json::to_vec(message).unwrap(), MAX_MESSAGE_BYTES).unwrap();
        }
        raw["conformance_status"] = serde_json::json!("pass");
        raw["native_stdout"] = serde_json::json!(String::from_utf8_lossy(&output.stdout));
        raw["native_stderr"] = serde_json::json!(String::from_utf8_lossy(&output.stderr));
        fs::write(
            evidence.join(format!("{}.json", std::process::id())),
            serde_json::to_vec_pretty(&raw).unwrap(),
        )
        .unwrap();
    }
    fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
