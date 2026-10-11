//! Target-neutral finite-plan evaluation. A worker reports observations, never PASS.
use dctx0_core::{
    Expression, Invocation, InvocationObservation, VerificationBudgets, VerificationIssue,
    VerificationResult as R, VerificationValue, WorkspaceIr,
};
use dctx0_interpreter::{Evaluation, evaluate};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const VERIFICATION_VERSION: &str = "dctx0-verification-1";
pub mod attestations;
mod preconditions;
pub mod program_inspection;
pub mod suites;
pub use preconditions::{
    PRECONDITION_SELECTION_VERSION, PreconditionSelection, resolved_state_preconditions,
    select_precondition_inputs,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationScope {
    pub subjects: BTreeSet<String>,
    pub tasks: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCase {
    pub id: String,
    pub variables: BTreeMap<String, VerificationValue>,
    pub assertion: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obligation {
    pub id: String,
    pub subject: String,
    pub cases: Vec<VerificationCase>,
    /// Obligations unsupported by the current planner remain mandatory open points.
    pub unsupported: Option<VerificationIssue>,
    pub bounded_corpus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPlan {
    pub schema_version: u32,
    pub snapshot: String,
    pub check_occurrence: u64,
    pub scope: VerificationScope,
    pub budgets: VerificationBudgets,
    pub obligations: Vec<Obligation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseEvidence {
    pub case_id: String,
    pub observations: Vec<InvocationObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    pub result: R,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanResult {
    pub result: R,
    pub cases: Vec<CaseResult>,
    pub incomplete_obligations: Vec<String>,
    pub bounded_obligations: Vec<String>,
}

fn unknown(code: &str, message: &str) -> R {
    R::Unknown {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: None,
        },
    }
}

/// This is an evaluation API, not native-execution provenance or adoption authority.
/// Evidence IDs must exactly match mandatory cases; missing and surplus evidence
/// are explicit. Known failures dominate UNKNOWN, but incomplete scope is retained.
pub fn assess(ir: &WorkspaceIr, plan: &VerificationPlan, evidence: &[CaseEvidence]) -> PlanResult {
    let mut report = PlanResult {
        result: R::Pass,
        cases: Vec::new(),
        incomplete_obligations: Vec::new(),
        bounded_obligations: Vec::new(),
    };
    if plan.schema_version != dctx0_core::VERIFICATION_SCHEMA_VERSION
        || plan.snapshot.is_empty()
        || plan.check_occurrence == 0
        || plan.scope.subjects.is_empty()
        || plan.obligations.is_empty()
    {
        report.result = unknown(
            "INVALID_PLAN",
            "A versioned, pinned, nonempty explicit plan is required",
        );
        return report;
    }
    let mut ids = BTreeSet::new();
    let mut obligations = BTreeSet::new();
    for obligation in &plan.obligations {
        if !obligations.insert(&obligation.id)
            || obligation.id.is_empty()
            || !plan.scope.subjects.contains(&obligation.subject)
            || obligation
                .cases
                .iter()
                .any(|case| case.id.is_empty() || !ids.insert(&case.id))
        {
            report.result = unknown(
                "INVALID_PLAN",
                "Duplicate identifiers or obligation outside selected scope",
            );
            return report;
        }
    }
    let mut received = BTreeMap::new();
    for record in evidence {
        if !ids.contains(&record.case_id) || received.insert(&record.case_id, record).is_some() {
            report.result = unknown("INVALID_EVIDENCE", "Surplus or duplicate case evidence");
            return report;
        }
    }
    let covered_subjects = plan
        .obligations
        .iter()
        .map(|o| &o.subject)
        .collect::<BTreeSet<_>>();
    if plan
        .scope
        .subjects
        .iter()
        .any(|s| !covered_subjects.contains(s))
    {
        report.result = unknown(
            "MISSING_OBLIGATION",
            "A selected subject has no obligations",
        );
    }
    for obligation in &plan.obligations {
        if obligation.bounded_corpus {
            report.bounded_obligations.push(obligation.id.clone());
        }
        let mut incomplete = false;
        if let Some(issue) = &obligation.unsupported {
            merge(
                &mut report.result,
                R::Unknown {
                    issue: issue.clone(),
                },
            );
            incomplete = true;
        }
        if obligation.cases.is_empty() {
            merge(
                &mut report.result,
                unknown("EMPTY_CORPUS", "Mandatory obligation has no valid cases"),
            );
            incomplete = true;
        }
        for case in &obligation.cases {
            let result = match received.get(&case.id) {
                None => unknown("MISSING_CASE", "Mandatory case was not executed"),
                Some(record) => match evaluate(
                    ir,
                    &case.assertion,
                    &case.variables,
                    &record.observations,
                    &plan.budgets,
                ) {
                    Evaluation::Complete(VerificationValue::Boolean(true)) => R::Pass,
                    Evaluation::Complete(VerificationValue::Boolean(false)) => R::Fail {
                        issue: VerificationIssue {
                            code: "ASSERTION_FAILED".into(),
                            message: "Observed implementation violates the modeled assertion"
                                .into(),
                            span: Some(case.assertion.span.clone()),
                        },
                    },
                    Evaluation::Complete(_) => unknown(
                        "NON_BOOLEAN_ASSERTION",
                        "Obligation did not produce a Boolean",
                    ),
                    Evaluation::Invoke(_) => unknown(
                        "INCOMPLETE_OBSERVATIONS",
                        "Mandatory invocation has no observation",
                    ),
                    Evaluation::Unknown(issue) => R::Unknown { issue },
                    Evaluation::ApplicationFailure(issue) => R::Fail { issue },
                },
            };
            incomplete |= matches!(result, R::Unknown { .. });
            merge(&mut report.result, result.clone());
            report.cases.push(CaseResult {
                case_id: case.id.clone(),
                result,
            });
        }
        if incomplete {
            report.incomplete_obligations.push(obligation.id.clone());
        }
    }
    report
}
fn merge(current: &mut R, next: R) {
    if matches!(current, R::Pass)
        || matches!(current, R::Unknown { .. }) && matches!(next, R::Fail { .. })
    {
        *current = next;
    }
}

/// The next actual call is derived by Rust from the assertion and continuation.
pub fn next_invocation(
    ir: &WorkspaceIr,
    case: &VerificationCase,
    observations: &[InvocationObservation],
    budgets: &VerificationBudgets,
) -> Result<Option<Invocation>, Box<R>> {
    match evaluate(ir, &case.assertion, &case.variables, observations, budgets) {
        Evaluation::Invoke(call) => Ok(Some(call)),
        Evaluation::Complete(_) => Ok(None),
        Evaluation::Unknown(issue) => Err(Box::new(R::Unknown { issue })),
        Evaluation::ApplicationFailure(issue) => Err(Box::new(R::Fail { issue })),
    }
}

mod generation;
pub use generation::{
    GENERATOR_VERSION, InputCorpus, REJECTION_GENERATOR_VERSION, ValueCorpus, generate_inputs,
    generate_rejections, generate_values, missing_rejections,
};

mod projection;
pub use projection::*;
mod entity_identity;
mod operation;
mod operation_effects;
mod operation_state;
pub use entity_identity::{entity_identity_case, validate_entity_identity_cases};
pub use operation::{
    OperationAssessment, OperationCaseInput, OperationObservers, assess_operation, operation_ready,
};
mod scenario;
pub use scenario::*;
mod lifecycle;
pub use lifecycle::*;

mod manifest;
pub use manifest::{CaseManifest, case_manifest, supports_generated_operation};

mod resource_effects;

mod shrinking;
pub use shrinking::{
    LifecycleShrinkAttempt, LifecycleShrinkReport, SHRINK_VERSION, ShrinkAttempt, ShrinkReport,
    shrink_invocations, shrink_lifecycle_inputs,
};

mod operation_mocks;
pub use operation_mocks::{OperationMock, OperationMockSession};

/// Version of central observation and contract assessment, distinct from generation.
pub const ASSESSMENT_VERSION: &str = "dctx0-assessment-15";
