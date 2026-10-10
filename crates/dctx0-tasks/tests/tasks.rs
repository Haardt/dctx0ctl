use dctx0_changes::{Snapshot, snapshot};
use dctx0_core::Source;
use dctx0_tasks::*;
use std::collections::{BTreeMap, BTreeSet};

const SPEC: &str = r#"workspace W {
 domain D {
 intent "Domain intent"
 value Id = Int { range 1..100; stableId "domain-key"; where value > 0 }
 value Label = String { length 1..30; format Email }
 type Choice = Empty | Present(Id)
 entity Item { id: Id; label: Label; choice: Choice; key id }
 failure Missing { id: Id }
 event Saved { id: Id }
 module M {
  intent "Module intent"
  owns { Item }
  decision ReadModel { choose "Dedicated contract"; because "Stable clients"; avoid "Entity leakage" }
  constraint "Respect limits"
  port Repo { operation load(id: Id) -> Option<Item> ! Missing { state observes Item by id } effects { read Item } }
  surface Api {
   operation get(id: Id) -> Item ! Missing { requires id > 0; ensures success implies result.id == id }
   effects { call Repo.load; read Item }
   operation other() -> Int { pure }
  }
  example Sample { given Repo.load(sample) returns None; when result = Api.get(sample); expect result fails Missing }
  invariant I { forall item: Item; assert item.id > 0 }
  review Architecture { intent "Review boundaries"; inspect { contracts }; criteria { "Do not leak persistence" }; question Q { """Is the boundary clear?""" } }
 }
 lifecycle L for Item { state Active initial; state Removed terminal; transition M.Api.get from Active to Removed when true }
 }
 proof TypeConformance
 test Examples
 policy Release { require Examples == PASS }
}"#;
fn snap(text: &str) -> Snapshot {
    let sources = vec![Source {
        file: "spec.dctx0".into(),
        text: text.into(),
    }];
    let compiled = dctx0_language::compile(&sources);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let graph = dctx0_graph::build(&ir);
    snapshot(&sources, ir, graph)
}
fn session(snapshot: &Snapshot) -> Session {
    let candidate = plan(snapshot).unwrap();
    let members = candidate
        .tasks
        .iter()
        .enumerate()
        .map(|(i, c)| Member {
            task_id: i as i64 + 1,
            candidate: c.clone(),
        })
        .collect::<Vec<_>>();
    let definitions = members
        .iter()
        .map(|m| {
            (
                m.task_id,
                Definition {
                    adjustment: None,
                    id: m.task_id,
                    candidate: m.candidate.clone(),
                    origin_snapshot: snapshot.id.clone(),
                    origin_check: 1,
                    reason: "Initial".into(),
                    before_snapshot: None,
                    changes: vec![],
                },
            )
        })
        .collect();
    let states = members
        .iter()
        .map(|m| (m.task_id, State::default()))
        .collect();
    Session {
        project: "project".into(),
        plan: Plan {
            id: 1,
            parent: None,
            snapshot_id: snapshot.id.clone(),
            check_run_id: 1,
            planner_version: 1,
            hash: "plan".into(),
            members,
            obligations: candidate.obligations,
        },
        event_seq: 1,
        definitions,
        states,
        links: vec![],
        events: vec![],
    }
}

fn adoption_inputs(
    s: &Session,
) -> (
    dctx0_core::VerificationTaskPin,
    BTreeMap<i64, AdoptionTaskProof>,
) {
    let pin = dctx0_core::VerificationTaskPin {
        plan_id: s.plan.id,
        event_sequence: s.event_seq,
        requirements: s
            .plan
            .members
            .iter()
            .map(|m| (m.task_id, m.candidate.requirements_hash.clone()))
            .collect(),
        profile_bundles: s
            .plan
            .members
            .iter()
            .map(|m| (m.task_id, "profile".into()))
            .collect(),
    };
    let proofs = s
        .plan
        .members
        .iter()
        .map(|m| {
            (
                m.task_id,
                AdoptionTaskProof {
                    requirements_hash: m.candidate.requirements_hash.clone(),
                    covered_requirements: m
                        .candidate
                        .subjects
                        .iter()
                        .chain(&m.candidate.types)
                        .chain(&m.candidate.facts)
                        .cloned()
                        .collect(),
                    required_cases: ["positive".into(), "negative".into()].into(),
                    executed_cases: [
                        ("positive".into(), dctx0_core::VerificationResult::Pass),
                        ("negative".into(), dctx0_core::VerificationResult::Pass),
                    ]
                    .into(),
                    implementation: "implementation-content-hash".into(),
                    artifacts: vec!["src/app.ts".into()],
                    representations: [("Int".into(), "lossless integer".into())].into(),
                    profile_bundle: "profile".into(),
                    incomplete: vec![],
                },
            )
        })
        .collect();
    (pin, proofs)
}

