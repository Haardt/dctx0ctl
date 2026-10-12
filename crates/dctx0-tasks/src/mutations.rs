//! Work lifecycle rules are pure; repositories only commit their state events.
use crate::*;
use std::collections::BTreeSet;

pub struct MutationGuard {
    pub actor: String,
    pub claim: i64,
    pub version: i64,
}
pub fn guarded(state: &State, guard: &MutationGuard) -> Result<State> {
    if state.work != WorkState::Progress
        || state.actor.as_ref() != Some(&guard.actor)
        || state.claim != Some(guard.claim)
        || state.version != guard.version
    {
        return Err(Error::Conflict(
            "Invalid actor, claim token, state or version".into(),
        ));
    }
    Ok(state.clone())
}
pub fn start_state(
    session: &Session,
    id: i64,
    actor: String,
    reservation: Option<i64>,
    claim: i64,
) -> Result<State> {
    if actor.trim().is_empty() {
        return Err(Error::Invalid("Actor is required".into()));
    }
    let overview = session
        .overviews()
        .into_iter()
        .find(|r| r.task_id == id)
        .ok_or(Error::Missing(id))?;
    let mut state = session.states[&id].clone();
    if !overview.ready
        || state.actor.as_ref().is_some_and(|a| a != &actor)
        || state.reservation != reservation
    {
        return Err(Error::Conflict(format!(
            "T{id} is not ready or reservation does not match: {:?}",
            overview.blockers
        )));
    }
    state.work = WorkState::Progress;
    state.version += 1;
    state.actor = Some(actor);
    state.ever_started = true;
    state.claim = Some(claim);
    Ok(state)
}
pub fn release_state(state: &State, guard: &MutationGuard, reason: &str) -> Result<State> {
    if reason.trim().is_empty() {
        return Err(Error::Invalid("Release reason is required".into()));
    }
    let mut state = guarded(state, guard)?;
    state.execution_bundle = None;
    state.work = WorkState::Open;
    state.actor = None;
    state.claim = None;
    state.reservation = None;
    state.version += 1;
    Ok(state)
}
pub fn complete_state(
    definition: &Definition,
    state: &State,
    guard: &MutationGuard,
    result: TaskResult,
) -> Result<State> {
    if result.summary.trim().is_empty() {
        return Err(Error::Invalid("Result summary is required".into()));
    }
    let mut state = guarded(state, guard)?;
    if definition.candidate.kind == TaskKind::Review && state.reviews.is_empty() {
        return Err(Error::Invalid(
            "Review completion requires a review report".into(),
        ));
    }
    state.work = WorkState::Done;
    state.result = Some(result);
    state.actor = None;
    state.claim = None;
    state.reservation = None;
    state.version += 1;
    Ok(state)
}
pub fn check_state(
    definition: &Definition,
    state: &State,
    evidence: CheckEvidence,
) -> Result<State> {
    if state.work != WorkState::Done
        || state
            .result
            .as_ref()
            .and_then(|r| r.implementation.as_ref())
            != Some(&evidence.implementation)
        || definition.candidate.requirements_hash != evidence.requirements_hash
    {
        return Err(Error::Invalid(
            "Evidence must match a delivered result, requirements and implementation".into(),
        ));
    }
    if evidence.status == CheckStatus::Unverified
        || [
            &evidence.implementation,
            &evidence.verifier,
            &evidence.verifier_version,
            &evidence.command,
            &evidence.started_at,
            &evidence.finished_at,
            &evidence.report,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        || matches!(evidence.verifier.as_str(), "manual" | "llm" | "review")
    {
        return Err(Error::Invalid(
            "Executed-check provenance and PASS/FAIL are required".into(),
        ));
    }
    let mut state = state.clone();
    state.check = evidence.status;
    state.evidence.push(evidence);
    state.version += 1;
    Ok(state)
}
pub fn review_state(
    definition: &Definition,
    state: &State,
    guard: &MutationGuard,
    report: ReviewReport,
    known: &BTreeSet<i64>,
) -> Result<State> {
    let mut state = guarded(state, guard)?;
    if definition.candidate.kind != TaskKind::Review
        || report.reviewer.trim().is_empty()
        || report.summary.trim().is_empty()
        || !matches!(report.judgement.as_str(), "approved" | "request-changes")
    {
        return Err(Error::Invalid(
            "A review task and a structured approved/request-changes report are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    if let Some(evidence) = &report.conformance {
        if evidence.requirements_hash != definition.candidate.requirements_hash
            || evidence.snapshot.trim().is_empty()
            || evidence.spec_source_hash.trim().is_empty()
            || evidence.implementation_artifacts.is_empty()
            || evidence.subjects.is_empty()
            || evidence.subjects.keys().cloned().collect::<BTreeSet<_>>()
                != definition
                    .candidate
                    .review_obligations
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            || evidence.subjects.iter().any(|(subject, note)| {
                !definition.candidate.review_obligations.contains(subject)
                    || note.report.trim().is_empty()
            })
        {
            return Err(Error::Invalid("Conformance review requires matching task requirements, source bindings and concrete subject reports".into()));
        }
    }
    for f in &report.findings {
        if f.id.trim().is_empty()
            || !ids.insert(&f.id)
            || f.description.trim().is_empty()
            || !matches!(f.severity.as_str(), "low" | "medium" | "high")
            || f.task_ids.iter().any(|id| !known.contains(id))
        {
            return Err(Error::Invalid("Invalid or duplicate review finding".into()));
        }
    }
    state.reviews.push(report);
    state.version += 1;
    Ok(state)
}
