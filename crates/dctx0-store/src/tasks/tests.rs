use super::read::schema;
use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn snapshot() -> Snapshot {
    let sources = vec![dctx0_core::Source {
        file: "spec.dctx0".into(),
        text: "workspace W { type A = Int; module M { surface Api { operation get(a: A) -> A { pure } } } }"
            .into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    dctx0_changes::snapshot(&sources, ir, graph)
}
#[test]
fn legacy_readers_never_migrate_and_writers_preserve_existing_history() {
    let file = std::env::temp_dir().join(format!(
        "dctx0-legacy-{}-{}.sqlite3",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let connection = Connection::open(&file).unwrap();
    connection.execute_batch(crate::SCHEMA).unwrap();
    let mut legacy = Store {
        connection,
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    let input = snapshot();
    legacy.publish("p", &input, None).unwrap();
    drop(legacy);
    let reader = Store::open(&file, AccessMode::ReadOnly).unwrap();
    assert_eq!(reader.task_head("p").unwrap(), None);
    assert_eq!(reader.task_sequence("p").unwrap(), 0);
    assert!(reader.task_session("p", None, None).is_err());
    assert_eq!(reader.latest("p").unwrap().unwrap().id, input.id);
    drop(reader);
    let writer = Store::open(&file, AccessMode::ReadWrite).unwrap();
    assert_eq!(schema(&writer.connection).unwrap(), 6);
    assert_eq!(writer.history("p").unwrap().len(), 1);
    assert_eq!(writer.latest("p").unwrap().unwrap().id, input.id);
    assert!(writer.task_check("missing").is_err());
    drop(writer);
    fs::remove_file(file).unwrap();
}
#[test]
fn every_publication_write_failure_rolls_back_the_complete_plan() {
    for table in [
        "tasks",
        "task_events",
        "task_states",
        "task_plans",
        "task_members",
        "storage_state_metadata",
        "task_heads",
    ] {
        let mut store = Store {
            connection: Connection::open_in_memory().unwrap(),
            mode: AccessMode::ReadWrite,
            maintenance_lock: None,
        };
        initialize(&mut store.connection, 0, AccessMode::ReadWrite).unwrap();
        let input = snapshot();
        store.publish("p", &input, None).unwrap();
        let (run, _) = store.task_check("p").unwrap();
        store.connection.execute_batch(&format!("CREATE TRIGGER reject BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'injected publication failure'); END;")).unwrap();
        let result = store.publish_tasks(
            "p",
            &dctx0_tasks::plan(&input).unwrap(),
            run,
            None,
            0,
            &BTreeMap::new(),
        );
        assert!(result.is_err(), "{table}");
        assert_eq!(store.task_head("p").unwrap(), None);
        assert_eq!(store.task_sequence("p").unwrap(), 0);
        let count: i64 = store
            .connection
            .query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
#[test]
fn missing_states_and_corrupt_reports_are_explicit_errors() {
    let mut store = Store {
        connection: Connection::open_in_memory().unwrap(),
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    initialize(&mut store.connection, 0, AccessMode::ReadWrite).unwrap();
    let input = snapshot();
    store.publish("p", &input, None).unwrap();
    let (run, _) = store.task_check("p").unwrap();
    let p = store
        .publish_tasks(
            "p",
            &dctx0_tasks::plan(&input).unwrap(),
            run,
            None,
            0,
            &BTreeMap::new(),
        )
        .unwrap();
    let id = p.members[0].task_id;
    store
        .connection
        .execute(
            "UPDATE task_events SET state_data=NULL WHERE task_id=?1",
            [id],
        )
        .unwrap();
    assert!(
        store
            .task_session("p", None, None)
            .unwrap_err()
            .to_string()
            .contains("missing required state")
    );
    store
        .connection
        .execute(
            "UPDATE task_events SET state_data='broken' WHERE task_id=?1",
            [id],
        )
        .unwrap();
    assert!(store.task_session("p", None, None).is_err());
}

fn prepared() -> (Store, Snapshot, Plan) {
    let mut store = Store {
        connection: Connection::open_in_memory().unwrap(),
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    initialize(&mut store.connection, 0, AccessMode::ReadWrite).unwrap();
    let input = snapshot();
    store.publish("p", &input, None).unwrap();
    let (run, _) = store.task_check("p").unwrap();
    let plan = store
        .publish_tasks(
            "p",
            &dctx0_tasks::plan(&input).unwrap(),
            run,
            None,
            0,
            &BTreeMap::new(),
        )
        .unwrap();
    (store, input, plan)
}
#[test]
fn missing_tables_snapshots_and_updated_row_failures_never_reset_state() {
    for table in ["task_events", "task_links"] {
        let (store, _, _) = prepared();
        store
            .connection
            .execute_batch(&format!(
                "PRAGMA foreign_keys=OFF; DROP TABLE {table}; PRAGMA foreign_keys=ON;"
            ))
            .unwrap();
        if table == "task_events" {
            assert!(store.task_sequence("p").is_err());
        } else {
            assert!(store.task_session("p", None, None).is_err());
        }
    }
    for clause in [
        "BEFORE UPDATE OF data ON tasks",
        "BEFORE UPDATE OF created_seq ON tasks",
        "BEFORE UPDATE ON task_plans",
        "BEFORE INSERT ON task_events WHEN NEW.kind='plan'",
    ] {
        let mut store = Store {
            connection: Connection::open_in_memory().unwrap(),
            mode: AccessMode::ReadWrite,
            maintenance_lock: None,
        };
        initialize(&mut store.connection, 0, AccessMode::ReadWrite).unwrap();
        let input = snapshot();
        store.publish("p", &input, None).unwrap();
        let (run, _) = store.task_check("p").unwrap();
        store.connection.execute_batch(&format!("CREATE TRIGGER reject {clause} BEGIN SELECT RAISE(ABORT,'updated-row rejection'); END;")).unwrap();
        assert!(
            store
                .publish_tasks(
                    "p",
                    &dctx0_tasks::plan(&input).unwrap(),
                    run,
                    None,
                    0,
                    &BTreeMap::new()
                )
                .is_err()
        );
        assert_eq!(store.task_head("p").unwrap(), None);
    }
    let mut store = Store {
        connection: Connection::open_in_memory().unwrap(),
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    initialize(&mut store.connection, 0, AccessMode::ReadWrite).unwrap();
    let input = snapshot();
    store.publish("p", &input, None).unwrap();
    let (run, _) = store.task_check("p").unwrap();
    store
        .connection
        .execute_batch("PRAGMA foreign_keys=OFF; DELETE FROM snapshots; PRAGMA foreign_keys=ON;")
        .unwrap();
    assert!(
        store
            .publish_tasks(
                "p",
                &dctx0_tasks::plan(&input).unwrap(),
                run,
                None,
                0,
                &BTreeMap::new()
            )
            .is_err()
    );
}

#[test]
fn changed_publications_roll_back_deltas_and_invalidated_reservations() {
    for clause in [
        "BEFORE UPDATE OF data ON tasks WHEN json_array_length(json_extract(NEW.data,'$.changes'))>0",
        "BEFORE UPDATE ON task_groups",
        "BEFORE INSERT ON task_events WHEN NEW.kind='reservation-invalidated'",
    ] {
        let (mut store, input, old) = prepared();
        let anchor = old
            .members
            .iter()
            .max_by_key(|m| m.candidate.wave)
            .unwrap()
            .task_id;
        store
            .task_action(
                "p",
                Action::ClaimSlice {
                    anchor,
                    actor: "worker".into(),
                    domain: None,
                },
            )
            .unwrap();
        let old_seq = store.task_sequence("p").unwrap();
        let mut sources = input.sources.clone();
        sources[0].text = sources[0].text.replace("Int", "String");
        let ir = dctx0_language::compile(&sources).ir.unwrap();
        let graph = dctx0_graph::build(&ir);
        let changed = dctx0_changes::snapshot(&sources, ir, graph);
        store.publish("p", &changed, None).unwrap();
        let (run, _) = store.task_check("p").unwrap();
        let continuity = input
            .ir
            .symbols
            .keys()
            .map(|id| (id.clone(), id.clone()))
            .collect();
        store.connection.execute_batch(&format!("CREATE TRIGGER reject {clause} BEGIN SELECT RAISE(ABORT,'changed-plan failure'); END;")).unwrap();
        assert!(
            store
                .publish_tasks(
                    "p",
                    &dctx0_tasks::plan(&changed).unwrap(),
                    run,
                    Some(old.id),
                    old_seq,
                    &continuity
                )
                .is_err(),
            "{clause}"
        );
        assert_eq!(store.task_head("p").unwrap(), Some(old.id));
        assert_eq!(store.task_sequence("p").unwrap(), old_seq);
        assert!(
            store
                .task_session("p", None, None)
                .unwrap()
                .states
                .values()
                .all(|s| s.actor.as_deref() == Some("worker"))
        );
    }
}

#[test]
fn explicit_remediation_is_atomic_when_new_work_or_assembly_cannot_be_written() {
    for kind in ["Operation", "Integration"] {
        let (mut store, _, old) = prepared();
        let session = store.task_session("p", None, None).unwrap();
        let id = session
            .plan
            .members
            .iter()
            .find(|m| m.candidate.kind == TaskKind::Operation)
            .unwrap()
            .task_id;
        while !store
            .task_session("p", None, None)
            .unwrap()
            .overviews()
            .iter()
            .find(|r| r.task_id == id)
            .unwrap()
            .ready
        {
            let ready = store
                .task_session("p", None, None)
                .unwrap()
                .next(&Filters::default(), None)
                .unwrap()
                .task_id;
            let state = store
                .task_action(
                    "p",
                    Action::Start {
                        task: ready,
                        actor: "helper".into(),
                        reservation: None,
                    },
                )
                .unwrap()
                .state
                .unwrap();
            store
                .task_action(
                    "p",
                    Action::Complete {
                        task: ready,
                        actor: "helper".into(),
                        claim: state.claim.unwrap(),
                        version: state.version,
                        result: TaskResult {
                            summary: "Prerequisite delivered".into(),
                            artifacts: vec![],
                            decisions: vec![],
                            implementation: Some("commit".into()),
                        },
                    },
                )
                .unwrap();
        }
        let state = store
            .task_action(
                "p",
                Action::Start {
                    task: id,
                    actor: "worker".into(),
                    reservation: None,
                },
            )
            .unwrap()
            .state
            .unwrap();
        store
            .task_action(
                "p",
                Action::Complete {
                    task: id,
                    actor: "worker".into(),
                    claim: state.claim.unwrap(),
                    version: state.version,
                    result: TaskResult {
                        summary: "Delivered".into(),
                        artifacts: vec![],
                        decisions: vec![],
                        implementation: Some("commit".into()),
                    },
                },
            )
            .unwrap();
        store
            .task_action(
                "p",
                Action::RecordCheck {
                    task: id,
                    evidence: CheckEvidence {
                        requirements_hash: session.definitions[&id]
                            .candidate
                            .requirements_hash
                            .clone(),
                        implementation: "commit".into(),
                        verifier: "cargo".into(),
                        verifier_version: "1".into(),
                        command: "cargo test".into(),
                        started_at: "2026-10-09T00:00:00Z".into(),
                        finished_at: "2026-10-09T00:01:00Z".into(),
                        report: "test failed".into(),
                        status: CheckStatus::Fail,
                    },
                },
            )
            .unwrap();
        let seq = store.task_sequence("p").unwrap();
        store.connection.execute_batch(&format!("CREATE TRIGGER reject BEFORE INSERT ON tasks WHEN NEW.kind='{kind}' BEGIN SELECT RAISE(ABORT,'remediation failure'); END;")).unwrap();
        assert!(
            store
                .task_follow_up("p", id, None, "Fix failing check")
                .is_err(),
            "{kind}"
        );
        assert_eq!(store.task_head("p").unwrap(), Some(old.id));
        assert_eq!(store.task_sequence("p").unwrap(), seq);
        assert_eq!(
            store.task_session("p", None, None).unwrap().states[&id].check,
            CheckStatus::Fail
        );
    }
}

#[test]
fn structurally_corrupt_plans_are_rejected_before_queries_can_panic() {
    for corruption in 0..7 {
        let (store, _, mut plan) = prepared();
        match corruption {
            0 => plan.id += 10,
            1 => plan.members[0].task_id = 999,
            2 => plan.members.push(plan.members[0].clone()),
            3 => plan.members[0].candidate.wave += 1,
            4 => plan.members[0].candidate.prerequisites.push(Prerequisite {
                key: "missing".into(),
                condition: Condition::Delivered,
                reason: "corrupt".into(),
            }),
            5 => {
                let id = plan.members[0].task_id;
                let data: String = store
                    .connection
                    .query_row("SELECT data FROM tasks WHERE id=?1", [id], |r| r.get(0))
                    .unwrap();
                let mut d: Definition = decode(&data).unwrap();
                d.id += 10;
                store
                    .connection
                    .execute(
                        "UPDATE tasks SET data=?1 WHERE id=?2",
                        params![serde_json::to_string(&d).unwrap(), id],
                    )
                    .unwrap();
            }
            _ => {
                store.connection.execute_batch("PRAGMA foreign_keys=OFF; INSERT INTO task_links(project,from_id,to_id,kind,reason) SELECT 'p',999,min(id),'update','corrupt' FROM tasks; PRAGMA foreign_keys=ON;").unwrap();
            }
        }
        store
            .connection
            .execute(
                "UPDATE task_plans SET data=?1",
                [serde_json::to_string(&plan).unwrap()],
            )
            .unwrap();
        assert!(
            store.task_session("p", None, None).is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn schema_two_readers_preserve_bytes_and_writers_add_profile_tables() {
    let file = std::env::temp_dir().join(format!(
        "dctx0-schema-two-{}-{}.sqlite3",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let connection = Connection::open(&file).unwrap();
    connection.execute_batch(crate::SCHEMA).unwrap();
    connection.execute_batch(MIGRATION).unwrap();
    drop(connection);
    let before = fs::read(&file).unwrap();
    let reader = Store::open(&file, AccessMode::ReadOnly).unwrap();
    assert_eq!(schema(&reader.connection).unwrap(), 2);
    drop(reader);
    assert_eq!(before, fs::read(&file).unwrap());
    let writer = Store::open(&file, AccessMode::ReadWrite).unwrap();
    assert_eq!(schema(&writer.connection).unwrap(), 6);
    let count: i64 = writer
        .connection
        .query_row("SELECT COUNT(*) FROM execution_bundles", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    drop(writer);
    fs::remove_file(file).unwrap();
}

fn profile_catalog() -> (dctx0_core::TargetManifest, Vec<dctx0_core::ProfileDocument>) {
    use dctx0_core::*;
    let manifest = TargetManifest {
        format_version: 1,
        target: "rust".into(),
        template_version: 1,
        standard_keys: standard_profile_keys(),
    };
    let mut docs = vec![ProfileDocument {
        target: "rust".into(),
        key: None,
        path: ".dctx0/profiles/rust/common.md".into(),
        text: "Original guidance".into(),
        content_hash: dctx0_changes::hash(&"Original guidance"),
        subjects: vec![],
    }];
    for key in &manifest.standard_keys {
        docs.push(ProfileDocument {
            target: "rust".into(),
            key: Some(key.clone()),
            path: format!(".dctx0/profiles/rust/{}", key.path()),
            text: "Original guidance".into(),
            content_hash: dctx0_changes::hash(&"Original guidance"),
            subjects: vec![],
        });
    }
    (manifest, docs)
}
fn deliver(
    store: &mut Store,
    id: i64,
    manifest: &dctx0_core::TargetManifest,
    docs: &[dctx0_core::ProfileDocument],
) {
    let r = store
        .task_action_with_profiles(
            "p",
            Action::Start {
                task: id,
                actor: "a".into(),
                reservation: None,
            },
            Some((manifest, docs)),
        )
        .unwrap();
    let state = r.state.unwrap();
    store
        .task_action(
            "p",
            Action::Complete {
                task: id,
                actor: "a".into(),
                claim: state.claim.unwrap(),
                version: state.version,
                result: TaskResult {
                    summary: "Delivered".into(),
                    artifacts: vec![],
                    decisions: vec![],
                    implementation: Some("revision1".into()),
                },
            },
        )
        .unwrap();
}
#[test]
fn profile_binding_write_failures_roll_back_claims_and_bundle_history() {
    let (manifest, docs) = profile_catalog();
    for table in ["execution_bundles", "task_profile_bindings"] {
        let (mut store, _, _) = prepared();
        let session = store.task_session("p", None, None).unwrap();
        let seq = session.event_seq;
        store.connection.execute_batch(&format!("CREATE TRIGGER reject_profile BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'disk unavailable'); END;")).unwrap();
        assert!(
            store
                .task_action_with_profiles(
                    "p",
                    Action::ClaimNext {
                        actor: "a".into(),
                        filters: Filters::default()
                    },
                    Some((&manifest, &docs))
                )
                .is_err()
        );
        let after = store.task_session("p", None, None).unwrap();
        assert_eq!(after.event_seq, seq);
        assert!(
            after
                .states
                .values()
                .all(|s| s.actor.is_none() && s.execution_bundle.is_none())
        );
        let count: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM execution_bundles", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
#[test]
fn profile_update_write_failures_roll_back_definitions_plans_and_receipts() {
    for clause in [
        "BEFORE INSERT ON tasks",
        "BEFORE UPDATE OF data ON tasks WHEN OLD.data != '{}'",
        "BEFORE INSERT ON task_profile_updates",
    ] {
        let (mut store, input, _) = prepared();
        let (manifest, mut docs) = profile_catalog();
        let session = store.task_session("p", None, None).unwrap();
        let id = session.next(&Filters::default(), None).unwrap().task_id;
        deliver(&mut store, id, &manifest, &docs);
        let old = store.task_session("p", None, None).unwrap();
        docs[0].text = "Revised guidance".into();
        docs[0].content_hash = dctx0_changes::hash(&docs[0].text);
        let preview = profile_changes(
            &old,
            &input,
            &manifest,
            &docs,
            ProfileUpdateSelection {
                tasks: vec![id],
                ..Default::default()
            },
        )
        .unwrap();
        store.connection.execute_batch(&format!("CREATE TRIGGER reject_profile {clause} BEGIN SELECT RAISE(ABORT,'disk unavailable'); END;")).unwrap();
        assert!(
            store
                .task_profile_update("p", &preview, &manifest, &docs, "Adopt")
                .is_err()
        );
        let current = store.task_session("p", None, None).unwrap();
        assert_eq!(current.plan.id, old.plan.id);
        assert_eq!(current.event_seq, old.event_seq);
        assert_eq!(current.definitions.len(), old.definitions.len());
    }
}
#[test]
fn missing_snapshots_reject_profile_claims_and_updates_without_resetting_history() {
    for update in [false, true] {
        let (mut store, input, _) = prepared();
        let (manifest, mut docs) = profile_catalog();
        let session = store.task_session("p", None, None).unwrap();
        let id = session.next(&Filters::default(), None).unwrap().task_id;
        let preview = if update {
            deliver(&mut store, id, &manifest, &docs);
            docs[0].text = "Revised".into();
            docs[0].content_hash = dctx0_changes::hash(&docs[0].text);
            Some(
                profile_changes(
                    &store.task_session("p", None, None).unwrap(),
                    &input,
                    &manifest,
                    &docs,
                    ProfileUpdateSelection {
                        tasks: vec![id],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
        } else {
            None
        };
        let seq = store.task_sequence("p").unwrap();
        store
            .connection
            .execute_batch("PRAGMA foreign_keys=OFF; DELETE FROM snapshots;")
            .unwrap();
        if let Some(preview) = preview {
            assert!(
                store
                    .task_profile_update("p", &preview, &manifest, &docs, "Adopt")
                    .is_err()
            );
        } else {
            assert!(
                store
                    .task_action_with_profiles(
                        "p",
                        Action::ClaimNext {
                            actor: "a".into(),
                            filters: Filters::default()
                        },
                        Some((&manifest, &docs))
                    )
                    .is_err()
            );
        }
        assert_eq!(store.task_sequence("p").unwrap(), seq);
    }
}
#[test]
fn typed_follow_up_definition_write_failures_preserve_failed_evidence() {
    let (mut store, _, _) = prepared();
    let (manifest, docs) = profile_catalog();
    let old = store.task_session("p", None, None).unwrap();
    let id = old.next(&Filters::default(), None).unwrap().task_id;
    deliver(&mut store, id, &manifest, &docs);
    store
        .task_action(
            "p",
            Action::RecordCheck {
                task: id,
                evidence: CheckEvidence {
                    requirements_hash: old.definitions[&id].candidate.requirements_hash.clone(),
                    implementation: "revision1".into(),
                    verifier: "native-test".into(),
                    verifier_version: "1".into(),
                    command: "test".into(),
                    started_at: "2026-10-09T00:00:00Z".into(),
                    finished_at: "2026-10-09T00:00:01Z".into(),
                    report: "Failed".into(),
                    status: CheckStatus::Fail,
                },
            },
        )
        .unwrap();
    let seq = store.task_sequence("p").unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_definition BEFORE UPDATE OF data ON tasks WHEN OLD.data != '{}' BEGIN SELECT RAISE(ABORT,'disk unavailable'); END;").unwrap();
    assert!(store.task_follow_up("p", id, None, "Repair").is_err());
    let session = store.task_session("p", None, None).unwrap();
    assert_eq!(session.event_seq, seq);
    assert_eq!(session.states[&id].check, CheckStatus::Fail);
}

fn batch_inputs(
    store: &Store,
    input: &Snapshot,
) -> (dctx0_core::VerificationTaskPin, Vec<TaskDelivery>) {
    let session = store.task_session("p", None, None).unwrap();
    let (manifest, docs) = profile_catalog();
    let mut members = session
        .plan
        .members
        .iter()
        .filter(|m| matches!(m.candidate.kind, TaskKind::Contract | TaskKind::Operation))
        .collect::<Vec<_>>();
    members.sort_by_key(|m| (m.candidate.wave, m.candidate.key.clone()));
    let pin = dctx0_core::VerificationTaskPin {
        plan_id: session.plan.id,
        event_sequence: session.event_seq,
        requirements: members
            .iter()
            .map(|m| (m.task_id, m.candidate.requirements_hash.clone()))
            .collect(),
        profile_bundles: members
            .iter()
            .map(|m| {
                (
                    m.task_id,
                    select_profiles(input, &m.candidate, &manifest, &docs)
                        .bundle
                        .hash,
                )
            })
            .collect(),
    };
    let deliveries = members
        .iter()
        .map(|m| TaskDelivery {
            task: m.task_id,
            result: TaskResult {
                summary: "Explicit result delivery".into(),
                artifacts: vec!["src/app.rs".into()],
                decisions: vec![],
                implementation: Some("content-hash".into()),
            },
            evidence: CheckEvidence {
                requirements_hash: m.candidate.requirements_hash.clone(),
                implementation: "content-hash".into(),
                verifier: "test-verifier".into(),
                verifier_version: "1".into(),
                command: "test".into(),
                started_at: "1".into(),
                finished_at: "2".into(),
                report: "report-artifact".into(),
                status: CheckStatus::Pass,
            },
        })
        .collect();
    (pin, deliveries)
}

#[test]
fn atomic_result_delivery_uses_existing_guards_and_preserves_definitions() {
    let (mut store, input, _) = prepared();
    let before = store.task_session("p", None, None).unwrap();
    let (pin, deliveries) = batch_inputs(&store, &input);
    let (manifest, docs) = profile_catalog();
    let receipts = store
        .deliver_tasks_atomically("p", "verifier", &pin, &deliveries, (&manifest, &docs))
        .unwrap();
    assert_eq!(receipts.len(), deliveries.len());
    let after = store.task_session("p", None, None).unwrap();
    assert_eq!(
        serde_json::to_value(&after.definitions).unwrap(),
        serde_json::to_value(&before.definitions).unwrap()
    );
    for delivery in &deliveries {
        let state = &after.states[&delivery.task];
        assert_eq!(state.work, WorkState::Done);
        assert_eq!(state.check, CheckStatus::Pass);
        assert_eq!(state.result.as_ref(), Some(&delivery.result));
        assert_eq!(
            state.execution_bundle.as_ref().unwrap().hash,
            pin.profile_bundles[&delivery.task]
        );
        let kinds = after
            .events
            .iter()
            .filter(|e| e.task_id == Some(delivery.task) && e.seq > before.event_seq)
            .map(|e| e.kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(kinds, ["started", "completed", "check"]);
    }
    assert!(
        store
            .deliver_tasks_atomically("p", "verifier", &pin, &deliveries, (&manifest, &docs))
            .is_err()
    );
    assert_eq!(store.task_sequence("p").unwrap(), after.event_seq);
}

#[test]
fn atomic_result_delivery_rolls_back_all_prior_tasks_when_a_later_step_fails() {
    for failure in [
        "started",
        "completed",
        "check",
        "profile",
        "evidence",
        "duplicate",
        "stale",
        "order",
        "claimed",
        "new-check",
    ] {
        let (mut store, input, _) = prepared();
        let (mut pin, mut deliveries) = batch_inputs(&store, &input);
        assert!(deliveries.len() >= 2);
        let last = deliveries.last().unwrap().task;
        let (manifest, docs) = profile_catalog();
        match failure {
            "started" | "completed" | "check" => {
                store.connection.execute_batch(&format!("CREATE TRIGGER reject_batch BEFORE INSERT ON task_events WHEN NEW.task_id={last} AND NEW.kind='{failure}' BEGIN SELECT RAISE(ABORT,'write failed'); END;")).unwrap();
            }
            "profile" => {
                pin.profile_bundles.insert(last, "changed".into());
            }
            "evidence" => {
                deliveries.last_mut().unwrap().evidence.implementation = "different-code".into()
            }
            "duplicate" => deliveries.push(deliveries[0].clone()),
            "stale" => pin.event_sequence -= 1,
            "order" => deliveries.reverse(),
            "claimed" => {
                store
                    .task_action(
                        "p",
                        Action::Start {
                            task: deliveries[0].task,
                            actor: "other".into(),
                            reservation: None,
                        },
                    )
                    .unwrap();
                pin.event_sequence = store.task_sequence("p").unwrap();
            }
            "new-check" => {
                store.publish("p", &input, None).unwrap();
            }
            _ => unreachable!(),
        }
        let before = store.task_session("p", None, None).unwrap();
        let bundles: i64 = store
            .connection
            .query_row("SELECT count(*) FROM execution_bundles", [], |r| r.get(0))
            .unwrap();
        assert!(
            store
                .deliver_tasks_atomically("p", "verifier", &pin, &deliveries, (&manifest, &docs))
                .is_err(),
            "{failure}"
        );
        assert_eq!(
            serde_json::to_value(store.task_session("p", None, None).unwrap()).unwrap(),
            serde_json::to_value(before).unwrap(),
            "{failure}"
        );
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM execution_bundles", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            bundles,
            "{failure}"
        );
    }
}

// Storage-only fixture: this metadata is not proof of native execution.
// Automatic adoption must never derive authority from this construction.
fn linked_delivery_fixture(
    store: &mut Store,
    input: &Snapshot,
) -> (dctx0_core::VerificationTaskPin, Vec<TaskDelivery>, String) {
    use dctx0_core::*;
    let (pin, mut deliveries) = batch_inputs(store, input);
    let artifact = crate::VerificationArtifact::new(
        "application/vnd.dctx0.plan+json",
        b"storage-only plan".to_vec(),
    );
    let subjects = store
        .task_session("p", None, None)
        .unwrap()
        .plan
        .members
        .iter()
        .filter(|m| pin.requirements.contains_key(&m.task_id))
        .flat_map(|m| {
            m.candidate
                .subjects
                .iter()
                .chain(&m.candidate.types)
                .chain(&m.candidate.facts)
                .cloned()
        })
        .collect::<BTreeSet<_>>();
    let manifest = VerificationCaseManifest {
        schema_version: 1,
        required_cases: ["C1".into()].into(),
        case_requirements: [("C1".into(), subjects.clone())].into(),
        open_obligations: vec![],
        bounded_obligations: vec![],
    };
    let mapping = crate::VerificationArtifact::new(
        "application/vnd.dctx0.case-manifest+json",
        serde_json::to_vec(&manifest).unwrap(),
    );
    let run = VerificationRun {
        schema_version: 1,
        origin: VerificationRunOrigin::Native,
        started_at_unix_ns: "1234".into(),
        elapsed_us: 1,
        provenance: VerificationProvenance {
            snapshot: input.id.clone(),
            check_occurrence: store.task_check("p").unwrap().0,
            plan_hash: artifact.reference.hash.clone(),
            implementation: [("src/app.rs".into(), "content-hash".into())].into(),
            dependencies: BTreeMap::new(),
            adapter_hash: "adapter".into(),
            configuration_hash: "config".into(),
            harness_hash: "harness".into(),
            interpreter_version: "1".into(),
            generator_version: "1".into(),
            protocol_version: 1,
            target: "rust".into(),
            runtime_version: "storage-fixture".into(),
            build_configuration: BTreeMap::new(),
            task_pin: Some(pin.clone()),
            incomplete: vec![],
        },
        budgets: Default::default(),
        selected_subjects: subjects.clone(),
        required_subjects: subjects,
        not_selected: vec![],
        required_cases: ["C1".into()].into(),
        cases: vec![VerificationCaseRecord {
            resources: None,
            id: "C1".into(),
            subject: "W.A".into(),
            kind: VerificationCaseKind::Contract,
            bindings: BTreeMap::new(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 1,
        }],
        open_obligations: vec![],
        bounded_obligations: vec![],
        artifacts: vec![artifact.reference.clone(), mapping.reference.clone()],
        result: VerificationResult::Pass,
    };
    let id = store
        .record_verification_run("p", &run, &[artifact, mapping])
        .unwrap()
        .id;
    for delivery in &mut deliveries {
        delivery.evidence.report = id.clone();
    }
    (pin, deliveries, id)
}

#[test]
fn explicitly_linked_deliveries_publish_idempotent_receipts_and_links_atomically() {
    let (mut store, input, _) = prepared();
    let (pin, deliveries, run_id) = linked_delivery_fixture(&mut store, &input);
    let (manifest, docs) = profile_catalog();
    let publish = |store: &mut Store, deliveries: &[TaskDelivery]| {
        store.publish_task_deliveries(
            "p",
            TaskDeliveryPublication {
                actor: "explicit-publisher",
                pin: &pin,
                deliveries,
                profiles: (&manifest, &docs),
                run_id: Some(&run_id),
            },
        )
    };
    let first = publish(&mut store, &deliveries).unwrap();
    assert_eq!(
        serde_json::to_value(
            store
                .task_delivery_publication("p", &run_id)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(&first).unwrap()
    );
    assert!(
        store
            .task_delivery_publication("other", &run_id)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .task_delivery_publication("p", "missing")
            .unwrap()
            .is_none()
    );
    let seq = store.task_sequence("p").unwrap();
    assert_eq!(first.event_sequence, seq);
    let second = publish(&mut store, &deliveries).unwrap();
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(second).unwrap()
    );
    assert_eq!(store.task_sequence("p").unwrap(), seq);
    let links: i64 = store
        .connection
        .query_row(
            "SELECT count(*) FROM verification_task_links WHERE run_id=?1",
            [&run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(links as usize, deliveries.len());
    let mut different = deliveries.clone();
    different[0].result.summary = "Different publication".into();
    assert!(publish(&mut store, &different).is_err());
    assert_eq!(store.task_sequence("p").unwrap(), seq);
    let expected = store
        .task_delivery_publication("p", &run_id)
        .unwrap()
        .unwrap();
    let mut run = store.verification_run("p", &run_id).unwrap().run;
    let artifacts = run
        .artifacts
        .iter()
        .map(|a| store.verification_artifact(&run_id, &a.hash).unwrap())
        .collect::<Vec<_>>();
    for n in 0..3 {
        run.started_at_unix_ns = (10000 + n).to_string();
        store
            .record_verification_run("p", &run, &artifacts)
            .unwrap();
    }
    assert!(matches!(
        store.verification_run("p", &run_id),
        Err(Error::DetailsPruned(_))
    ));
    // This returns the original receipt; it never creates fresh proof or events.
    assert_eq!(
        serde_json::to_value(publish(&mut store, &deliveries).unwrap()).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert!(publish(&mut store, &different).is_err());
    assert_eq!(store.task_sequence("p").unwrap(), seq);
}

#[test]
fn run_link_and_receipt_failures_rollback_delivery_but_preserve_executed_history() {
    for table in ["verification_task_links", "verification_adoptions"] {
        let (mut store, input, _) = prepared();
        let (pin, deliveries, run_id) = linked_delivery_fixture(&mut store, &input);
        let (manifest, docs) = profile_catalog();
        let before = store.task_session("p", None, None).unwrap();
        let stored = store.verification_run("p", &run_id).unwrap();
        store.connection.execute_batch(&format!("CREATE TRIGGER reject_link BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'write failed'); END;")).unwrap();
        assert!(
            store
                .publish_task_deliveries(
                    "p",
                    TaskDeliveryPublication {
                        actor: "explicit-publisher",
                        pin: &pin,
                        deliveries: &deliveries,
                        profiles: (&manifest, &docs),
                        run_id: Some(&run_id)
                    }
                )
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(store.task_session("p", None, None).unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert_eq!(store.verification_run("p", &run_id).unwrap(), stored);
        let links: i64 = store
            .connection
            .query_row("SELECT count(*) FROM verification_task_links", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(links, 0);
    }
}

#[test]
fn incomplete_or_imported_runs_cannot_be_linked_by_explicit_publication() {
    for reason in [
        "imported",
        "incomplete",
        "no-pin",
        "unknown",
        "unexecuted",
        "duplicate-case",
        "empty-case",
        "scope",
        "artifact",
        "report",
        "missing-manifest",
        "duplicate-manifest",
        "corrupt-manifest",
        "malformed-manifest",
        "manual-not-in-manifest",
        "unsupported-open-obligation",
        "manual-primary-mismatch",
        "unmapped-subject",
        "manifest-reference-size",
        "manifest-invalid-version",
    ] {
        let (mut store, input, _) = prepared();
        let (pin, mut deliveries, run_id) = linked_delivery_fixture(&mut store, &input);
        let (manifest, docs) = profile_catalog();
        let mut run = store.verification_run("p", &run_id).unwrap().run;
        match reason {
            "manifest-reference-size" => {
                let reference = run
                    .artifacts
                    .iter_mut()
                    .find(|artifact| {
                        artifact.media_type == "application/vnd.dctx0.case-manifest+json"
                    })
                    .unwrap();
                reference.bytes += 1;
            }
            "manifest-invalid-version" => {
                let reference = run
                    .artifacts
                    .iter_mut()
                    .find(|artifact| {
                        artifact.media_type == "application/vnd.dctx0.case-manifest+json"
                    })
                    .unwrap();
                let stored = store
                    .verification_artifact(&run_id, &reference.hash)
                    .unwrap();
                let mut mapping: dctx0_core::VerificationCaseManifest =
                    serde_json::from_slice(&stored.contents).unwrap();
                mapping.schema_version += 1;
                let changed = crate::VerificationArtifact::new(
                    &reference.media_type,
                    serde_json::to_vec(&mapping).unwrap(),
                );
                store.connection.execute("INSERT INTO verification_artifacts(hash,media_type,bytes) VALUES(?1,?2,?3)", params![changed.reference.hash, changed.reference.media_type, changed.contents]).unwrap();
                store.connection.execute("INSERT INTO verification_run_artifacts(run_id,artifact_hash) VALUES(?1,?2)", params![run_id, changed.reference.hash]).unwrap();
                *reference = changed.reference;
            }
            "unmapped-subject" => {
                let reference = run
                    .artifacts
                    .iter_mut()
                    .find(|artifact| {
                        artifact.media_type == "application/vnd.dctx0.case-manifest+json"
                    })
                    .unwrap();
                let stored = store
                    .verification_artifact(&run_id, &reference.hash)
                    .unwrap();
                let mut mapping: dctx0_core::VerificationCaseManifest =
                    serde_json::from_slice(&stored.contents).unwrap();
                mapping
                    .case_requirements
                    .values_mut()
                    .for_each(|requirements| {
                        requirements.remove("W.A");
                    });
                let changed = crate::VerificationArtifact::new(
                    &reference.media_type,
                    serde_json::to_vec(&mapping).unwrap(),
                );
                store.connection.execute("INSERT INTO verification_artifacts(hash,media_type,bytes) VALUES(?1,?2,?3)", params![changed.reference.hash, changed.reference.media_type, changed.contents]).unwrap();
                store.connection.execute("INSERT INTO verification_run_artifacts(run_id,artifact_hash) VALUES(?1,?2)", params![run_id, changed.reference.hash]).unwrap();
                *reference = changed.reference;
            }
            "manual-not-in-manifest"
            | "unsupported-open-obligation"
            | "manual-primary-mismatch" => {
                let issue = dctx0_core::VerificationIssue {
                    code: if reason == "unsupported-open-obligation" {
                        "STATE_OBSERVER_REQUIRED"
                    } else {
                        "MANUAL_REQUIREMENT"
                    }
                    .into(),
                    message: "A stored obligation remains open".into(),
                    span: None,
                };
                run.open_obligations.push(issue.clone());
                run.result = dctx0_core::VerificationResult::Unknown {
                    issue: if reason == "manual-primary-mismatch" {
                        dctx0_core::VerificationIssue {
                            message: "Not present in open obligations".into(),
                            ..issue
                        }
                    } else {
                        issue
                    },
                };
            }
            "imported" => run.origin = dctx0_core::VerificationRunOrigin::Imported,
            "incomplete" => run
                .provenance
                .incomplete
                .push(dctx0_core::VerificationIssue {
                    code: "UNCAPTURED_DEPENDENCY".into(),
                    message: "An input is unknown".into(),
                    span: None,
                }),
            "no-pin" => run.provenance.task_pin = None,
            "unknown" => {
                run.cases[0].result = dctx0_core::VerificationResult::Unknown {
                    issue: dctx0_core::VerificationIssue {
                        code: "UNEXECUTED".into(),
                        message: "Missing actual execution".into(),
                        span: None,
                    },
                }
            }
            "unexecuted" => {
                run.required_cases.insert("missing-case".into());
            }
            "duplicate-case" => run.cases.push(run.cases[0].clone()),
            "empty-case" => {
                let old = run.cases[0].id.clone();
                run.required_cases.remove(&old);
                run.required_cases.insert(String::new());
                run.cases[0].id.clear();
            }
            "scope" => {
                run.required_subjects.clear();
            }
            "artifact" => {
                deliveries.last_mut().unwrap().result.artifacts = vec!["uncaptured.rs".into()]
            }
            "report" => deliveries.last_mut().unwrap().evidence.report = "different-run".into(),
            "missing-manifest" => run.artifacts.retain(|artifact| {
                artifact.media_type != "application/vnd.dctx0.case-manifest+json"
            }),
            "duplicate-manifest" => {
                let reference = run
                    .artifacts
                    .iter()
                    .find(|artifact| {
                        artifact.media_type == "application/vnd.dctx0.case-manifest+json"
                    })
                    .unwrap()
                    .clone();
                run.artifacts.push(reference);
            }
            "malformed-manifest" => {
                let reference = run
                    .artifacts
                    .iter_mut()
                    .find(|artifact| {
                        artifact.media_type == "application/vnd.dctx0.case-manifest+json"
                    })
                    .unwrap();
                let artifact =
                    crate::VerificationArtifact::new(&reference.media_type, b"{invalid".to_vec());
                store.connection.execute("INSERT INTO verification_artifacts(hash,media_type,bytes) VALUES(?1,?2,?3)", params![artifact.reference.hash,artifact.reference.media_type,artifact.contents]).unwrap();
                store.connection.execute("UPDATE verification_run_artifacts SET artifact_hash=?1 WHERE run_id=?2 AND artifact_hash=?3", params![artifact.reference.hash,run_id,reference.hash]).unwrap();
                *reference = artifact.reference;
            }
            "corrupt-manifest" => {
                let reference = run
                    .artifacts
                    .iter()
                    .find(|artifact| {
                        artifact.media_type == "application/vnd.dctx0.case-manifest+json"
                    })
                    .unwrap();
                store
                    .connection
                    .execute(
                        "UPDATE verification_artifacts SET bytes=?1 WHERE hash=?2",
                        params![b"{}".as_slice(), reference.hash],
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        store
            .connection
            .execute(
                "DELETE FROM verification_case_projections WHERE run_id=?1",
                [&run_id],
            )
            .unwrap();
        // Simulate a corrupt/legacy imported storage row to exercise rejection
        // independently of the normal immutable recording validations.
        store
            .connection
            .execute(
                "UPDATE verification_runs SET data=?1,corpus_hash=NULL WHERE id=?2",
                params![serde_json::to_string(&run).unwrap(), run_id],
            )
            .unwrap();
        let before = store.task_session("p", None, None).unwrap();
        let rejected = store
            .publish_task_deliveries(
                "p",
                TaskDeliveryPublication {
                    actor: "explicit-publisher",
                    pin: &pin,
                    deliveries: &deliveries,
                    profiles: (&manifest, &docs),
                    run_id: Some(&run_id),
                },
            )
            .unwrap_err();
        if reason == "manifest-reference-size" {
            assert!(
                rejected
                    .to_string()
                    .contains("Mandatory case manifest artifact integrity"),
                "{rejected}"
            );
        }
        if reason == "manifest-invalid-version" {
            assert!(
                rejected
                    .to_string()
                    .contains("Stored mandatory case manifest does not match"),
                "{rejected}"
            );
        }
        if reason == "malformed-manifest" {
            assert!(
                rejected
                    .to_string()
                    .contains("Invalid mandatory case manifest"),
                "{rejected}"
            );
        }
        assert_eq!(
            serde_json::to_value(store.task_session("p", None, None).unwrap()).unwrap(),
            serde_json::to_value(before).unwrap(),
            "{reason}"
        );
        assert!(
            store
                .task_delivery_publication("p", &run_id)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn discharged_state_hints_are_recomputed_from_immutable_plan_and_actual_before_states() {
    use crate::VerificationArtifact;
    use dctx0_core::*;
    let (mut store, old, _) = prepared();
    let (_, _, old_run) = linked_delivery_fixture(&mut store, &old);
    let mut run = store.verification_run("p", &old_run).unwrap().run;
    let sources = vec![Source {
        file: "state.dctx0".into(),
        text: "workspace W { entity Item { id: String\nkey id } surface Api { operation read(id:String) -> String { requires exists Item(id) } } review R { inspect { implementation } criteria { \"Human review remains open\" } } }".into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let input = dctx0_changes::snapshot(&sources, ir.clone(), dctx0_graph::build(&ir));
    store.publish("p", &input, None).unwrap();
    let occurrence = store.task_check("p").unwrap().0;
    let plan = dctx0_verification::project_program(
        &ir,
        &input.id,
        occurrence as u64,
        dctx0_verification::VerificationScope {
            subjects: ["W".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let operation = &plan.operations[0];
    run.cases = operation
        .inputs
        .bindings
        .iter()
        .enumerate()
        .map(|(index, bindings)| {
            let invocation = Invocation {
                subject: operation.operation.id.clone(),
                arguments: vec![bindings["id"].clone()],
            };
            VerificationCaseRecord {
                id: format!("W.Api.read:operation:{index}"),
                subject: "W.Api.read".into(),
                kind: VerificationCaseKind::Operation,
                bindings: bindings.clone(),
                setup: vec![],
                arguments: vec![invocation.arguments.clone()],
                observations: vec![InvocationObservation {
                    invocation,
                    observation: Observation::Returned {
                        value: bindings["id"].clone(),
                    },
                }],
                events: None,
                resources: None,
                states: vec![VerificationStateEvidence {
                    phase: StatePhase::Before,
                    phase_recorded: Some(true),
                    entities: [(
                        "W.Item".into(),
                        vec![VerificationValue::Record(
                            [("id".into(), bindings["id"].clone())].into(),
                        )],
                    )]
                    .into(),
                }],
                result: VerificationResult::Pass,
                elapsed_us: 0,
            }
        })
        .collect();
    let manifest = dctx0_verification::case_manifest(&plan);
    let resolved = dctx0_verification::resolved_state_preconditions(&ir, &plan, &run.cases);
    assert!(!resolved.is_empty());
    run.provenance.snapshot = input.id.clone();
    run.provenance.check_occurrence = occurrence;
    run.provenance.task_pin = None;
    run.required_cases = manifest.required_cases.clone();
    run.selected_subjects = plan.scope.subjects.clone();
    run.required_subjects = plan.closure.clone();
    run.open_obligations = manifest
        .open_obligations
        .iter()
        .filter(|issue| !resolved.contains(issue))
        .cloned()
        .collect();
    run.result = VerificationResult::Unknown {
        issue: run.open_obligations[0].clone(),
    };
    let artifacts = vec![
        VerificationArtifact::new(
            "application/vnd.dctx0.plan+json",
            serde_json::to_vec(&plan).unwrap(),
        ),
        VerificationArtifact::new(
            "application/vnd.dctx0.case-manifest+json",
            serde_json::to_vec(&manifest).unwrap(),
        ),
        VerificationArtifact::new(
            "application/vnd.dctx0.resolved-preconditions+json",
            serde_json::to_vec(&resolved).unwrap(),
        ),
    ];
    run.provenance.plan_hash = artifacts[0].reference.hash.clone();
    run.artifacts = artifacts
        .iter()
        .map(|artifact| artifact.reference.clone())
        .collect();
    // This is a partial storage fixture, not execution authority: the outer
    // delivery gate separately rejects its missing contract cases.
    let saved = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap();
    super::native_proof::validate_open_obligations(&store.connection, &saved.id, &run, &manifest)
        .unwrap();
    // Incremental ancestors use this same nonrecursive check after their own
    // native cohort and identity proof has been validated.
    super::native_proof::validate_remaining_obligations(
        &store.connection,
        &saved.id,
        &run,
        &manifest,
    )
    .unwrap();
    let mut missing_before = run.clone();
    missing_before.cases[0].states.clear();
    assert!(
        super::native_proof::validate_remaining_obligations(
            &store.connection,
            &saved.id,
            &missing_before,
            &manifest
        )
        .is_err()
    );
    for mutation in 0..8 {
        let mut invalid = run.clone();
        match mutation {
            0 => invalid.cases[0].states.clear(),
            1 => invalid.cases[0].states[0].phase_recorded = None,
            2 => invalid.provenance.plan_hash = "different-plan".into(),
            3 => invalid.budgets.seed += 1,
            4 => invalid.artifacts.retain(|artifact| {
                artifact.media_type != "application/vnd.dctx0.resolved-preconditions+json"
            }),
            5 => invalid.artifacts.push(artifacts[0].reference.clone()),
            6 => invalid.open_obligations.clear(),
            _ => invalid.cases.clear(),
        }
        assert!(
            super::native_proof::validate_open_obligations(
                &store.connection,
                &saved.id,
                &invalid,
                &manifest
            )
            .is_err(),
            "Mutation {mutation}"
        );
    }
    // A valid content hash does not make a malformed typed proof valid.
    for (media_type, contents) in [
        ("application/vnd.dctx0.plan+json", b"not JSON".as_slice()),
        ("application/vnd.dctx0.plan+json", b"{}".as_slice()),
        (
            "application/vnd.dctx0.resolved-preconditions+json",
            b"{\"invalid\":true}".as_slice(),
        ),
    ] {
        let mut invalid = run.clone();
        let mut altered = artifacts.clone();
        let index = altered
            .iter()
            .position(|artifact| artifact.reference.media_type == media_type)
            .unwrap();
        altered[index] = VerificationArtifact::new(media_type, contents.to_vec());
        invalid.artifacts = altered
            .iter()
            .map(|artifact| artifact.reference.clone())
            .collect();
        invalid.provenance.plan_hash = altered[0].reference.hash.clone();
        let saved_invalid = store
            .record_verification_run("p", &invalid, &altered)
            .unwrap();
        let error = super::native_proof::validate_open_obligations(
            &store.connection,
            &saved_invalid.id,
            &invalid,
            &manifest,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("Invalid native proof artifact"),
            "{error}"
        );
    }
    let original_snapshot: String = store
        .connection
        .query_row(
            "SELECT data FROM snapshots WHERE id=?1",
            [&input.id],
            |row| row.get(0),
        )
        .unwrap();
    store
        .connection
        .execute("UPDATE snapshots SET data='{}' WHERE id=?1", [&input.id])
        .unwrap();
    assert!(
        super::native_proof::validate_open_obligations(
            &store.connection,
            &saved.id,
            &run,
            &manifest
        )
        .is_err()
    );
    store
        .connection
        .execute(
            "UPDATE snapshots SET data=?1 WHERE id=?2",
            params![original_snapshot, input.id],
        )
        .unwrap();
    for media_type in [
        "application/vnd.dctx0.plan+json",
        "application/vnd.dctx0.resolved-preconditions+json",
    ] {
        let artifact = artifacts
            .iter()
            .find(|artifact| artifact.reference.media_type == media_type)
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE verification_artifacts SET bytes=?1 WHERE hash=?2",
                params![b"{}".as_slice(), artifact.reference.hash],
            )
            .unwrap();
        assert!(
            super::native_proof::validate_open_obligations(
                &store.connection,
                &saved.id,
                &run,
                &manifest
            )
            .is_err()
        );
        store
            .connection
            .execute(
                "UPDATE verification_artifacts SET bytes=?1 WHERE hash=?2",
                params![artifact.contents, artifact.reference.hash],
            )
            .unwrap();
    }
}

#[test]
fn malformed_application_receipts_cannot_publish_tasks_or_create_delivery_links() {
    for bytes in [b"null".as_slice(), b"{}", br#"{"schema_version":99}"#] {
        let (mut store, input, _) = prepared();
        let (pin, mut deliveries, run_id) = linked_delivery_fixture(&mut store, &input);
        let saved = store.verification_run("p", &run_id).unwrap();
        let mut run = saved.run.clone();
        let mut artifacts = run
            .artifacts
            .iter()
            .map(|reference| {
                store
                    .verification_artifact(&run_id, &reference.hash)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let forged = crate::VerificationArtifact::new(
            dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE,
            bytes.to_vec(),
        );
        run.artifacts.push(forged.reference.clone());
        artifacts.push(forged);
        let forged_run = store
            .record_verification_run("p", &run, &artifacts)
            .unwrap();
        for delivery in &mut deliveries {
            delivery.evidence.report = forged_run.id.clone();
        }
        let before = store.task_session("p", None, None).unwrap();
        let (manifest, docs) = profile_catalog();
        assert!(
            store
                .publish_task_deliveries(
                    "p",
                    TaskDeliveryPublication {
                        actor: "native-publisher",
                        pin: &pin,
                        deliveries: &deliveries,
                        profiles: (&manifest, &docs),
                        run_id: Some(&forged_run.id),
                    }
                )
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(store.task_session("p", None, None).unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert!(
            store
                .task_delivery_publication("p", &forged_run.id)
                .unwrap()
                .is_none()
        );
        let historical = store.verification_run_summary("p", &run_id).unwrap();
        assert_eq!(historical.result, saved.run.result);
        assert_eq!(historical.case_count, saved.run.cases.len());
    }
}

#[test]
fn native_batch_preserves_completed_result_and_commits_open_and_done_together() {
    let (mut store, input, _) = prepared();
    let (_, initial) = batch_inputs(&store, &input);
    let id = initial[0].task;
    let started = store
        .task_action(
            "p",
            Action::Start {
                task: id,
                actor: "author".into(),
                reservation: None,
            },
        )
        .unwrap()
        .state
        .unwrap();
    store
        .task_action(
            "p",
            Action::Complete {
                task: id,
                actor: "author".into(),
                claim: started.claim.unwrap(),
                version: started.version,
                result: TaskResult {
                    summary: "Historical result".into(),
                    artifacts: vec![],
                    decisions: vec!["Preserve".into()],
                    implementation: None,
                },
            },
        )
        .unwrap();
    // Seed a historical review to verify that arbitrary existing review content survives.
    let mut historical = store.task_session("p", None, None).unwrap().states[&id].clone();
    historical.reviews.push(dctx0_tasks::ReviewReport {
        reviewer: "historical reviewer".into(),
        judgement: "approved".into(),
        summary: "Retained review".into(),
        findings: vec![],
        conformance: None,
    });
    super::read::event(
        &store.connection,
        "p",
        Some(id),
        "legacy-review-fixture",
        "Existing review",
        Some(&historical),
    )
    .unwrap();
    let before = store.task_session("p", None, None).unwrap();
    let (pin, deliveries, run_id) = linked_delivery_fixture(&mut store, &input);
    let open = deliveries
        .iter()
        .filter(|d| d.task != id)
        .take(1)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(open.len(), 1);
    let run = store.verification_run("p", &run_id).unwrap().run;
    let identities: BTreeMap<&String, &String> = BTreeMap::new();
    let mut open = open;
    open[0].result.implementation = Some(dctx0_changes::hash(&(&run.provenance, identities)));
    let done = [(id, before.states[&id].version)].into();
    let (manifest, docs) = profile_catalog();
    let publish = |store: &mut Store, versions: &BTreeMap<i64, i64>| {
        store.publish_native_task_batch(
            "p",
            TaskDeliveryPublication {
                actor: "verifier",
                pin: &pin,
                deliveries: &open,
                profiles: (&manifest, &docs),
                run_id: Some(&run_id),
            },
            versions,
        )
    };
    let bad = [(id, before.states[&id].version + 1)].into();
    assert!(publish(&mut store, &bad).is_err());
    assert_eq!(store.task_sequence("p").unwrap(), before.event_seq);
    let receipt = publish(&mut store, &done).unwrap();
    let after = store.task_session("p", None, None).unwrap();
    assert_eq!(
        serde_json::to_vec(&before.states[&id].result).unwrap(),
        serde_json::to_vec(&after.states[&id].result).unwrap()
    );
    assert_eq!(before.states[&id].reviews, after.states[&id].reviews);
    assert_eq!(
        serde_json::to_vec(&before.definitions).unwrap(),
        serde_json::to_vec(&after.definitions).unwrap()
    );
    assert_eq!(after.states[&id].work, WorkState::Done);
    assert_eq!(after.states[&id].check, CheckStatus::Pass);
    assert_ne!(
        after.states[&id].evidence.last().unwrap().implementation,
        ""
    );
    assert_eq!(after.states[&open[0].task].check, CheckStatus::Pass);
    assert_eq!(
        serde_json::to_vec(&receipt).unwrap(),
        serde_json::to_vec(&publish(&mut store, &done).unwrap()).unwrap()
    );
    assert!(publish(&mut store, &bad).is_err());
    assert_eq!(store.task_sequence("p").unwrap(), receipt.event_sequence);
    let links: i64 = store
        .connection
        .query_row(
            "SELECT count(*) FROM verification_task_links WHERE run_id=?1",
            [&run_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(links, 2);
    let file = std::env::temp_dir().join(format!(
        "dctx0-native-reopen-{}-{}.sqlite3",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    store
        .connection
        .execute("VACUUM INTO ?1", [file.to_str().unwrap()])
        .unwrap();
    drop(store);
    let reader = Store::open(&file, AccessMode::ReadOnly).unwrap();
    let reopened = reader.task_session("p", None, None).unwrap();
    assert_eq!(
        serde_json::to_vec(&after.states[&id]).unwrap(),
        serde_json::to_vec(&reopened.states[&id]).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&before.states[&id].result).unwrap(),
        serde_json::to_vec(&reopened.states[&id].result).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&before.states[&id].reviews).unwrap(),
        serde_json::to_vec(&reopened.states[&id].reviews).unwrap()
    );
    assert!(
        reopened
            .events
            .iter()
            .any(|event| event.task_id == Some(id) && event.kind == "native-check")
    );
    let old = reader
        .task_session("p", Some(pin.plan_id), Some(pin.event_sequence))
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&before.states[&id]).unwrap(),
        serde_json::to_vec(&old.states[&id]).unwrap()
    );
    let metadata = reader.task_metadata_session("p", None, None).unwrap();
    assert_eq!(metadata.states[&id].work, WorkState::Done);
    assert_eq!(metadata.states[&id].check, CheckStatus::Pass);
    let references = reader
        .task_evidence_references("p", reopened.event_seq)
        .unwrap()
        .unwrap();
    let reference = references
        .iter()
        .find(|reference| reference.task_id == id)
        .unwrap();
    assert_eq!(
        reference.implementation,
        reopened.states[&id].evidence.last().unwrap().implementation
    );
    assert_eq!(reference.requirements_hash, pin.requirements[&id]);
    assert_eq!(reference.status, "PASS");
    assert_eq!(reference.verifier, "dctx0-native-verifier");
    let cached: String = reader
        .connection
        .query_row(
            "SELECT data FROM task_states WHERE task_id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    let cached: State = storage::load(&reader.connection, &cached).unwrap();
    assert_eq!(
        serde_json::to_vec(&reopened.states[&id]).unwrap(),
        serde_json::to_vec(&cached).unwrap()
    );
    drop(reader);
    fs::remove_file(file).unwrap();
}

#[test]
fn native_done_batch_rejects_untrusted_and_incomplete_runs_without_mutation() {
    for failure in 0..7 {
        let (mut store, input, _) = prepared();
        let (_, deliveries) = batch_inputs(&store, &input);
        let id = deliveries[0].task;
        let started = store
            .task_action(
                "p",
                Action::Start {
                    task: id,
                    actor: "author".into(),
                    reservation: None,
                },
            )
            .unwrap()
            .state
            .unwrap();
        store
            .task_action(
                "p",
                Action::Complete {
                    task: id,
                    actor: "author".into(),
                    claim: started.claim.unwrap(),
                    version: started.version,
                    result: TaskResult {
                        summary: "Original".into(),
                        artifacts: vec![],
                        decisions: vec![],
                        implementation: None,
                    },
                },
            )
            .unwrap();
        let before = store.task_session("p", None, None).unwrap();
        let (pin, _, run_id) = linked_delivery_fixture(&mut store, &input);
        let mut run = store.verification_run("p", &run_id).unwrap().run;
        let artifacts = run
            .artifacts
            .iter()
            .map(|a| store.verification_artifact(&run_id, &a.hash).unwrap())
            .collect::<Vec<_>>();
        match failure {
            0 => run.origin = dctx0_core::VerificationRunOrigin::Imported,
            1 => run.cases.clear(),
            2 => run.cases.push(run.cases[0].clone()),
            3 => run.provenance.implementation.clear(),
            4 => run.provenance.task_pin.as_mut().unwrap().event_sequence += 1,
            5 | 6 => {
                let issue = dctx0_core::VerificationIssue {
                    code: "TEST_FAILURE".into(),
                    message: "failed".into(),
                    span: None,
                };
                let result = if failure == 5 {
                    dctx0_core::VerificationResult::Fail { issue }
                } else {
                    dctx0_core::VerificationResult::Unknown { issue }
                };
                run.result = result.clone();
                run.cases[0].result = result;
            }
            _ => unreachable!(),
        }
        let invalid_run = match store.record_verification_run("p", &run, &artifacts) {
            Ok(saved) => saved.id,
            Err(_) => {
                assert_eq!(
                    serde_json::to_vec(&before).unwrap(),
                    serde_json::to_vec(&store.task_session("p", None, None).unwrap()).unwrap()
                );
                continue;
            }
        };
        let (manifest, docs) = profile_catalog();
        let versions = [(id, before.states[&id].version)].into();
        assert!(
            store
                .publish_native_task_batch(
                    "p",
                    TaskDeliveryPublication {
                        actor: "verifier",
                        pin: &pin,
                        deliveries: &[],
                        profiles: (&manifest, &docs),
                        run_id: Some(&invalid_run)
                    },
                    &versions
                )
                .is_err(),
            "failure {failure}"
        );
        assert_eq!(
            serde_json::to_vec(&before).unwrap(),
            serde_json::to_vec(&store.task_session("p", None, None).unwrap()).unwrap(),
            "failure {failure}"
        );
        assert!(
            store
                .task_delivery_publication("p", &invalid_run)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn native_batch_rejects_unrepresentable_proof_timing_without_mutating_history() {
    for (started_at, elapsed_us, interpreter, message) in [
        ("invalid", 1, "1", "Invalid native run timing"),
        (
            "340282366920938463463374607431768211455",
            1,
            "1",
            "Invalid native run timing",
        ),
        ("1", 1, " ", "Native run has no interpreter version"),
    ] {
        let (mut store, input, _) = prepared();
        let (_, deliveries) = batch_inputs(&store, &input);
        let task = deliveries[0].task;
        let started = store
            .task_action(
                "p",
                Action::Start {
                    task,
                    actor: "author".into(),
                    reservation: None,
                },
            )
            .unwrap()
            .state
            .unwrap();
        store
            .task_action(
                "p",
                Action::Complete {
                    task,
                    actor: "author".into(),
                    claim: started.claim.unwrap(),
                    version: started.version,
                    result: TaskResult {
                        summary: "Historical implementation".into(),
                        artifacts: vec![],
                        decisions: vec![],
                        implementation: None,
                    },
                },
            )
            .unwrap();
        let (pin, _, run_id) = linked_delivery_fixture(&mut store, &input);
        let mut run = store.verification_run("p", &run_id).unwrap().run;
        // Simulate a legacy/corrupt saved row: normal recording remains immutable.
        run.started_at_unix_ns = started_at.into();
        run.elapsed_us = elapsed_us;
        run.provenance.interpreter_version = interpreter.into();
        store
            .connection
            .execute(
                "UPDATE verification_runs SET data=?1,corpus_hash=NULL WHERE id=?2",
                params![serde_json::to_string(&run).unwrap(), run_id],
            )
            .unwrap();
        store
            .connection
            .execute(
                "DELETE FROM verification_case_projections WHERE run_id=?1",
                [&run_id],
            )
            .unwrap();
        let before = store.task_session("p", None, None).unwrap();
        let original = store.verification_run("p", &run_id).unwrap();
        let (manifest, docs) = profile_catalog();
        let error = store
            .publish_native_task_batch(
                "p",
                TaskDeliveryPublication {
                    actor: "verifier",
                    pin: &pin,
                    deliveries: &[],
                    profiles: (&manifest, &docs),
                    run_id: Some(&run_id),
                },
                &[(task, before.states[&task].version)].into(),
            )
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert_eq!(
            serde_json::to_vec(&before).unwrap(),
            serde_json::to_vec(&store.task_session("p", None, None).unwrap()).unwrap()
        );
        assert_eq!(store.verification_run("p", &run_id).unwrap(), original);
        assert!(
            store
                .task_delivery_publication("p", &run_id)
                .unwrap()
                .is_none()
        );
        let links: i64 = store
            .connection
            .query_row(
                "SELECT count(*) FROM verification_task_links WHERE run_id=?1",
                [&run_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 0);
    }
}

#[test]
fn manual_and_native_publication_modes_reject_mixed_or_missing_authority() {
    let (mut store, input, _) = prepared();
    let (pin, deliveries, run_id) = linked_delivery_fixture(&mut store, &input);
    let before = store.task_session("p", None, None).unwrap();
    let run = store.verification_run("p", &run_id).unwrap();
    let (manifest, docs) = profile_catalog();
    let request = |run_id| TaskDeliveryPublication {
        actor: "reviewer",
        pin: &pin,
        deliveries: &deliveries,
        profiles: (&manifest, &docs),
        run_id,
    };
    assert!(
        store
            .publish_inspected_task_deliveries("p", request(Some(&run_id)), &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("separate from automatic native adoption")
    );
    let versions = [(
        deliveries[0].task,
        before.states[&deliveries[0].task].version,
    )]
    .into();
    for inspected in [false, true] {
        for (run_id, versions) in [(None, &versions), (Some(run_id.as_str()), &BTreeMap::new())] {
            let result = if inspected {
                store.publish_inspected_native_task_batch("p", request(run_id), versions)
            } else {
                store.publish_native_task_batch("p", request(run_id), versions)
            };
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("requires a run and completed tasks")
            );
        }
    }
    assert_eq!(
        serde_json::to_vec(&before).unwrap(),
        serde_json::to_vec(&store.task_session("p", None, None).unwrap()).unwrap()
    );
    assert_eq!(store.verification_run("p", &run_id).unwrap(), run);
    assert!(
        store
            .task_delivery_publication("p", &run_id)
            .unwrap()
            .is_none()
    );
    let links: i64 = store
        .connection
        .query_row(
            "SELECT count(*) FROM verification_task_links WHERE run_id=?1",
            [&run_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(links, 0);
}
