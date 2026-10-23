use dctx0_changes::{HistoryReader, HistoryWriter};
use dctx0_changes::{RenameMapping, snapshot};
use dctx0_core::{Diagnostic, Source};
use dctx0_store::AccessMode;
use dctx0_store::{RenameEvent, Store};
use std::{fs, path::PathBuf};
struct Temp(PathBuf);
impl Temp {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("dctx0-store-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn snap(text: &str) -> dctx0_changes::Snapshot {
    let sources = vec![Source {
        file: "workspace.dctx0".into(),
        text: text.into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    snapshot(&sources, ir, graph)
}
#[test]
fn snapshots_deduplicate_failures_preserve_head_and_readers_cannot_write() {
    let temp = Temp::new("history");
    let path = temp.0.join("state.sqlite3");
    let first = snap("workspace W { type T = Int }");
    let mut store = Store::open(&path, AccessMode::ReadWrite).unwrap();
    assert!(store.latest("project").unwrap().is_none());
    assert!(store.load("project", "latest").is_err());
    store.publish("project", &first, None).unwrap();
    store.publish("project", &first, None).unwrap();
    store.failure("project", "bad", &[]).unwrap();
    assert_eq!(store.latest("project").unwrap().unwrap().id, first.id);
    assert_eq!(store.history("project").unwrap().len(), 3);
    assert_eq!(store.load("project", &first.id[..12]).unwrap(), first);
    for id in ["bad", "zxxxxxxxx", "0123456789"] {
        assert!(store.load("project", id).is_err());
    }
    assert!(!store.rename_committed("unknown").unwrap());
    drop(store);
    let mut reader = Store::open(&path, AccessMode::ReadOnly).unwrap();
    assert_eq!(reader.latest("project").unwrap().unwrap(), first);
    assert!(reader.publish("project", &first, None).is_err());
    assert!(
        reader
            .failure("project", "bad", &Vec::<Diagnostic>::new())
            .is_err()
    );
    let connection = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM snapshots", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn rename_events_are_transactional_and_newer_schemas_are_not_reset() {
    let temp = Temp::new("rename");
    let path = temp.0.join("state.sqlite3");
    let mut store = Store::open(&path, AccessMode::ReadWrite).unwrap();
    let a = snap("workspace W { type A = Int }");
    let b = snap("workspace W { type B = Int }");
    store.publish("p", &a, None).unwrap();
    let event = RenameEvent {
        id: "rename1".into(),
        before: a.id.clone(),
        after: b.id.clone(),
        mappings: vec![RenameMapping {
            old: "W.A".into(),
            new: "W.B".into(),
        }],
    };
    store.publish("p", &b, Some(&event)).unwrap();
    assert!(store.rename_committed("rename1").unwrap());
    assert_eq!(
        store.rename_mappings("p", &a.id, &b.id).unwrap(),
        event.mappings
    );
    let mut invalid = event.clone();
    invalid.id = "bad".into();
    invalid.before = "missing".into();
    assert!(store.publish("p", &a, Some(&invalid)).is_err());
    assert_eq!(store.latest("p").unwrap().unwrap().id, b.id);
    drop(store);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    assert!(Store::open(&path, AccessMode::ReadWrite).is_err());
    assert!(Store::open(&path, AccessMode::ReadOnly).is_err());
    let empty = temp.0.join("empty.sqlite3");
    drop(rusqlite::Connection::open(&empty).unwrap());
    assert!(Store::open(&empty, AccessMode::ReadOnly).is_err());
}
#[test]
fn parallel_publishers_commit_consistent_history() {
    let temp = Temp::new("parallel");
    let path = temp.0.join("state.sqlite3");
    drop(Store::open(&path, AccessMode::ReadWrite).unwrap());
    let snapshot = snap("workspace W {}");
    let handles = (0..4)
        .map(|_| {
            let path = path.clone();
            let snapshot = snapshot.clone();
            std::thread::spawn(move || {
                Store::open(&path, AccessMode::ReadWrite)
                    .unwrap()
                    .publish("p", &snapshot, None)
                    .unwrap();
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap();
    }
    let store = Store::open(&path, AccessMode::ReadOnly).unwrap();
    assert_eq!(store.history("p").unwrap().len(), 4);
    assert_eq!(store.latest("p").unwrap().unwrap(), snapshot);
}

#[test]
fn deeply_nested_snapshots_roundtrip_and_corrupt_history_is_reported() {
    let temp = Temp::new("deep");
    let path = temp.0.join("state.sqlite3");
    let ty = format!("{}Int{}", "List<".repeat(70), ">".repeat(70));
    let snapshot = snap(&format!("workspace W {{ type Deep = {ty} }}"));
    let mut store = Store::open(&path, AccessMode::ReadWrite).unwrap();
    store.publish("p", &snapshot, None).unwrap();
    assert_eq!(store.load("p", &snapshot.id).unwrap(), snapshot);
    assert_eq!(store.latest("p").unwrap().unwrap(), snapshot);
    store.publish("p", &snapshot, None).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("UPDATE snapshots SET data='{} trailing'", [])
        .unwrap();
    assert!(store.latest("p").is_err());
    assert!(store.load("p", &snapshot.id).is_err());
    assert!(store.publish("p", &snapshot, None).is_err());
    connection
        .execute("UPDATE checks SET diagnostics='broken'", [])
        .unwrap();
    assert!(store.history("p").is_err());
}

#[test]
fn sqlite_write_failures_roll_back_without_publishing_partial_history() {
    let temp = Temp::new("write-errors");
    let path = temp.0.join("state.sqlite3");
    let mut store = Store::open(&path, AccessMode::ReadWrite).unwrap();
    let snapshot = snap("workspace W { type T = Int }");
    store.publish("p", &snapshot, None).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_check BEFORE INSERT ON checks BEGIN SELECT RAISE(ABORT,'simulated disk write rejection'); END;").unwrap();
    assert!(store.failure("p", "bad", &[]).is_err());
    assert!(store.publish("p", &snapshot, None).is_err());
    assert_eq!(store.history("p").unwrap().len(), 1);
    assert_eq!(store.latest("p").unwrap().unwrap().id, snapshot.id);
    assert!(store.rename_mappings("p", "missing", &snapshot.id).is_err());
    assert!(store.rename_mappings("p", &snapshot.id, "missing").is_err());
}

#[test]
fn missing_schema_tables_abort_publication_without_resetting_history() {
    let temp = Temp::new("corrupt-schema");
    let path = temp.0.join("state.sqlite3");
    let mut store = Store::open(&path, AccessMode::ReadWrite).unwrap();
    let snapshot = snap("workspace W {}");
    store.publish("p", &snapshot, None).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("DROP TABLE query_snapshots")
        .unwrap();
    assert!(store.publish("p", &snapshot, None).is_err());
    assert_eq!(store.history("p").unwrap().len(), 1);
    assert_eq!(store.latest("p").unwrap().unwrap(), snapshot);
}
