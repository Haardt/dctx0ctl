//! Independently check discharged planning hints from immutable native evidence.
use super::*;
use crate::Result;
use dctx0_core::{VerificationCaseManifest, VerificationIssue, VerificationRun};
use serde::de::DeserializeOwned;

pub(super) fn artifact<T: DeserializeOwned>(
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
    let artifact = crate::verification::codec::read_artifact(tx, run_id, &reference.hash)?;
    if artifact.reference != **reference {
        return Err(invalid(
            "Native proof artifact integrity does not match its run reference",
        ));
    }
    let bytes = artifact.contents;
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("Invalid native proof artifact: {error}")))?;
    Ok((reference.hash.clone(), value))
}

fn sha256_string(value: &serde_json::Value) -> bool {
    value
        .as_str()
        .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn controlled_runtime_identity(runtime: &serde_json::Value) -> bool {
    runtime["schema_version"] == 3
        && runtime["method"] == "controlled-procfs-live-image"
        && runtime["launch"]["directory"]
            .as_str()
            .is_some_and(|path| std::path::Path::new(path).is_absolute())
        && sha256_string(&runtime["launch"]["parent_environment_hash"])
        && runtime["launch"]["forwarded_environment"]
            .as_array()
            .is_some_and(|names| {
                names.len() <= 4096
                    && names.iter().all(|name| {
                        name.as_str()
                            .is_some_and(|name| !name.is_empty() && !name.contains(['=', '\0']))
                    })
            })
        && runtime["launch"]["arguments"]
            .as_array()
            .is_some_and(|arguments| arguments.iter().all(serde_json::Value::is_string))
}

/// Recheck every historical edge inside the delivery transaction. The current
/// task/check/profile pins are checked separately by the publication path.
fn validate_incremental_lineage(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
) -> Result<()> {
    use dctx0_verification::{
        application_reuse::APPLICATION_BUNDLE_MEDIA_TYPE,
        incremental::{INCREMENTAL_INPUTS_MEDIA_TYPE, INCREMENTAL_PROOF_MEDIA_TYPE},
    };
    let project: String = tx.query_row(
        "SELECT project FROM verification_runs WHERE id=?1",
        [run_id],
        |r| r.get(0),
    )?;
    let mut current_id = run_id.to_owned();
    let mut current = run.clone();
    let mut seen = std::collections::BTreeSet::new();
    loop {
        if !seen.insert(current_id.clone()) || seen.len() > 256 {
            return Err(invalid(
                "Incremental ancestry is cyclic or exceeds the delivery depth limit",
            ));
        }
        if seen.len() > 1 {
            let ids: std::collections::BTreeSet<_> =
                current.cases.iter().map(|c| c.id.clone()).collect();
            if current.origin != dctx0_core::VerificationRunOrigin::Native
                || current.result != dctx0_core::VerificationResult::Pass
                || !current.open_obligations.is_empty()
                || !current.provenance.incomplete.is_empty()
                || ids != current.required_cases
                || ids.len() != current.cases.len()
                || current
                    .cases
                    .iter()
                    .any(|c| c.result != dctx0_core::VerificationResult::Pass)
            {
                return Err(invalid(
                    "Incremental task delivery requires complete passing ancestor evidence",
                ));
            }
        }
        if !current
            .artifacts
            .iter()
            .any(|a| a.media_type == INCREMENTAL_PROOF_MEDIA_TYPE)
        {
            return validate_incremental_root(tx, &current_id, &current);
        }
        let mut artifacts = Vec::new();
        for reference in current.artifacts.iter().filter(|a| {
            matches!(
                a.media_type.as_str(),
                INCREMENTAL_PROOF_MEDIA_TYPE
                    | INCREMENTAL_INPUTS_MEDIA_TYPE
                    | APPLICATION_BUNDLE_MEDIA_TYPE
                    | "application/vnd.dctx0.plan+json"
                    | "application/vnd.dctx0.application-native-receipt+json"
            )
        }) {
            let artifact =
                crate::verification::codec::read_artifact(tx, &current_id, &reference.hash)?;
            if artifact.reference != *reference {
                return Err(invalid(
                    "Incremental lineage artifact differs from its run reference",
                ));
            }
            artifacts.push(artifact);
        }
        let parent = crate::verification::validate_incremental(tx, &project, &current, &artifacts)?
            .ok_or_else(|| invalid("Incremental lineage has no validated parent"))?;
        if seen.len() > 1 {
            let (_, manifest): (_, VerificationCaseManifest) = artifact(
                tx,
                &current_id,
                &current,
                "application/vnd.dctx0.case-manifest+json",
            )?;
            let (_, plan): (_, dctx0_verification::ProgramPlan) =
                artifact(tx, &current_id, &current, "application/vnd.dctx0.plan+json")?;
            if dctx0_verification::case_manifest(&plan) != manifest {
                return Err(invalid(
                    "Incremental ancestor manifest does not match its pinned plan",
                ));
            }
            let (_, proof): (_, dctx0_verification::incremental::IncrementalProof) =
                artifact(tx, &current_id, &current, INCREMENTAL_PROOF_MEDIA_TYPE)?;
            validate_incremental_cohort(tx, &current_id, &current, &manifest, &proof)?;
            validate_identity_cases(tx, &current_id, &current)?;
            validate_remaining_obligations(tx, &current_id, &current, &manifest)?;
        }
        current = crate::verification::codec::read_run(tx, &project, &parent)?.run;
        current_id = parent;
    }
}

fn validate_incremental_root(tx: &Connection, run_id: &str, run: &VerificationRun) -> Result<()> {
    let (plan_hash, plan): (_, dctx0_verification::ProgramPlan) =
        artifact(tx, run_id, run, "application/vnd.dctx0.plan+json")?;
    let (_, manifest): (_, VerificationCaseManifest) =
        artifact(tx, run_id, run, "application/vnd.dctx0.case-manifest+json")?;
    if plan_hash != run.provenance.plan_hash
        || plan.snapshot != run.provenance.snapshot
        || i64::try_from(plan.check_occurrence).ok() != Some(run.provenance.check_occurrence)
        || plan.budgets != run.budgets
        || plan.scope.subjects != run.selected_subjects
        || plan.closure != run.required_subjects
        || dctx0_verification::case_manifest(&plan) != manifest
        || manifest.required_cases != run.required_cases
    {
        return Err(invalid(
            "Incremental ancestor plan or manifest does not match its native evidence",
        ));
    }
    let is_hybrid = run
        .artifacts
        .iter()
        .any(|a| a.media_type == dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE);
    let is_application = run.provenance.protocol_version
        == dctx0_verification::application_evidence::APPLICATION_EXECUTION_PROTOCOL_VERSION;
    if is_application
        && !run.artifacts.iter().any(|a| {
            a.media_type
                == dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE
        })
    {
        return Err(invalid(
            "Incremental ancestor lacks a complete application execution proof",
        ));
    }
    if !is_application && !is_hybrid {
        let (_, audit): (_, serde_json::Value) =
            artifact(tx, run_id, run, "application/vnd.dctx0.native-audit+json")?;
        let (_, limits): (_, serde_json::Value) =
            artifact(tx, run_id, run, "application/vnd.dctx0.native-limits+json")?;
        if !successful_native_audit(&audit) || !valid_native_limits(&limits) {
            return Err(invalid(
                "Incremental ancestor lacks a complete managed native audit",
            ));
        }
    }
    validate_open_obligations(tx, run_id, run, &manifest)
}

fn successful_native_audit(audit: &serde_json::Value) -> bool {
    audit["pid"]
        .as_u64()
        .is_some_and(|pid| pid > 0 && u32::try_from(pid).is_ok())
        && audit["received_messages"].as_u64().is_some_and(|n| n > 0)
        && audit["stderr"].is_string()
        && audit["stdout"].is_string()
        && audit["exit_code"] == 0
        && audit["supervision"]["process_tree"] == true
}

fn valid_native_limits(limits: &serde_json::Value) -> bool {
    limits["schema_version"] == 1
        && limits["limits"]["supervision"] == "auto"
        && ["initialization", "case", "run", "cleanup"]
            .iter()
            .all(|name| {
                let duration = &limits["limits"][name];
                duration["secs"]
                    .as_u64()
                    .zip(duration["nanos"].as_u64())
                    .is_some_and(|(secs, nanos)| (secs > 0 || nanos > 0) && nanos < 1_000_000_000)
            })
        && [
            "log_bytes",
            "message_bytes",
            "evidence_bytes",
            "evidence_entries",
        ]
        .iter()
        .all(|name| {
            limits["limits"][name]
                .as_u64()
                .is_some_and(|n| n > 0 && usize::try_from(n).is_ok())
        })
}

fn validate_hybrid(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
) -> Result<()> {
    use dctx0_verification::hybrid_evidence::{HYBRID_EXECUTION_MEDIA_TYPE, HybridExecutionProof};
    let (_, proof): (_, HybridExecutionProof) =
        artifact(tx, run_id, run, HYBRID_EXECUTION_MEDIA_TYPE)?;
    validate_hybrid_artifacts(tx, run_id, run, manifest, &proof, proof.validates(run))
}

fn validate_hybrid_artifacts(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
    proof: &dctx0_verification::hybrid_evidence::HybridExecutionProof,
    valid_cohort: bool,
) -> Result<()> {
    let (plan_hash, plan): (_, dctx0_verification::ProgramPlan) =
        artifact(tx, run_id, run, "application/vnd.dctx0.plan+json")?;
    let (receipt_hash, receipt): (_, serde_json::Value) = artifact(
        tx,
        run_id,
        run,
        "application/vnd.dctx0.application-native-receipt+json",
    )?;
    let (_, audit): (_, serde_json::Value) =
        artifact(tx, run_id, run, "application/vnd.dctx0.native-audit+json")?;
    let (_, runtime): (_, serde_json::Value) = artifact(
        tx,
        run_id,
        run,
        "application/vnd.dctx0.runtime-identity+json",
    )?;
    let (_, limits): (_, serde_json::Value) =
        artifact(tx, run_id, run, "application/vnd.dctx0.native-limits+json")?;
    let (_, supervision): (_, serde_json::Value) = artifact(
        tx,
        run_id,
        run,
        "application/vnd.dctx0.native-supervision+json",
    )?;
    let app = &proof.application;
    let phases_match = receipt["phases"].as_array().is_some_and(|phases| {
        phases.len() == app.phases.len()
            && phases.iter().zip(&app.phases).all(|(actual, expected)| {
                actual["phase"] == expected.phase
                    && actual["runtime"]["executable_hash"] == expected.runtime_hash
                    && sha256_string(&actual["runtime"]["executable_hash"])
                    && actual["audit"]["exit_code"].as_i64() == expected.exit_code.map(i64::from)
                    && actual["audit"]["supervision"]["process_tree"].as_bool()
                        == Some(expected.supervised)
                    && actual["audit"]["pid"]
                        .as_u64()
                        .is_some_and(|pid| pid > 0 && u32::try_from(pid).is_ok())
            })
    });
    if !valid_cohort
        || plan_hash != run.provenance.plan_hash
        || serde_json::to_vec(&plan)? != serde_json::to_vec(&app.plan.program)?
        || dctx0_verification::case_manifest(&plan) != *manifest
        || receipt_hash != app.receipt_hash
        || receipt_hash != proof.application_run.provenance.harness_hash
        || receipt["schema_version"] != 1
        || receipt["backend"] != "application-tests"
        || receipt["group"] != app.assessment.group
        || receipt["target"] != proof.application_run.provenance.target
        || receipt["result"] != serde_json::json!({"status":"PASS"})
        || receipt["issues"] != serde_json::json!([])
        || !receipt["evidence"].is_null()
        || receipt["adoption_authority"] != false
        || receipt["adopted_tasks"] != serde_json::json!([])
        || receipt["adoption_issues"] != serde_json::json!([])
        || !receipt["adoption_proposal"].is_null()
        || !receipt["replay_of"].is_null()
        || !receipt["requested_case"].is_null()
        || receipt["plan_hash"] != proof.application_run.provenance.adapter_hash
        || receipt["assessment"] != serde_json::to_value(&app.assessment)?
        || receipt["source_inputs"] != serde_json::to_value(&app.source_inputs)?
        || receipt["build_inputs"] != serde_json::to_value(&app.build_inputs)?
        || receipt["materialized_inputs"] != serde_json::to_value(&app.materialized_inputs)?
        || receipt["toolchain_inputs"] != serde_json::to_value(&app.toolchain_inputs)?
        || !phases_match
        || !successful_native_audit(&audit)
        || (runtime["schema_version"] != 1 && !controlled_runtime_identity(&runtime))
        || !sha256_string(&runtime["executable_hash"])
        || !runtime["executable_path"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
        || !runtime["executable_bytes"].as_u64().is_some_and(|n| n > 0)
        || !sha256_string(&runtime["initial_environment_hash"])
        || !valid_native_limits(&limits)
        || supervision["process_tree"] != true
        || !supervision["detached_descendants"].is_boolean()
        || !supervision["collective_memory_limit"].is_boolean()
    {
        return Err(invalid(
            "Hybrid task delivery requires the complete fresh cohort and matching managed native artifacts",
        ));
    }
    Ok(())
}

fn validate_hybrid_inspections(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
) -> Result<()> {
    use dctx0_verification::program_inspection::{
        EffectiveProgramAssessment, ProgramInspection, assess_with_inspections,
    };
    if !run.artifacts.iter().any(|artifact| {
        artifact.media_type == dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE
    }) || run.origin != dctx0_core::VerificationRunOrigin::Native
        || run.result != dctx0_core::VerificationResult::Pass
        || !run.provenance.incomplete.is_empty()
    {
        return Err(invalid(
            "Only a complete fresh hybrid run can carry inspected native obligations",
        ));
    }
    let (_, inspections): (_, Vec<ProgramInspection>) = artifact(
        tx,
        run_id,
        run,
        "application/vnd.dctx0.program-inspections+json",
    )?;
    let (_, recorded): (_, EffectiveProgramAssessment) = artifact(
        tx,
        run_id,
        run,
        "application/vnd.dctx0.effective-program-assessment+json",
    )?;
    let (hash, plan): (_, dctx0_verification::ProgramPlan) =
        artifact(tx, run_id, run, "application/vnd.dctx0.plan+json")?;
    let data: String = tx.query_row(
        "SELECT data FROM snapshots WHERE id=?1",
        [&run.provenance.snapshot],
        |row| row.get(0),
    )?;
    let snapshot: Snapshot = storage::load(tx, &data)?;
    if hash != run.provenance.plan_hash
        || plan.snapshot != run.provenance.snapshot
        || i64::try_from(plan.check_occurrence).ok() != Some(run.provenance.check_occurrence)
        || plan.budgets != run.budgets
        || plan.scope.subjects != run.selected_subjects
        || plan.closure != run.required_subjects
        || dctx0_verification::case_manifest(&plan) != *manifest
    {
        return Err(invalid(
            "Inspected hybrid assessment requires the original complete native plan",
        ));
    }
    let assessed = assess_with_inspections(
        &snapshot.ir,
        &plan,
        &run.cases,
        &run.provenance.plan_hash,
        &run.provenance.implementation,
        &inspections,
    );
    if assessed != recorded
        || assessed.result != dctx0_core::VerificationResult::Pass
        || !assessed.open_obligations.is_empty()
        || run.open_obligations != assessed.open_obligations
    {
        return Err(invalid(
            "Inspected hybrid obligations do not reproduce the bound authored assessment",
        ));
    }
    Ok(())
}

fn validate_application_artifacts(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
    proof: &dctx0_verification::application_evidence::ApplicationExecutionProof,
    valid_cohort: bool,
) -> Result<()> {
    let (receipt_hash, receipt): (_, serde_json::Value) = artifact(
        tx,
        run_id,
        run,
        "application/vnd.dctx0.application-native-receipt+json",
    )?;
    let (plan_hash, plan): (_, dctx0_verification::ProgramPlan) =
        artifact(tx, run_id, run, "application/vnd.dctx0.plan+json")?;
    if !valid_cohort
        || proof.receipt_hash != receipt_hash
        || receipt["backend"] != "application-tests"
        || receipt["result"]["status"] != "PASS"
        || receipt["plan_hash"] != run.provenance.adapter_hash
        || !receipt["replay_of"].is_null()
        || !receipt["requested_case"].is_null()
        || receipt["issues"] != serde_json::json!([])
        || receipt["assessment"] != serde_json::to_value(&proof.assessment)?
        || receipt["source_inputs"] != serde_json::to_value(&proof.source_inputs)?
        || receipt["build_inputs"] != serde_json::to_value(&proof.build_inputs)?
        || receipt["materialized_inputs"] != serde_json::to_value(&proof.materialized_inputs)?
        || receipt["toolchain_inputs"] != serde_json::to_value(&proof.toolchain_inputs)?
        || plan_hash != run.provenance.plan_hash
        || serde_json::to_vec(&plan)? != serde_json::to_vec(&proof.plan.program)?
        || dctx0_verification::case_manifest(&plan) != *manifest
    {
        return Err(invalid(
            "Application task delivery requires the exact complete native group, plan and bound receipt",
        ));
    }
    Ok(())
}

fn validate_identity_cases(tx: &Connection, run_id: &str, run: &VerificationRun) -> Result<()> {
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
        let snapshot: Snapshot = storage::load(tx, &data)?;
        if hash != run.provenance.plan_hash
            || !dctx0_verification::validate_entity_identity_cases(&snapshot.ir, &plan, &run.cases)
        {
            return Err(invalid(
                "Entity identity evidence does not reproduce its recorded native source observations",
            ));
        }
    }
    Ok(())
}

fn validate_incremental_cohort(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
    incremental: &dctx0_verification::incremental::IncrementalProof,
) -> Result<()> {
    let (plan_hash, plan): (_, dctx0_verification::ProgramPlan) =
        artifact(tx, run_id, run, "application/vnd.dctx0.plan+json")?;
    if plan_hash != run.provenance.plan_hash
        || dctx0_verification::case_manifest(&plan) != *manifest
    {
        return Err(invalid(
            "Incremental task coverage must match the complete pinned plan manifest",
        ));
    }
    use dctx0_verification::application_evidence::{
        INCREMENTAL_APPLICATION_EXECUTION_MEDIA_TYPE, IncrementalApplicationExecutionProof,
    };
    use dctx0_verification::hybrid_evidence::{
        INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE, IncrementalHybridExecutionProof,
    };
    if run.artifacts.iter().any(|a| {
        matches!(
            a.media_type.as_str(),
            dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE
                | dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE
        )
    }) {
        return Err(invalid(
            "Incremental evidence cannot claim a fresh-only execution proof",
        ));
    }
    if run
        .artifacts
        .iter()
        .any(|a| a.media_type == INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE)
    {
        let (_, proof): (_, IncrementalHybridExecutionProof) =
            artifact(tx, run_id, run, INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE)?;
        validate_hybrid_artifacts(
            tx,
            run_id,
            run,
            manifest,
            &proof.cohort,
            proof.validates(run, incremental),
        )?;
    } else if run.provenance.protocol_version
        == dctx0_verification::application_evidence::APPLICATION_EXECUTION_PROTOCOL_VERSION
    {
        let (_, proof): (_, IncrementalApplicationExecutionProof) = artifact(
            tx,
            run_id,
            run,
            INCREMENTAL_APPLICATION_EXECUTION_MEDIA_TYPE,
        )?;
        validate_application_artifacts(
            tx,
            run_id,
            run,
            manifest,
            &proof.application,
            proof.validates(run, incremental),
        )?;
    } else {
        if run
            .artifacts
            .iter()
            .any(|a| a.media_type == "application/vnd.dctx0.application-native-receipt+json")
        {
            return Err(invalid(
                "Incremental hybrid delivery requires its typed cohort proof",
            ));
        }
        let (_, audit): (_, serde_json::Value) =
            artifact(tx, run_id, run, "application/vnd.dctx0.native-audit+json")?;
        let (_, limits): (_, serde_json::Value) =
            artifact(tx, run_id, run, "application/vnd.dctx0.native-limits+json")?;
        if !successful_native_audit(&audit) || !valid_native_limits(&limits) {
            return Err(invalid(
                "Incremental delivery requires managed native audit and limits",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_open_obligations(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
) -> Result<()> {
    let incremental = if run.artifacts.iter().any(|reference| {
        reference.media_type == dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE
    }) {
        validate_incremental_lineage(tx, run_id, run)?;
        Some(
            artifact::<dctx0_verification::incremental::IncrementalProof>(
                tx,
                run_id,
                run,
                dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE,
            )?
            .1,
        )
    } else {
        None
    };
    if let Some(incremental) = &incremental {
        validate_incremental_cohort(tx, run_id, run, manifest, incremental)?;
    }
    if run.artifacts.iter().any(|reference| {
        reference.media_type == dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE
    }) {
        validate_hybrid(tx, run_id, run, manifest)?;
    }
    let application_kind =
        dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE;
    if run
        .artifacts
        .iter()
        .any(|reference| reference.media_type == application_kind)
    {
        let (_, proof): (
            _,
            dctx0_verification::application_evidence::ApplicationExecutionProof,
        ) = artifact(tx, run_id, run, application_kind)?;
        validate_application_artifacts(tx, run_id, run, manifest, &proof, proof.validates(run))?;
    }
    validate_identity_cases(tx, run_id, run)?;
    validate_remaining_obligations(tx, run_id, run, manifest)
}

pub(super) fn validate_remaining_obligations(
    tx: &Connection,
    run_id: &str,
    run: &VerificationRun,
    manifest: &VerificationCaseManifest,
) -> Result<()> {
    if run.artifacts.iter().any(|artifact| {
        matches!(
            artifact.media_type.as_str(),
            "application/vnd.dctx0.program-inspections+json"
                | "application/vnd.dctx0.effective-program-assessment+json"
        )
    }) {
        return validate_hybrid_inspections(tx, run_id, run, manifest);
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
    let snapshot: Snapshot = storage::load(tx, &data)?;
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

#[cfg(test)]
mod tests;
