use super::*;
use crate::Result;
pub(super) fn insert_task(
    tx: &Connection,
    project: &str,
    candidate: &Candidate,
    snapshot: &str,
    run: i64,
    reason: &str,
    before: Option<String>,
) -> Result<i64> {
    tx.execute("INSERT INTO tasks(project,logical_key,kind,requirements_hash,data) VALUES(?1,?2,?3,?4,'{}')",params![project,candidate.key,format!("{:?}",candidate.kind),candidate.requirements_hash])?;
    let id = tx.last_insert_rowid();
    let definition = Definition {
        adjustment: None,
        id,
        candidate: candidate.clone(),
        origin_snapshot: snapshot.into(),
        origin_check: run,
        reason: reason.into(),
        before_snapshot: before,
        changes: vec![],
    };
    tx.execute(
        "UPDATE tasks SET data=?1 WHERE id=?2",
        params![serde_json::to_string(&definition)?, id],
    )?;
    let seq = event(
        tx,
        project,
        Some(id),
        "created",
        reason,
        Some(&State::default()),
    )?;
    tx.execute(
        "UPDATE tasks SET created_seq=?1 WHERE id=?2",
        params![seq, id],
    )?;
    Ok(id)
}
pub(super) fn store_plan(tx: &Connection, project: &str, mut plan: Plan) -> Result<Plan> {
    tx.execute("INSERT INTO task_plans(project,parent_id,snapshot_id,check_run_id,data) VALUES(?1,?2,?3,?4,'{}')",params![project,plan.parent,plan.snapshot_id,plan.check_run_id])?;
    plan.id = tx.last_insert_rowid();
    let ids = plan
        .members
        .iter()
        .map(|m| (&m.candidate.key, m.task_id))
        .collect::<BTreeMap<_, _>>();
    for m in &plan.members {
        tx.execute(
            "INSERT INTO task_members(plan_id,task_id,wave) VALUES(?1,?2,?3)",
            params![plan.id, m.task_id, m.candidate.wave as i64],
        )?;
        for p in &m.candidate.prerequisites {
            tx.execute("INSERT INTO task_dependencies(plan_id,task_id,prerequisite_id,condition,reason) VALUES(?1,?2,?3,?4,?5)",params![plan.id,m.task_id,ids[&p.key],serde_json::to_string(&p.condition)?,p.reason])?;
        }
    }
    let seq = event(
        tx,
        project,
        None,
        "plan",
        "Published an immutable task plan",
        None,
    )?;
    tx.execute(
        "UPDATE task_plans SET data=?1,created_seq=?2 WHERE id=?3",
        params![serde_json::to_string(&plan)?, seq, plan.id],
    )?;
    tx.execute("INSERT INTO task_heads(project,plan_id) VALUES(?1,?2) ON CONFLICT(project) DO UPDATE SET plan_id=excluded.plan_id",params![project,plan.id])?;
    Ok(plan)
}
pub(super) fn invalidate_groups(tx: &Connection, old: &Session, members: &[Member]) -> Result<()> {
    let removed = old
        .plan
        .members
        .iter()
        .filter(|m| {
            !members.iter().any(|new| {
                new.task_id == m.task_id && new.candidate.prerequisites == m.candidate.prerequisites
            })
        })
        .filter_map(|m| old.states[&m.task_id].reservation)
        .collect::<BTreeSet<_>>();
    for group in removed {
        tx.execute(
            "UPDATE task_groups SET status='superseded' WHERE id=?1",
            [group],
        )?;
        for (id, state) in &old.states {
            if state.reservation == Some(group) && state.work == WorkState::Open {
                let mut next = state.clone();
                next.actor = None;
                next.reservation = None;
                next.execution_bundle = None;
                next.version += 1;
                event(
                    tx,
                    &old.project,
                    Some(*id),
                    "reservation-invalidated",
                    "Plan changed this reserved slice",
                    Some(&next),
                )?;
            }
        }
    }
    Ok(())
}

