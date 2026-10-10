//! Reconcile only with current work; historical equal hashes are not reusable.
use crate::*;
use dctx0_changes::hash;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct Proposed {
    pub candidate: Candidate,
    pub reuse: Option<i64>,
    pub predecessors: Vec<(i64, String)>,
    pub reason: String,
}
pub fn reconcile(
    old: Option<&Session>,
    candidate: &CandidatePlan,
    continuity: &BTreeMap<String, String>,
) -> Result<Vec<Proposed>> {
    if candidate
        .tasks
        .iter()
        .map(|t| &t.key)
        .collect::<BTreeSet<_>>()
        .len()
        != candidate.tasks.len()
    {
        return Err(Error::Invalid("Duplicate task producer keys".into()));
    }
    let mut proposed = Vec::new();
    let mut matched = BTreeSet::new();
    for task in &candidate.tasks {
        let previous = old
            .into_iter()
            .flat_map(|s| &s.plan.members)
            .filter(|m| {
                m.candidate.kind == task.kind
                    && m.candidate.subjects.iter().any(|subject| {
                        continuity
                            .get(subject)
                            .is_some_and(|current| task.subjects.contains(current))
                    })
            })
            .collect::<Vec<_>>();
        let reuse = previous
            .iter()
            .find(|m| {
                m.candidate.key == task.key
                    && m.candidate.subjects == task.subjects
                    && m.candidate.requirements_hash == task.requirements_hash
                    && (m.candidate.work_order_hash == task.work_order_hash
                        || old.unwrap().definitions[&m.task_id].is_adjustment())
            })
            .map(|m| m.task_id);
        for m in &previous {
            matched.insert(m.task_id);
        }
        let predecessors = if reuse.is_some() {
            vec![]
        } else {
            previous
                .iter()
                .map(|m| {
                    let kind = if previous.len() > 1 {
                        "merge"
                    } else if m.candidate.subjects.len() > task.subjects.len() {
                        "split"
                    } else if m.candidate.subjects != task.subjects {
                        "rename"
                    } else {
                        "update"
                    };
                    (m.task_id, kind.into())
                })
                .collect()
        };
        let reason = if reuse.is_some() {
            "Unchanged requirements".into()
        } else if predecessors.is_empty() {
            "Implement this specification requirement".into()
        } else {
            format!(
                "Adapt the existing implementation: requirements or consumed contracts changed for {}",
                task.subjects.join(", ")
            )
        };
        let mut current = task.clone();
        if let Some(id) = reuse {
            let prev = &old.unwrap().definitions[&id];
            if prev.is_adjustment() {
                current.work_order_hash = prev.candidate.work_order_hash.clone();
            }
        }
        proposed.push(Proposed {
            candidate: current,
            reuse,
            predecessors,
            reason,
        });
    }
    if let Some(old) = old {
        for member in &old.plan.members {
            if member.candidate.kind == TaskKind::Retire {
                // Outstanding retirement work survives repeated planning.
                if !candidate.tasks.iter().any(|t| {
                    t.subjects
                        .iter()
                        .any(|s| member.candidate.subjects.contains(s))
                }) {
                    proposed.push(Proposed {
                        candidate: member.candidate.clone(),
                        reuse: Some(member.task_id),
                        predecessors: vec![],
                        reason: "Preserve retirement work".into(),
                    });
                }
                continue;
            }
            if matched.contains(&member.task_id)
                || !old.states[&member.task_id].ever_started
                || candidate
                    .tasks
                    .iter()
                    .any(|t| t.kind == member.candidate.kind && t.key == member.candidate.key)
            {
                continue;
            }
            let mut retire = member.candidate.clone();
            retire.kind = TaskKind::Retire;
            retire.key = format!("retire:{}", member.candidate.key);
            retire.title = format!("Retire: {}", member.candidate.title);
            retire.prerequisites.clear();
            retire.facts.clear();
            retire.types.clear();
            retire.requirements_hash = hash(&("retire", &member.candidate.requirements_hash));
            retire.work_order_hash = retire.requirements_hash.clone();
            retire.acceptance = TaskKind::Retire.acceptance();
            proposed.push(Proposed {
                candidate: retire,
                reuse: None,
                predecessors: vec![(member.task_id, "retire".into())],
                reason: format!(
                    "Remove the started or delivered implementation of {}",
                    member.candidate.subjects.join(", ")
                ),
            });
        }
        // Retiring a provider waits for changed/retired consumers from the old DAG.
        let successor = proposed
            .iter()
            .flat_map(|p| {
                p.predecessors
                    .iter()
                    .map(|(id, _)| (*id, p.candidate.key.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        for p in &mut proposed {
            if p.candidate.kind == TaskKind::Retire {
                let old_id = p.predecessors.first().map(|x| x.0).or(p
                    .reuse
                    .and_then(|id| old.links.iter().find(|l| l.to == id).map(|l| l.from)));
                if let Some(id) = old_id {
                    for edge in old
                        .dependencies()
                        .into_iter()
                        .filter(|e| e.prerequisite_id == id)
                    {
                        if let Some(key) = successor.get(&edge.task_id) {
                            if key != &p.candidate.key {
                                p.candidate.prerequisites.push(Prerequisite{key:key.clone(),condition:Condition::Delivered,reason:"Adapt or remove the former consumer before retiring its provider".into()});
                            }
                        }
                    }
                }
            }
        }
    }
    let mut tasks = proposed
        .iter()
        .map(|p| (p.candidate.key.clone(), p.candidate.clone()))
        .collect::<BTreeMap<_, _>>();
    assign_waves(&mut tasks)?;
    for p in &mut proposed {
        p.candidate.wave = tasks[&p.candidate.key].wave;
    }
    // Propagate work-order revisions through assemblies without reopening history.
    let mut order = (0..proposed.len()).collect::<Vec<_>>();
    order.sort_by_key(|i| proposed[*i].candidate.wave);
    for i in order {
        if !matches!(
            proposed[i].candidate.kind,
            TaskKind::Integration | TaskKind::Review
        ) {
            continue;
        }
        if proposed[i]
            .reuse
            .is_some_and(|id| old.is_some_and(|s| s.definitions[&id].is_adjustment()))
        {
            continue;
        }
        let inputs = proposed[i]
            .candidate
            .prerequisites
            .iter()
            .map(|d| {
                (
                    &d.key,
                    &proposed
                        .iter()
                        .find(|p| p.candidate.key == d.key)
                        .unwrap()
                        .candidate
                        .work_order_hash,
                )
            })
            .collect::<Vec<_>>();
        let work = hash(&(&proposed[i].candidate.requirements_hash, inputs));
        proposed[i].candidate.work_order_hash = work.clone();
        if let Some(old) = old {
            if let Some(previous) = old.plan.members.iter().find(|m| {
                matched.contains(&m.task_id)
                    && m.candidate.key == proposed[i].candidate.key
                    && m.candidate.requirements_hash == proposed[i].candidate.requirements_hash
            }) {
                let same_producers = proposed[i].candidate.prerequisites.iter().all(|input| {
                    let current = proposed
                        .iter()
                        .find(|p| p.candidate.key == input.key)
                        .unwrap();
                    old.plan
                        .members
                        .iter()
                        .find(|m| m.candidate.key == input.key)
                        .is_some_and(|producer| current.reuse == Some(producer.task_id))
                });
                if previous.candidate.work_order_hash == work && same_producers {
                    proposed[i].reuse = Some(previous.task_id);
                    proposed[i].predecessors.clear();
                } else {
                    proposed[i].reuse = None;
                    proposed[i].predecessors = vec![(previous.task_id, "update".into())];
                    proposed[i].reason =
                        "Reintegrate or review changed specification or requested remediation"
                            .into();
                }
            }
        }
    }
    Ok(proposed)
}
