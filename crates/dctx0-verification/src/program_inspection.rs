//! Pure effective assessment. Agent judgements never replace recorded execution.
use crate::{
    ProgramPlan,
    attestations::{AttestationBinding, apply_attestation},
    suites::*,
};
use dctx0_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionFinding {
    pub id: String,
    pub severity: FindingSeverity,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectedReview {
    pub approved: bool,
    pub findings: Vec<InspectionFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramInspection {
    pub inspection: VerificationAttestation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<InspectedReview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveSubjectAssessment {
    pub subject: String,
    pub result: AttestedVerificationResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveProgramAssessment {
    pub schema_version: u32,
    pub subjects: Vec<EffectiveSubjectAssessment>,
    pub policies: Vec<NamedAssessment>,
    pub open_obligations: Vec<VerificationIssue>,
    pub result: VerificationResult,
}

fn unknown(code: &str, message: impl Into<String>) -> VerificationResult {
    VerificationResult::Unknown {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: None,
        },
    }
}

pub fn inspection_requirements(plan: &ProgramPlan, subject: &str) -> Option<BTreeSet<String>> {
    let nodes = if let Some(suite) = plan
        .suites
        .iter()
        .find(|suite| suite.declaration.id == subject)
    {
        &suite.nodes
    } else {
        &plan
            .manual_checks
            .iter()
            .find(|manual| manual.subject == subject && manual.kind != StatementKind::Intent)?
            .nodes
    };
    Some(
        std::iter::once(subject.to_owned())
            .chain(nodes.iter().map(|node| node.id.clone()))
            .collect(),
    )
}

/// Binding and finite-plan evaluation only; Engine validates stored native origin,
/// artifact integrity, current sources/profiles and publication separately.
pub fn assess_with_inspections(
    ir: &WorkspaceIr,
    plan: &ProgramPlan,
    cases: &[VerificationCaseRecord],
    plan_hash: &str,
    artifacts: &BTreeMap<String, String>,
    inspections: &[ProgramInspection],
) -> EffectiveProgramAssessment {
    let automatic = native_suite_policy_assessment(ir, plan, cases);
    let mut result = EffectiveProgramAssessment {
        schema_version: 1,
        subjects: vec![],
        policies: vec![],
        open_obligations: plan.issues.clone(),
        result: VerificationResult::Pass,
    };
    let mut seen = BTreeSet::new();
    for inspection in inspections {
        let subject = &inspection.inspection.subject;
        if !seen.insert(subject) || inspection_requirements(plan, subject).is_none() {
            if let VerificationResult::Unknown { issue } =
                unknown("INVALID_INSPECTION_SCOPE", subject)
            {
                result.open_obligations.push(issue);
            }
        }
    }
    let candidates = automatic
        .suites
        .iter()
        .map(|suite| (suite.subject.clone(), suite.result.clone(), false))
        .chain(
            plan.manual_checks
                .iter()
                .filter(|manual| manual.kind != StatementKind::Intent)
                .map(|manual| {
                    (
                        manual.subject.clone(),
                        unknown("MANUAL_REQUIREMENT", &manual.subject),
                        manual.kind == StatementKind::Review,
                    )
                }),
        );
    let mut outcomes = BTreeMap::new();
    for (subject, automatic, review_required) in candidates {
        let mut effective = AttestedVerificationResult {
            automatic: automatic.clone(),
            effective: automatic.clone(),
            basis: VerificationEvidenceBasis::Automatic,
        };
        let matching = inspections
            .iter()
            .filter(|inspection| inspection.inspection.subject == subject)
            .collect::<Vec<_>>();
        let mut review = None;
        if let [inspection] = matching.as_slice() {
            let requirements = inspection_requirements(plan, &subject).unwrap_or_default();
            let binding = AttestationBinding {
                subject: &subject,
                snapshot: &plan.snapshot,
                plan_hash,
                requirements: &requirements,
                implementation_artifacts: artifacts,
            };
            let valid_review = match (&inspection.review, review_required) {
                (None, false) => true,
                (Some(value), true) => {
                    let ids = value
                        .findings
                        .iter()
                        .map(|finding| &finding.id)
                        .collect::<BTreeSet<_>>();
                    ids.len() == value.findings.len()
                        && value.findings.iter().all(|finding| {
                            !finding.id.trim().is_empty() && !finding.description.trim().is_empty()
                        })
                        && (value.approved
                            == (inspection.inspection.result == VerificationResult::Pass))
                }
                _ => false,
            };
            let execution_suite = plan.suites.iter().any(|suite| {
                suite.declaration.id == subject
                    && matches!(suite.declaration.name.as_str(), "Examples" | "Properties")
            });
            if valid_review
                && !(execution_suite && matches!(automatic, VerificationResult::Unknown { .. }))
            {
                effective = apply_attestation(&automatic, &inspection.inspection, &binding);
                if effective.basis != VerificationEvidenceBasis::Automatic {
                    review = inspection.review.as_ref();
                }
            }
        }
        let status = match &effective.effective {
            VerificationResult::Pass => PolicyStatus::Pass,
            VerificationResult::Fail { .. } => PolicyStatus::Fail,
            VerificationResult::Unknown { .. } => PolicyStatus::Unknown,
        };
        let outcome = if let Some(review) = review {
            PolicyOutcome {
                status: if !review.approved {
                    PolicyStatus::RequestChanges
                } else if review.findings.is_empty() {
                    PolicyStatus::Approve
                } else {
                    PolicyStatus::ApproveWithFindings
                },
                high_findings: Some(
                    review
                        .findings
                        .iter()
                        .filter(|finding| finding.severity == FindingSeverity::High)
                        .count() as u64,
                ),
            }
        } else {
            PolicyOutcome {
                status,
                high_findings: None,
            }
        };
        outcomes.insert(subject.clone(), outcome);
        result.subjects.push(EffectiveSubjectAssessment {
            subject,
            result: effective,
        });
    }
    result.policies = assess_program_policies(ir, plan, &outcomes);
    let passed = result
        .subjects
        .iter()
        .filter(|subject| subject.result.effective == VerificationResult::Pass)
        .map(|subject| subject.subject.clone())
        .collect::<BTreeSet<_>>();
    let passed_policies = result
        .policies
        .iter()
        .filter(|policy| policy.result == VerificationResult::Pass)
        .map(|policy| policy.subject.clone())
        .collect::<BTreeSet<_>>();
    let manifest = crate::case_manifest(plan);
    for issue in &manifest.open_obligations {
        let discharged = (issue.code == "SUITE_INSPECTION_REQUIRED"
            && passed.contains(&issue.message))
            || (issue.code == "MANUAL_REQUIREMENT"
                && passed
                    .iter()
                    .chain(&passed_policies)
                    .any(|subject| issue.message.starts_with(&format!("{subject}: "))));
        if !discharged {
            result.open_obligations.push(issue.clone());
        }
    }
    let resolved_preconditions = crate::resolved_state_preconditions(ir, plan, cases);
    result
        .open_obligations
        .retain(|issue| !resolved_preconditions.contains(issue));
    let actual = cases
        .iter()
        .map(|case| case.id.clone())
        .collect::<BTreeSet<_>>();
    if actual != manifest.required_cases || actual.len() != cases.len() || actual.is_empty() {
        if let VerificationResult::Unknown { issue } = unknown(
            "INCOMPLETE_RUN",
            "Inspection cannot replace mandatory native execution",
        ) {
            result.open_obligations.push(issue);
        }
    }
    let all_results = cases
        .iter()
        .map(|case| &case.result)
        .chain(
            result
                .subjects
                .iter()
                .map(|subject| &subject.result.effective),
        )
        .chain(result.policies.iter().map(|policy| &policy.result))
        .collect::<Vec<_>>();
    result.result = all_results
        .iter()
        .find(|r| matches!(r, VerificationResult::Fail { .. }))
        .or_else(|| {
            all_results
                .iter()
                .find(|r| matches!(r, VerificationResult::Unknown { .. }))
        })
        .map(|r| (*r).clone())
        .unwrap_or_else(|| {
            result
                .open_obligations
                .first()
                .map_or(VerificationResult::Pass, |issue| {
                    VerificationResult::Unknown {
                        issue: issue.clone(),
                    }
                })
        });
    result
}
