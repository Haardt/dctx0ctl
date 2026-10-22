//! Read-only comparison with the last task review; never renews approval.
use super::*;
use dctx0_tasks::{
    ConformanceReview, ReviewBindingChanges, ReviewReport, compare_review_bindings,
    empty_review_subjects,
};

#[derive(Debug, Serialize)]
pub struct ReviewChangesReport {
    pub task: i64,
    pub plan: i64,
    pub event_seq: i64,
    pub changes: ReviewBindingChanges,
    pub previous: Option<ReviewReport>,
    pub previous_task: Option<i64>,
    pub template: Option<ReviewReport>,
    pub blockers: Vec<String>,
    pub limitations: Vec<String>,
}
impl Project {
    pub fn review_changes(
        &self,
        application: Option<&Path>,
        task: i64,
    ) -> Result<ReviewChangesReport> {
        let (application, _) = self.verification_configuration(application)?;
        let session = self.task_session(None, None)?;
        let member = session.member(task)?;
        let snapshot = self.load(&session.plan.snapshot_id)?;
        let subjects = dctx0_tasks::conformance_subjects(&snapshot.ir, &member.candidate);
        if subjects.is_empty() {
            return Err(Error::Invalid(
                "Task has no assigned conformance obligations".into(),
            ));
        }
        let source_hash = dctx0_changes::source_hash(&self.sources()?);
        let current = ConformanceReview {
            snapshot: snapshot.id,
            requirements_hash: member.candidate.requirements_hash.clone(),
            // Include current drift in diagnostics, but never offer an unpinned template.
            spec_source_hash: source_hash.clone(),
            implementation_artifacts: Capture::inventory(&application)?,
            subjects: empty_review_subjects(subjects),
        };
        // Follow only explicit renewal ancestry, never a similar or convenient
        // older approval. The latest judgement at the nearest source wins.
        let mut source = task;
        let mut visited = std::collections::BTreeSet::new();
        let (previous_task, previous) = loop {
            if !visited.insert(source) {
                return Err(Error::Invalid("Cyclic review renewal ancestry".into()));
            }
            if let Some(report) = session.states.get(&source).and_then(|s| s.reviews.last()) {
                break (Some(source), Some(report.clone()));
            }
            match session
                .definitions
                .get(&source)
                .and_then(|d| d.adjustment.as_ref())
            {
                Some(dctx0_tasks::AdjustmentReason::ReviewRenewal {
                    source: predecessor,
                    ..
                }) => source = *predecessor,
                _ => break (None, None),
            }
        };
        let changes = compare_review_bindings(previous.as_ref(), &current);
        let mut blockers = vec![];
        let template = if source_hash != snapshot.source_hash {
            blockers.push("Spec files changed since the task snapshot; run check and publish matching task requirements before renewing the review".into());
            None
        } else {
            Some(ReviewReport {
                reviewer: String::new(),
                judgement: "request-changes".into(),
                summary: String::new(),
                findings: vec![],
                conformance: Some(current),
            })
        };
        Ok(ReviewChangesReport {
            task,
            plan: session.plan.id,
            event_seq: session.event_seq,
            changes,
            previous,
            previous_task,
            template,
            blockers,
            limitations: vec!["Files are bound to the complete review group; no file-to-obligation dependency is inferred. This comparison neither renews approval nor establishes native PASS.".into()],
        })
    }
}
