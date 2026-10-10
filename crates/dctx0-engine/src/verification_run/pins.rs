//! Capture immutable task/profile requirements before executing application code.
use super::*;

pub(super) fn capture(
    project: &Project,
    application: &Path,
    target: &str,
    plan: &ProgramPlan,
    snapshot: &dctx0_changes::Snapshot,
) -> Result<(Option<VerificationTaskPin>, Vec<VerificationIssue>)> {
    if plan.scope.tasks.is_empty() {
        return Ok((None, vec![]));
    }
    let session = project.task_session(None, None)?;
    if session.plan.snapshot_id != plan.snapshot
        || session.plan.check_run_id as u64 != plan.check_occurrence
    {
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
    for task in &plan.scope.tasks {
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
