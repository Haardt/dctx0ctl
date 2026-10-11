//! Capture immutable task/profile requirements before executing application code.
use super::*;

pub(super) fn capture(
    project: &Project,
    application: &Path,
    target: &str,
    plan: &ProgramPlan,
    snapshot: &dctx0_changes::Snapshot,
) -> Result<(Option<VerificationTaskPin>, Vec<VerificationIssue>)> {
    let session = match project.task_session(None, None) {
        Ok(session) => session,
        Err(_) if plan.scope.tasks.is_empty() => return Ok((None, vec![])),
        Err(error) => return Err(error),
    };
    let selected_tasks = if plan.scope.tasks.is_empty() {
        session
            .plan
            .members
            .iter()
            .filter(|member| {
                !member.candidate.subjects.is_empty()
                    && member.candidate.subjects.iter().all(|subject| {
                        plan.scope.subjects.iter().any(|selected| {
                            subject == selected || subject.starts_with(&format!("{selected}."))
                        })
                    })
            })
            .map(|member| format!("T{}", member.task_id))
            .collect::<BTreeSet<_>>()
    } else {
        plan.scope.tasks.clone()
    };
    if selected_tasks.is_empty() {
        return Ok((None, vec![]));
    }
    if session.plan.snapshot_id != plan.snapshot
        || session.plan.check_run_id as u64 != plan.check_occurrence
    {
        if plan.scope.tasks.is_empty() {
            return Ok((
                None,
                vec![issue(
                    "ADOPTION_HISTORICAL_SCOPE",
                    "The active task plan does not match this historical verification occurrence",
                )],
            ));
        }
        return Err(Error::Invalid(
            "Task requirements changed before native execution; plan verification again".into(),
        ));
    }
    let mut pin = VerificationTaskPin {
        plan_id: session.plan.id,
        event_sequence: session.event_seq,
        requirements: BTreeMap::new(),
        profile_bundles: BTreeMap::new(),
    };
    let mut issues = vec![];
    let profiles = match project.profiles(Some(application), Some(target)) {
        Ok(profiles) => profiles,
        Err(error) => {
            issues.push(issue("ADOPTION_PROFILE_UNAVAILABLE", error.to_string()));
            None
        }
    };
    for task in &selected_tasks {
        let id = task
            .strip_prefix('T')
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|id| *id > 0)
            .ok_or_else(|| Error::Invalid(format!("Invalid pinned verification task {task}")))?;
        let member = session.member(id)?;
        pin.requirements
            .insert(id, member.candidate.requirements_hash.clone());
        if let Some(profiles) = &profiles {
            let selection = dctx0_tasks::select_profiles(
                snapshot,
                &member.candidate,
                &profiles.manifest,
                &profiles.documents,
            );
            match selection.require_complete() {
                Ok(()) => {
                    pin.profile_bundles.insert(id, selection.bundle.hash);
                }
                Err(error) => issues.push(issue(
                    "ADOPTION_PROFILE_INCOMPLETE",
                    format!("{task}: {error}"),
                )),
            }
        }
    }
    Ok((Some(pin), issues))
}
