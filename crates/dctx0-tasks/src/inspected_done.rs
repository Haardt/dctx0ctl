//! Authored, source-bound manual findings for a freshly executed hybrid run.
//!
//! Validation here checks the exact task and source binding, not execution. The
//! caller must independently validate the actual full native run and its cases.
use crate::{Candidate, TaskKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const TASK_INSPECTIONS_MEDIA_TYPE: &str = "application/vnd.dctx0.inspected-done-tasks+json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectedDoneBatch {
    pub schema_version: u32,
    pub actor: String,
    pub snapshot: String,
    pub check_occurrence: i64,
    pub task_pin: dctx0_core::VerificationTaskPin,
    /// Source hashes are independently checked against the executed inventory.
    pub sources: BTreeMap<String, String>,
    pub facts: BTreeMap<String, SharedFactInspection>,
    pub tasks: BTreeMap<i64, DoneTaskInspection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedFactInspection {
    pub fact: String,
    pub task_ids: BTreeSet<i64>,
    pub finding: String,
    pub sources: BTreeSet<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoneTaskInspection {
    pub state_version: i64,
    pub requirements_hash: String,
    pub profile_bundle: String,
    pub subjects: BTreeSet<String>,
    pub types: BTreeSet<String>,
    pub facts: BTreeSet<String>,
    /// Keys in the batch's fact catalog; each fact is resolved independently.
    pub fact_reviews: BTreeSet<String>,
    pub implementation_finding: String,
    pub profile_finding: String,
    /// Required for adapter tasks; absent for other task kinds.
    pub adapter_binding_finding: Option<String>,
    pub sources: BTreeSet<String>,
    pub limitations: Vec<String>,
}

impl InspectedDoneBatch {
    /// Reject orphaned or one-sided shared findings and source substitutions.
    pub fn valid_catalog(&self, captured_sources: &BTreeMap<String, String>) -> bool {
        !self.tasks.is_empty()
            && !self.sources.is_empty()
            && self
                .tasks
                .keys()
                .all(|id| self.task_pin.requirements.contains_key(id))
            && self
                .sources
                .iter()
                .all(|(path, hash)| !hash.is_empty() && captured_sources.get(path) == Some(hash))
            && self.facts.iter().all(|(key, fact)| {
                !fact.task_ids.is_empty()
                    && fact.task_ids.iter().all(|id| {
                        self.tasks
                            .get(id)
                            .is_some_and(|task| task.fact_reviews.contains(key))
                    })
            })
    }

    /// Check authored findings against the *current* candidate and the actual
    /// native run's captured implementation. No saved PASS can satisfy this.
    pub fn validate_task(
        &self,
        id: i64,
        candidate: &Candidate,
        captured_sources: &BTreeMap<String, String>,
    ) -> bool {
        let Some(task) = self.tasks.get(&id) else {
            return false;
        };
        let sources_match = |paths: &BTreeSet<String>| {
            !paths.is_empty()
                && paths.iter().all(|path| {
                    self.sources.get(path).is_some_and(|hash| {
                        !hash.is_empty() && captured_sources.get(path) == Some(hash)
                    })
                })
        };
        let facts = task
            .fact_reviews
            .iter()
            .filter_map(|key| self.facts.get(key));
        let mut reviewed = BTreeSet::new();
        let facts_valid = facts.fold(true, |valid, review| {
            valid
                && review.task_ids.contains(&id)
                && !review.finding.trim().is_empty()
                && sources_match(&review.sources)
                && reviewed.insert(review.fact.clone())
        });
        self.schema_version == 1
            && !self.actor.trim().is_empty()
            && !self.snapshot.trim().is_empty()
            && self.check_occurrence > 0
            && matches!(
                candidate.kind,
                TaskKind::Contract | TaskKind::Operation | TaskKind::Adapter
            )
            && self.task_pin.requirements.get(&id) == Some(&candidate.requirements_hash)
            && self.task_pin.profile_bundles.get(&id) == Some(&task.profile_bundle)
            && task.requirements_hash == candidate.requirements_hash
            && !task.profile_bundle.is_empty()
            && task.state_version >= 0
            && !task.subjects.is_empty()
            && task.subjects == candidate.subjects.iter().cloned().collect()
            && task.types == candidate.types.iter().cloned().collect()
            && task.facts == candidate.facts.iter().cloned().collect()
            && task.fact_reviews.len() == facts_count(&self.facts, &task.fact_reviews)
            && facts_valid
            && reviewed == task.facts
            && !task.implementation_finding.trim().is_empty()
            && !task.profile_finding.trim().is_empty()
            && match (&candidate.kind, &task.adapter_binding_finding) {
                (TaskKind::Adapter, Some(finding)) => !finding.trim().is_empty(),
                (TaskKind::Adapter, None) => false,
                (_, None) => true,
                (_, Some(_)) => false,
            }
            && sources_match(&task.sources)
    }
}

fn facts_count(catalog: &BTreeMap<String, SharedFactInspection>, keys: &BTreeSet<String>) -> usize {
    keys.iter().filter(|key| catalog.contains_key(*key)).count()
}
