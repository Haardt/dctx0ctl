//! Pure binding validation for explicit agent inspection. This is not a signature
//! verifier: the engine/store must authenticate publication and retain history.
use dctx0_core::{
    AttestedVerificationResult, VerificationAttestation, VerificationEvidenceBasis as Basis,
    VerificationResult as R,
};
use std::collections::{BTreeMap, BTreeSet};

pub struct AttestationBinding<'a> {
    pub subject: &'a str,
    pub snapshot: &'a str,
    pub plan_hash: &'a str,
    pub requirements: &'a BTreeSet<String>,
    pub implementation_artifacts: &'a BTreeMap<String, String>,
}

/// Requires complete exact bindings. Does not infer requirements from prose.
pub fn attestation_matches(
    attestation: &VerificationAttestation,
    binding: &AttestationBinding<'_>,
) -> bool {
    attestation.schema_version == 1
        && !attestation.actor.trim().is_empty()
        && !attestation.report.trim().is_empty()
        && !binding.subject.is_empty()
        && !binding.snapshot.is_empty()
        && !binding.plan_hash.is_empty()
        && !binding.requirements.is_empty()
        && !binding.implementation_artifacts.is_empty()
        && binding.requirements.iter().all(|id| !id.is_empty())
        && binding
            .implementation_artifacts
            .iter()
            .all(|(path, hash)| !path.is_empty() && !hash.is_empty())
        && attestation.subject == binding.subject
        && attestation.snapshot == binding.snapshot
        && attestation.plan_hash == binding.plan_hash
        && &attestation.requirements == binding.requirements
        && &attestation.implementation_artifacts == binding.implementation_artifacts
}

/// A bound agent inspection may resolve UNKNOWN, while automatic failures and
/// their counterexamples remain authoritative. Automatic status is always kept.
pub fn apply_attestation(
    automatic: &R,
    attestation: &VerificationAttestation,
    binding: &AttestationBinding<'_>,
) -> AttestedVerificationResult {
    let applicable =
        attestation_matches(attestation, binding) && !matches!(automatic, R::Fail { .. });
    let (effective, basis) = if applicable {
        match (automatic, &attestation.result) {
            (R::Pass, R::Pass) => (R::Pass, Basis::Mixed),
            (_, R::Fail { .. }) => (attestation.result.clone(), Basis::Mixed),
            (R::Unknown { .. }, R::Pass) => (R::Pass, Basis::Agent),
            _ => (automatic.clone(), Basis::Automatic),
        }
    } else {
        (automatic.clone(), Basis::Automatic)
    };
    AttestedVerificationResult {
        automatic: automatic.clone(),
        effective,
        basis,
    }
}