#[test]
fn adoption_proposals_deliver_verified_producers_topologically_without_mutating_history() {
    let input = snap(
        "workspace W { value Id = Int { range 0..2 } surface Api { operation read(id:Id) -> Int { pure } } }",
    );
    let s = session(&input);
    let before = serde_json::to_value(&s).unwrap();
    let (mut pin, mut proofs) = adoption_inputs(&s);
    let producer = s
        .plan
        .members
        .iter()
        .find(|m| m.candidate.subjects == ["W.Id"])
        .unwrap()
        .task_id;
    let consumer = s
        .plan
        .members
        .iter()
        .find(|m| m.candidate.subjects == ["W.Api.read"])
        .unwrap()
        .task_id;
    let selected = [producer, consumer].into_iter().collect::<BTreeSet<_>>();
    pin.requirements.retain(|id, _| selected.contains(id));
    pin.profile_bundles.retain(|id, _| selected.contains(id));
    proofs.retain(|id, _| selected.contains(id));
    let proposal = propose_adoption(&s, &input.id, 1, &pin, &proofs).unwrap();
    assert_eq!(proposal.tasks, [producer, consumer]);
    assert_eq!(serde_json::to_value(&s).unwrap(), before);
    let mut missing = proofs.clone();
    missing.remove(&producer);
    let blocked = propose_adoption(&s, &input.id, 1, &pin, &missing).unwrap();
    assert!(blocked.tasks.is_empty());
    assert!(
        blocked
            .blocked
            .iter()
            .any(|b| b.task_id == producer && b.code == "TASK_EVIDENCE_REQUIRED")
    );
    assert!(
        blocked
            .blocked
            .iter()
            .any(|b| b.task_id == consumer && b.code == "SCHEDULING_BLOCKED")
    );
    let mut reserved = s.clone();
    reserved.states.get_mut(&producer).unwrap().reservation = Some(99);
    assert!(
        propose_adoption(&reserved, &input.id, 1, &pin, &proofs)
            .unwrap()
            .blocked
            .iter()
            .any(|b| b.task_id == producer && b.code == "TASK_NOT_UNCLAIMED_OPEN")
    );
    for work in [WorkState::Progress, WorkState::Done] {
        let mut ongoing = s.clone();
        ongoing.states.get_mut(&producer).unwrap().work = work;
        assert!(
            !propose_adoption(&ongoing, &input.id, 1, &pin, &proofs)
                .unwrap()
                .tasks
                .contains(&producer)
        );
    }
}

#[test]
fn adoption_proposals_reject_partial_unknown_failed_unmapped_and_stale_evidence() {
    let input = snap("workspace W { value Id = Int { range 0..2 } }");
    let s = session(&input);
    let (pin, proofs) = adoption_inputs(&s);
    let id = s
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Contract)
        .unwrap()
        .task_id;
    for mutation in 0..10 {
        let mut changed = proofs.clone();
        let proof = changed.get_mut(&id).unwrap();
        match mutation {
            0 => {
                proof.required_cases.clear();
            }
            1 => {
                proof.executed_cases.remove("negative");
            }
            2 => {
                proof.executed_cases.insert(
                    "negative".into(),
                    dctx0_core::VerificationResult::Unknown {
                        issue: dctx0_core::VerificationIssue {
                            code: "MISSING_OBSERVER".into(),
                            message: "Actual observation absent".into(),
                            span: None,
                        },
                    },
                );
            }
            3 => {
                proof.executed_cases.insert(
                    "negative".into(),
                    dctx0_core::VerificationResult::Fail {
                        issue: dctx0_core::VerificationIssue {
                            code: "VIOLATED".into(),
                            message: "Actual violation".into(),
                            span: None,
                        },
                    },
                );
            }
            4 => {
                proof.covered_requirements.clear();
            }
            5 => {
                proof
                    .incomplete
                    .push("Incomplete dependency capture".into());
            }
            6 => {
                proof.artifacts.clear();
            }
            7 => {
                proof.representations.clear();
            }
            8 => {
                proof.profile_bundle = "edited-profile".into();
            }
            _ => {
                proof.requirements_hash = "old-requirements".into();
            }
        }
        let proposal = propose_adoption(&s, &input.id, 1, &pin, &changed).unwrap();
        assert!(!proposal.tasks.contains(&id), "mutation {mutation}");
        assert!(proposal.blocked.iter().any(|b| b.task_id == id));
    }
    for mutation in 0..3 {
        let mut stale = pin.clone();
        match mutation {
            0 => stale.plan_id += 1,
            1 => stale.event_sequence += 1,
            _ => stale.requirements.clear(),
        }
        assert!(propose_adoption(&s, &input.id, 1, &stale, &proofs).is_err());
    }
    assert!(propose_adoption(&s, "other-snapshot", 1, &pin, &proofs).is_err());
    assert!(propose_adoption(&s, &input.id, 2, &pin, &proofs).is_err());
    let mut extra = proofs.clone();
    extra.insert(9999, proofs[&id].clone());
    assert!(propose_adoption(&s, &input.id, 1, &pin, &extra).is_err());
}
#[test]
fn planner_models_contracts_rules_examples_reviews_and_recursive_groups() {
    let first = snap(SPEC);
    let a = plan(&first).unwrap();
    let b = plan(&snap(&SPEC.replace("\n", "\n\n"))).unwrap();
    assert_eq!(a.tasks, b.tasks);
    assert_eq!(a.obligations.len(), 3);
    let op = a
        .tasks
        .iter()
        .find(|t| t.key == "implementation:W.D.M.Api.get")
        .unwrap();
    assert!(op.types.contains(&"W.D.Id".into()));
    assert!(op.facts.iter().any(|f| f.contains("Sample")));
    assert!(op.facts.iter().any(|f| f.contains("ReadModel")));
    assert!(op.facts.iter().any(|f| f.contains("L#transition")));
    assert!(a.tasks.iter().any(|t| t.kind == TaskKind::Adapter));
    assert!(a.tasks.iter().any(|t| t.kind == TaskKind::Review));
}

