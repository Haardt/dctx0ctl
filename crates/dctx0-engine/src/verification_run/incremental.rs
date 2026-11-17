//! Read-only comparison; this does not authorize reuse or execute cases.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalComparison {
    pub component: String,
    pub matches: Option<bool>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationIncrementalReport {
    /// Complete static partition from the unit planner. Unknown target boundaries
    /// are explicit; this projection cannot authorize technical or task PASS.
    pub unit_execution_plan: UnitExecutionPlan,
    pub schema_version: u32,
    pub parent_run: String,
    pub snapshot: String,
    pub check_occurrence: u64,
    pub required_cases: usize,
    /// Passing contract cases in complete contract units, before provenance checks.
    pub potentially_reusable_cases: usize,
    /// Potential cases blocked by the absence of a complete external input receipt.
    pub blocked_cases: usize,
    /// Read-only planning performs no execution; actual counts are on native run reports.
    pub actual_reused_cases: usize,
    pub execution_available: bool,
    /// Cases requiring execution under the currently available evidence.
    pub rerun_cases: usize,
    pub comparisons: Vec<IncrementalComparison>,
    pub reasons: Vec<VerificationIssue>,
}

impl Project {
    /// Compare a saved run to current published requirements and source inventory.
    /// This performs no execution, writes, reuse or task adoption.
    pub fn verification_incremental_plan(
        &self,
        application: Option<&Path>,
        mut selection: VerificationSelection,
        parent_run: &str,
    ) -> Result<VerificationIncrementalReport> {
        if selection.historical_check.is_some() {
            return Err(Error::Invalid(
                "Incremental comparison requires the current published check".into(),
            ));
        }
        let store = self.store(AccessMode::ReadOnly)?;
        let parent = store.verification_run(&self.key(), parent_run)?;
        let run = &parent.run;
        let plan_ref = run
            .artifacts
            .iter()
            .find(|r| {
                r.hash == run.provenance.plan_hash
                    && r.media_type == "application/vnd.dctx0.plan+json"
            })
            .ok_or_else(|| {
                Error::Invalid("Saved run does not reference its program plan artifact".into())
            })?;
        let artifact = store.verification_artifact(parent_run, &plan_ref.hash)?;
        if artifact.reference != *plan_ref {
            return Err(Error::Invalid(
                "Saved program plan artifact metadata differs from its reference".into(),
            ));
        }
        let saved: ProgramPlan = serde_json::from_slice(&artifact.contents)?;
        if saved.snapshot != run.provenance.snapshot
            || saved.check_occurrence as i64 != run.provenance.check_occurrence
            || saved.budgets != run.budgets
        {
            return Err(Error::Invalid(
                "Saved program plan does not match its run provenance".into(),
            ));
        }
        let (application, config) = self.verification_configuration(application)?;
        if selection.subjects.is_empty() && selection.tasks.is_empty() {
            selection.subjects = config.scope.subjects.clone();
            selection.tasks = config.scope.tasks.clone();
        }
        let fixtures: ProjectionFixtures = crate::read_review_input(
            &crate::verification_setup::safe_file(&application, &config.fixtures)?.1,
            crate::DEFAULT_REVIEW_INPUT_BYTES,
            "Incremental planning fixtures",
        )?;
        let mut current = self.verification_plan(selection, &fixtures, config.budgets.clone())?;
        let lifecycle_issues =
            self.verification_lifecycle_input_issues(&application, &config.adapter, &current)?;
        current.issues.extend(lifecycle_issues);
        let inventory = Capture::inventory_with_budget(&application, config.capture_budgets())?;
        let inventory_ref = run
            .artifacts
            .iter()
            .find(|r| r.media_type == "application/vnd.dctx0.captured-input-inventory+json");
        let prior_inventory = if let Some(reference) = inventory_ref {
            let artifact = store.verification_artifact(parent_run, &reference.hash)?;
            if artifact.reference != *reference {
                return Err(Error::Invalid(
                    "Saved inventory artifact metadata differs from its reference".into(),
                ));
            }
            Some(Capture::code_inventory(
                &application,
                &serde_json::from_slice(&artifact.contents)?,
            )?)
        } else {
            None
        };
        let manifest = dctx0_verification::case_manifest(&current);
        let unit_execution_plan = super::unit_plan::static_plan(
            self,
            &application,
            &current,
            &inventory,
            &run.required_cases,
        )?;
        let mut potential = 0;
        if run.origin == VerificationRunOrigin::Native {
            let records = run
                .cases
                .iter()
                .map(|case| (case.id.as_str(), case))
                .collect::<BTreeMap<_, _>>();
            for unit in dctx0_verification::incremental::program_units(&current)
                .into_iter()
                .filter(|u| u.reusable)
            {
                if unit.cases.iter().all(|id| {
                    records
                        .get(id.as_str())
                        .is_some_and(|case| case.result == VerificationResult::Pass)
                }) {
                    potential += unit.cases.len();
                }
            }
        }
        let plan_matches = saved == current;
        let inventory_matches = prior_inventory.as_ref().map(|prior| prior == &inventory);
        let config_matches =
            run.provenance.configuration_hash == digest(&serde_json::to_vec(&config)?);
        let comparisons = vec![
            IncrementalComparison { component: "program-plan".into(), matches: Some(plan_matches), detail: "Exact published plan including check occurrence, requirements and budgets".into() },
            IncrementalComparison { component: "captured-input-inventory".into(), matches: inventory_matches, detail: "Complete captured application inventory; this does not include all external state".into() },
            IncrementalComparison { component: "configuration".into(), matches: Some(config_matches), detail: "Verification configuration".into() },
            IncrementalComparison { component: "runtime-and-external-inputs".into(), matches: None, detail: "A read-only comparison cannot establish current runtime identity or complete external inputs".into() },
        ];
        let bound = if let Some(reference) = run.artifacts.iter().find(|a| {
            a.media_type == dctx0_verification::incremental::INCREMENTAL_INPUTS_MEDIA_TYPE
        }) {
            let artifact = store.verification_artifact(parent_run, &reference.hash)?;
            let receipt: dctx0_verification::incremental::ExternalInputReceipt =
                serde_json::from_slice(&artifact.contents)?;
            !receipt.deterministic_units.is_empty()
        } else {
            false
        };
        let reasons = vec![if bound {
            issue(
                "INCREMENTAL_EXECUTION_REQUIRED",
                "Planning does not query live revisions or execute cases. Use --no-adopt --reuse-inputs PATH --reuse-from RUN_ID on the matching native run command; that execution verifies current bindings and reports actual reuse",
            )
        } else {
            issue(
                "INCREMENTAL_EXTERNAL_INPUTS_UNAVAILABLE",
                "This parent has no bound deterministic scope. Supply captured snapshot/revision inputs for eligible units; unobserved historical inputs cannot be invented",
            )
        }];
        Ok(VerificationIncrementalReport {
            unit_execution_plan,
            schema_version: 1,
            parent_run: parent.id,
            snapshot: current.snapshot,
            check_occurrence: current.check_occurrence,
            required_cases: manifest.required_cases.len(),
            potentially_reusable_cases: potential,
            blocked_cases: potential,
            actual_reused_cases: 0,
            execution_available: true,
            rerun_cases: manifest.required_cases.len(),
            comparisons,
            reasons,
        })
    }
}
