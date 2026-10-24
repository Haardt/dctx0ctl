//! Bounded views over immutable evidence; filtering never changes run assessment.
use dctx0_core::{VerificationCaseRecord, VerificationIssue, VerificationResult, VerificationRun};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFilter {
    #[serde(default)]
    pub causes: BTreeSet<dctx0_core::VerificationCause>,
    pub outcomes: BTreeSet<String>,
    pub subjects: BTreeSet<String>,
    pub case: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    #[serde(default)]
    pub causes: BTreeMap<dctx0_core::VerificationCause, usize>,
    pub result: VerificationResult,
    pub mandatory_cases: usize,
    pub recorded_cases: usize,
    pub outcomes: BTreeMap<String, usize>,
    pub issues: BTreeMap<String, usize>,
    pub open_obligations: Vec<VerificationIssue>,
    pub incomplete_provenance: Vec<VerificationIssue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCursor {
    pub run_id: String,
    pub filter: DiagnosticFilter,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseDiagnostic {
    #[serde(default)]
    pub diagnosis: Option<IssueDiagnosis>,
    pub id: String,
    pub subject: String,
    pub result: VerificationResult,
    pub observations: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticPage {
    #[serde(default = "details_available")]
    pub details_available: bool,
    pub schema_version: u32,
    pub run_id: String,
    pub snapshot: String,
    pub check_occurrence: i64,
    pub filter: DiagnosticFilter,
    pub summary: DiagnosticSummary,
    pub matching_cases: usize,
    pub summary_only: bool,
    pub cases: Vec<CaseDiagnostic>,
    /// Included only for an explicit exact-case request.
    pub detail: Option<VerificationCaseRecord>,
    pub next_cursor: Option<DiagnosticCursor>,
    pub complete: bool,
}
fn outcome(result: &VerificationResult) -> &'static str {
    match result {
        VerificationResult::Pass => "PASS",
        VerificationResult::Fail { .. } => "FAIL",
        VerificationResult::Unknown { .. } => "UNKNOWN",
    }
}
pub fn diagnostic_page(
    run_id: &str,
    run: &VerificationRun,
    filter: DiagnosticFilter,
    limit: usize,
    cursor: Option<&DiagnosticCursor>,
    summary_only: bool,
) -> Result<DiagnosticPage, String> {
    if limit == 0
        || filter
            .outcomes
            .iter()
            .any(|o| !matches!(o.as_str(), "PASS" | "FAIL" | "UNKNOWN"))
    {
        return Err("Positive diagnostic limit and PASS/FAIL/UNKNOWN outcomes required".into());
    }
    let offset = if let Some(cursor) = cursor {
        if cursor.run_id != run_id
            || cursor.filter != filter
            || cursor.limit != limit
            || summary_only
        {
            return Err("Diagnostic cursor does not match run, filter or limit".into());
        }
        cursor.offset
    } else {
        0
    };
    let mut outcomes = BTreeMap::new();
    let mut issues = BTreeMap::new();
    for case in &run.cases {
        *outcomes.entry(outcome(&case.result).into()).or_insert(0) += 1;
        match &case.result {
            VerificationResult::Fail { issue } | VerificationResult::Unknown { issue } => {
                *issues
                    .entry(format!("{}:{}", outcome(&case.result), issue.code))
                    .or_insert(0) += 1;
            }
            _ => {}
        }
    }
    let selected =
        run.cases
            .iter()
            .filter(|case| {
                (filter.outcomes.is_empty() || filter.outcomes.contains(outcome(&case.result)))
                    && (filter.causes.is_empty()
                        || diagnose_result(&case.subject, &case.result)
                            .is_some_and(|d| filter.causes.contains(&d.cause)))
                    && (filter.subjects.is_empty()
                        || filter.subjects.iter().any(|s| {
                            case.subject == *s || case.subject.starts_with(&format!("{s}."))
                        }))
                    && filter.case.as_ref().is_none_or(|id| case.id == *id)
            })
            .collect::<Vec<_>>();
    if offset > selected.len() {
        return Err("Diagnostic cursor exceeds matching cases".into());
    }
    if filter.case.is_some() && selected.is_empty() {
        return Err("No saved case matches the exact ID and filters".into());
    }
    let end = if summary_only {
        selected.len()
    } else {
        offset.saturating_add(limit).min(selected.len())
    };
    Ok(DiagnosticPage {
        details_available: true,
        schema_version: 1,
        run_id: run_id.into(),
        snapshot: run.provenance.snapshot.clone(),
        check_occurrence: run.provenance.check_occurrence,
        summary: DiagnosticSummary {
            causes: cause_counts(run),
            result: run.result.clone(),
            mandatory_cases: run.required_cases.len(),
            recorded_cases: run.cases.len(),
            outcomes,
            issues,
            open_obligations: run.open_obligations.clone(),
            incomplete_provenance: run.provenance.incomplete.clone(),
        },
        matching_cases: selected.len(),
        summary_only,
        cases: if summary_only {
            vec![]
        } else {
            selected[offset..end]
                .iter()
                .map(|case| CaseDiagnostic {
                    diagnosis: diagnose_result(&case.subject, &case.result),
                    id: case.id.clone(),
                    subject: case.subject.clone(),
                    result: case.result.clone(),
                    observations: case.observations.len(),
                })
                .collect()
        },
        detail: if summary_only {
            None
        } else {
            filter
                .case
                .as_ref()
                .and_then(|_| selected.first().map(|case| (*case).clone()))
        },
        next_cursor: (end < selected.len()).then(|| DiagnosticCursor {
            run_id: run_id.into(),
            filter: filter.clone(),
            offset: end,
            limit,
        }),
        complete: end == selected.len(),
        filter,
    })
}

fn details_available() -> bool {
    true
}

/// Stable structured classification. Unknown codes remain unclassified even if
/// their message happens to contain words such as database, timeout or adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDiagnosis {
    pub issue: VerificationIssue,
    pub cause: dctx0_core::VerificationCause,
    pub subject: String,
    pub attribution: String,
    pub next_step: String,
}
pub fn diagnose_result(subject: &str, result: &VerificationResult) -> Option<IssueDiagnosis> {
    let issue = match result {
        VerificationResult::Pass => return None,
        VerificationResult::Fail { issue } | VerificationResult::Unknown { issue } => issue,
    };
    use dctx0_core::VerificationCause::*;
    let cause = match issue.code.as_str() {
        "ASSERTION_FAILED"
        | "NATIVE_RETURN_CONTRACT"
        | "INVALID_RETURN_TYPE"
        | "OPERATION_CLAUSE_VIOLATED"
        | "OPERATION_RESULT_CONTRACT"
        | "CALL_CONTRACT_VIOLATED"
        | "EVENT_CONTRACT_VIOLATED"
        | "EVENT_EFFECT_VIOLATED"
        | "STATE_EFFECT_VIOLATED"
        | "STATE_RESULT_VIOLATED"
        | "PURE_OPERATION_EMITTED"
        | "UNDECLARED_OPERATION_EFFECT" => ContractViolation,
        "NATIVE_ENVIRONMENT_UNAVAILABLE" => Environment,
        "NATIVE_ADAPTER_ERROR"
        | "NATIVE_PROTOCOL_ERROR"
        | "MISSING_OBSERVER"
        | "STATE_OBSERVER_REQUIRED"
        | "CALL_OBSERVER_REQUIRED"
        | "EVENT_OBSERVER_REQUIRED"
        | "INVALID_OBSERVED_STATE"
        | "OBSERVATION_MISMATCH" => Adapter,
        "EVALUATION_BUDGET"
        | "RECURSION_BUDGET"
        | "VALUE_DEPTH"
        | "VALUE_SIZE"
        | "CLAUSE_NOT_EXECUTABLE"
        | "UNSUPPORTED_EXPRESSION"
        | "UNSUPPORTED_LITERAL"
        | "UNSUPPORTED_OPERATOR"
        | "UNSUPPORTED_BUILTIN"
        | "UNSUPPORTED_COLLECTION"
        | "NONTERMINATING_DECIMAL" => EvaluatorLimit,
        "MISSING_CASE"
        | "MISSING_OBLIGATION"
        | "INCOMPLETE_OBSERVATIONS"
        | "CLAUSE_OBSERVATION_REQUIRED"
        | "STATE_SOURCE_INCOMPLETE" => MissingEvidence,
        "MANUAL_REQUIREMENT" | "SUITE_INSPECTION_REQUIRED" => InspectionRequired,
        _ => Unclassified,
    };
    let next_step = match cause {
        ContractViolation => {
            "Inspect the actual return/state against the pinned contract and repair the implementation"
        }
        Adapter => {
            "Check bindings, representation conversion and actual observers; rerun the affected case"
        }
        Environment => "Restore the explicitly reported external dependency and rerun",
        EvaluatorLimit => {
            "Inspect the expression at its source position and supported evaluation budgets"
        }
        MissingEvidence => {
            "Execute the missing required case or supply the required actual observation"
        }
        InspectionRequired => "Publish a source-bound inspection with reviewer and explicit limits",
        Unclassified => {
            "Investigate the original evidence; do not infer a cause from the error message"
        }
    };
    Some(IssueDiagnosis {
        issue: issue.clone(),
        cause,
        subject: subject.into(),
        attribution: "structured-issue-code-v1".into(),
        next_step: next_step.into(),
    })
}
pub fn cause_counts(run: &VerificationRun) -> BTreeMap<dctx0_core::VerificationCause, usize> {
    let mut counts = BTreeMap::new();
    for case in &run.cases {
        if let Some(diagnosis) = diagnose_result(&case.subject, &case.result) {
            *counts.entry(diagnosis.cause).or_default() += 1;
        }
    }
    counts
}