#[test]
fn root_intents_empty_modules_and_review_criteria_follow_their_scope() {
    let input = snap(
        "workspace W { intent \"Shared purpose\"; module Empty { intent \"No operations\" } module M { surface S { operation f() -> Int } surface Other { operation g() -> Int } review R for S { inspect { implementation }; criteria { \"Only S\" } } } }",
    );
    let p = plan(&input).unwrap();
    let mut duplicate = p.clone();
    duplicate.tasks.push(duplicate.tasks[0].clone());
    assert!(
        reconcile(None, &duplicate, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("Duplicate")
    );
    assert!(!p.tasks.iter().any(|t| t.key == "integration:W.Empty"));
    let criteria = input
        .ir
        .nodes
        .iter()
        .find(|n| n.kind == dctx0_core::StatementKind::Criterion)
        .unwrap();
    assert!(
        p.tasks
            .iter()
            .find(|t| t.key == "implementation:W.M.S.f")
            .unwrap()
            .facts
            .contains(&criteria.id)
    );
    assert!(
        !p.tasks
            .iter()
            .find(|t| t.key == "implementation:W.M.Other.g")
            .unwrap()
            .facts
            .contains(&criteria.id)
    );
    let mut s = session(&input);
    let ids = s
        .plan
        .members
        .iter()
        .take(3)
        .map(|m| m.task_id)
        .collect::<Vec<_>>();
    s.links = vec![
        HistoryLink {
            from: ids[0],
            to: ids[1],
            kind: "update".into(),
            reason: "first".into(),
        },
        HistoryLink {
            from: ids[1],
            to: ids[2],
            kind: "update".into(),
            reason: "second".into(),
        },
    ];
    assert_eq!(s.history_links(ids[2], "ancestors").unwrap().len(), 2);
}
#[test]
fn semantic_cycles_external_contracts_and_explicit_review_targets() {
    let input = snap(
        "workspace W { type A { b: Option<B> } type B { a: Option<A> } surface Api { operation a() -> Int; effects { call Api.b }; operation b() -> Int; effects { call Api.a } } external Remote { surface Service { operation fetch() -> A } } module M { surface S { operation f() -> Int } review R for S { inspect { implementation }; criteria { \"Match the contract\" } } } }",
    );
    let p = plan(&input).unwrap();
    assert!(
        p.tasks
            .iter()
            .any(|t| t.kind == TaskKind::Contract && t.subjects.len() == 2)
    );
    assert!(
        !p.tasks
            .iter()
            .any(|t| t.key == "implementation:W.Remote.Service.fetch")
    );
    let r = p.tasks.iter().find(|t| t.kind == TaskKind::Review).unwrap();
    assert!(
        r.prerequisites
            .iter()
            .any(|p| p.key == "implementation:W.M.S.f")
    );
    assert_eq!(
        contract_text(&input.ir, "W.A"),
        "type W.A\n  b: Option<W.B>"
    );
    assert_eq!(contract_text(&input.ir, "unknown"), "unknown");
}
#[test]
fn dag_diagnoses_missing_producers_and_hard_cycles() {
    let mut tasks = plan(&snap(SPEC))
        .unwrap()
        .tasks
        .into_iter()
        .map(|t| (t.key.clone(), t))
        .collect::<BTreeMap<_, _>>();
    let key = tasks.keys().next().unwrap().clone();
    tasks.get_mut(&key).unwrap().prerequisites = vec![Prerequisite {
        key: "missing".into(),
        condition: Condition::Delivered,
        reason: "Required output".into(),
    }];
    assert!(
        assign_waves(&mut tasks)
            .unwrap_err()
            .to_string()
            .contains("Missing prerequisite")
    );
    tasks.get_mut(&key).unwrap().prerequisites[0].key = key.clone();
    assert!(
        assign_waves(&mut tasks)
            .unwrap_err()
            .to_string()
            .contains("Hard task cycle")
    );
}
#[test]
fn queries_distinguish_work_readiness_evidence_reservations_and_history() {
    let input = snap(SPEC);
    let mut s = session(&input);
    let first = s.next(&Filters::default(), None).unwrap();
    let id = first.task_id;
    assert!(!s.overviews().iter().all(|r| r.ready));
    assert!(
        s.next(
            &Filters {
                state: Some(WorkState::Done),
                ..Filters::default()
            },
            None
        )
        .is_none()
    );
    assert!(s.member(-1).is_err());
    s.states.get_mut(&id).unwrap().actor = Some("one".into());
    s.states.get_mut(&id).unwrap().reservation = Some(1);
    assert_ne!(s.next(&Filters::default(), None).unwrap().task_id, id);
    assert_eq!(
        s.next(&Filters::default(), Some("one")).unwrap().task_id,
        id
    );
    let filters = Filters {
        wave: Some(first.wave),
        state: Some(WorkState::Open),
        check: Some(CheckStatus::Unverified),
        kind: Some(first.kind),
        subject: Some(first.subjects[0].clone()),
        domain: Some("W.D".into()),
        actor: Some("one".into()),
    };
    assert_eq!(s.list(&filters, 50, None).unwrap().items.len(), 1);
    assert!(s.list(&Filters::default(), 0, None).is_err());
    let page = s.list(&Filters::default(), 1, None).unwrap();
    assert!(!page.complete);
    let next = s
        .list(&Filters::default(), 1, page.next_cursor.as_ref())
        .unwrap();
    assert_ne!(page.items[0].task_id, next.items[0].task_id);
    let mut bad = page.next_cursor.clone().unwrap();
    bad.query_hash = "wrong".into();
    assert!(s.list(&Filters::default(), 1, Some(&bad)).is_err());
    bad.query_hash = dctx0_changes::hash(&Filters::default());
    bad.offset = usize::MAX;
    assert!(s.list(&Filters::default(), 1, Some(&bad)).is_err());
    assert!(!s.related(id, true, true).unwrap().is_empty());
    assert!(s.related(id, false, false).unwrap().is_empty());
    assert!(s.slice(-1, None, None).is_err());
    let anchor = s
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Operation)
        .unwrap()
        .task_id;
    assert!(!s.slice(anchor, None, None).unwrap().closed);
    assert!(s.slice(anchor, None, Some("one")).unwrap().closed);
    assert!(
        !s.slice(anchor, Some("outside"), Some("one"))
            .unwrap()
            .closed
    );
    for state in s.states.values_mut() {
        state.work = WorkState::Done;
        state.actor = None;
        state.reservation = None;
        state.result = Some(TaskResult {
            summary: "Delivered".into(),
            artifacts: vec![],
            decisions: vec![],
            implementation: Some("revision".into()),
        });
    }
    assert!(s.next(&Filters::default(), None).is_none());
    assert!(s.slice(anchor, None, None).unwrap().open_work.is_empty());
    let m = s
        .plan
        .members
        .iter_mut()
        .find(|m| !m.candidate.prerequisites.is_empty())
        .unwrap();
    m.candidate.prerequisites[0].condition = Condition::CheckPass;
    assert!(!s.overviews().iter().all(|r| r.delivered));
    assert!(
        !s.slice(anchor, None, None)
            .unwrap()
            .boundary
            .contains(&anchor)
    );
    s.links = vec![HistoryLink {
        from: id,
        to: anchor,
        kind: "update".into(),
        reason: "changed".into(),
    }];
    assert_eq!(s.history_links(anchor, "ancestors").unwrap().len(), 1);
    assert_eq!(s.history_links(id, "descendants").unwrap().len(), 1);
    assert_eq!(s.history_links(anchor, "both").unwrap().len(), 1);
    assert!(s.history_links(-1, "both").is_err());
    assert!(s.history_links(id, "wrong").is_err());
    let mut old = s.plan.clone();
    old.members.retain(|m| m.task_id != anchor);
    let diff = diff_plans(&old, &s.plan, &s.links);
    assert_eq!(diff.added, vec![anchor]);
    assert!(!diff.kept.is_empty());
    // A superseded actor blocks replacement work separately from history edges.
    let old_id = 999;
    let mut definition = s.definitions[&anchor].clone();
    definition.id = old_id;
    s.definitions.insert(old_id, definition);
    let state = State {
        work: WorkState::Progress,
        actor: Some("previous".into()),
        ..State::default()
    };
    s.states.insert(old_id, state);
    assert!(
        s.overviews()
            .iter()
            .find(|r| r.task_id == anchor)
            .unwrap()
            .blockers
            .iter()
            .any(|b| b.contains("superseded work"))
    );
}
#[test]
fn context_pages_preserve_types_refinements_and_historical_summaries() {
    let input = snap(SPEC);
    let mut s = session(&input);
    let id = s
        .plan
        .members
        .iter()
        .find(|m| m.candidate.key == "implementation:W.D.M.Api.get")
        .unwrap()
        .task_id;
    let predecessor = s.plan.members[0].task_id;
    s.links.push(HistoryLink {
        from: predecessor,
        to: id,
        kind: "update".into(),
        reason: "A new rule".into(),
    });
    s.states.get_mut(&predecessor).unwrap().result = Some(TaskResult {
        summary: "Earlier delivery".into(),
        artifacts: vec!["src/types.rs".into()],
        decisions: vec![],
        implementation: None,
    });
    s.states.get_mut(&id).unwrap().result = Some(TaskResult {
        summary: "Current delivery".into(),
        artifacts: vec![],
        decisions: vec!["Use a port".into()],
        implementation: None,
    });
    let provider = s
        .related(id, false, false)
        .unwrap()
        .into_iter()
        .find(|e| e.prerequisite_id != predecessor)
        .unwrap()
        .prerequisite_id;
    s.states.get_mut(&provider).unwrap().result = Some(TaskResult {
        summary: "Provider ready".into(),
        artifacts: vec!["src/provider.rs".into()],
        decisions: vec!["Preserve the interface".into()],
        implementation: None,
    });
    let full = context(&s, &input, id, "core", usize::MAX, None).unwrap();
    let text = context_markdown(&full);
    assert!(text.contains("range 1..100"));
    assert!(text.contains("length 1..30"));
    assert!(text.contains("format Email"));
    assert!(text.contains("where value > 0"));
    assert!(text.contains("Earlier delivery"));
    assert!(text.contains("src/provider.rs"));
    assert!(text.contains("Preserve the interface"));
    assert!(text.contains("Current delivery"));
    assert!(text.contains("key id"));
    assert!(full.missing_sections.is_empty());
    let small = context(&s, &input, id, "core", 1, None).unwrap();
    assert!(small.oversized_item);
    assert!(!small.missing_sections.is_empty());
    assert!(context_markdown(&small).contains("--cursor"));
    let next = context(&s, &input, id, "core", 1, small.page.next_cursor.as_ref()).unwrap();
    assert_ne!(small.page.items[0].title, next.page.items[0].title);
    for section in [
        "types",
        "rules",
        "examples",
        "decisions",
        "changes",
        "dependencies",
        "history",
        "result",
    ] {
        let p = context(&s, &input, id, section, 2000, None).unwrap();
        assert!(p.page.items.iter().all(|i| i.section == section));
    }
    assert!(context(&s, &input, id, "wrong", 100, None).is_err());
    assert!(context(&s, &input, id, "core", 0, None).is_err());
    let mut bad = s.plan.clone();
    bad.snapshot_id = "wrong".into();
    let mut altered = s.clone();
    altered.plan = bad;
    assert!(context(&altered, &input, id, "core", 100, None).is_err());
    let mut cursor = small.page.next_cursor.unwrap();
    let mut old_layout = cursor.clone();
    old_layout.query_hash = dctx0_changes::hash(&(id, "core", 1_usize));
    assert!(context(&s, &input, id, "core", 1, Some(&old_layout)).is_err());
    cursor.offset = usize::MAX;
    assert!(context(&s, &input, id, "core", 1, Some(&cursor)).is_err());
    let mut missing_sources = input.clone();
    missing_sources.sources.clear();
    assert!(
        context_markdown(&context(&s, &missing_sources, id, "core", usize::MAX, None).unwrap())
            .contains("range")
    );
    assert!(fence("````\ntext").starts_with("`````"));
}
#[test]
fn reconciliation_keeps_current_work_and_builds_reversion_retirement_split_merge() {
    let input = snap(SPEC);
    let mut old = session(&input);
    let continuity = input
        .ir
        .symbols
        .keys()
        .map(|s| (s.clone(), s.clone()))
        .collect();
    let candidate = plan(&input).unwrap();
    assert!(
        reconcile(Some(&old), &candidate, &continuity)
            .unwrap()
            .iter()
            .all(|p| p.reuse.is_some())
    );
    assert!(
        reconcile(None, &candidate, &BTreeMap::new())
            .unwrap()
            .iter()
            .all(|p| p.reuse.is_none())
    );
    let change = plan(&snap(
        &SPEC.replace("Respect limits", "Respect changed limits"),
    ))
    .unwrap();
    assert!(
        reconcile(Some(&old), &change, &continuity)
            .unwrap()
            .iter()
            .any(|p| !p.predecessors.is_empty())
    );
    let id = old
        .plan
        .members
        .iter()
        .find(|m| m.candidate.key == "implementation:W.D.M.Api.other")
        .unwrap()
        .task_id;
    old.states.get_mut(&id).unwrap().ever_started = true;
    let removed = plan(&snap(
        &SPEC.replace("operation other() -> Int { pure }", ""),
    ))
    .unwrap();
    let proposals = reconcile(Some(&old), &removed, &continuity).unwrap();
    assert!(
        proposals
            .iter()
            .any(|p| p.candidate.kind == TaskKind::Retire)
    );
    let retire = proposals
        .iter()
        .find(|p| p.candidate.kind == TaskKind::Retire)
        .unwrap()
        .candidate
        .clone();
    old.plan.members.push(Member {
        task_id: 999,
        candidate: retire,
    });
    old.states.insert(999, State::default());
    assert!(
        reconcile(Some(&old), &removed, &continuity)
            .unwrap()
            .iter()
            .any(|p| p.reuse == Some(999))
    );
    let cyclic = snap("workspace W { type A { b: Option<B> } type B { a: Option<A> } }");
    let separated = snap("workspace W { type A { b: Option<B> } type B = Int }");
    let map = cyclic
        .ir
        .symbols
        .keys()
        .map(|s| (s.clone(), s.clone()))
        .collect();
    let split = reconcile(Some(&session(&cyclic)), &plan(&separated).unwrap(), &map).unwrap();
    assert!(
        split
            .iter()
            .any(|p| p.predecessors.iter().any(|(_, kind)| kind == "split"))
    );
    let merge = reconcile(Some(&session(&separated)), &plan(&cyclic).unwrap(), &map).unwrap();
    assert!(
        merge
            .iter()
            .any(|p| p.predecessors.iter().any(|(_, kind)| kind == "merge"))
    );
}

