//! Pure unit planning. Callers validate native origins before supplying candidates.
use dctx0_core::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub mod catalog;
pub mod native;

pub fn evidence_digest(value: &impl Serialize) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

pub fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

pub fn valid_unit_binding(binding: &UnitEvidenceBindingV1) -> bool {
    binding.schema_version == UNIT_EVIDENCE_VERSION
        && !binding.unit.id.is_empty()
        && !binding.unit.cases.is_empty()
        && binding.unit.cases.iter().all(|id| !id.is_empty())
        && binding
            .logical_cases
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            == binding.unit.cases
        && !binding.requirements.is_empty()
        && !binding.dependencies.is_empty()
        && !binding.build_inputs.is_empty()
        && !binding.semantics.is_empty()
        && [
            &binding.logical_cases,
            &binding.requirements,
            &binding.dependencies,
            &binding.fixtures,
            &binding.build_inputs,
            &binding.semantics,
        ]
        .into_iter()
        .all(|map| {
            map.iter()
                .all(|(key, hash)| !key.is_empty() && valid_digest(hash))
        })
        && binding
            .environment
            .iter()
            .all(|(key, hash)| !key.is_empty() && hash.as_deref().is_none_or(valid_digest))
        && match &binding.unit.boundary {
            UnitDependencyBoundary::Complete {
                analyzer,
                version,
                roots,
                discovery,
            } => {
                !analyzer.is_empty()
                    && !version.is_empty()
                    && !roots.is_empty()
                    && roots.iter().all(|(role, paths)| {
                        !role.is_empty()
                            && !paths.is_empty()
                            && paths
                                .iter()
                                .all(|path| binding.dependencies.contains_key(path))
                    })
                    && !discovery.is_empty()
                    && discovery
                        .iter()
                        .all(|(key, hash)| !key.is_empty() && valid_digest(hash))
            }
            UnitDependencyBoundary::Unknown { group, reasons } => {
                !group.is_empty() && !reasons.is_empty()
            }
        }
}

/// Constructed only after checking the immutable original and required artifacts.
#[derive(Debug, Clone)]
pub struct ValidatedUnitEvidence {
    binding: UnitEvidenceBindingV1,
    proof: UnitCompletionProofV1,
}
impl ValidatedUnitEvidence {
    pub fn binding(&self) -> &UnitEvidenceBindingV1 {
        &self.binding
    }
    pub fn proof(&self) -> &UnitCompletionProofV1 {
        &self.proof
    }
}

pub struct UnitPlannerInput<'a> {
    pub required_cases: &'a BTreeSet<String>,
    pub historical_cases: &'a BTreeSet<String>,
    pub bindings: &'a [UnitEvidenceBindingV1],
    pub candidates: &'a [ValidatedUnitEvidence],
    pub prior_bindings: &'a [UnitEvidenceBindingV1],
    pub attempts: &'a [UnitAttemptV1],
    pub reuse_from: Option<&'a BTreeSet<String>>,
    pub force: bool,
}

fn reason(code: &str, input: &str) -> UnitPlanReason {
    UnitPlanReason {
        code: code.into(),
        input: input.into(),
    }
}

/// Exact coverage is validated across all backends together, before execution.
pub fn validate_unit_partition<'a>(
    required: &BTreeSet<String>,
    units: impl IntoIterator<Item = &'a VerificationUnit>,
) -> Result<(), String> {
    let mut cases = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for unit in units {
        if unit.id.is_empty() || unit.cases.is_empty() || !ids.insert(&unit.id) {
            return Err("UNIT_PARTITION_INVALID: empty or duplicate unit".into());
        }
        for case in &unit.cases {
            if case.is_empty() || !cases.insert(case.clone()) {
                return Err(format!(
                    "UNIT_PARTITION_INVALID: duplicate or empty case {case}"
                ));
            }
        }
    }
    if &cases != required {
        return Err("UNIT_PARTITION_INVALID: incomplete or extra coverage".into());
    }
    Ok(())
}

