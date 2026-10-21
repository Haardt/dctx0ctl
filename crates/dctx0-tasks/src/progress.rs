//! Endpoint transitions and the exact intervening events, without inferred feature yield.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressReport {
    pub before: WorkPin,
    pub after: WorkPin,
    pub added: Vec<i64>,
    pub removed: Vec<i64>,
    pub added_done: Vec<i64>,
    pub added_pass: Vec<i64>,
    pub newly_done: Vec<i64>,
    pub newly_pass: Vec<i64>,
    pub lost_pass: Vec<i64>,
    pub unfinished: Vec<i64>,
    pub blocked: Vec<i64>,
    pub events: Vec<Event>,
    pub history_links: Vec<HistoryLink>,
}
pub fn task_progress(before: &Session, after: &Session) -> Result<ProgressReport> {
    if before.project != after.project || before.event_seq > after.event_seq {
        return Err(Error::Invalid(
            "Progress requires one project and ordered event pins".into(),
        ));
    }
    let a = before
        .plan
        .members
        .iter()
        .map(|m| m.task_id)
        .collect::<BTreeSet<_>>();
    let b = after
        .plan
        .members
        .iter()
        .map(|m| m.task_id)
        .collect::<BTreeSet<_>>();
    // Only work present at both pins has a measured transition. New work is
    // reported separately, even when created and completed within this interval.
    let common = a.intersection(&b).copied().collect::<Vec<_>>();
    let rows = after.overviews();
    Ok(ProgressReport {
        before: WorkPin::of(before),
        after: WorkPin::of(after),
        added: b.difference(&a).copied().collect(),
        removed: a.difference(&b).copied().collect(),
        added_done: b
            .difference(&a)
            .filter(|id| after.states[id].work == WorkState::Done)
            .copied()
            .collect(),
        added_pass: b
            .difference(&a)
            .filter(|id| after.states[id].check == CheckStatus::Pass)
            .copied()
            .collect(),
        newly_done: common
            .iter()
            .filter(|id| {
                before.states[id].work != WorkState::Done
                    && after.states[id].work == WorkState::Done
            })
            .copied()
            .collect(),
        newly_pass: common
            .iter()
            .filter(|id| {
                before.states[id].check != CheckStatus::Pass
                    && after.states[id].check == CheckStatus::Pass
            })
            .copied()
            .collect(),
        lost_pass: a
            .iter()
            .filter(|id| {
                before.states[id].check == CheckStatus::Pass
                    && (!b.contains(id) || after.states[id].check != CheckStatus::Pass)
            })
            .copied()
            .collect(),
        unfinished: rows
            .iter()
            .filter(|r| r.work != WorkState::Done)
            .map(|r| r.task_id)
            .collect(),
        blocked: rows
            .iter()
            .filter(|r| r.work != WorkState::Done && !r.blockers.is_empty())
            .map(|r| r.task_id)
            .collect(),
        events: after
            .events
            .iter()
            .filter(|e| e.seq > before.event_seq)
            .cloned()
            .collect(),
        history_links: after
            .links
            .iter()
            .filter(|l| !before.links.contains(l))
            .cloned()
            .collect(),
    })
}
