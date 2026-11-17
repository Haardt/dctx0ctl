//! Conservative, explicit reuse planning. This module does not grant adoption authority.
//!
//! Callers derive connected units and capture explicitly declared inputs. A
//! receipt binds those declarations; it does not prove process isolation or
//! observe undeclared external state. Reuse is historical evidence and does not
//! itself authorize delivery of tasks.
use dctx0_core::{
    VerificationBudgets, VerificationProvenance, VerificationResult, VerificationRun,
    VerificationRunOrigin,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
mod decisions;
pub use decisions::*;
mod code;
pub use code::*;

pub const INCREMENTAL_SCHEMA_VERSION: u32 = 1;
pub const INPUT_RECEIPT_SCHEMA_VERSION: u32 = 2;
pub const INCREMENTAL_PROOF_MEDIA_TYPE: &str = "application/vnd.dctx0.incremental-proof+json";
pub const INCREMENTAL_INPUTS_MEDIA_TYPE: &str = "application/vnd.dctx0.incremental-inputs+json";

/// Engine-captured values for explicitly declared deterministic scopes. This
/// receipt does not assert isolation from undeclared inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalInputReceipt {
    pub schema_version: u32,
    pub deterministic_units: BTreeSet<String>,
    pub files: BTreeMap<String, String>,
    pub environment: BTreeMap<String, Option<String>>,
    /// Missing observations are explicit; an incomplete receipt cannot grant reuse.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incomplete: Vec<InputGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputGap {
    pub code: String,
    pub input: String,
}

