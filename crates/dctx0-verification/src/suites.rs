//! Pure suite aggregation. These inputs are evidence, not publication authority.
use dctx0_core::{VerificationIssue, VerificationResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofCategory {
    Type,
    Data,
    Surface,
    Architecture,
    Ownership,
    Effects,
}

/// Matches the reference proof catalog; unknown names require explicit meaning.
pub fn named_proof_categories(name: &str) -> &'static [ProofCategory] {
    use ProofCategory::*;
    match name {
        "TypeConformance" | "Type" => &[Type, Data],
        "SurfaceConformance" => &[Surface],
        "ArchitectureConformance" | "Architecture" => &[Architecture],
        "OwnershipConformance" => &[Ownership],
        "EffectConformance" => &[Effects],
        _ => &[],
    }
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

/// Known failures dominate uncertainty. A missing category never passes.
pub fn assess_proof_suite(
    name: &str,
    categories: &BTreeMap<ProofCategory, VerificationResult>,
) -> VerificationResult {
    let required = named_proof_categories(name);
    if required.is_empty() {
        return unknown("UNSUPPORTED_PROOF_SUITE", name);
    }
    for category in required {
        if let Some(result @ VerificationResult::Fail { .. }) = categories.get(category) {
            return result.clone();
        }
    }
    for category in required {
        match categories.get(category) {
            Some(VerificationResult::Pass) => {}
            Some(result) => return result.clone(),
            None => return unknown("PROOF_CATEGORY_NOT_RUN", format!("{category:?}")),
        }
    }
    VerificationResult::Pass
}

/// Exact required case membership, including uniqueness and nonempty evidence.
/// An empty expected set permits explicitly supplied conformance scenarios,
/// but never vacuous success. Callers must bind these inputs to provenance.
pub fn assess_test_suite(
    expected: &BTreeSet<String>,
    results: &[(String, VerificationResult)],
) -> VerificationResult {
    let actual: BTreeSet<_> = results.iter().map(|(id, _)| id.clone()).collect();
    if results.is_empty()
        || actual.len() != results.len()
        || actual.contains("")
        || (!expected.is_empty() && &actual != expected)
    {
        return unknown(
            "SUITE_EXECUTION_INCOMPLETE",
            "Missing, surplus or duplicate suite cases",
        );
    }
    for (_, result) in results {
        if matches!(result, VerificationResult::Fail { .. }) {
            return result.clone();
        }
    }
    results
        .iter()
        .find(|(_, result)| matches!(result, VerificationResult::Unknown { .. }))
        .map_or(VerificationResult::Pass, |(_, result)| result.clone())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyStatus {
    Pass,
    Fail,
    Unknown,
    NotRun,
    Stale,
    Approve,
    ApproveWithFindings,
    RequestChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyOutcome {
    pub status: PolicyStatus,
    /// None means findings were not inspected; it must not become zero.
    pub high_findings: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyValue {
    Status(PolicyStatus),
    HighFindings(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub target: String,
    pub expected: PolicyValue,
    pub equal: bool,
}

/// Missing or stale evidence cannot satisfy even a negative requirement.
/// This deliberately fixes the reference's default-zero missing-findings case.
pub fn assess_policy_rule(
    rule: &PolicyRule,
    outcomes: &BTreeMap<String, PolicyOutcome>,
) -> VerificationResult {
    let Some(outcome) = outcomes.get(&rule.target) else {
        return unknown("POLICY_EVIDENCE_NOT_RUN", &rule.target);
    };
    if matches!(
        outcome.status,
        PolicyStatus::Unknown | PolicyStatus::NotRun | PolicyStatus::Stale
    ) {
        return unknown("POLICY_EVIDENCE_INCOMPLETE", &rule.target);
    }
    let actual = match &rule.expected {
        PolicyValue::Status(_) => PolicyValue::Status(outcome.status.clone()),
        PolicyValue::HighFindings(_) => {
            let Some(count) = outcome.high_findings else {
                return unknown("POLICY_FINDINGS_NOT_INSPECTED", &rule.target);
            };
            PolicyValue::HighFindings(count)
        }
    };
    if (actual == rule.expected) == rule.equal {
        VerificationResult::Pass
    } else {
        VerificationResult::Fail {
            issue: VerificationIssue {
                code: "POLICY_REQUIREMENT_VIOLATED".into(),
                message: format!(
                    "{}: observed {actual:?}, expected {} {:?}",
                    rule.target,
                    if rule.equal { "==" } else { "!=" },
                    rule.expected
                ),
                span: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedAssessment {
    pub subject: String,
    pub result: VerificationResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuitePolicyAssessment {
    pub schema_version: u32,
    pub suites: Vec<NamedAssessment>,
    pub policies: Vec<NamedAssessment>,
}

pub fn native_suite_policy_assessment(
    ir: &dctx0_core::WorkspaceIr,
    plan: &crate::ProgramPlan,
    cases: &[dctx0_core::VerificationCaseRecord],
) -> SuitePolicyAssessment {
    let suites = assess_program_suites(plan, cases, &BTreeMap::new());
    let policies = assess_program_policies(ir, plan, &suite_policy_outcomes(&suites));
    SuitePolicyAssessment {
        schema_version: 1,
        suites,
        policies,
    }
}

/// Only complete actual native suite cases discharge their own planning issue.
pub fn resolved_native_suite_obligations(
    plan: &crate::ProgramPlan,
    cases: &[dctx0_core::VerificationCaseRecord],
) -> Vec<VerificationIssue> {
    let passed = assess_program_suites(plan, cases, &BTreeMap::new())
        .into_iter()
        .filter(|suite| suite.result == VerificationResult::Pass)
        .map(|suite| suite.subject)
        .collect::<BTreeSet<_>>();
    crate::case_manifest(plan)
        .open_obligations
        .into_iter()
        .filter(|issue| {
            issue.code == "SUITE_INSPECTION_REQUIRED" && passed.contains(&issue.message)
        })
        .collect()
}

/// Projects policy syntax from validated IR without modifying compiler models.
pub fn project_policy_rule(
    ir: &dctx0_core::WorkspaceIr,
    node: &dctx0_core::SemanticNode,
) -> Result<PolicyRule, VerificationIssue> {
    let invalid = || VerificationIssue {
        code: "UNSUPPORTED_POLICY_RULE".into(),
        message: node.id.clone(),
        span: Some(node.span.clone()),
    };
    if node.kind != dctx0_core::StatementKind::Require || node.terms.len() != 3 {
        return Err(invalid());
    }
    let left = &node.terms[0];
    let target = ir
        .references
        .iter()
        .find(|reference| {
            reference.kind == dctx0_core::ReferenceKind::Verifies
                && reference.span.start == left.span.start
                && reference.span.file == left.span.file
        })
        .ok_or_else(invalid)?
        .target
        .clone();
    let equal = match node.terms[1].text.as_str() {
        "==" => true,
        "!=" => false,
        _ => return Err(invalid()),
    };
    let right = node.terms[2].text.as_str();
    let expected = if left.text.ends_with(".highFindings") {
        PolicyValue::HighFindings(right.parse().map_err(|_| invalid())?)
    } else {
        PolicyValue::Status(match right {
            "PASS" => PolicyStatus::Pass,
            "FAIL" => PolicyStatus::Fail,
            "UNKNOWN" => PolicyStatus::Unknown,
            "NOT_RUN" => PolicyStatus::NotRun,
            "STALE" => PolicyStatus::Stale,
            "APPROVE" => PolicyStatus::Approve,
            "APPROVE_WITH_FINDINGS" => PolicyStatus::ApproveWithFindings,
            "REQUEST_CHANGES" => PolicyStatus::RequestChanges,
            _ => return Err(invalid()),
        })
    };
    Ok(PolicyRule {
        target,
        expected,
        equal,
    })
}

/// Aggregate executed examples and properties against the exact pinned plan.
/// Other suites remain open until their dedicated category/scenario evidence exists.
pub fn assess_program_suites(
    plan: &crate::ProgramPlan,
    cases: &[dctx0_core::VerificationCaseRecord],
    categories: &BTreeMap<ProofCategory, VerificationResult>,
) -> Vec<NamedAssessment> {
    use dctx0_core::{StatementKind as K, VerificationCaseKind as C};
    plan.suites
        .iter()
        .map(|suite| {
            let name = suite.declaration.name.as_str();
            let result = if suite.declaration.kind == K::Proof {
                assess_proof_suite(name, categories)
            } else {
                let (kind, expected): (_, BTreeSet<String>) = match name {
                    "Examples" => (
                        C::Example,
                        plan.examples
                            .iter()
                            .map(|example| example.subject.clone())
                            .collect(),
                    ),
                    "Properties" => (
                        C::Assertion,
                        plan.properties
                            .iter()
                            .flat_map(|property| &property.assertions)
                            .flat_map(|assertion| &assertion.cases)
                            .map(|case| case.id.clone())
                            .collect(),
                    ),
                    _ => {
                        return NamedAssessment {
                            subject: suite.declaration.id.clone(),
                            result: unknown("CONFORMANCE_SCENARIOS_REQUIRED", name),
                        };
                    }
                };
                if expected.is_empty() {
                    return NamedAssessment {
                        subject: suite.declaration.id.clone(),
                        result: unknown("EMPTY_SUITE_CORPUS", name),
                    };
                }
                let results = cases
                    .iter()
                    .filter(|case| case.kind == kind)
                    .map(|case| (case.id.clone(), case.result.clone()))
                    .collect::<Vec<_>>();
                let result = assess_test_suite(&expected, &results);
                let planning_issue = if name == "Examples" {
                    plan.examples
                        .iter()
                        .flat_map(|example| &example.issues)
                        .next()
                } else {
                    plan.properties
                        .iter()
                        .flat_map(|property| &property.assertions)
                        .flat_map(|assertion| &assertion.issues)
                        .next()
                };
                match (result, planning_issue) {
                    (VerificationResult::Pass, Some(issue)) => VerificationResult::Unknown {
                        issue: issue.clone(),
                    },
                    (result, _) => result,
                }
            };
            NamedAssessment {
                subject: suite.declaration.id.clone(),
                result,
            }
        })
        .collect()
}

pub fn resolved_native_suite_policy_obligations(
    ir: &dctx0_core::WorkspaceIr,
    plan: &crate::ProgramPlan,
    cases: &[dctx0_core::VerificationCaseRecord],
) -> Vec<VerificationIssue> {
    let mut resolved = resolved_native_suite_obligations(plan, cases);
    let passed_policies = native_suite_policy_assessment(ir, plan, cases)
        .policies
        .into_iter()
        .filter(|policy| policy.result == VerificationResult::Pass)
        .map(|policy| {
            format!(
                "{}: Policy has no automatically asserted meaning",
                policy.subject
            )
        })
        .collect::<BTreeSet<_>>();
    resolved.extend(
        crate::case_manifest(plan)
            .open_obligations
            .into_iter()
            .filter(|issue| {
                issue.code == "MANUAL_REQUIREMENT" && passed_policies.contains(&issue.message)
            }),
    );
    resolved
}

pub fn assess_program_policies(
    ir: &dctx0_core::WorkspaceIr,
    plan: &crate::ProgramPlan,
    outcomes: &BTreeMap<String, PolicyOutcome>,
) -> Vec<NamedAssessment> {
    plan.policies
        .iter()
        .map(|policy| {
            let rules = policy
                .nodes
                .iter()
                .filter(|node| node.kind == dctx0_core::StatementKind::Require)
                .collect::<Vec<_>>();
            let mut result = if rules.is_empty() {
                unknown("EMPTY_POLICY", &policy.subject)
            } else {
                VerificationResult::Pass
            };
            for node in rules {
                let next = match project_policy_rule(ir, node) {
                    Ok(rule) => assess_policy_rule(&rule, outcomes),
                    Err(issue) => VerificationResult::Unknown { issue },
                };
                match (&result, &next) {
                    (_, VerificationResult::Fail { .. }) => result = next,
                    (VerificationResult::Pass, VerificationResult::Unknown { .. }) => result = next,
                    _ => {}
                }
            }
            NamedAssessment {
                subject: policy.subject.clone(),
                result,
            }
        })
        .collect()
}

pub fn suite_policy_outcomes(suites: &[NamedAssessment]) -> BTreeMap<String, PolicyOutcome> {
    suites
        .iter()
        .map(|suite| {
            (
                suite.subject.clone(),
                PolicyOutcome {
                    status: match suite.result {
                        VerificationResult::Pass => PolicyStatus::Pass,
                        VerificationResult::Fail { .. } => PolicyStatus::Fail,
                        VerificationResult::Unknown { .. } => PolicyStatus::Unknown,
                    },
                    high_findings: None,
                },
            )
        })
        .collect()
}
