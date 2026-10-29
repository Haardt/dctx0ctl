use dctx0_store::{AccessMode, AnalysisRecord, Store};
use dctx0_test_analysis::{Artifact, ResearchEvent};
#[test]
fn immutable_records_and_event_cache_commit_together_and_reads_do_not_initialize() {
    let mut store = Store::open(std::path::Path::new(":memory:"), AccessMode::ReadWrite).unwrap();
    assert!(store.analysis_records("project").unwrap().is_empty());
    assert!(
        store
            .analysis_events("project", "owner")
            .unwrap()
            .is_empty()
    );
    let record = AnalysisRecord::Artifact(Artifact {
        sha256: "hash".into(),
        bytes: 7,
        media_type: "text/plain".into(),
    });
    let event = |sequence| ResearchEvent {
        sequence,
        at: "time".into(),
        kind: "published".into(),
        subject: "owner".into(),
        diagnostic: None,
    };
    store
        .publish_analysis("project", std::slice::from_ref(&record), "owner", &event(1))
        .unwrap();
    store
        .publish_analysis("project", std::slice::from_ref(&record), "owner", &event(2))
        .unwrap();
    assert_eq!(store.analysis_records("project").unwrap().len(), 1);
    let mut conflict = record.clone();
    if let AnalysisRecord::Artifact(a) = &mut conflict {
        a.bytes = 8;
    }
    assert!(
        store
            .publish_analysis("project", &[conflict], "owner", &event(3))
            .is_err()
    );
    assert_eq!(store.analysis_events("project", "owner").unwrap().len(), 2);
    assert!(
        store
            .publish_analysis("project", &[], "owner", &event(4))
            .is_err()
    );
    store
        .publish_analysis("project", &[], "owner", &event(3))
        .unwrap();
    assert!(
        store
            .publish_analysis("project", &[], "owner", &event(u64::MAX))
            .is_err()
    );
    assert!(store.analysis_records("other-project").unwrap().is_empty());
}
