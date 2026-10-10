//! Pure eligibility proposals. A proposal is not native execution authority.
use crate::*;
use dctx0_core::{VerificationResult, VerificationTaskPin};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionTaskProof {
    pub requirements_hash: String,
    /// Exact task subjects, required types and governing requirements accounted for.
    pub covered_requirements: BTreeSet<String>,
    pub required_cases: BTreeSet<String>,
    pub executed_cases: BTreeMap<String, VerificationResult>,
    pub implementation: String,
    pub artifacts: Vec<String>,
    pub representations: BTreeMap<String, String>,
    pub profile_bundle: String,
    pub incomplete: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionBlocker {
    pub task_id: i64,
    pub code: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptionProposal {
    pub plan_id: i64,
    pub event_sequence: i64,
    /// Deterministic topological delivery order; no state is changed here.
    pub tasks: Vec<i64>,
    pub blocked: Vec<AdoptionBlocker>,
}
fn block(proposal: &mut AdoptionProposal, id: i64, code: &str, reason: impl Into<String>) {
    proposal.blocked.push(AdoptionBlocker {
        task_id: id,
        code: code.into(),
        reason: reason.into(),
    });
}

/// The engine must validate actual native evidence and freshness separately.
/// Imported/self-declared results may be inspected but cannot authorize mutation.
pub fn propose_adoption(
    session: &Session,
    snapshot: &str,
    check_occurrence: i64,
    pin: &VerificationTaskPin,
    proofs: &BTreeMap<i64, AdoptionTaskProof>,
) -> Result<AdoptionProposal> {
    if pin.requirements.is_empty() {
        return Err(Error::Invalid(
            "Adoption requires a nonempty pinned task scope".into(),
        ));
    }
    if session.plan.id != pin.plan_id
        || session.event_seq != pin.event_sequence
        || session.plan.snapshot_id != snapshot
        || session.plan.check_run_id != check_occurrence
    {
        return Err(Error::Conflict(
            "Adoption requirements belong to a stale plan, occurrence or event sequence".into(),
        ));
    }
    if proofs.keys().any(|id| !pin.requirements.contains_key(id)) {
        return Err(Error::Invalid(
            "Adoption proof selects a task outside its pinned scope".into(),
        ));
    }
    let mut proposal = AdoptionProposal {
        plan_id: pin.plan_id,
        event_sequence: pin.event_sequence,
        tasks: vec![],
        blocked: vec![],
    };
    let mut simulation = session.clone();
    let mut ordered = pin.requirements.keys().copied().collect::<Vec<_>>();
    ordered.sort_by_key(|id| {
        session
            .plan
            .members
            .iter()
            .find(|m| m.task_id == *id)
            .map(|m| (m.candidate.wave, m.candidate.key.clone(), *id))
            .unwrap_or((usize::MAX, String::new(), *id))
    });
    for id in ordered {
        let Some(member) = session.plan.members.iter().find(|m| m.task_id == id) else {
            block(
                &mut proposal,
                id,
                "INACTIVE_TASK",
                "Historical or superseded work is not adopted",
            );
            continue;
        };
        let candidate = &member.candidate;
        let state = session.states.get(&id).ok_or(Error::Missing(id))?;
        if !matches!(candidate.kind, TaskKind::Contract | TaskKind::Operation) {
            block(
                &mut proposal,
                id,
                "TASK_KIND_REQUIRES_DEDICATED_EVIDENCE",
                "Review/retirement cannot be adopted; adapter/integration require dedicated conformance evidence",
            );
            continue;
        }
        if state.work != WorkState::Open
            || state.actor.is_some()
            || state.claim.is_some()
            || state.reservation.is_some()
        {
            block(
                &mut proposal,
                id,
                "TASK_NOT_UNCLAIMED_OPEN",
                "Done, ongoing or reserved work is preserved",
            );
            continue;
        }
        let Some(proof) = proofs.get(&id) else {
            block(
                &mut proposal,
                id,
                "TASK_EVIDENCE_REQUIRED",
                "Selected task has no complete native proof",
            );
            continue;
        };
        if pin.requirements.get(&id) != Some(&candidate.requirements_hash)
            || proof.requirements_hash != candidate.requirements_hash
        {
            block(
                &mut proposal,
                id,
                "TASK_REQUIREMENTS_CHANGED",
                "Proof does not match immutable active requirements",
            );
            continue;
        }
        if proof.profile_bundle.is_empty()
            || pin.profile_bundles.get(&id) != Some(&proof.profile_bundle)
        {
            block(
                &mut proposal,
                id,
                "TASK_PROFILE_UNPINNED",
                "Proof must bind the exact selected profile bundle",
            );
            continue;
        }
        if proof.implementation.trim().is_empty()
            || proof.artifacts.is_empty()
            || proof.artifacts.iter().any(|a| a.trim().is_empty())
            || proof.representations.is_empty()
            || proof.representations.values().any(|r| r.trim().is_empty())
        {
            block(
                &mut proposal,
                id,
                "IMPLEMENTATION_MAPPING_REQUIRED",
                "Actual implementation artifacts and representation choices are required",
            );
            continue;
        }
        let mut requirements = candidate
            .subjects
            .iter()
            .chain(&candidate.types)
            .chain(&candidate.facts);
        if !proof.incomplete.is_empty()
            || requirements.any(|r| !proof.covered_requirements.contains(r))
        {
            block(
                &mut proposal,
                id,
                "INCOMPLETE_REQUIREMENTS",
                "Subject/type/fact closure contains unsupported or unaccounted requirements",
            );
            continue;
        }
        let executed = proof
            .executed_cases
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if proof.required_cases.is_empty()
            || proof.required_cases.iter().any(|id| id.trim().is_empty())
            || executed != proof.required_cases
            || proof
                .executed_cases
                .values()
                .any(|r| r != &VerificationResult::Pass)
        {
            block(
                &mut proposal,
                id,
                "INCOMPLETE_CASE_EVIDENCE",
                "Every nonempty mandatory case must actually pass; partial/UNKNOWN/FAIL evidence cannot complete work",
            );
            continue;
        }
        let row = simulation
            .overviews()
            .into_iter()
            .find(|r| r.task_id == id)
            .ok_or(Error::Missing(id))?;
        if !row.ready {
            block(
                &mut proposal,
                id,
                "SCHEDULING_BLOCKED",
                row.blockers.join("; "),
            );
            continue;
        }
        // Simulate delivery only to evaluate later scheduling prerequisites.
        // The store must apply existing lifecycle rules/events atomically.
        let next = simulation.states.get_mut(&id).ok_or(Error::Missing(id))?;
        next.work = WorkState::Done;
        next.check = CheckStatus::Pass;
        next.result = Some(TaskResult {
            summary: "Proposed native verification adoption".into(),
            artifacts: proof.artifacts.clone(),
            decisions: vec![],
            implementation: Some(proof.implementation.clone()),
        });
        proposal.tasks.push(id);
    }
    Ok(proposal)
}

/// Pure mapping of mandatory requirements to the cases that must discharge them.
/// Passing unrelated cases cannot cover a requirement with no planned cases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCaseCoverage {
    pub required_cases: BTreeSet<String>,
    pub missing_requirements: BTreeSet<String>,
    pub unexecuted_cases: BTreeSet<String>,
    pub result: VerificationResult,
}
pub fn task_case_coverage(
    requirements: &BTreeSet<String>,
    case_requirements: &BTreeMap<String, BTreeSet<String>>,
    outcomes: &BTreeMap<String, VerificationResult>,
) -> TaskCaseCoverage {
    let mut coverage = TaskCaseCoverage {
        required_cases: BTreeSet::new(),
        missing_requirements: BTreeSet::new(),
        unexecuted_cases: BTreeSet::new(),
        result: VerificationResult::Pass,
    };
    for requirement in requirements {
        let cases = case_requirements
            .iter()
            .filter(|(_, mapped)| mapped.contains(requirement))
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        if cases.is_empty() {
            coverage.missing_requirements.insert(requirement.clone());
        }
        coverage.required_cases.extend(cases);
    }
    coverage.unexecuted_cases = coverage
        .required_cases
        .iter()
        .filter(|id| !outcomes.contains_key(*id))
        .cloned()
        .collect();
    // A real violation dominates incomplete coverage, as in native run reports.
    if let Some(failed) = coverage
        .required_cases
        .iter()
        .filter_map(|id| outcomes.get(id))
        .find(|result| matches!(result, VerificationResult::Fail { .. }))
    {
        coverage.result = failed.clone();
    } else if requirements.is_empty()
        || coverage.required_cases.is_empty()
        || !coverage.missing_requirements.is_empty()
        || !coverage.unexecuted_cases.is_empty()
    {
        coverage.result = VerificationResult::Unknown {
            issue: dctx0_core::VerificationIssue {
                code: "INCOMPLETE_TASK_CASE_MAPPING".into(),
                message:
                    "Every nonempty task requirement needs planned and executed mandatory cases"
                        .into(),
                span: None,
            },
        };
    } else if let Some(unknown) = coverage
        .required_cases
        .iter()
        .filter_map(|id| outcomes.get(id))
        .find(|result| matches!(result, VerificationResult::Unknown { .. }))
    {
        coverage.result = unknown.clone();
    }
    coverage
}
