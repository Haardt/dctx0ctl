//! Pure, machine-readable explanations. Values and secret digests are omitted.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitAction {
    Reuse,
    ExecuteNew,
    ExecuteChanged,
    ExecuteUnbound,
    ExecuteAlways,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReuseReason {
    pub code: String,
    pub input: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitDecision {
    pub unit: String,
    pub cases: usize,
    pub decision: UnitAction,
    pub reasons: Vec<ReuseReason>,
}
fn reason(code: &str, input: &str) -> ReuseReason {
    ReuseReason {
        code: code.into(),
        input: input.into(),
    }
}
fn map_changes<T: PartialEq>(
    old: &BTreeMap<String, T>,
    current: &BTreeMap<String, T>,
    code: &str,
) -> Vec<ReuseReason> {
    old.keys()
        .chain(current.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|key| old.get(*key) != current.get(*key))
        .map(|key| reason(code, key))
        .collect()
}
pub fn receipt_changes(
    old: &ExternalInputReceipt,
    current: &ExternalInputReceipt,
) -> Vec<ReuseReason> {
    let mut changes = map_changes(&old.files, &current.files, "INPUT_CHANGED");
    changes.extend(map_changes(
        &old.environment,
        &current.environment,
        "ENVIRONMENT_CHANGED",
    ));
    if old.deterministic_units != current.deterministic_units {
        changes.push(reason("DETERMINISTIC_SCOPE_CHANGED", "deterministic_units"));
    }
    changes.extend(
        current
            .incomplete
            .iter()
            .map(|gap| reason(&gap.code, &gap.input)),
    );
    changes
}
fn binding_changes(old: &ReuseBinding, current: &ReuseBinding) -> Vec<ReuseReason> {
    let mut changes = map_changes(
        &old.provenance.implementation,
        &current.provenance.implementation,
        "SOURCE_CHANGED",
    );
    changes.extend(map_changes(
        &old.provenance.dependencies,
        &current.provenance.dependencies,
        "DEPENDENCY_CHANGED",
    ));
    changes.extend(map_changes(
        &old.provenance.build_configuration,
        &current.provenance.build_configuration,
        "BUILD_INPUT_CHANGED",
    ));
    for (name, left, right) in [
        (
            "requirements",
            &old.requirements_hash,
            &current.requirements_hash,
        ),
        ("case-manifest", &old.manifest_hash, &current.manifest_hash),
        ("program", &old.program_hash, &current.program_hash),
        (
            "external-inputs",
            &old.external_inputs_receipt_hash,
            &current.external_inputs_receipt_hash,
        ),
        (
            "adapter",
            &old.provenance.adapter_hash,
            &current.provenance.adapter_hash,
        ),
        (
            "configuration",
            &old.provenance.configuration_hash,
            &current.provenance.configuration_hash,
        ),
        (
            "harness",
            &old.provenance.harness_hash,
            &current.provenance.harness_hash,
        ),
        (
            "runtime",
            &old.provenance.runtime_version,
            &current.provenance.runtime_version,
        ),
        ("target", &old.provenance.target, &current.provenance.target),
        (
            "evaluator",
            &old.provenance.interpreter_version,
            &current.provenance.interpreter_version,
        ),
        (
            "generator",
            &old.provenance.generator_version,
            &current.provenance.generator_version,
        ),
    ] {
        if left != right {
            changes.push(reason("BINDING_CHANGED", name));
        }
    }
    if old.budgets != current.budgets {
        changes.push(reason("BUDGET_CHANGED", "budgets"));
    }
    if old.provenance.protocol_version != current.provenance.protocol_version
        || old.schema_version != current.schema_version
    {
        changes.push(reason("SEMANTICS_CHANGED", "protocol-or-binding-version"));
    }
    changes
}
pub fn unit_decisions(
    units: &[ReuseUnit],
    plan: &IncrementalPlan,
    old: Option<&ReuseBinding>,
    current: &ReuseBinding,
    force: bool,
) -> Vec<UnitDecision> {
    let changes = old
        .map(|old| binding_changes(old, current))
        .unwrap_or_default();
    units
        .iter()
        .map(|unit| {
            let (decision, reasons) = if force {
                (
                    UnitAction::ExecuteAlways,
                    vec![reason("FRESH_EXECUTION_REQUESTED", "reuse")],
                )
            } else if unit.cases.is_subset(&plan.reused) {
                (UnitAction::Reuse, vec![])
            } else if !unit.reusable {
                (
                    UnitAction::ExecuteUnbound,
                    vec![reason("DEPENDENCY_BOUNDARY_UNPROVEN", &unit.id)],
                )
            } else if old.is_none() {
                (
                    UnitAction::ExecuteNew,
                    vec![reason("INPUT_RECEIPT_MISSING", "parent")],
                )
            } else if !changes.is_empty() {
                (UnitAction::ExecuteChanged, changes.clone())
            } else {
                let mut reasons = current
                    .provenance
                    .incomplete
                    .iter()
                    .map(|issue| reason(&issue.code, "provenance"))
                    .collect::<Vec<_>>();
                reasons.push(reason("COMPLETE_NATIVE_PASS_UNAVAILABLE", &unit.id));
                (UnitAction::ExecuteUnbound, reasons)
            };
            UnitDecision {
                unit: unit.id.clone(),
                cases: unit.cases.len(),
                decision,
                reasons,
            }
        })
        .collect()
}
