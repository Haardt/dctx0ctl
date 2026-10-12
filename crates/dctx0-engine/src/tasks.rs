//! Explicit task use cases. Reading never checks sources, locks or migrates.
use super::*;
use dctx0_tasks::{Action, CandidatePlan, Plan, Receipt, Session};
use std::collections::BTreeMap;

impl Project {
    pub fn task_preview(&self, snapshot: &str) -> Result<CandidatePlan> {
        let store = self.store(AccessMode::ReadOnly)?;
        let input = store.load(&self.key(), snapshot)?;
        let candidate = dctx0_tasks::plan(&input)?;
        let old = store
            .task_head(&self.key())?
            .map(|id| store.task_session(&self.key(), Some(id), None))
            .transpose()?;
        let (run, latest) = store.task_check(&self.key())?;
        if latest != input.id {
            return Ok(candidate);
        }
        let continuity = if let Some(old) = &old {
            store.task_continuity(&self.key(), old.plan.check_run_id, run)?
        } else {
            BTreeMap::new()
        };
        let proposals = dctx0_tasks::reconcile(old.as_ref(), &candidate, &continuity)?;
        Ok(CandidatePlan {
            tasks: proposals.into_iter().map(|p| p.candidate).collect(),
            ..candidate
        })
    }
    pub fn task_plan(&self, snapshot: &str) -> Result<Plan> {
        // Ensure a valid published input exists before creating any writer state.
        self.load(snapshot)?;
        let _lock = self.lock()?;
        self.recover()?;
        let mut store = self.store(AccessMode::ReadWrite)?;
        let input = store.load(&self.key(), snapshot)?;
        let (run, latest) = store.task_check(&self.key())?;
        if latest != input.id {
            return Err(Error::Invalid("Publication requires the latest valid snapshot; historical snapshots support preview".into()));
        }
        let old = store.task_head(&self.key())?;
        let seq = store.task_sequence(&self.key())?;
        let continuity = if let Some(id) = old {
            let old = store.task_session(&self.key(), Some(id), Some(seq))?;
            store.task_continuity(&self.key(), old.plan.check_run_id, run)?
        } else {
            BTreeMap::new()
        };
        let candidate = dctx0_tasks::plan(&input)?;
        Ok(store.publish_tasks(&self.key(), &candidate, run, old, seq, &continuity)?)
    }
    pub fn task_session(&self, plan: Option<i64>, as_of: Option<i64>) -> Result<Session> {
        Ok(self
            .store(AccessMode::ReadOnly)?
            .task_session(&self.key(), plan, as_of)?)
    }
    pub fn task_show_session(
        &self,
        task: i64,
        plan: Option<i64>,
        as_of: Option<i64>,
    ) -> Result<Session> {
        let store = self.store(AccessMode::ReadOnly)?;
        let plan = if let Some(plan) = plan {
            plan
        } else {
            store.task_membership(&self.key(), task, as_of)?
        };
        let session = store.task_session(&self.key(), Some(plan), as_of)?;
        session.member(task)?;
        Ok(session)
    }
    pub fn task_action(&self, action: Action) -> Result<Receipt> {
        let _lock = self.lock()?;
        self.recover()?;
        self.validate_conformance_action(&action, None)?;
        Ok(self
            .store(AccessMode::ReadWrite)?
            .task_action(&self.key(), action)?)
    }
    pub fn task_action_with_profiles(
        &self,
        action: Action,
        profiles: Option<&LoadedProfiles>,
    ) -> Result<Receipt> {
        let _lock = self.lock()?;
        self.recover()?;
        self.validate_conformance_action(&action, None)?;
        Ok(self
            .store(AccessMode::ReadWrite)?
            .task_action_with_profiles(
                &self.key(),
                action,
                profiles.map(|p| (&p.manifest, p.documents.as_slice())),
            )?)
    }
    pub fn task_action_selected(
        &self,
        action: Action,
        project: Option<&std::path::Path>,
        target: Option<&str>,
    ) -> Result<Receipt> {
        let _lock = self.lock()?;
        self.recover()?;
        self.validate_conformance_action(&action, project)?;
        let loaded = self.profiles(project, target);
        let error = loaded.as_ref().err().map(ToString::to_string);
        let profiles = loaded.as_ref().ok().and_then(Option::as_ref);
        Ok(self
            .store(AccessMode::ReadWrite)?
            .task_action_profile_input(
                &self.key(),
                action,
                profiles.map(|p| (&p.manifest, p.documents.as_slice())),
                error.as_deref(),
            )?)
    }
    pub fn task_context(
        &self,
        session: &Session,
        snapshot: &Snapshot,
        task: i64,
        section: &str,
        page: (usize, Option<&dctx0_tasks::Cursor>),
        profiles: Option<&LoadedProfiles>,
    ) -> Result<dctx0_tasks::ContextPage> {
        session.member(task)?;
        let selection = profiles.map(|p| {
            dctx0_tasks::select_profiles(
                snapshot,
                &session.member(task).unwrap().candidate,
                &p.manifest,
                &p.documents,
            )
        });
        Ok(dctx0_tasks::context_with_profiles(
            session, snapshot, task, section, page.0, page.1, selection,
        )?)
    }
    pub fn task_profile_changes(
        &self,
        profiles: &LoadedProfiles,
        selection: dctx0_tasks::ProfileUpdateSelection,
    ) -> Result<dctx0_tasks::ProfileUpdatePreview> {
        let store = self.store(AccessMode::ReadOnly)?;
        let session = store.task_session(&self.key(), None, None)?;
        let (run, snapshot_id) = store.task_check(&self.key())?;
        if snapshot_id != session.plan.snapshot_id {
            return Err(Error::Invalid(
                "Plan the latest checked specification before selecting profile adaptations".into(),
            ));
        }
        let snapshot = store.load(&self.key(), &snapshot_id)?;
        Ok(dctx0_tasks::profile_changes_at_check(
            &session,
            &snapshot,
            &profiles.manifest,
            &profiles.documents,
            selection,
            run,
        )?)
    }
    pub fn task_profile_update(
        &self,
        preview: &dctx0_tasks::ProfileUpdatePreview,
        profiles: &LoadedProfiles,
        reason: &str,
    ) -> Result<Plan> {
        let _lock = self.lock()?;
        self.recover()?;
        let profiles = self
            .profiles(
                Some(&profiles.project_root),
                Some(&profiles.manifest.target),
            )?
            .ok_or(Error::Invalid("Profile configuration was removed".into()))?;
        Ok(self.store(AccessMode::ReadWrite)?.task_profile_update(
            &self.key(),
            preview,
            &profiles.manifest,
            &profiles.documents,
            reason,
        )?)
    }
    pub fn task_follow_up(&self, task: i64, finding: Option<&str>, reason: &str) -> Result<Plan> {
        let _lock = self.lock()?;
        self.recover()?;
        Ok(self
            .store(AccessMode::ReadWrite)?
            .task_follow_up(&self.key(), task, finding, reason)?)
    }

    pub fn task_renew_review(
        &self,
        task: i64,
        application: Option<&Path>,
        reason: &str,
    ) -> Result<Plan> {
        let _lock = self.lock()?;
        self.recover()?;
        let session = self.task_session(None, None)?;
        let current = session
            .plan
            .members
            .iter()
            .find(|member| member.task_id == task)
            .or_else(|| {
                session.plan.members.iter().find(|member| {
                    matches!(session.definitions[&member.task_id].adjustment,
                Some(dctx0_tasks::AdjustmentReason::ReviewRenewal { source, .. }) if source == task)
                })
            })
            .ok_or_else(|| {
                Error::Invalid("Review is no longer active; use its current successor".into())
            })?;
        let template = self.conformance_review_template(application, current.task_id)?;
        let binding = template
            .conformance
            .ok_or_else(|| Error::Invalid("Conformance binding unavailable".into()))?;
        let hash = dctx0_changes::hash(&(
            binding.snapshot,
            binding.spec_source_hash,
            binding.requirements_hash,
            binding.implementation_artifacts,
        ));
        Ok(self.store(AccessMode::ReadWrite)?.task_renew_review(
            &self.key(),
            task,
            reason,
            &hash,
        )?)
    }
}
