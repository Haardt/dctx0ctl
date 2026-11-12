//! Pure assembly of freshly executed, centrally assigned backend groups.
//! This is not native execution authority: engine receipts and the transactional
//! Store guards must additionally prove execution before any task delivery.
use crate::application_plan::{ApplicationTestPlan, TestBackend, TestForm};
use dctx0_core::{
    VerificationCaseKind, VerificationCaseRecord, VerificationIssue, VerificationProvenance,
    VerificationResult, VerificationRun, VerificationRunOrigin,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HybridGroupRun {
    pub group: String,
    pub run: VerificationRun,
}

fn issue(code: &str, message: impl Into<String>) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: None,
    }
}

fn same_inputs(actual: &VerificationProvenance, expected: &VerificationProvenance) -> bool {
    actual.snapshot == expected.snapshot
        && actual.check_occurrence == expected.check_occurrence
        && actual.plan_hash == expected.plan_hash
        && actual.implementation == expected.implementation
        && actual.configuration_hash == expected.configuration_hash
        && actual.interpreter_version == expected.interpreter_version
        && actual.generator_version == expected.generator_version
        && actual.task_pin == expected.task_pin
        && actual.incomplete.is_empty()
}

/// Every group must be fresh within one managed execution window. Saved PASS,
/// imported runs, duplicate groups and matching IDs with different source pins
/// cannot supply missing work. Keep the original complete plan and case IDs.
pub fn assemble_hybrid_cases(
    plan: &ApplicationTestPlan,
    expected: &VerificationProvenance,
    started_at_unix_ns: &str,
    completed_at_unix_ns: &str,
    groups: &[HybridGroupRun],
) -> Result<Vec<VerificationCaseRecord>, Vec<VerificationIssue>> {
    plan.validate()?;
    let mut issues = Vec::new();
    let window = started_at_unix_ns
        .parse::<u128>()
        .ok()
        .zip(completed_at_unix_ns.parse::<u128>().ok());
    if !window.is_some_and(|(start, end)| start > 0 && end >= start)
        || expected.snapshot != plan.program.snapshot
        || u64::try_from(expected.check_occurrence).ok() != Some(plan.program.check_occurrence)
        || expected.plan_hash.is_empty()
        || expected.configuration_hash.is_empty()
        || expected.implementation.is_empty()
        || !expected.incomplete.is_empty()
    {
        return Err(vec![issue(
            "HYBRID_COHORT_PIN",
            "A current complete source pin and execution window are required",
        )]);
    }
    let mut seen = BTreeSet::new();
    let mut cases = BTreeMap::new();
    for evidence in groups {
        let Some(group) = plan
            .configuration
            .groups
            .iter()
            .find(|group| group.id == evidence.group)
        else {
            issues.push(issue("HYBRID_UNKNOWN_GROUP", &evidence.group));
            continue;
        };
        if !seen.insert(evidence.group.clone()) {
            issues.push(issue("HYBRID_DUPLICATE_GROUP", &evidence.group));
        }
        let run = &evidence.run;
        let fresh = run
            .started_at_unix_ns
            .parse::<u128>()
            .ok()
            .zip(window)
            .is_some_and(|(start, (first, last))| {
                start >= first
                    && start
                        .checked_add(u128::from(run.elapsed_us) * 1_000)
                        .is_some_and(|end| end <= last)
            });
        if !fresh || run.origin != VerificationRunOrigin::Native {
            issues.push(issue("HYBRID_FRESH_EXECUTION_REQUIRED", &evidence.group));
        }
        let protocol = match group.backend {
            TestBackend::Adapter => 1,
            TestBackend::ApplicationTests => 2,
        };
        if run.schema_version != 1
            || !same_inputs(&run.provenance, expected)
            || run.provenance.target != group.target
            || run.provenance.protocol_version != protocol
            || run.budgets != plan.program.budgets
            || run.selected_subjects != plan.program.scope.subjects
            || run.required_subjects != plan.program.closure
            || run.not_selected != plan.program.not_selected
        {
            issues.push(issue("HYBRID_GROUP_PIN_MISMATCH", &evidence.group));
        }
        let assigned = plan
            .case_groups
            .iter()
            .filter(|(_, owner)| *owner == &evidence.group)
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        let actual = run
            .cases
            .iter()
            .map(|case| case.id.clone())
            .collect::<BTreeSet<_>>();
        if assigned.is_empty()
            || assigned != run.required_cases
            || assigned != actual
            || actual.len() != run.cases.len()
        {
            issues.push(issue("HYBRID_GROUP_CASE_MISMATCH", &evidence.group));
        }
        if !run.permits_task_specific_proof()
            || run
                .cases
                .iter()
                .any(|case| case.result != VerificationResult::Pass)
        {
            issues.push(issue("HYBRID_GROUP_NOT_PASS", &evidence.group));
        }
        for case in &run.cases {
            let kind = plan.case_forms.get(&case.id).map(|form| match form {
                TestForm::Decoder => VerificationCaseKind::Contract,
                TestForm::Operation | TestForm::StateChange => VerificationCaseKind::Operation,
                TestForm::Assertion => VerificationCaseKind::Assertion,
                TestForm::Scenario => VerificationCaseKind::Example,
                TestForm::Lifecycle => VerificationCaseKind::Lifecycle,
                TestForm::EntityIdentity => VerificationCaseKind::EntityIdentity,
            });
            if kind.as_ref() != Some(&case.kind) {
                issues.push(issue("HYBRID_CASE_KIND_MISMATCH", &case.id));
            }
            if cases.insert(case.id.clone(), case.clone()).is_some() {
                issues.push(issue("HYBRID_DUPLICATE_CASE", &case.id));
            }
        }
    }
    if seen
        != plan
            .configuration
            .groups
            .iter()
            .map(|group| group.id.clone())
            .collect()
        || cases.keys().cloned().collect::<BTreeSet<_>>() != plan.manifest.required_cases
    {
        issues.push(issue(
            "HYBRID_INCOMPLETE_COHORT",
            "Every assigned group and mandatory case must execute in this cohort",
        ));
    }
    if issues.is_empty() {
        Ok(cases.into_values().collect())
    } else {
        Err(issues)
    }
}
