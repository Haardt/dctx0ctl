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
pub(crate) fn setup(
    store: &mut Store,
    project: &str,
) -> (VerificationRun, Vec<VerificationArtifact>) {
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
fn storage_retains_latest_across_scopes_with_shared_bodies_and_historical_adoptions() {
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
    assert_eq!(store.verification_runs("p").unwrap().len(), 1);
    assert!(matches!(
        store.verification_run("p", &first.id),
        Err(Error::DetailsPruned(_))
    ));
    let summaries = store.verification_run_summaries("p", 0, 200).unwrap();
    assert_eq!(summaries.len(), 6);
    assert_eq!(summaries.iter().filter(|s| s.details_available).count(), 1);
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
        .query_row("SELECT count(*) FROM verification_case_bodies", [], |r| {
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
    assert_eq!(count, 1);
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
    assert_eq!(store.verification_runs("p").unwrap().len(), 1);
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
        .execute("UPDATE verification_case_bodies SET bytes=x'00'", [])
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
        .execute("UPDATE verification_case_bodies SET raw_length=-1", [])
        .unwrap();
    assert!(store.verification_run("p", &saved.id).is_err());
    let mut altered = run.clone();
    altered.cases[0]
        .bindings
        .insert("payload".into(), VerificationValue::String("y".into()));
    let (_, length, packed) = codec::compress_json(&altered.cases[0]).unwrap();
    store
        .connection
        .execute(
            "UPDATE verification_case_bodies SET bytes=?1,raw_length=?2",
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
    let corpus = serde_json::to_vec(&run.cases[0]).unwrap();
    let mut encoder = flate2::GzBuilder::new()
        .mtime(123)
        .write(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&corpus).unwrap();
    let bytes = encoder.finish().unwrap();
    store
        .connection
        .execute("UPDATE verification_case_bodies SET bytes=?1", [bytes])
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
            .query_row("SELECT count(*) FROM verification_case_bodies", [], |r| r
                .get::<_, i64>(
                0
            ))
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
    source
        .connection
        .execute("UPDATE checks SET future_metadata='keep'", [])
        .unwrap();
    let copy = t.0.join("incompatible.sqlite3");
    source.verification_compact_copy("p", &copy).unwrap();
    let copied = Connection::open(&copy).unwrap();
    assert_eq!(
        copied
            .query_row("SELECT future_metadata FROM checks", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "keep"
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

#[test]
fn query_views_filter_join_aggregate_and_keep_pins_without_opening_case_bodies() {
    use crate::QueryPins;
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (mut run, artifacts) = setup(&mut store, "p");
    run.provenance
        .implementation
        .insert("src/application.ts".into(), "captured-hash".into());
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    let query = |sql: &str, parameters: std::collections::BTreeMap<String, serde_json::Value>| {
        store
            .query_sql(
                "p",
                QueryPins::default(),
                sql,
                &parameters,
                crate::QueryLimits::default(),
            )
            .unwrap()
    };
    let result = query(
        "SELECT c.subject,c.outcome,COUNT(*) AS cases FROM dctx0_cases c JOIN dctx0_runs r USING(run_id) WHERE c.run_id=:run_id GROUP BY c.subject,c.outcome ORDER BY c.subject,c.outcome",
        [("run_id".into(), serde_json::json!(saved.id))].into(),
    );
    assert_eq!(
        result.rows,
        vec![vec![
            serde_json::json!("W.A"),
            serde_json::json!("PASS"),
            serde_json::json!(1)
        ]]
    );
    assert_eq!(
        result.pins.snapshot.as_deref(),
        Some(run.provenance.snapshot.as_str())
    );
    assert_eq!(
        query("SELECT kind FROM dctx0_cases", Default::default()).rows[0][0],
        "contract"
    );
    assert_eq!(
        query(
            "SELECT case_id,recorded FROM dctx0_run_requirements",
            Default::default()
        )
        .rows[0][1],
        1
    );
    assert_eq!(query("SELECT file,start_line,subject FROM dctx0_source_refs WHERE origin='captured-implementation'",Default::default()).rows[0],vec![serde_json::json!("src/application.ts"),serde_json::Value::Null,serde_json::Value::Null]);
    assert!(
        !query(
            "SELECT subject FROM dctx0_facts WHERE text LIKE :text",
            [("text".into(), serde_json::json!("%Int%"))].into()
        )
        .rows
        .is_empty()
    );
    store
        .connection
        .execute("UPDATE verification_case_bodies SET bytes=x'00'", [])
        .unwrap();
    assert_eq!(
        query(
            "SELECT count(*) FROM dctx0_cases WHERE outcome='PASS'",
            Default::default()
        )
        .rows[0][0],
        1
    );
    let page = store
        .verification_diagnostics("p", &saved.id, Default::default(), 10, None, false)
        .unwrap();
    assert_eq!(page.cases.len(), 1);
    assert!(page.detail.is_none());
    assert!(store.verification_case("p", &saved.id, "C").is_err());
    assert!(
        store
            .verification_diagnostics(
                "p",
                &saved.id,
                dctx0_verification::diagnostics::DiagnosticFilter {
                    case: Some("C".into()),
                    ..Default::default()
                },
                10,
                None,
                false
            )
            .is_err()
    );
    assert!(
        store
            .query_sql(
                "other",
                QueryPins::default(),
                "SELECT * FROM dctx0_runs",
                &Default::default(),
                crate::QueryLimits::default()
            )
            .is_err()
    );
}

#[test]
fn query_sql_rejects_unsafe_statements_parameters_and_resource_exhaustion() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let _ = setup(&mut store, "p");
    let query = |sql: &str,
                 params: &std::collections::BTreeMap<String, serde_json::Value>,
                 rows,
                 bytes,
                 time| {
        store.query_sql(
            "p",
            crate::QueryPins::default(),
            sql,
            params,
            crate::QueryLimits {
                max_rows: rows,
                max_bytes: bytes,
                timeout_ms: time,
            },
        )
    };
    for sql in [
        "DELETE FROM dctx0_tasks",
        "SELECT * FROM snapshots",
        "SELECT * FROM query_rows",
        "WITH dctx0_tasks AS (SELECT * FROM query_rows) SELECT * FROM dctx0_tasks",
        "WITH dctx0_tasks AS (SELECT * FROM [QuErY_RoWs]) SELECT * FROM dctx0_tasks",
        "WITH dctx0_tasks AS (SELECT * FROM `query_rows`) SELECT * FROM dctx0_tasks",
        "WITH dctx0_tasks AS (SELECT * FROM 'query_rows') SELECT * FROM dctx0_tasks",
        r#"WITH dctx0_tasks AS (SELECT * FROM "query_rows") SELECT * FROM dctx0_tasks"#,
        "SELECT * FROM sqlite_master",
        "ATTACH ':memory:' AS other",
        "PRAGMA user_version",
        "SELECT load_extension('x')",
        "SELECT readfile('/etc/passwd')",
        "SELECT 1; SELECT 2",
        "CREATE TABLE bad(x)",
        "BEGIN",
        "SELECT 1 AS a UNION SELECT zeroblob(1000000)",
    ] {
        assert!(
            query(sql, &Default::default(), 100, 1048576, 5000).is_err(),
            "{sql}"
        );
    }
    assert!(query("SELECT :missing", &Default::default(), 100, 1048576, 5000).is_err());
    assert!(
        query(
            "SELECT :x",
            &[("unused".into(), serde_json::json!(1))].into(),
            100,
            1048576,
            5000
        )
        .is_err()
    );
    for value in [serde_json::json!([]), serde_json::json!({})] {
        assert!(
            query(
                "SELECT :x",
                &[("x".into(), value)].into(),
                100,
                1048576,
                5000
            )
            .is_err()
        );
    }
    for value in [
        serde_json::Value::Null,
        serde_json::json!(true),
        serde_json::json!(1),
        serde_json::json!(1.5),
        serde_json::json!("text"),
    ] {
        assert!(
            query(
                "SELECT :x",
                &[("x".into(), value)].into(),
                100,
                1048576,
                5000
            )
            .is_ok()
        );
    }
    assert!(
        query(
            r#"/* query_rows */ SELECT 'a''b', task_id AS "a""b" FROM dctx0_tasks -- query_rows"#,
            &Default::default(),
            100,
            1048576,
            5000
        )
        .is_ok()
    );
    assert!(
        query(
            "SELECT 'x', 1 AS `a``b`, 2 AS [plain]",
            &Default::default(),
            100,
            1048576,
            5000
        )
        .is_ok()
    );
    let result=query("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<5) SELECT x FROM n ORDER BY x",&Default::default(),2,1048576,5000).unwrap();
    assert!(!result.complete);
    assert_eq!(result.rows.len(), 2);
    assert!(
        query(
            "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT sum(x) FROM n",
            &Default::default(),
            2,
            1048576,
            1
        )
        .is_err()
    );
    assert!(
        query(
            "SELECT :x",
            &[("x".into(), serde_json::json!("x".repeat(5000)))].into(),
            100,
            1024,
            5000
        )
        .is_err()
    );
    assert!(query("SELECT 1", &Default::default(), 0, 1048576, 5000).is_err());
    assert!(query("SELECT 1", &Default::default(), 1, 0, 5000).is_err());
    assert!(query("SELECT 1", &Default::default(), 1, 1000, 0).is_err());
}

#[test]
fn query_prepare_resumes_legacy_conversion_and_exports_canonical_evidence_atomically() {
    let t = Temp::new();
    let mut old = legacy_store(&t.db());
    let (run, artifacts) = setup(&mut old, "p");
    let id = legacy_insert(&old, "p", &run, &artifacts);
    drop(old);
    let reader = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    assert!(
        reader
            .query_sql(
                "p",
                crate::QueryPins::default(),
                "SELECT * FROM dctx0_runs",
                &Default::default(),
                crate::QueryLimits {
                    max_rows: 10,
                    max_bytes: 100000,
                    timeout_ms: 5000
                }
            )
            .unwrap_err()
            .to_string()
            .contains("QUERY_INDEX_MISSING")
    );
    drop(reader);
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    assert_eq!(store.prepare_verification_queries("p").unwrap(), 1);
    assert_eq!(store.prepare_verification_queries("p").unwrap(), 0);
    store.prepare_query_snapshot("p", "latest").unwrap();
    store.prepare_query_snapshot("p", "latest").unwrap();
    assert_eq!(store.verification_run("p", &id).unwrap().run, run);
    let path = t.0.join("report.json");
    store.export_verification("p", &id, None, &path).unwrap();
    let exported: StoredVerificationRun =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(exported.run, run);
    store
        .export_verification("p", &id, Some("C"), &path)
        .unwrap();
    let case: VerificationCaseRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(case, run.cases[0]);
    let before = fs::read(&path).unwrap();
    store
        .connection
        .execute("UPDATE verification_case_bodies SET bytes=x'00'", [])
        .unwrap();
    assert!(store.export_verification("p", &id, None, &path).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        store.verification_run_summary("p", &id).unwrap().case_count,
        1
    );
    assert!(store.verification_case("q", &id, "C").is_err());
}

#[test]
fn exact_case_reads_ignore_unrelated_corruption_and_conversion_failures_keep_the_old_corpus() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (mut run, artifacts) = setup(&mut store, "p");
    let mut other = run.cases[0].clone();
    other.id = "other".into();
    run.required_cases.insert(other.id.clone());
    run.cases.push(other);
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    store.connection.execute("UPDATE verification_case_bodies SET bytes=x'00' WHERE hash=(SELECT body_hash FROM verification_case_index WHERE run_id=?1 AND case_id='other')",[&saved.id]).unwrap();
    assert_eq!(
        store.verification_case("p", &saved.id, "C").unwrap(),
        run.cases[0]
    );
    let page = store
        .verification_diagnostics(
            "p",
            &saved.id,
            dctx0_verification::diagnostics::DiagnosticFilter {
                case: Some("C".into()),
                ..Default::default()
            },
            1,
            None,
            false,
        )
        .unwrap();
    assert_eq!(page.detail, Some(run.cases[0].clone()));
    assert!(store.verification_run("p", &saved.id).is_err());
    let destination = t.0.join("case.json");
    store
        .export_verification("p", &saved.id, Some("C"), &destination)
        .unwrap();
    run.started_at_unix_ns = "2".into();
    assert!(
        store
            .record_verification_run("p", &run, &artifacts)
            .is_err()
    );
    assert_eq!(
        store.verification_run_summaries("p", 0, 10).unwrap().len(),
        1
    );
    let legacy = Temp::new();
    let mut old = legacy_store(&legacy.db());
    let (run, artifacts) = setup(&mut old, "p");
    let id = legacy_insert(&old, "p", &run, &artifacts);
    drop(old);
    let mut store = Store::open(&legacy.db(), AccessMode::ReadWrite).unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_case BEFORE INSERT ON verification_case_index BEGIN SELECT RAISE(ABORT,'conversion rejected'); END;").unwrap();
    assert!(store.prepare_verification_queries("p").is_err());
    assert!(!cases::prepared(&store.connection, &id).unwrap());
    assert_eq!(store.verification_run("p", &id).unwrap().run, run);
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM verification_case_bodies", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
    store
        .connection
        .execute_batch("DROP TRIGGER reject_case")
        .unwrap();
    assert_eq!(store.prepare_verification_queries("p").unwrap(), 1);
}

#[test]
fn public_task_queries_use_the_pure_model_and_historical_plan_event_and_snapshot_pins() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (run, _) = setup(&mut store, "p");
    let snapshot = store.latest("p").unwrap().unwrap();
    let candidate = dctx0_tasks::plan(&snapshot).unwrap();
    let plan = store
        .publish_tasks(
            "p",
            &candidate,
            run.provenance.check_occurrence,
            None,
            0,
            &Default::default(),
        )
        .unwrap();
    let session = store.task_session("p", None, None).unwrap();
    let pins = crate::QueryPins {
        plan: Some(plan.id),
        as_of: Some(session.event_seq),
        snapshot: None,
    };
    let before = store
        .query_sql(
            "p",
            pins.clone(),
            "SELECT task_id,ready,delivered,work FROM dctx0_tasks ORDER BY task_id",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    for row in &before.rows {
        let id = row[0].as_i64().unwrap();
        let overview = session
            .overviews()
            .into_iter()
            .find(|o| o.task_id == id)
            .unwrap();
        assert_eq!(row[1], i32::from(overview.ready));
        assert_eq!(row[2], i32::from(overview.delivered));
    }
    let sources = vec![Source {
        file: "w.dctx0".into(),
        text: "workspace W { type B = String }".into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let next = dctx0_changes::snapshot(&sources, ir.clone(), dctx0_graph::build(&ir));
    store.publish("p", &next, None).unwrap();
    let default = store
        .query_sql(
            "p",
            Default::default(),
            "SELECT snapshot FROM dctx0_subjects LIMIT 1",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(default.pins.snapshot.as_deref(), Some(snapshot.id.as_str()));
    let explicit = store
        .query_sql(
            "p",
            crate::QueryPins {
                snapshot: Some(next.id.clone()),
                ..pins.clone()
            },
            "SELECT subject FROM dctx0_subjects WHERE subject='W.B'",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(explicit.rows[0][0], "W.B");
    let old = store
        .query_sql(
            "p",
            pins,
            "SELECT task_id,ready,delivered,work FROM dctx0_tasks ORDER BY task_id",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(old.rows, before.rows);
    assert!(
        store
            .query_sql(
                "p",
                crate::QueryPins {
                    as_of: Some(session.event_seq + 100),
                    ..Default::default()
                },
                "SELECT * FROM dctx0_tasks",
                &Default::default(),
                Default::default()
            )
            .is_err()
    );
    let saved_sequence = store.task_sequence("p").unwrap();
    for _ in 0..3 {
        store
            .query_sql(
                "p",
                Default::default(),
                "SELECT COUNT(*) FROM dctx0_task_dependencies",
                &Default::default(),
                Default::default(),
            )
            .unwrap();
    }
    assert_eq!(store.task_sequence("p").unwrap(), saved_sequence);
}

#[test]
fn schema_five_compressed_corpora_stay_readable_and_convert_without_changing_identity() {
    let t = Temp::new();
    let mut old = legacy_store(&t.db());
    let (mut run, artifacts) = setup(&mut old, "p");
    run.open_obligations.push(VerificationIssue {
        code: "OPEN".into(),
        message: "Recorded obligation".into(),
        span: None,
    });
    run.result = VerificationResult::Unknown {
        issue: run.open_obligations[0].clone(),
    };
    run.required_cases.insert("missing".into());
    let id = legacy_insert(&old, "p", &run, &artifacts);
    old.connection.execute_batch(STORAGE_MIGRATION).unwrap();
    let (hash, length, bytes) = codec::compress_json(&codec::Corpus {
        cases: &run.cases,
        required_cases: &run.required_cases,
    })
    .unwrap();
    let mut summary =
        serde_json::to_value(VerificationRunSummary::new(&id, &run).unwrap()).unwrap();
    for key in [
        "schema_version",
        "open_obligation_count",
        "open_obligation_count_available",
        "missing_case_count",
    ] {
        summary.as_object_mut().unwrap().remove(key);
    }
    old.connection
        .execute(
            "INSERT INTO verification_corpora VALUES(?1,?2,?3)",
            params![hash, bytes, length as i64],
        )
        .unwrap();
    old.connection
        .execute(
            "UPDATE verification_runs SET data=?1,corpus_hash=?2,summary=?3 WHERE id=?4",
            params![
                serde_json::to_string(&codec::Header(&run)).unwrap(),
                hash,
                summary.to_string(),
                id
            ],
        )
        .unwrap();
    drop(old);
    let before = fs::read(t.db()).unwrap();
    let reader = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    assert_eq!(reader.verification_run("p", &id).unwrap().run, run);
    let summary = reader.verification_run_summary("p", &id).unwrap();
    assert_eq!(summary.open_obligation_count, 1);
    assert!(summary.open_obligation_count_available);
    assert_eq!(summary.missing_case_count, 1);
    drop(reader);
    assert_eq!(fs::read(t.db()).unwrap(), before);
    let mut writer = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    assert_eq!(writer.prepare_verification_queries("p").unwrap(), 1);
    assert_eq!(writer.verification_run("p", &id).unwrap().run, run);
    assert_eq!(
        writer
            .connection
            .query_row("SELECT count(*) FROM verification_corpora", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn preparing_pre_retention_history_keeps_three_bodies_and_all_historical_summaries() {
    let t = Temp::new();
    let mut old = legacy_store(&t.db());
    let (mut run, artifacts) = setup(&mut old, "p");
    let mut ids = vec![];
    for n in 1..=7 {
        run.started_at_unix_ns = n.to_string();
        ids.push(legacy_insert(&old, "p", &run, &artifacts));
    }
    drop(old);
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    assert_eq!(store.prepare_verification_queries("p").unwrap(), 7);
    assert_eq!(
        store.verification_run_summaries("p", 0, 100).unwrap().len(),
        7
    );
    assert_eq!(store.verification_runs("p").unwrap().len(), 3);
    for id in &ids[..4] {
        assert!(matches!(
            store.verification_case("p", id, "C"),
            Err(Error::DetailsPruned(_))
        ));
    }
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM verification_case_bodies", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM verification_case_index", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(store.prepare_verification_queries("p").unwrap(), 0);
}

#[test]
fn legacy_case_selection_and_exports_work_without_preparing_or_migrating_readers() {
    let t = Temp::new();
    let mut old = legacy_store(&t.db());
    let (run, artifacts) = setup(&mut old, "p");
    let id = legacy_insert(&old, "p", &run, &artifacts);
    drop(old);
    let before = fs::read(t.db()).unwrap();
    let reader = Store::open(&t.db(), AccessMode::ReadOnly).unwrap();
    assert_eq!(
        reader.verification_case("p", &id, "C").unwrap(),
        run.cases[0]
    );
    assert!(matches!(
        reader.verification_case("p", &id, "absent"),
        Err(Error::Missing(_))
    ));
    let destination = t.0.join("legacy.json");
    reader
        .export_verification("p", &id, None, &destination)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<StoredVerificationRun>(&fs::read(&destination).unwrap())
            .unwrap()
            .run,
        run
    );
    reader
        .export_verification("p", &id, Some("C"), &destination)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<VerificationCaseRecord>(&fs::read(&destination).unwrap()).unwrap(),
        run.cases[0]
    );
    drop(reader);
    assert_eq!(fs::read(t.db()).unwrap(), before);
}

#[test]
fn schema_five_compaction_validates_old_corpora_and_preserves_read_only_exports() {
    let t = Temp::new();
    let mut old = legacy_store(&t.db());
    let (run, artifacts) = setup(&mut old, "p");
    let id = legacy_insert(&old, "p", &run, &artifacts);
    old.connection.execute_batch(STORAGE_MIGRATION).unwrap();
    let (hash, length, bytes) = codec::compress_json(&codec::Corpus {
        cases: &run.cases,
        required_cases: &run.required_cases,
    })
    .unwrap();
    old.connection
        .execute(
            "INSERT INTO verification_corpora VALUES(?1,?2,?3)",
            params![hash, bytes, length as i64],
        )
        .unwrap();
    old.connection
        .execute(
            "UPDATE verification_runs SET data=?1,corpus_hash=?2,summary=?4 WHERE id=?3",
            params![
                serde_json::to_string(&codec::Header(&run)).unwrap(),
                hash,
                id,
                serde_json::to_string(&VerificationRunSummary::new(&id, &run).unwrap()).unwrap()
            ],
        )
        .unwrap();
    let destination = t.0.join("compact.sqlite3");
    assert_eq!(
        old.verification_compact_copy("p", &destination)
            .unwrap()
            .full_runs_after,
        1
    );
    let reader = Store::open(&destination, AccessMode::ReadOnly).unwrap();
    assert_eq!(
        reader.verification_case("p", &id, "C").unwrap(),
        run.cases[0]
    );
    assert_eq!(reader.verification_run("p", &id).unwrap().run, run);
    let exported = t.0.join("report.json");
    reader
        .export_verification("p", &id, None, &exported)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<StoredVerificationRun>(&fs::read(exported).unwrap())
            .unwrap()
            .run,
        run
    );
    assert!(reader.verification_artifact(&id, "missing").is_err());
    // Legacy compressed corpora are validated before migration, even when a
    // syntactically valid stream has a wrong digest or claimed length.
    assert!(codec::validate_corpus(&bytes, length as i64, "wrong").is_err());
    assert!(codec::validate_corpus(&bytes, -1, &hash).is_err());
    old.connection
        .execute(
            "UPDATE verification_corpora SET raw_length=raw_length+1",
            [],
        )
        .unwrap();
    assert!(
        old.verification_compact_copy("p", &t.0.join("bad.sqlite3"))
            .is_err()
    );
    assert_eq!(
        old.connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
}

#[test]
fn old_summary_and_diagnostic_json_defaults_remain_readable() {
    let mut store = Store::open(Path::new(":memory:"), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = setup(&mut store, "p");
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    let mut summary =
        serde_json::to_value(store.verification_run_summary("p", &saved.id).unwrap()).unwrap();
    summary.as_object_mut().unwrap().remove("schema_version");
    assert_eq!(
        serde_json::from_value::<VerificationRunSummary>(summary)
            .unwrap()
            .schema_version,
        1
    );
    let page = store
        .verification_diagnostics("p", &saved.id, Default::default(), 10, None, false)
        .unwrap();
    let mut page = serde_json::to_value(page).unwrap();
    page.as_object_mut().unwrap().remove("details_available");
    assert!(
        serde_json::from_value::<dctx0_verification::diagnostics::DiagnosticPage>(page)
            .unwrap()
            .details_available
    );
}

#[test]
fn schema_six_pinned_spec_queries_read_legacy_projections_without_migration() {
    let t = Temp::new();
    let mut old = legacy_store(&t.db());
    old.connection.execute_batch(STORAGE_MIGRATION).unwrap();
    old.connection.execute_batch(cases::MIGRATION).unwrap();
    let _ = setup(&mut old, "p");
    old.prepare_query_snapshot("p", "latest").unwrap();
    let expected = old.latest("p").unwrap().unwrap();
    let result = old
        .query_sql(
            "p",
            Default::default(),
            "SELECT subject,text FROM dctx0_subjects ORDER BY subject",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(result.rows.len(), expected.ir.nodes.len());
    assert_eq!(result.pins.snapshot, Some(expected.id));
    assert_eq!(
        old.connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        6
    );
}

#[test]
fn missing_snapshot_indexes_and_unrepresentable_sql_values_fail_explicitly() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let _ = setup(&mut store, "p");
    store
        .connection
        .execute("DELETE FROM query_snapshots", [])
        .unwrap();
    let error = store
        .query_sql(
            "p",
            Default::default(),
            "SELECT * FROM dctx0_subjects",
            &Default::default(),
            Default::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("QUERY_INDEX_MISSING") && error.contains("--snapshot"));
    store.prepare_query_snapshot("p", "latest").unwrap();
    for (sql, message) in [
        ("SELECT ?", "named"),
        ("SELECT CAST(x'80' AS TEXT)", "UTF-8"),
        ("SELECT CAST('data' AS BLOB)", "Binary"),
    ] {
        let error = store
            .query_sql(
                "p",
                Default::default(),
                sql,
                &Default::default(),
                Default::default(),
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains(message), "{sql}: {error}");
    }
    let error = store
        .query_sql(
            "p",
            Default::default(),
            "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT sum(x) FROM n",
            &Default::default(),
            crate::QueryLimits {
                timeout_ms: 50,
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("QUERY_TIMEOUT"), "{error}");
    let error = store
        .query_sql(
            "p",
            Default::default(),
            "SELECT :x",
            &[("x".into(), serde_json::json!("x".repeat(1000)))].into(),
            crate::QueryLimits {
                max_bytes: 1200,
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("QUERY_OUTPUT_LIMIT_EXCEEDED"), "{error}");
}

#[test]
fn case_digest_validation_rejects_invalid_utf8_and_mismatched_case_associations() {
    use std::io::Write;
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = setup(&mut store, "p");
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    let raw = b"\x80";
    let hash = format!("{:x}", Sha256::digest(raw));
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(raw).unwrap();
    let bytes = encoder.finish().unwrap();
    store
        .connection
        .execute(
            "INSERT INTO verification_case_bodies VALUES(?1,?2,1)",
            params![hash, bytes],
        )
        .unwrap();
    store
        .connection
        .execute(
            "UPDATE verification_case_index SET body_hash=?1 WHERE run_id=?2",
            params![hash, saved.id],
        )
        .unwrap();
    assert!(
        store
            .verification_case("p", &saved.id, "C")
            .unwrap_err()
            .to_string()
            .contains("UTF-8")
    );
    let mut other = run.cases[0].clone();
    other.id = "other".into();
    let (hash, length, bytes) = codec::compress_json(&other).unwrap();
    store
        .connection
        .execute(
            "INSERT INTO verification_case_bodies VALUES(?1,?2,?3)",
            params![hash, bytes, length as i64],
        )
        .unwrap();
    store
        .connection
        .execute(
            "UPDATE verification_case_index SET body_hash=?1 WHERE run_id=?2",
            params![hash, saved.id],
        )
        .unwrap();
    assert!(
        store
            .verification_case("p", &saved.id, "C")
            .unwrap_err()
            .to_string()
            .contains("association")
    );
}

#[test]
fn indexed_headers_cannot_hide_a_second_case_corpus_during_full_read_or_export() {
    let t = Temp::new();
    let mut store = Store::open(&t.db(), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = setup(&mut store, "p");
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    store
        .connection
        .execute(
            "UPDATE verification_runs SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&run).unwrap(), saved.id],
        )
        .unwrap();
    assert!(
        store
            .verification_run("p", &saved.id)
            .unwrap_err()
            .to_string()
            .contains("second case corpus")
    );
    let destination = t.0.join("report.json");
    fs::write(&destination, b"existing").unwrap();
    assert!(
        store
            .export_verification("p", &saved.id, None, &destination)
            .unwrap_err()
            .to_string()
            .contains("second case corpus")
    );
    assert_eq!(fs::read(&destination).unwrap(), b"existing");
}