impl Store {
    pub fn publish_tasks(
        &mut self,
        project: &str,
        candidate: &CandidatePlan,
        run: i64,
        expected_head: Option<i64>,
        expected_seq: i64,
        continuity: &BTreeMap<String, String>,
    ) -> Result<Plan> {
        self.require_write()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if head(&tx, project)? != expected_head
            || watermark(&tx, project)? != expected_seq
            || latest_check(&tx, project)? != (run, candidate.snapshot_id.clone())
        {
            return Err(conflict("Plan inputs changed; preview or plan again"));
        }
        let old = expected_head
            .map(|id| session(&tx, project, Some(id), Some(expected_seq)))
            .transpose()?;
        let proposals = reconcile(old.as_ref(), candidate, continuity)?;
        let unchanged = old.as_ref().is_some_and(|s| {
            s.plan.snapshot_id == candidate.snapshot_id
                && s.plan.planner_version == PLANNER_VERSION
                && proposals.len() == s.plan.members.len()
                && proposals.iter().all(|p| {
                    p.reuse.is_some()
                        && s.plan.members.iter().any(|m| {
                            Some(m.task_id) == p.reuse
                                && m.candidate.prerequisites == p.candidate.prerequisites
                        })
                })
        });
        if unchanged {
            let previous = old.unwrap().plan;
            if previous.check_run_id == run {
                return Ok(previous);
            }
            // A fresh successful check is a new occurrence, even for identical
            // requirements. Retain definitions, states and the historical plan;
            // publish only a new immutable plan binding to the current check.
            let plan = store_plan(
                &tx,
                project,
                Plan {
                    id: 0,
                    parent: Some(previous.id),
                    check_run_id: run,
                    ..previous
                },
            )?;
            tx.commit()?;
            return Ok(plan);
        }
        let previous_snapshot = old
            .as_ref()
            .map(|s| {
                tx.query_row(
                    "SELECT data FROM snapshots WHERE id=?1",
                    [&s.plan.snapshot_id],
                    |r| r.get::<_, String>(0),
                )
            })
            .transpose()?
            .map(|s| decode::<Snapshot>(&s))
            .transpose()?;
        let target_data: String = tx.query_row(
            "SELECT data FROM snapshots WHERE id=?1",
            [&candidate.snapshot_id],
            |r| r.get(0),
        )?;
        let target_snapshot: Snapshot = decode(&target_data)?;
        let target_delta = DeltaIndex::new(&target_snapshot);
        let previous_delta = previous_snapshot.as_ref().map(DeltaIndex::new);
        let mut members = vec![];
        for p in proposals {
            let id = if let Some(id) = p.reuse {
                id
            } else {
                let id = insert_task(
                    &tx,
                    project,
                    &p.candidate,
                    &candidate.snapshot_id,
                    run,
                    &p.reason,
                    old.as_ref().map(|s| s.plan.snapshot_id.clone()),
                )?;
                if let (Some(old), Some(delta)) = (&old, &previous_delta) {
                    let mut changes = vec![];
                    for (from, _) in &p.predecessors {
                        if let Some(previous) = old.plan.members.iter().find(|m| m.task_id == *from)
                        {
                            changes.extend(delta.compare(
                                &target_delta,
                                &previous.candidate,
                                &p.candidate,
                            ));
                        }
                    }
                    let data: String =
                        tx.query_row("SELECT data FROM tasks WHERE id=?1", [id], |r| r.get(0))?;
                    let mut definition: Definition = decode(&data)?;
                    definition.changes = changes;
                    tx.execute(
                        "UPDATE tasks SET data=?1 WHERE id=?2",
                        params![serde_json::to_string(&definition)?, id],
                    )?;
                }
                for (from, kind) in &p.predecessors {
                    tx.execute("INSERT INTO task_links(project,from_id,to_id,kind,reason) VALUES(?1,?2,?3,?4,?5)",params![project,from,id,kind,p.reason])?;
                }
                id
            };
            members.push(Member {
                task_id: id,
                candidate: p.candidate,
            });
        }
        if let Some(old) = &old {
            invalidate_groups(&tx, old, &members)?;
        }
        let hash = dctx0_changes::hash(
            &members
                .iter()
                .map(|m| {
                    (
                        &m.candidate.key,
                        &m.candidate.work_order_hash,
                        &m.candidate.prerequisites,
                    )
                })
                .collect::<Vec<_>>(),
        );
        let plan = store_plan(
            &tx,
            project,
            Plan {
                id: 0,
                parent: expected_head,
                snapshot_id: candidate.snapshot_id.clone(),
                check_run_id: run,
                planner_version: PLANNER_VERSION,
                hash,
                members,
                obligations: candidate.obligations.clone(),
            },
        )?;
        tx.commit()?;
        Ok(plan)
    }
    // Historical task context selects its latest membership at the pinned time.
    pub fn task_follow_up(
        &mut self,
        project: &str,
        source: i64,
        finding: Option<&str>,
        reason: &str,
    ) -> Result<Plan> {
        self.task_follow_up_bound(project, source, finding, reason, None)
    }

    pub fn task_renew_review(
        &mut self,
        project: &str,
        source: i64,
        reason: &str,
        source_hash: &str,
    ) -> Result<Plan> {
        self.task_follow_up_bound(project, source, None, reason, Some(source_hash))
    }

