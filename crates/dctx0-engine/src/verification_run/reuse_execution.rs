//! Explicit historical case reuse. A declaration is a cache contract, not sandboxing.
use super::*;
use dctx0_verification::incremental::{
    self as reuse, ExternalInputReceipt, IncrementalPlan, IncrementalProof, ReuseBinding, ReuseUnit,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeReuseOptions {
    pub inputs: String,
    pub parent: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeReuseSummary {
    pub parent_run: Option<String>,
    pub reused_cases: usize,
    pub executed_cases: usize,
    pub required_cases: usize,
    pub adoption_authority: bool,
}
pub(super) struct PreparedReuse {
    options: NativeReuseOptions,
    pub(super) receipt: ExternalInputReceipt,
    pub(super) receipt_artifact: VerificationArtifact,
    pub(super) parent: Option<StoredVerificationRun>,
    pub(super) parent_binding: Option<ReuseBinding>,
    pub(super) units: Vec<ReuseUnit>,
    reused: BTreeSet<String>,
}
impl PreparedReuse {
    pub(super) fn reused_cases(&self) -> &BTreeSet<String> {
        &self.reused
    }
    pub(super) async fn capture(
        project: &Project,
        application: &Path,
        program: &ProgramPlan,
        options: &NativeReuseOptions,
        cancel: &NativeCancellation,
    ) -> Result<Self> {
        let receipt = super::reuse_inputs::capture(application, &options.inputs, cancel).await?;
        let units = reuse::receipt_units(program, &receipt).ok_or_else(|| {
            Error::Invalid(
                "Reuse declaration must select complete known deterministic units".into(),
            )
        })?;
        let receipt_artifact = VerificationArtifact::new(
            reuse::INCREMENTAL_INPUTS_MEDIA_TYPE,
            serde_json::to_vec(&receipt)?,
        );
        let mut parent = None;
        let mut parent_binding = None;
        if let Some(id) = &options.parent {
            let store = project.store(AccessMode::ReadOnly)?;
            let saved = store.verification_run(&project.key(), id)?;
            if let Some(reference) = saved
                .run
                .artifacts
                .iter()
                .find(|a| a.media_type == reuse::INCREMENTAL_INPUTS_MEDIA_TYPE)
            {
                let old = store.verification_artifact(id, &reference.hash)?;
                let old_receipt: ExternalInputReceipt = serde_json::from_slice(&old.contents)?;
                if old_receipt == receipt {
                    let plan = store.verification_artifact(id, &saved.run.provenance.plan_hash)?;
                    let program: ProgramPlan = serde_json::from_slice(&plan.contents)?;
                    parent_binding = Some(reuse::binding(
                        &program,
                        saved.run.provenance.clone(),
                        reference.hash.clone(),
                    )?);
                }
            }
            parent = Some(saved);
        }
        Ok(Self {
            options: options.clone(),
            receipt,
            receipt_artifact,
            parent,
            parent_binding,
            units,
            reused: BTreeSet::new(),
        })
    }
    pub(super) fn select_binding(&mut self, current: &ReuseBinding) -> Result<IncrementalPlan> {
        let required = self
            .units
            .iter()
            .flat_map(|u| u.cases.iter().cloned())
            .collect();
        let plan = match (&self.parent, &self.parent_binding) {
            (Some(parent), Some(old)) => reuse::plan_reuse_projected(
                old,
                current,
                &parent.run,
                &self.units,
                &parent.run.provenance,
            ),
            _ => None,
        }
        .unwrap_or(IncrementalPlan {
            reused: BTreeSet::new(),
            rerun: required,
        });
        self.reused = plan.reused.clone();
        Ok(plan)
    }
    pub(super) fn select(
        &mut self,
        program: &ProgramPlan,
        run: &VerificationRun,
    ) -> Result<IncrementalPlan> {
        let current = reuse::binding(
            program,
            run.provenance.clone(),
            self.receipt_artifact.reference.hash.clone(),
        )?;
        self.select_binding(&current)
    }
    pub(super) fn summary(
        &self,
        program: &ProgramPlan,
        run: &VerificationRun,
    ) -> NativeReuseSummary {
        NativeReuseSummary {
            parent_run: self.options.parent.clone(),
            reused_cases: self.reused.len(),
            executed_cases: run
                .cases
                .iter()
                .filter(|c| !self.reused.contains(&c.id))
                .count(),
            required_cases: dctx0_verification::case_manifest(program)
                .required_cases
                .len(),
            adoption_authority: false,
        }
    }
    /// A failed native cohort cannot retain copied historical observations as
    /// evidence for this attempt. Preserve only cases actually executed here.
    pub(super) fn discard_cached_records(&mut self, run: &mut VerificationRun) {
        run.cases.retain(|case| !self.reused.contains(&case.id));
        self.reused.clear();
    }
    pub(super) async fn finish_binding(
        &self,
        application: &Path,
        program: &ProgramPlan,
        run: &VerificationRun,
        current: ReuseBinding,
        artifacts: &mut Vec<VerificationArtifact>,
        cancel: &NativeCancellation,
    ) -> Result<()> {
        if super::reuse_inputs::capture(application, &self.options.inputs, cancel).await?
            != self.receipt
        {
            return Err(Error::Invalid("INCREMENTAL_INPUTS_CHANGED: declared files, fixture revisions or environment changed during execution".into()));
        }
        let expected = match (&self.parent, &self.parent_binding) {
            (Some(parent), Some(old)) => reuse::plan_reuse_projected(
                old,
                &current,
                &parent.run,
                &self.units,
                &parent.run.provenance,
            ),
            _ => None,
        };
        if !self.reused.is_empty() && expected.as_ref().is_none_or(|p| p.reused != self.reused) {
            return Err(Error::Invalid(
                "INCREMENTAL_BINDING_CHANGED: execution identity changed after case selection"
                    .into(),
            ));
        }
        artifacts.push(self.receipt_artifact.clone());
        if !self.reused.is_empty() {
            let parent = self.parent.as_ref().expect("reused records have a parent");
            artifacts.push(VerificationArtifact::new(
                reuse::INCREMENTAL_PROOF_MEDIA_TYPE,
                serde_json::to_vec(&IncrementalProof {
                    parent_run: parent.id.clone(),
                    binding: current,
                    units: self.units.clone(),
                    plan: expected.expect("checked plan"),
                })?,
            ));
        }
        artifacts.push(VerificationArtifact::new("application/vnd.dctx0.incremental-summary+json",serde_json::to_vec(&serde_json::json!({"schema_version":1,"parent_run":self.options.parent,"reused_cases":self.reused.len(),"executed_cases":run.cases.iter().filter(|c|!self.reused.contains(&c.id)).count(),"required_cases":dctx0_verification::case_manifest(program).required_cases.len(),"basis":"pre-adoption execution summary; historical native cases under explicit declared input bindings are not fresh external observations","adoption_authority":false}))?));
        Ok(())
    }
    pub(super) async fn finish(
        &self,
        application: &Path,
        program: &ProgramPlan,
        run: &VerificationRun,
        artifacts: &mut Vec<VerificationArtifact>,
        cancel: &NativeCancellation,
    ) -> Result<()> {
        let binding = reuse::binding(
            program,
            run.provenance.clone(),
            self.receipt_artifact.reference.hash.clone(),
        )?;
        self.finish_binding(application, program, run, binding, artifacts, cancel)
            .await
    }
}
