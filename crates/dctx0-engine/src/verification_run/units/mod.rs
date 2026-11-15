//! Isolated contract cohorts, planned together and published as technical compositions.
use super::*;
use dctx0_verification::unit_planner::{self, UnitPlannerInput, catalog};
pub(super) mod binding;
pub(super) mod materialize;
mod native;
pub(super) use native::UnitNativeContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderUnitGroup {
    pub id: String,
    pub subjects: BTreeSet<String>,
    pub decoders: Vec<ContractDecoderBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderUnitConfiguration {
    pub schema_version: u32,
    pub groups: Vec<DecoderUnitGroup>,
}

#[derive(Debug, Serialize)]
pub struct UnitVerificationReport {
    pub schema_version: u32,
    pub plan: UnitExecutionPlan,
    pub composition_id: String,
    pub composition: EvidenceCompositionV1,
    pub reused_cases: usize,
    pub executed_cases: usize,
    pub native_runs: Vec<String>,
    pub production_calls: BTreeMap<String, usize>,
    pub worker_starts: Option<usize>,
    pub production_calls_complete: bool,
    pub adoption_authority: bool,
}

struct PreparedUnit {
    group: DecoderUnitGroup,
    program: ProgramPlan,
    binding: UnitEvidenceBindingV1,
    recipe: materialize::UnitRecipe,
}

impl Project {
    /// This first connection emits technical compositions only. Task delivery
    /// remains a separate guarded operation, never an imported native PASS.
    pub async fn run_decoder_units(
        &self,
        application: &Path,
        configuration: &str,
        generator: &dyn HarnessGenerator,
        options: &NativeReuseOptions,
        cancellation: &NativeCancellation,
    ) -> Result<UnitVerificationReport> {
        let (root, config) = self.verification_configuration(Some(application))?;
        if config.target != "go" || config.environment.is_none() || !options.inputs.is_empty() {
            return Err(Error::Invalid("Decoder units require Go, an explicit controlled environment and no legacy reuse declaration".into()));
        }
        if !matches!(
            config.environment,
            Some(crate::NativeEnvironment::Controlled { .. })
        ) {
            return Err(Error::Invalid(
                "Decoder units require an explicitly controlled environment".into(),
            ));
        }
        let (_, path) = crate::verification_setup::safe_file(&root, configuration)?;
        let groups: DecoderUnitConfiguration = crate::read_review_input(
            &path,
            crate::DEFAULT_REVIEW_INPUT_BYTES,
            "Decoder unit configuration",
        )?;
        if groups.schema_version != 1 || groups.groups.is_empty() || groups.groups.len() > 128 {
            return Err(Error::Invalid("Invalid decoder unit configuration".into()));
        }
        let fixtures: ProjectionFixtures = crate::read_review_input(
            &crate::verification_setup::safe_file(&root, &config.fixtures)?.1,
            crate::DEFAULT_REVIEW_INPUT_BYTES,
            "Decoder projection fixtures",
        )?;
        let program = self.verification_plan(
            VerificationSelection {
                subjects: config.scope.subjects.clone(),
                tasks: config.scope.tasks.clone(),
                historical_check: None,
            },
            &fixtures,
            config.budgets.clone(),
        )?;
        let required = dctx0_verification::case_manifest(&program).required_cases;
        let store = self.store(AccessMode::ReadWrite)?;
        let mut prepared = Vec::new();
        let mut candidates = Vec::new();
        let mut prior_bindings = Vec::new();
        let mut attempts = Vec::new();
        let mut historical_cases = BTreeSet::new();
        let allowed = binding::allowed_sources(&store, &self.key(), options.parent.as_deref())?;
        for group in groups.groups {
            if group.id.is_empty()
                || !group
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(Error::Invalid(
                    "Decoder group IDs must be nonempty path-safe names".into(),
                ));
            }
            let group_program = self.verification_plan(
                VerificationSelection {
                    subjects: group.subjects.clone(),
                    tasks: BTreeSet::new(),
                    historical_check: None,
                },
                &fixtures,
                config.budgets.clone(),
            )?;
            if !group_program.operations.is_empty()
                || !group_program.lifecycles.is_empty()
                || !group_program.examples.is_empty()
                || !group_program.properties.is_empty()
                || group_program
                    .contracts
                    .iter()
                    .any(|c| !c.identity_requirements.is_empty())
            {
                return Err(Error::Invalid(
                    "Direct decoder units cover complete stateless factory corpora only".into(),
                ));
            }
            let expected: BTreeSet<_> = group_program
                .contracts
                .iter()
                .map(|c| c.subject.clone())
                .collect();
            if group
                .decoders
                .iter()
                .map(|d| d.subject.clone())
                .collect::<BTreeSet<_>>()
                != expected
                || group.decoders.len() != expected.len()
            {
                return Err(Error::Invalid(format!(
                    "Decoder group {} must bind each required contract exactly once",
                    group.id
                )));
            }
            let recipe = materialize::recipe(&root, &config, &group, generator)?;
            let old = store.latest_unit_binding(&self.key(), &group.id)?;
            if let Some(old) = &old {
                historical_cases.extend(old.unit.cases.iter().cloned());
                prior_bindings.push(old.clone());
            }
            let binding = binding::current(self, &group_program, &group, &recipe, old.as_ref())?;
            let fingerprint = unit_planner::evidence_digest(&binding)?;
            candidates.extend(store.unit_evidence_candidates(
                &self.key(),
                &group.id,
                &fingerprint,
            )?);
            attempts.extend(store.unit_attempts(&self.key(), &group.id, &fingerprint)?);
            prepared.push(PreparedUnit {
                group,
                program: group_program,
                binding,
                recipe,
            });
        }
        let bindings = prepared
            .iter()
            .map(|unit| unit.binding.clone())
            .collect::<Vec<_>>();
        let plan = unit_planner::plan_units(UnitPlannerInput {
            required_cases: &required,
            historical_cases: &historical_cases,
            bindings: &bindings,
            candidates: &candidates,
            prior_bindings: &prior_bindings,
            attempts: &attempts,
            reuse_from: allowed.as_ref(),
            force: options.mode == NativeReuseMode::Off,
        })
        .map_err(Error::Invalid)?;
        let reused: BTreeSet<_> = plan
            .decisions
            .iter()
            .filter(|d| d.action == UnitExecutionAction::Reuse)
            .flat_map(|d| d.cases.iter().cloned())
            .collect();
        unit_planner::execution_gate(
            &required,
            &reused,
            options.mode == NativeReuseMode::Off,
            options.mode == NativeReuseMode::Require,
            !historical_cases.is_empty(),
        )
        .map_err(|code| {
            Error::Invalid(format!(
                "{code}: {}",
                serde_json::to_string(&plan).unwrap_or_default()
            ))
        })?;
        eprintln!(
            "Unit inventory: {} mandatory cases; {} reusable; {} fresh",
            required.len(),
            reused.len(),
            required.len() - reused.len()
        );
        drop(store);
        let mut composition = EvidenceCompositionV1 {
            schema_version: 1,
            required_cases: required.clone(),
            units: Vec::new(),
            removed_cases: plan.removed_cases.clone(),
        };
        let mut native_runs = Vec::new();
        let mut production_calls = BTreeMap::new();
        let mut worker_starts = Some(0);
        let mut executed_cases = 0;
        for (unit, decision) in prepared.iter_mut().zip(&plan.decisions) {
            if decision.action == UnitExecutionAction::Reuse {
                composition.units.push(ComposedUnitEvidenceV1 {
                    binding: unit.binding.clone(),
                    coverage: UnitCoverage::ReusedPass,
                    proof: decision.source.clone(),
                });
                continue;
            }
            let mut context = UnitNativeContext::new(self, unit)?;
            let workspace = materialize::write(&root, &unit.group, &unit.recipe)?;
            let fresh = NativeReuseOptions {
                mode: NativeReuseMode::Off,
                inputs: String::new(),
                parent: None,
            };
            let outcome = self
                .execute_verification(
                    Some(&workspace),
                    VerificationSelection {
                        subjects: unit.group.subjects.clone(),
                        tasks: BTreeSet::new(),
                        historical_check: None,
                    },
                    generator,
                    cancellation,
                    true,
                    None,
                    unit.recipe
                        .configuration
                        .execution
                        .as_ref()
                        .map(VerificationExecutionBudgets::native_limits)
                        .unwrap_or_default(),
                    None,
                    None,
                    None,
                    None,
                    Some(&fresh),
                    None,
                    Some(&mut context),
                )
                .await;
            let report = match outcome {
                Ok(ExecutionReport::Published(report)) => report,
                Ok(_) => return Err(Error::Invalid("Unexpected transient unit evidence".into())),
                Err(error) => {
                    context.fail(self)?;
                    return Err(error);
                }
            };
            let (entry, calls, starts) = context.publish(self, &report)?;
            executed_cases += report.evidence.run.cases.len();
            for (subject, count) in calls {
                *production_calls.entry(subject).or_insert(0) += count;
            }
            worker_starts = worker_starts
                .zip(starts)
                .map(|(total, count)| total + count);
            native_runs.push(report.evidence.id);
            unit.binding = entry.binding.clone();
            composition.units.push(entry);
        }
        // A second static binding pass catches drift affecting reused units while
        // another unit was built or executed. No business code is started here.
        for (unit, evidence) in prepared.iter().zip(&composition.units) {
            let current = binding::current(
                self,
                &unit.program,
                &unit.group,
                &unit.recipe,
                Some(&evidence.binding),
            )?;
            if current != evidence.binding {
                return Err(Error::Invalid(format!(
                    "UNIT_INPUTS_DRIFT: {}",
                    unit.group.id
                )));
            }
        }
        let composition_id = self
            .store(AccessMode::ReadWrite)?
            .record_evidence_composition(&self.key(), &composition)?;
        Ok(UnitVerificationReport {
            schema_version: 1,
            plan,
            composition_id,
            composition,
            reused_cases: reused.len(),
            executed_cases,
            native_runs,
            production_calls,
            worker_starts,
            production_calls_complete: worker_starts.is_some(),
            adoption_authority: false,
        })
    }
}
