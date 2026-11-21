use super::*;
use crate::Result;
type ProfileInputs<'a> = Option<(
    &'a dctx0_core::TargetManifest,
    &'a [dctx0_core::ProfileDocument],
    &'a Snapshot,
)>;
/// An explicitly authorized result delivery, not native verification authority.
/// Automatic callers must validate execution, coverage and freshness themselves.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskDelivery {
    pub task: i64,
    pub result: TaskResult,
    pub evidence: CheckEvidence,
}
/// Explicit publication inputs; this request is not a native execution permit.
pub struct TaskDeliveryPublication<'a> {
    pub actor: &'a str,
    pub pin: &'a dctx0_core::VerificationTaskPin,
    pub deliveries: &'a [TaskDelivery],
    pub profiles: (
        &'a dctx0_core::TargetManifest,
        &'a [dctx0_core::ProfileDocument],
    ),
    pub run_id: Option<&'a str>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDeliveryPublicationReceipt {
    pub run_id: Option<String>,
    pub request_hash: String,
    pub event_sequence: i64,
    pub deliveries: Vec<Receipt>,
}
fn native_evidence(
    run: &dctx0_core::VerificationRun,
    run_id: &str,
    requirements_hash: &str,
) -> Result<CheckEvidence> {
    let identities = run
        .artifacts
        .iter()
        .filter(|artifact| {
            matches!(
                artifact.media_type.as_str(),
                "application/vnd.dctx0.verifier-identity+json"
                    | "application/vnd.dctx0.runtime-identity+json"
                    | "application/vnd.dctx0.assessment-version+json"
                    | "application/vnd.dctx0.native-limits+json"
                    | "application/vnd.dctx0.dependency-resolution-limits+json"
            )
        })
        .map(|artifact| (&artifact.media_type, &artifact.hash))
        .collect::<BTreeMap<_, _>>();
    let finished = run
        .started_at_unix_ns
        .parse::<u128>()
        .ok()
        .and_then(|started| started.checked_add(u128::from(run.elapsed_us) * 1000))
        .ok_or_else(|| invalid("Invalid native run timing"))?;
    if run.provenance.interpreter_version.trim().is_empty() {
        return Err(invalid("Native run has no interpreter version"));
    }
    let evidence = CheckEvidence {
        requirements_hash: requirements_hash.into(),
        implementation: dctx0_changes::hash(&(&run.provenance, identities)),
        verifier: "dctx0-native-verifier".into(),
        verifier_version: run.provenance.interpreter_version.clone(),
        command: "dctx0 native verification".into(),
        started_at: run.started_at_unix_ns.clone(),
        finished_at: finished.to_string(),
        report: run_id.into(),
        status: CheckStatus::Pass,
    };
    Ok(evidence)
}
fn bind(
    tx: &Connection,
    session: &Session,
    id: i64,
    state: &mut State,
    profiles: ProfileInputs<'_>,
    profile_error: Option<&str>,
) -> Result<()> {
    if state.execution_bundle.is_none() {
        if let Some(AdjustmentReason::Profile { desired, .. }) =
            &session.definitions[&id].adjustment
        {
            state.execution_bundle = Some(desired.clone());
        }
    }
    if state.execution_bundle.is_none() {
        if let Some(error) = profile_error {
            return Err(invalid(error));
        }
        if let Some((manifest, documents, snapshot)) = profiles {
            let selection = select_profiles(
                snapshot,
                &session.member(id)?.candidate,
                manifest,
                documents,
            );
            selection.require_complete()?;
            state.execution_bundle = Some(selection.bundle);
        }
    }
    if let Some(bundle) = &state.execution_bundle {
        tx.execute(
            "INSERT OR IGNORE INTO execution_bundles(hash,data) VALUES(?1,?2)",
            params![bundle.hash, storage::store(tx, bundle)?],
        )?;
    }
    Ok(())
}
fn binding_event(
    tx: &Connection,
    session: &Session,
    id: i64,
    kind: &str,
    message: &str,
    state: &State,
) -> Result<()> {
    let seq = event(tx, &session.project, Some(id), kind, message, Some(state))?;
    if let Some(bundle) = &state.execution_bundle {
        tx.execute("INSERT INTO task_profile_bindings(event_seq,task_id,bundle_hash,claim,reservation) VALUES(?1,?2,?3,?4,?5)",params![seq,id,bundle.hash,state.claim,state.reservation])?;
    }
    Ok(())
}
fn begin(
    tx: &Connection,
    session: &Session,
    id: i64,
    actor: String,
    reservation: Option<i64>,
    profiles: ProfileInputs<'_>,
    profile_error: Option<&str>,
) -> Result<Receipt> {
    let mut state = start_state(
        session,
        id,
        actor,
        reservation,
        watermark(tx, &session.project)? + 1,
    )?;
    bind(tx, session, id, &mut state, profiles, profile_error)?;
    binding_event(tx, session, id, "started", "Task work started", &state)?;
    Ok(Receipt {
        task_id: Some(id),
        group_id: state.reservation,
        state: Some(state),
        message: "Started".into(),
    })
}
fn apply_action(
    tx: &Connection,
    session: &Session,
    action: Action,
    profiles: ProfileInputs<'_>,
    profile_error: Option<&str>,
) -> Result<Receipt> {
    let mut output = Receipt {
        task_id: None,
        group_id: None,
        state: None,
        message: String::new(),
    };
    match action {
        Action::Start {
            task,
            actor,
            reservation,
        } => {
            return begin(
                tx,
                session,
                task,
                actor,
                reservation,
                profiles,
                profile_error,
            );
        }
        Action::ClaimNext { actor, filters } => {
            if actor.trim().is_empty() {
                return Err(invalid("Actor is required"));
            }
            if let Some(row) = session.next(&filters, Some(&actor)) {
                return begin(
                    tx,
                    session,
                    row.task_id,
                    actor,
                    row.reservation,
                    profiles,
                    profile_error,
                );
            }
            output.message =
                "No matching ready open task; inspect tasks list for prerequisites or claims"
                    .into();
        }
        Action::Release {
            task,
            actor,
            claim,
            version,
            reason,
        } => {
            let current = session
                .states
                .get(&task)
                .ok_or(dctx0_tasks::Error::Missing(task))?;
            let state = release_state(
                current,
                &MutationGuard {
                    actor,
                    claim,
                    version,
                },
                &reason,
            )?;
            event(
                tx,
                &session.project,
                Some(task),
                "released",
                &reason,
                Some(&state),
            )?;
            output.task_id = Some(task);
            output.state = Some(state);
            output.message = "Released".into();
        }
        Action::Complete {
            task,
            actor,
            claim,
            version,
            result,
        } => {
            let current = session
                .states
                .get(&task)
                .ok_or(dctx0_tasks::Error::Missing(task))?;
            let state = complete_state(
                &session.definitions[&task],
                current,
                &MutationGuard {
                    actor,
                    claim,
                    version,
                },
                result,
            )?;
            event(
                tx,
                &session.project,
                Some(task),
                "completed",
                "Result delivered; testing remains separate",
                Some(&state),
            )?;
            output.task_id = Some(task);
            output.state = Some(state);
            output.message = "Done; no testing PASS implied".into();
        }
        Action::ClaimSlice {
            anchor,
            actor,
            domain,
        } => {
            if actor.trim().is_empty() {
                return Err(invalid("Actor is required"));
            }
            let slice = session.slice(anchor, domain.as_deref(), Some(&actor))?;
            if !slice.closed {
                return Err(conflict(format!("Slice blocked: {:?}", slice.blockers)));
            }
            if slice.open_work.is_empty() {
                output.message = "Slice has no open work".into();
                return Ok(output);
            }
            if slice
                .open_work
                .iter()
                .all(|id| session.states[id].actor.is_some())
            {
                let groups = slice
                    .open_work
                    .iter()
                    .filter_map(|id| session.states[id].reservation)
                    .collect::<BTreeSet<_>>();
                output.group_id = (groups.len() == 1).then(|| *groups.first().unwrap());
                output.message="Slice work is already owned; continue using each task's existing claim or reservation".into();
                return Ok(output);
            }
            tx.execute("INSERT INTO task_groups(project,plan_id,actor,status,data) VALUES(?1,?2,?3,'active',?4)",params![session.project,session.plan.id,actor,storage::store(tx,&slice)?])?;
            let group = tx.last_insert_rowid();
            for id in &slice.open_work {
                let mut state = session.states[id].clone();
                if state.actor.is_some() {
                    continue;
                }
                state.actor = Some(actor.clone());
                state.reservation = Some(group);
                state.version += 1;
                bind(tx, session, *id, &mut state, profiles, profile_error)?;
                binding_event(
                    tx,
                    session,
                    *id,
                    "reserved",
                    "Reserved without starting future work",
                    &state,
                )?;
            }
            output.group_id = Some(group);
            output.message =
                "Slice reserved; start each task only when its prerequisites are delivered".into();
        }
        Action::ReleaseSlice {
            group,
            actor,
            reason,
        } => {
            if reason.trim().is_empty() {
                return Err(invalid("Slice release reason is required"));
            }
            let owner: Option<String> = tx
                .query_row(
                    "SELECT actor FROM task_groups WHERE id=?1 AND project=?2 AND status='active'",
                    params![group, session.project],
                    |r| r.get(0),
                )
                .optional()?;
            if owner.as_deref() != Some(&actor) {
                return Err(conflict("Unknown slice or different actor"));
            }
            for (id, state) in &session.states {
                if state.reservation == Some(group) && state.work == WorkState::Open {
                    let mut next = state.clone();
                    next.actor = None;
                    next.reservation = None;
                    next.execution_bundle = None;
                    next.version += 1;
                    event(
                        tx,
                        &session.project,
                        Some(*id),
                        "reservation-released",
                        &reason,
                        Some(&next),
                    )?;
                }
            }
            tx.execute(
                "UPDATE task_groups SET status='released' WHERE id=?1",
                [group],
            )?;
            output.group_id = Some(group);
            output.message = "Unstarted tasks released; ongoing work retains its claim".into();
        }
        Action::RecordCheck { task, evidence } => {
            let current = session
                .states
                .get(&task)
                .ok_or(dctx0_tasks::Error::Missing(task))?;
            let state = check_state(&session.definitions[&task], current, evidence)?;
            event(
                tx,
                &session.project,
                Some(task),
                "check",
                "Imported external executed-check report; worktree freshness is not assessed",
                Some(&state),
            )?;
            output.task_id = Some(task);
            output.state = Some(state);
            output.message = "External check evidence recorded".into();
        }
        Action::RecordReview {
            task,
            actor,
            claim,
            version,
            report,
        } => {
            let current = session
                .states
                .get(&task)
                .ok_or(dctx0_tasks::Error::Missing(task))?;
            let known = session.definitions.keys().copied().collect();
            let state = review_state(
                &session.definitions[&task],
                current,
                &MutationGuard {
                    actor,
                    claim,
                    version,
                },
                report,
                &known,
            )?;
            event(
                tx,
                &session.project,
                Some(task),
                "review",
                "Review recorded separately from testing evidence",
                Some(&state),
            )?;
            output.task_id = Some(task);
            output.state = Some(state);
            output.message = "Review recorded; create follow-up explicitly".into();
        }
        Action::Note { task, note } => {
            if !session.definitions.contains_key(&task) {
                return Err(dctx0_tasks::Error::Missing(task).into());
            }
            if note.trim().is_empty() {
                return Err(invalid("Note is required"));
            }
            event(tx, &session.project, Some(task), "note", &note, None)?;
            output.task_id = Some(task);
            output.message = "Note recorded without changing requirements".into();
        }
    }
    Ok(output)
}

