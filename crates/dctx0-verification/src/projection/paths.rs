//! Mandatory transition coverage with shortest prerequisite paths.
//! A loop is exercised as a transition, rather than silently removed by BFS.
use super::{LifecyclePath, LifecyclePlan, issue};
use std::collections::{BTreeMap, VecDeque};

pub(super) fn plan_paths(plan: &mut LifecyclePlan, limit: usize) {
    let initials = plan.states.iter().filter(|s| s.initial).collect::<Vec<_>>();
    if initials.len() != 1 {
        return;
    }
    let initial = initials[0].name.clone();
    let mut prefixes = BTreeMap::from([(initial.clone(), Vec::<usize>::new())]);
    let mut queue = VecDeque::from([initial.clone()]);
    while let Some(state) = queue.pop_front() {
        let prefix = prefixes[&state].clone();
        for (index, transition) in plan.transitions.iter().enumerate() {
            if transition.from == state && !prefixes.contains_key(&transition.to) {
                let mut path = prefix.clone();
                path.push(index);
                prefixes.insert(transition.to.clone(), path);
                queue.push_back(transition.to.clone());
            }
        }
    }
    for (index, transition) in plan.transitions.iter().enumerate() {
        let valid_states = [&transition.from, &transition.to]
            .into_iter()
            .all(|name| plan.states.iter().any(|s| &s.name == name));
        if !valid_states {
            plan.issues.push(issue(
                None,
                "UNKNOWN_LIFECYCLE_STATE",
                &format!(
                    "Transition {} references an undeclared state",
                    transition.source_id
                ),
            ));
            continue;
        }
        let Some(prefix) = prefixes.get(&transition.from) else {
            plan.issues.push(issue(
                None,
                "UNREACHABLE_TRANSITION",
                &format!(
                    "Transition {} is unreachable from {initial}",
                    transition.source_id
                ),
            ));
            continue;
        };
        let mut path = prefix.clone();
        path.push(index);
        if path.len() > limit {
            plan.issues.push(issue(
                None,
                "LIFECYCLE_PATH_BUDGET",
                &format!(
                    "Mandatory transition {} needs {} steps; limit is {limit}",
                    transition.source_id,
                    path.len()
                ),
            ));
            continue;
        }
        plan.paths.push(LifecyclePath {
            id: format!("{}:transition:{index}", plan.subject),
            initial: initial.clone(),
            transitions: path,
            final_state: transition.to.clone(),
        });
    }
    for state in &plan.states {
        if !prefixes.contains_key(&state.name) {
            plan.issues.push(issue(
                None,
                "UNREACHABLE_STATE",
                &format!("State {} is unreachable from {initial}", state.name),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LifecycleState, LifecycleTransition};
    fn plan() -> LifecyclePlan {
        LifecyclePlan {
            subject: "Flow".into(),
            entity: "Item".into(),
            field: "status".into(),
            states: ["Start", "Middle", "Done", "Detached"]
                .into_iter()
                .map(|name| LifecycleState {
                    name: name.into(),
                    initial: name == "Start",
                    terminal: name == "Done",
                })
                .collect(),
            transitions: [
                ("Start", "Middle"),
                ("Middle", "Middle"),
                ("Middle", "Done"),
                ("Detached", "Done"),
                ("Done", "Missing"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (from, to))| LifecycleTransition {
                source_id: index.to_string(),
                operation: "advance".into(),
                from: from.into(),
                to: to.into(),
                guard: None,
            })
            .collect(),
            paths: Vec::new(),
            issues: Vec::new(),
        }
    }
    #[test]
    fn prerequisites_are_connected_loops_are_exercised_and_unreachable_work_is_reported() {
        let mut flow = plan();
        plan_paths(&mut flow, 8);
        assert_eq!(
            flow.paths
                .iter()
                .map(|p| p.transitions.clone())
                .collect::<Vec<_>>(),
            vec![vec![0], vec![0, 1], vec![0, 2]]
        );
        assert_eq!(flow.paths[2].final_state, "Done");
        for code in [
            "UNKNOWN_LIFECYCLE_STATE",
            "UNREACHABLE_TRANSITION",
            "UNREACHABLE_STATE",
        ] {
            assert!(flow.issues.iter().any(|i| i.code == code));
        }
    }
    #[test]
    fn budget_does_not_silently_drop_mandatory_transitions() {
        let mut flow = plan();
        plan_paths(&mut flow, 1);
        assert_eq!(flow.paths.len(), 1);
        assert_eq!(
            flow.issues
                .iter()
                .filter(|i| i.code == "LIFECYCLE_PATH_BUDGET")
                .count(),
            2
        );
        flow.states[0].initial = false;
        flow.paths.clear();
        plan_paths(&mut flow, 8);
        assert!(flow.paths.is_empty());
    }
}
