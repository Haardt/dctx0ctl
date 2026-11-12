//! Store an explicit effective assessment without rewriting native observations.
use super::*;
use dctx0_verification::attestations::{AttestationBinding, attestation_matches};
use dctx0_verification::program_inspection::{
    EffectiveProgramAssessment, ProgramInspection, assess_with_inspections,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectedVerificationReport {
    pub native_run: String,
    pub evidence: StoredVerificationRun,
    pub assessment: EffectiveProgramAssessment,
}

/// Apply only explicitly authored inspections to this still-locked, freshly
/// executed hybrid occurrence. Native observations and original provenance
/// remain unchanged; Store independently recomputes the effective assessment.
pub(super) fn apply_fresh_hybrid_inspections(
    project: &Project,
    application: &Path,
    snapshot: &dctx0_changes::Snapshot,
    plan: &ProgramPlan,
    run: &mut VerificationRun,
    artifacts: &mut Vec<VerificationArtifact>,
    notes: &crate::verification_complete::AuthoredInspections,
) -> Result<()> {
    if run.origin != VerificationRunOrigin::Native
        || !run.provenance.incomplete.is_empty()
        || !run.permits_task_specific_proof()
        || run.required_cases.is_empty()
        || run
            .cases
            .iter()
            .map(|case| case.id.clone())
            .collect::<BTreeSet<_>>()
            != run.required_cases
        || run.cases.len() != run.required_cases.len()
        || run
            .cases
            .iter()
            .any(|case| case.result != VerificationResult::Pass)
    {
        return Ok(());
    }
    let manifest = dctx0_verification::case_manifest(plan);
    if manifest.required_cases != run.required_cases
        || run
            .open_obligations
            .iter()
            .any(|issue| !manifest.open_obligations.contains(issue))
        || snapshot.id != run.provenance.snapshot
        || plan.snapshot != run.provenance.snapshot
        || i64::try_from(plan.check_occurrence).ok() != Some(run.provenance.check_occurrence)
    {
        return Err(Error::Invalid(
            "Inspections require the exact original complete native plan".into(),
        ));
    }
    let plans = artifacts
        .iter()
        .filter(|artifact| artifact.reference.media_type == "application/vnd.dctx0.plan+json")
        .collect::<Vec<_>>();
    let [stored_plan] = plans.as_slice() else {
        return Err(Error::Invalid(
            "One native plan artifact is required".into(),
        ));
    };
    if stored_plan.reference.hash != run.provenance.plan_hash
        || stored_plan.contents != serde_json::to_vec(plan)?
    {
        return Err(Error::Invalid(
            "Inspected plan does not match the executed artifact".into(),
        ));
    }
    let hybrid = artifacts
        .iter()
        .filter(|artifact| {
            artifact.reference.media_type
                == dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE
        })
        .collect::<Vec<_>>();
    let [hybrid] = hybrid.as_slice() else {
        return Err(Error::Invalid(
            "One fresh hybrid execution proof is required".into(),
        ));
    };
    let proof: dctx0_verification::hybrid_evidence::HybridExecutionProof =
        serde_json::from_slice(&hybrid.contents)?;
    if !proof.validates(run) {
        return Err(Error::Invalid(
            "Hybrid execution proof is incomplete or mismatched".into(),
        ));
    }
    if !notes.subjects.is_empty() {
        notes.validate(application)?;
    }
    notes.validate_spec(project, application)?;
    if let Some(batch) = &notes.done_inspections {
        if batch.snapshot != run.provenance.snapshot
            || batch.check_occurrence != run.provenance.check_occurrence
            || run.provenance.task_pin.as_ref() != Some(&batch.task_pin)
            || !batch.valid_catalog(&run.provenance.implementation)
            || batch
                .sources
                .iter()
                .any(|(path, hash)| notes.reviewed_hashes.get(path) != Some(hash))
        {
            return Err(Error::Invalid(
                "Inspected task source or provenance changed".into(),
            ));
        }
        let session = project.task_metadata_session(None, None)?;
        for id in batch.tasks.keys() {
            if !batch.validate_task(
                *id,
                &session.member(*id)?.candidate,
                &run.provenance.implementation,
            ) {
                return Err(Error::Invalid(format!(
                    "T{id}: incomplete inspected task findings"
                )));
            }
        }
        artifacts.push(VerificationArtifact::new(
            dctx0_tasks::TASK_INSPECTIONS_MEDIA_TYPE,
            serde_json::to_vec(batch)?,
        ));
    }
    let inspections = notes.program_inspections(plan, run);
    let assessment = assess_with_inspections(
        &snapshot.ir,
        plan,
        &run.cases,
        &run.provenance.plan_hash,
        &run.provenance.implementation,
        &inspections,
    );
    if assessment.result != VerificationResult::Pass || !assessment.open_obligations.is_empty() {
        return Ok(());
    }
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.program-inspections+json",
        serde_json::to_vec(&inspections)?,
    ));
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.effective-program-assessment+json",
        serde_json::to_vec(&assessment)?,
    ));
    run.open_obligations = assessment.open_obligations;
    run.result = assessment.result;
    Ok(())
}

