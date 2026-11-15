//! Composition-backed inspection keeps native origins and task pins separate.
use super::*;
use crate::Result;
use dctx0_core::{UnitEvidenceBindingV1, VerificationAttestation, VerificationResult};
use dctx0_verification::{
    ProgramPlan,
    attestations::{AttestationBinding, attestation_matches},
    unit_planner::{self, catalog},
};

/// Current inputs captured by the engine, independently of historical task IDs.
pub struct CompositionTaskPublication<'a> {
    pub composition: &'a str,
    pub program: &'a ProgramPlan,
    pub current_bindings: &'a [UnitEvidenceBindingV1],
    pub configuration: &'a str,
    pub input_hashes: &'a BTreeMap<String, String>,
}

/// Explicit evidence type: a composition is never presented as a native run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposedTaskReport {
    pub schema_version: u32,
    pub basis: String,
    pub composition: String,
    pub inspection: VerificationAttestation,
    pub check_occurrence: i64,
    pub unit_configuration: String,
    pub input_hashes: BTreeMap<String, String>,
    pub profile_bundle_hash: String,
}

/// Names include unit identity because generated inputs can differ by cohort.
pub fn composition_implementation(bindings: &[UnitEvidenceBindingV1]) -> BTreeMap<String, String> {
    bindings
        .iter()
        .flat_map(|binding| {
            binding
                .dependencies
                .iter()
                .map(move |(path, hash)| (format!("unit:{}:{path}", binding.unit.id), hash.clone()))
        })
        .collect()
}

pub(super) fn validate(
    tx: &Connection,
    project: &str,
    publication: &CompositionTaskPublication<'_>,
    snapshot: &Snapshot,
    check: i64,
) -> Result<()> {
    let program = publication.program;
    let required = dctx0_verification::case_manifest(program).required_cases;
    if program.snapshot != snapshot.id
        || i64::try_from(program.check_occurrence).ok() != Some(check)
        || required.is_empty()
    {
        return Err(invalid("COMPOSITION_PUBLICATION_PLAN_STALE"));
    }
    crate::verification::units::validate_current_composition(
        tx,
        project,
        publication.composition,
        publication.current_bindings,
        &required,
    )?;
    let logical = catalog::logical_unit_catalog(program).map_err(invalid)?;
    for binding in publication.current_bindings {
        let members: Vec<_> = logical
            .iter()
            .filter(|unit| !unit.cases.is_disjoint(&binding.unit.cases))
            .collect();
        if members
            .iter()
            .any(|unit| !unit.cases.is_subset(&binding.unit.cases))
        {
            return Err(invalid("COMPOSITION_SPLITS_ATOMIC_LOGICAL_UNIT"));
        }
        let mut descriptions = BTreeMap::new();
        let roots = members
            .iter()
            .flat_map(|unit| unit.requirements.iter().cloned())
            .collect();
        for member in members {
            for case in &member.cases {
                descriptions.insert(
                    case.clone(),
                    unit_planner::evidence_digest(&(&member.description_hash, case))?,
                );
            }
        }
        let requirements = snapshot
            .graph
            .index()
            .verification_closure(&roots)
            .into_iter()
            .map(|id| {
                let fingerprint = snapshot
                    .fingerprints
                    .get(&id)
                    .ok_or_else(|| invalid("COMPOSITION_REQUIREMENT_MISSING"))?;
                Ok((
                    id,
                    unit_planner::evidence_digest(&(
                        &fingerprint.contract,
                        &fingerprint.behavior,
                        &fingerprint.facts,
                    ))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        if descriptions != binding.logical_cases
            || requirements != binding.requirements
            || binding.semantics.get("budgets")
                != Some(&unit_planner::evidence_digest(&program.budgets)?)
            || binding.semantics.get("generator")
                != Some(&unit_planner::evidence_digest(&program.generator_version)?)
            || binding.semantics.get("compiler")
                != Some(&unit_planner::evidence_digest(
                    &snapshot.ir.compiler_version,
                )?)
            || binding.semantics.get("evaluator")
                != Some(&unit_planner::evidence_digest(
                    &dctx0_verification::VERIFICATION_VERSION,
                )?)
        {
            return Err(invalid(
                "COMPOSITION_LOGICAL_OR_REQUIREMENT_BINDING_CHANGED",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_delivery(
    publication: &CompositionTaskPublication<'_>,
    actor: &str,
    candidate: &Candidate,
    delivery: &TaskDelivery,
    review: Option<&ReviewReport>,
    profile_bundle: Option<&String>,
) -> Result<()> {
    let report: ComposedTaskReport = serde_json::from_str(&delivery.evidence.report)?;
    let implementation = composition_implementation(publication.current_bindings);
    let plan_hash = unit_planner::evidence_digest(publication.program)?;
    let requirements = candidate
        .subjects
        .iter()
        .chain(&candidate.types)
        .chain(&candidate.facts)
        .cloned()
        .collect();
    let subject = format!("T{}", delivery.task);
    let binding = AttestationBinding {
        subject: &subject,
        snapshot: &publication.program.snapshot,
        plan_hash: &plan_hash,
        requirements: &requirements,
        implementation_artifacts: &implementation,
    };
    let evidence_identity =
        dctx0_changes::hash(&(publication.composition, &plan_hash, &implementation));
    if report.schema_version != 1
        || report.basis != "native-evidence-composition+authored-inspection"
        || report.composition != publication.composition
        || i64::try_from(publication.program.check_occurrence).ok() != Some(report.check_occurrence)
        || report.unit_configuration != publication.configuration
        || &report.input_hashes != publication.input_hashes
        || profile_bundle != Some(&report.profile_bundle_hash)
        || report.inspection.actor != actor
        || report.inspection.result != VerificationResult::Pass
        || !attestation_matches(&report.inspection, &binding)
        || delivery.evidence.status != CheckStatus::Pass
        || delivery.evidence.verifier != actor
        || delivery.evidence.implementation != evidence_identity
        || delivery.result.implementation.as_deref() != Some(evidence_identity.as_str())
        || delivery.result.artifacts.is_empty()
        || delivery
            .result
            .artifacts
            .iter()
            .any(|path| !implementation.contains_key(path))
        || candidate
            .subjects
            .iter()
            .chain(&candidate.types)
            .any(|subject| !publication.program.closure.contains(subject))
    {
        return Err(invalid("COMPOSITION_TASK_INSPECTION_BINDING_INVALID"));
    }
    match (&candidate.kind, review) {
        (TaskKind::Review, Some(review))
            if review.reviewer == actor
                && review.judgement == "approved"
                && !review
                    .findings
                    .iter()
                    .any(|finding| finding.action_required || finding.severity == "high") =>
        {
            Ok(())
        }
        (kind, None) if *kind != TaskKind::Review => Ok(()),
        _ => Err(invalid("COMPOSITION_TASK_REVIEW_REQUIRED")),
    }
}

impl Store {
    /// Read-only current-context validation, with the same proof rules as write
    /// publication. It neither executes cases nor changes historical evidence.
    pub fn validate_composition_publication(
        &self,
        project: &str,
        publication: &CompositionTaskPublication<'_>,
    ) -> Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        let (check, snapshot_id) = latest_check(&tx, project)?;
        let data: String = tx.query_row(
            "SELECT data FROM snapshots WHERE id=?1",
            [&snapshot_id],
            |row| row.get(0),
        )?;
        let snapshot = storage::load(&tx, &data)?;
        validate(&tx, project, publication, &snapshot, check)?;
        tx.commit()?;
        Ok(())
    }
}
