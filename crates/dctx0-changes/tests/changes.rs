use dctx0_changes::ChangeFacet;
use dctx0_changes::{ChangeKind, RenameMapping, diff, semantic_value, snapshot};
use dctx0_core::Source;
fn snapshot_of(file: &str, text: &str) -> dctx0_changes::Snapshot {
    let sources = vec![Source {
        file: file.into(),
        text: text.into(),
    }];
    let result = dctx0_language::compile(&sources);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let ir = result.ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    snapshot(&sources, ir, graph)
}
#[test]
fn formatting_comments_and_files_do_not_change_semantics() {
    let a = snapshot_of("a.dctx0", "workspace W { type T { id: Int } }");
    let b = snapshot_of("b.dctx0", "// moved\nworkspace W {\n type T { id: Int }\n}");
    assert_ne!(a.source_hash, b.source_hash);
    assert_eq!(a.semantic_hash, b.semantic_hash);
    assert!(
        diff(Some(&a), &b, &[])
            .changes
            .iter()
            .all(|c| c.facets == [ChangeFacet::Location])
    );
    assert!(diff(Some(&a), &a, &[]).changes.is_empty());
    assert!(diff(None, &a, &[]).before.is_none());
    assert_eq!(
        semantic_value(
            serde_json::json!({"span":{"file":"x"},"field":[{"name":"span","value":"hello"}]})
        ),
        serde_json::json!({"field":[{"name":"span","value":"hello"}]})
    );
}
#[test]
fn contracts_behavior_facts_removal_and_explicit_rename_remain_distinct() {
    let a = snapshot_of(
        "a.dctx0",
        "workspace W { type T { id: Int } surface Api { operation get() -> T { ensures success } } constraint \"One\" }",
    );
    let b = snapshot_of(
        "a.dctx0",
        "workspace W { type T { id: Int; extra: String } surface Api { operation get() -> T { ensures not success } } constraint \"Two\" }",
    );
    let changes = diff(Some(&a), &b, &[]);
    assert!(
        changes
            .changes
            .iter()
            .any(|c| c.new.as_deref() == Some("W.T")
                && c.facets.contains(&ChangeFacet::Contract)
                && c.impacts.iter().any(|i| i.subject == "W.Api.get"))
    );
    assert!(changes.changes.iter().any(
        |c| c.new.as_deref() == Some("W.Api.get") && c.facets.contains(&ChangeFacet::Behavior)
    ));
    assert!(
        changes
            .changes
            .iter()
            .any(|c| c.facets.contains(&ChangeFacet::Facts))
    );
    let renamed = snapshot_of(
        "a.dctx0",
        "workspace W { type Renamed { id: Int } surface Api { operation get() -> Renamed { ensures success } } constraint \"One\" }",
    );
    let plain = diff(Some(&a), &renamed, &[]);
    assert!(
        plain
            .changes
            .iter()
            .any(|c| c.kind == ChangeKind::Removed && c.old.as_deref() == Some("W.T"))
    );
    assert!(
        plain
            .changes
            .iter()
            .any(|c| c.kind == ChangeKind::Added && c.new.as_deref() == Some("W.Renamed"))
    );
    let explicit = diff(
        Some(&a),
        &renamed,
        &[RenameMapping {
            old: "W.T".into(),
            new: "W.Renamed".into(),
        }],
    );
    assert!(
        explicit
            .changes
            .iter()
            .any(|c| c.kind == ChangeKind::Renamed)
    );
    let removed = snapshot_of("a.dctx0", "workspace W {}");
    assert!(diff(Some(&a), &removed, &[]).changes.iter().any(
        |c| c.kind == ChangeKind::Removed && c.impacts.iter().any(|i| i.subject == "W.Api.get")
    ));
}

#[test]
fn exposure_changes_contract_without_changing_operation_behavior() {
    let request = snapshot_of(
        "a.dctx0",
        "workspace W { surface Api exposes requestReply { operation get() -> Int { pure } } }",
    );
    let stream = snapshot_of(
        "a.dctx0",
        "workspace W { surface Api exposes stream { operation get() -> Int { pure } } }",
    );
    let changes = diff(Some(&request), &stream, &[]);
    assert_eq!(changes.changes.len(), 1);
    assert_eq!(changes.changes[0].new.as_deref(), Some("W.Api"));
    assert_eq!(changes.changes[0].facets, [ChangeFacet::Contract]);
    assert_eq!(
        serde_json::to_value(&changes.changes[0].facets).unwrap(),
        serde_json::json!(["contract"])
    );
    assert_eq!(
        request.fingerprints["W.Api.get"],
        stream.fingerprints["W.Api.get"]
    );
}

#[test]
fn complete_example_snapshots_preserve_the_pre_refactoring_wire_format() {
    // Captured from the original release implementation before refactoring;
    // intentionally includes every IR field, graph span, and fingerprint.
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/semantic/snapshot-hashes.json"
    ))
    .unwrap();
    for example in ["cinema", "runatron-next"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(example)
            .join("dctx0");
        let sources = std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "dctx0"))
            .map(|path| Source {
                file: path.file_name().unwrap().to_string_lossy().into_owned(),
                text: std::fs::read_to_string(path).unwrap(),
            })
            .collect::<Vec<_>>();
        assert_eq!(sources.len(), if example == "cinema" { 2 } else { 18 });
        let result = dctx0_language::compile(&sources);
        assert!(
            result.diagnostics.is_empty(),
            "{example}: {:?}",
            result.diagnostics
        );
        let ir = result.ir.unwrap();
        let graph = dctx0_graph::build(&ir);
        let actual = snapshot(&sources, ir, graph);
        assert_eq!(
            actual.id,
            expected[example]["snapshot_id"].as_str().unwrap(),
            "{example} identity"
        );
        assert_eq!(
            dctx0_changes::hash(&actual),
            expected[example]["serialized_snapshot_sha256"]
                .as_str()
                .unwrap(),
            "{example} complete serialized snapshot"
        );
    }
}
