use dctx0_changes::{HistoryReader, HistoryWriter};
use dctx0_core::*;
use dctx0_store::{AccessMode, Store, VerificationArtifact};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dctx0-verification-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> PathBuf {
        self.0.join("state.sqlite3")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixture(store: &mut Store) -> (VerificationRun, Vec<VerificationArtifact>) {
    let sources = vec![Source {
        file: "w.dctx0".into(),
        text: "workspace W { type Number = Int }".into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    let snapshot = dctx0_changes::snapshot(&sources, ir, graph);
    store.publish("p", &snapshot, None).unwrap();
    let occurrence = store.history("p").unwrap()[0].run_id;
    let artifact=VerificationArtifact::new("application/json",serde_json::to_vec(&serde_json::json!({"snapshot":snapshot.id,"scope":["W.Number"],"required_cases":["C1"]})).unwrap());
    let run = VerificationRun {
        schema_version: 1,
        origin: VerificationRunOrigin::Imported,
        started_at_unix_ns: "1234567890000000000".into(),
        elapsed_us: 0,
        provenance: VerificationProvenance {
            snapshot: snapshot.id,
            check_occurrence: occurrence,
            plan_hash: artifact.reference.hash.clone(),
            implementation: BTreeMap::new(),
            dependencies: BTreeMap::new(),
            adapter_hash: String::new(),
            configuration_hash: String::new(),
            harness_hash: String::new(),
            interpreter_version: "dctx0-interpreter-1".into(),
            generator_version: "test-import".into(),
            protocol_version: 1,
            target: "typescript".into(),
            runtime_version: "not-executed-import".into(),
            build_configuration: BTreeMap::new(),
            task_pin: None,
            incomplete: vec![VerificationIssue {
                code: "IMPORTED_EVIDENCE".into(),
                message: "No native execution authority".into(),
                span: None,
            }],
        },
        budgets: Default::default(),
        selected_subjects: BTreeSet::from(["W.Number".into()]),
        required_subjects: BTreeSet::from(["W.Number".into()]),
        not_selected: vec![],
        required_cases: BTreeSet::from(["C1".into()]),
        cases: vec![VerificationCaseRecord {
            resources: None,
            id: "C1".into(),
            subject: "W.Number".into(),
            kind: VerificationCaseKind::Contract,
            bindings: BTreeMap::new(),
            setup: vec![],
            arguments: vec![vec![VerificationValue::Integer("1".into())]],
            observations: vec![],
            events: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        }],
        open_obligations: vec![],
        bounded_obligations: vec!["finite imported corpus".into()],
        artifacts: vec![artifact.reference.clone()],
        result: VerificationResult::Pass,
    };
    (run, vec![artifact])
}
#[test]
fn immutable_runs_artifacts_and_occurrences_roundtrip_without_task_adoption() {
    let t = Temp::new();
    let mut s = Store::open(&t.path(), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = fixture(&mut s);
    let saved = s.record_verification_run("p", &run, &artifacts).unwrap();
    assert_eq!(saved.run, run);
    assert_eq!(
        s.record_verification_run("p", &run, &artifacts).unwrap(),
        saved
    );
    assert_eq!(s.verification_runs("p").unwrap(), vec![saved.clone()]);
    assert_eq!(
        s.verification_artifact(&saved.id, &artifacts[0].reference.hash)
            .unwrap(),
        artifacts[0]
    );
    assert!(s.verification_run("other", &saved.id).is_err());
    assert!(
        s.verification_artifact("other", &artifacts[0].reference.hash)
            .is_err()
    );
    let mut later = run.clone();
    later.started_at_unix_ns = "1234567890000000001".into();
    assert_ne!(
        s.record_verification_run("p", &later, &artifacts)
            .unwrap()
            .id,
        saved.id
    );
    assert_eq!(s.verification_runs("p").unwrap().len(), 2);
    let c = rusqlite::Connection::open(t.path()).unwrap();
    for table in [
        "task_events",
        "verification_task_links",
        "verification_adoptions",
    ] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    drop(s);
    let mut reader = Store::open(&t.path(), AccessMode::ReadOnly).unwrap();
    assert_eq!(reader.verification_run("p", &saved.id).unwrap(), saved);
    assert!(
        reader
            .record_verification_run("p", &run, &artifacts)
            .is_err()
    );
}
#[test]
fn contradictory_scope_coverage_digest_and_check_provenance_are_rejected() {
    let t = Temp::new();
    let mut s = Store::open(&t.path(), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = fixture(&mut s);
    for mode in 0..12 {
        let mut bad = run.clone();
        match mode {
            0 => bad.schema_version = 9,
            1 => bad.started_at_unix_ns = "bad".into(),
            2 => bad.selected_subjects.clear(),
            3 => bad.cases.push(bad.cases[0].clone()),
            4 => bad.cases[0].id = "surplus".into(),
            5 => bad.cases[0].subject = "unselected".into(),
            6 => bad.cases.clear(),
            7 => bad.open_obligations.push(VerificationIssue {
                code: "UNKNOWN".into(),
                message: "unobserved".into(),
                span: None,
            }),
            8 => bad.provenance.check_occurrence = 999,
            9 => bad.provenance.snapshot = "wrong".into(),
            10 => bad.artifacts.clear(),
            _ => {
                bad.cases[0].result = VerificationResult::Fail {
                    issue: VerificationIssue {
                        code: "VIOLATION".into(),
                        message: "failed case".into(),
                        span: None,
                    },
                }
            }
        }
        assert!(
            s.record_verification_run("p", &bad, &artifacts).is_err(),
            "mode {mode}"
        );
        assert!(s.verification_runs("p").unwrap().is_empty());
    }
    let mut bad_artifact = artifacts.clone();
    bad_artifact[0].contents.push(b'x');
    assert!(s.record_verification_run("p", &run, &bad_artifact).is_err());
    assert!(
        s.record_verification_run("p", &run, &[artifacts[0].clone(), artifacts[0].clone()])
            .is_err()
    );
    s.failure("p", "bad-source", &[]).unwrap();
    let mut failed_check = run.clone();
    failed_check.provenance.check_occurrence = s.history("p").unwrap()[0].run_id;
    assert!(
        s.record_verification_run("p", &failed_check, &artifacts)
            .is_err()
    );
    let mut unknown = run.clone();
    unknown.cases[0].result = VerificationResult::Unknown {
        issue: VerificationIssue {
            code: "UNSUPPORTED".into(),
            message: "missing observer".into(),
            span: None,
        },
    };
    unknown.result = unknown.cases[0].result.clone();
    s.record_verification_run("p", &unknown, &artifacts)
        .unwrap();
    let mut failure = unknown.clone();
    failure.started_at_unix_ns = "2".into();
    failure.cases[0].result = VerificationResult::Fail {
        issue: VerificationIssue {
            code: "VIOLATION".into(),
            message: "actual mismatch".into(),
            span: None,
        },
    };
    assert!(
        s.record_verification_run("p", &failure, &artifacts)
            .is_err()
    );
    failure.result = failure.cases[0].result.clone();
    s.record_verification_run("p", &failure, &artifacts)
        .unwrap();
}
#[test]
fn artifact_corruption_and_transaction_failure_never_publish_half_a_run() {
    let t = Temp::new();
    let mut s = Store::open(&t.path(), AccessMode::ReadWrite).unwrap();
    let (run, artifacts) = fixture(&mut s);
    let c = rusqlite::Connection::open(t.path()).unwrap();
    c.execute_batch("CREATE TRIGGER reject_run BEFORE INSERT ON verification_runs BEGIN SELECT RAISE(ABORT,'reject evidence'); END;").unwrap();
    assert!(s.record_verification_run("p", &run, &artifacts).is_err());
    assert_eq!(
        c.query_row("SELECT count(*) FROM verification_artifacts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    c.execute_batch("DROP TRIGGER reject_run;").unwrap();
    let saved = s.record_verification_run("p", &run, &artifacts).unwrap();
    c.execute("UPDATE verification_artifacts SET bytes=x'00'", [])
        .unwrap();
    assert!(
        s.verification_artifact(&saved.id, &artifacts[0].reference.hash)
            .is_err()
    );
    assert!(s.record_verification_run("p", &run, &artifacts).is_err());
    c.execute("UPDATE verification_runs SET data='invalid JSON'", [])
        .unwrap();
    assert!(s.verification_run("p", &saved.id).is_err());
    assert!(s.verification_runs("p").is_err());
}
#[test]
fn readers_accept_legacy_schemas_without_migrating_and_writers_add_schema_four() {
    for version in 1..=3 {
        let t = Temp::new();
        let mut writer = Store::open(&t.path(), AccessMode::ReadWrite).unwrap();
        let (run, _) = fixture(&mut writer);
        drop(writer);
        let c = rusqlite::Connection::open(t.path()).unwrap();
        c.execute_batch("DROP TABLE verification_adoptions; DROP TABLE verification_task_links; DROP TABLE verification_run_artifacts; DROP TABLE verification_runs; DROP TABLE verification_artifacts;").unwrap();
        c.pragma_update(None, "user_version", version).unwrap();
        drop(c);
        let before = fs::read(t.path()).unwrap();
        let reader = Store::open(&t.path(), AccessMode::ReadOnly).unwrap();
        assert!(reader.verification_runs("p").unwrap().is_empty());
        assert!(reader.verification_run("p", "missing").is_err());
        assert!(reader.verification_artifact("missing", "hash").is_err());
        assert_eq!(
            reader.latest("p").unwrap().unwrap().id,
            run.provenance.snapshot
        );
        drop(reader);
        assert_eq!(fs::read(t.path()).unwrap(), before);
        // Version 1/2 simulations retain later tables, so verify migration 3 directly;
        // independent existing migration fixtures exercise the genuinely old schemas.
        let c = rusqlite::Connection::open(t.path()).unwrap();
        c.pragma_update(None, "user_version", 3).unwrap();
        drop(c);
        let writer = Store::open(&t.path(), AccessMode::ReadWrite).unwrap();
        assert!(writer.verification_runs("p").unwrap().is_empty());
        assert_eq!(
            rusqlite::Connection::open(t.path())
                .unwrap()
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            4
        );
    }
}
