use crate::*;
use dctx0_changes::hash;
use std::collections::{BTreeMap, BTreeSet};

impl Session {
    pub fn member(&self, id: i64) -> Result<&Member> {
        self.plan
            .members
            .iter()
            .find(|m| m.task_id == id)
            .ok_or(Error::Missing(id))
    }
    pub fn dependencies(&self) -> Vec<DependencyView> {
        let ids = self
            .plan
            .members
            .iter()
            .map(|m| (&m.candidate.key, m.task_id))
            .collect::<BTreeMap<_, _>>();
        self.plan
            .members
            .iter()
            .flat_map(|m| {
                m.candidate.prerequisites.iter().map(|p| DependencyView {
                    task_id: m.task_id,
                    prerequisite_id: ids[&p.key],
                    condition: p.condition.clone(),
                    reason: p.reason.clone(),
                })
            })
            .collect()
    }
    pub fn overviews(&self) -> Vec<Overview> {
        let ids = self
            .plan
            .members
            .iter()
            .map(|m| (&m.candidate.key, m.task_id))
            .collect::<BTreeMap<_, _>>();
        let active = self
            .plan
            .members
            .iter()
            .map(|m| m.task_id)
            .collect::<BTreeSet<_>>();
        let superseded = self
            .definitions
            .iter()
            .filter(|(id, _)| !active.contains(id) && self.states[id].work == WorkState::Progress)
            .collect::<Vec<_>>();
        let mut delivered = BTreeMap::<i64, bool>::new();
        let mut rows = Vec::new();
        let mut ordered = self.plan.members.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|m| (m.candidate.wave, &m.candidate.key));
        for m in ordered {
            let state = &self.states[&m.task_id];
            let mut blockers = Vec::new();
            for p in &m.candidate.prerequisites {
                let id = ids[&p.key];
                if !delivered[&id]
                    || p.condition == Condition::CheckPass
                        && self.states[&id].check != CheckStatus::Pass
                {
                    blockers.push(format!("T{id}: {}", p.reason));
                }
            }
            for (id, definition) in &superseded {
                if self
                    .history_links(m.task_id, "ancestors")
                    .unwrap()
                    .iter()
                    .any(|l| l.from == **id)
                    || definition
                        .candidate
                        .subjects
                        .iter()
                        .any(|s| m.candidate.subjects.contains(s))
                {
                    blockers.push(format!(
                        "T{id}: superseded work still claimed by {}",
                        self.states[*id].actor.as_deref().unwrap_or("unknown")
                    ));
                }
            }
            let available =
                state.work == WorkState::Done && state.result.is_some() && blockers.is_empty();
            delivered.insert(m.task_id, available);
            rows.push(Overview {
                task_id: m.task_id,
                title: m.candidate.title.clone(),
                kind: m.candidate.kind,
                subjects: m.candidate.subjects.clone(),
                wave: m.candidate.wave,
                work: state.work,
                check: state.check,
                version: state.version,
                actor: state.actor.clone(),
                claim: state.claim,
                reservation: state.reservation,
                ready: state.work == WorkState::Open && blockers.is_empty(),
                delivered: available,
                blockers,
                result_summary: state.result.as_ref().map(|r| r.summary.clone()),
            });
        }
        rows
    }
    pub fn matches(row: &Overview, filters: &Filters) -> bool {
        filters.wave.is_none_or(|x| x == row.wave)
            && filters.state.is_none_or(|x| x == row.work)
            && filters.check.is_none_or(|x| x == row.check)
            && filters.kind.is_none_or(|x| x == row.kind)
            && filters.subject.as_ref().is_none_or(|s| {
                row.subjects
                    .iter()
                    .any(|x| x == s || x.starts_with(&format!("{s}.")))
            })
            && filters.domain.as_ref().is_none_or(|s| {
                row.subjects
                    .iter()
                    .any(|x| x == s || x.starts_with(&format!("{s}.")))
            })
            && filters
                .actor
                .as_ref()
                .is_none_or(|s| row.actor.as_ref() == Some(s))
    }
    pub fn next(&self, filters: &Filters, actor: Option<&str>) -> Option<Overview> {
        if filters.state.is_some_and(|state| state != WorkState::Open) {
            return None;
        }
        let mut filters = filters.clone();
        filters.state = Some(WorkState::Open);
        // Actor selection includes free work and one's own reserved work.
        if actor.is_some() {
            filters.actor = None;
        }
        self.overviews().into_iter().find(|row| {
            row.ready
                && Self::matches(row, &filters)
                && (row.actor.is_none() || row.actor.as_deref() == actor)
        })
    }
    pub fn list(
        &self,
        filters: &Filters,
        limit: usize,
        cursor: Option<&Cursor>,
    ) -> Result<Page<Overview>> {
        let query = hash(filters);
        let offset = validate_cursor(self, cursor, &query)?;
        let rows = self
            .overviews()
            .into_iter()
            .filter(|r| Self::matches(r, filters))
            .collect::<Vec<_>>();
        paginate(self, rows, limit, offset, query)
    }
    pub fn related(
        &self,
        id: i64,
        dependents: bool,
        transitive: bool,
    ) -> Result<Vec<DependencyView>> {
        self.member(id)?;
        let edges = self.dependencies();
        let mut seen = BTreeSet::from([id]);
        let mut frontier = vec![id];
        let mut selected = BTreeSet::new();
        while let Some(current) = frontier.pop() {
            for (i, edge) in edges.iter().enumerate() {
                let (from, to) = if dependents {
                    (edge.prerequisite_id, edge.task_id)
                } else {
                    (edge.task_id, edge.prerequisite_id)
                };
                if from == current {
                    selected.insert(i);
                    if transitive && seen.insert(to) {
                        frontier.push(to);
                    }
                }
            }
        }
        Ok(selected.into_iter().map(|i| edges[i].clone()).collect())
    }
    pub fn history_links(&self, id: i64, direction: &str) -> Result<Vec<HistoryLink>> {
        if !self.definitions.contains_key(&id) {
            return Err(Error::Missing(id));
        }
        if !matches!(direction, "ancestors" | "descendants" | "both") {
            return Err(Error::Invalid(
                "History direction must be ancestors, descendants or both".into(),
            ));
        }
        let mut found = vec![];
        let mut seen = BTreeSet::from([id]);
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            for link in &self.links {
                let next = if direction != "descendants" && link.to == current {
                    Some(link.from)
                } else if direction != "ancestors" && link.from == current {
                    Some(link.to)
                } else {
                    None
                };
                if let Some(next) = next {
                    if !found.contains(link) {
                        found.push(link.clone());
                    }
                    if seen.insert(next) {
                        pending.push(next);
                    }
                }
            }
        }
        found.sort_by_key(|l| (l.from, l.to));
        Ok(found)
    }
    pub fn slice(&self, anchor: i64, domain: Option<&str>, actor: Option<&str>) -> Result<Slice> {
        self.member(anchor)?;
        let edges = self.related(anchor, false, true)?;
        let mut ids = BTreeSet::from([anchor]);
        ids.extend(edges.iter().map(|e| e.prerequisite_id));
        let tasks = self
            .overviews()
            .into_iter()
            .filter(|r| ids.contains(&r.task_id))
            .collect::<Vec<_>>();
        let mut open_work = vec![];
        let mut boundary = vec![];
        let mut blockers = vec![];
        for row in &tasks {
            if row.delivered {
                boundary.push(row.task_id);
            } else if row.work != WorkState::Done {
                let external = domain.is_some_and(|d| {
                    !row.subjects
                        .iter()
                        .any(|s| s == d || s.starts_with(&format!("{d}.")))
                });
                if external {
                    blockers.push(format!(
                        "T{}: open prerequisite outside requested domain",
                        row.task_id
                    ));
                } else {
                    open_work.push(row.task_id);
                }
                if row.actor.as_deref().is_some_and(|a| Some(a) != actor) {
                    blockers.push(format!(
                        "T{}: claimed or reserved by {}",
                        row.task_id,
                        row.actor.as_deref().unwrap()
                    ));
                }
            }
            blockers.extend(
                row.blockers
                    .iter()
                    .filter(|b| b.contains("superseded work"))
                    .cloned(),
            );
        }
        Ok(Slice {
            anchor,
            tasks,
            dependencies: edges,
            open_work,
            boundary,
            closed: blockers.is_empty(),
            blockers,
        })
    }
}
pub fn validate_cursor(session: &Session, cursor: Option<&Cursor>, query: &str) -> Result<usize> {
    if let Some(c) = cursor {
        if c.plan_id != session.plan.id || c.event_seq != session.event_seq || c.query_hash != query
        {
            return Err(Error::Invalid(
                "Cursor does not match pinned plan, event sequence or query".into(),
            ));
        }
        Ok(c.offset)
    } else {
        Ok(0)
    }
}
pub fn paginate<T>(
    session: &Session,
    rows: Vec<T>,
    limit: usize,
    offset: usize,
    query_hash: String,
) -> Result<Page<T>> {
    if limit == 0 || offset > rows.len() {
        return Err(Error::Invalid("Invalid page limit or offset".into()));
    }
    let total = rows.len();
    let end = offset.saturating_add(limit).min(total);
    Ok(Page {
        schema_version: 1,
        plan_id: session.plan.id,
        snapshot_id: session.plan.snapshot_id.clone(),
        event_seq: session.event_seq,
        items: rows.into_iter().skip(offset).take(limit).collect(),
        complete: end == total,
        next_cursor: (end < total).then_some(Cursor {
            plan_id: session.plan.id,
            event_seq: session.event_seq,
            query_hash,
            offset: end,
        }),
    })
}
pub fn diff_plans(before: &Plan, after: &Plan, links: &[HistoryLink]) -> PlanDiff {
    let a = before
        .members
        .iter()
        .map(|m| m.task_id)
        .collect::<BTreeSet<_>>();
    let b = after
        .members
        .iter()
        .map(|m| m.task_id)
        .collect::<BTreeSet<_>>();
    PlanDiff {
        before: before.id,
        after: after.id,
        added: b.difference(&a).copied().collect(),
        removed: a.difference(&b).copied().collect(),
        kept: a.intersection(&b).copied().collect(),
        links: links
            .iter()
            .filter(|l| a.contains(&l.from) && b.contains(&l.to))
            .cloned()
            .collect(),
    }
}
