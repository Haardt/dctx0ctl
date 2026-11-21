//! Read-only comparison; this does not authorize reuse or execute cases.
use super::*;
mod groups;
mod tasks;
pub use groups::{IncrementalTestGroup, TestGroupChange, TestGroupRunnerStatus};
pub use tasks::{IncrementalTaskCause, IncrementalTaskImpact, TaskImpactClassification};

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
    #[serde(default)]
    pub historical_check_occurrence: i64,
    pub historical_task_plan_id: Option<i64>,
    pub current_task_plan_id: Option<i64>,
    #[serde(default)]
    pub task_impacts: Vec<IncrementalTaskImpact>,
    #[serde(default)]
    pub test_groups: Vec<IncrementalTestGroup>,
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
        let key = self.key();
        let old_check = run.provenance.check_occurrence;
        let (new_check, new_snapshot) = store.task_check(&key)?;
        let before = store.load(&key, &saved.snapshot)?;
        let after = if before.id == new_snapshot {
            &before
        } else {
            &store.load(&key, &new_snapshot)?
        };
        let old_plan = store.task_plan_at_check(
            &key,
            old_check,
            run.provenance.task_pin.as_ref().map(|pin| pin.plan_id),
        )?;
        let new_plan = store.task_plan_at_check(&key, new_check, None)?;
        let lineage = tasks::node_lineage(
            &before,
            after,
            store.task_continuity(&key, old_check, new_check)?,
        );
        let [old_subjects, new_subjects] = comparison_subjects(
            selection.subjects.clone(),
            &selection.tasks,
            [old_plan.as_ref(), new_plan.as_ref()],
            &lineage,
            [&before, after],
        )?;
        let fixtures: ProjectionFixtures = crate::read_review_input(
            &crate::verification_setup::safe_file(&application, &config.fixtures)?.1,
            crate::DEFAULT_REVIEW_INPUT_BYTES,
            "Incremental planning fixtures",
        )?;
        if dctx0_changes::source_hash(&self.sources()?) != after.source_hash {
            return Err(Error::Invalid("Spec files changed since the published check; run check first or select a historical check explicitly".into()));
        }
        let mut current = dctx0_verification::project_program(
            &after.ir,
            &after.id,
            u64::try_from(new_check)
                .map_err(|_| Error::Invalid("Check occurrence must be positive".into()))?,
            dctx0_verification::VerificationScope {
                subjects: new_subjects.clone(),
                tasks: selection.tasks.iter().map(|id| format!("T{id}")).collect(),
            },
            &fixtures,
            config.budgets.clone(),
        );
        let lifecycle_issues =
            self.verification_lifecycle_input_issues(&application, &config.adapter, &current)?;
        current.issues.extend(lifecycle_issues);
        let inventory = Capture::inventory_with_budget(&application, config.capture_budgets())?;
        let inventory_ref = run
            .artifacts
            .iter()
            .find(|r| r.media_type == "application/vnd.dctx0.captured-input-inventory+json");
        let captured_inventory: Option<BTreeMap<String, String>> =
            if let Some(reference) = inventory_ref {
                let artifact = store.verification_artifact(parent_run, &reference.hash)?;
                if artifact.reference != *reference {
                    return Err(Error::Invalid(
                        "Saved inventory artifact metadata differs from its reference".into(),
                    ));
                }
                Some(serde_json::from_slice(&artifact.contents)?)
            } else {
                None
            };
        let prior_inventory = captured_inventory
            .as_ref()
            .map(|prior| Capture::code_inventory(&application, prior))
            .transpose()?;
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
        let receipt: Option<dctx0_verification::incremental::ExternalInputReceipt> =
            if let Some(reference) = run.artifacts.iter().find(|a| {
                a.media_type == dctx0_verification::incremental::INCREMENTAL_INPUTS_MEDIA_TYPE
            }) {
                let artifact = store.verification_artifact(parent_run, &reference.hash)?;
                if artifact.reference != *reference {
                    return Err(Error::Invalid(
                        "Saved input receipt metadata differs from its reference".into(),
                    ));
                }
                Some(serde_json::from_slice(&artifact.contents)?)
            } else {
                None
            };
        let mut reasons = vec![if receipt
            .as_ref()
            .is_some_and(|receipt| !receipt.deterministic_units.is_empty())
        {
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
        if new_plan.is_none() {
            reasons.push(issue(
                "CURRENT_TASK_PLAN_MISSING",
                "Current task IDs will be available after a task plan is published for this check",
            ));
        }
        if old_plan.is_none() {
            reasons.push(issue(
                "HISTORICAL_TASK_PLAN_MISSING",
                "No saved task plan is available for the historical check",
            ));
        }
        let historical_scope = if old_subjects == saved.scope.subjects {
            saved.closure.clone()
        } else {
            let saved_fixtures = ProjectionFixtures {
                operations: saved
                    .operations
                    .iter()
                    .map(|operation| (operation.operation.id.clone(), operation.cases.clone()))
                    .collect(),
                generated_preparations: saved
                    .operations
                    .iter()
                    .map(|operation| {
                        (
                            operation.operation.id.clone(),
                            operation.generated_preparations.clone(),
                        )
                    })
                    .collect(),
                ..Default::default()
            };
            dctx0_verification::project_program(
                &before.ir,
                &before.id,
                saved.check_occurrence,
                dctx0_verification::VerificationScope {
                    subjects: old_subjects.clone(),
                    tasks: BTreeSet::new(),
                },
                &saved_fixtures,
                saved.budgets.clone(),
            )
            .closure
        };
        let scoped_tasks = if selection.tasks.is_empty() {
            [
                task_scope(&before, &historical_scope, &old_subjects),
                task_scope(after, &current.closure, &new_subjects),
            ]
        } else {
            // Explicit task selectors add their own IDs and corresponding
            // candidates. A simultaneous subject selection adds its own scope.
            let scopes = if selection.subjects.is_empty() {
                [BTreeSet::new(), BTreeSet::new()]
            } else {
                comparison_subjects(
                    selection.subjects,
                    &BTreeSet::new(),
                    [None, None],
                    &lineage,
                    [&before, after],
                )?
            };
            std::array::from_fn(|side| {
                let subjects = &scopes[side];
                let snapshot = [&before, after][side];
                subjects
                    .iter()
                    .flat_map(|subject| {
                        snapshot
                            .ir
                            .symbols
                            .keys()
                            .filter(move |id| {
                                *id == subject
                                    || id
                                        .strip_prefix(subject)
                                        .is_some_and(|suffix| suffix.starts_with('.'))
                            })
                            .cloned()
                    })
                    .collect()
            })
        };
        let task_impacts = tasks::compare(
            [(old_check, &before), (new_check, after)],
            [old_plan.as_ref(), new_plan.as_ref()],
            [&scoped_tasks[0], &scoped_tasks[1]],
            &selection.tasks,
            &lineage,
        );
        let binding_inventory_matches = captured_inventory
            .as_ref()
            .map(|prior| {
                Capture::reuse_inventory_matches(&application, config.capture_budgets(), run, prior)
            })
            .transpose()?;
        let runner = groups::StaticBinding::inspect(
            [&saved, &current],
            run,
            receipt.as_ref(),
            &application,
            binding_inventory_matches,
            &config.target,
            &store.verification_later_nonpass(&key, parent_run)?,
        )?;
        let lifecycle_fixtures = if current.lifecycles.is_empty() {
            Default::default()
        } else {
            crate::verification::read_verification_lifecycle_fixtures(
                &application,
                &config.adapter,
            )?
        };
        let test_groups = groups::compare(
            [&saved, &current],
            [&before, after],
            &historical_scope,
            run,
            &runner,
            &lifecycle_fixtures,
            &lineage,
        )?;
        Ok(VerificationIncrementalReport {
            unit_execution_plan,
            schema_version: 1,
            parent_run: parent.id,
            snapshot: current.snapshot,
            check_occurrence: current.check_occurrence,
            historical_check_occurrence: old_check,
            historical_task_plan_id: old_plan.map(|plan| plan.id),
            current_task_plan_id: new_plan.map(|plan| plan.id),
            task_impacts,
            test_groups,
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

fn comparison_subjects(
    subjects: BTreeSet<String>,
    tasks: &BTreeSet<i64>,
    plans: [Option<&dctx0_tasks::Plan>; 2],
    lineage: &BTreeMap<String, String>,
    snapshots: [&dctx0_changes::Snapshot; 2],
) -> Result<[BTreeSet<String>; 2]> {
    if subjects.is_empty() && tasks.is_empty() {
        return Err(Error::Invalid("Verification needs explicit subjects or tasks; an empty scope never selects the whole application".into()));
    }
    let reverse: BTreeMap<_, _> = lineage.iter().map(|(old, new)| (new, old)).collect();
    let mut old: BTreeSet<_> = subjects
        .iter()
        .filter_map(|id| {
            reverse
                .get(id)
                .copied()
                .or_else(|| snapshots[0].ir.symbols.contains_key(id).then_some(id))
        })
        .cloned()
        .collect();
    let mut new = subjects;
    for subject in &old {
        if !snapshots[1].ir.symbols.contains_key(subject)
            && let Some(current) = lineage.get(subject)
        {
            new.remove(subject);
            new.insert(current.clone());
        }
    }
    for task in tasks {
        let [historical, current] = plans.map(|plan| {
            plan.and_then(|plan| plan.members.iter().find(|member| member.task_id == *task))
        });
        if let Some(member) = current {
            new.extend(member.candidate.subjects.iter().cloned());
            if historical.is_none() {
                old.extend(
                    member
                        .candidate
                        .subjects
                        .iter()
                        .filter_map(|id| reverse.get(id).map(|old| (*old).clone())),
                );
            }
        }
        if let Some(member) = historical {
            old.extend(member.candidate.subjects.iter().cloned());
            if current.is_none() {
                new.extend(
                    member
                        .candidate
                        .subjects
                        .iter()
                        .filter_map(|id| lineage.get(id).cloned()),
                );
            }
        }
        if historical.is_none() && current.is_none() {
            return Err(dctx0_tasks::Error::Missing(*task).into());
        }
    }
    Ok([old, new])
}

fn task_scope(
    snapshot: &dctx0_changes::Snapshot,
    closure: &BTreeSet<String>,
    selected: &BTreeSet<String>,
) -> BTreeSet<String> {
    // Projection includes ancestors to collect governing facts. Their tasks
    // enter scope only when explicitly selected.
    let mut result = closure.clone();
    for subject in closure {
        if let Some(parent) = snapshot
            .ir
            .symbols
            .get(subject)
            .and_then(|symbol| symbol.parent.as_ref())
            && !selected.iter().any(|root| {
                parent == root
                    || parent
                        .strip_prefix(root)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
        {
            result.remove(parent);
        }
    }
    result
}
