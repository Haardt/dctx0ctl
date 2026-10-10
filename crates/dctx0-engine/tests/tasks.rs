use dctx0_engine::{AccessMode, Project};
use dctx0_store::Store;
use dctx0_tasks::*;
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
            "dctx0-tasks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
    fn write(&self, text: &str) {
        fs::write(self.0.join("spec.dctx0"), text).unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
const SPEC: &str = r#"workspace W { domain D { value Id = Int { range 1..10 } type Item { id: Id } module M { intent "First"; owns { Item } port Repo { operation get(id: Id) -> Item } surface Api { operation get(id: Id) -> Item; effects { call Repo.get } operation other() -> Int { pure } } example E { given Repo.get(sample) returns Item { id = sample }; when result = Api.get(sample); expect Success } review R { inspect { contracts }; criteria { "Respect boundaries" }; question Q { """Clear?""" } } } } }"#;
fn setup() -> (Temp, Project, Plan) {
    let temp = Temp::new();
    temp.write(SPEC);
    let p = temp.project();
    assert!(p.check().unwrap().ok);
    let plan = p.task_plan("latest").unwrap();
    (temp, p, plan)
}
fn result() -> TaskResult {
    TaskResult {
        summary: "Implemented with focused tests".into(),
        artifacts: vec!["src/example.rs".into()],
        decisions: vec!["Keep adapter boundary".into()],
        implementation: Some("revision1".into()),
    }
}
fn complete(p: &Project, id: i64) {
    let r = p
        .task_action(Action::Start {
            task: id,
            actor: "worker".into(),
            reservation: None,
        })
        .unwrap();
    let s = r.state.unwrap();
    p.task_action(Action::Complete {
        task: id,
        actor: "worker".into(),
        claim: s.claim.unwrap(),
        version: s.version,
        result: result(),
    })
    .unwrap();
}
fn deliver_prerequisites(p: &Project, anchor: i64) {
    loop {
        let s = p.task_session(None, None).unwrap();
        if s.overviews()
            .iter()
            .find(|r| r.task_id == anchor)
            .unwrap()
            .ready
        {
            break;
        }
        let ready = s.next(&Filters::default(), None).unwrap();
        assert_ne!(ready.task_id, anchor);
        complete(p, ready.task_id);
    }
}
fn evidence(p: &Project, id: i64, status: CheckStatus) -> CheckEvidence {
    let s = p.task_show_session(id, None, None).unwrap();
    CheckEvidence {
        requirements_hash: s.definitions[&id].candidate.requirements_hash.clone(),
        implementation: "revision1".into(),
        verifier: "cargo-test".into(),
        verifier_version: "1".into(),
        command: "cargo test".into(),
        started_at: "2026-10-09T10:00:00Z".into(),
        finished_at: "2026-10-09T10:01:00Z".into(),
        report: "Executed test report".into(),
        status,
    }
}
#[test]
fn plan_queries_are_read_only_idempotent_and_keep_valid_heads() {
    let temp = Temp::new();
    let p = temp.project();
    assert!(p.task_preview("latest").is_err());
    assert!(!temp.0.join(".dctx0ctl").exists());
    assert!(p.task_plan("latest").is_err());
    assert!(!temp.0.join(".dctx0ctl").exists());
    temp.write(SPEC);
    assert!(p.check().unwrap().ok);
    let before = fs::read(&p.database).unwrap();
    let preview = p.task_preview("latest").unwrap();
    assert!(!preview.tasks.is_empty());
    assert_eq!(before, fs::read(&p.database).unwrap());
    let plan = p.task_plan("latest").unwrap();
    let seq = p.task_session(None, None).unwrap().event_seq;
    assert_eq!(p.task_plan("latest").unwrap().id, plan.id);
    assert_eq!(p.task_session(None, None).unwrap().event_seq, seq);
    assert!(p.task_session(Some(9999), None).is_err());
    assert!(p.task_session(None, Some(-1)).is_err());
    assert!(p.task_session(None, Some(i64::MAX)).is_err());
    assert!(p.task_show_session(-1, None, None).is_err());
    assert!(p.task_show_session(-1, Some(plan.id), None).is_err());
    temp.write("workspace W { type Missing = Unknown }");
    assert!(!p.check().unwrap().ok);
    assert_eq!(p.task_plan("latest").unwrap().id, plan.id);
    let store = Store::open(&p.database, AccessMode::ReadOnly).unwrap();
    let mut reader = store;
    assert!(
        reader
            .task_action(
                &p.key(),
                Action::Note {
                    task: plan.members[0].task_id,
                    note: "No writes".into()
                }
            )
            .is_err()
    );
}
#[test]
fn changes_preserve_history_rename_occurrences_and_reversions() {
    let (temp, p, first) = setup();
    let id = first
        .members
        .iter()
        .find(|m| m.candidate.key == "contract:W.D.Id")
        .unwrap()
        .task_id;
    complete(&p, id);
    let pass = evidence(&p, id, CheckStatus::Pass);
    p.task_action(Action::RecordCheck {
        task: id,
        evidence: pass,
    })
    .unwrap();
    temp.write(&SPEC.replace("range 1..10", "range 1..20"));
    p.check().unwrap();
    let second = p.task_plan("latest").unwrap();
    assert_ne!(first.id, second.id);
    let replacement = second
        .members
        .iter()
        .find(|m| m.candidate.key == "contract:W.D.Id")
        .unwrap()
        .task_id;
    assert_ne!(replacement, id);
    let historical = p.task_show_session(id, None, None).unwrap();
    assert_eq!(historical.states[&id].work, WorkState::Done);
    assert_eq!(historical.states[&id].check, CheckStatus::Pass);
    temp.write(SPEC);
    p.check().unwrap();
    let reverted = p.task_plan("latest").unwrap();
    assert_ne!(
        reverted
            .members
            .iter()
            .find(|m| m.candidate.key == "contract:W.D.Id")
            .unwrap()
            .task_id,
        id
    );
    assert_eq!(first.snapshot_id, reverted.snapshot_id);
    assert_ne!(first.check_run_id, reverted.check_run_id);
    assert_eq!(p.task_plan("latest").unwrap().id, reverted.id);
    p.rename("W.D.Id", "Identifier", false).unwrap();
    let renamed = p.task_plan("latest").unwrap();
    let current = p.task_session(None, None).unwrap();
    assert!(current.links.iter().any(|l| l.kind == "rename"));
    assert!(
        renamed
            .members
            .iter()
            .any(|m| m.candidate.subjects.contains(&"W.D.Identifier".into()))
    );
    assert!(p.task_plan(&second.snapshot_id).is_err());
    assert!(p.task_preview(&second.snapshot_id).is_ok());
    let store = Store::open(&p.database, AccessMode::ReadOnly).unwrap();
    assert!(store.task_continuity(&p.key(), 100000, 100001).is_err());
    assert!(store.task_continuity(&p.key(), 100, 1).is_err());
}
#[test]
fn deletion_of_started_work_creates_retirement_and_conflicting_actors_block_it() {
    let (temp, p, first) = setup();
    let id = first
        .members
        .iter()
        .find(|m| m.candidate.key == "implementation:W.D.M.Api.other")
        .unwrap()
        .task_id;
    deliver_prerequisites(&p, id);
    let r = p
        .task_action(Action::Start {
            task: id,
            actor: "old".into(),
            reservation: None,
        })
        .unwrap();
    let claimed = r.state.unwrap();
    temp.write(&SPEC.replace("operation other() -> Int { pure }", ""));
    p.check().unwrap();
    let next = p.task_plan("latest").unwrap();
    let retire = next
        .members
        .iter()
        .find(|m| m.candidate.key == "retire:implementation:W.D.M.Api.other")
        .unwrap()
        .task_id;
    assert_eq!(p.task_plan("latest").unwrap().id, next.id);
    let s = p.task_session(None, None).unwrap();
    assert!(
        !s.overviews()
            .iter()
            .find(|r| r.task_id == retire)
            .unwrap()
            .ready
    );
    p.task_action(Action::Release {
        task: id,
        actor: "old".into(),
        claim: claimed.claim.unwrap(),
        version: claimed.version,
        reason: "Stop superseded work".into(),
    })
    .unwrap();
    let s = p.task_session(None, None).unwrap();
    assert!(
        !s.overviews()
            .iter()
            .find(|r| r.task_id == retire)
            .unwrap()
            .blockers
            .iter()
            .any(|b| b.contains("superseded work"))
    );
    deliver_prerequisites(&p, retire);

    assert!(p.task_show_session(id, None, None).is_ok());
    let c = dctx0_tasks::context(
        &s,
        &p.load("latest").unwrap(),
        retire,
        "core",
        usize::MAX,
        None,
    )
    .unwrap();
    assert!(
        c.page
            .items
            .iter()
            .any(|i| i.text.contains("Remove the started"))
    );
}
#[test]
fn claims_transitions_and_external_evidence_enforce_guards() {
    let (_temp, p, _) = setup();
    let id = p
        .task_session(None, None)
        .unwrap()
        .next(&Filters::default(), None)
        .unwrap()
        .task_id;
    assert!(
        p.task_action(Action::Start {
            task: id,
            actor: "".into(),
            reservation: None
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::Start {
            task: -1,
            actor: "worker".into(),
            reservation: None
        })
        .is_err()
    );
    let r = p
        .task_action(Action::ClaimNext {
            actor: "worker".into(),
            filters: Filters::default(),
        })
        .unwrap();
    assert_eq!(r.task_id, Some(id));
    let state = r.state.unwrap();
    assert!(
        p.task_action(Action::Start {
            task: id,
            actor: "other".into(),
            reservation: None
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::Complete {
            task: id,
            actor: "wrong".into(),
            claim: state.claim.unwrap(),
            version: state.version,
            result: result()
        })
        .is_err()
    );
    let mut empty = result();
    empty.summary = "".into();
    assert!(
        p.task_action(Action::Complete {
            task: id,
            actor: "worker".into(),
            claim: state.claim.unwrap(),
            version: state.version,
            result: empty
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::Release {
            task: id,
            actor: "worker".into(),
            claim: state.claim.unwrap(),
            version: state.version,
            reason: "".into()
        })
        .is_err()
    );
    p.task_action(Action::Release {
        task: id,
        actor: "worker".into(),
        claim: state.claim.unwrap(),
        version: state.version,
        reason: "Reassign".into(),
    })
    .unwrap();
    let r = p
        .task_action(Action::Start {
            task: id,
            actor: "worker".into(),
            reservation: None,
        })
        .unwrap();
    let next = r.state.unwrap();
    assert_ne!(state.claim, next.claim);
    assert!(
        p.task_action(Action::Complete {
            task: id,
            actor: "worker".into(),
            claim: state.claim.unwrap(),
            version: state.version,
            result: result()
        })
        .is_err()
    );
    p.task_action(Action::Complete {
        task: id,
        actor: "worker".into(),
        claim: next.claim.unwrap(),
        version: next.version,
        result: result(),
    })
    .unwrap();
    assert_eq!(
        p.task_session(None, None).unwrap().states[&id].check,
        CheckStatus::Unverified
    );
    for change in 0..4 {
        let mut e = evidence(&p, id, CheckStatus::Pass);
        match change {
            0 => e.requirements_hash = "wrong".into(),
            1 => e.verifier = "llm".into(),
            2 => e.command = "".into(),
            _ => e.status = CheckStatus::Unverified,
        };
        assert!(
            p.task_action(Action::RecordCheck {
                task: id,
                evidence: e
            })
            .is_err()
        );
    }
    let pass = evidence(&p, id, CheckStatus::Pass);
    p.task_action(Action::RecordCheck {
        task: id,
        evidence: pass,
    })
    .unwrap();
    let fail = evidence(&p, id, CheckStatus::Fail);
    p.task_action(Action::RecordCheck {
        task: id,
        evidence: fail,
    })
    .unwrap();
    assert_eq!(
        p.task_session(None, None).unwrap().states[&id]
            .evidence
            .len(),
        2
    );
    let repair = p
        .task_follow_up(id, None, "Repair contract implementation")
        .unwrap();
    assert_eq!(
        p.task_follow_up(id, None, "Repair contract implementation")
            .unwrap()
            .id,
        repair.id
    );
    assert_ne!(
        repair
            .members
            .iter()
            .find(|m| m.candidate.key
                == p.task_show_session(id, None, None).unwrap().definitions[&id]
                    .candidate
                    .key)
            .unwrap()
            .task_id,
        id
    );
    assert_eq!(p.task_plan("latest").unwrap().id, repair.id);
    assert!(p.task_follow_up(id, None, "").is_err());
    assert!(p.task_follow_up(-1, None, "repair").is_err());
    assert!(
        p.task_action(Action::RecordCheck {
            task: -1,
            evidence: evidence(&p, id, CheckStatus::Pass)
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::Note {
            task: -1,
            note: "A".into()
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::Note {
            task: id,
            note: "".into()
        })
        .is_err()
    );
    p.task_action(Action::Note {
        task: id,
        note: "Implementation context".into(),
    })
    .unwrap();
    let absent = Filters {
        subject: Some("absent".into()),
        ..Filters::default()
    };
    assert!(
        p.task_action(Action::ClaimNext {
            actor: "worker".into(),
            filters: absent
        })
        .unwrap()
        .task_id
        .is_none()
    );
    assert!(
        p.task_action(Action::ClaimNext {
            actor: "".into(),
            filters: Filters::default()
        })
        .is_err()
    );
}
#[test]
fn slices_reserve_atomically_keep_future_tasks_open_and_preserve_pinned_pages() {
    let (_temp, p, plan) = setup();
    let anchor = plan
        .members
        .iter()
        .find(|m| {
            m.candidate.kind == TaskKind::Operation && m.candidate.subjects[0].ends_with(".get")
        })
        .unwrap()
        .task_id;
    let initial = p.task_session(None, None).unwrap();
    let page = initial.list(&Filters::default(), 1, None).unwrap();
    let cursor = page.next_cursor.unwrap();
    assert!(
        p.task_action(Action::ClaimSlice {
            anchor,
            actor: "".into(),
            domain: None
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::ClaimSlice {
            anchor,
            actor: "one".into(),
            domain: Some("absent".into())
        })
        .is_err()
    );
    let receipt = p
        .task_action(Action::ClaimSlice {
            anchor,
            actor: "one".into(),
            domain: None,
        })
        .unwrap();
    let group = receipt.group_id.unwrap();
    let s = p.task_session(None, None).unwrap();
    assert!(
        s.states
            .values()
            .filter(|s| s.reservation == Some(group))
            .all(|s| s.work == WorkState::Open)
    );
    assert!(
        p.task_action(Action::ClaimSlice {
            anchor,
            actor: "two".into(),
            domain: None
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::ReleaseSlice {
            group,
            actor: "two".into(),
            reason: "Reassign".into()
        })
        .is_err()
    );
    assert!(
        p.task_action(Action::ReleaseSlice {
            group,
            actor: "one".into(),
            reason: "".into()
        })
        .is_err()
    );
    let pinned = p
        .task_session(Some(cursor.plan_id), Some(cursor.event_seq))
        .unwrap();
    assert_eq!(pinned.states, initial.states);
    assert!(pinned.list(&Filters::default(), 1, Some(&cursor)).is_ok());
    let ready = s.next(&Filters::default(), Some("one")).unwrap();
    let started = p
        .task_action(Action::Start {
            task: ready.task_id,
            actor: "one".into(),
            reservation: Some(group),
        })
        .unwrap()
        .state
        .unwrap();
    p.task_action(Action::ReleaseSlice {
        group,
        actor: "one".into(),
        reason: "Keep only ongoing work".into(),
    })
    .unwrap();
    assert_eq!(
        p.task_session(None, None).unwrap().states[&ready.task_id].work,
        WorkState::Progress
    );
    p.task_action(Action::Complete {
        task: ready.task_id,
        actor: "one".into(),
        claim: started.claim.unwrap(),
        version: started.version,
        result: result(),
    })
    .unwrap();
    deliver_prerequisites(&p, anchor);
    complete(&p, anchor);
    assert!(
        p.task_action(Action::ClaimSlice {
            anchor,
            actor: "one".into(),
            domain: None
        })
        .unwrap()
        .group_id
        .is_none()
    );
}
#[test]
fn reviews_are_explicit_tasks_with_separate_reports_and_followups() {
    let (_temp, p, plan) = setup();
    let review = plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Review)
        .unwrap()
        .task_id;
    deliver_prerequisites(&p, review);
    let claimed = p
        .task_action(Action::Start {
            task: review,
            actor: "reviewer".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    assert!(
        p.task_action(Action::Complete {
            task: review,
            actor: "reviewer".into(),
            claim: claimed.claim.unwrap(),
            version: claimed.version,
            result: result()
        })
        .is_err()
    );
    let target = plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Operation)
        .unwrap()
        .task_id;
    let report = ReviewReport {
        reviewer: "llm-reviewer".into(),
        judgement: "request-changes".into(),
        summary: "Boundary needs repair".into(),
        findings: vec![Finding {
            id: "F1".into(),
            severity: "high".into(),
            description: "Implementation violates the modeled boundary".into(),
            task_ids: vec![target],
            action_required: true,
        }],
    };
    let mut bad = report.clone();
    bad.judgement = "PASS".into();
    assert!(
        p.task_action(Action::RecordReview {
            task: review,
            actor: "reviewer".into(),
            claim: claimed.claim.unwrap(),
            version: claimed.version,
            report: bad
        })
        .is_err()
    );
    let mut bad = report.clone();
    bad.findings[0].task_ids = vec![-1];
    assert!(
        p.task_action(Action::RecordReview {
            task: review,
            actor: "reviewer".into(),
            claim: claimed.claim.unwrap(),
            version: claimed.version,
            report: bad
        })
        .is_err()
    );
    let next = p
        .task_action(Action::RecordReview {
            task: review,
            actor: "reviewer".into(),
            claim: claimed.claim.unwrap(),
            version: claimed.version,
            report,
        })
        .unwrap()
        .state
        .unwrap();
    assert_eq!(next.check, CheckStatus::Unverified);
    p.task_action(Action::Complete {
        task: review,
        actor: "reviewer".into(),
        claim: next.claim.unwrap(),
        version: next.version,
        result: result(),
    })
    .unwrap();
    assert!(p.task_follow_up(review, Some("unknown"), "Repair").is_err());
    assert!(p.task_follow_up(target, None, "Repair").is_err());
    let repair = p
        .task_follow_up(review, Some("F1"), "Repair reviewed boundary")
        .unwrap();
    assert_eq!(p.task_plan("latest").unwrap().id, repair.id);
}
#[test]
fn simultaneous_store_claims_never_allocate_the_same_task_and_stale_plans_roll_back() {
    let (_temp, p, plan) = setup();
    let handles = (0..4)
        .map(|i| {
            let db = p.database.clone();
            let key = p.key();
            std::thread::spawn(move || {
                Store::open(&db, AccessMode::ReadWrite)
                    .unwrap()
                    .task_action(
                        &key,
                        Action::ClaimNext {
                            actor: format!("worker-{i}"),
                            filters: Filters::default(),
                        },
                    )
                    .unwrap()
                    .task_id
            })
        })
        .collect::<Vec<_>>();
    let claimed = handles
        .into_iter()
        .filter_map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    let ids = claimed
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), claimed.len());
    assert_eq!(ids.len(), 2);
    let mut store = Store::open(&p.database, AccessMode::ReadWrite).unwrap();
    let snapshot = p.load("latest").unwrap();
    let candidate = dctx0_tasks::plan(&snapshot).unwrap();
    let (run, _) = store.task_check(&p.key()).unwrap();
    let seq = store.task_sequence(&p.key()).unwrap();
    assert!(
        store
            .publish_tasks(
                &p.key(),
                &candidate,
                run,
                Some(plan.id),
                seq - 1,
                &Default::default()
            )
            .is_err()
    );
    assert_eq!(store.task_head(&p.key()).unwrap(), Some(plan.id));
    let connection = rusqlite::Connection::open(&p.database).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_task BEFORE INSERT ON task_events BEGIN SELECT RAISE(ABORT,'simulated rejection'); END;").unwrap();
    let before = store.task_sequence(&p.key()).unwrap();
    assert!(
        store
            .task_action(
                &p.key(),
                Action::Note {
                    task: plan.members[0].task_id,
                    note: "Reject".into()
                }
            )
            .is_err()
    );
    assert_eq!(store.task_sequence(&p.key()).unwrap(), before);
}
#[test]
fn both_complete_examples_produce_explainable_target_neutral_plans() {
    for name in ["cinema", "runatron-next"] {
        let temp = Temp::new();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../examples/{name}/dctx0"));
        for entry in fs::read_dir(source).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|x| x == "dctx0") {
                fs::copy(&path, temp.0.join(path.file_name().unwrap())).unwrap();
            }
        }
        let p = temp.project();
        assert!(p.check().unwrap().ok);
        let plan = p.task_plan("latest").unwrap();
        assert!(!plan.members.is_empty());
        let s = p.task_session(None, None).unwrap();
        assert!(s.overviews().iter().any(|r| r.wave == 1 && r.ready));
        assert_eq!(p.task_plan("latest").unwrap().id, plan.id);
        assert!(s.plan.members.iter().all(|m| {
            m.candidate
                .prerequisites
                .iter()
                .all(|d| !d.reason.is_empty())
        }));
    }
}

#[test]
fn source_locations_and_unplanned_deletion_never_restore_old_identity() {
    let (temp, p, first) = setup();
    let ids = first.members.iter().map(|m| m.task_id).collect::<Vec<_>>();
    fs::rename(temp.0.join("spec.dctx0"), temp.0.join("moved.dctx0")).unwrap();
    p.check().unwrap();
    let moved = p.task_plan("latest").unwrap();
    assert_ne!(first.id, moved.id);
    assert_eq!(
        ids,
        moved.members.iter().map(|m| m.task_id).collect::<Vec<_>>()
    );
    assert_eq!(
        p.task_preview("latest").unwrap().tasks.len(),
        moved.members.len()
    );
    // A name deleted between plans is a new declaration on reintroduction.
    let added = format!("{SPEC}\ndomain Extra {{ type Reused = Int }}");
    fs::write(temp.0.join("moved.dctx0"), &added).unwrap();
    p.check().unwrap();
    let old = p.task_plan("latest").unwrap();
    let original = old
        .members
        .iter()
        .find(|m| m.candidate.subjects.contains(&"W.Extra.Reused".into()))
        .unwrap()
        .task_id;
    complete(&p, original);
    fs::write(temp.0.join("moved.dctx0"), SPEC).unwrap();
    p.check().unwrap();
    fs::write(temp.0.join("moved.dctx0"), added).unwrap();
    p.check().unwrap();
    let next = p.task_plan("latest").unwrap();
    assert!(
        !next
            .members
            .iter()
            .any(|m| m.candidate.kind == TaskKind::Retire
                && m.candidate.subjects.contains(&"W.Extra.Reused".into()))
    );
    assert_ne!(
        original,
        next.members
            .iter()
            .find(|m| m.candidate.subjects.contains(&"W.Extra.Reused".into()))
            .unwrap()
            .task_id
    );
}
#[test]
fn plan_changes_invalidate_reserved_slices_without_starting_their_tasks() {
    let (temp, p, first) = setup();
    let id = first
        .members
        .iter()
        .find(|m| m.candidate.key == "implementation:W.D.M.Api.other")
        .unwrap()
        .task_id;
    let group = p
        .task_action(Action::ClaimSlice {
            anchor: id,
            actor: "one".into(),
            domain: None,
        })
        .unwrap()
        .group_id
        .unwrap();
    temp.write(&SPEC.replace("First", "Changed module intent"));
    p.check().unwrap();
    p.task_plan("latest").unwrap();
    let s = p.task_session(None, None).unwrap();
    assert!(s.states.values().all(|s| s.reservation != Some(group)));
    assert!(s.events.iter().any(|e| e.kind == "reservation-invalidated"));
}

#[test]
fn action_write_failures_roll_back_work_results_evidence_and_reservations() {
    let (_temp, p, plan) = setup();
    let connection = rusqlite::Connection::open(&p.database).unwrap();
    let reject = |kind: &str| {
        connection.execute_batch(&format!("CREATE TRIGGER reject_event BEFORE INSERT ON task_events WHEN NEW.kind='{kind}' BEGIN SELECT RAISE(ABORT,'simulated event failure'); END;")).unwrap()
    };
    let clear = || {
        connection
            .execute_batch("DROP TRIGGER reject_event")
            .unwrap()
    };
    let id = p
        .task_session(None, None)
        .unwrap()
        .next(&Filters::default(), None)
        .unwrap()
        .task_id;
    reject("started");
    assert!(
        p.task_action(Action::Start {
            task: id,
            actor: "one".into(),
            reservation: None
        })
        .is_err()
    );
    clear();
    assert_eq!(
        p.task_session(None, None).unwrap().states[&id].work,
        WorkState::Open
    );
    let started = p
        .task_action(Action::Start {
            task: id,
            actor: "one".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    reject("released");
    assert!(
        p.task_action(Action::Release {
            task: id,
            actor: "one".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            reason: "Release".into()
        })
        .is_err()
    );
    clear();
    reject("completed");
    assert!(
        p.task_action(Action::Complete {
            task: id,
            actor: "one".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            result: result()
        })
        .is_err()
    );
    clear();
    p.task_action(Action::Complete {
        task: id,
        actor: "one".into(),
        claim: started.claim.unwrap(),
        version: started.version,
        result: result(),
    })
    .unwrap();
    reject("check");
    assert!(
        p.task_action(Action::RecordCheck {
            task: id,
            evidence: evidence(&p, id, CheckStatus::Pass)
        })
        .is_err()
    );
    clear();
    assert_eq!(
        p.task_session(None, None).unwrap().states[&id].check,
        CheckStatus::Unverified
    );
    let anchor = plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Operation)
        .unwrap()
        .task_id;
    reject("reserved");
    assert!(
        p.task_action(Action::ClaimSlice {
            anchor,
            actor: "two".into(),
            domain: None
        })
        .is_err()
    );
    clear();
    assert!(
        p.task_session(None, None)
            .unwrap()
            .states
            .values()
            .all(|s| s.reservation.is_none())
    );
    let group = p
        .task_action(Action::ClaimSlice {
            anchor,
            actor: "two".into(),
            domain: None,
        })
        .unwrap()
        .group_id
        .unwrap();
    assert_eq!(
        p.task_action(Action::ClaimSlice {
            anchor,
            actor: "two".into(),
            domain: None
        })
        .unwrap()
        .group_id,
        Some(group)
    );
    reject("reservation-released");
    assert!(
        p.task_action(Action::ReleaseSlice {
            group,
            actor: "two".into(),
            reason: "Free work".into()
        })
        .is_err()
    );
    clear();
    connection.execute_batch("CREATE TRIGGER reject_group BEFORE UPDATE ON task_groups BEGIN SELECT RAISE(ABORT,'group update failure'); END;").unwrap();
    assert!(
        p.task_action(Action::ReleaseSlice {
            group,
            actor: "two".into(),
            reason: "Free work".into()
        })
        .is_err()
    );
    connection
        .execute_batch("DROP TRIGGER reject_group")
        .unwrap();
    p.task_action(Action::ReleaseSlice {
        group,
        actor: "two".into(),
        reason: "Free work".into(),
    })
    .unwrap();
    let review = plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Review)
        .unwrap()
        .task_id;
    deliver_prerequisites(&p, review);
    let s = p
        .task_action(Action::Start {
            task: review,
            actor: "reviewer".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    let report = ReviewReport {
        reviewer: "human".into(),
        judgement: "approved".into(),
        summary: "Criteria met".into(),
        findings: vec![],
    };
    reject("review");
    assert!(
        p.task_action(Action::RecordReview {
            task: review,
            actor: "reviewer".into(),
            claim: s.claim.unwrap(),
            version: s.version,
            report: report.clone()
        })
        .is_err()
    );
    clear();
    let s = p
        .task_action(Action::RecordReview {
            task: review,
            actor: "reviewer".into(),
            claim: s.claim.unwrap(),
            version: s.version,
            report,
        })
        .unwrap()
        .state
        .unwrap();
    p.task_action(Action::Complete {
        task: review,
        actor: "reviewer".into(),
        claim: s.claim.unwrap(),
        version: s.version,
        result: result(),
    })
    .unwrap();
    p.task_action(Action::RecordCheck {
        task: review,
        evidence: evidence(&p, review, CheckStatus::Fail),
    })
    .unwrap();
    let repair = p
        .task_follow_up(review, None, "Repair the review report")
        .unwrap();
    assert_eq!(p.task_plan("latest").unwrap().id, repair.id);
}
#[test]
fn slice_claims_can_include_existing_own_work_and_keep_its_token() {
    let (_temp, p, plan) = setup();
    let anchor = plan
        .members
        .iter()
        .find(|m| m.candidate.key == "implementation:W.D.M.Api.get")
        .unwrap()
        .task_id;
    let id = p
        .task_session(None, None)
        .unwrap()
        .next(&Filters::default(), None)
        .unwrap()
        .task_id;
    let state = p
        .task_action(Action::Start {
            task: id,
            actor: "one".into(),
            reservation: None,
        })
        .unwrap()
        .state
        .unwrap();
    p.task_action(Action::ClaimSlice {
        anchor,
        actor: "one".into(),
        domain: None,
    })
    .unwrap();
    assert_eq!(
        p.task_session(None, None).unwrap().states[&id].claim,
        state.claim
    );
}

#[test]
fn restored_operation_identity_requires_new_integration_even_with_identical_contracts() {
    let (temp, p, _) = setup();
    let extended = SPEC.replace(
        "operation other() -> Int { pure }",
        "operation other() -> Int { pure } operation transient() -> Int { pure }",
    );
    temp.write(&extended);
    assert!(p.check().unwrap().ok);
    let old = p.task_plan("latest").unwrap();
    let integration = old
        .members
        .iter()
        .find(|m| m.candidate.key == "integration:W.D.M")
        .unwrap()
        .task_id;
    deliver_prerequisites(&p, integration);
    complete(&p, integration);
    p.task_action(Action::RecordCheck {
        task: integration,
        evidence: evidence(&p, integration, CheckStatus::Pass),
    })
    .unwrap();
    temp.write(SPEC);
    assert!(p.check().unwrap().ok);
    temp.write(&extended);
    assert!(p.check().unwrap().ok);
    let current = p.task_plan("latest").unwrap();
    let replacement = current
        .members
        .iter()
        .find(|m| m.candidate.key == "integration:W.D.M")
        .unwrap()
        .task_id;
    assert_ne!(replacement, integration);
    let s = p.task_session(None, None).unwrap();
    assert_eq!(s.states[&integration].work, WorkState::Done);
    assert_eq!(s.states[&integration].check, CheckStatus::Pass);
    assert_eq!(s.states[&replacement].check, CheckStatus::Unverified);
    assert_eq!(p.task_plan("latest").unwrap().id, current.id);
}