#[test]
fn cross_module_obligations_and_global_reviews_have_explicit_delivery_dependencies() {
    let input = snap(
        "workspace W { module A { surface Api { operation f() -> Int { pure } } } module B { surface Api { operation g() -> Int { pure } } } invariant Together { assert A.Api.f() == B.Api.g() } }",
    );
    let p = plan(&input).unwrap();
    assert!(
        p.tasks
            .iter()
            .any(|t| t.key == "integration:W.Together" && t.prerequisites.len() == 2)
    );
    let root = snap(
        "workspace W { surface Api { operation f() -> Int { pure } } review R { inspect { implementation }; criteria { \"Follow the contract\" } } }",
    );
    let p = plan(&root).unwrap();
    assert!(
        p.tasks
            .iter()
            .find(|t| t.kind == TaskKind::Review)
            .unwrap()
            .prerequisites
            .iter()
            .any(|d| d.key == "implementation:W.Api.f")
    );
}
#[test]
fn concrete_deltas_compare_contracts_and_facts_even_without_raw_source_excerpts() {
    let before = snap(SPEC);
    let after = snap(&SPEC.replace("range 1..100", "range 1..200"));
    let a = plan(&before).unwrap();
    let b = plan(&after).unwrap();
    let old = a
        .tasks
        .iter()
        .find(|t| t.key == "implementation:W.D.M.Api.get")
        .unwrap();
    let new = b.tasks.iter().find(|t| t.key == old.key).unwrap();
    let changes = DeltaIndex::new(&before).compare(&DeltaIndex::new(&after), old, new);
    assert!(
        changes
            .iter()
            .any(|c| c.before.as_ref().is_some_and(|s| s.contains("1..100")))
    );
    assert!(
        changes
            .iter()
            .any(|c| c.after.as_ref().is_some_and(|s| s.contains("1..200")))
    );
    let mut missing = before.clone();
    missing.sources.clear();
    assert!(
        !DeltaIndex::new(&missing)
            .compare(&DeltaIndex::new(&after), old, new)
            .is_empty()
    );
}

