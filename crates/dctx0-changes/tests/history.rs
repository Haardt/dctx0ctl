use dctx0_changes::{Fingerprints, HistoryEntry, LineagePath, RenameLineage, RenameMapping};
use std::collections::BTreeMap;

fn declarations(ids: &[&str]) -> BTreeMap<String, Fingerprints> {
    ids.iter()
        .map(|id| {
            (
                id.to_string(),
                Fingerprints {
                    contract: "contract".into(),
                    behavior: "behavior".into(),
                    facts: "facts".into(),
                    location: "location".into(),
                },
            )
        })
        .collect()
}
#[test]
fn rename_lineage_composes_but_deletion_and_name_reuse_end_continuity() {
    let original = declarations(&["W.A", "W.Unchanged"]);
    let rename = |old: &str, new: &str| RenameMapping {
        old: old.into(),
        new: new.into(),
    };
    let mut lineage = RenameLineage::new(&original);
    lineage.advance(
        &[rename("W.A", "W.B")],
        &declarations(&["W.B", "W.Unchanged"]),
    );
    lineage.advance(
        &[rename("W.B", "W.C")],
        &declarations(&["W.C", "W.Unchanged"]),
    );
    assert_eq!(lineage.finish(false), [rename("W.A", "W.C")]);
    let mut inverse = RenameLineage::new(&original);
    inverse.advance(
        &[rename("W.A", "W.C")],
        &declarations(&["W.C", "W.Unchanged"]),
    );
    assert_eq!(inverse.finish(true), [rename("W.C", "W.A")]);
    let mut deleted = RenameLineage::new(&original);
    deleted.advance(&[rename("W.A", "W.B")], &declarations(&["W.B"]));
    deleted.advance(&[], &declarations(&[]));
    deleted.advance(&[rename("W.B", "W.Reused")], &declarations(&["W.Reused"]));
    assert!(deleted.finish(false).is_empty());
}
#[test]
fn history_paths_use_latest_occurrences_and_ignore_failed_runs() {
    let history = [Some("C"), Some("B"), None, Some("A"), Some("B"), Some("A")]
        .into_iter()
        .enumerate()
        .map(|(index, id)| HistoryEntry {
            run_id: (6 - index) as i64,
            snapshot_id: id.map(str::to_owned),
            status: if id.is_some() { "pass" } else { "fail" }.into(),
            source_hash: "source".into(),
            timestamp: "timestamp".into(),
            diagnostics: vec![],
        })
        .collect::<Vec<_>>();
    let forward = LineagePath::between(&history, "A", "C").unwrap();
    assert_eq!(forward.snapshots, ["A", "B", "C"]);
    assert!(!forward.reverse);
    let reverse = LineagePath::between(&history, "C", "A").unwrap();
    assert_eq!(reverse.snapshots, ["A", "B", "C"]);
    assert!(reverse.reverse);
    assert_eq!(
        LineagePath::between(&history, "missing", "C")
            .err()
            .as_deref(),
        Some("missing")
    );
    assert_eq!(
        LineagePath::between(&history, "A", "missing")
            .err()
            .as_deref(),
        Some("missing")
    );
}
