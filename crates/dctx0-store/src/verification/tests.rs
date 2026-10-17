use super::*;
use dctx0_core::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dctx0-storage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn db(&self) -> PathBuf {
        self.0.join("state.sqlite3")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn setup(store: &mut Store, project: &str) -> (VerificationRun, Vec<VerificationArtifact>) {
    let sources = vec![Source {
        file: "w.dctx0".into(),
        text: "workspace W { type A = Int }".into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    let snapshot = dctx0_changes::snapshot(&sources, ir, graph);
    store.publish(project, &snapshot, None).unwrap();
    let plan = VerificationArtifact::new("application/json", b"{}".to_vec());
    let large = VerificationArtifact::new("text/plain", b"repeatable evidence".repeat(1000));
    let artifacts = vec![plan, large];
    let run = serde_json::from_value(serde_json::json!({
        "schema_version":1,"origin":"imported","started_at_unix_ns":"1","elapsed_us":0,
        "provenance":{"snapshot":snapshot.id,"check_occurrence":store.task_check(project).unwrap().0,
        "plan_hash":artifacts[0].reference.hash,"implementation":{},"dependencies":{},"adapter_hash":"a",
        "configuration_hash":"c","harness_hash":"h","interpreter_version":"1","generator_version":"1",
        "protocol_version":1,"target":"typescript","runtime_version":"not-executed","build_configuration":{},"task_pin":null,"incomplete":[]},
        "budgets":VerificationBudgets::default(),"selected_subjects":["W.A"],"required_subjects":["W.A"],"not_selected":[],
        "required_cases":["C"],"cases":[{"id":"C","subject":"W.A","kind":"contract","bindings":{"payload":{"kind":"string","value":"x"}},"setup":[],"arguments":[],"observations":[],"events":null,"states":[],"result":{"status":"PASS"},"elapsed_us":0}],
        "open_obligations":[],"bounded_obligations":[],"artifacts":artifacts.iter().map(|a|a.reference.clone()).collect::<Vec<_>>(),"result":{"status":"PASS"}
    })).unwrap();
    (run, artifacts)
}
fn legacy_store(path: &Path) -> Store {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(crate::SCHEMA).unwrap();
    connection.execute_batch(crate::tasks::MIGRATION).unwrap();
    connection
        .execute_batch(crate::tasks::PROFILE_MIGRATION)
        .unwrap();
    connection.execute_batch(MIGRATION).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    Store {
        connection,
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    }
}
fn legacy_insert(
    store: &Store,
    project: &str,
    run: &VerificationRun,
    artifacts: &[VerificationArtifact],
) -> String {
    let data = serde_json::to_string(run).unwrap();
    let id = format!("{:x}", Sha256::digest(&data));
    store.connection.execute("INSERT INTO verification_runs(id,project,snapshot_id,check_id,origin,result,data) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,project,run.provenance.snapshot,run.provenance.check_occurrence,serde_json::to_string(&run.origin).unwrap(),serde_json::to_string(&run.result).unwrap(),data]).unwrap();
    for a in artifacts {
        store.connection.execute("INSERT OR IGNORE INTO verification_artifacts(hash,media_type,bytes) VALUES(?1,?2,?3)",params![a.reference.hash,a.reference.media_type,a.contents]).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO verification_run_artifacts VALUES(?1,?2)",
                params![id, a.reference.hash],
            )
            .unwrap();
    }
    id
}
#[test]
fn storage_retains_three_across_scopes_with_shared_corpora_and_historical_adoptions() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (mut run, artifacts) = setup(&mut store, "p");
    let (other, other_artifacts) = setup(&mut store, "q");
    let other_id = store
        .record_verification_run("q", &other, &other_artifacts)
        .unwrap()
        .id;
    let first = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    assert_eq!(first.run, run);
    assert_eq!(
        first.id,
        format!("{:x}", Sha256::digest(serde_json::to_vec(&run).unwrap()))
    );
    store
        .connection
        .execute(
            "INSERT INTO verification_adoptions(run_id,data) VALUES(?1,'{}')",
            [&first.id],
        )
        .unwrap();
    let extra = VerificationArtifact::new("text/plain", b"obsolete unique diagnostic".to_vec());
    run.artifacts.push(extra.reference.clone());
    let mut with_extra = artifacts.clone();
    with_extra.push(extra.clone());
    let mut ids = vec![first.id.clone()];
    for n in 2..=6 {
        run.started_at_unix_ns = n.to_string();
        if n == 3 {
            run.selected_subjects.insert("W".into());
            run.required_subjects.insert("W".into());
        }
        if n != 2 {
            run.artifacts.retain(|a| a.hash != extra.reference.hash);
        }
        ids.push(
            store
                .record_verification_run("p", &run, if n == 2 { &with_extra } else { &artifacts })
                .unwrap()
                .id,
        );
    }
    assert_eq!(store.verification_runs("p").unwrap().len(), 3);
    assert!(matches!(
        store.verification_run("p", &first.id),
        Err(Error::DetailsPruned(_))
    ));
    let summaries = store.verification_run_summaries("p", 0, 200).unwrap();
    assert_eq!(summaries.len(), 6);
    assert_eq!(summaries.iter().filter(|s| s.details_available).count(), 3);
    assert_eq!(summaries.last().unwrap().result, VerificationResult::Pass);
    assert!(summaries.last().unwrap().pruned_at.is_some());
    assert!(store.verification_run("q", &other_id).is_ok());
    assert!(
        store
            .verification_artifact(&first.id, &artifacts[0].reference.hash)
            .is_err()
    );
    assert!(
        store
            .verification_artifact(ids.last().unwrap(), &artifacts[1].reference.hash)
            .is_ok()
    );
    let count: i64 = store
        .connection
        .query_row("SELECT count(*) FROM verification_corpora", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    let count: i64 = store
        .connection
        .query_row(
            "SELECT count(*) FROM verification_artifacts WHERE hash=?1",
            [&extra.reference.hash],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    let count: i64 = store
        .connection
        .query_row(
            "SELECT count(*) FROM verification_adoptions WHERE run_id=?1",
            [&first.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    assert_eq!(&saved.id, ids.last().unwrap());
    assert_eq!(
        store.verification_run_summaries("p", 0, 200).unwrap().len(),
        6
    );
    assert!(store.verification_run_summaries("p", 0, 0).is_err());
    assert!(store.verification_run_summaries("p", u64::MAX, 1).is_err());
    assert!(store.verification_run_summary("p", "missing").is_err());
    assert_eq!(
        store.verification_run_summaries("p", 2, 2).unwrap().len(),
        2
    );
    let before = store.site_revision("p").unwrap();
    store.verification_gc("p").unwrap();
    assert_ne!(before, store.site_revision("p").unwrap());
}
#[test]
fn storage_retention_failure_rolls_back_new_run_and_artifacts() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (mut run, artifacts) = setup(&mut store, "p");
    for n in 1..=3 {
        run.started_at_unix_ns = n.to_string();
        store
            .record_verification_run("p", &run, &artifacts)
            .unwrap();
    }
    store.connection.execute_batch("CREATE TRIGGER fail_gc BEFORE UPDATE OF pruned_at ON verification_runs BEGIN SELECT RAISE(ABORT,'interrupted retention'); END;").unwrap();
    let before = store.verification_run_summaries("p", 0, 200).unwrap();
    run.started_at_unix_ns = "4".into();
    assert!(
        store
            .record_verification_run("p", &run, &artifacts)
            .is_err()
    );
    assert_eq!(
        before,
        store.verification_run_summaries("p", 0, 200).unwrap()
    );
    store
        .connection
        .execute_batch("DROP TRIGGER fail_gc")
        .unwrap();
    store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    assert_eq!(store.verification_runs("p").unwrap().len(), 3);
}
#[test]
fn storage_legacy_readers_are_unchanged_and_gc_preserves_run_ids() {
    let t = Temp::new();
    let mut legacy = legacy_store(&t.db());
    let (mut run, artifacts) = setup(&mut legacy, "p");
    let mut ids = vec![];
    for n in 1..=5 {
        run.started_at_unix_ns = n.to_string();
        ids.push(legacy_insert(&legacy, "p", &run, &artifacts));
    }
    drop(legacy);
    let before = fs::read(t.db()).unwrap();
    let reader = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    assert_eq!(
        reader
            .verification_run("p", ids.last().unwrap())
            .unwrap()
            .run,
        run
    );
    assert_eq!(
        reader
            .verification_run_summaries("p", 0, 200)
            .unwrap()
            .len(),
        5
    );
    assert_eq!(reader.verification_runs("p").unwrap().len(), 3);
    let site = reader
        .site_records("p", &crate::site::SiteSelection::default())
        .unwrap();
    assert_eq!(site.runs.len(), 3);
    assert!(
        site.runs
            .iter()
            .all(|entry| entry.header.cases.is_empty() && !entry.corpus.is_empty())
    );

    assert_eq!(
        reader
            .verification_artifact(&ids[0], &artifacts[1].reference.hash)
            .unwrap(),
        artifacts[1]
    );
    assert_eq!(reader.verification_gc_preview("p").unwrap().pruned_runs, 2);
    assert_eq!(before, fs::read(t.db()).unwrap());
    let connection = Connection::open(t.db()).unwrap();
    let original: String = connection
        .query_row(
            "SELECT data FROM verification_runs WHERE id=?1",
            [&ids[0]],
            |r| r.get(0),
        )
        .unwrap();
    connection
        .execute(
            "UPDATE verification_runs SET data=json_set(data,'$.elapsed_us',1) WHERE id=?1",
            [&ids[0]],
        )
        .unwrap();
    assert!(
        reader
            .verification_run_summary("p", &ids[0])
            .unwrap_err()
            .to_string()
            .contains("digest")
    );
    connection
        .execute(
            "UPDATE verification_runs SET data=?2 WHERE id=?1",
            params![ids[0], original],
        )
        .unwrap();
    drop(connection);
    drop(reader);
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let report = store.verification_gc("p").unwrap();
    assert_eq!(report.pruned_runs, 2);
    assert!(report.logical_bytes_after < report.logical_bytes_before);
    assert_eq!(
        store
            .verification_run("p", ids.last().unwrap())
            .unwrap()
            .run,
        run
    );
    assert!(
        !store
            .verification_run_summary("p", &ids[0])
            .unwrap()
            .details_available
    );
    assert_eq!(store.verification_gc("p").unwrap().pruned_runs, 0);
    assert!(store.verification_artifact("missing", "missing").is_err());
    drop(store);
    let mut reader = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    assert!(matches!(reader.verification_gc("p"), Err(Error::ReadOnly)));
}
#[test]
fn storage_compact_copy_discards_old_bodies_but_preserves_other_projects_and_checks() {
    let t = Temp::new();
    let mut source = legacy_store(&t.db());
    let (mut run, artifacts) = setup(&mut source, "p");
    let mut ids = vec![];
    for n in 1..=5 {
        run.started_at_unix_ns = n.to_string();
        ids.push(legacy_insert(&source, "p", &run, &artifacts));
    }
    let (mut other, other_artifacts) = setup(&mut source, "q");
    for n in 1..=5 {
        other.started_at_unix_ns = n.to_string();
        legacy_insert(&source, "q", &other, &other_artifacts);
    }
    let copy = t.0.join("copy.sqlite3");
    let report = source.verification_compact_copy("p", &copy).unwrap();
    assert_eq!(report.pruned_runs, 2);
    assert_eq!(report.full_runs_after, 3);
    assert!(source.verification_compact_copy("p", &copy).is_err());
    let target = Store::open(&copy, AccessMode::ReadOnly).unwrap();
    assert_eq!(
        target
            .verification_run_summaries("q", 0, 200)
            .unwrap()
            .iter()
            .filter(|s| s.details_available)
            .count(),
        5
    );
    assert_eq!(
        serde_json::to_value(target.history("p").unwrap()).unwrap(),
        serde_json::to_value(source.history("p").unwrap()).unwrap()
    );
    assert_eq!(
        target
            .verification_run("p", ids.last().unwrap())
            .unwrap()
            .run,
        run
    );
    assert!(target.verification_run("p", &ids[0]).is_err());
    drop(target);
    let version = source.verification_storage_version().unwrap();
    source.install_verification_copy(&copy, version).unwrap();
    let target = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    assert_eq!(
        target
            .verification_run_summaries("p", 0, 200)
            .unwrap()
            .len(),
        5
    );
    assert!(!copy.exists());
}
#[test]
fn storage_corrupt_payloads_and_stale_compaction_never_publish_success() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = setup(&mut store, "p");
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    let version = store.verification_storage_version().unwrap();
    let copy = t.0.join("copy.sqlite3");
    store.verification_compact_copy("p", &copy).unwrap();
    let mut second = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    second.failure("p", "changed", &[]).unwrap();
    drop(second);
    assert!(
        store
            .install_verification_copy(&copy, version)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    let store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    store
        .connection
        .execute("UPDATE verification_corpora SET bytes=x'00'", [])
        .unwrap();
    assert!(store.verification_run("p", &saved.id).is_err());
    // Listing summaries must not read or decompress the corrupt full body.
    assert_eq!(
        store.verification_run_summaries("p", 0, 10).unwrap().len(),
        1
    );
    assert!(
        store
            .verification_compact_copy("p", &t.0.join("bad.sqlite3"))
            .is_err()
    );
    store
        .connection
        .execute("UPDATE verification_corpora SET raw_length=-1", [])
        .unwrap();
    assert!(store.verification_run("p", &saved.id).is_err());
    let mut altered = run.clone();
    altered.cases[0]
        .bindings
        .insert("payload".into(), VerificationValue::String("y".into()));
    let (_, length, packed) = codec::compress_json(&codec::Corpus {
        cases: &altered.cases,
        required_cases: &altered.required_cases,
    })
    .unwrap();
    store
        .connection
        .execute(
            "UPDATE verification_corpora SET bytes=?1,raw_length=?2",
            params![packed, length as i64],
        )
        .unwrap();
    assert!(
        store
            .verification_run("p", &saved.id)
            .unwrap_err()
            .to_string()
            .contains("corrupt")
    );
    let (encoding, packed) = codec::encode(&vec![b'x'; 8192]).unwrap();
    assert_eq!(encoding, "gzip");
    assert_eq!(
        codec::decode_bytes(encoding, packed.clone(), Some(8192))
            .unwrap()
            .len(),
        8192
    );
    for length in [None, Some(-1), Some(8191), Some(8193)] {
        assert!(codec::decode_bytes(encoding, packed.clone(), length).is_err());
    }
    assert!(codec::decode_bytes("unknown", vec![], None).is_err());
    assert!(codec::decode_bytes("identity", vec![1], Some(2)).is_err());
    assert!(codec::decode_bytes("gzip", vec![0], Some(1)).is_err());
    store
        .connection
        .execute(
            "UPDATE verification_artifacts SET bytes=x'00',encoding='identity' WHERE hash=?1",
            [&artifacts[1].reference.hash],
        )
        .unwrap();
    assert!(
        store
            .verification_artifact(&saved.id, &artifacts[1].reference.hash)
            .is_err()
    );
}
#[test]
fn storage_schema_one_queries_do_not_create_verification_tables() {
    let t = Temp::new();
    let connection = Connection::open(t.db()).unwrap();
    connection.execute_batch(crate::SCHEMA).unwrap();
    drop(connection);
    let store = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    assert!(store.verification_runs("p").unwrap().is_empty());
    assert!(
        store
            .verification_run_summaries("p", 0, 10)
            .unwrap()
            .is_empty()
    );
    assert!(store.verification_run("p", "missing").is_err());
    assert!(store.verification_run_summary("p", "missing").is_err());
    assert!(store.verification_artifact("p", "missing").is_err());
    assert_eq!(store.verification_gc_preview("p").unwrap().pruned_runs, 0);
    assert!(
        store
            .verification_compact_copy("p", &t.0.join("copy.sqlite3"))
            .is_err()
    );
}

#[test]
fn storage_compaction_excludes_live_readers_and_rejects_unknown_tables() {
    let t = Temp::new();
    let mut source = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = setup(&mut source, "p");
    let saved = source
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    let candidate = t.0.join("copy.sqlite3");
    source.verification_compact_copy("p", &candidate).unwrap();
    let reader = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    let version = source.verification_storage_version().unwrap();
    assert!(
        source
            .install_verification_copy(&candidate, version)
            .is_err()
    );
    assert_eq!(reader.verification_run("p", &saved.id).unwrap(), saved);
    assert!(candidate.exists());
    drop(reader);
    let source = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let version = source.verification_storage_version().unwrap();
    source
        .install_verification_copy(&candidate, version)
        .unwrap();
    let source = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    source.connection.execute_batch("CREATE TABLE verification_unrecognized(data TEXT); INSERT INTO verification_unrecognized VALUES('keep');").unwrap();
    assert!(
        source
            .verification_compact_copy("p", &candidate)
            .unwrap_err()
            .to_string()
            .contains("Unrecognized")
    );
    assert_eq!(source.verification_run("p", &saved.id).unwrap(), saved);
}

#[test]
fn storage_corpus_identity_survives_valid_alternate_gzip_encodings() {
    use std::io::Write;
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (mut run, artifacts) = setup(&mut store, "p");
    let first = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    let corpus = serde_json::to_vec(&codec::Corpus {
        cases: &run.cases,
        required_cases: &run.required_cases,
    })
    .unwrap();
    let mut encoder = flate2::GzBuilder::new()
        .mtime(123)
        .write(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&corpus).unwrap();
    let bytes = encoder.finish().unwrap();
    store
        .connection
        .execute("UPDATE verification_corpora SET bytes=?1", [bytes])
        .unwrap();
    assert_eq!(store.verification_run("p", &first.id).unwrap(), first);
    run.started_at_unix_ns = "2".into();
    let second = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    assert_eq!(store.verification_run("p", &second.id).unwrap(), second);
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM verification_corpora", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn storage_maintenance_rejects_non_file_sources_and_incompatible_table_layouts() {
    use std::io::Write;
    let t = Temp::new();
    let memory = Store::open(Path::new(":memory:"), AccessMode::ReadWrite).unwrap();
    let destination = t.0.join("memory.sqlite3");
    assert!(
        memory
            .verification_compact_copy("p", &destination)
            .unwrap_err()
            .to_string()
            .contains("file database")
    );
    assert!(!destination.exists());
    assert!(
        memory
            .install_verification_copy(&destination, 1)
            .unwrap_err()
            .to_string()
            .contains("file database")
    );
    let mut source = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let _ = setup(&mut source, "p");
    source
        .connection
        .execute("ALTER TABLE checks ADD COLUMN future_metadata TEXT", [])
        .unwrap();
    assert!(
        source
            .verification_compact_copy("p", &t.0.join("incompatible.sqlite3"))
            .unwrap_err()
            .to_string()
            .contains("Copying checks")
    );
    assert_eq!(
        source
            .connection
            .query_row("SELECT count(*) FROM checks", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    source.connection.execute("DELETE FROM checks", []).unwrap();
    assert!(
        source
            .site_records("p", &crate::site::SiteSelection::default())
            .unwrap_err()
            .to_string()
            .contains("No valid published check")
    );
    let output = t.0.join("streamed.json");
    let file = fs::File::create(&output).unwrap();
    let mut writer = codec::HashWriter::new(std::io::BufWriter::new(file));
    writer.write_all(b"buffered evidence").unwrap();
    assert!(fs::read(&output).unwrap().is_empty());
    writer.flush().unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"buffered evidence");
    assert_eq!(writer.length, 17);
    assert_eq!(
        writer.hash.finalize().as_slice(),
        Sha256::digest(b"buffered evidence").as_slice()
    );
}