    fn task_follow_up_bound(
        &mut self,
        project: &str,
        source: i64,
        finding: Option<&str>,
        reason: &str,
        renewal: Option<&str>,
    ) -> Result<Plan> {
        self.require_write()?;
        if reason.trim().is_empty() {
            return Err(invalid("Follow-up reason is required"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old = session(&tx, project, None, None)?;
        let renewal_origin =
            renewal.map(|hash| format!("Follow-up: T{source}/REVIEW:{hash}: {reason}"));
        if renewal_origin.as_ref().is_some_and(|origin| {
            old.plan
                .members
                .iter()
                .any(|m| old.definitions[&m.task_id].reason == *origin)
        }) {
            return Ok(old.plan);
        }
        let state = old
            .states
            .get(&source)
            .ok_or(dctx0_tasks::Error::Missing(source))?;
        let subjects = if renewal.is_some() {
            let member = old.member(source)?;
            if member.candidate.kind != TaskKind::Review
                || state.work != WorkState::Done
                || state.actor.is_some()
                || state.reservation.is_some()
                || state.claim.is_some()
            {
                return Err(invalid("Only a delivered, unclaimed review can be renewed"));
            }
            vec![source]
        } else if let Some(finding) = finding {
            let matches = state
                .reviews
                .iter()
                .flat_map(|r| &r.findings)
                .filter(|f| f.id == finding && f.action_required)
                .collect::<Vec<_>>();
            if matches.len() != 1 || matches[0].task_ids.is_empty() {
                return Err(invalid("Unknown or ambiguous actionable review finding"));
            }
            matches[0].task_ids.clone()
        } else {
            if state.check != CheckStatus::Fail {
                return Err(invalid(
                    "Follow-up needs a FAIL or an actionable review finding",
                ));
            }
            vec![source]
        };
        let origin = renewal_origin.unwrap_or_else(|| {
            format!(
                "Follow-up: T{source}/{}: {reason}",
                finding.unwrap_or("FAIL")
            )
        });
        if old
            .plan
            .members
            .iter()
            .any(|m| old.definitions[&m.task_id].reason == origin)
        {
            return Ok(old.plan);
        }
        let mut members = old.plan.members.clone();
        for id in subjects {
            let member = old.member(id)?;
            let mut task = member.candidate.clone();
            task.work_order_hash = dctx0_changes::hash(&(&task.requirements_hash, &origin));
            let new = insert_task(
                &tx,
                project,
                &task,
                &old.plan.snapshot_id,
                old.plan.check_run_id,
                &origin,
                Some(old.plan.snapshot_id.clone()),
            )?;
            let data: String =
                tx.query_row("SELECT data FROM tasks WHERE id=?1", [new], |r| r.get(0))?;
            let mut definition: Definition = decode(&data)?;
            definition.adjustment = Some(if let Some(source_hash) = renewal {
                AdjustmentReason::ReviewRenewal {
                    source,
                    source_hash: source_hash.into(),
                }
            } else if let Some(finding) = finding {
                AdjustmentReason::ReviewFinding {
                    source,
                    finding: finding.into(),
                }
            } else {
                AdjustmentReason::FailedCheck { source }
            });
            tx.execute(
                "UPDATE tasks SET data=?1 WHERE id=?2",
                params![serde_json::to_string(&definition)?, new],
            )?;
            tx.execute("INSERT INTO task_links(project,from_id,to_id,kind,reason) VALUES(?1,?2,?3,'follow-up',?4)",params![project,id,new,origin])?;
            members
                .iter_mut()
                .find(|m| m.task_id == id)
                .unwrap()
                .task_id = new;
            members
                .iter_mut()
                .find(|m| m.task_id == new)
                .unwrap()
                .candidate = task;
        }
        // Recreate assemblies/reviews whose work inputs changed; keep other tasks.
        let mut order = (0..members.len()).collect::<Vec<_>>();
        order.sort_by_key(|i| members[*i].candidate.wave);
        for i in order {
            if !matches!(
                members[i].candidate.kind,
                TaskKind::Integration | TaskKind::Review
            ) {
                continue;
            }
            let inputs = members[i]
                .candidate
                .prerequisites
                .iter()
                .map(|p| {
                    (
                        &p.key,
                        &members
                            .iter()
                            .find(|m| m.candidate.key == p.key)
                            .unwrap()
                            .candidate
                            .work_order_hash,
                    )
                })
                .collect::<Vec<_>>();
            let work = dctx0_changes::hash(&(&members[i].candidate.requirements_hash, inputs));
            // Existing integration hashes may use key/hash pairs; compare revisions directly.
            let changed = members[i].candidate.prerequisites.iter().any(|p| {
                members
                    .iter()
                    .find(|m| m.candidate.key == p.key)
                    .unwrap()
                    .task_id
                    != old
                        .plan
                        .members
                        .iter()
                        .find(|m| m.candidate.key == p.key)
                        .unwrap()
                        .task_id
            });
            if changed {
                let from = members[i].task_id;
                members[i].candidate.work_order_hash = work;
                let id = insert_task(
                    &tx,
                    project,
                    &members[i].candidate,
                    &old.plan.snapshot_id,
                    old.plan.check_run_id,
                    "Reintegrate or review explicitly requested remediation",
                    Some(old.plan.snapshot_id.clone()),
                )?;
                members[i].task_id = id;
                tx.execute("INSERT INTO task_links(project,from_id,to_id,kind,reason) VALUES(?1,?2,?3,'update','Remediation changed delivered work')",params![project,from,id])?;
            }
        }
        invalidate_groups(&tx, &old, &members)?;
        let mut plan = old.plan.clone();
        plan.parent = Some(plan.id);
        plan.members = members;
        plan.hash = dctx0_changes::hash(
            &plan
                .members
                .iter()
                .map(|m| (&m.candidate.key, &m.candidate.work_order_hash))
                .collect::<Vec<_>>(),
        );
        let plan = store_plan(&tx, project, plan)?;
        tx.commit()?;
        Ok(plan)
    }
}
