//! Explicit single-case replay observations in a diagnostic ledger. This is
//! deliberately separate from validated reuse and cannot authorize delivery.
use super::*;

pub(super) fn merge(
    project: &Project,
    store: &dctx0_store::Store,
    basis: &mut basis::Basis,
    sources: &BTreeSet<String>,
) -> Result<()> {
    if sources.len() > 64 {
        return Err(Error::Invalid(
            "At most 64 explicit observation sources are supported".into(),
        ));
    }
    let snapshot = store.load(&project.key(), &basis.program.snapshot)?;
    let root = &basis.root;
    let inventory = root
        .run
        .artifacts
        .iter()
        .find(|artifact| {
            artifact.media_type == "application/vnd.dctx0.captured-input-inventory+json"
        })
        .ok_or_else(|| Error::Invalid("Root input inventory missing".into()))?;
    for id in sources {
        let saved = store.verification_run(&project.key(), id)?;
        let run = &saved.run;
        let [case] = run.cases.as_slice() else {
            return Err(Error::Invalid(
                "Additional observation must be one exact native replay".into(),
            ));
        };
        if !store.verification_direct_parent(&project.key(), id, &root.id)?
            || run.origin != VerificationRunOrigin::Native
            || run.result != VerificationResult::Pass
            || !run.provenance.incomplete.is_empty()
            || case.result != VerificationResult::Pass
            || run.required_cases != BTreeSet::from([case.id.clone()])
            || run.provenance.snapshot != root.run.provenance.snapshot
            || run.provenance.check_occurrence != root.run.provenance.check_occurrence
            || run.provenance.configuration_hash != root.run.provenance.configuration_hash
            || run.provenance.adapter_hash != root.run.provenance.adapter_hash
            || run.provenance.interpreter_version != root.run.provenance.interpreter_version
            || run.provenance.generator_version != root.run.provenance.generator_version
            || run.provenance.runtime_version != root.run.provenance.runtime_version
            || !run.artifacts.contains(inventory)
            || !root.run.artifacts.iter().any(|artifact| {
                artifact.media_type == "application/vnd.dctx0.assessment-version+json"
                    && run.artifacts.contains(artifact)
            })
            || store.verification_later_case_problem(&project.key(), id, &case.id)?
        {
            return Err(Error::Invalid(format!(
                "Observation source lacks a compatible uncontradicted native replay: {id}"
            )));
        }
        let previous = basis
            .cases
            .iter()
            .position(|old| old.id == case.id)
            .ok_or_else(|| Error::Invalid("Replay has no original concrete input record".into()))?;
        if let Some(old_source) = basis.observations.get(&case.id)
            && store.verification_observation_order(&project.key(), old_source)?
                >= store.verification_observation_order(&project.key(), id)?
        {
            continue;
        }
        let mut projected = basis.program.clone();
        super::super::replay::only(&mut projected, &basis.cases[previous], Some(&snapshot.ir))?;
        projected.budgets.shrink_steps = 0;
        let artifact = store.verification_artifact(id, &run.provenance.plan_hash)?;
        let actual: ProgramPlan = serde_json::from_slice(&artifact.contents)?;
        if projected != actual {
            return Err(Error::Invalid(
                "Replay input projection differs from the original concrete case".into(),
            ));
        }
        basis.cases[previous] = case.clone();
        basis.observations.insert(case.id.clone(), id.clone());
    }
    Ok(())
}