impl Store {
    /// Read historical publication evidence without execution or migration.
    /// This does not assert freshness of the current implementation.
    pub fn task_delivery_publication(
        &self,
        project: &str,
        run_id: &str,
    ) -> Result<Option<TaskDeliveryPublicationReceipt>> {
        if read::schema(&self.connection)? < 4 {
            return Ok(None);
        }
        let data: Option<String> = self.connection.query_row(
            "SELECT a.data FROM verification_adoptions a JOIN verification_runs r ON r.id=a.run_id WHERE r.project=?1 AND r.id=?2",
            params![project, run_id], |row| row.get(0),
        ).optional()?;
        data.map(|data| storage::load(&self.connection, &data))
            .transpose()
    }
    /// Apply a guarded, ordered result batch in one transaction using the same
    /// start/complete/check rules as individual actions. No imported report or
    /// `Native` origin flag invokes this operation automatically.
    pub fn deliver_tasks_atomically(
        &mut self,
        project: &str,
        actor: &str,
        pin: &dctx0_core::VerificationTaskPin,
        deliveries: &[TaskDelivery],
        profiles: (&dctx0_core::TargetManifest, &[dctx0_core::ProfileDocument]),
    ) -> Result<Vec<Receipt>> {
        Ok(self
            .publish_task_deliveries(
                project,
                TaskDeliveryPublication {
                    actor,
                    pin,
                    deliveries,
                    profiles,
                    run_id: None,
                },
            )?
            .deliveries)
    }
    /// Persist links and a repeatable receipt together with an explicitly
    /// authorized batch. The engine must establish native authority separately.
    pub fn publish_task_deliveries(
        &mut self,
        project: &str,
        request: TaskDeliveryPublication<'_>,
    ) -> Result<TaskDeliveryPublicationReceipt> {
        self.publish_deliveries(project, request, &BTreeMap::new(), None, false, None)
    }

