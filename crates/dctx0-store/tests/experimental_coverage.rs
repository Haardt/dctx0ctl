use dctx0_store::{AccessMode, CoverageExecution, Store};
use serde_json::json;
use std::path::Path;
fn execution() -> CoverageExecution {
    CoverageExecution {
        test_id: "crate::test".into(),
        outcome: "pass".into(),
        duration_ms: 12.0,
        peak_rss_bytes: Some(4096),
        provenance: json!({"revision":"abc", "dirty":true}),
        llvm_export: json!({"type":"llvm.coverage.json.export", "data":[{"files":[{"filename":"/src/lib.rs","segments":[[2,1,1,true,true,false],[5,1,0,true,true,false],[8,1,1,true,true,false],[9,4,0,true,false,false]]}]}]}),
    }
}
#[test]
fn coverage_history_is_append_only_queryable_and_independent_of_checks() {
    let mut store = Store::open(Path::new(":memory:"), AccessMode::ReadWrite).unwrap();
    assert!(
        store
            .coverage_records("p", None, None, 20)
            .unwrap()
            .is_empty()
    );
    let ids = store
        .record_coverage("p", &[execution(), execution()])
        .unwrap();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    let all = store.coverage_records("p", None, None, 1).unwrap();
    assert_eq!(all[0].id, ids[1]);
    assert_eq!(all[0].execution.llvm_export, execution().llvm_export);
    for line in [2, 3, 4, 8, 9] {
        assert_eq!(
            store
                .coverage_records("p", Some("/src/lib.rs"), Some(line), 20)
                .unwrap()
                .len(),
            2
        );
    }
    for line in [1, 5, 6, 7, 10] {
        assert!(
            store
                .coverage_records("p", Some("/src/lib.rs"), Some(line), 20)
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        store
            .coverage_records("other", None, None, 20)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .coverage_records("p", Some("missing"), None, 20)
            .unwrap()
            .is_empty()
    );
    use dctx0_store::HistoryReader;
    assert!(store.history("p").unwrap().is_empty());
    let mut invalid = execution();
    invalid.outcome = "PASS".into();
    assert!(store.record_coverage("p", &[execution(), invalid]).is_err());
    assert_eq!(
        store.coverage_records("p", None, None, 20).unwrap().len(),
        2
    );
    let streamed = [
        Ok(execution()),
        Err(dctx0_store::Error::Missing("interrupted artifact".into())),
    ];
    assert!(store.record_coverage_stream("p", streamed).is_err());
    assert_eq!(
        store.coverage_records("p", None, None, 20).unwrap().len(),
        2
    );
    let mut invalid = execution();
    invalid.duration_ms = -1.0;
    assert!(store.record_coverage("p", &[invalid]).is_err());
}
#[test]
fn read_only_queries_do_not_create_extension_tables_and_writes_are_rejected() {
    let path = std::env::temp_dir().join(format!(
        "dctx0-coverage-readonly-{}.sqlite",
        std::process::id()
    ));
    let store = Store::open(&path, AccessMode::ReadWrite).unwrap();
    drop(store);
    let mut store = Store::open(&path, AccessMode::ReadOnly).unwrap();
    assert!(
        store
            .coverage_records("p", None, None, 20)
            .unwrap()
            .is_empty()
    );
    assert!(store.record_coverage("p", &[execution()]).is_err());
    drop(store);
    let connection = rusqlite::Connection::open(&path).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name LIKE 'experimental_coverage%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    drop(connection);
    std::fs::remove_file(path).unwrap();
}
