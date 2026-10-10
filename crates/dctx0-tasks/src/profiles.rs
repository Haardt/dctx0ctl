//! Pure profile selection from pinned task requirements and semantic references.
use crate::*;
use dctx0_changes::{Snapshot, hash};
use dctx0_core::{
    ExecutionBundle, ProfileDocument, ProfileKey, ProfileRole, ReferenceKind, StatementKind as K,
    TargetManifest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSelection {
    pub bundle: ExecutionBundle,
    pub missing_files: Vec<String>,
}
pub fn select_profiles(
    snapshot: &Snapshot,
    candidate: &Candidate,
    manifest: &TargetManifest,
    documents: &[ProfileDocument],
) -> ProfileSelection {
    let mut selected = BTreeMap::<ProfileKey, BTreeSet<String>>::new();
    let mut add = |subject: &str, kind: K, role: ProfileRole| {
        selected
            .entry(ProfileKey {
                role,
                semantic: kind.as_str().into(),
            })
            .or_default()
            .insert(subject.into());
    };
    for subject in &candidate.subjects {
        if let Some(symbol) = snapshot.ir.symbols.get(subject) {
            if !matches!(symbol.kind, K::Workspace | K::Domain) {
                add(subject, symbol.kind, ProfileRole::Implement);
            }
            if symbol.kind == K::Operation {
                for parent in symbol
                    .parent
                    .iter()
                    .filter_map(|p| snapshot.ir.symbols.get(p))
                    .filter(|p| matches!(p.kind, K::Port | K::Surface))
                {
                    add(&parent.id, parent.kind, ProfileRole::Use);
                }
            }
        }
    }
    for ty in &candidate.types {
        if !candidate.subjects.contains(ty) {
            if let Some(symbol) = snapshot.ir.symbols.get(ty) {
                add(ty, symbol.kind, ProfileRole::Use);
            }
        }
    }
    let nodes = snapshot
        .ir
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    for node in candidate
        .facts
        .iter()
        .filter_map(|fact| nodes.get(fact.as_str()))
    {
        if !candidate.subjects.contains(&node.id) {
            add(&node.id, node.kind, ProfileRole::Use);
        }
        if let Some(owner) = snapshot.ir.symbols.get(&node.owner).filter(|owner| {
            !candidate.subjects.contains(&owner.id)
                && matches!(
                    owner.kind,
                    K::Example | K::Invariant | K::Lifecycle | K::Proof | K::Test | K::Review
                )
        }) {
            add(&owner.id, owner.kind, ProfileRole::Use);
        }
    }
    for symbol in snapshot
        .ir
        .references
        .iter()
        .filter(|reference| {
            reference.kind == ReferenceKind::Calls
                && (candidate.subjects.contains(&reference.from)
                    || candidate.facts.contains(&reference.from))
        })
        .filter_map(|reference| snapshot.ir.symbols.get(&reference.target))
    {
        add(&symbol.id, symbol.kind, ProfileRole::Use);
        for parent in symbol
            .parent
            .iter()
            .filter_map(|p| snapshot.ir.symbols.get(p))
            .filter(|p| matches!(p.kind, K::Port | K::Surface))
        {
            add(&parent.id, parent.kind, ProfileRole::Use);
        }
    }
    let mut used = vec![];
    let mut missing_files = vec![];
    let base = format!(".dctx0/profiles/{}", manifest.target);
    if let Some(common) = documents
        .iter()
        .find(|d| d.key.is_none() && d.target == manifest.target)
    {
        let mut common = common.clone();
        common.subjects = candidate.subjects.clone();
        used.push(common);
    } else {
        missing_files.push(format!("{base}/common.md"));
    }
    for (key, subjects) in selected {
        if let Some(document) = documents
            .iter()
            .find(|d| d.key.as_ref() == Some(&key) && d.target == manifest.target)
        {
            let mut document = document.clone();
            document.subjects = subjects.into_iter().collect();
            used.push(document);
        } else if manifest.standard_keys.contains(&key) {
            missing_files.push(format!("{base}/{}", key.path()));
        }
    }
    let bundle_hash = hash(&(1_u32, &manifest.target, &used));
    ProfileSelection {
        bundle: ExecutionBundle {
            target: manifest.target.clone(),
            documents: used,
            hash: bundle_hash,
        },
        missing_files,
    }
}
impl ProfileSelection {
    pub fn require_complete(&self) -> Result<()> {
        if self.missing_files.is_empty() {
            Ok(())
        } else {
            Err(Error::Invalid(format!(
                "Required implementation profiles missing: {}",
                self.missing_files.join(", ")
            )))
        }
    }
}

/// Explicit selection; legacy tasks are not implicitly classified as drift.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileUpdateSelection {
    pub tasks: Vec<i64>,
    pub domain: Option<String>,
    pub semantic: Option<String>,
    pub all_affected: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileFileChange {
    pub path: String,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileComparison {
    UnknownOrigin,
    Unavailable,
    Changed,
    Unchanged,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileChange {
    pub comparison: ProfileComparison,
    pub task_id: i64,
    pub old_target: Option<String>,
    pub old_bundle_hash: Option<String>,
    pub desired: ProfileSelection,
    pub changed_files: Vec<ProfileFileChange>,
    pub unknown_origin: bool,
    pub selected: bool,
    pub work: WorkState,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileUpdatePreview {
    pub check_run_id: i64,
    pub schema_version: u32,
    pub plan_id: i64,
    pub snapshot_id: String,
    pub event_seq: i64,
    pub selection: ProfileUpdateSelection,
    pub changes: Vec<ProfileChange>,
    pub selected_tasks: Vec<i64>,
    pub revised_assemblies: Vec<i64>,
    pub possible_impacts: Vec<i64>,
    pub hash: String,
}
pub fn profile_changes(
    session: &Session,
    snapshot: &Snapshot,
    manifest: &TargetManifest,
    documents: &[ProfileDocument],
    selection: ProfileUpdateSelection,
) -> Result<ProfileUpdatePreview> {
    profile_changes_at_check(
        session,
        snapshot,
        manifest,
        documents,
        selection,
        session.plan.check_run_id,
    )
}
pub fn profile_changes_at_check(
    session: &Session,
    snapshot: &Snapshot,
    manifest: &TargetManifest,
    documents: &[ProfileDocument],
    selection: ProfileUpdateSelection,
    check_run_id: i64,
) -> Result<ProfileUpdatePreview> {
    let filtered = selection.domain.is_some() || selection.semantic.is_some();
    if usize::from(!selection.tasks.is_empty())
        + usize::from(filtered)
        + usize::from(selection.all_affected)
        > 1
    {
        return Err(Error::Invalid(
            "Choose task IDs, domain/semantic filters, or --all-affected".into(),
        ));
    }
    for id in &selection.tasks {
        session.member(*id)?;
    }
    let mut changes = vec![];
    let mut selected_tasks = vec![];
    for member in &session.plan.members {
        let id = member.task_id;
        let state = &session.states[&id];
        let desired = select_profiles(snapshot, &member.candidate, manifest, documents);
        let old = state.execution_bundle.as_ref();
        let unknown_origin = old.is_none();
        let drift =
            desired.missing_files.is_empty() && old.is_some_and(|b| b.hash != desired.bundle.hash);
        let comparison = if unknown_origin {
            ProfileComparison::UnknownOrigin
        } else if !desired.missing_files.is_empty() {
            ProfileComparison::Unavailable
        } else if drift {
            ProfileComparison::Changed
        } else {
            ProfileComparison::Unchanged
        };
        let selected = selection.tasks.contains(&id)
            || (selection.all_affected && drift && state.work == WorkState::Done)
            || (filtered
                && state.work == WorkState::Done
                && selection.domain.as_ref().is_none_or(|domain| {
                    member.candidate.subjects.iter().any(|s| {
                        s == domain
                            || s.starts_with(&format!("{domain}."))
                            || s.split('.').any(|part| part == domain)
                    })
                })
                && selection.semantic.as_ref().is_none_or(|semantic| {
                    member.candidate.subjects.iter().any(|s| {
                        snapshot
                            .ir
                            .symbols
                            .get(s)
                            .is_some_and(|s| s.kind.as_str() == semantic)
                    })
                }));
        if selected && state.work != WorkState::Done {
            return Err(Error::Invalid(format!(
                "Profile adaptation requires completed work: T{id}"
            )));
        }
        if selected {
            desired.require_complete()?;
            if drift || unknown_origin {
                selected_tasks.push(id);
            }
        }
        let mut files = BTreeMap::<String, (Option<String>, Option<String>)>::new();
        for doc in old.into_iter().flat_map(|b| &b.documents) {
            files.entry(doc.path.clone()).or_default().0 = Some(doc.content_hash.clone());
        }
        for doc in &desired.bundle.documents {
            files.entry(doc.path.clone()).or_default().1 = Some(doc.content_hash.clone());
        }
        let changed_files = files
            .into_iter()
            .filter(|(_, (a, b))| a != b)
            .map(|(path, (before_hash, after_hash))| ProfileFileChange {
                path,
                before_hash,
                after_hash,
            })
            .collect();
        changes.push(ProfileChange {
            comparison,
            task_id: id,
            old_target: old.map(|b| b.target.clone()),
            old_bundle_hash: old.map(|b| b.hash.clone()),
            desired,
            changed_files,
            unknown_origin,
            selected: selected_tasks.contains(&id),
            work: state.work,
        });
    }
    let mut changed_keys = session
        .plan
        .members
        .iter()
        .filter(|m| selected_tasks.contains(&m.task_id))
        .map(|m| m.candidate.key.clone())
        .collect::<BTreeSet<_>>();
    let mut revised_assemblies = vec![];
    let mut ordered = session.plan.members.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|m| (m.candidate.wave, m.task_id));
    for member in ordered {
        if !selected_tasks.contains(&member.task_id)
            && member
                .candidate
                .prerequisites
                .iter()
                .any(|p| changed_keys.contains(&p.key))
        {
            changed_keys.insert(member.candidate.key.clone());
            if matches!(
                member.candidate.kind,
                TaskKind::Integration | TaskKind::Review
            ) {
                revised_assemblies.push(member.task_id);
            }
        }
    }
    let possible_impacts = session
        .plan
        .members
        .iter()
        .filter(|m| {
            !selected_tasks.contains(&m.task_id)
                && !revised_assemblies.contains(&m.task_id)
                && m.candidate
                    .prerequisites
                    .iter()
                    .any(|p| changed_keys.contains(&p.key))
        })
        .map(|m| m.task_id)
        .collect();
    let mut preview = ProfileUpdatePreview {
        check_run_id,
        schema_version: 2,
        plan_id: session.plan.id,
        snapshot_id: snapshot.id.clone(),
        event_seq: session.event_seq,
        selection,
        changes,
        selected_tasks,
        revised_assemblies,
        possible_impacts,
        hash: String::new(),
    };
    preview.hash = profile_preview_hash(&preview);
    Ok(preview)
}

/// Only selected revisions participate in the write guard; other profiles are informational.
pub fn profile_preview_hash(preview: &ProfileUpdatePreview) -> String {
    let bundles = preview
        .changes
        .iter()
        .filter(|c| {
            preview.selected_tasks.contains(&c.task_id)
                || preview.revised_assemblies.contains(&c.task_id)
        })
        .map(|c| {
            (
                c.task_id,
                &c.old_bundle_hash,
                &c.desired.bundle.hash,
                &c.desired.missing_files,
            )
        })
        .collect::<Vec<_>>();
    hash(&(
        preview.schema_version,
        preview.check_run_id,
        preview.plan_id,
        &preview.snapshot_id,
        preview.event_seq,
        &preview.selection,
        &preview.selected_tasks,
        &preview.revised_assemblies,
        bundles,
    ))
}