    /// Explicit inspected delivery, including ordinary guarded review events.
    /// Native automatic publication cannot use this path for review completion.
    pub fn publish_inspected_task_deliveries(
        &mut self,
        project: &str,
        request: TaskDeliveryPublication<'_>,
        reviews: &BTreeMap<i64, dctx0_tasks::ReviewReport>,
    ) -> Result<TaskDeliveryPublicationReceipt> {
        if request.run_id.is_some() {
            return Err(invalid(
                "Inspected delivery is separate from automatic native adoption",
            ));
        }
        self.publish_deliveries(project, request, reviews, None, false, None)
    }

    /// Explicit inspected delivery backed by a separately stored composition.
    /// Rechecks every native origin and latest generation in this same write
    /// transaction as the task/profile/claim and source-bound review guards.
    pub fn publish_composed_task_deliveries(
        &mut self,
        project: &str,
        request: TaskDeliveryPublication<'_>,
        reviews: &BTreeMap<i64, dctx0_tasks::ReviewReport>,
        composition: &super::CompositionTaskPublication<'_>,
    ) -> Result<TaskDeliveryPublicationReceipt> {
        if request.run_id.is_some() {
            return Err(invalid(
                "Composition delivery cannot identify a global native run",
            ));
        }
        self.publish_deliveries(project, request, reviews, None, false, Some(composition))
    }

