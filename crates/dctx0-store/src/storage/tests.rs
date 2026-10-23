use super::*;
use serde_json::json;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn memory() -> Store {
    Store::open(Path::new(":memory:"), AccessMode::ReadWrite).unwrap()
}
#[test]
fn maintenance_lock_waits_for_transient_readers_but_rejects_persistent_ones() {
    let path = std::env::temp_dir().join(format!(
        "dctx0-maintenance-lock-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, b"unchanged").unwrap();
    let reader = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    fs2::FileExt::try_lock_shared(&reader).unwrap();
    let writer = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    assert_eq!(
        lock_exclusive(&writer).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"unchanged");
    let released = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(30));
        drop(reader);
    });
    lock_exclusive(&writer).unwrap();
    released.join().unwrap();
    let other = std::fs::File::open(&path).unwrap();
    assert!(fs2::FileExt::try_lock_shared(&other).is_err());
    drop(writer);
    fs2::FileExt::try_lock_shared(&other).unwrap();
    drop(other);
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn installation_rejects_a_replaced_source_inode_while_waiting_for_readers() {
    let dir = std::env::temp_dir().join(format!(
        "dctx0-replaced-source-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("source.sqlite3");
    let mut store = Store::open(&source, AccessMode::ReadWrite).unwrap();
    let _ = crate::verification::tests::setup(&mut store, "p");
    store
        .connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
        .unwrap();
    let candidate = dir.join("candidate.sqlite3");
    store.storage_compact_copy(&candidate).unwrap();
    let replacement = dir.join("replacement.sqlite3");
    std::fs::copy(&source, &replacement).unwrap();
    let reader = Store::open(&source, AccessMode::ReadOnly).unwrap();
    let replaced = source.clone();
    let pending = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::rename(replacement, replaced).unwrap();
        drop(reader);
    });
    let version = store.verification_storage_version().unwrap();
    let backup = dir.join("backup.sqlite3");
    assert!(
        store
            .install_storage_copy(&candidate, version, &backup)
            .unwrap_err()
            .to_string()
            .contains("Storage changed")
    );
    pending.join().unwrap();
    assert!(!backup.exists());
    assert!(candidate.exists());
    assert_eq!(
        Store::open(&source, AccessMode::ReadOnly)
            .unwrap()
            .storage_inspect()
            .unwrap()
            .schema_version,
        7
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn missing_chunks_invalid_utf8_and_invalid_object_bytes_are_rejected() {
    let store = memory();
    let c = &store.connection;
    let text = "original text".repeat(10000);
    let packed = store_value(c, &json!(text));
    let root: i64 = packed.strip_prefix(PREFIX).unwrap().parse().unwrap();
    // Losing the membership must not silently return a shortened report.
    c.execute("DELETE FROM storage_chunk_refs WHERE parent=?1", [root])
        .unwrap();
    assert!(
        load::<Value>(c, &packed)
            .unwrap_err()
            .to_string()
            .contains("Missing stored text chunk")
    );
    let packed = store_value(c, &json!("different text".repeat(10000)));
    let root: i64 = packed.strip_prefix(PREFIX).unwrap().parse().unwrap();
    let chunk: i64 = c
        .query_row(
            "SELECT chunk FROM storage_chunk_refs WHERE parent=?1 LIMIT 1",
            [root],
            |r| r.get(0),
        )
        .unwrap();
    // A valid chunk digest does not make arbitrary bytes valid UTF-8.
    let hash = Sha256::digest([0xff]).to_vec();
    c.execute("UPDATE storage_chunks SET hash=?2,encoding='identity',raw_length=1,bytes=x'ff' WHERE id=?1",params![chunk,hash]).unwrap();
    assert!(
        load::<Value>(c, &packed)
            .unwrap_err()
            .to_string()
            .contains("Invalid stored text UTF-8")
    );
    let packed = store_value(c, &json!({"value": (0..3000).collect::<Vec<i64>>() }));
    let root: i64 = packed.strip_prefix(PREFIX).unwrap().parse().unwrap();
    c.execute(
        "UPDATE storage_objects SET encoding='identity',encoded_length=1,bytes=x'ff' WHERE id=?1",
        [root],
    )
    .unwrap();
    assert!(
        load::<Value>(c, &packed)
            .unwrap_err()
            .to_string()
            .contains("Invalid storage UTF-8")
    );
}

#[test]
fn normalized_spec_relations_and_facts_preserve_the_pinned_graph() {
    let mut store = memory();
    let sources = vec![dctx0_core::Source {
        file: "w.dctx0".into(),
        text: "workspace W { type A = Int; surface Api { operation get(a:A) -> A { pure } } }"
            .into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let snapshot = dctx0_changes::snapshot(&sources, ir.clone(), dctx0_graph::build(&ir));
    store.publish("p", &snapshot, None).unwrap();
    let result = store
        .query_sql(
            "p",
            Default::default(),
            "SELECT source,target,kind FROM dctx0_relations ORDER BY source,target,kind",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(result.rows.len(), snapshot.graph.edges.len());
    assert!(!result.rows.is_empty());
    let facts = store
        .query_sql(
            "p",
            Default::default(),
            "SELECT subject,text FROM dctx0_facts WHERE text LIKE '%operation%'",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert!(!facts.rows.is_empty());
}

#[test]
fn in_memory_installation_requires_a_file_lock_and_conversion_requires_a_source_file() {
    let store = memory();
    assert!(
        store
            .install_storage_copy(Path::new("unused"), 0, Path::new("unused-backup"))
            .unwrap_err()
            .to_string()
            .contains("maintenance lock")
    );
    let store = memory();
    let destination = std::env::temp_dir().join(format!(
        "dctx0-memory-conversion-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(
        store
            .storage_compact_copy(&destination)
            .unwrap_err()
            .to_string()
            .contains("file database")
    );
    assert!(!destination.exists());
}

#[test]
fn schema_one_conversion_and_invalid_source_timestamps_preserve_the_source() {
    let dir = std::env::temp_dir().join(format!(
        "dctx0-schema-one-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("source.sqlite3");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch(crate::SCHEMA).unwrap();
    let source = Store {
        connection,
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    let target = dir.join("converted.sqlite3");
    source.storage_compact_copy(&target).unwrap();
    assert!(source.storage_verify_copy(&target).unwrap().integrity_ok);
    assert_eq!(
        source
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    #[cfg(unix)]
    {
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let old = std::time::UNIX_EPOCH - std::time::Duration::from_secs(1);
        file.set_modified(old).unwrap();
        let failed = dir.join("invalid-time.sqlite3");
        assert!(source.storage_compact_copy(&failed).is_err());
        assert!(!failed.exists());
    }
    drop(source);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn source_projection_timeout_is_bounded_and_removes_its_progress_handler() {
    let mut store = memory();
    let _ = crate::verification::tests::setup(&mut store, "p");
    // The expensive scan is on the source database, before public SQL runs.
    store.connection.execute_batch("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000) INSERT INTO checks(project,snapshot_id,status,source_hash,diagnostics) SELECT project,snapshot_id,status,source_hash,diagnostics FROM checks,n WHERE checks.id=1;").unwrap();
    let error = store
        .query_sql(
            "p",
            Default::default(),
            "SELECT count(*) FROM dctx0_checks",
            &Default::default(),
            QueryLimits {
                timeout_ms: 1,
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("QUERY_TIMEOUT"), "{error}");
    // A timed-out read must leave neither an open transaction nor its deadline
    // callback installed on the caller's reusable source connection.
    let checks: i64 = store
        .connection
        .query_row("SELECT count(*) FROM checks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(checks, 20001);
    let report = store
        .query_sql(
            "p",
            Default::default(),
            "SELECT 42",
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(report.rows[0][0], 42);
}
#[test]
fn objects_share_large_fields_preserve_json_and_reject_corruption() {
    let store = memory();
    let c = &store.connection;
    let value = json!({"large":"large repeated value".repeat(4000),"array":[1,null,{"nested":[true,"x"]}],"literal":{"$dctx0_object":12}});
    let first = store_value(c, &value);
    let count: i64 = c
        .query_row("SELECT count(*) FROM storage_objects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(first, store_value(c, &value));
    assert_eq!(
        count,
        c.query_row("SELECT count(*) FROM storage_objects", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
    );
    let second = json!({"large":value["large"],"new":true});
    store_value(c, &second);
    assert_eq!(
        c.query_row("SELECT count(*) FROM storage_objects", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        count + 1
    );
    assert!(load::<Value>(c, "@dctx0:7:not-an-id").is_err());
    assert!(load::<Value>(c, "@dctx0:7:999999").is_err());
    let id: i64 = first.strip_prefix(PREFIX).unwrap().parse().unwrap();
    c.execute(
        "UPDATE storage_objects SET hash=zeroblob(32) WHERE id=?1",
        [id],
    )
    .unwrap();
    assert!(load::<Value>(c, &first).is_err());
}
fn store_value(c: &Connection, value: &Value) -> String {
    let packed = store(c, value).unwrap();
    assert_eq!(load::<Value>(c, &packed).unwrap(), *value);
    packed
}
#[test]
fn arrays_and_scalar_objects_validate_encoding_length_and_graph() {
    let store = memory();
    let c = &store.connection;
    let raw = json!(
        (0..40)
            .map(|i| json!({"i":i,"body":"body".repeat(2000)}))
            .collect::<Vec<_>>()
    );
    let packed = store_value(c, &raw);
    let root: i64 = packed.strip_prefix(PREFIX).unwrap().parse().unwrap();
    c.execute(
        "UPDATE storage_objects SET encoding='unsupported' WHERE id=?1",
        [root],
    )
    .unwrap();
    assert!(load::<Value>(c, &packed).is_err());
    c.execute(
        "UPDATE storage_objects SET encoding='identity',encoded_length=1,bytes=x'7b' WHERE id=?1",
        [root],
    )
    .unwrap();
    assert!(load::<Value>(c, &packed).is_err());
    let mut objects = BTreeMap::new();
    objects.insert(
        1,
        Stored {
            hash: vec![],
            raw_length: 0,
            node: Node::Array(vec![Part::Object(1)]),
        },
    );
    assert!(expand(1, &mut objects, &mut BTreeSet::new()).is_err());
}
#[test]
fn migration_preserves_every_task_pin_and_isolated_queries_skip_large_details() {
    let dir = std::env::temp_dir().join(format!(
        "dctx0-compact-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("source.sqlite3");
    let dest = dir.join("compact.sqlite3");
    let connection = Connection::open(&source).unwrap();
    for migration in [
        crate::SCHEMA,
        tasks::MIGRATION,
        tasks::PROFILE_MIGRATION,
        verification::MIGRATION,
        verification::STORAGE_MIGRATION,
        verification::cases::MIGRATION,
    ] {
        connection.execute_batch(migration).unwrap();
    }
    let mut old = Store {
        connection,
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    let sources=vec![dctx0_core::Source{file:"test.dctx0".into(),text:"workspace W { type A = Int; module M { surface S { operation get(a: A) -> A { pure } } } }".into()}];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    let snapshot = dctx0_changes::snapshot(&sources, ir, graph);
    old.publish("p", &snapshot, None).unwrap();
    let (check, _) = old.task_check("p").unwrap();
    let plan = old
        .publish_tasks(
            "p",
            &dctx0_tasks::plan(&snapshot).unwrap(),
            check,
            None,
            0,
            &BTreeMap::new(),
        )
        .unwrap();
    let original = old.task_session("p", None, None).unwrap();
    let original_json = serde_json::to_value(&original).unwrap();
    let before = old.storage_inspect().unwrap();
    assert_eq!(before.schema_version, 6);
    assert_eq!(old.storage_compact_copy(&dest).unwrap().schema_version, 7);
    assert!(old.storage_compact_copy(&dest).is_err());
    assert!(old.storage_verify_copy(&dest).unwrap().integrity_ok);
    // Resume a fully converted but not yet accepted staging database.
    let staging = Connection::open(&dest).unwrap();
    staging
        .execute("UPDATE storage_conversion SET status='incomplete'", [])
        .unwrap();
    drop(staging);
    assert_eq!(old.storage_compact_copy(&dest).unwrap().schema_version, 7);
    let compact = Store::open(&dest, AccessMode::ReadOnly).unwrap();
    assert_eq!(compact.latest("p").unwrap(), Some(snapshot.clone()));
    assert_eq!(
        serde_json::to_value(
            compact
                .task_session("p", Some(plan.id), Some(original.event_seq))
                .unwrap()
        )
        .unwrap(),
        original_json
    );
    let summary = compact.task_metadata_session("p", None, None).unwrap();
    assert_eq!(
        serde_json::to_value(summary.overviews()).unwrap(),
        serde_json::to_value(original.overviews()).unwrap()
    );
    let query=compact.query_sql("p",QueryPins::default(),"SELECT t.task_id,r.file FROM dctx0_tasks t JOIN dctx0_task_subjects s USING(task_id) JOIN dctx0_source_refs r ON r.subject=s.subject ORDER BY t.task_id",&BTreeMap::new(),QueryLimits::default()).unwrap();
    assert!(!query.rows.is_empty());
    assert_eq!(query.view_version, 2);
    let staging = Connection::open(&dest).unwrap();
    staging
        .execute("UPDATE task_events SET message='tampered' WHERE seq=1", [])
        .unwrap();
    assert!(old.storage_verify_copy(&dest).is_err());
    drop(staging);
    assert_eq!(old.storage_inspect().unwrap().schema_version, 6);
    let backup = dir.join("backup.sqlite3");
    old.storage_backup(&backup).unwrap();
    assert!(old.storage_backup(&backup).is_err());
    // Corrupt only execution detail: metadata queries and pins must still work.
    drop(compact);
    drop(old);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn retention_keeps_latest_and_latest_problem_but_metadata_outlives_bodies() {
    use dctx0_core::{VerificationIssue, VerificationResult};
    let mut store = memory();
    let (mut run, artifacts) = crate::verification::tests::setup(&mut store, "p");
    let issue = VerificationIssue {
        code: "ACTUAL_FAILURE".into(),
        message: "An imported storage fixture, not native execution".into(),
        span: None,
    };
    let outcomes = [
        VerificationResult::Pass,
        VerificationResult::Fail {
            issue: issue.clone(),
        },
        VerificationResult::Pass,
        VerificationResult::Unknown { issue },
        VerificationResult::Pass,
    ];
    let mut ids = Vec::new();
    for (n, result) in outcomes.into_iter().enumerate() {
        run.started_at_unix_ns = (n + 1).to_string();
        run.result = result.clone();
        run.cases[0].result = result;
        ids.push(
            store
                .record_verification_run("p", &run, &artifacts)
                .unwrap()
                .id,
        );
    }
    assert_eq!(store.verification_runs("p").unwrap().len(), 2);
    assert!(store.verification_run("p", &ids[3]).is_ok());
    assert!(store.verification_run("p", &ids[4]).is_ok());
    assert!(matches!(
        store.verification_run("p", &ids[1]),
        Err(Error::DetailsPruned(_))
    ));
    let report = store
        .query_sql(
            "p",
            QueryPins::default(),
            "SELECT run_id,body_available FROM dctx0_cases ORDER BY run_id",
            &BTreeMap::new(),
            QueryLimits::default(),
        )
        .unwrap();
    assert_eq!(report.rows.len(), 5);
    assert_eq!(report.rows.iter().filter(|r| r[1] == 1).count(), 2);
    let report = store
        .query_sql(
            "p",
            QueryPins::default(),
            "SELECT COUNT(*) FROM dctx0_run_requirements",
            &BTreeMap::new(),
            QueryLimits::default(),
        )
        .unwrap();
    assert_eq!(report.rows[0][0], 5);
}

#[test]
fn metadata_queries_preserve_pass_and_readiness_without_decompressing_evidence() {
    let mut store = memory();
    let sources = vec![dctx0_core::Source {
        file: "a.dctx0".into(),
        text: "workspace W { type A = Int }".into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    let snapshot = dctx0_changes::snapshot(&sources, ir, graph);
    store.publish("p", &snapshot, None).unwrap();
    let (check, _) = store.task_check("p").unwrap();
    let plan = store
        .publish_tasks(
            "p",
            &dctx0_tasks::plan(&snapshot).unwrap(),
            check,
            None,
            0,
            &BTreeMap::new(),
        )
        .unwrap();
    let task = plan.members[0].task_id;
    let mut state = dctx0_tasks::State {
        work: dctx0_tasks::WorkState::Done,
        check: dctx0_tasks::CheckStatus::Pass,
        ..Default::default()
    };
    state.result = Some(dctx0_tasks::TaskResult {
        summary: "Delivered".into(),
        artifacts: vec!["artifact".repeat(8000)],
        decisions: vec![],
        implementation: Some("pinned-implementation".into()),
    });
    state.evidence.push(dctx0_tasks::CheckEvidence {
        requirements_hash: plan.members[0].candidate.requirements_hash.clone(),
        implementation: "pinned-implementation".into(),
        verifier: "test fixture".into(),
        verifier_version: "1".into(),
        command: "fixture".into(),
        started_at: "1".into(),
        finished_at: "2".into(),
        report: "captured evidence".repeat(8000),
        status: dctx0_tasks::CheckStatus::Pass,
    });
    let raw = store_value(&store.connection, &serde_json::to_value(&state).unwrap());
    store.connection.execute("INSERT INTO task_events(project,task_id,kind,message,state_data) VALUES('p',?1,'check','fixture',?2)",params![task,raw]).unwrap();
    let seq = store.connection.last_insert_rowid();
    state_metadata(&store.connection, seq, task, &state).unwrap();
    let complete = store.task_session("p", None, None).unwrap();
    let compact = store.task_metadata_session("p", None, None).unwrap();
    assert_eq!(
        serde_json::to_value(complete.overviews()).unwrap(),
        serde_json::to_value(compact.overviews()).unwrap()
    );
    // Corrupt execution-body storage while retaining the independently saved metadata.
    store
        .connection
        .execute("UPDATE storage_objects SET bytes=x'ff'", [])
        .unwrap();
    let report = store
        .query_sql(
            "p",
            QueryPins::default(),
            "SELECT check_status,delivered FROM dctx0_tasks WHERE task_id=:task",
            &BTreeMap::from([("task".into(), json!(task))]),
            QueryLimits::default(),
        )
        .unwrap();
    assert_eq!(report.rows, vec![vec![json!("PASS"), json!(1)]]);
    let report = store
        .query_sql(
            "p",
            QueryPins::default(),
            "SELECT status,report_hash FROM dctx0_task_evidence WHERE task_id=:task",
            &BTreeMap::from([("task".into(), json!(task))]),
            QueryLimits::default(),
        )
        .unwrap();
    assert_eq!(report.rows[0][0], "PASS");
    assert!(store.task_session("p", None, None).is_err());
}

#[test]
fn content_defined_chunks_share_shifted_reports_and_preserve_unicode_and_whitespace() {
    let store = memory();
    let c = &store.connection;
    let shared = (0..10000)
        .map(|i| format!("  file-{i:05}-ä🦀: {:064x}\n", i * 7919))
        .collect::<String>();
    let first = json!(format!("task-one\n{shared}\n"));
    let second = json!(format!("task-two-with-a-different-prefix\n{shared}\n"));
    store_value(c, &first);
    let before: i64 = c
        .query_row("SELECT count(*) FROM storage_chunks", [], |r| r.get(0))
        .unwrap();
    let packed = store_value(c, &second);
    let after: i64 = c
        .query_row("SELECT count(*) FROM storage_chunks", [], |r| r.get(0))
        .unwrap();
    assert!(before > 5 && after < before + 5, "{before} -> {after}");
    let changed=c.execute(
        "UPDATE storage_chunks SET hash=zeroblob(32) WHERE id=(SELECT chunk FROM storage_chunk_refs GROUP BY chunk HAVING count(DISTINCT parent)>1 LIMIT 1)",
        [],
    )
    .unwrap();
    assert_eq!(changed, 1);
    assert!(load::<Value>(c, &packed).is_err());
}

#[test]
fn workflow_parent_links_preserve_retries_without_changing_run_ids_and_expire_with_the_workflow() {
    let mut store = memory();
    let (mut run, mut artifacts) = crate::verification::tests::setup(&mut store, "p");
    let metrics = VerificationArtifact::new(
        "application/vnd.dctx0.runtime-metrics+json",
        br#"{"kind":"fixture","peak_rss_bytes":123}"#.to_vec(),
    );
    artifacts.push(metrics.clone());
    run.artifacts.push(metrics.reference.clone());
    let parent = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    for n in 2..=3 {
        run.started_at_unix_ns = n.to_string();
        let child = store
            .record_verification_run_owned_from("p", run.clone(), &artifacts, Some(&parent.id))
            .unwrap();
        assert_eq!(child.id, verification::codec::json_hash(&run).unwrap());
        assert_eq!(store.verification_run("p", &parent.id).unwrap(), parent);
        assert_eq!(store.verification_runs("p").unwrap().len(), 2);
    }
    let links = store
        .query_sql(
            "p",
            QueryPins::default(),
            "SELECT COUNT(*) FROM dctx0_run_links",
            &BTreeMap::new(),
            QueryLimits::default(),
        )
        .unwrap();
    assert_eq!(links.rows[0][0], 2);
    run.started_at_unix_ns = "4".into();
    assert!(
        store
            .record_verification_run_owned_from(
                "p",
                run.clone(),
                &artifacts,
                Some("missing-parent")
            )
            .is_err()
    );
    assert_eq!(store.verification_runs("p").unwrap().len(), 2);
    run.artifacts.retain(|a| a.hash != metrics.reference.hash);
    artifacts.retain(|a| a.reference.hash != metrics.reference.hash);
    store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    assert!(matches!(
        store.verification_run("p", &parent.id),
        Err(Error::DetailsPruned(_))
    ));
    assert_eq!(
        store
            .verification_artifact(&parent.id, &metrics.reference.hash)
            .unwrap(),
        metrics
    );
}

#[test]
fn legacy_pruned_empty_headers_migrate_without_inventing_missing_case_metadata() {
    let dir = std::env::temp_dir().join(format!(
        "dctx0-empty-headers-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("source.sqlite3");
    let destination = dir.join("compact.sqlite3");
    let connection = Connection::open(&source).unwrap();
    for migration in [
        crate::SCHEMA,
        tasks::MIGRATION,
        tasks::PROFILE_MIGRATION,
        verification::MIGRATION,
        verification::STORAGE_MIGRATION,
        verification::cases::MIGRATION,
    ] {
        connection.execute_batch(migration).unwrap();
    }
    let mut old = Store {
        connection,
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    let (mut run, artifacts) = crate::verification::tests::setup(&mut old, "p");
    for n in 1..=5 {
        run.started_at_unix_ns = n.to_string();
        old.record_verification_run("p", &run, &artifacts).unwrap();
    }
    assert_eq!(
        old.connection
            .query_row(
                "SELECT count(*) FROM verification_runs WHERE data=''",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    old.storage_compact_project_copy(Some("p"), &destination)
        .unwrap();
    assert!(old.storage_verify_copy(&destination).unwrap().integrity_ok);
    let compact = Store::open(&destination, AccessMode::ReadOnly).unwrap();
    assert_eq!(
        compact
            .verification_run_summaries("p", 0, 200)
            .unwrap()
            .len(),
        5
    );
    let cases = compact
        .query_sql(
            "p",
            QueryPins::default(),
            "SELECT COUNT(*),SUM(body_available) FROM dctx0_cases",
            &BTreeMap::new(),
            QueryLimits::default(),
        )
        .unwrap();
    assert_eq!(cases.rows[0], vec![json!(3), json!(1)]);
    drop(compact);
    drop(old);
    std::fs::remove_dir_all(dir).unwrap();
}