/// No I/O, provider queries, task mutation or native execution occurs here.
pub fn plan_units(input: UnitPlannerInput<'_>) -> Result<UnitExecutionPlan, String> {
    validate_unit_partition(input.required_cases, input.bindings.iter().map(|b| &b.unit))?;
    let mut decisions = Vec::new();
    for binding in input.bindings {
        if !valid_unit_binding(binding) {
            return Err(format!("UNIT_BINDING_INVALID: {}", binding.unit.id));
        }
        let fingerprint = evidence_digest(binding).map_err(|e| e.to_string())?;
        let newest = input
            .attempts
            .iter()
            .filter(|a| a.unit == binding.unit.id && a.fingerprint == fingerprint)
            .max_by_key(|a| a.generation);
        let candidate = input
            .candidates
            .iter()
            .filter(|c| {
                c.binding == *binding
                    && input
                        .reuse_from
                        .is_none_or(|allowed| allowed.contains(&c.proof.native_run))
                    && newest.is_some_and(|a| {
                        a.generation == c.proof.generation && a.outcome == UnitAttemptOutcome::Pass
                    })
            })
            .max_by_key(|c| c.proof.generation);
        let (action, source, reasons) = if input.force {
            (
                UnitExecutionAction::ExecuteAlways,
                None,
                vec![reason("FRESH_EXECUTION_REQUESTED", &binding.unit.id)],
            )
        } else if binding.unit.execute_always {
            (
                UnitExecutionAction::ExecuteAlways,
                None,
                vec![reason("EXTERNAL_STATE_NOT_VERSIONED", &binding.unit.id)],
            )
        } else if matches!(
            binding.unit.boundary,
            UnitDependencyBoundary::Unknown { .. }
        ) {
            (
                UnitExecutionAction::ExecuteUnbound,
                None,
                vec![reason("DEPENDENCY_BOUNDARY_UNPROVEN", &binding.unit.id)],
            )
        } else if let Some(candidate) = candidate {
            (
                UnitExecutionAction::Reuse,
                Some(candidate.proof.clone()),
                vec![],
            )
        } else if newest.is_some_and(|a| a.outcome != UnitAttemptOutcome::Pass) {
            (
                UnitExecutionAction::ExecuteUnbound,
                None,
                vec![reason("LATEST_ATTEMPT_NOT_PASS", &binding.unit.id)],
            )
        } else if input.reuse_from.is_some()
            && input.candidates.iter().any(|c| c.binding == *binding)
        {
            (
                UnitExecutionAction::ExecuteUnbound,
                None,
                vec![reason("REUSE_SOURCE_RESTRICTED", &binding.unit.id)],
            )
        } else if input
            .candidates
            .iter()
            .any(|c| c.binding.unit.id == binding.unit.id)
            || input
                .prior_bindings
                .iter()
                .any(|prior| prior.unit.id == binding.unit.id && prior != binding)
        {
            (
                UnitExecutionAction::ExecuteChanged,
                None,
                vec![reason("UNIT_BINDING_CHANGED", &binding.unit.id)],
            )
        } else if input
            .prior_bindings
            .iter()
            .any(|prior| prior.unit.id == binding.unit.id)
        {
            (
                UnitExecutionAction::ExecuteUnbound,
                None,
                vec![reason(
                    "COMPLETE_NATIVE_UNIT_EVIDENCE_MISSING",
                    &binding.unit.id,
                )],
            )
        } else {
            (
                UnitExecutionAction::ExecuteNew,
                None,
                vec![reason(
                    "COMPLETE_NATIVE_UNIT_EVIDENCE_MISSING",
                    &binding.unit.id,
                )],
            )
        };
        decisions.push(UnitExecutionDecision {
            unit: binding.unit.id.clone(),
            cases: binding.unit.cases.clone(),
            fingerprint,
            action,
            source,
            reasons,
        });
    }
    Ok(UnitExecutionPlan {
        schema_version: 1,
        required_cases: input.required_cases.clone(),
        decisions,
        removed_cases: input
            .historical_cases
            .difference(input.required_cases)
            .cloned()
            .collect(),
    })
}

/// Shared pre-execution gate. The caller must pass the UNION of backend scopes.
pub fn execution_gate(
    required: &BTreeSet<String>,
    reused: &BTreeSet<String>,
    force: bool,
    require: bool,
    has_basis: bool,
) -> Result<(), &'static str> {
    if !reused.is_subset(required) {
        return Err("UNIT_PARTITION_INVALID");
    }
    if force {
        return Ok(());
    }
    if require && has_basis && reused.is_empty() {
        return Err("REUSE_REQUIRED");
    }
    if !required.is_empty() && reused.is_empty() {
        return Err("FULL_EXECUTION_REQUIRES_EXPLICIT_OFF");
    }
    Ok(())
}

/// A complete partition remains visible even when individual units lack evidence.
pub fn validate_composition(composition: &EvidenceCompositionV1) -> Result<(), String> {
    if composition.schema_version != 1
        || !composition
            .removed_cases
            .is_disjoint(&composition.required_cases)
    {
        return Err("COMPOSITION_INVALID".into());
    }
    validate_unit_partition(
        &composition.required_cases,
        composition.units.iter().map(|u| &u.binding.unit),
    )?;
    for unit in &composition.units {
        if !valid_unit_binding(&unit.binding) {
            return Err("UNIT_BINDING_INVALID".into());
        }
        if matches!(
            unit.coverage,
            UnitCoverage::ReusedPass | UnitCoverage::FreshPass
        ) {
            let proof = unit.proof.as_ref().ok_or("UNIT_COMPLETION_MISSING")?;
            if !native::valid_completion(&unit.binding, proof) {
                return Err("UNIT_COMPLETION_INVALID".into());
            }
        } else if unit.proof.is_some() {
            return Err("NONPASS_HAS_PASS_PROOF".into());
        }
    }
    Ok(())
}