    /// Atomically publish one native run for open deliveries and previously completed
    /// tasks. The engine must independently check source and runtime freshness.
    /// Done task values are their expected state versions, captured before execution.
    pub fn publish_native_task_batch(
        &mut self,
        project: &str,
        request: TaskDeliveryPublication<'_>,
        done_tasks: &BTreeMap<i64, i64>,
    ) -> Result<TaskDeliveryPublicationReceipt> {
        if request.run_id.is_none() || done_tasks.is_empty() {
            return Err(invalid("Native batch requires a run and completed tasks"));
        }
        self.publish_deliveries(
            project,
            request,
            &BTreeMap::new(),
            Some(done_tasks),
            false,
            None,
        )
    }

    /// Publish explicitly inspected historical tasks together with this fresh
    /// native run. The inspection artifact is loaded from the run in the same
    /// transaction; callers cannot supply a detached or imported approval.
    pub fn publish_inspected_native_task_batch(
        &mut self,
        project: &str,
        request: TaskDeliveryPublication<'_>,
        done_tasks: &BTreeMap<i64, i64>,
    ) -> Result<TaskDeliveryPublicationReceipt> {
        if request.run_id.is_none() || done_tasks.is_empty() {
            return Err(invalid(
                "Inspected native batch requires a run and completed tasks",
            ));
        }
        self.publish_deliveries(
            project,
            request,
            &BTreeMap::new(),
            Some(done_tasks),
            true,
            None,
        )
    }