impl Project {
    pub fn publish_program_inspections(
        &self,
        application: Option<&Path>,
        native_run: &str,
        inspections: &[ProgramInspection],
    ) -> Result<InspectedVerificationReport> {
        let (application, config) = self.verification_configuration(application)?;
        let _lock = self.lock()?;
        let mut store = self.store(AccessMode::ReadWrite)?;
        let source = store.verification_run(&self.key(), native_run)?;
        let run = &source.run;
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
            return Err(Error::Invalid(
                "Program inspections require complete actual native evidence without failures"
                    .into(),
            ));
        }
        let mut artifacts = run
            .artifacts
            .iter()
            .map(|reference| {
                let artifact = store.verification_artifact(native_run, &reference.hash)?;
                if artifact.reference != *reference {
                    return Err(Error::Invalid("Native artifact reference mismatch".into()));
                }
                Ok(artifact)
            })
            .collect::<Result<Vec<_>>>()?;
        let unique = |media_type: &str| -> Result<&VerificationArtifact> {
            let matches = artifacts
                .iter()
                .filter(|artifact| artifact.reference.media_type == media_type)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [artifact] => Ok(*artifact),
                _ => Err(Error::Invalid(format!(
                    "One {media_type} artifact is required"
                ))),
            }
        };
        let inputs: BTreeMap<String, String> = serde_json::from_slice(
            &unique("application/vnd.dctx0.captured-input-inventory+json")?.contents,
        )?;
        if Capture::proof_inventory(&application, config.capture_budgets(), run)? != inputs {
            return Err(Error::Invalid(
                "Program inspection sources or dependencies changed since execution".into(),
            ));
        }
        let plan_artifact = unique("application/vnd.dctx0.plan+json")?;
        if plan_artifact.reference.hash != run.provenance.plan_hash {
            return Err(Error::Invalid("Native plan hash mismatch".into()));
        }
        let plan: ProgramPlan = serde_json::from_slice(&plan_artifact.contents)?;
        let mut subjects = BTreeSet::new();
        for report in inspections {
            let inspection = &report.inspection;
            let requirements = dctx0_verification::program_inspection::inspection_requirements(
                &plan,
                &inspection.subject,
            )
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "Unknown inspection obligation: {}",
                    inspection.subject
                ))
            })?;
            let binding = AttestationBinding {
                subject: &inspection.subject,
                snapshot: &plan.snapshot,
                plan_hash: &run.provenance.plan_hash,
                requirements: &requirements,
                implementation_artifacts: &run.provenance.implementation,
            };
            if !subjects.insert(&inspection.subject) || !attestation_matches(inspection, &binding) {
                return Err(Error::Invalid(format!(
                    "Invalid binding or duplicate inspection: {}",
                    inspection.subject
                )));
            }
        }
        let (check, snapshot_id) = store.task_check(&self.key())?;
        if check != run.provenance.check_occurrence || snapshot_id != run.provenance.snapshot {
            return Err(Error::Invalid(
                "Program inspection belongs to a historical check".into(),
            ));
        }
        let snapshot = store.load(&self.key(), &snapshot_id)?;
        if dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash {
            return Err(Error::Invalid(
                "Specification changed since its published check".into(),
            ));
        }
        if let Some(pin) = &run.provenance.task_pin {
            let session = self.task_session(None, None)?;
            let profiles = self
                .profiles(Some(&application), Some(&run.provenance.target))?
                .ok_or_else(|| Error::Invalid("Inspection profiles unavailable".into()))?;
            if session.plan.id != pin.plan_id {
                return Err(Error::Invalid("Inspected task plan changed".into()));
            }
            for (id, hash) in &pin.requirements {
                let member = session.member(*id)?;
                let selection = dctx0_tasks::select_profiles(
                    &snapshot,
                    &member.candidate,
                    &profiles.manifest,
                    &profiles.documents,
                );
                selection.require_complete()?;
                if *hash != member.candidate.requirements_hash
                    || pin.profile_bundles.get(id) != Some(&selection.bundle.hash)
                {
                    return Err(Error::Invalid(
                        "Inspected task/profile requirements changed".into(),
                    ));
                }
            }
        }
        let assessment = assess_with_inspections(
            &snapshot.ir,
            &plan,
            &run.cases,
            &run.provenance.plan_hash,
            &run.provenance.implementation,
            inspections,
        );
        for added in [
            VerificationArtifact::new(
                "application/vnd.dctx0.program-inspections+json",
                serde_json::to_vec(inspections)?,
            ),
            VerificationArtifact::new(
                "application/vnd.dctx0.effective-program-assessment+json",
                serde_json::to_vec(&assessment)?,
            ),
        ] {
            if !artifacts
                .iter()
                .any(|existing| existing.reference == added.reference)
            {
                artifacts.push(added);
            }
        }
        artifacts.push(VerificationArtifact::new("application/vnd.dctx0.native-parent+json",serde_json::to_vec(&serde_json::json!({"schema_version":1,"native_run":native_run,"native_started_at_unix_ns":run.started_at_unix_ns,"basis":"explicit-inspection","execution":"reused native observations; no new execution"}))?));
        let mut derived = source.run;
        // Imported inspection evidence has no automatic task-adoption authority.
        derived.origin = VerificationRunOrigin::Imported;
        derived.elapsed_us = 0;
        derived.open_obligations = assessment.open_obligations.clone();
        derived.result = assessment.result.clone();
        derived.artifacts = artifacts
            .iter()
            .map(|artifact| artifact.reference.clone())
            .collect();
        preserve_verification_history(&config, &mut derived, &mut artifacts);
        let evidence = store.record_verification_run_owned(&self.key(), derived, &artifacts)?;
        Ok(InspectedVerificationReport {
            native_run: native_run.into(),
            evidence,
            assessment,
        })
    }
}
