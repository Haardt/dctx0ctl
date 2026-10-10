use super::publish::{insert_task, invalidate_groups, store_plan};
use super::*;
use crate::Result;
impl Store {
    pub fn task_profile_update(
        &mut self,
        project: &str,
        preview: &ProfileUpdatePreview,
        manifest: &dctx0_core::TargetManifest,
        documents: &[dctx0_core::ProfileDocument],
        reason: &str,
    ) -> Result<Plan> {
        self.require_write()?;
        if reason.trim().is_empty() {
            return Err(invalid("Profile update reason is required"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let request_hash = dctx0_changes::hash(&(preview, reason));
        let existing: Option<i64> = tx
            .query_row(
                "SELECT plan_id FROM task_profile_updates WHERE hash=?1 AND project=?2",
                params![request_hash, project],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(session(&tx, project, Some(id), None)?.plan);
        }
        let old = session(&tx, project, None, None)?;
        let (run, checked_snapshot) = latest_check(&tx, project)?;
        if checked_snapshot != old.plan.snapshot_id {
            return Err(conflict(
                "Checked specification changed; replan before adapting profiles",
            ));
        }
        let data: String = tx.query_row(
            "SELECT data FROM snapshots WHERE id=?1",
            [&old.plan.snapshot_id],
            |r| r.get(0),
        )?;
        let snapshot: Snapshot = decode(&data)?;
        let current = profile_changes_at_check(
            &old,
            &snapshot,
            manifest,
            documents,
            preview.selection.clone(),
            run,
        )?;
        if current.hash != preview.hash || profile_preview_hash(preview) != preview.hash {
            return Err(conflict(
                "Profile preview is stale or modified; preview again",
            ));
        }
        if current.selected_tasks.is_empty() {
            return Err(invalid(
                "No completed tasks explicitly selected for profile adaptation",
            ));
        }
        let mut members = old.plan.members.clone();
        let replacements = current
            .selected_tasks
            .iter()
            .chain(&current.revised_assemblies)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut order = (0..members.len()).collect::<Vec<_>>();
        order.sort_by_key(|i| (members[*i].candidate.wave, members[*i].task_id));
        for i in order {
            let source = members[i].task_id;
            if !replacements.contains(&source) {
                continue;
            }
            let change = current
                .changes
                .iter()
                .find(|c| c.task_id == source)
                .unwrap();
            change.desired.require_complete()?;
            let desired = &change.desired.bundle;
            let direct = current.selected_tasks.contains(&source);
            let origin = if direct {
                format!(
                    "Adapt implementation to profile {} ({}): {reason}",
                    desired.target, desired.hash
                )
            } else {
                format!(
                    "Reintegrate or review selected profile adaptations {:?}: {reason}",
                    current.selected_tasks
                )
            };
            let mut candidate = members[i].candidate.clone();
            if direct {
                candidate.work_order_hash = dctx0_changes::hash(&(
                    &candidate.requirements_hash,
                    &desired.hash,
                    &request_hash,
                ));
            } else {
                let inputs = candidate
                    .prerequisites
                    .iter()
                    .map(|p| {
                        let producer = members.iter().find(|m| m.candidate.key == p.key).unwrap();
                        (&p.key, &producer.candidate.work_order_hash)
                    })
                    .collect::<Vec<_>>();
                candidate.work_order_hash =
                    dctx0_changes::hash(&(&candidate.requirements_hash, inputs, &current.hash));
            }
            let id = insert_task(
                &tx,
                project,
                &candidate,
                &old.plan.snapshot_id,
                run,
                &origin,
                Some(old.plan.snapshot_id.clone()),
            )?;
            let data: String =
                tx.query_row("SELECT data FROM tasks WHERE id=?1", [id], |r| r.get(0))?;
            let mut definition: Definition = decode(&data)?;
            definition.adjustment = Some(AdjustmentReason::Profile {
                source,
                desired: desired.clone(),
            });
            tx.execute(
                "UPDATE tasks SET data=?1 WHERE id=?2",
                params![serde_json::to_string(&definition)?, id],
            )?;
            tx.execute("INSERT INTO task_links(project,from_id,to_id,kind,reason) VALUES(?1,?2,?3,'profile-update',?4)",params![project,source,id,origin])?;
            members[i] = Member {
                task_id: id,
                candidate,
            };
        }
        invalidate_groups(&tx, &old, &members)?;
        let mut plan = old.plan.clone();
        plan.parent = Some(plan.id);
        plan.check_run_id = run;
        plan.members = members;
        plan.hash = dctx0_changes::hash(
            &plan
                .members
                .iter()
                .map(|m| (&m.candidate.key, &m.candidate.work_order_hash))
                .collect::<Vec<_>>(),
        );
        let plan = store_plan(&tx, project, plan)?;
        tx.execute(
            "INSERT INTO task_profile_updates(hash,project,plan_id,data) VALUES(?1,?2,?3,?4)",
            params![
                request_hash,
                project,
                plan.id,
                serde_json::to_string(preview)?
            ],
        )?;
        tx.commit()?;
        Ok(plan)
    }
}
