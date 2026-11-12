//! Stored application evidence is checked again by the transactional publisher.
//! A diagnostic JSON receipt alone never creates native execution authority.
use crate::{
    application_plan::ApplicationTestPlan, application_session::ApplicationAssessmentReport,
};
use dctx0_core::{VerificationResult, VerificationRun};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const APPLICATION_EXECUTION_PROTOCOL_VERSION: u32 = 2;

pub const APPLICATION_EXECUTION_MEDIA_TYPE: &str =
    "application/vnd.dctx0.application-execution+json";
pub const INCREMENTAL_APPLICATION_EXECUTION_MEDIA_TYPE: &str =
    "application/vnd.dctx0.incremental-application-execution+json";

/// A current native registration and assessment with explicitly identified
/// historical cases. Store must independently validate the incremental lineage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncrementalApplicationExecutionProof {
    pub schema_version: u32,
    pub application: ApplicationExecutionProof,
    pub reused_cases: BTreeSet<String>,
}
impl IncrementalApplicationExecutionProof {
    pub fn validates(
        &self,
        run: &VerificationRun,
        incremental: &crate::incremental::IncrementalProof,
    ) -> bool {
        self.schema_version == 1
            && !self.reused_cases.is_empty()
            && self.reused_cases == incremental.plan.reused
            && self
                .reused_cases
                .is_subset(&self.application.assessment.required_cases)
            && self.application.validates(run)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationPhaseProof {
    pub phase: String,
    pub exit_code: Option<i32>,
    pub runtime_hash: String,
    pub supervised: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationExecutionProof {
    pub schema_version: u32,
    pub plan: ApplicationTestPlan,
    pub assessment: ApplicationAssessmentReport,
    pub source_inputs: BTreeMap<String, String>,
    pub build_inputs: BTreeMap<String, String>,
    pub materialized_inputs: BTreeMap<String, String>,
    pub toolchain_inputs: BTreeMap<String, String>,
    pub phases: Vec<ApplicationPhaseProof>,
    pub receipt_hash: String,
}

impl ApplicationExecutionProof {
    /// Independent finite-inventory and native-finalization gate. Task state,
    /// profiles, claims and event versions are checked by the store transaction.
    pub fn validates(&self, run: &VerificationRun) -> bool {
        self.validates_scope(run, true)
    }

    /// Validate one freshly managed group's native receipt for a hybrid cohort.
    /// This does not authorize task delivery: the existing standalone validator
    /// still requires the complete program, and Store must validate the cohort.
    pub fn validates_assigned_group(&self, run: &VerificationRun) -> bool {
        self.validates_scope(run, false)
    }

    fn validates_scope(&self, run: &VerificationRun, complete: bool) -> bool {
        let assessment = &self.assessment;
        let plan = &self.plan.program;
        let manifest = crate::case_manifest(plan);
        let assigned = if complete {
            manifest.required_cases.clone()
        } else {
            self.plan
                .case_groups
                .iter()
                .filter(|(_, owner)| *owner == &assessment.group)
                .map(|(id, _)| id.clone())
                .collect()
        };
        let required = &assigned;
        let actual = assessment
            .cases
            .iter()
            .map(|case| case.id.clone())
            .collect::<BTreeSet<_>>();
        let group = self
            .plan
            .configuration
            .groups
            .iter()
            .find(|group| group.id == assessment.group);
        self.schema_version == 1
            && run.origin == dctx0_core::VerificationRunOrigin::Native
            && run.provenance.protocol_version == APPLICATION_EXECUTION_PROTOCOL_VERSION
            && assessment.assessment_version
                == crate::application_session::APPLICATION_ASSESSMENT_VERSION
            && self.plan.validate().is_ok()
            && group.is_some_and(|group| {
                group.target == run.provenance.target
                    && group.backend == crate::application_plan::TestBackend::ApplicationTests
            })
            && (!complete
                || self
                    .plan
                    .case_groups
                    .values()
                    .all(|group| group == &assessment.group))
            && !required.is_empty()
            && assessment.finalized
            && assessment.issues.is_empty()
            && assessment.missing_cases.is_empty()
            && assessment.missing_tests.is_empty()
            && assessment.result == VerificationResult::Pass
            && &assessment.required_cases == required
            && &actual == required
            && actual.len() == assessment.cases.len()
            && assessment
                .cases
                .iter()
                .all(|case| case.result == VerificationResult::Pass)
            && assessment.cases == run.cases
            && required == &run.required_cases
            && plan.snapshot == run.provenance.snapshot
            && plan.check_occurrence == run.provenance.check_occurrence as u64
            && plan.budgets == run.budgets
            && plan.scope.subjects == run.selected_subjects
            && plan.closure == run.required_subjects
            && self.source_inputs == run.provenance.implementation
            && self.materialized_inputs == run.provenance.dependencies
            && self.build_inputs == run.provenance.build_configuration
            && !self.source_inputs.is_empty()
            && !self.build_inputs.is_empty()
            && !self.materialized_inputs.is_empty()
            && self.receipt_hash == run.provenance.harness_hash
            && self.phases.iter().all(|phase| {
                phase.exit_code == Some(0) && phase.runtime_hash.len() == 64 && phase.supervised
            })
            && (run.provenance.target != "go"
                || (!self.toolchain_inputs.is_empty()
                    && self
                        .phases
                        .iter()
                        .filter(|phase| phase.phase == "toolchain")
                        .count()
                        == 1))
            && ["catalog", "native-tests"].iter().all(|required| {
                self.phases
                    .iter()
                    .filter(|phase| phase.phase == *required)
                    .count()
                    == 1
            })
    }
}
