//! Exact pending-case execution from an immutable plan, without scope expansion.
//! This diagnostic produces new observations; it never promotes a partial parent.
use super::*;
mod basis;
mod observations;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationContinuationPlan {
    pub schema_version: u32,
    pub parent_run: String,
    pub plan_hash: String,
    pub required_cases: usize,
    pub recorded_pass_cases: usize,
    pub selected_cases: BTreeSet<String>,
    pub derived_cases: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_inputs_hash: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub additional_sources: BTreeSet<String>,
    #[serde(default)]
    pub current_bindings_validated: bool,
    pub adoption_authority: bool,
}

pub(super) struct ContinuationSelection {
    pub plan: VerificationContinuationPlan,
    pub identity_context: Vec<VerificationCaseRecord>,
    pub lifecycle_inputs: Option<VerificationLifecycleFixtures>,
}

impl Project {
    /// Read only: select exact missing/nonpassing IDs, never their spec closure.
    pub fn verification_continuation_plan(
        &self,
        parent_run: &str,
        requested: &BTreeSet<String>,
        max_cases: usize,
    ) -> Result<VerificationContinuationPlan> {
        self.verification_continuation_plan_from_evidence(
            parent_run,
            requested,
            max_cases,
            &BTreeSet::new(),
        )
    }

    /// Explicit native replay sources affect the diagnostic pending ledger only.
    pub fn verification_continuation_plan_from_evidence(
        &self,
        parent_run: &str,
        requested: &BTreeSet<String>,
        max_cases: usize,
        additional: &BTreeSet<String>,
    ) -> Result<VerificationContinuationPlan> {
        let store = self.store(AccessMode::ReadOnly)?;
        let mut basis = basis::load(self, &store, parent_run)?;
        let mut sources = basis.additional_sources.clone();
        sources.extend(additional.iter().cloned());
        observations::merge(self, &store, &mut basis, &sources)?;
        let parent = &basis.root;
        let program = &basis.program;
        let manifest = dctx0_verification::case_manifest(program);
        let passed = basis
            .cases
            .iter()
            .filter(|case| case.result == VerificationResult::Pass)
            .map(|case| case.id.clone())
            .collect::<BTreeSet<_>>();
        let selected = if requested.is_empty() {
            manifest
                .required_cases
                .difference(&passed)
                .cloned()
                .collect()
        } else {
            requested.clone()
        };
        if !selected.is_subset(&manifest.required_cases) {
            return Err(Error::Invalid(
                "Continuation needs nonempty exact mandatory case IDs".into(),
            ));
        }
        if selected.len() > max_cases {
            return Err(Error::Invalid(format!(
                "CONTINUATION_LIMIT: {} selected cases exceed the explicit limit {max_cases}; no execution started",
                selected.len()
            )));
        }
        let derived: BTreeSet<String> = program
            .contracts
            .iter()
            .filter(|contract| !contract.identity_requirements.is_empty())
            .map(|contract| format!("{}:identity", contract.subject))
            .filter(|id| selected.contains(id))
            .collect();
        let snapshot = store.load(&self.key(), &program.snapshot)?;
        // A central identity derivation keeps every producer's original backend.
        let mut ownership = Vec::with_capacity(selected.len());
        for id in &selected {
            let marker = if derived.contains(id) {
                dctx0_verification::entity_identity_case(
                    &snapshot.ir,
                    id.strip_suffix(":identity").unwrap_or(id),
                    &basis.cases,
                    &program.budgets,
                )
            } else {
                VerificationCaseRecord {
                    id: id.clone(),
                    subject: program.scope.subjects.first().cloned().unwrap_or_default(),
                    kind: VerificationCaseKind::Example,
                    bindings: Default::default(),
                    setup: vec![],
                    arguments: vec![],
                    observations: vec![],
                    events: None,
                    resources: None,
                    states: vec![],
                    result: unknown("PENDING_NATIVE_CASE", id),
                    elapsed_us: 0,
                }
            };
            ownership.push(marker);
        }
        super::replay::require_adapter_replay_owners(
            &store,
            &parent.id,
            &parent.run,
            program,
            &ownership,
        )?;
        Ok(VerificationContinuationPlan {
            schema_version: 1,
            parent_run: parent_run.into(),
            plan_hash: parent.run.provenance.plan_hash.clone(),
            required_cases: manifest.required_cases.len(),
            recorded_pass_cases: passed.len(),
            selected_cases: selected,
            derived_cases: derived,
            lifecycle_inputs_hash: None,
            additional_sources: sources,
            current_bindings_validated: false,
            adoption_authority: false,
        })
    }

