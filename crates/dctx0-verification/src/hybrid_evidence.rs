//! Mixed execution receipts bind a complete manifest to fresh backend groups.
//! The engine owns execution and current runtime/source checks; Store must load
//! and hash-check the referenced native receipt before accepting this proof.
use crate::{
    application_evidence::ApplicationExecutionProof,
    application_plan::TestBackend,
    hybrid_assessment::{HybridGroupRun, assemble_hybrid_cases},
};
use dctx0_core::{VerificationRun, VerificationRunOrigin};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const HYBRID_EXECUTION_MEDIA_TYPE: &str = "application/vnd.dctx0.hybrid-execution+json";
pub const INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE: &str =
    "application/vnd.dctx0.incremental-hybrid-execution+json";

/// The application group ran in this cohort; selected adapter cases came from
/// a separately validated immutable lineage. It is not a fresh cohort proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncrementalHybridExecutionProof {
    pub schema_version: u32,
    pub cohort: HybridExecutionProof,
    pub reused_cases: BTreeSet<String>,
}
impl IncrementalHybridExecutionProof {
    pub fn validates(
        &self,
        run: &VerificationRun,
        incremental: &crate::incremental::IncrementalProof,
    ) -> bool {
        use dctx0_core::VerificationResult;
        let proof = &self.cohort;
        let plan = &proof.application.plan;
        let app = &proof.application.assessment.group;
        let Some(adapter) =
            plan.configuration.groups.iter().find(|group| {
                group.id == proof.adapter_group && group.backend == TestBackend::Adapter
            })
        else {
            return false;
        };
        let actual: BTreeSet<_> = run.cases.iter().map(|case| case.id.clone()).collect();
        let app_cases: Vec<_> = run
            .cases
            .iter()
            .filter(|case| plan.case_groups.get(&case.id) == Some(app))
            .cloned()
            .collect();
        let window = run
            .started_at_unix_ns
            .parse::<u128>()
            .ok()
            .zip(
                proof
                    .application_run
                    .started_at_unix_ns
                    .parse::<u128>()
                    .ok(),
            )
            .zip(proof.adapter_started_at_unix_ns.parse::<u128>().ok());
        let within = window.is_some_and(|((start, app_start), adapter_start)| {
            let end = start.checked_add(u128::from(run.elapsed_us) * 1000);
            end.is_some_and(|end| {
                start > 0
                    && app_start >= start
                    && adapter_start >= start
                    && app_start
                        .checked_add(u128::from(proof.application_run.elapsed_us) * 1000)
                        .is_some_and(|x| x <= end)
                    && adapter_start
                        .checked_add(u128::from(proof.adapter_elapsed_us) * 1000)
                        .is_some_and(|x| x <= end)
            })
        });
        let kinds_valid = run.cases.iter().all(|case| {
            use crate::application_plan::TestForm;
            use dctx0_core::VerificationCaseKind;
            plan.case_forms.get(&case.id).is_some_and(|form| {
                case.kind
                    == match form {
                        TestForm::Decoder => VerificationCaseKind::Contract,
                        TestForm::Operation | TestForm::StateChange => {
                            VerificationCaseKind::Operation
                        }
                        TestForm::Assertion => VerificationCaseKind::Assertion,
                        TestForm::Scenario => VerificationCaseKind::Example,
                        TestForm::Lifecycle => VerificationCaseKind::Lifecycle,
                        TestForm::EntityIdentity => VerificationCaseKind::EntityIdentity,
                    }
            })
        });
        self.schema_version == 1
            && proof.schema_version == 1
            && !self.reused_cases.is_empty()
            && self.reused_cases == incremental.plan.reused
            && self
                .reused_cases
                .iter()
                .all(|id| plan.case_groups.get(id) == Some(&proof.adapter_group))
            && plan.configuration.groups.len() == 2
            && app != &proof.adapter_group
            && adapter.target == run.provenance.target
            && plan.validate().is_ok()
            && run.schema_version == 1
            && run.origin == VerificationRunOrigin::Native
            && run.provenance.protocol_version == 1
            && run.provenance.incomplete.is_empty()
            && run.permits_task_specific_proof()
            && run.required_cases == plan.manifest.required_cases
            && run.provenance.snapshot == plan.program.snapshot
            && u64::try_from(run.provenance.check_occurrence).ok()
                == Some(plan.program.check_occurrence)
            && run.budgets == plan.program.budgets
            && run.selected_subjects == plan.program.scope.subjects
            && run.required_subjects == plan.program.closure
            && run.not_selected == plan.program.not_selected
            && actual == run.required_cases
            && actual.len() == run.cases.len()
            && run
                .cases
                .iter()
                .all(|case| case.result == VerificationResult::Pass)
            && kinds_valid
            && proof
                .application
                .validates_assigned_group(&proof.application_run)
            && app_cases == proof.application_run.cases
            && proof.application_run.provenance.target == run.provenance.target
            && proof.application_run.provenance.snapshot == run.provenance.snapshot
            && proof.application_run.provenance.check_occurrence == run.provenance.check_occurrence
            && proof.application_run.provenance.plan_hash == run.provenance.plan_hash
            && proof.application_run.provenance.implementation == run.provenance.implementation
            && proof.application_run.provenance.configuration_hash
                == run.provenance.configuration_hash
            && proof.application_run.provenance.interpreter_version
                == run.provenance.interpreter_version
            && proof.application_run.provenance.generator_version
                == run.provenance.generator_version
            && proof.application_run.provenance.task_pin == run.provenance.task_pin
            && proof.application_run.not_selected == run.not_selected
            && proof.application_run.provenance.incomplete.is_empty()
            && within
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HybridExecutionProof {
    pub schema_version: u32,
    pub application: ApplicationExecutionProof,
    pub application_run: VerificationRun,
    pub adapter_group: String,
    pub adapter_started_at_unix_ns: String,
    pub adapter_elapsed_us: u64,
}

impl HybridExecutionProof {
    /// First managed mixed format: exactly one adapter and one application
    /// group of the same target. Unsupported arrangements fail closed rather
    /// than silently reusing historical or imported results.
    pub fn validates(&self, run: &VerificationRun) -> bool {
        let plan = &self.application.plan;
        let application_group = &self.application.assessment.group;
        let adapter =
            plan.configuration.groups.iter().find(|group| {
                group.id == self.adapter_group && group.backend == TestBackend::Adapter
            });
        let actual = run
            .cases
            .iter()
            .map(|case| case.id.clone())
            .collect::<BTreeSet<_>>();
        if self.schema_version != 1
            || run.schema_version != 1
            || run.origin != VerificationRunOrigin::Native
            || run.provenance.protocol_version != 1
            || !run.provenance.incomplete.is_empty()
            || !run.permits_task_specific_proof()
            || plan.configuration.groups.len() != 2
            || self.adapter_group == *application_group
            || adapter.is_none_or(|group| group.target != run.provenance.target)
            || !self
                .application
                .validates_assigned_group(&self.application_run)
            || self.application_run.provenance.target != run.provenance.target
            || run.required_cases != plan.manifest.required_cases
            || actual != run.required_cases
            || actual.len() != run.cases.len()
        {
            return false;
        }
        let selected = |group: &str| {
            run.cases
                .iter()
                .filter(|case| {
                    plan.case_groups
                        .get(&case.id)
                        .is_some_and(|owner| owner == group)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        // Preserve actual native records byte-for-byte, including inputs,
        // observed state and source IDs. Identity derivation is independently
        // reproduced by Store against the combined source cases.
        if selected(application_group) != self.application_run.cases {
            return false;
        }
        let mut adapter_run = run.clone();
        adapter_run.started_at_unix_ns = self.adapter_started_at_unix_ns.clone();
        adapter_run.elapsed_us = self.adapter_elapsed_us;
        adapter_run.required_cases = plan
            .case_groups
            .iter()
            .filter(|(_, group)| *group == &self.adapter_group)
            .map(|(id, _)| id.clone())
            .collect();
        adapter_run.cases = selected(&self.adapter_group);
        let run_end = run
            .started_at_unix_ns
            .parse::<u128>()
            .ok()
            .and_then(|start| start.checked_add(u128::from(run.elapsed_us) * 1_000));
        let Some(run_end) = run_end else {
            return false;
        };
        assemble_hybrid_cases(
            plan,
            &run.provenance,
            &run.started_at_unix_ns,
            &run_end.to_string(),
            &[
                HybridGroupRun {
                    group: application_group.clone(),
                    run: self.application_run.clone(),
                },
                HybridGroupRun {
                    group: self.adapter_group.clone(),
                    run: adapter_run,
                },
            ],
        )
        .is_ok()
    }
}
