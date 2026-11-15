//! Explicit historical case reuse. A declaration is a cache contract, not sandboxing.
use super::*;
use dctx0_verification::incremental::{
    self as reuse, ExternalInputReceipt, IncrementalPlan, IncrementalProof, ReuseBinding, ReuseUnit,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeReuseMode {
    #[default]
    Auto,
    Off,
    Require,
}
impl std::str::FromStr for NativeReuseMode {
    type Err = String;
    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "off" => Ok(Self::Off),
            "require" => Ok(Self::Require),
            _ => Err("Reuse mode must be auto, off or require".into()),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeReuseOptions {
    #[serde(default)]
    pub mode: NativeReuseMode,
    #[serde(default)]
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
    #[serde(default)]
    pub decisions: Vec<reuse::UnitDecision>,
}
pub(super) struct PreparedReuse {
    options: NativeReuseOptions,
    pub(super) receipt: ExternalInputReceipt,
    pub(super) receipt_artifact: VerificationArtifact,
    pub(super) parent: Option<StoredVerificationRun>,
    pub(super) parent_binding: Option<ReuseBinding>,
    pub(super) units: Vec<ReuseUnit>,
    reused: BTreeSet<String>,
    decisions: Vec<reuse::UnitDecision>,
    parent_receipt: Option<ExternalInputReceipt>,
    contrary: BTreeSet<String>,
}
impl PreparedReuse {
    pub(super) fn enabled(&self) -> bool {
        self.options.mode != NativeReuseMode::Off
    }
    pub(super) fn input_issues(&self) -> Vec<VerificationIssue> {
        self.receipt
            .incomplete
            .iter()
            .map(|gap| issue(&gap.code, &gap.input))
            .collect()
    }
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
        let mut options = options.clone();
        if options.mode == NativeReuseMode::Off && options.parent.is_some() {
            return Err(Error::Invalid(
                "--reuse=off conflicts with --reuse-from".into(),
            ));
        }
        if options.inputs.is_empty()
            && application
                .join(".dctx0/testing/reuse-inputs.json")
                .exists()
        {
            options.inputs = ".dctx0/testing/reuse-inputs.json".into();
        }
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
        let mut parent_receipt = None;
        let mut contrary = BTreeSet::new();
        if options.parent.is_none() && options.mode != NativeReuseMode::Off {
            options.parent = project
                .store(AccessMode::ReadOnly)?
                .verification_reuse_candidate(&project.key(), &program.scope.subjects)?;
        }
        if let Some(id) = &options.parent {
            let store = project.store(AccessMode::ReadOnly)?;
            let saved = store.verification_run(&project.key(), id)?;
            contrary = store.verification_later_nonpass(&project.key(), id)?;
            if let Some(reference) = saved
                .run
                .artifacts
                .iter()
                .find(|a| a.media_type == reuse::INCREMENTAL_INPUTS_MEDIA_TYPE)
            {
                let old = store.verification_artifact(id, &reference.hash)?;
                let old_receipt: ExternalInputReceipt = serde_json::from_slice(&old.contents)?;
                parent_receipt = Some(old_receipt);
                let plan = store.verification_artifact(id, &saved.run.provenance.plan_hash)?;
                let program: ProgramPlan = serde_json::from_slice(&plan.contents)?;
                parent_binding = Some(reuse::binding(
                    &program,
                    saved.run.provenance.clone(),
                    reference.hash.clone(),
                )?);
            }
            parent = Some(saved);
        }
        Ok(Self {
            options,
            receipt,
            receipt_artifact,
            parent,
            parent_binding,
            units,
            reused: BTreeSet::new(),
            decisions: Vec::new(),
            parent_receipt,
            contrary,
        })
    }
    pub(super) fn select_binding(&mut self, current: &ReuseBinding) -> Result<IncrementalPlan> {
        let required = self
            .units
            .iter()
            .flat_map(|u| u.cases.iter().cloned())
            .collect();
        let plan = match (&self.parent, &self.parent_binding, self.options.mode) {
            (Some(parent), Some(old), mode)
                if mode != NativeReuseMode::Off && self.contrary.is_empty() =>
            {
                reuse::plan_reuse_projected(
                    old,
                    current,
                    &parent.run,
                    &self.units,
                    &parent.run.provenance,
                )
            }
            _ => None,
        }
        .unwrap_or(IncrementalPlan {
            reused: BTreeSet::new(),
            rerun: required,
        });
        self.decisions = reuse::unit_decisions(
            &self.units,
            &plan,
            self.parent_binding.as_ref(),
            current,
            self.options.mode == NativeReuseMode::Off,
        );
        if let Some(old) = &self.parent_receipt {
            let changes = reuse::receipt_changes(old, &self.receipt);
            for decision in &mut self.decisions {
                if decision.decision != reuse::UnitAction::Reuse {
                    decision.reasons.extend(changes.clone());
                }
            }
        }
        for decision in &mut self.decisions {
            decision.reasons.extend(
                self.receipt
                    .incomplete
                    .iter()
                    .map(|gap| reuse::ReuseReason {
                        code: gap.code.clone(),
                        input: gap.input.clone(),
                    }),
            );
            decision.reasons.dedup();
        }
        if self.options.mode != NativeReuseMode::Off
            && let Some(parent) = &self.parent
        {
            let causes = reuse::reuse_blocking_issues(&parent.run)
                .map(|issue| reuse::ReuseReason {
                    code: "PARENT_GROUP_INCOMPLETE".into(),
                    input: issue.code.clone(),
                })
                .collect::<Vec<_>>();
            for decision in &mut self.decisions {
                decision.reasons.extend(causes.clone());
            }
        }
        if !self.contrary.is_empty() && self.options.mode != NativeReuseMode::Off {
            let cause = self
                .contrary
                .first()
                .expect("nonempty contrary observations");
            for decision in &mut self.decisions {
                decision.decision = reuse::UnitAction::ExecuteChanged;
                decision.reasons.push(reuse::ReuseReason {
                    code: "LATER_NONPASS_OBSERVATION".into(),
                    input: format!("parent-cohort:{cause}"),
                });
            }
        }
        eprintln!(
            "Verification inventory: {} required cases; {} reusable; {} to execute",
            plan.reused.len() + plan.rerun.len(),
            plan.reused.len(),
            plan.rerun.len()
        );
        if self.options.mode == NativeReuseMode::Require
            && self.parent.is_some()
            && plan.reused.is_empty()
        {
            return Err(Error::Invalid(format!(
                "REUSE_REQUIRED: no compatible units: {}",
                serde_json::to_string(&self.decisions)?
            )));
        }
        dctx0_verification::unit_planner::execution_gate(
            &plan.reused.union(&plan.rerun).cloned().collect(),
            &plan.reused,
            self.options.mode == NativeReuseMode::Off,
            self.options.mode == NativeReuseMode::Require,
            self.parent.is_some(),
        )
        .map_err(|code| {
            Error::Invalid(format!(
                "{code}: {}",
                serde_json::to_string(&self.decisions).unwrap_or_default()
            ))
        })?;
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
            decisions: self.decisions.clone(),
        }
    }
    /// A failed native cohort cannot retain copied historical observations as
    /// evidence for this attempt. Preserve only cases actually executed here.
    pub(super) fn discard_cached_records(&mut self, run: &mut VerificationRun) {
        run.cases.retain(|case| !self.reused.contains(&case.id));
        self.reused.clear();
    }
    pub(super) async fn finish_binding(
        &mut self,
        application: &Path,
        program: &ProgramPlan,
        run: &mut VerificationRun,
        current: ReuseBinding,
        artifacts: &mut Vec<VerificationArtifact>,
        cancel: &NativeCancellation,
    ) -> Result<()> {
        let drift =
            match super::reuse_inputs::capture(application, &self.options.inputs, cancel).await {
                Ok(receipt) if receipt == self.receipt => None,
                Ok(receipt) => {
                    let changes = reuse::receipt_changes(&self.receipt, &receipt);
                    for decision in &mut self.decisions {
                        decision.reasons.extend(changes.clone());
                    }
                    artifacts.push(VerificationArtifact::new(
                        "application/vnd.dctx0.incremental-final-inputs+json",
                        serde_json::to_vec(&receipt)?,
                    ));
                    Some("INCREMENTAL_INPUTS_CHANGED")
                }
                Err(_) => Some("INCREMENTAL_INPUTS_UNAVAILABLE"),
            };
        if let Some(code) = drift {
            self.invalidate(run, code);
        }
        if !self.receipt.incomplete.is_empty() {
            self.invalidate(run, "INCREMENTAL_INPUTS_INCOMPLETE");
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
            self.invalidate(run, "INCREMENTAL_BINDING_CHANGED");
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
        let mut summary = serde_json::to_value(self.summary(program, run))?;
        summary["schema_version"] = 2.into();
        summary["basis"] = "Historical native cases under captured input bindings are not fresh external observations".into();
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.incremental-summary+json",
            serde_json::to_vec(&summary)?,
        ));
        Ok(())
    }
    pub(super) async fn finish(
        &mut self,
        application: &Path,
        program: &ProgramPlan,
        run: &mut VerificationRun,
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

    fn invalidate(&mut self, run: &mut VerificationRun, code: &str) {
        self.discard_cached_records(run);
        let problem = issue(
            code,
            "Execution inputs are unavailable or changed before publication",
        );
        run.provenance.incomplete.push(problem.clone());
        if matches!(run.result, VerificationResult::Pass) {
            run.result = VerificationResult::Unknown { issue: problem };
        }
        for decision in &mut self.decisions {
            decision.decision = reuse::UnitAction::Blocked;
            decision.reasons.push(reuse::ReuseReason {
                code: code.into(),
                input: "publication".into(),
            });
        }
    }
}