#[test]
fn constant_examples_do_not_invent_operation_producers() {
    let input = snap("workspace W { example Constant { when result = 1; expect result == 1 } }");
    assert!(plan(&input).unwrap().tasks.is_empty());
}
#[test]
fn caller_contract_changes_have_concrete_imported_signature_deltas() {
    let before = snap(
        "workspace W { surface Api { operation supplier() -> Int { pure } operation caller() -> Int; effects { call Api.supplier } } }",
    );
    let after = snap(
        "workspace W { surface Api { operation supplier() -> String { pure } operation caller() -> Int; effects { call Api.supplier } } }",
    );
    let a = plan(&before).unwrap();
    let b = plan(&after).unwrap();
    let old = a
        .tasks
        .iter()
        .find(|t| t.key == "implementation:W.Api.caller")
        .unwrap();
    let new = b.tasks.iter().find(|t| t.key == old.key).unwrap();
    assert_ne!(old.requirements_hash, new.requirements_hash);
    let changes = DeltaIndex::new(&before).compare(&DeltaIndex::new(&after), old, new);
    assert!(changes.iter().any(|c| c.subject == "W.Api.supplier"
        && c.before.as_ref().is_some_and(|s| s.contains("Int"))
        && c.after.as_ref().is_some_and(|s| s.contains("String"))));
}

