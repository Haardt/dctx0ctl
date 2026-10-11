//! Explicit agent deliveries. They remain separate from automatic native adoption.
use super::*;
use dctx0_store::{TaskDelivery, TaskDeliveryPublication, TaskDeliveryPublicationReceipt};
use dctx0_tasks::{CheckEvidence, CheckStatus, TaskResult};
use dctx0_verification::attestations::{AttestationBinding, attestation_matches};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAttestation {
    pub task: i64,
    pub inspection: VerificationAttestation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<dctx0_tasks::ReviewReport>,
}

impl Project {
    /// Explicitly publish inspected task results, preserving the native run and
    /// using ordinary guarded start/complete/check events in one transaction.
    /// The caller supplies reviewed reports; this method never manufactures them.
    pub fn publish_task_attestations(
        &self,
        application: Option<&Path>,
        native_run: &str,
        attestations: &[TaskAttestation],
    ) -> Result<TaskDeliveryPublicationReceipt> {
        if attestations.is_empty() {
            return Err(Error::Invalid(
                "Nonempty inspected task reports are required".into(),
            ));
        }
        let (application, _) = self.verification_configuration(application)?;
        let _lock = self.lock()?;
        let saved = self.saved_verification_run(native_run)?;
        let run = &saved.run;
        let actual = run
            .cases
            .iter()
            .map(|case| case.id.clone())
            .collect::<BTreeSet<_>>();
        if run.origin != VerificationRunOrigin::Native
            || !run.permits_task_specific_proof()
            || !run.provenance.incomplete.is_empty()
            || run.required_cases.is_empty()
            || actual != run.required_cases
            || actual.len() != run.cases.len()
            || run
                .cases
                .iter()
                .any(|case| case.result != VerificationResult::Pass)
        {
            return Err(Error::Invalid("Agent delivery requires a complete actual native corpus without failures or provenance gaps".into()));
        }
        let store = self.store(AccessMode::ReadOnly)?;
        let references = run
            .artifacts
            .iter()
            .filter(|reference| {
                reference.media_type == "application/vnd.dctx0.captured-input-inventory+json"
            })
            .collect::<Vec<_>>();
        let [reference] = references.as_slice() else {
            return Err(Error::Invalid(
                "Agent inspection needs the original captured input inventory".into(),
            ));
        };
        let artifact = store.verification_artifact(native_run, &reference.hash)?;
        if VerificationArtifact::new(&reference.media_type, artifact.contents.clone()).reference
            != **reference
        {
            return Err(Error::Invalid(
                "Captured input inventory integrity mismatch".into(),
            ));
        }
        let captured: BTreeMap<String, String> = serde_json::from_slice(&artifact.contents)?;
        if Capture::inventory(&application)? != captured {
            return Err(Error::Invalid(
                "Inspected implementation or dependencies changed since native verification".into(),
            ));
        }
        let session = self.task_session(None, None)?;
        let (check, snapshot_id) = store.task_check(&self.key())?;
        if check != run.provenance.check_occurrence
            || snapshot_id != run.provenance.snapshot
            || session.plan.check_run_id != check
            || session.plan.snapshot_id != snapshot_id
        {
            return Err(Error::Invalid(
                "Agent inspection belongs to a stale specification or task plan".into(),
            ));
        }
        let snapshot = store.load(&self.key(), &snapshot_id)?;
        if dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash {
            return Err(Error::Invalid(
                "Specification sources changed since the published check".into(),
            ));
        }
        let profiles = self
            .profiles(Some(&application), Some(&run.provenance.target))?
            .ok_or_else(|| Error::Invalid("Agent task delivery needs complete profiles".into()))?;
        let actor = &attestations[0].inspection.actor;
        let mut pin = VerificationTaskPin {
            plan_id: session.plan.id,
            event_sequence: session.event_seq,
            requirements: BTreeMap::new(),
            profile_bundles: BTreeMap::new(),
        };
        let mut deliveries = Vec::new();
        let mut reviews = BTreeMap::new();
        for attestation in attestations {
            let member = session.member(attestation.task)?;
            match (&attestation.review, &member.candidate.kind) {
                (Some(review), dctx0_tasks::TaskKind::Review) if review.reviewer == *actor && review.judgement == "approved" && !review.findings.iter().any(|finding| finding.severity == "high" || finding.action_required) => {
                    reviews.insert(attestation.task, review.clone());
                }
                (None, kind) if *kind != dctx0_tasks::TaskKind::Review => {}
                _ => return Err(Error::Invalid("Review delivery requires the actor's structured approval without unresolved high/action findings".into())),
            }
            if member
                .candidate
                .subjects
                .iter()
                .chain(&member.candidate.types)
                .any(|subject| !run.required_subjects.contains(subject))
            {
                return Err(Error::Invalid(
                    "Agent delivery task is outside the verified subject/type closure".into(),
                ));
            }
            let subject = format!("T{}", attestation.task);
            let requirements = member
                .candidate
                .subjects
                .iter()
                .chain(&member.candidate.types)
                .chain(&member.candidate.facts)
                .cloned()
                .collect::<BTreeSet<_>>();
            let binding = AttestationBinding {
                subject: &subject,
                snapshot: &snapshot_id,
                plan_hash: &run.provenance.plan_hash,
                requirements: &requirements,
                implementation_artifacts: &run.provenance.implementation,
            };
            if attestation.inspection.actor != *actor
                || attestation.inspection.result != VerificationResult::Pass
                || !attestation_matches(&attestation.inspection, &binding)
            {
                return Err(Error::Invalid(format!(
                    "{subject}: inspection bindings or verdict are incomplete"
                )));
            }
            let selection = dctx0_tasks::select_profiles(
                &snapshot,
                &member.candidate,
                &profiles.manifest,
                &profiles.documents,
            );
            selection.require_complete()?;
            if run.provenance.task_pin.as_ref().is_none_or(|original| {
                original.plan_id != session.plan.id
                    || original.requirements.get(&attestation.task)
                        != Some(&member.candidate.requirements_hash)
                    || original.profile_bundles.get(&attestation.task)
                        != Some(&selection.bundle.hash)
            }) {
                return Err(Error::Invalid(
                    "Agent inspection lacks matching pinned task/profile requirements".into(),
                ));
            }
            if pin
                .requirements
                .insert(attestation.task, member.candidate.requirements_hash.clone())
                .is_some()
            {
                return Err(Error::Invalid(
                    "Duplicate task attestations are not permitted".into(),
                ));
            }
            pin.profile_bundles
                .insert(attestation.task, selection.bundle.hash);
            let implementation = dctx0_changes::hash(&run.provenance);
            deliveries.push(TaskDelivery {
                task: attestation.task,
                result: TaskResult { summary: attestation.inspection.report.clone(), artifacts: run.provenance.implementation.keys().cloned().collect(), decisions: vec!["Evidence basis: agent inspection with complete native corpus; not an automatic proof".into()], implementation: Some(implementation.clone()) },
                evidence: CheckEvidence { requirements_hash: member.candidate.requirements_hash.clone(), implementation, verifier: actor.clone(), verifier_version: "dctx0-attestation-1".into(), command: "dctx0 verify attest".into(), started_at: run.started_at_unix_ns.clone(), finished_at: SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| Error::Invalid(error.to_string()))?.as_nanos().to_string(), report: serde_json::to_string(&serde_json::json!({"schema_version":1,"basis":"agent","native_run":native_run,"inspection":attestation.inspection}))?, status: CheckStatus::Pass },
            });
        }
        deliveries.sort_by_key(|delivery| {
            session
                .plan
                .members
                .iter()
                .find(|member| member.task_id == delivery.task)
                .map(|member| {
                    (
                        member.candidate.wave,
                        member.candidate.key.clone(),
                        delivery.task,
                    )
                })
                .unwrap_or((usize::MAX, String::new(), delivery.task))
        });
        Ok(self
            .store(AccessMode::ReadWrite)?
            .publish_inspected_task_deliveries(
                &self.key(),
                TaskDeliveryPublication {
                    actor,
                    pin: &pin,
                    deliveries: &deliveries,
                    profiles: (&profiles.manifest, &profiles.documents),
                    run_id: None,
                },
                &reviews,
            )?)
    }
}
