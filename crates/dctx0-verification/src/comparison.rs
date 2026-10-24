//! Exact-identity comparison. Removed requirements never count as repaired issues.
use dctx0_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssueChange {
    New,
    Persisting,
    Changed,
    Resolved,
    Unproven,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparedIssue {
    pub case_id: String,
    pub subject: String,
    pub change: IssueChange,
    pub before: Option<VerificationResult>,
    pub after: Option<VerificationResult>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunComparison {
    pub schema_version: u32,
    pub compatible_requirements: bool,
    pub newly_executed_after: bool,
    pub added_cases: BTreeSet<String>,
    pub removed_cases: BTreeSet<String>,
    pub new: usize,
    pub persisting: usize,
    pub changed: usize,
    pub resolved: usize,
    pub unproven: usize,
    pub issues: Vec<ComparedIssue>,
    pub next_offset: Option<usize>,
    pub total_issues: usize,
}
/// Works with compact case projections: requirements are bound by the exact plan
/// hash and generation/evaluation versions, never guessed from names or text.
pub fn compare_runs(
    before: &VerificationRun,
    after: &VerificationRun,
    offset: usize,
    limit: usize,
) -> Result<RunComparison, String> {
    if limit == 0 || limit > 1000 {
        return Err("Comparison limit must be 1..1000".into());
    }
    let assessment = |run: &VerificationRun| {
        let versions = run
            .artifacts
            .iter()
            .filter(|a| a.media_type == "application/vnd.dctx0.assessment-version+json")
            .collect::<Vec<_>>();
        match versions.as_slice() {
            [version] => Some(version.hash.clone()),
            _ => None,
        }
    };
    let assessment_before = assessment(before);
    let compatible = assessment_before.is_some()
        && assessment_before == assessment(after)
        && before.provenance.plan_hash == after.provenance.plan_hash
        && before.provenance.generator_version == after.provenance.generator_version
        && before.provenance.interpreter_version == after.provenance.interpreter_version
        && before.provenance.protocol_version == after.provenance.protocol_version
        && before.budgets == after.budgets;
    let fresh = after.origin == VerificationRunOrigin::Native
        && before.started_at_unix_ns != after.started_at_unix_ns
        && !after
            .artifacts
            .iter()
            .any(|a| a.media_type == "application/vnd.dctx0.native-parent+json")
        && after.provenance.incomplete.is_empty();
    fn index(run: &VerificationRun) -> Result<BTreeMap<String, &VerificationCaseRecord>, String> {
        let mut cases = BTreeMap::new();
        for case in &run.cases {
            if cases.insert(case.id.clone(), case).is_some() {
                return Err("Duplicate case IDs cannot be compared".into());
            }
        }
        Ok(cases)
    }
    let previous = index(before)?;
    let current = index(after)?;
    let mut report = RunComparison {
        schema_version: 1,
        compatible_requirements: compatible,
        newly_executed_after: fresh,
        added_cases: after
            .required_cases
            .difference(&before.required_cases)
            .cloned()
            .collect(),
        removed_cases: before
            .required_cases
            .difference(&after.required_cases)
            .cloned()
            .collect(),
        new: 0,
        persisting: 0,
        changed: 0,
        resolved: 0,
        unproven: 0,
        issues: vec![],
        next_offset: None,
        total_issues: 0,
    };
    let ids: BTreeSet<_> = previous.keys().chain(current.keys()).collect();
    for id in ids {
        let a = previous.get(id).copied();
        let b = current.get(id).copied();
        let problem = |c: Option<&VerificationCaseRecord>| {
            c.is_some_and(|c| c.result != VerificationResult::Pass)
        };
        if !problem(a) && !problem(b) {
            continue;
        }
        let change = match (a, b) {
            (Some(a), Some(b)) if problem(Some(a)) && b.result == VerificationResult::Pass => {
                if compatible
                    && fresh
                    && a.subject == b.subject
                    && a.kind == b.kind
                    && before.required_cases.contains(id)
                    && after.required_cases.contains(id)
                {
                    IssueChange::Resolved
                } else {
                    IssueChange::Unproven
                }
            }
            (Some(a), Some(b)) if problem(Some(a)) && problem(Some(b)) => {
                if compatible && a.subject == b.subject && a.kind == b.kind && a.result == b.result
                {
                    IssueChange::Persisting
                } else {
                    IssueChange::Changed
                }
            }
            (_, Some(_)) => IssueChange::New,
            _ => IssueChange::Unproven,
        };
        match change {
            IssueChange::New => report.new += 1,
            IssueChange::Persisting => report.persisting += 1,
            IssueChange::Changed => report.changed += 1,
            IssueChange::Resolved => report.resolved += 1,
            IssueChange::Unproven => report.unproven += 1,
        }
        report.issues.push(ComparedIssue {
            case_id: id.clone(),
            subject: b.or(a).unwrap().subject.clone(),
            change,
            before: a.map(|c| c.result.clone()),
            after: b.map(|c| c.result.clone()),
        });
    }
    report.total_issues = report.issues.len();
    if offset > report.total_issues {
        return Err("Comparison offset exceeds issue count".into());
    }
    let end = offset.saturating_add(limit).min(report.total_issues);
    report.next_offset = (end < report.total_issues).then_some(end);
    report.issues = report.issues.drain(offset..end).collect();
    Ok(report)
}
