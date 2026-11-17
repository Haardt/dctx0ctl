//! Publish retained complete native evidence without rerunning production.
use super::*;
use dctx0_store::TaskDeliveryPublication;
use dctx0_tasks::{CheckEvidence, CheckStatus, InspectedDoneBatch, WorkState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationCheck {
    pub task: i64,
    pub evidence: CheckEvidence,
}

#[derive(Serialize)]
pub struct NativeTaskReconciliation {
    pub native_run: String,
    pub mandatory_cases: usize,
    pub published_checks: usize,
    pub tasks: usize,
    pub done_pass: usize,
    pub without_pass: usize,
    pub native_cases_executed: usize,
}

impl Project {
    /// The source run remains immutable. All normal native/task publication guards apply.
    pub fn reconcile_native_tasks(
        &self,
        application: &Path,
        run_id: &str,
        checks: &[ReconciliationCheck],
    ) -> Result<NativeTaskReconciliation> {
        let _lock = self.lock()?;
        let (root, config) = self.verification_configuration(Some(application))?;
        let mut store = self.store(AccessMode::ReadWrite)?;
        let saved = store.verification_run(&self.key(), run_id)?;
        let run = &saved.run;
        let scope = store
            .complete_native_task_scope(&self.key(), run_id)?
            .ok_or_else(|| Error::Invalid("Complete native PASS evidence required".into()))?;
        if run.origin != VerificationRunOrigin::Native
            || run.result != VerificationResult::Pass
            || !run.provenance.incomplete.is_empty()
            || !run.open_obligations.is_empty()
            || !store
                .verification_later_nonpass(&self.key(), run_id)?
                .is_empty()
        {
            return Err(Error::Invalid(
                "Native evidence is incomplete or contradicted".into(),
            ));
        }
        let artifact = |media: &str| -> Result<VerificationArtifact> {
            let entries = run
                .artifacts
                .iter()
                .filter(|a| a.media_type == media)
                .collect::<Vec<_>>();
            let [reference] = entries.as_slice() else {
                return Err(Error::Invalid(format!("One {media} artifact required")));
            };
            let artifact = store.verification_artifact(run_id, &reference.hash)?;
            if artifact.reference != **reference {
                return Err(Error::Invalid("Native artifact identity changed".into()));
            }
            Ok(artifact)
        };
        let inventory: BTreeMap<String, String> = serde_json::from_slice(
            &artifact("application/vnd.dctx0.captured-input-inventory+json")?.contents,
        )?;
        if Capture::code_inventory(&root, &inventory)?
            != Capture::inventory_with_budget(&root, config.capture_budgets())?
        {
            return Err(Error::Invalid(
                "Native code changed; affected code requires verification".into(),
            ));
        }
        let batch: InspectedDoneBatch =
            serde_json::from_slice(&artifact(dctx0_tasks::TASK_INSPECTIONS_MEDIA_TYPE)?.contents)?;
        let pin = run
            .provenance
            .task_pin
            .as_ref()
            .ok_or_else(|| Error::Invalid("Native task pin required".into()))?;
        let session = self.task_metadata_session(None, None)?;
        let snapshot = self.load(&session.plan.snapshot_id)?;
        if session.plan.snapshot_id != run.provenance.snapshot
            || session.plan.check_run_id != run.provenance.check_occurrence
            || dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash
        {
            return Err(Error::Invalid(
                "Current specification differs from native evidence".into(),
            ));
        }
        let mut seen = BTreeSet::new();
        for check in checks {
            let member = session.member(check.task)?;
            let state = &session.states[&check.task];
            let report: serde_json::Value = serde_json::from_str(&check.evidence.report)?;
            let requirements = member
                .candidate
                .subjects
                .iter()
                .chain(&member.candidate.types)
                .chain(&member.candidate.facts)
                .cloned()
                .collect::<BTreeSet<_>>();
            let inspected_requirements: BTreeSet<String> =
                serde_json::from_value(report["inspection"]["requirements"].clone())?;
            if !seen.insert(check.task)
                || state.work != WorkState::Done
                || state.actor.is_some()
                || state.claim.is_some()
                || state.reservation.is_some()
                || pin.requirements.get(&check.task) != Some(&member.candidate.requirements_hash)
                || check.evidence.status != CheckStatus::Pass
                || check.evidence.requirements_hash != member.candidate.requirements_hash
                || state
                    .result
                    .as_ref()
                    .and_then(|r| r.implementation.as_ref())
                    != Some(&check.evidence.implementation)
                || !member
                    .candidate
                    .subjects
                    .iter()
                    .chain(&member.candidate.types)
                    .all(|s| scope.contains(s))
                || check.evidence.verifier_version != "dctx0-agent-revalidation-1"
                || report["basis"] != "agent-revalidation"
                || report["native_run"] != run_id
                || report["delivered_implementation"] != check.evidence.implementation
                || report["inspection"]["subject"] != format!("T{}", check.task)
                || report["tested_implementation"] != dctx0_changes::hash(&run.provenance)
                || report["inspection"]["actor"] != check.evidence.verifier
                || report["inspection"]["snapshot"] != run.provenance.snapshot
                || report["inspection"]["plan_hash"] != run.provenance.plan_hash
                || report["inspection"]["result"]["status"] != "PASS"
                || requirements != inspected_requirements
                || report["inspection"]["implementation_artifacts"]
                    != serde_json::to_value(&run.provenance.implementation)?
            {
                return Err(Error::Invalid(
                    "Authored check does not match the delivered task".into(),
                ));
            }
        }
        let mut selected = BTreeMap::new();
        for (id, inspection) in &batch.tasks {
            let member = session.member(*id)?;
            let state = &session.states[id];
            if state.check == CheckStatus::Pass || seen.contains(id) {
                continue;
            }
            if state.work != WorkState::Done || state.version != inspection.state_version {
                return Err(Error::Invalid(format!(
                    "T{id}: native inspection state changed"
                )));
            }
            let _ = member;
            selected.insert(*id, state.version);
        }
        let profiles = self
            .profiles(Some(&root), Some(&config.target))?
            .ok_or_else(|| Error::Invalid("Current profiles required".into()))?;
        if !selected.is_empty() {
            store.publish_inspected_native_task_batch(
                &self.key(),
                TaskDeliveryPublication {
                    actor: "dctx0-verifier",
                    pin,
                    deliveries: &[],
                    profiles: (&profiles.manifest, &profiles.documents),
                    run_id: Some(run_id),
                },
                &selected,
            )?;
        }
        let mut authored_published = 0;
        for check in checks {
            let state = store.task_metadata_session(&self.key(), None, None)?;
            if state.states[&check.task]
                .evidence
                .last()
                .is_some_and(|evidence| {
                    dctx0_changes::hash(evidence) == dctx0_changes::hash(&check.evidence)
                })
            {
                continue;
            }
            self.validate_conformance_action(
                &dctx0_tasks::Action::RecordCheck {
                    task: check.task,
                    evidence: check.evidence.clone(),
                },
                None,
            )?;
            store.task_action(
                &self.key(),
                dctx0_tasks::Action::RecordCheck {
                    task: check.task,
                    evidence: check.evidence.clone(),
                },
            )?;
            authored_published += 1;
        }
        let current = store.task_metadata_session(&self.key(), None, None)?;
        let done_pass = current
            .plan
            .members
            .iter()
            .filter(|member| {
                let state = &current.states[&member.task_id];
                state.work == WorkState::Done && state.check == CheckStatus::Pass
            })
            .count();
        Ok(NativeTaskReconciliation {
            native_run: run_id.into(),
            mandatory_cases: run.required_cases.len(),
            published_checks: selected.len() + authored_published,
            tasks: current.plan.members.len(),
            done_pass,
            without_pass: current.plan.members.len().saturating_sub(done_pass),
            native_cases_executed: 0,
        })
    }
}
