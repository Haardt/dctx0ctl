//! Static upper-bound planning before capture materialization or native startup.
//! A successful preflight is provisional; execution still validates live inputs.
use super::*;
use dctx0_verification::incremental as reuse;

pub(super) async fn guard(
    project: &Project,
    application: &Path,
    program: &ProgramPlan,
    options: &NativeReuseOptions,
    cancellation: &NativeCancellation,
) -> Result<()> {
    if options.mode == NativeReuseMode::Off {
        if options.parent.is_some() {
            return Err(Error::Invalid(
                "--reuse=off conflicts with --reuse-from".into(),
            ));
        }
        return Ok(());
    }
    let required = dctx0_verification::case_manifest(program).required_cases;
    if required.is_empty() {
        return Ok(());
    }
    let store = project.store(AccessMode::ReadOnly)?;
    let parent_id = match &options.parent {
        Some(id) => Some(id.clone()),
        None => store.verification_reuse_candidate(&project.key(), &program.scope.subjects)?,
    };
    let mut reused = BTreeSet::new();
    let mut reasons = Vec::new();
    if let Some(id) = &parent_id {
        let saved = store.verification_run(&project.key(), id)?;
        let reference = saved
            .run
            .artifacts
            .iter()
            .find(|a| a.media_type == reuse::INCREMENTAL_INPUTS_MEDIA_TYPE);
        if let Some(reference) = reference {
            let mut inputs = options.inputs.clone();
            if inputs.is_empty()
                && application
                    .join(".dctx0/testing/reuse-inputs.json")
                    .exists()
            {
                inputs = ".dctx0/testing/reuse-inputs.json".into();
            }
            let receipt =
                super::reuse_inputs::capture_static(application, &inputs, cancellation).await?;
            reasons.extend(receipt.incomplete.iter().map(|gap| gap.code.clone()));
            let old_receipt = store.verification_artifact(id, &reference.hash)?;
            let old_receipt: reuse::ExternalInputReceipt =
                reuse::code_receipt(&serde_json::from_slice(&old_receipt.contents)?);
            reasons.extend(
                reuse::receipt_changes(&old_receipt, &receipt)
                    .into_iter()
                    .map(|change| format!("{}:{}", change.code, change.input)),
            );
            let old_program = store.verification_artifact(id, &saved.run.provenance.plan_hash)?;
            let old_program: ProgramPlan = serde_json::from_slice(&old_program.contents)?;
            let old = reuse::binding(
                &old_program,
                saved.run.provenance.clone(),
                digest(&serde_json::to_vec(&old_receipt)?),
            )?;
            // Execution replaces generated harness files before deriving its
            // implementation map. Compare the original full discovery receipt
            // first; projecting a raw inventory into that map would falsely
            // invalidate unchanged generated files (or hide newly added files).
            let inventory_matches = if let Some(reference) = saved
                .run
                .artifacts
                .iter()
                .find(|a| a.media_type == "application/vnd.dctx0.captured-input-inventory+json")
            {
                let artifact = store.verification_artifact(id, &reference.hash)?;
                Capture::reuse_inventory_matches(
                    application,
                    project
                        .verification_configuration(Some(application))?
                        .1
                        .capture_budgets(),
                    &saved.run,
                    &serde_json::from_slice(&artifact.contents)?,
                )?
            } else {
                false
            };
            if !inventory_matches {
                reasons.push("CAPTURED_SOURCE_DISCOVERY_CHANGED_OR_MISSING".into());
            }
            let current = reuse::binding(
                program,
                saved.run.provenance.clone(),
                digest(&serde_json::to_vec(&receipt)?),
            )?;
            let units = reuse::receipt_units(program, &receipt).ok_or_else(|| {
                Error::Invalid(
                    "Reuse declaration must select complete known deterministic units".into(),
                )
            })?;
            if old_receipt == receipt
                && inventory_matches
                && store
                    .verification_later_nonpass(&project.key(), id)?
                    .is_empty()
                && let Some(plan) = reuse::plan_reuse_projected(
                    &old,
                    &current,
                    &saved.run,
                    &units,
                    &saved.run.provenance,
                )
            {
                reused = plan.reused;
            }
            if reused.is_empty() {
                reasons.push("COMPLETE_CURRENT_NATIVE_UNIT_EVIDENCE_UNAVAILABLE".into());
            }
        } else {
            reasons.push("PARENT_INPUT_RECEIPT_MISSING".into());
        }
    } else {
        reasons.push("NATIVE_BASIS_MISSING".into());
    }
    dctx0_verification::unit_planner::execution_gate(&required,&reused,false,
        options.mode == NativeReuseMode::Require,parent_id.is_some()).map_err(|code|Error::Invalid(format!(
            "{code}: {} mandatory cases, {} statically reusable; reasons={}; no native execution started. A full execution requires --reuse=off",
            required.len(),reused.len(),reasons.join(","))))
}
