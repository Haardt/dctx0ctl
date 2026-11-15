//! Validate immutable native origins before a planner can receive a candidate.
use super::*;

pub fn valid_completion(binding: &UnitEvidenceBindingV1, proof: &UnitCompletionProofV1) -> bool {
    valid_unit_binding(binding)
        && proof.schema_version == 1
        && valid_digest(&proof.native_run)
        && proof.unit == binding.unit.id
        && evidence_digest(binding).is_ok_and(|hash| hash == proof.fingerprint)
        && proof.generation > 0
        && proof.completed
        && proof.cleanup_completed
        && proof.dependencies_complete
        && matches!(
            binding.unit.boundary,
            UnitDependencyBoundary::Complete { .. }
        )
        && proof.case_bodies.keys().cloned().collect::<BTreeSet<_>>() == binding.unit.cases
        && proof.case_bodies.values().all(|hash| valid_digest(hash))
        && !proof.artifacts.is_empty()
        && proof.artifacts.contains(&proof.fingerprint)
        && proof.artifacts.iter().all(|hash| valid_digest(hash))
}

/// Artifact contents are independently fetched and hash-checked by the store.
/// A declaration, imported PASS or incomplete native cohort cannot grant reuse.
pub fn validate_native_unit(
    binding: UnitEvidenceBindingV1,
    proof: UnitCompletionProofV1,
    native_id: &str,
    original: &VerificationRun,
    artifacts: &BTreeMap<String, Vec<u8>>,
) -> Result<ValidatedUnitEvidence, String> {
    if !valid_completion(&binding, &proof)
        || proof.native_run != native_id
        || original.origin != VerificationRunOrigin::Native
        || !binding.unit.cases.is_subset(&original.required_cases)
        || crate::incremental::reuse_blocking_issues(original)
            .next()
            .is_some()
    {
        return Err("NATIVE_UNIT_ORIGIN_INVALID".into());
    }
    let mut records = BTreeMap::new();
    for record in &original.cases {
        if records.insert(&record.id, record).is_some() {
            return Err("DUPLICATE_NATIVE_CASE".into());
        }
    }
    for (id, hash) in &proof.case_bodies {
        let record = records.get(id).ok_or("NATIVE_UNIT_CASE_MISSING")?;
        if record.result != VerificationResult::Pass
            || evidence_digest(record).map_err(|e| e.to_string())? != *hash
        {
            return Err("NATIVE_UNIT_CASE_INVALID".into());
        }
    }
    for hash in &proof.artifacts {
        let reference = original
            .artifacts
            .iter()
            .find(|a| &a.hash == hash)
            .ok_or("NATIVE_UNIT_ARTIFACT_UNLINKED")?;
        let bytes = artifacts.get(hash).ok_or("NATIVE_UNIT_ARTIFACT_MISSING")?;
        if format!("{:x}", Sha256::digest(bytes)) != *hash || reference.bytes != bytes.len() as u64
        {
            return Err("NATIVE_UNIT_ARTIFACT_INVALID".into());
        }
    }
    let binding_reference = original
        .artifacts
        .iter()
        .find(|reference| {
            reference.media_type == UNIT_BINDING_MEDIA_TYPE
                && reference.hash == proof.fingerprint
                && proof.artifacts.contains(&reference.hash)
        })
        .ok_or("NATIVE_UNIT_BINDING_ARTIFACT_MISSING")?;
    let captured_binding: UnitEvidenceBindingV1 = serde_json::from_slice(
        artifacts
            .get(&binding_reference.hash)
            .ok_or("NATIVE_UNIT_BINDING_ARTIFACT_MISSING")?,
    )
    .map_err(|error| error.to_string())?;
    if captured_binding != binding {
        return Err("NATIVE_UNIT_BINDING_ARTIFACT_CHANGED".into());
    }
    let expected = UnitNativeCompletionV1 {
        schema_version: 1,
        unit: proof.unit.clone(),
        fingerprint: proof.fingerprint.clone(),
        generation: proof.generation,
        completed: true,
        cleanup_completed: true,
        dependencies_complete: true,
        case_bodies: proof.case_bodies.clone(),
    };
    let completion_matches = original
        .artifacts
        .iter()
        .filter(|reference| {
            reference.media_type == UNIT_COMPLETION_MEDIA_TYPE
                && proof.artifacts.contains(&reference.hash)
        })
        .any(|reference| {
            artifacts
                .get(&reference.hash)
                .and_then(|bytes| serde_json::from_slice::<UnitNativeCompletionV1>(bytes).ok())
                .as_ref()
                == Some(&expected)
        });
    if !completion_matches {
        return Err("NATIVE_UNIT_COMPLETION_ARTIFACT_MISSING".into());
    }
    Ok(ValidatedUnitEvidence { binding, proof })
}
