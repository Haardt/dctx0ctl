//! Read-only planning from published requirements. Never implicitly checks specs.
use super::*;
use dctx0_core::VerificationBudgets;
use dctx0_verification::{ProgramPlan, ProjectionFixtures, VerificationScope};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default)]
pub struct VerificationSelection {
    pub subjects: BTreeSet<String>,
    pub tasks: BTreeSet<i64>,
    /// Explicitly inspect this historical occurrence; never adoption authority.
    pub historical_check: Option<i64>,
}
impl Project {
    /// Inspect configured lifecycle inputs without executing an adapter.
    pub fn verification_lifecycle_input_issues(
        &self,
        application: &std::path::Path,
        adapter: &str,
        plan: &ProgramPlan,
    ) -> Result<Vec<dctx0_core::VerificationIssue>> {
        if plan.lifecycles.is_empty() {
            return Ok(vec![]);
        }
        let adapter = std::path::Path::new(adapter);
        if adapter.is_absolute()
            || adapter
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(Error::Invalid("Invalid lifecycle adapter path".into()));
        }
        let path = application
            .join(adapter.parent().unwrap_or(std::path::Path::new("")))
            .join("lifecycles.json");
        let fixtures: crate::VerificationLifecycleFixtures = if path.try_exists()? {
            if !path
                .canonicalize()?
                .starts_with(application.canonicalize()?)
                || fs::metadata(&path)?.len() > 524_288
            {
                return Err(Error::Invalid(
                    "Lifecycle inputs escape the application or exceed the size limit".into(),
                ));
            }
            serde_json::from_slice(&fs::read(path)?)?
        } else {
            Default::default()
        };
        let snapshot = self
            .store(AccessMode::ReadOnly)?
            .load(&self.key(), &plan.snapshot)?;
        let mut issues = vec![];
        for lifecycle in &plan.lifecycles {
            for path in &lifecycle.paths {
                let message = match fixtures.paths.get(&path.id) {
                    None => Some(
                        "Lifecycle path needs explicitly reviewed keys, setup and transition arguments",
                    ),
                    Some(input) if input.arguments.len() != path.transitions.len() => {
                        Some("Lifecycle arguments do not cover the complete mandatory path")
                    }
                    Some(input) => {
                        if let dctx0_core::VerificationResult::Unknown { mut issue } =
                            dctx0_verification::lifecycle_input_contract(
                                &snapshot.ir,
                                lifecycle,
                                path,
                                input,
                                &plan.budgets,
                            )
                        {
                            issue.message = format!("{}: {}", path.id, issue.message);
                            issues.push(issue);
                        }
                        None
                    }
                };
                if let Some(message) = message {
                    issues.push(dctx0_core::VerificationIssue {
                        code: "LIFECYCLE_INPUT_REQUIRED".into(),
                        message: format!("{}: {message}", path.id),
                        span: None,
                    });
                }
            }
        }
        Ok(issues)
    }

    pub fn saved_verification_run(&self, id: &str) -> Result<dctx0_store::StoredVerificationRun> {
        Ok(self
            .store(AccessMode::ReadOnly)?
            .verification_run(&self.key(), id)?)
    }

    pub fn verification_plan(
        &self,
        selection: VerificationSelection,
        fixtures: &ProjectionFixtures,
        budgets: VerificationBudgets,
    ) -> Result<ProgramPlan> {
        if selection.subjects.is_empty() && selection.tasks.is_empty() {
            return Err(Error::Invalid("Verification needs explicit subjects or tasks; an empty scope never selects the whole application".into()));
        }
        let store = self.store(AccessMode::ReadOnly)?;
        let key = self.key();
        let (latest_check, latest_snapshot) = store.task_check(&key)?;
        let (occurrence, snapshot) = if let Some(check) = selection.historical_check {
            let entry = store
                .history(&key)?
                .into_iter()
                .find(|r| r.run_id == check && r.status == "pass")
                .ok_or_else(|| {
                    Error::Invalid(format!("Unknown successful check occurrence {check}"))
                })?;
            (
                entry.run_id,
                entry
                    .snapshot_id
                    .ok_or_else(|| Error::Invalid("Successful check has no snapshot".into()))?,
            )
        } else {
            (latest_check, latest_snapshot)
        };
        let snapshot = store.load(&key, &snapshot)?;
        if selection.historical_check.is_none()
            && dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash
        {
            return Err(Error::Invalid("Spec files changed since the published check; run check first or select a historical check explicitly".into()));
        }
        let mut subjects = selection.subjects;
        if !selection.tasks.is_empty() {
            let session = store.task_session(&key, None, None)?;
            if session.plan.check_run_id != occurrence || session.plan.snapshot_id != snapshot.id {
                return Err(Error::Invalid("Selected task plan belongs to a different check occurrence; publish matching task requirements first".into()));
            }
            for task in &selection.tasks {
                subjects.extend(session.member(*task)?.candidate.subjects.clone());
            }
        }
        let occurrence = u64::try_from(occurrence)
            .map_err(|_| Error::Invalid("Check occurrence must be positive".into()))?;
        Ok(dctx0_verification::project_program(
            &snapshot.ir,
            &snapshot.id,
            occurrence,
            VerificationScope {
                subjects,
                tasks: selection.tasks.iter().map(|id| format!("T{id}")).collect(),
            },
            fixtures,
            budgets,
        ))
    }
}
