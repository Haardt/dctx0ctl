//! Native lifecycle receipts are emitted by the orchestrator, never by a task report.
use super::*;

pub(in crate::verification_run) struct UnitNativeContext {
    pub planned: UnitEvidenceBindingV1,
    pub group: DecoderUnitGroup,
    pub recipe_go: PathBuf,
    pub recipe_files: BTreeMap<String, Vec<u8>>,
    attempt: UnitAttemptV1,
    completion: Option<UnitNativeCompletionV1>,
}

impl UnitNativeContext {
    pub(super) fn new(project: &Project, unit: &PreparedUnit) -> Result<Self> {
        let attempt = project
            .store(AccessMode::ReadWrite)?
            .begin_unit_attempt(&project.key(), &unit.binding)?;
        Ok(Self {
            planned: unit.binding.clone(),
            group: unit.group.clone(),
            recipe_go: unit.recipe.go.clone(),
            recipe_files: unit.recipe.files.clone(),
            attempt,
            completion: None,
        })
    }
    pub(in crate::verification_run) fn bind_compilation(
        &mut self,
        project: &Project,
        receipt: &build::GoBuildReceipt,
        generator: &dyn HarnessGenerator,
        artifacts: &mut Vec<VerificationArtifact>,
    ) -> Result<()> {
        let binding = binding::compiled(self, receipt, generator)?;
        if unit_planner::code_fingerprint(&binding)? != self.attempt.fingerprint {
            self.fail(project)?;
            self.attempt = project
                .store(AccessMode::ReadWrite)?
                .begin_unit_attempt(&project.key(), &binding)?;
        }
        self.planned = binding;
        artifacts.push(VerificationArtifact::new(
            UNIT_BINDING_MEDIA_TYPE,
            serde_json::to_vec(&self.planned)?,
        ));
        Ok(())
    }
    pub(in crate::verification_run) fn complete(
        &mut self,
        run: &VerificationRun,
        artifacts: &mut Vec<VerificationArtifact>,
    ) -> Result<()> {
        if run.result != VerificationResult::Pass
            || !run.provenance.incomplete.is_empty()
            || !run.open_obligations.is_empty()
            || run.required_cases != self.planned.unit.cases
            || run.cases.len() != run.required_cases.len()
            || run
                .cases
                .iter()
                .any(|case| case.result != VerificationResult::Pass)
            || !artifacts
                .iter()
                .any(|a| a.reference.media_type == UNIT_BINDING_MEDIA_TYPE)
        {
            return Ok(());
        }
        let completion = UnitNativeCompletionV1 {
            schema_version: 1,
            unit: self.group.id.clone(),
            fingerprint: unit_planner::evidence_digest(&self.planned)?,
            generation: self.attempt.generation,
            completed: true,
            cleanup_completed: true,
            dependencies_complete: true,
            case_bodies: run
                .cases
                .iter()
                .map(|case| Ok((case.id.clone(), unit_planner::evidence_digest(case)?)))
                .collect::<Result<_>>()?,
        };
        artifacts.push(VerificationArtifact::new(
            UNIT_COMPLETION_MEDIA_TYPE,
            serde_json::to_vec(&completion)?,
        ));
        self.completion = Some(completion);
        Ok(())
    }
    pub(super) fn fail(&self, project: &Project) -> Result<()> {
        Ok(project
            .store(AccessMode::ReadWrite)?
            .finish_nonpass_unit_attempt(
                &project.key(),
                &self.attempt,
                UnitAttemptOutcome::Unknown,
            )?)
    }
    pub(super) fn publish(
        &self,
        project: &Project,
        report: &NativeVerificationReport,
    ) -> Result<(
        ComposedUnitEvidenceV1,
        BTreeMap<String, usize>,
        Option<usize>,
    )> {
        let mut store = project.store(AccessMode::ReadWrite)?;
        let mut calls = BTreeMap::new();
        let mut starts = 0;
        let mut audited = false;
        for reference in report
            .evidence
            .run
            .artifacts
            .iter()
            .filter(|a| a.media_type == "application/vnd.dctx0.native-audit+json")
        {
            let artifact = store.verification_artifact(&report.evidence.id, &reference.hash)?;
            let audit: crate::NativeAudit = serde_json::from_slice(&artifact.contents)?;
            audited = true;
            for line in audit.stderr.lines() {
                if let Some(subject) = line.strip_prefix("DCTX0_UNIT_PRODUCTION_CALL:") {
                    *calls.entry(subject.to_owned()).or_insert(0) += 1;
                }
                if line == "DCTX0_UNIT_WORKER_START" {
                    starts += 1;
                }
            }
        }
        let Some(completion) = &self.completion else {
            let outcome = if matches!(report.evidence.run.result, VerificationResult::Fail { .. }) {
                UnitAttemptOutcome::Fail
            } else {
                UnitAttemptOutcome::Unknown
            };
            store.finish_nonpass_unit_attempt(&project.key(), &self.attempt, outcome)?;
            return Ok((
                ComposedUnitEvidenceV1 {
                    binding: self.planned.clone(),
                    coverage: if outcome == UnitAttemptOutcome::Fail {
                        UnitCoverage::Fail
                    } else {
                        UnitCoverage::Unknown
                    },
                    proof: None,
                },
                calls,
                audited.then_some(starts),
            ));
        };
        let proof = UnitCompletionProofV1 {
            schema_version: 1,
            native_run: report.evidence.id.clone(),
            unit: self.group.id.clone(),
            fingerprint: completion.fingerprint.clone(),
            generation: completion.generation,
            completed: true,
            cleanup_completed: true,
            dependencies_complete: true,
            case_bodies: completion.case_bodies.clone(),
            artifacts: report
                .evidence
                .run
                .artifacts
                .iter()
                .map(|a| a.hash.clone())
                .collect(),
        };
        store.retain_unit_sources(
            &project.key(),
            &format!("unit-origin:{}:{}", self.group.id, proof.generation),
            &[proof.native_run.clone()].into(),
        )?;
        store.record_unit_origin(&project.key(), self.planned.clone(), proof.clone())?;
        Ok((
            ComposedUnitEvidenceV1 {
                binding: self.planned.clone(),
                coverage: UnitCoverage::FreshPass,
                proof: Some(proof),
            },
            calls,
            audited.then_some(starts),
        ))
    }
}
