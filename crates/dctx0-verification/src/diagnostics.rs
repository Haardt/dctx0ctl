//! Bounded views over immutable evidence; filtering never changes run assessment.
use dctx0_core::{VerificationCaseRecord, VerificationIssue, VerificationResult, VerificationRun};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFilter {
    pub outcomes: BTreeSet<String>,
    pub subjects: BTreeSet<String>,
    pub case: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSummary {
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
    pub id: String,
    pub subject: String,
    pub result: VerificationResult,
    pub observations: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticPage {
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
        schema_version: 1,
        run_id: run_id.into(),
        snapshot: run.provenance.snapshot.clone(),
        check_occurrence: run.provenance.check_occurrence,
        summary: DiagnosticSummary {
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