    /// Static freshness for a zero-pending diagnostic. Never launches revisions,
    /// a build or production. External input compatibility remains unestablished.
    pub fn validate_continuation_source_inputs(
        &self,
        application: Option<&Path>,
        selection: &VerificationContinuationPlan,
    ) -> Result<()> {
        let store = self.store(AccessMode::ReadOnly)?;
        let saved = store.verification_run(&self.key(), &selection.parent_run)?;
        if saved.run.provenance.plan_hash != selection.plan_hash {
            return Err(Error::Invalid("Continuation plan changed".into()));
        }
        let (root, config) = self.verification_configuration(application)?;
        let refs = saved
            .run
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact.media_type == "application/vnd.dctx0.captured-input-inventory+json"
            })
            .collect::<Vec<_>>();
        let [reference] = refs.as_slice() else {
            return Err(Error::Invalid(
                "Continuation needs one captured input inventory".into(),
            ));
        };
        let artifact = store.verification_artifact(&saved.id, &reference.hash)?;
        let captured: BTreeMap<String, String> = serde_json::from_slice(&artifact.contents)?;
        if Capture::proof_inventory(&root, config.capture_budgets(), &saved.run)? != captured {
            return Err(Error::Invalid("CONTINUATION_INPUTS_CHANGED: zero-pending history does not validate changed sources".into()));
        }
        let snapshot = store.load(&self.key(), &saved.run.provenance.snapshot)?;
        if dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash {
            return Err(Error::Invalid("CONTINUATION_SPEC_CHANGED: saved observations do not validate changed requirements".into()));
        }
        Ok(())
    }

    /// Execute only selected pending sessions. Historical identity inputs are
    /// explicitly referenced, never counted as freshly executed operations.
    pub async fn continue_verification(
        &self,
        application: Option<&Path>,
        selection: VerificationContinuationPlan,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
    ) -> Result<NativeVerificationReport> {
        self.continue_verification_with_lifecycle_inputs(
            application,
            selection,
            generator,
            cancellation,
            None,
        )
        .await
    }

    /// Fresh explicit fixture values change the selected input identities, not
    /// requirements or assertions. No previously passing case is adopted here.
    pub async fn continue_verification_with_lifecycle_inputs(
        &self,
        application: Option<&Path>,
        mut selection: VerificationContinuationPlan,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        inputs: Option<VerificationLifecycleFixtures>,
    ) -> Result<NativeVerificationReport> {
        if selection.selected_cases.is_empty() {
            return Err(Error::Invalid(
                "No pending cases; no execution started".into(),
            ));
        }
        let mut expected = self.verification_continuation_plan_from_evidence(
            &selection.parent_run,
            &selection.selected_cases,
            selection.selected_cases.len(),
            &selection.additional_sources,
        )?;
        expected.lifecycle_inputs_hash = inputs
            .as_ref()
            .map(|inputs| serde_json::to_vec(inputs).map(|bytes| digest(&bytes)))
            .transpose()?;
        if selection.lifecycle_inputs_hash.is_none() {
            selection.lifecycle_inputs_hash = expected.lifecycle_inputs_hash.clone();
        }
        if serde_json::to_vec(&expected)? != serde_json::to_vec(&selection)? {
            return Err(Error::Invalid(
                "Continuation selection changed before execution".into(),
            ));
        }
        let store = self.store(AccessMode::ReadOnly)?;
        let mut basis = basis::load(self, &store, &selection.parent_run)?;
        observations::merge(self, &store, &mut basis, &selection.additional_sources)?;
        let parent = &basis.root;
        let program = basis.program;
        let (root, config) = self.verification_configuration(application)?;
        let inventories = parent
            .run
            .artifacts
            .iter()
            .filter(|entry| {
                entry.media_type == "application/vnd.dctx0.captured-input-inventory+json"
            })
            .collect::<Vec<_>>();
        let [inventory] = inventories.as_slice() else {
            return Err(Error::Invalid(
                "Continuation needs one intact captured-input inventory".into(),
            ));
        };
        let inventory = store.verification_artifact(&parent.id, &inventory.hash)?;
        let original_inputs: BTreeMap<String, String> =
            serde_json::from_slice(&inventory.contents)?;
        if Capture::proof_inventory(&root, config.capture_budgets(), &parent.run)?
            != original_inputs
        {
            return Err(Error::Invalid(
                "CONTINUATION_INPUTS_CHANGED: original inputs changed; no pending cases started"
                    .into(),
            ));
        }
        let snapshot = store.load(&self.key(), &program.snapshot)?;
        if dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash {
            return Err(Error::Invalid(
                "CONTINUATION_SPEC_CHANGED: original requirements changed; no execution started"
                    .into(),
            ));
        }
        let directory = Path::new(&config.adapter).parent().unwrap_or(Path::new(""));
        let path = root.join(directory).join("lifecycles.json");
        let mut lifecycles: VerificationLifecycleFixtures = if path.try_exists()? {
            serde_json::from_slice(&fs::read(path)?)?
        } else {
            Default::default()
        };
        if let Some(inputs) = &inputs {
            if inputs.paths.is_empty() {
                return Err(Error::Invalid(
                    "Explicit lifecycle inputs must not be empty".into(),
                ));
            }
            for (id, input) in &inputs.paths {
                let original = lifecycles.paths.get(id).ok_or_else(|| {
                    Error::Invalid(format!(
                        "Explicit lifecycle input is absent from the original plan: {id}"
                    ))
                })?;
                if !selection.selected_cases.contains(id)
                    || !program
                        .lifecycles
                        .iter()
                        .flat_map(|lifecycle| &lifecycle.paths)
                        .any(|path| &path.id == id)
                    || input.keys != original.keys
                    || input.setup != original.setup
                    || input.arguments.len() != original.arguments.len()
                    || input
                        .arguments
                        .iter()
                        .zip(&original.arguments)
                        .any(|(new, old)| new.len() != old.len())
                {
                    return Err(Error::Invalid(format!(
                        "Explicit lifecycle inputs may change selected argument values only: {id}"
                    )));
                }
            }
            lifecycles.paths.extend(inputs.paths.clone());
        }
        let context = ContinuationSelection {
            plan: selection.clone(),
            identity_context: basis
                .cases
                .into_iter()
                .filter(|case| case.kind == VerificationCaseKind::Operation)
                .collect(),
            lifecycle_inputs: inputs,
        };
        drop(store);
        eprintln!(
            "Continuation: {} original mandatory cases; {} selected, including {} central identity derivations; no historical PASS promoted",
            selection.required_cases,
            selection.selected_cases.len(),
            selection.derived_cases.len()
        );
        let limits = config
            .execution
            .as_ref()
            .map(VerificationExecutionBudgets::native_limits)
            .unwrap_or_default();
        match self
            .execute_verification(
                Some(&root),
                Default::default(),
                generator,
                cancellation,
                true,
                Some((program, lifecycles, Some(selection.parent_run))),
                limits,
                None,
                None,
                None,
                None,
                None,
                Some(&context),
                None,
            )
            .await?
        {
            ExecutionReport::Published(report) => Ok(report),
            ExecutionReport::Transient(_) => {
                Err(Error::Invalid("Unexpected transient continuation".into()))
            }
        }
    }
}
