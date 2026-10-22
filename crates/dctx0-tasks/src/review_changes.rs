//! Review renewal diagnostics do not grant approval or infer file-to-duty dependencies.
use crate::{ConformanceReview, ReviewReport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFileChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBindingChanges {
    pub comparable: bool,
    pub files: Vec<ReviewFileChange>,
    pub requirements_changed: bool,
    pub specification_changed: bool,
    pub snapshot_changed: bool,
    pub affected_subjects: BTreeSet<String>,
    pub reasons: Vec<String>,
}
/// Compare only the last judgement. Older approvals never override a later report.
pub fn compare_review_bindings(
    previous: Option<&ReviewReport>,
    current: &ConformanceReview,
) -> ReviewBindingChanges {
    let mut result = ReviewBindingChanges {
        comparable: false,
        files: vec![],
        requirements_changed: false,
        specification_changed: false,
        snapshot_changed: false,
        affected_subjects: current.subjects.keys().cloned().collect(),
        reasons: vec![],
    };
    let Some(previous) = previous else {
        result
            .reasons
            .push("No stored review; a substantive first review is required".into());
        return result;
    };
    let Some(binding) = &previous.conformance else {
        result
            .reasons
            .push("Latest review has no comparable source binding".into());
        return result;
    };
    result.comparable = true;
    let paths: BTreeSet<_> = binding
        .implementation_artifacts
        .keys()
        .chain(current.implementation_artifacts.keys())
        .collect();
    for path in paths {
        let before = binding.implementation_artifacts.get(path);
        let after = current.implementation_artifacts.get(path);
        if before != after {
            result.files.push(ReviewFileChange {
                path: path.clone(),
                before: before.cloned(),
                after: after.cloned(),
            });
        }
    }
    result.requirements_changed = binding.requirements_hash != current.requirements_hash
        || binding.subjects.keys().collect::<Vec<_>>()
            != current.subjects.keys().collect::<Vec<_>>();
    result.specification_changed = binding.spec_source_hash != current.spec_source_hash;
    result.snapshot_changed = binding.snapshot != current.snapshot;
    if !result.files.is_empty() {
        result.reasons.push("Application inventory changed".into());
    }
    if result.requirements_changed {
        result.reasons.push("Assigned requirements changed".into());
    }
    if result.specification_changed {
        result
            .reasons
            .push("Specification source hash changed".into());
    }
    if result.snapshot_changed {
        result.reasons.push("Published snapshot changed".into());
    }
    if previous.judgement != "approved"
        || previous
            .findings
            .iter()
            .any(|finding| finding.action_required)
    {
        result
            .reasons
            .push("Latest judgement is not an approval without actionable findings".into());
    }
    if result.reasons.is_empty() {
        result.affected_subjects.clear();
    } else {
        result
            .affected_subjects
            .extend(binding.subjects.keys().cloned());
    }
    result
}

/// Useful for explicit first-review templates, without inheriting historical text.
pub fn empty_review_subjects(
    subjects: impl IntoIterator<Item = String>,
) -> BTreeMap<String, crate::ReviewedRequirement> {
    subjects
        .into_iter()
        .map(|subject| {
            (
                subject,
                crate::ReviewedRequirement {
                    report: String::new(),
                    limitations: vec![],
                },
            )
        })
        .collect()
}
