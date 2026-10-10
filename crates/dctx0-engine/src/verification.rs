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
