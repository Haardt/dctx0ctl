//! Store an explicit effective assessment without rewriting native observations.
use super::*;
use dctx0_verification::program_inspection::{
    EffectiveProgramAssessment, ProgramInspection, assess_with_inspections,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectedVerificationReport {
    pub native_run: String,
    pub evidence: StoredVerificationRun,
    pub assessment: EffectiveProgramAssessment,
}

impl Project {
    pub fn publish_program_inspections(
        &self,
        application: Option<&Path>,
        native_run: &str,
        inspections: &[ProgramInspection],
    ) -> Result<InspectedVerificationReport> {
        let (application, _) = self.verification_configuration(application)?;
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
        if Capture::inventory(&application)? != inputs {
            return Err(Error::Invalid(
                "Program inspection sources or dependencies changed since execution".into(),
            ));
        }
        let plan_artifact = unique("application/vnd.dctx0.plan+json")?;
        if plan_artifact.reference.hash != run.provenance.plan_hash {
            return Err(Error::Invalid("Native plan hash mismatch".into()));
        }
        let plan: ProgramPlan = serde_json::from_slice(&plan_artifact.contents)?;
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
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.program-inspections+json",
            serde_json::to_vec(inspections)?,
        ));
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.effective-program-assessment+json",
            serde_json::to_vec(&assessment)?,
        ));
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
        let evidence = store.record_verification_run_owned(&self.key(), derived, &artifacts)?;
        Ok(InspectedVerificationReport {
            native_run: native_run.into(),
            evidence,
            assessment,
        })
    }
}
