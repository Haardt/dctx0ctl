use super::*;
use dctx0_changes::{Change, ChangeFacet, ChangeKind, Snapshot};
use dctx0_graph::{GraphIndex, RequirementPath};
use dctx0_tasks::{Candidate, Plan};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalTaskImpact {
    pub task_id: i64,
    pub historical_member: bool,
    pub current_member: bool,
    pub classification: TaskImpactClassification,
    pub causes: Vec<IncrementalTaskCause>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskImpactClassification {
    InheritedConstraintOnly,
    Independent,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalTaskCause {
    pub old: Option<String>,
    pub new: Option<String>,
    pub kind: ChangeKind,
    pub facets: Vec<ChangeFacet>,
    pub check_occurrence: i64,
    #[serde(flatten)]
    pub path: RequirementPath,
}

/// Carry named lineage into anonymous clauses. Match unchanged constraints
/// before pairing edits so inserting a clause does not rename its neighbors.
pub(super) fn node_lineage(
    before: &Snapshot,
    after: &Snapshot,
    mut lineage: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut current_constraints = BTreeMap::<_, BTreeSet<_>>::new();
    for (id, node) in &after.graph.nodes {
        if let Some((owner, _)) = id.split_once('#')
            && node.kind == StatementKind::Constraint
        {
            current_constraints.entry(owner).or_default().insert(id);
        }
    }
    let mut unmatched = BTreeMap::<String, Vec<&String>>::new();
    for (id, node) in &before.graph.nodes {
        let Some((owner, suffix)) = id.split_once('#') else {
            continue;
        };
        let Some(new_owner) = lineage.get(owner) else {
            continue;
        };
        let new = format!("{new_owner}#{suffix}");
        if node.kind == StatementKind::Constraint {
            let Some(available) = current_constraints.get_mut(new_owner.as_str()) else {
                continue;
            };
            let same = |candidate: &String| {
                before.fingerprints[id].behavior == after.fingerprints[candidate].behavior
            };
            let matched = available
                .get(&new)
                .copied()
                .filter(|id| same(id))
                .or_else(|| available.iter().copied().find(|id| same(id)));
            if let Some(matched) = matched {
                available.remove(matched);
                lineage.insert(id.clone(), matched.clone());
            } else {
                unmatched.entry(new_owner.clone()).or_default().push(id);
            }
        } else if after.fingerprints.contains_key(&new) {
            lineage.insert(id.clone(), new);
        }
    }
    for (owner, old_constraints) in unmatched {
        for (old, new) in old_constraints
            .into_iter()
            .zip(&current_constraints[owner.as_str()])
        {
            lineage.insert(old.clone(), (*new).clone());
        }
    }
    lineage
}

fn paths(
    graph: &GraphIndex<'_>,
    candidate: &Candidate,
    translate: impl Fn(&String) -> Option<String>,
) -> BTreeMap<String, Vec<RequirementPath>> {
    let roots = candidate
        .subjects
        .iter()
        .chain(&candidate.types)
        .filter_map(&translate)
        .collect();
    let mut paths = graph.requirement_paths(&roots);
    // Inherited facts are already reachable along their governing path.
    // Treat only otherwise unreachable pinned facts as explicit roots.
    let facts = candidate
        .facts
        .iter()
        .chain(&candidate.review_obligations)
        .filter_map(translate)
        .filter(|id| !paths.contains_key(id))
        .collect();
    for (id, additional) in graph.requirement_paths(&facts) {
        paths.entry(id).or_default().extend(additional);
    }
    paths
}

pub(super) fn compare(
    checks: [(i64, &Snapshot); 2],
    plans: [Option<&Plan>; 2],
    scopes: [&BTreeSet<String>; 2],
    selected_tasks: &BTreeSet<i64>,
    lineage: &BTreeMap<String, String>,
) -> Vec<IncrementalTaskImpact> {
    let [(old_check, before), (new_check, after)] = checks;
    if old_check == new_check || plans.iter().all(Option::is_none) {
        return vec![];
    }
    let before_facts = dctx0_changes::facts_without_constraints(&before.ir);
    let after_facts = dctx0_changes::facts_without_constraints(&after.ir);
    let mut changes =
        dctx0_changes::compare_fingerprints(&before.fingerprints, &after.fingerprints, |id| {
            lineage.get(id)
        });
    changes.retain_mut(|change| {
        change.facets.retain(|facet| match facet {
            ChangeFacet::Location => false,
            ChangeFacet::Facts => {
                before_facts.get(change.old.as_deref().unwrap_or_default())
                    != after_facts.get(change.new.as_deref().unwrap_or_default())
            }
            _ => true,
        });
        if change.kind == ChangeKind::Renamed
            && let (Some(old), Some(new)) = (&change.old, &change.new)
            && before.graph.nodes[old].kind == StatementKind::Constraint
            && let (Some((old_owner, _)), Some((new_owner, _))) =
                (old.split_once('#'), new.split_once('#'))
            && old_owner == new_owner
        {
            change.kind = ChangeKind::Changed;
        }
        change.kind != ChangeKind::Changed || !change.facets.is_empty()
    });
    if changes.is_empty() {
        return vec![];
    }

    let old_graph = before.graph.index();
    let new_graph = after.graph.index();
    let reverse: BTreeMap<_, _> = lineage.iter().map(|(old, new)| (new, old)).collect();
    let mut members = BTreeMap::<i64, [Option<&Candidate>; 2]>::new();
    for (side, plan) in plans.into_iter().enumerate() {
        for member in plan.into_iter().flat_map(|plan| &plan.members) {
            members.entry(member.task_id).or_default()[side] = Some(&member.candidate);
        }
    }
    let mut result = vec![];
    let selected_candidates: Vec<_> = selected_tasks
        .iter()
        .filter_map(|id| members.get(id))
        .flat_map(|candidates| {
            candidates
                .iter()
                .enumerate()
                .filter_map(|(side, candidate)| candidate.map(|candidate| (side, candidate)))
        })
        .collect();
    for (task_id, [old, new]) in members {
        if !selected_tasks.contains(&task_id)
            && ![(old, scopes[0]), (new, scopes[1])]
                .into_iter()
                .enumerate()
                .any(|(side, (candidate, scope))| {
                    candidate.is_some_and(|c| {
                        c.subjects.iter().any(|id| scope.contains(id))
                            || selected_candidates.iter().any(|&(other_side, selected)| {
                                match (side, other_side) {
                                    (0, 1) => matching_subjects(c, selected, lineage),
                                    (1, 0) => matching_subjects(selected, c, lineage),
                                    _ => false,
                                }
                            })
                    })
                })
        {
            continue;
        }
        let old_paths = if let Some(old) = old {
            paths(&old_graph, old, |id| Some(id.clone()))
        } else {
            paths(&old_graph, new.expect("task belongs to a plan"), |id| {
                reverse.get(id).map(|old| (*old).clone())
            })
        };
        let new_paths = if let Some(new) = new {
            paths(&new_graph, new, |id| Some(id.clone()))
        } else {
            paths(&new_graph, old.expect("task belongs to a plan"), |id| {
                lineage.get(id).cloned()
            })
        };
        let mut causes = vec![];
        for change in &changes {
            let mut explanations = [None, None];
            for (check, id, snapshot, paths) in [
                (old_check, &change.old, before, &old_paths),
                (new_check, &change.new, after, &new_paths),
            ] {
                let Some(id) = id else { continue };
                let is_constraint = snapshot.graph.nodes[id].kind == StatementKind::Constraint;
                for path in paths.get(id).into_iter().flatten() {
                    let inherited = is_constraint && path.inherited_constraint;
                    let slot = &mut explanations[usize::from(inherited)];
                    if slot.as_ref().is_none_or(|cause: &IncrementalTaskCause| {
                        cause.path.path.len() > path.path.len()
                    }) {
                        *slot = Some(cause(change, check, path, inherited));
                    }
                }
            }
            causes.extend(explanations.into_iter().flatten());
        }
        if !causes.is_empty() {
            let classification = match (
                causes.iter().any(|cause| cause.path.inherited_constraint),
                causes.iter().any(|cause| !cause.path.inherited_constraint),
            ) {
                (true, false) => TaskImpactClassification::InheritedConstraintOnly,
                (true, true) => TaskImpactClassification::Mixed,
                _ => TaskImpactClassification::Independent,
            };
            result.push(IncrementalTaskImpact {
                task_id,
                historical_member: old.is_some(),
                current_member: new.is_some(),
                classification,
                causes,
            });
        }
    }
    result
}

fn matching_subjects(old: &Candidate, new: &Candidate, lineage: &BTreeMap<String, String>) -> bool {
    old.kind == new.kind
        && old.subjects.len() == new.subjects.len()
        && old
            .subjects
            .iter()
            .all(|id| lineage.get(id).is_some_and(|id| new.subjects.contains(id)))
}

fn cause(
    change: &Change,
    check_occurrence: i64,
    path: &RequirementPath,
    inherited_constraint: bool,
) -> IncrementalTaskCause {
    IncrementalTaskCause {
        old: change.old.clone(),
        new: change.new.clone(),
        kind: change.kind.clone(),
        facets: change.facets.clone(),
        check_occurrence,
        path: RequirementPath {
            inherited_constraint,
            ..path.clone()
        },
    }
}
