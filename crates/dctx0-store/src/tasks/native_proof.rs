//! Independently check discharged planning hints from immutable native evidence.
use super::*;
use crate::Result;
use dctx0_core::{VerificationCaseManifest, VerificationIssue, VerificationRun};
use serde::de::DeserializeOwned;

fn artifact<T: DeserializeOwned>(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    media_type: &str,
) -> Result<(String, T)> {
    let references = run
        .artifacts
        .iter()
        .filter(|a| a.media_type == media_type)
        .collect::<Vec<_>>();
    let [reference] = references.as_slice() else {
        return Err(invalid(format!(
            "Native task delivery requires one {media_type} artifact"
        )));
    };
    let bytes: Vec<u8> = tx.query_row(
        "SELECT a.bytes FROM verification_artifacts a JOIN verification_run_artifacts r ON r.artifact_hash=a.hash WHERE r.run_id=?1 AND a.hash=?2",
        params![run_id, reference.hash], |row| row.get(0),
    )?;
    if crate::VerificationArtifact::new(media_type, bytes.clone()).reference != **reference {
        return Err(invalid(
            "Native proof artifact integrity does not match its run reference",
        ));
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("Invalid native proof artifact: {error}")))?;
    Ok((reference.hash.clone(), value))
}

pub(super) fn validate_open_obligations(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
) -> Result<()> {
    if run
        .cases
        .iter()
        .any(|case| case.kind == dctx0_core::VerificationCaseKind::EntityIdentity)
    {
        let (hash, plan): (_, dctx0_verification::ProgramPlan) =
            artifact(tx, run_id, run, "application/vnd.dctx0.plan+json")?;
        let data: String = tx.query_row(
            "SELECT data FROM snapshots WHERE id=?1",
            [&run.provenance.snapshot],
            |row| row.get(0),
        )?;
        let snapshot: Snapshot = decode(&data)?;
        if hash != run.provenance.plan_hash
            || !dctx0_verification::validate_entity_identity_cases(&snapshot.ir, &plan, &run.cases)
        {
            return Err(invalid(
                "Entity identity evidence does not reproduce its recorded native source observations",
            ));
        }
    }
    if manifest.open_obligations == run.open_obligations {
        return Ok(());
    }
    let (hash, plan): (_, dctx0_verification::ProgramPlan) =
        artifact(tx, run_id, run, "application/vnd.dctx0.plan+json")?;
    if hash != run.provenance.plan_hash
        || plan.schema_version != 1
        || plan.snapshot != run.provenance.snapshot
        || plan.check_occurrence != run.provenance.check_occurrence as u64
        || plan.budgets != run.budgets
        || plan.scope.subjects != run.selected_subjects
        || plan.closure != run.required_subjects
        || dctx0_verification::case_manifest(&plan) != *manifest
    {
        return Err(invalid(
            "Discharged obligations require the exact stored native plan and case manifest",
        ));
    }
    let data: String = tx.query_row(
        "SELECT data FROM snapshots WHERE id=?1",
        [&run.provenance.snapshot],
        |row| row.get(0),
    )?;
    let snapshot: Snapshot = decode(&data)?;
    let resolved =
        dctx0_verification::resolved_state_preconditions(&snapshot.ir, &plan, &run.cases);
    if !resolved.is_empty() {
        let (_, recorded): (_, Vec<VerificationIssue>) = artifact(
            tx,
            run_id,
            run,
            "application/vnd.dctx0.resolved-preconditions+json",
        )?;
        if recorded != resolved {
            return Err(invalid(
                "Discharged preconditions differ from actual-state reconstruction",
            ));
        }
    }
    let resolved_suites = dctx0_verification::suites::resolved_native_suite_policy_obligations(
        &snapshot.ir,
        &plan,
        &run.cases,
    );
    if !resolved_suites.is_empty() {
        let (_, recorded): (_, dctx0_verification::suites::SuitePolicyAssessment) = artifact(
            tx,
            run_id,
            run,
            "application/vnd.dctx0.suite-policy-assessment+json",
        )?;
        if recorded
            != dctx0_verification::suites::native_suite_policy_assessment(
                &snapshot.ir,
                &plan,
                &run.cases,
            )
        {
            return Err(invalid(
                "Discharged suites differ from actual native case reconstruction",
            ));
        }
    }
    let remaining = manifest
        .open_obligations
        .iter()
        .filter(|issue| !resolved.contains(issue) && !resolved_suites.contains(issue))
        .cloned()
        .collect::<Vec<_>>();
    if (resolved.is_empty() && resolved_suites.is_empty()) || remaining != run.open_obligations {
        return Err(invalid(
            "Discharged obligations lack independently reproduced actual-state evidence",
        ));
    }
    Ok(())
}