#[test]
fn indexes_keep_unresolved_annotations_without_inventing_type_or_parent_links() {
    let mut input = snap("workspace W { intent \"Known purpose\"; type Id = Int }");
    let mut orphan = input
        .ir
        .nodes
        .iter()
        .find(|n| n.kind == dctx0_core::StatementKind::Intent)
        .unwrap()
        .clone();
    orphan.id = "unmodeled#intent".into();
    orphan.owner = "unmodeled".into();
    input.ir.nodes.push(orphan.clone());
    let index = SpecIndex::new(&input.ir);
    assert_eq!(index.ancestors("unmodeled"), vec!["unmodeled"]);
    assert_eq!(index.facts("unmodeled", true), vec![orphan.id.clone()]);
    assert!(
        index
            .type_closure(&["unmodeled".into()], &[orphan.id])
            .is_empty()
    );
}

#[test]
fn assignments_include_complete_entity_message_specs_and_specialized_acceptance() {
    let source = r#"workspace W {
      intent "Shared domain purpose"
      value Id = UUID
      message Request { id: Id; items: List<Id> }
      entity Record { id: Id; request: Request; key id }
      event Changed { id: Id }
      failure Rejected { id: Id }
      type Choice = None | Some(Id)
      module M { surface Api { operation save(input: Request) -> Record ! Rejected { pure } } }
    }"#;
    let input = snap(source);
    let mut s = session(&input);
    for member in &s.plan.members {
        let page = context(&s, &input, member.task_id, "core", usize::MAX, None).unwrap();
        let md = context_markdown(&page);
        assert!(md.contains("## Acceptance"));
        assert!(!md.contains("Change request"));
        assert!(!md.contains("Implement this specification requirement"));
        assert!(page.missing_sections.is_empty());
    }
    for (subject, excerpt) in [
        ("W.Request", "message Request { id: Id; items: List<Id> }"),
        (
            "W.Record",
            "entity Record { id: Id; request: Request; key id }",
        ),
        ("W.Changed", "event Changed { id: Id }"),
        ("W.Rejected", "failure Rejected { id: Id }"),
    ] {
        let id = s
            .plan
            .members
            .iter()
            .find(|m| m.candidate.key == format!("contract:{subject}"))
            .unwrap()
            .task_id;
        let page = context(&s, &input, id, "core", usize::MAX, None).unwrap();
        let md = context_markdown(&page);
        assert_eq!(md.matches(excerpt).count(), 1);
        let own = page
            .page
            .items
            .iter()
            .find(|i| i.section == "contract" && i.title.ends_with(subject))
            .unwrap();
        assert_eq!(own.text, excerpt);
        assert!(own.type_definition.is_some());
        assert!(own.source.is_some());
        assert!(md.contains("field name, type"));
    }
    let id = s
        .plan
        .members
        .iter()
        .find(|m| m.candidate.key == "contract:W.Id")
        .unwrap()
        .task_id;
    let page = context(&s, &input, id, "core", usize::MAX, None).unwrap();
    let md = context_markdown(&page);
    assert_eq!(md.matches("value Id = UUID").count(), 1);
    assert!(md.contains("using `UUID`"));
    assert!(!md.contains("variants"));
    assert!(!md.contains("identity keys"));
    assert!(!md.contains("fields"));
    assert!(
        context(&s, &input, id, "types", usize::MAX, None)
            .unwrap()
            .page
            .items
            .iter()
            .any(|i| i.type_definition.is_some())
    );
    let refined = snap(SPEC);
    let refined_session = session(&refined);
    for member in &refined_session.plan.members {
        let page = context(
            &refined_session,
            &refined,
            member.task_id,
            "core",
            usize::MAX,
            None,
        )
        .unwrap();
        let text = context_markdown(&page);
        if member.candidate.key == "contract:W.D.Id" {
            assert!(text.contains("stableId metadata `domain-key`"));
            assert!(text.contains("test their boundaries"));
        }
        if member.candidate.kind == TaskKind::Adapter {
            assert!(text.contains("port adapter operation"));
        }
        if member.candidate.kind == TaskKind::Review {
            assert!(text.contains("Review the implementation"));
        }
    }
    let recursive = snap("workspace W { type A { b: Option<B> } type B { a: Option<A> } }");
    let recursive_session = session(&recursive);
    let group = recursive_session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.subjects.len() == 2)
        .unwrap();
    let grouped = context_markdown(
        &context(
            &recursive_session,
            &recursive,
            group.task_id,
            "core",
            usize::MAX,
            None,
        )
        .unwrap(),
    );
    assert!(grouped.contains("recursive contract component together"));
    assert_eq!(grouped.matches("type A { b: Option<B> }").count(), 1);
    assert_eq!(grouped.matches("type B { a: Option<A> }").count(), 1);
    // A removal keeps the old contract and explains retirement rather than
    // presenting it as a new UUID implementation.
    let member = s.plan.members.iter_mut().find(|m| m.task_id == id).unwrap();
    member.candidate.kind = TaskKind::Retire;
    s.definitions.get_mut(&id).unwrap().before_snapshot = Some("old".into());
    let removed = context(&s, &input, id, "core", usize::MAX, None).unwrap();
    assert!(context_markdown(&removed).contains("Retire the obsolete"));
    assert!(context_markdown(&removed).contains("Removed contract"));
    assert!(context_markdown(&removed).contains("Change request"));
}

