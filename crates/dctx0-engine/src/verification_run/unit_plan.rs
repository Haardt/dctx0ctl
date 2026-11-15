//! Static unit projection. Unsupported target boundaries remain conservative.
use super::*;
use dctx0_verification::unit_planner::{self, UnitPlannerInput, catalog};

pub(super) fn static_plan(
    project: &Project,
    application: &Path,
    program: &ProgramPlan,
    inventory: &BTreeMap<String, String>,
    historical_cases: &BTreeSet<String>,
) -> Result<UnitExecutionPlan> {
    let (_, config) = project.verification_configuration(Some(application))?;
    let logical = catalog::logical_unit_catalog(program).map_err(Error::Invalid)?;
    let boundaries = logical
        .iter()
        .map(|unit| {
            (
                unit.id.clone(),
                UnitDependencyBoundary::Unknown {
                    group: format!("{}:native", config.target),
                    reasons: ["TARGET_UNIT_BOUNDARY_UNSUPPORTED".into()].into(),
                },
            )
        })
        .collect();
    let units = catalog::merge_unit_boundaries(&logical, &boundaries).map_err(Error::Invalid)?;
    let snapshot = project.load(&program.snapshot)?;
    let graph = snapshot.graph.index();
    let mut bindings = Vec::new();
    for unit in units {
        let members: Vec<_> = logical
            .iter()
            .filter(|member| !member.cases.is_disjoint(&unit.cases))
            .collect();
        let roots = members
            .iter()
            .flat_map(|member| member.requirements.iter().cloned())
            .collect();
        let requirements = graph
            .verification_closure(&roots)
            .into_iter()
            .map(|id| {
                let fingerprints = snapshot
                    .fingerprints
                    .get(&id)
                    .ok_or_else(|| Error::Invalid(format!("UNIT_SEMANTIC_INPUT_MISSING: {id}")))?;
                Ok((
                    id,
                    unit_planner::evidence_digest(&(
                        &fingerprints.contract,
                        &fingerprints.behavior,
                        &fingerprints.facts,
                    ))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut logical_cases = BTreeMap::new();
        for member in members {
            for case in &member.cases {
                logical_cases.insert(
                    case.clone(),
                    unit_planner::evidence_digest(&(&member.description_hash, case))?,
                );
            }
        }
        let policy = config.environment.clone().unwrap_or_default();
        let environment = policy
            .entries()
            .map_err(|e| Error::Invalid(e.to_string()))?
            .into_iter()
            .map(|(key, value)| {
                let key = key
                    .into_string()
                    .map_err(|_| Error::Invalid("Non-UTF8 unit environment key".into()))?;
                Ok((key, Some(digest(value.as_encoded_bytes()))))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        bindings.push(UnitEvidenceBindingV1 {
            schema_version: 1,
            unit,
            logical_cases,
            requirements,
            dependencies: inventory.clone(),
            fixtures: BTreeMap::new(),
            build_inputs: [(
                "configuration".into(),
                unit_planner::evidence_digest(&config)?,
            )]
            .into(),
            environment,
            semantics: [
                (
                    "budgets".into(),
                    unit_planner::evidence_digest(&program.budgets)?,
                ),
                (
                    "generator".into(),
                    unit_planner::evidence_digest(&program.generator_version)?,
                ),
                (
                    "compiler".into(),
                    unit_planner::evidence_digest(&snapshot.ir.compiler_version)?,
                ),
                (
                    "evaluator".into(),
                    unit_planner::evidence_digest(&dctx0_verification::VERIFICATION_VERSION)?,
                ),
            ]
            .into(),
        });
    }
    let required_cases = dctx0_verification::case_manifest(program).required_cases;
    unit_planner::plan_units(UnitPlannerInput {
        required_cases: &required_cases,
        historical_cases,
        bindings: &bindings,
        candidates: &[],
        prior_bindings: &[],
        attempts: &[],
        reuse_from: None,
        force: false,
    })
    .map_err(Error::Invalid)
}
