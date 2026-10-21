//! Portable, read-only work handoffs. Content references deduplicate binding context.
use crate::*;
use dctx0_changes::{Snapshot, hash};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkPin {
    pub project: String,
    pub plan_id: i64,
    pub snapshot_id: String,
    pub check_occurrence: i64,
    pub event_seq: i64,
}
impl WorkPin {
    pub fn of(session: &Session) -> Self {
        Self {
            project: session.project.clone(),
            plan_id: session.plan.id,
            snapshot_id: session.plan.snapshot_id.clone(),
            check_occurrence: session.plan.check_run_id,
            event_seq: session.event_seq,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageTask {
    pub overview: Overview,
    pub requirements_hash: String,
    pub work_order_hash: String,
    pub context: Vec<String>,
    /// Per-task subject attribution for shared profile bodies.
    pub profile_subjects: BTreeMap<String, Vec<String>>,
    pub implementation: Option<ImplementationContext>,
    pub result: Option<TaskResult>,
    /// Compact evidence references; full reports remain in history.
    pub evidence: Vec<String>,
    /// Includes source bindings and authored review metadata even if a report ID is reused.
    pub evidence_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkPackage {
    pub schema_version: u32,
    pub id: String,
    pub pin: WorkPin,
    pub spec_source_hash: String,
    pub slice: Slice,
    pub tasks: Vec<PackageTask>,
    pub content: BTreeMap<String, ContextItem>,
    pub complete: bool,
    pub limitations: Vec<String>,
}
impl WorkPackage {
    fn digest(&self) -> String {
        hash(&(
            self.schema_version,
            &self.pin,
            &self.spec_source_hash,
            &self.slice,
            &self.tasks,
            &self.content,
            self.complete,
            &self.limitations,
        ))
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1
            || self.id != self.digest()
            || self
                .tasks
                .iter()
                .any(|task| task.context.iter().any(|id| !self.content.contains_key(id)))
            || self.tasks.iter().any(|task| {
                task.profile_subjects.keys().any(|id| {
                    !task.context.contains(id)
                        || self
                            .content
                            .get(id)
                            .is_none_or(|item| item.profile.is_none())
                })
            })
            || self.content.iter().any(|(id, item)| *id != hash(item))
        {
            return Err(Error::Invalid(
                "Invalid work package identity or context references".into(),
            ));
        }
        Ok(())
    }
}
/// All core pages are materialized once; mandatory text is never truncated.
/// Profile selections must be captured by the I/O adapter before calling this function.
pub fn work_package(
    session: &Session,
    snapshot: &Snapshot,
    anchor: i64,
    domain: Option<&str>,
    actor: Option<&str>,
    profiles: &BTreeMap<i64, ProfileSelection>,
) -> Result<WorkPackage> {
    let slice = session.slice(anchor, domain, actor)?;
    let indexes = crate::context::ContextIndexes::new(session, snapshot);
    let mut content = BTreeMap::new();
    let mut tasks = Vec::new();
    let mut limitations = BTreeSet::new();
    for overview in &slice.tasks {
        let id = overview.task_id;
        let context = crate::context::context_indexed(
            session,
            snapshot,
            id,
            ("core", usize::MAX, None),
            profiles.get(&id).cloned(),
            &indexes,
        )?;
        if !context.page.complete || !context.missing_sections.is_empty() {
            limitations.insert(format!("T{id}: mandatory context is incomplete"));
        }
        if let Some(implementation) = &context.implementation {
            for path in &implementation.missing_files {
                limitations.insert(format!("T{id}: required profile missing: {path}"));
            }
        } else {
            limitations.insert(format!("T{id}: implementation profile context unavailable"));
        }
        let mut profile_subjects = BTreeMap::new();
        let refs = context
            .page
            .items
            .into_iter()
            .map(|mut item| {
                // A shared type and its own task contract retain one exact declaration.
                if item.type_definition.is_some() {
                    item.section = "specification".into();
                }
                let subjects = item.profile.as_mut().map(|profile| {
                    item.title = format!("Profile: {}", profile.path);
                    std::mem::take(&mut profile.subjects)
                });
                let key = hash(&item);
                if let Some(subjects) = subjects {
                    profile_subjects.insert(key.clone(), subjects);
                }
                content.entry(key.clone()).or_insert(item);
                key
            })
            .collect();
        let candidate = &session.member(id)?.candidate;
        let state = &session.states[&id];
        tasks.push(PackageTask {
            overview: overview.clone(),
            requirements_hash: candidate.requirements_hash.clone(),
            work_order_hash: candidate.work_order_hash.clone(),
            context: refs,
            profile_subjects,
            implementation: context.implementation,
            result: state.result.clone(),
            evidence_hash: hash(&(&state.evidence, &state.reviews)),
            evidence: state
                .evidence
                .iter()
                .map(|e| e.report.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        });
    }
    let mut package = WorkPackage {
        schema_version: 1,
        id: String::new(),
        pin: WorkPin::of(session),
        spec_source_hash: snapshot.source_hash.clone(),
        slice,
        tasks,
        content,
        complete: limitations.is_empty(),
        limitations: limitations.into_iter().collect(),
    };
    package.id = package.digest();
    Ok(package)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageTaskChange {
    pub task_id: i64,
    pub requirements_changed: bool,
    pub assignment_changed: bool,
    pub prerequisites_changed: bool,
    pub profiles_changed: bool,
    pub state_changed: bool,
    pub evidence_changed: bool,
    pub reload_context: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDelta {
    pub before: String,
    pub after: String,
    pub before_pin: WorkPin,
    pub after_pin: WorkPin,
    pub spec_sources_changed: bool,
    pub after_spec_source_hash: String,
    pub after_tasks: Vec<PackageTask>,
    pub after_slice: Slice,
    pub after_complete: bool,
    pub after_limitations: Vec<String>,
    pub added_tasks: Vec<i64>,
    pub removed_tasks: Vec<i64>,
    pub changed_tasks: Vec<PackageTaskChange>,
    pub added_content: BTreeMap<String, ContextItem>,
    pub removed_content: Vec<String>,
    pub history_links: Vec<HistoryLink>,
}
pub fn package_delta(
    before: &WorkPackage,
    after: &WorkPackage,
    links: &[HistoryLink],
) -> Result<PackageDelta> {
    before.validate()?;
    after.validate()?;
    if before.pin.project != after.pin.project || before.pin.event_seq > after.pin.event_seq {
        return Err(Error::Invalid(
            "Package comparison requires one project and ordered event pins".into(),
        ));
    }
    let a = before
        .tasks
        .iter()
        .map(|t| (t.overview.task_id, t))
        .collect::<BTreeMap<_, _>>();
    let b = after
        .tasks
        .iter()
        .map(|t| (t.overview.task_id, t))
        .collect::<BTreeMap<_, _>>();
    let mut changed_tasks = Vec::new();
    for (id, next) in &b {
        if let Some(old) = a.get(id) {
            let dependencies = |p: &WorkPackage| {
                hash(
                    &p.slice
                        .dependencies
                        .iter()
                        .filter(|d| d.task_id == *id)
                        .collect::<Vec<_>>(),
                )
            };
            let profiles_changed = hash(&(&old.implementation, &old.profile_subjects))
                != hash(&(&next.implementation, &next.profile_subjects));
            let requirements_changed = old.requirements_hash != next.requirements_hash;
            let assignment_changed = old.work_order_hash != next.work_order_hash;
            let prerequisites_changed = dependencies(before) != dependencies(after);
            let state_changed =
                hash(&old.overview) != hash(&next.overview) || old.result != next.result;
            let evidence_changed = old.evidence_hash != next.evidence_hash;
            let reload_context = old.context != next.context
                || requirements_changed
                || assignment_changed
                || prerequisites_changed
                || profiles_changed;
            if reload_context || state_changed || evidence_changed {
                changed_tasks.push(PackageTaskChange {
                    task_id: *id,
                    requirements_changed,
                    assignment_changed,
                    prerequisites_changed,
                    profiles_changed,
                    state_changed,
                    evidence_changed,
                    reload_context,
                });
            }
        }
    }
    Ok(PackageDelta {
        before: before.id.clone(),
        after: after.id.clone(),
        before_pin: before.pin.clone(),
        after_pin: after.pin.clone(),
        spec_sources_changed: before.spec_source_hash != after.spec_source_hash,
        after_spec_source_hash: after.spec_source_hash.clone(),
        after_tasks: after.tasks.clone(),
        after_slice: after.slice.clone(),
        after_complete: after.complete,
        after_limitations: after.limitations.clone(),
        added_tasks: b.keys().filter(|id| !a.contains_key(id)).copied().collect(),
        removed_tasks: a.keys().filter(|id| !b.contains_key(id)).copied().collect(),
        changed_tasks,
        added_content: after
            .content
            .iter()
            .filter(|(key, _)| !before.content.contains_key(*key))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        removed_content: before
            .content
            .keys()
            .filter(|key| !after.content.contains_key(*key))
            .cloned()
            .collect(),
        history_links: links
            .iter()
            .filter(|l| a.contains_key(&l.from) && b.contains_key(&l.to))
            .cloned()
            .collect(),
    })
}