    fn publish_deliveries(
        &mut self,
        project: &str,
        request: TaskDeliveryPublication<'_>,
        reviews: &BTreeMap<i64, dctx0_tasks::ReviewReport>,
        done_tasks: Option<&BTreeMap<i64, i64>>,
        inspected: bool,
        composition: Option<&super::CompositionTaskPublication<'_>>,
    ) -> Result<TaskDeliveryPublicationReceipt> {
        let TaskDeliveryPublication {
            actor,
            pin,
            deliveries,
            profiles,
            run_id,
        } = request;
        self.require_write()?;
        if actor.trim().is_empty()
            || (deliveries.is_empty() && done_tasks.is_none_or(BTreeMap::is_empty))
        {
            return Err(invalid("An actor and nonempty result batch are required"));
        }
        if reviews
            .keys()
            .any(|id| !deliveries.iter().any(|delivery| delivery.task == *id))
        {
            return Err(invalid(
                "Review reports must belong to the delivered task batch",
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let request_hash = if let Some(done_tasks) = done_tasks {
            dctx0_changes::hash(&(
                if inspected {
                    "inspected-done-batch-v1"
                } else {
                    "native-done-batch-v1"
                },
                project,
                actor,
                pin,
                deliveries,
                profiles,
                run_id,
                done_tasks,
            ))
        } else if reviews.is_empty() {
            dctx0_changes::hash(&(project, actor, pin, deliveries, profiles, run_id))
        } else {
            dctx0_changes::hash(&(project, actor, pin, deliveries, profiles, run_id, reviews))
        };
        let associated_run = if let Some(run_id) = run_id {
            let receipt: Option<String> = tx
                .query_row(
                    "SELECT a.data FROM verification_adoptions a JOIN verification_runs r ON r.id=a.run_id WHERE r.project=?1 AND r.id=?2",
                    params![project,run_id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(receipt) = receipt {
                let receipt: TaskDeliveryPublicationReceipt = storage::load(&tx, &receipt)?;
                if receipt.request_hash != request_hash {
                    return Err(conflict("Run already has a different delivery receipt"));
                }
                return Ok(receipt);
            }
            let run = crate::verification::codec::read_run(&tx, project, run_id)?.run;
            let executed = run
                .cases
                .iter()
                .map(|c| c.id.clone())
                .collect::<BTreeSet<_>>();
            if run.origin != dctx0_core::VerificationRunOrigin::Native
                || (done_tasks.is_some() && run.result != dctx0_core::VerificationResult::Pass)
                || !run.permits_task_specific_proof()
                || !run.provenance.incomplete.is_empty()
                || run.provenance.task_pin.as_ref() != Some(pin)
                || run.required_cases.is_empty()
                || executed != run.required_cases
                || executed.len() != run.cases.len()
                || run.required_cases.iter().any(|id| id.trim().is_empty())
                || run
                    .cases
                    .iter()
                    .any(|c| c.result != dctx0_core::VerificationResult::Pass)
            {
                return Err(invalid(
                    "Run lacks complete pinned execution evidence; imported flags are insufficient",
                ));
            }
            Some(run)
        } else {
            None
        };
        let associated_manifest = if let (Some(run), Some(run_id)) = (&associated_run, run_id) {
            let references = run
                .artifacts
                .iter()
                .filter(|artifact| {
                    artifact.media_type == "application/vnd.dctx0.case-manifest+json"
                })
                .collect::<Vec<_>>();
            if references.len() != 1 {
                return Err(invalid(
                    "Native task delivery requires one mandatory case manifest",
                ));
            }
            let reference = references[0];
            let artifact = crate::verification::codec::read_artifact(&tx, run_id, &reference.hash)?;
            if artifact.reference != *reference {
                return Err(invalid(
                    "Mandatory case manifest artifact integrity does not match its run reference",
                ));
            }
            let bytes = artifact.contents;
            let manifest: dctx0_core::VerificationCaseManifest = serde_json::from_slice(&bytes)
                .map_err(|error| invalid(format!("Invalid mandatory case manifest: {error}")))?;
            if manifest.schema_version != 1
                || manifest.required_cases != run.required_cases
                || manifest
                    .case_requirements
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    != run.required_cases
            {
                return Err(invalid(
                    "Stored mandatory case manifest does not match executed evidence",
                ));
            }
            super::native_proof::validate_open_obligations(&tx, run_id, run, &manifest)?;
            Some(manifest)
        } else {
            None
        };
        let inspected_batch = if inspected {
            let run = associated_run
                .as_ref()
                .ok_or_else(|| invalid("Native run required"))?;
            let run_id = run_id.ok_or_else(|| invalid("Native run required"))?;
            if !run.artifacts.iter().any(|item| {
                item.media_type == dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE
            }) {
                return Err(invalid(
                    "Inspected task evidence requires the complete hybrid proof",
                ));
            }
            let (_, inventory): (_, BTreeMap<String, String>) = super::native_proof::artifact(
                &tx,
                run_id,
                run,
                "application/vnd.dctx0.captured-input-inventory+json",
            )?;
            if inventory != run.provenance.implementation {
                return Err(invalid(
                    "Inspected hybrid source inventory does not match native provenance",
                ));
            }
            let (hash, batch): (_, dctx0_tasks::InspectedDoneBatch) =
                super::native_proof::artifact(
                    &tx,
                    run_id,
                    run,
                    dctx0_tasks::TASK_INSPECTIONS_MEDIA_TYPE,
                )?;
            if batch.snapshot != run.provenance.snapshot
                || batch.check_occurrence != run.provenance.check_occurrence
                || &batch.task_pin != pin
                || !batch.valid_catalog(&run.provenance.implementation)
                || done_tasks
                    .into_iter()
                    .flat_map(|tasks| tasks.keys())
                    .any(|id| !batch.tasks.contains_key(id))
            {
                return Err(invalid(
                    "Inspected task batch does not match the fresh run and task pin",
                ));
            }
            Some((hash, batch))
        } else {
            None
        };
        let initial = session(&tx, project, None, None)?;
        let (check, snapshot_id) = latest_check(&tx, project)?;
        if initial.plan.id != pin.plan_id
            || initial.event_seq != pin.event_sequence
            || initial.plan.check_run_id != check
            || initial.plan.snapshot_id != snapshot_id
        {
            return Err(conflict(
                "Result batch belongs to a stale plan, check or event sequence",
            ));
        }
        let data: String = tx.query_row(
            "SELECT data FROM snapshots WHERE id=?1",
            [&snapshot_id],
            |r| r.get(0),
        )?;
        let snapshot: Snapshot = storage::load(&tx, &data)?;
        if let Some(composition) = composition {
            super::composition_proof::validate(&tx, project, composition, &snapshot, check)?;
        }
        let inputs = Some((profiles.0, profiles.1, &snapshot));
        // Build the reverse mapping once: a large hybrid manifest may contain
        // tens of thousands of cases shared by many inspected tasks.
        let inspected_facts = inspected_batch
            .as_ref()
            .map(|(_, batch)| {
                batch
                    .tasks
                    .values()
                    .flat_map(|task| task.facts.iter().cloned())
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let mut native_fact_cases = BTreeMap::<String, BTreeSet<String>>::new();
        if !inspected_facts.is_empty() {
            if let Some(manifest) = &associated_manifest {
                for (case, requirements) in &manifest.case_requirements {
                    for fact in requirements.intersection(&inspected_facts) {
                        native_fact_cases
                            .entry(fact.clone())
                            .or_default()
                            .insert(case.clone());
                    }
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut receipts = vec![];
        for (id, delivery) in deliveries
            .iter()
            .map(|delivery| (delivery.task, Some(delivery)))
            .chain(
                done_tasks
                    .into_iter()
                    .flat_map(|tasks| tasks.keys().map(|id| (*id, None))),
            )
        {
            // Check-only publications cannot change another task's captured state.
            // The transaction excludes concurrent writers and IDs are distinct.
            let refreshed;
            let current = if deliveries.is_empty() {
                &initial
            } else {
                refreshed = session(&tx, project, None, None)?;
                &refreshed
            };
            let member = current.member(id)?;
            let state = &current.states[&id];
            if !seen.insert(id)
                || pin.requirements.get(&id) != Some(&member.candidate.requirements_hash)
                || match delivery {
                    Some(_) => state.work != WorkState::Open,
                    None => {
                        state.work != WorkState::Done
                            || state.check != CheckStatus::Unverified
                            || state.result.is_none()
                            || done_tasks.and_then(|tasks| tasks.get(&id)) != Some(&state.version)
                            || current.definitions[&id].is_adjustment()
                    }
                }
                || state.actor.is_some()
                || state.claim.is_some()
                || state.reservation.is_some()
            {
                return Err(conflict(
                    "Result batch requires distinct pinned, open and unreserved tasks",
                ));
            }
            let task_inspection = inspected_batch
                .as_ref()
                .and_then(|(_, batch)| batch.tasks.get(&id));
            if let Some(composition) = composition {
                let delivery = delivery
                    .ok_or_else(|| invalid("Composition requires explicit task inspections"))?;
                super::composition_proof::validate_delivery(
                    composition,
                    actor,
                    &member.candidate,
                    delivery,
                    reviews.get(&id),
                    pin.profile_bundles.get(&id),
                )?;
            }
            if let Some(run) = &associated_run {
                let requirements = member
                    .candidate
                    .subjects
                    .iter()
                    .chain(&member.candidate.types)
                    .chain(
                        member
                            .candidate
                            .facts
                            .iter()
                            .filter(|_| task_inspection.is_none()),
                    )
                    .filter(|id| {
                        !snapshot.ir.nodes.iter().any(|node| {
                            node.id == **id && node.kind == dctx0_core::StatementKind::Intent
                        })
                    })
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let outcomes = run
                    .cases
                    .iter()
                    .map(|case| (case.id.clone(), case.result.clone()))
                    .collect::<BTreeMap<_, _>>();
                let covered = associated_manifest.as_ref().is_some_and(|manifest| {
                    task_case_coverage(&requirements, &manifest.case_requirements, &outcomes).result
                        == dctx0_core::VerificationResult::Pass
                });
                if !(matches!(
                    member.candidate.kind,
                    TaskKind::Contract | TaskKind::Operation
                ) || (task_inspection.is_some() && member.candidate.kind == TaskKind::Adapter))
                    || !covered
                    || (done_tasks.is_some() && current.definitions[&id].is_adjustment())
                    || run.provenance.snapshot != snapshot_id
                    || run.provenance.check_occurrence != check
                    || (done_tasks.is_none()
                        && delivery.is_some_and(|delivery| {
                            delivery.evidence.report != run_id.unwrap()
                                || delivery.evidence.status != CheckStatus::Pass
                        }))
                    || member
                        .candidate
                        .subjects
                        .iter()
                        .chain(&member.candidate.types)
                        .any(|subject| !run.required_subjects.contains(subject))
                    || (task_inspection.is_none()
                        && member.candidate.facts.iter().any(|fact| {
                            !snapshot.ir.nodes.iter().any(|node| {
                                node.id == *fact && node.kind == dctx0_core::StatementKind::Intent
                            }) && !associated_manifest.as_ref().is_some_and(|manifest| {
                                manifest
                                    .case_requirements
                                    .values()
                                    .any(|requirements| requirements.contains(fact))
                            })
                        }))
                    || run.provenance.implementation.is_empty()
                    || delivery.is_some_and(|delivery| {
                        delivery.result.artifacts.is_empty()
                            || delivery
                                .result
                                .artifacts
                                .iter()
                                .any(|path| !run.provenance.implementation.contains_key(path))
                    })
                {
                    return Err(invalid(
                        "Delivery is not covered by the exact stored run and implementation artifacts",
                    ));
                }
            }
            if let Some(inspection) = task_inspection {
                let (_, batch) = inspected_batch.as_ref().unwrap();
                let run = associated_run.as_ref().unwrap();
                if inspection.state_version != state.version {
                    return Err(conflict("Inspected historical task version changed"));
                }
                if !batch.validate_task(id, &member.candidate, &run.provenance.implementation) {
                    return Err(invalid(
                        "Inspected task findings or source bindings do not match",
                    ));
                }
                if member.candidate.kind == TaskKind::Adapter {
                    let run_id = run_id.unwrap();
                    let (_, hybrid): (
                        _,
                        dctx0_verification::hybrid_evidence::HybridExecutionProof,
                    ) = super::native_proof::artifact(
                        &tx,
                        run_id,
                        run,
                        dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE,
                    )?;
                    let manifest = associated_manifest.as_ref().unwrap();
                    let adapter_subjects = member
                        .candidate
                        .subjects
                        .iter()
                        .filter(|subject| {
                            manifest
                                .case_requirements
                                .iter()
                                .any(|(case, requirements)| {
                                    requirements.contains(*subject)
                                        && hybrid.application.plan.case_groups.get(case)
                                            == Some(&hybrid.adapter_group)
                                })
                        })
                        .collect::<Vec<_>>();
                    if !adapter_subjects.is_empty() {
                        let (_, description): (_, serde_json::Value) =
                            super::native_proof::artifact(
                                &tx,
                                run_id,
                                run,
                                "application/vnd.dctx0.adapter-description+json",
                            )?;
                        let entries = description
                            .get("subjects")
                            .and_then(serde_json::Value::as_array)
                            .ok_or_else(|| invalid("Adapter description is missing subjects"))?;
                        for subject in adapter_subjects {
                            let matching = entries
                                .iter()
                                .filter(|entry| {
                                    entry["subject"] == **subject && entry["role"] == "port"
                                })
                                .collect::<Vec<_>>();
                            let [entry] = matching.as_slice() else {
                                return Err(invalid(
                                    "Inspected adapter requires one matching port binding",
                                ));
                            };
                            let path = entry["artifact"]
                                .as_str()
                                .filter(|path| !path.is_empty())
                                .ok_or_else(|| {
                                invalid("Adapter binding artifact is missing")
                            })?;
                            if !inspection.sources.contains(path)
                                || !run.provenance.implementation.contains_key(path)
                                || entry["entry_point"].as_str().is_none_or(str::is_empty)
                            {
                                return Err(invalid(
                                    "Inspected adapter must review the actual bound implementation source",
                                ));
                            }
                        }
                    }
                }
            }
            let selection = select_profiles(&snapshot, &member.candidate, profiles.0, profiles.1);
            selection.require_complete()?;
            if pin.profile_bundles.get(&id) != Some(&selection.bundle.hash) {
                return Err(conflict("Result batch profile bundle changed"));
            }
            let Some(delivery) = delivery else {
                let run = associated_run
                    .as_ref()
                    .ok_or_else(|| invalid("Native run required"))?;
                let mut evidence =
                    native_evidence(run, run_id.unwrap(), &member.candidate.requirements_hash)?;
                if let Some(inspection) = task_inspection {
                    let (hash, batch) = inspected_batch.as_ref().unwrap();
                    evidence.verifier = "dctx0-inspected-verifier".into();
                    evidence.command =
                        "dctx0 verify complete hybrid (INSPECTED + actual native)".into();
                    let task_native_fact_cases = member
                        .candidate
                        .facts
                        .iter()
                        .filter_map(|fact| native_fact_cases.get(fact).map(|cases| (fact, cases)))
                        .collect::<BTreeMap<_, _>>();
                    evidence.report = serde_json::to_string(&serde_json::json!({
                        "basis": "INSPECTED + actual native", "native_run": run_id.unwrap(), "actor": batch.actor,
                        "inspection_artifact": hash,
                        "native_fact_cases": task_native_fact_cases,
                        "manual_fact_reviews": inspection.fact_reviews,
                        "limitations": "Finite native case coverage and explicit manual findings are separate evidence bases",
                    }))?;
                }
                let mut next = state.clone();
                next.check = CheckStatus::Pass;
                next.evidence.push(evidence);
                next.version += 1;
                event(
                    &tx,
                    project,
                    Some(id),
                    "native-check",
                    if task_inspection.is_some() {
                        "INSPECTED + actual native evidence recorded; manual findings and finite execution have separate scopes"
                    } else {
                        "Native run evidence recorded; source and runtime freshness require engine verification"
                    },
                    Some(&next),
                )?;
                receipts.push(Receipt {
                    task_id: Some(id),
                    group_id: None,
                    state: Some(next),
                    message: if task_inspection.is_some() {
                        "INSPECTED + actual native evidence recorded"
                    } else {
                        "Native check evidence recorded"
                    }
                    .into(),
                });
                continue;
            };
            let generated_evidence = if done_tasks.is_some() {
                let evidence = native_evidence(
                    associated_run.as_ref().unwrap(),
                    run_id.unwrap(),
                    &member.candidate.requirements_hash,
                )?;
                if delivery.result.implementation.as_ref() != Some(&evidence.implementation) {
                    return Err(invalid(
                        "Open delivery must identify the stored native run provenance",
                    ));
                }
                Some(evidence)
            } else {
                None
            };
            let started = apply_action(
                &tx,
                current,
                Action::Start {
                    task: id,
                    actor: actor.into(),
                    reservation: None,
                },
                inputs,
                None,
            )?;
            let mut state = started
                .state
                .ok_or_else(|| invalid("Started task has no state"))?;
            // Use the claim/version actually created by the start rule.
            let claim = state
                .claim
                .ok_or_else(|| invalid("Started task has no claim"))?;
            if let Some(report) = reviews.get(&id) {
                let current = session(&tx, project, None, None)?;
                state = apply_action(
                    &tx,
                    &current,
                    Action::RecordReview {
                        task: id,
                        actor: actor.into(),
                        claim,
                        version: state.version,
                        report: report.clone(),
                    },
                    inputs,
                    None,
                )?
                .state
                .ok_or_else(|| invalid("Reviewed task has no state"))?;
            }
            let current = session(&tx, project, None, None)?;
            apply_action(
                &tx,
                &current,
                Action::Complete {
                    task: id,
                    actor: actor.into(),
                    claim,
                    version: state.version,
                    result: delivery.result.clone(),
                },
                inputs,
                None,
            )?;
            let current = session(&tx, project, None, None)?;
            receipts.push(apply_action(
                &tx,
                &current,
                Action::RecordCheck {
                    task: id,
                    evidence: generated_evidence.unwrap_or_else(|| delivery.evidence.clone()),
                },
                inputs,
                None,
            )?);
        }
        let receipt = TaskDeliveryPublicationReceipt {
            run_id: run_id.map(str::to_owned),
            request_hash,
            event_sequence: watermark(&tx, project)?,
            deliveries: receipts,
        };
        if let Some(run_id) = run_id {
            for id in deliveries.iter().map(|delivery| delivery.task).chain(
                done_tasks
                    .into_iter()
                    .flat_map(|tasks| tasks.keys().copied()),
            ) {
                tx.execute("INSERT INTO verification_task_links(run_id,task_id,requirements_hash) VALUES(?1,?2,?3)", params![run_id, id, pin.requirements[&id]])?;
            }
            tx.execute(
                "INSERT INTO verification_adoptions(run_id,data) VALUES(?1,?2)",
                params![run_id, storage::store(&tx, &receipt)?],
            )?;
        }
        tx.commit()?;
        Ok(receipt)
    }
    pub fn task_action(&mut self, project: &str, action: Action) -> Result<Receipt> {
        self.task_action_with_profiles(project, action, None)
    }
    pub fn task_action_with_profiles(
        &mut self,
        project: &str,
        action: Action,
        profiles: Option<(&dctx0_core::TargetManifest, &[dctx0_core::ProfileDocument])>,
    ) -> Result<Receipt> {
        self.task_action_profile_input(project, action, profiles, None)
    }
    pub fn task_action_profile_input(
        &mut self,
        project: &str,
        action: Action,
        profiles: Option<(&dctx0_core::TargetManifest, &[dctx0_core::ProfileDocument])>,
        profile_error: Option<&str>,
    ) -> Result<Receipt> {
        self.require_write()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = session(&tx, project, None, None)?;
        let snapshot = if profiles.is_some() {
            let data: String = tx.query_row(
                "SELECT data FROM snapshots WHERE id=?1",
                [&session.plan.snapshot_id],
                |r| r.get(0),
            )?;
            Some(storage::load::<Snapshot>(&tx, &data)?)
        } else {
            None
        };
        let inputs =
            profiles.map(|(manifest, documents)| (manifest, documents, snapshot.as_ref().unwrap()));
        let receipt = apply_action(&tx, &session, action, inputs, profile_error)?;
        tx.commit()?;
        Ok(receipt)
    }
    // Explicit remediation creates new work and a new plan on the same snapshot.
}
