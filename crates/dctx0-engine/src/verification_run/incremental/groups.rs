use super::*;
use dctx0_changes::Snapshot;
use dctx0_verification::{
    incremental as reuse,
    unit_planner::catalog::{LogicalVerificationUnit, logical_unit_catalog},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalTestGroup {
    pub id: String,
    pub historical_cases: usize,
    pub current_cases: usize,
    pub change: TestGroupChange,
    pub historical_pass: bool,
    pub runner_status: TestGroupRunnerStatus,
    pub reasons: Vec<reuse::ReuseReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestGroupChange {
    New,
    Changed,
    Removed,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestGroupRunnerStatus {
    StaticCompatible,
    BindingCheckRequired,
    ExecutionRequired,
    EvidenceMissing,
    NotSelected,
}

fn reason(code: &str, input: &str) -> reuse::ReuseReason {
    reuse::ReuseReason {
        code: code.into(),
        input: input.into(),
    }
}

pub(super) struct StaticBinding {
    // None means local inputs cannot establish this runner's binding.
    compatible_cases: Option<BTreeSet<String>>,
    reasons: Vec<reuse::ReuseReason>,
}

impl StaticBinding {
    pub(super) fn inspect(
        plans: [&ProgramPlan; 2],
        parent: &VerificationRun,
        receipt: Option<&reuse::ExternalInputReceipt>,
        application: &Path,
        inventory_matches: Option<bool>,
        target: &str,
        contrary: &BTreeSet<String>,
    ) -> Result<Self> {
        let [saved, current] = plans;
        let mut reasons = Vec::new();
        if !contrary.is_empty() {
            reasons.push(reason("LATER_NONPASS_OBSERVATION", "parent-cohort"));
        }
        if inventory_matches == Some(false) {
            reasons.push(reason(
                "LOCAL_CODE_INPUTS_CHANGED",
                "captured-input-inventory",
            ));
        }
        if target != parent.provenance.target {
            reasons.push(reason("TARGET_CHANGED", target));
        }
        let old = reuse::binding(saved, parent.provenance.clone(), String::new())?;
        let new = reuse::binding(current, parent.provenance.clone(), String::new())?;
        if old.program_hash != new.program_hash
            || old.manifest_hash != new.manifest_hash
            || old.requirements_hash != new.requirements_hash
        {
            reasons.push(reason("SHARED_ADAPTER_PROGRAM_CHANGED", "program-plan"));
        }
        if !reasons.is_empty() {
            return Ok(Self {
                compatible_cases: Some(BTreeSet::new()),
                reasons,
            });
        }

        let mut pending = |code| {
            reasons.push(reason(code, "parent-cohort"));
            Self {
                compatible_cases: None,
                reasons: std::mem::take(&mut reasons),
            }
        };
        if parent.artifacts.iter().any(|artifact| {
            matches!(
                artifact.media_type.as_str(),
                dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE
                    | dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE
                    | dctx0_verification::hybrid_evidence::INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE
            )
        }) {
            return Ok(pending("NATIVE_RECIPE_BINDING_REQUIRED"));
        }
        if parent.provenance.protocol_version != PROTOCOL_VERSION
            || parent.provenance.interpreter_version != dctx0_interpreter::INTERPRETER_VERSION
        {
            return Ok(Self {
                compatible_cases: Some(BTreeSet::new()),
                reasons: vec![reason("RUNNER_SEMANTICS_CHANGED", "parent-cohort")],
            });
        }
        if inventory_matches.is_none() {
            return Ok(pending("CAPTURED_INPUT_INVENTORY_MISSING"));
        }
        let Some(receipt) = receipt else {
            return Ok(pending("PARENT_INPUT_RECEIPT_MISSING"));
        };
        let inputs = ".dctx0/testing/reuse-inputs.json";
        let local = match super::super::reuse_inputs::capture_local(
            application,
            if application.join(inputs).exists() {
                inputs
            } else {
                ""
            },
        ) {
            Ok(local) => local,
            Err(_) => return Ok(pending("LOCAL_INPUT_BINDING_UNAVAILABLE")),
        };
        let receipt = reuse::code_receipt(receipt);
        let changes = reuse::receipt_changes(&receipt, &local);
        if !changes.is_empty() {
            return Ok(Self {
                compatible_cases: Some(BTreeSet::new()),
                reasons: changes,
            });
        }
        let Some(units) = reuse::receipt_units(current, &local) else {
            return Ok(pending("DETERMINISTIC_INPUT_BINDING_REQUIRED"));
        };
        let mut old = old;
        let mut new = new;
        old.external_inputs_receipt_hash = digest(&serde_json::to_vec(&receipt)?);
        new.external_inputs_receipt_hash = digest(&serde_json::to_vec(&local)?);
        Ok(Self {
            compatible_cases: reuse::plan_reuse_projected(
                &old,
                &new,
                parent,
                &units,
                &parent.provenance,
            )
            .map(|plan| plan.reused),
            reasons,
        })
    }
}

struct Requirements<'a> {
    snapshots: [&'a Snapshot; 2],
    graphs: [dctx0_graph::GraphIndex<'a>; 2],
    facts: [BTreeMap<&'a str, Vec<serde_json::Value>>; 2],
    lineage: &'a BTreeMap<String, String>,
}

impl Requirements<'_> {
    fn changes(&self, roots: [BTreeSet<String>; 2]) -> Vec<reuse::ReuseReason> {
        let [before, after] =
            std::array::from_fn(|side| self.graphs[side].verification_closure(&roots[side]));
        let mut matched = BTreeSet::new();
        let mut changed = BTreeSet::new();
        for id in &before {
            let Some(next) = self.lineage.get(id).filter(|next| after.contains(*next)) else {
                changed.insert(id);
                continue;
            };
            matched.insert(next);
            let [old, new] = [
                self.snapshots[0].fingerprints.get(id),
                self.snapshots[1].fingerprints.get(next),
            ];
            if old.zip(new).is_none_or(|(old, new)| {
                (&old.contract, &old.behavior) != (&new.contract, &new.behavior)
                    || self.facts[0].get(id.as_str()) != self.facts[1].get(next.as_str())
            }) {
                changed.insert(next);
            }
        }
        changed.extend(after.iter().filter(|id| !matched.contains(id)));
        changed
            .into_iter()
            .map(|id| reason("SEMANTIC_REQUIREMENT_CHANGED", id))
            .collect()
    }
}

fn requirement_roots(
    group: &LogicalVerificationUnit,
    program: &ProgramPlan,
    lifecycle_setup: &[Invocation],
) -> BTreeSet<String> {
    let mut roots = group.requirements.clone();
    roots.insert(group.subject.clone());
    roots.extend(lifecycle_setup.iter().map(|call| call.subject.clone()));
    if group.id.starts_with("operation:")
        && let Some(operation) = program
            .operations
            .iter()
            .find(|op| op.operation.id == group.subject)
    {
        roots.extend(
            operation
                .cases
                .iter()
                .flat_map(|case| &case.setup)
                .chain(
                    operation
                        .generated_preparations
                        .iter()
                        .flat_map(|prep| &prep.setup),
                )
                .map(|call| call.subject.clone()),
        );
    }
    roots
}

fn lifecycle_inputs_match(
    id: &str,
    records: &BTreeMap<&str, &VerificationCaseRecord>,
    fixtures: &VerificationLifecycleFixtures,
) -> Option<bool> {
    let case = records.get(id)?;
    let Some(VerificationValue::List(keys)) = case.bindings.get("keys") else {
        return None;
    };
    Some(fixtures.paths.get(id).is_some_and(|input| {
        input.keys == *keys && input.setup == case.setup && input.arguments == case.arguments
    }))
}

pub(super) fn compare(
    programs: [&ProgramPlan; 2],
    snapshots: [&Snapshot; 2],
    historical_scope: &BTreeSet<String>,
    parent: &VerificationRun,
    runner: &StaticBinding,
    lifecycle_fixtures: &VerificationLifecycleFixtures,
    lineage: &BTreeMap<String, String>,
) -> Result<Vec<IncrementalTestGroup>> {
    let [saved, current] = programs;
    let mut groups = BTreeMap::<String, [Option<LogicalVerificationUnit>; 2]>::new();
    for (side, program) in [saved, current].into_iter().enumerate() {
        for unit in logical_unit_catalog(program).map_err(Error::Invalid)? {
            if side == 1 || historical_scope.contains(&unit.subject) {
                let id = unit.id.clone();
                groups.entry(id).or_default()[side] = Some(unit);
            }
        }
    }
    let records: BTreeMap<_, _> = parent
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect();
    let blockers: Vec<_> = reuse::reuse_blocking_issues(parent).collect();
    let requirements = Requirements {
        snapshots,
        graphs: snapshots.map(|snapshot| snapshot.graph.index()),
        facts: snapshots.map(|snapshot| dctx0_changes::facts_without_constraints(&snapshot.ir)),
        lineage,
    };
    let mut result = Vec::new();
    for (id, [old, new]) in groups {
        let mut reasons = Vec::new();
        let mut case_inputs_known = true;
        let change = match (&old, &new) {
            (Some(old), Some(new)) => {
                if old.cases != new.cases || old.description_hash != new.description_hash {
                    reasons.push(reason("LOGICAL_CASES_CHANGED", &id));
                }
                let mut lifecycle_setup = [vec![], vec![]];
                if let Some(path) = id.strip_prefix("lifecycle:") {
                    match lifecycle_inputs_match(path, &records, lifecycle_fixtures) {
                        Some(true) => {}
                        Some(false) => reasons.push(reason("LIFECYCLE_INPUTS_CHANGED", path)),
                        None => case_inputs_known = false,
                    }
                    if let Some(case) = records.get(path) {
                        lifecycle_setup[0] = case.setup.clone();
                    }
                    if let Some(input) = lifecycle_fixtures.paths.get(path) {
                        lifecycle_setup[1] = input.setup.clone();
                    }
                }
                reasons.extend(requirements.changes([
                    requirement_roots(old, saved, &lifecycle_setup[0]),
                    requirement_roots(new, current, &lifecycle_setup[1]),
                ]));
                if reasons.is_empty() {
                    TestGroupChange::Unchanged
                } else {
                    TestGroupChange::Changed
                }
            }
            (None, Some(_)) => {
                reasons.push(reason("NEW_LOGICAL_GROUP", &id));
                TestGroupChange::New
            }
            (_, None) => {
                reasons.push(reason("LOGICAL_GROUP_REMOVED", &id));
                TestGroupChange::Removed
            }
        };
        let historical_pass = old.as_ref().is_some_and(|group| {
            case_inputs_known
                && parent.origin == VerificationRunOrigin::Native
                && blockers.is_empty()
                && group.cases.is_subset(&parent.required_cases)
                && group.cases.iter().all(|id| {
                    records
                        .get(id.as_str())
                        .is_some_and(|case| case.result == VerificationResult::Pass)
                })
        });
        let runner_status = match (&new, change) {
            (None, _) => TestGroupRunnerStatus::NotSelected,
            (_, TestGroupChange::New | TestGroupChange::Changed) => {
                TestGroupRunnerStatus::ExecutionRequired
            }
            (_, _) if !historical_pass => {
                reasons.push(reason("COMPLETE_NATIVE_PASS_UNAVAILABLE", &id));
                reasons.extend(
                    blockers
                        .iter()
                        .map(|issue| reason("PARENT_GROUP_INCOMPLETE", &issue.code)),
                );
                TestGroupRunnerStatus::EvidenceMissing
            }
            (Some(group), _) => {
                reasons.extend(runner.reasons.iter().cloned());
                match &runner.compatible_cases {
                    Some(cases) if group.cases.is_subset(cases) => {
                        TestGroupRunnerStatus::StaticCompatible
                    }
                    Some(_) => {
                        if runner.reasons.is_empty() {
                            reasons.push(reason("DETERMINISTIC_UNIT_NOT_REUSABLE", &id));
                        }
                        TestGroupRunnerStatus::ExecutionRequired
                    }
                    None => TestGroupRunnerStatus::BindingCheckRequired,
                }
            }
        };
        result.push(IncrementalTestGroup {
            id,
            historical_cases: old.map_or(0, |group| group.cases.len()),
            current_cases: new.map_or(0, |group| group.cases.len()),
            change,
            historical_pass,
            runner_status,
            reasons,
        });
    }
    Ok(result)
}