#[test]
fn task_case_coverage_requires_every_mapped_case_and_never_uses_unrelated_success() {
    use dctx0_core::{VerificationIssue, VerificationResult as V};
    let requirements = ["contract".into(), "postcondition".into()].into();
    let mapping = [
        ("positive".into(), ["contract".into()].into()),
        ("negative".into(), ["contract".into()].into()),
        ("operation".into(), ["postcondition".into()].into()),
        ("other".into(), ["unrelated".into()].into()),
    ]
    .into();
    let complete = [
        ("positive".into(), V::Pass),
        ("negative".into(), V::Pass),
        ("operation".into(), V::Pass),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let passed = task_case_coverage(&requirements, &mapping, &complete);
    assert_eq!(passed.result, V::Pass);
    assert_eq!(passed.required_cases, complete.keys().cloned().collect());
    assert!(passed.missing_requirements.is_empty());
    let mut partial = complete.clone();
    partial.remove("negative");
    let incomplete = task_case_coverage(&requirements, &mapping, &partial);
    assert_eq!(incomplete.unexecuted_cases, ["negative".into()].into());
    assert!(matches!(incomplete.result, V::Unknown { .. }));
    let missing = task_case_coverage(&["unmodeled".into()].into(), &mapping, &complete);
    assert_eq!(missing.missing_requirements, ["unmodeled".into()].into());
    assert!(matches!(missing.result, V::Unknown { .. }));
    assert!(matches!(
        task_case_coverage(&BTreeSet::new(), &mapping, &complete).result,
        V::Unknown { .. }
    ));
    let unknown = V::Unknown {
        issue: VerificationIssue {
            code: "OBSERVER_REQUIRED".into(),
            message: "No observer".into(),
            span: None,
        },
    };
    let failed = V::Fail {
        issue: VerificationIssue {
            code: "VIOLATION".into(),
            message: "Actual violation".into(),
            span: None,
        },
    };
    let mut outcomes = complete;
    outcomes.insert("operation".into(), unknown.clone());
    assert_eq!(
        task_case_coverage(&requirements, &mapping, &outcomes).result,
        unknown
    );
    outcomes.insert("negative".into(), failed.clone());
    assert_eq!(
        task_case_coverage(&requirements, &mapping, &outcomes).result,
        failed
    );
    outcomes.remove("positive");
    assert_eq!(
        task_case_coverage(&requirements, &mapping, &outcomes).result,
        failed
    );
    outcomes.insert("other".into(), failed);
    assert_eq!(
        task_case_coverage(&["postcondition".into()].into(), &mapping, &outcomes).result,
        unknown
    );
    outcomes.insert("operation".into(), V::Pass);
    assert_eq!(
        task_case_coverage(&["postcondition".into()].into(), &mapping, &outcomes).result,
        V::Pass
    );
}