/// Restrict independently projected units to explicitly declared scopes.
pub fn receipt_units(
    program: &crate::ProgramPlan,
    receipt: &ExternalInputReceipt,
) -> Option<Vec<ReuseUnit>> {
    fn hash(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    fn name(value: &str) -> bool {
        !value.is_empty() && value.len() <= 4096 && !value.contains('\0')
    }
    if !matches!(
        receipt.schema_version,
        INCREMENTAL_SCHEMA_VERSION | INPUT_RECEIPT_SCHEMA_VERSION
    ) || (receipt.schema_version == INCREMENTAL_SCHEMA_VERSION && !receipt.incomplete.is_empty())
        || receipt.incomplete.len() > 4096
        || receipt
            .incomplete
            .iter()
            .any(|gap| !name(&gap.code) || !name(&gap.input))
        || receipt.deterministic_units.len() > 4096
        || receipt.files.len() > 4096
        || receipt.environment.len() > 4096
        || receipt
            .files
            .iter()
            .any(|(key, value)| !name(key) || !hash(value))
        || receipt
            .environment
            .iter()
            .any(|(key, value)| !name(key) || value.as_ref().is_some_and(|v| !hash(v)))
    {
        return None;
    }
    let mut units = program_units(program);
    let allowed: BTreeSet<_> = units.iter().map(|u| u.id.clone()).collect();
    if !receipt.deterministic_units.is_subset(&allowed) {
        return None;
    }
    // The conservative non-contract unit can observe stateful external inputs.
    // Require at least one explicitly captured snapshot or live revision, not
    // merely the declaration, runtime, verifier, or environment inventory.
    if receipt.incomplete.is_empty()
        && receipt.deterministic_units.contains("other-cases")
        && !receipt.files.keys().any(|key| {
            key.strip_prefix("file:")
                .is_some_and(|name| !name.is_empty())
                || key
                    .strip_prefix("revision-output:")
                    .is_some_and(|name| !name.is_empty())
        })
    {
        return None;
    }
    for unit in &mut units {
        unit.reusable =
            receipt.incomplete.is_empty() && receipt.deterministic_units.contains(&unit.id);
    }
    Some(units)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReuseBinding {
    pub schema_version: u32,
    pub requirements_hash: String,
    pub manifest_hash: String,
    pub program_hash: String,
    /// Complete, stable receipt of external fixtures and environment inputs.
    pub external_inputs_receipt_hash: String,
    pub provenance: VerificationProvenance,
    pub budgets: VerificationBudgets,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReuseUnit {
    pub id: String,
    pub reusable: bool,
    pub cases: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncrementalPlan {
    pub reused: BTreeSet<String>,
    pub rerun: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncrementalProof {
    pub parent_run: String,
    pub binding: ReuseBinding,
    pub units: Vec<ReuseUnit>,
    pub plan: IncrementalPlan,
}

/// Build a complete partition from the central manifest. Contract values and
/// rejections are grouped by declaration; all other cases are rerun as one
/// conservative unit. This does not itself certify external-input isolation.
pub fn program_units(program: &crate::ProgramPlan) -> Vec<ReuseUnit> {
    let manifest = crate::case_manifest(program);
    let mut remaining = manifest.required_cases.clone();
    let mut units = Vec::new();
    for contract in &program.contracts {
        let cases = [
            ("value", contract.corpus.values.len()),
            ("rejection", contract.rejection_corpus.values.len()),
        ]
        .into_iter()
        .flat_map(|(kind, count)| {
            (0..count).map(move |i| format!("{}:{kind}:{i}", contract.subject))
        })
        .filter(|id| remaining.remove(id))
        .collect::<BTreeSet<_>>();
        if !cases.is_empty() {
            units.push(ReuseUnit {
                id: format!("contract:{}", contract.subject),
                reusable: true,
                cases,
            });
        }
    }
    if !remaining.is_empty() {
        units.push(ReuseUnit {
            id: "other-cases".into(),
            reusable: false,
            cases: remaining,
        });
    }
    units
}

fn digest(value: &impl Serialize) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

/// Derive complete program/requirement/manifest identities. The caller must
/// provide an engine-captured receipt for the explicitly declared cache inputs.
pub fn binding(
    program: &crate::ProgramPlan,
    mut provenance: VerificationProvenance,
    external_inputs_receipt_hash: String,
) -> Result<ReuseBinding, serde_json::Error> {
    let manifest = crate::case_manifest(program);
    let mut normalized = program.clone();
    normalized.snapshot.clear();
    normalized.check_occurrence = 0;
    normalized.scope.tasks.clear();
    // Task publication occurrence is not a native execution input. Preserve the
    // diagnostic on the actual run; it still blocks adoption. Source/runtime
    // drift and every other incomplete-provenance issue remain cache blockers.
    provenance
        .incomplete
        .retain(|issue| issue.code != "ADOPTION_HISTORICAL_SCOPE");
    // Execution receipts retain these values for auditing. They are not code
    // changes and do not participate in the implementation reuse key.
    provenance.implementation = code_inputs(&provenance.implementation);
    provenance.dependencies = code_inputs(&provenance.dependencies);
    provenance.build_configuration.clear();
    provenance.configuration_hash = "code-only".into();
    provenance.runtime_version = "code-only".into();
    provenance.adapter_hash = "code-only".into();
    provenance.harness_hash = "code-only".into();
    Ok(ReuseBinding {
        schema_version: INCREMENTAL_SCHEMA_VERSION,
        requirements_hash: digest(&manifest.case_requirements)?,
        manifest_hash: digest(&manifest)?,
        program_hash: digest(&normalized)?,
        external_inputs_receipt_hash,
        provenance,
        budgets: program.budgets.clone(),
    })
}

fn valid_binding(binding: &ReuseBinding) -> bool {
    let p = &binding.provenance;
    binding.schema_version == INCREMENTAL_SCHEMA_VERSION
        && [
            &binding.requirements_hash,
            &binding.manifest_hash,
            &binding.program_hash,
            &binding.external_inputs_receipt_hash,
            &p.plan_hash,
            &p.adapter_hash,
            &p.configuration_hash,
            &p.harness_hash,
            &p.interpreter_version,
            &p.generator_version,
            &p.target,
            &p.runtime_version,
        ]
        .iter()
        .all(|v| !v.is_empty())
        && !p.implementation.is_empty()
        && p.incomplete.is_empty()
}

fn compatible(old: &ReuseBinding, current: &ReuseBinding) -> bool {
    if !valid_binding(old) || !valid_binding(current) {
        return false;
    }
    let mut left = old.clone();
    let mut right = current.clone();
    // These pins identify publication occurrences, not execution inputs. The
    // complete requirements, manifest and program identities remain mandatory.
    // The normalized program hash binds the plan independently of its artifact ID.
    left.provenance.snapshot.clear();
    right.provenance.snapshot.clear();
    left.provenance.check_occurrence = 0;
    right.provenance.check_occurrence = 0;
    left.provenance.task_pin = None;
    right.provenance.task_pin = None;
    // Program hash validates normalized plan identity independently of its
    // occurrence-specific artifact digest.
    left.provenance.plan_hash.clear();
    right.provenance.plan_hash.clear();
    left == right
}

/// Only separately assessed manual work can remain open without casting doubt
/// on native group completion. Unknown issue codes fail closed: a future worker
/// or cleanup failure must not silently become reusable historical evidence.
pub fn reuse_blocking_issues(
    parent: &VerificationRun,
) -> impl Iterator<Item = &dctx0_core::VerificationIssue> {
    fn manual(issue: &dctx0_core::VerificationIssue) -> bool {
        matches!(
            issue.code.as_str(),
            "MANUAL_REQUIREMENT" | "SUITE_INSPECTION_REQUIRED"
        )
    }
    let unassigned_verdict = match &parent.result {
        VerificationResult::Fail { issue } | VerificationResult::Unknown { issue }
            if !manual(issue)
                && issue.code != "ADOPTION_HISTORICAL_SCOPE"
                && parent
                    .cases
                    .iter()
                    .all(|case| case.result == VerificationResult::Pass) =>
        {
            Some(issue)
        }
        _ => None,
    };
    parent
        .open_obligations
        .iter()
        .filter(|issue| !manual(issue))
        .chain(
            parent
                .provenance
                .incomplete
                .iter()
                .filter(|issue| issue.code != "ADOPTION_HISTORICAL_SCOPE"),
        )
        .chain(unassigned_verdict)
}

/// Return a plan only for a valid complete partition of the parent's manifest.
/// A caller must independently derive units, including all connected cases.
/// Invalid input returns None; incompatible evidence causes every case to rerun.
pub fn plan_reuse(
    old: &ReuseBinding,
    current: &ReuseBinding,
    parent: &VerificationRun,
    units: &[ReuseUnit],
) -> Option<IncrementalPlan> {
    plan_reuse_projected(old, current, parent, units, &old.provenance)
}

/// Plan using a separately verified projection of the parent's provenance.
/// The caller must derive `old` and `current` from the actual immutable native
/// receipts and must supply the exact original parent provenance here.
pub fn plan_reuse_projected(
    old: &ReuseBinding,
    current: &ReuseBinding,
    parent: &VerificationRun,
    units: &[ReuseUnit],
    parent_provenance: &VerificationProvenance,
) -> Option<IncrementalPlan> {
    let mut all = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for unit in units {
        if unit.id.is_empty() || unit.cases.is_empty() || !ids.insert(&unit.id) {
            return None;
        }
        for case in &unit.cases {
            if case.is_empty() || !all.insert(case.clone()) {
                return None;
            }
        }
    }
    if all.is_empty() || all != parent.required_cases {
        return None;
    }
    let mut records = BTreeMap::new();
    for case in &parent.cases {
        if !all.contains(&case.id) || records.insert(&case.id, case).is_some() {
            return None;
        }
    }
    let eligible = parent.origin == VerificationRunOrigin::Native
        && &parent.provenance == parent_provenance
        && parent.budgets == old.budgets
        && reuse_blocking_issues(parent).next().is_none()
        && compatible(old, current);
    let mut result = IncrementalPlan {
        reused: BTreeSet::new(),
        rerun: BTreeSet::new(),
    };
    for unit in units {
        let pass = eligible
            && unit.reusable
            && unit.cases.iter().all(|id| {
                records
                    .get(id)
                    .is_some_and(|r| r.result == VerificationResult::Pass)
            });
        if pass {
            result.reused.extend(unit.cases.iter().cloned());
        } else {
            result.rerun.extend(unit.cases.iter().cloned());
        }
    }
    Some(result)
}

/// Check proof against trusted current inputs, the parent and the completed run.
/// This checks copied records only; publication must separately validate all
/// fresh evidence, provenance, task pins and transactional adoption guards.
#[allow(clippy::too_many_arguments)] // Inputs are independently trusted evidence boundaries.
pub fn validate_proof(
    proof: &IncrementalProof,
    parent_id: &str,
    old: &ReuseBinding,
    current: &ReuseBinding,
    parent: &VerificationRun,
    units: &[ReuseUnit],
    completed: &VerificationRun,
) -> bool {
    validate_proof_projected(
        proof,
        parent_id,
        old,
        current,
        parent,
        units,
        completed,
        &old.provenance,
        &current.provenance,
    )
}

/// Validate using independently derived execution-identity projections.
/// Original provenance is checked separately and never rewritten in history.
#[allow(clippy::too_many_arguments)]
pub fn validate_proof_projected(
    proof: &IncrementalProof,
    parent_id: &str,
    old: &ReuseBinding,
    current: &ReuseBinding,
    parent: &VerificationRun,
    units: &[ReuseUnit],
    completed: &VerificationRun,
    parent_provenance: &VerificationProvenance,
    current_provenance: &VerificationProvenance,
) -> bool {
    if parent_id.is_empty()
        || proof.parent_run != parent_id
        || &proof.binding != current
        || proof.units != units
        || plan_reuse_projected(old, current, parent, units, parent_provenance).as_ref()
            != Some(&proof.plan)
        || completed.origin != VerificationRunOrigin::Native
        || &completed.provenance != current_provenance
        || completed.budgets != current.budgets
        || completed.required_cases != parent.required_cases
    {
        return false;
    }
    let mut final_records = BTreeMap::new();
    for record in &completed.cases {
        if !completed.required_cases.contains(&record.id)
            || final_records.insert(&record.id, record).is_some()
        {
            return false;
        }
    }
    if final_records.len() != completed.required_cases.len() {
        return false;
    }
    // Runner-sized corpora contain tens of thousands of records. Index once;
    // a linear parent search per reused case makes publication quadratic.
    let parent_records = parent
        .cases
        .iter()
        .map(|r| (&r.id, r))
        .collect::<BTreeMap<_, _>>();
    proof
        .plan
        .reused
        .iter()
        .all(|id| parent_records.get(id).copied() == final_records.get(id).copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dctx0_core::{VerificationCaseKind, VerificationCaseRecord};

    fn fixture() -> (ReuseBinding, VerificationRun, Vec<ReuseUnit>) {
        let provenance = VerificationProvenance {
            snapshot: "snapshot".into(),
            check_occurrence: 1,
            plan_hash: "plan".into(),
            implementation: [("source".into(), "hash".into())].into(),
            dependencies: BTreeMap::new(),
            adapter_hash: "adapter".into(),
            configuration_hash: "config".into(),
            harness_hash: "harness".into(),
            interpreter_version: "interpreter".into(),
            generator_version: "generator".into(),
            protocol_version: 1,
            target: "target".into(),
            runtime_version: "runtime".into(),
            build_configuration: BTreeMap::new(),
            task_pin: None,
            incomplete: vec![],
        };
        let binding = ReuseBinding {
            schema_version: 1,
            requirements_hash: "requirements".into(),
            manifest_hash: "manifest".into(),
            program_hash: "program".into(),
            external_inputs_receipt_hash: "receipt".into(),
            provenance: provenance.clone(),
            budgets: VerificationBudgets::default(),
        };
        let case = |id: &str| VerificationCaseRecord {
            id: id.into(),
            subject: "s".into(),
            kind: VerificationCaseKind::Contract,
            bindings: BTreeMap::new(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 1,
        };
        let run = VerificationRun {
            schema_version: 1,
            origin: VerificationRunOrigin::Native,
            started_at_unix_ns: "1".into(),
            elapsed_us: 2,
            provenance,
            budgets: binding.budgets.clone(),
            selected_subjects: ["s".into()].into(),
            required_subjects: ["s".into()].into(),
            not_selected: vec![],
            required_cases: ["a".into(), "b".into(), "c".into()].into(),
            cases: vec![case("a"), case("b"), case("c")],
            open_obligations: vec![],
            bounded_obligations: vec![],
            artifacts: vec![],
            result: VerificationResult::Pass,
        };
        let units = vec![
            ReuseUnit {
                id: "group".into(),
                reusable: true,
                cases: ["a".into(), "b".into()].into(),
            },
            ReuseUnit {
                id: "other".into(),
                reusable: false,
                cases: ["c".into()].into(),
            },
        ];
        (binding, run, units)
    }

    #[test]
    fn reuse_requires_matching_inputs_and_complete_passing_units() {
        let (old, mut run, units) = fixture();
        let plan = plan_reuse(&old, &old, &run, &units).unwrap();
        assert_eq!(plan.reused, ["a".into(), "b".into()].into());
        assert_eq!(plan.rerun, ["c".into()].into());
        run.cases.pop();
        run.cases.pop();
        assert!(
            plan_reuse(&old, &old, &run, &units)
                .unwrap()
                .reused
                .is_empty()
        );
        let mut changed = old.clone();
        changed.external_inputs_receipt_hash.clear();
        assert!(
            plan_reuse(&old, &changed, &run, &units)
                .unwrap()
                .reused
                .is_empty()
        );
        let mut changed = old.clone();
        changed.provenance.runtime_version = "other".into();
        assert!(
            plan_reuse(&old, &changed, &run, &units)
                .unwrap()
                .reused
                .is_empty()
        );
        run.origin = VerificationRunOrigin::Imported;
        assert!(
            plan_reuse(&old, &old, &run, &units)
                .unwrap()
                .reused
                .is_empty()
        );
    }

    #[test]
    fn opted_in_connected_unit_reuses_only_when_every_case_passed() {
        let (binding, mut run, mut units) = fixture();
        units[1].reusable = true;
        assert_eq!(
            plan_reuse(&binding, &binding, &run, &units).unwrap().reused,
            run.required_cases
        );
        run.cases[2].result = VerificationResult::Unknown { issue: issue() };
        let plan = plan_reuse(&binding, &binding, &run, &units).unwrap();
        assert_eq!(plan.rerun, ["c".into()].into());
        assert_eq!(plan.reused, ["a".into(), "b".into()].into());
    }

    #[test]
    fn proof_recomputes_plan_and_checks_copied_records() {
        let (old, run, units) = fixture();
        let plan = plan_reuse(&old, &old, &run, &units).unwrap();
        let proof = IncrementalProof {
            parent_run: "parent".into(),
            binding: old.clone(),
            units: units.clone(),
            plan,
        };
        assert!(validate_proof(
            &proof, "parent", &old, &old, &run, &units, &run
        ));
        let mut changed = run.clone();
        changed.cases[0].elapsed_us += 1;
        assert!(!validate_proof(
            &proof, "parent", &old, &old, &run, &units, &changed
        ));
        let mut duplicate = units.clone();
        duplicate.push(units[0].clone());
        assert!(plan_reuse(&old, &old, &run, &duplicate).is_none());
    }

    #[test]
    fn each_execution_identity_change_invalidates_reuse() {
        let (old, run, units) = fixture();
        macro_rules! changed {
            ($name:literal, $modify:expr) => {{
                let mut current = old.clone();
                ($modify)(&mut current);
                let plan = plan_reuse(&old, &current, &run, &units).unwrap();
                assert!(plan.reused.is_empty(), "{}", $name);
                assert_eq!(plan.rerun, run.required_cases, "{}", $name);
            }};
        }
        changed!("source", |b: &mut ReuseBinding| {
            b.provenance
                .implementation
                .insert("source".into(), "changed".into());
        });
        changed!("helper", |b: &mut ReuseBinding| {
            b.provenance
                .implementation
                .insert("helper".into(), "changed".into());
        });
        changed!("dependency", |b: &mut ReuseBinding| {
            b.provenance
                .dependencies
                .insert("dependency".into(), "changed".into());
        });
        changed!("adapter", |b: &mut ReuseBinding| b
            .provenance
            .adapter_hash
            .push('x'));
        changed!("harness", |b: &mut ReuseBinding| b
            .provenance
            .harness_hash
            .push('x'));
        changed!("toolchain", |b: &mut ReuseBinding| {
            b.provenance
                .build_configuration
                .insert("compiler".into(), "changed".into());
        });
        changed!("configuration", |b: &mut ReuseBinding| b
            .provenance
            .configuration_hash
            .push('x'));
        changed!("runtime", |b: &mut ReuseBinding| b
            .provenance
            .runtime_version
            .push('x'));
        changed!("target", |b: &mut ReuseBinding| b
            .provenance
            .target
            .push('x'));
        changed!("interpreter", |b: &mut ReuseBinding| b
            .provenance
            .interpreter_version
            .push('x'));
        changed!("generator", |b: &mut ReuseBinding| b
            .provenance
            .generator_version
            .push('x'));
        changed!("protocol", |b: &mut ReuseBinding| b
            .provenance
            .protocol_version +=
            1);
        changed!("budget", |b: &mut ReuseBinding| b.budgets.seed += 1);
        changed!("requirements", |b: &mut ReuseBinding| b
            .requirements_hash
            .push('x'));
        changed!("manifest", |b: &mut ReuseBinding| b.manifest_hash.push('x'));
        changed!("program and inputs", |b: &mut ReuseBinding| b
            .program_hash
            .push('x'));
        changed!("external receipt", |b: &mut ReuseBinding| b
            .external_inputs_receipt_hash
            .push('x'));
        changed!("version", |b: &mut ReuseBinding| b.schema_version += 1);
        changed!("incomplete provenance", |b: &mut ReuseBinding| b
            .provenance
            .incomplete
            .push(issue()));
        let mut repinned = old.clone();
        repinned.provenance.snapshot = "another".into();
        repinned.provenance.check_occurrence += 1;
        assert_eq!(
            plan_reuse(&old, &repinned, &run, &units)
                .unwrap()
                .reused
                .len(),
            2
        );
        // The occurrence-specific artifact hash may differ for the same program.
        repinned.provenance.plan_hash.push('x');
        assert_eq!(
            plan_reuse(&old, &repinned, &run, &units)
                .unwrap()
                .reused
                .len(),
            2
        );
    }

    fn issue() -> dctx0_core::VerificationIssue {
        dctx0_core::VerificationIssue {
            code: "issue".into(),
            message: "issue".into(),
            span: None,
        }
    }

    #[test]
    fn malformed_and_incomplete_evidence_never_reuses_a_unit() {
        let (old, run, units) = fixture();
        for result in [
            VerificationResult::Fail { issue: issue() },
            VerificationResult::Unknown { issue: issue() },
        ] {
            let mut altered = run.clone();
            altered.cases[0].result = result;
            let plan = plan_reuse(&old, &old, &altered, &units).unwrap();
            assert!(plan.reused.is_empty());
            assert_eq!(plan.rerun, run.required_cases);
        }
        let mut incomplete = run.clone();
        incomplete.open_obligations.push(issue());
        assert!(
            plan_reuse(&old, &old, &incomplete, &units)
                .unwrap()
                .reused
                .is_empty()
        );
        let mut duplicate = run.clone();
        duplicate.cases.push(duplicate.cases[0].clone());
        assert!(plan_reuse(&old, &old, &duplicate, &units).is_none());
        let mut missing = units.clone();
        missing[0].cases.remove("a");
        assert!(plan_reuse(&old, &old, &run, &missing).is_none());
        let mut overlap = units.clone();
        overlap[1].cases.insert("a".into());
        assert!(plan_reuse(&old, &old, &run, &overlap).is_none());
        let plan = plan_reuse(&old, &old, &run, &units).unwrap();
        let mut proof = IncrementalProof {
            parent_run: "parent".into(),
            binding: old.clone(),
            units: units.clone(),
            plan,
        };
        let mut incomplete_final = run.clone();
        incomplete_final.cases.pop();
        assert!(!validate_proof(
            &proof,
            "parent",
            &old,
            &old,
            &run,
            &units,
            &incomplete_final
        ));
        proof.plan.reused.insert("c".into());
        assert!(!validate_proof(
            &proof, "parent", &old, &old, &run, &units, &run
        ));
    }

    #[test]
    fn projected_program_units_cover_manifest_and_binding_tracks_plan() {
        let compiled = dctx0_language::compile(&[dctx0_core::Source {
            file: "incremental.dctx0".into(),
            text:
                "workspace W { type Year = Int\n port API { operation echo(value:Year) -> Year } }"
                    .into(),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let program = crate::project_program(
            &compiled.ir.unwrap(),
            "snapshot",
            7,
            crate::VerificationScope {
                subjects: ["W".into()].into(),
                tasks: BTreeSet::new(),
            },
            &Default::default(),
            VerificationBudgets {
                generated_cases: 3,
                ..Default::default()
            },
        );
        let manifest = crate::case_manifest(&program);
        let units = program_units(&program);
        assert!(units.iter().any(|unit| unit.reusable));
        assert!(units.iter().any(|unit| !unit.reusable));
        let mut receipt = ExternalInputReceipt {
            schema_version: INCREMENTAL_SCHEMA_VERSION,
            deterministic_units: units
                .iter()
                .filter(|u| u.reusable)
                .map(|u| u.id.clone())
                .collect(),
            files: [("file:fixtures/data".into(), "a".repeat(64))].into(),
            environment: [("SETTING".into(), None)].into(),
            incomplete: vec![],
        };
        assert!(receipt_units(&program, &receipt).is_some());
        receipt.schema_version = INPUT_RECEIPT_SCHEMA_VERSION;
        receipt.incomplete.push(InputGap {
            code: "REVISION_OUTPUT_MISSING".into(),
            input: "database".into(),
        });
        assert!(
            receipt_units(&program, &receipt)
                .unwrap()
                .iter()
                .all(|unit| !unit.reusable)
        );
        receipt.schema_version = INCREMENTAL_SCHEMA_VERSION;
        assert!(receipt_units(&program, &receipt).is_none());
        receipt.incomplete.clear();
        receipt.deterministic_units.insert("other-cases".into());
        assert!(
            receipt_units(&program, &receipt)
                .unwrap()
                .iter()
                .any(|u| u.id == "other-cases" && u.reusable)
        );
        receipt.files.clear();
        assert!(receipt_units(&program, &receipt).is_none());
        receipt
            .files
            .insert("revision-output:database".into(), "a".repeat(64));
        assert!(receipt_units(&program, &receipt).is_some());
        receipt.deterministic_units.remove("other-cases");
        receipt
            .files
            .insert("file:fixtures/data".into(), "bad".into());
        assert!(receipt_units(&program, &receipt).is_none());
        let all: Vec<_> = units
            .iter()
            .flat_map(|unit| unit.cases.iter().cloned())
            .collect();
        assert_eq!(all.len(), manifest.required_cases.len());
        assert_eq!(
            all.into_iter().collect::<BTreeSet<_>>(),
            manifest.required_cases
        );
        let (example, _, _) = fixture();
        let first = binding(&program, example.provenance.clone(), "receipt".into()).unwrap();
        let mut task_occurrence = program.clone();
        task_occurrence.scope.tasks.insert("937".into());
        assert_eq!(
            first.program_hash,
            binding(
                &task_occurrence,
                example.provenance.clone(),
                "receipt".into()
            )
            .unwrap()
            .program_hash
        );
        let mut revised = program.clone();
        revised.contracts[0].corpus.values.pop();
        let second = binding(&revised, example.provenance.clone(), "receipt".into()).unwrap();
        assert_ne!(first.program_hash, second.program_hash);
        assert_ne!(first.manifest_hash, second.manifest_hash);
        let mut changed_input = program.clone();
        changed_input.budgets.seed += 1;
        assert_ne!(
            first.program_hash,
            binding(&changed_input, example.provenance, "receipt".into())
                .unwrap()
                .program_hash
        );
    }
    #[test]
    fn large_complete_units_preserve_exact_record_binding_independent_of_order() {
        let (old, mut parent, _) = fixture();
        let template = parent.cases[0].clone();
        parent.cases = (0..25_000)
            .map(|i| {
                let mut record = template.clone();
                record.id = format!("case:{i}");
                record
            })
            .collect();
        parent.required_cases = parent.cases.iter().map(|c| c.id.clone()).collect();
        let units = vec![ReuseUnit {
            id: "large-contract".into(),
            reusable: true,
            cases: parent.required_cases.clone(),
        }];
        let proof = IncrementalProof {
            parent_run: "parent".into(),
            binding: old.clone(),
            units: units.clone(),
            plan: plan_reuse(&old, &old, &parent, &units).unwrap(),
        };
        let mut completed = parent.clone();
        completed.cases.reverse();
        assert!(validate_proof(
            &proof, "parent", &old, &old, &parent, &units, &completed
        ));
        completed.cases[0].elapsed_us += 1;
        assert!(!validate_proof(
            &proof, "parent", &old, &old, &parent, &units, &completed
        ));
    }
    #[test]
    fn decisions_name_changed_inputs_without_values_and_partition_fresh_or_reused_units() {
        let (binding, run, units) = fixture();
        let plan = plan_reuse(&binding, &binding, &run, &units).unwrap();
        let decisions = unit_decisions(&units, &plan, Some(&binding), &binding, false);
        assert_eq!(decisions[0].decision, UnitAction::Reuse);
        assert_eq!(decisions[1].decision, UnitAction::ExecuteUnbound);
        assert!(
            unit_decisions(&units, &plan, Some(&binding), &binding, true)
                .iter()
                .all(|d| d.decision == UnitAction::ExecuteAlways)
        );
        let all_fresh = IncrementalPlan {
            reused: Default::default(),
            rerun: run.required_cases.clone(),
        };
        assert_eq!(
            unit_decisions(&units, &all_fresh, None, &binding, false)[0].decision,
            UnitAction::ExecuteNew
        );
        let mut current = binding.clone();
        current
            .provenance
            .implementation
            .insert("source".into(), "private-new-digest".into());
        current.budgets.seed += 1;
        let decisions = unit_decisions(&units, &all_fresh, Some(&binding), &current, false);
        assert_eq!(decisions[0].decision, UnitAction::ExecuteChanged);
        assert!(
            decisions[0]
                .reasons
                .iter()
                .any(|r| r.code == "SOURCE_CHANGED" && r.input == "source")
        );
        assert!(
            !serde_json::to_string(&decisions)
                .unwrap()
                .contains("private-new-digest")
        );
        let old = ExternalInputReceipt {
            schema_version: 1,
            deterministic_units: Default::default(),
            files: [("file:fixture".into(), "a".repeat(64))].into(),
            environment: [("SETTING".into(), Some("b".repeat(64)))].into(),
            incomplete: vec![],
        };
        let mut changed = old.clone();
        changed.files.clear();
        changed.environment.insert("SETTING".into(), None);
        assert_eq!(
            receipt_changes(&old, &changed)
                .iter()
                .map(|r| r.input.as_str())
                .collect::<Vec<_>>(),
            vec!["file:fixture", "SETTING"]
        );
    }
    #[test]
    fn pending_manual_inspections_do_not_discard_complete_native_units() {
        let (binding, mut run, units) = fixture();
        for code in [
            "MANUAL_REQUIREMENT",
            "SUITE_INSPECTION_REQUIRED",
            "ADOPTION_HISTORICAL_SCOPE",
        ] {
            let issue = dctx0_core::VerificationIssue {
                code: code.into(),
                message: "Separate authored assessment remains pending".into(),
                span: None,
            };
            run.result = VerificationResult::Unknown {
                issue: issue.clone(),
            };
            if code == "ADOPTION_HISTORICAL_SCOPE" {
                run.open_obligations.clear();
            } else {
                run.open_obligations = vec![issue];
            }
            assert_eq!(
                plan_reuse(&binding, &binding, &run, &units).unwrap().reused,
                units[0].cases,
            );
        }
    }

    #[test]
    fn global_native_verdict_and_input_drift_block_otherwise_passing_cases() {
        let (binding, mut run, units) = fixture();
        for result in [
            VerificationResult::Fail { issue: issue() },
            VerificationResult::Unknown { issue: issue() },
        ] {
            run.result = result;
            assert!(
                plan_reuse(&binding, &binding, &run, &units)
                    .unwrap()
                    .reused
                    .is_empty()
            );
        }
        run.result = VerificationResult::Pass;
        run.provenance.incomplete.push(issue());
        assert!(
            plan_reuse(&binding, &binding, &run, &units)
                .unwrap()
                .reused
                .is_empty()
        );
        assert_eq!(reuse_blocking_issues(&run).count(), 1);
    }

    #[test]
    fn aborted_or_unclean_groups_do_not_supply_even_completed_case_units() {
        let (binding, mut run, units) = fixture();
        for code in [
            "NATIVE_RUN_ABORTED",
            "NATIVE_CLEANUP",
            "NATIVE_INITIALIZATION",
            "ADAPTER_DESCRIPTION",
            "NATIVE_WORKER_EXIT",
            "NATIVE_EXECUTION_INCOMPLETE",
            "UNRECOGNIZED_GROUP_FAILURE",
        ] {
            run.open_obligations = vec![dctx0_core::VerificationIssue {
                code: code.into(),
                message: "Unproven group boundary".into(),
                span: None,
            }];
            assert!(
                plan_reuse(&binding, &binding, &run, &units)
                    .unwrap()
                    .reused
                    .is_empty()
            );
        }
    }
}
