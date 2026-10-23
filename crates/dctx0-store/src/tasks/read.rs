use super::*;
use crate::Result;
pub(super) fn schema(connection: &Connection) -> Result<i64> {
    Ok(connection.pragma_query_value(None, "user_version", |r| r.get(0))?)
}
pub(crate) fn head(connection: &Connection, project: &str) -> Result<Option<i64>> {
    if schema(connection)? == 1 {
        return Ok(None);
    }
    Ok(connection
        .query_row(
            "SELECT plan_id FROM task_heads WHERE project=?1",
            [project],
            |r| r.get(0),
        )
        .optional()?)
}
pub(crate) fn watermark(connection: &Connection, project: &str) -> Result<i64> {
    if schema(connection)? == 1 {
        return Ok(0);
    }
    Ok(connection.query_row(
        "SELECT coalesce(max(seq),0) FROM task_events WHERE project=?1",
        [project],
        |r| r.get(0),
    )?)
}
pub(super) fn event(
    tx: &Connection,
    project: &str,
    id: Option<i64>,
    kind: &str,
    message: &str,
    state: Option<&State>,
) -> Result<i64> {
    let data = state.map(|s| storage::store(tx, s)).transpose()?;
    tx.execute(
        "INSERT INTO task_events(project,task_id,kind,message,state_data) VALUES(?1,?2,?3,?4,?5)",
        params![project, id, kind, message, data],
    )?;
    let seq = tx.last_insert_rowid();
    if let (Some(id), Some(state)) = (id, state) {
        storage::state_metadata(tx, seq, id, state)?;
        tx.execute("INSERT INTO task_states(task_id,work,actor,version,data) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(task_id) DO UPDATE SET work=excluded.work,actor=excluded.actor,version=excluded.version,data=excluded.data",params![id,format!("{:?}",state.work),state.actor,state.version,storage::store(tx,state)?])?;
    }
    Ok(seq)
}
pub(crate) fn session(
    connection: &Connection,
    project: &str,
    plan_id: Option<i64>,
    as_of: Option<i64>,
) -> Result<Session> {
    session_inner(connection, project, plan_id, as_of, false)
}
pub(crate) fn metadata_session(
    connection: &Connection,
    project: &str,
    plan_id: Option<i64>,
    as_of: Option<i64>,
) -> Result<Session> {
    session_inner(connection, project, plan_id, as_of, true)
}
fn session_inner(
    connection: &Connection,
    project: &str,
    plan_id: Option<i64>,
    as_of: Option<i64>,
    metadata: bool,
) -> Result<Session> {
    let id = plan_id
        .or(head(connection, project)?)
        .ok_or_else(|| invalid("No task plan; run check then tasks plan"))?;
    let seq = as_of.unwrap_or(watermark(connection, project)?);
    if seq < 0 || seq > watermark(connection, project)? {
        return Err(invalid("Unknown task event sequence"));
    }
    let data: Option<String> = connection
        .query_row(
            "SELECT data FROM task_plans WHERE project=?1 AND id=?2 AND created_seq<=?3",
            params![project, id, seq],
            |r| r.get(0),
        )
        .optional()?;
    let plan: Plan = storage::load(
        connection,
        &data.ok_or_else(|| invalid(format!("Unknown plan P{id} at event {seq}")))?,
    )?;
    let mut definitions = BTreeMap::new();
    let mut states = BTreeMap::new();
    let mut statement = connection
        .prepare("SELECT id,data FROM tasks WHERE project=?1 AND created_seq<=?2 ORDER BY id")?;
    for row in statement.query_map(params![project, seq], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })? {
        let (id, data) = row?;
        definitions.insert(id, storage::load::<Definition>(connection, &data)?);
    }
    let state_sql = if metadata && storage::version(connection)? >= 7 {
        "SELECT task_id,data FROM storage_state_metadata WHERE event_seq IN (SELECT max(event_seq) FROM storage_state_metadata WHERE event_seq<=?2 AND task_id IN (SELECT id FROM tasks WHERE project=?1) GROUP BY task_id)"
    } else {
        "SELECT task_id,state_data FROM task_events WHERE project=?1 AND state_data IS NOT NULL AND seq IN (SELECT max(seq) FROM task_events WHERE project=?1 AND state_data IS NOT NULL AND seq<=?2 GROUP BY task_id)"
    };
    let mut statement = connection.prepare(state_sql)?;
    /* let mut statement=connection.prepare("SELECT task_id,state_data FROM task_events WHERE project=?1 AND state_data IS NOT NULL AND seq IN (SELECT max(seq) FROM task_events WHERE project=?1 AND state_data IS NOT NULL AND seq<=?2 GROUP BY task_id)")?; */
    for row in statement.query_map(params![project, seq], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })? {
        let (id, data) = row?;
        states.insert(id, storage::load::<State>(connection, &data)?);
    }
    let mut events = vec![];
    let mut statement=connection.prepare("SELECT seq,task_id,kind,message FROM task_events WHERE project=?1 AND seq<=?2 ORDER BY seq")?;
    for row in statement.query_map(params![project, seq], |r| {
        Ok(Event {
            seq: r.get(0)?,
            task_id: r.get(1)?,
            kind: r.get(2)?,
            message: r.get(3)?,
        })
    })? {
        events.push(row?);
    }
    let mut links = vec![];
    let mut statement = connection.prepare(
        "SELECT from_id,to_id,kind,reason FROM task_links WHERE project=?1 ORDER BY from_id,to_id",
    )?;
    for row in statement.query_map([project], |r| {
        Ok(HistoryLink {
            from: r.get(0)?,
            to: r.get(1)?,
            kind: r.get(2)?,
            reason: r.get(3)?,
        })
    })? {
        let link = row?;
        if definitions.contains_key(&link.to) {
            links.push(link);
        }
    }
    if definitions.keys().any(|id| !states.contains_key(id)) {
        return Err(invalid("Task history missing required state"));
    }
    if plan.id != id
        || plan
            .members
            .iter()
            .any(|m| !definitions.contains_key(&m.task_id))
        || definitions.iter().any(|(id, d)| *id != d.id)
        || links.iter().any(|l| !definitions.contains_key(&l.from))
    {
        return Err(invalid(
            "Task history contains invalid definition references",
        ));
    }
    let mut candidates = plan
        .members
        .iter()
        .map(|m| (m.candidate.key.clone(), m.candidate.clone()))
        .collect::<BTreeMap<_, _>>();
    if candidates.len() != plan.members.len() {
        return Err(invalid("Task plan contains duplicate producer keys"));
    }
    assign_waves(&mut candidates)?;
    if plan
        .members
        .iter()
        .any(|m| candidates[&m.candidate.key].wave != m.candidate.wave)
    {
        return Err(invalid("Task plan contains invalid dependency waves"));
    }
    Ok(Session {
        project: project.into(),
        plan,
        event_seq: seq,
        states,
        definitions,
        links,
        events,
    })
}
pub(super) fn latest_check(connection: &Connection, project: &str) -> Result<(i64, String)> {
    connection.query_row("SELECT c.id,c.snapshot_id FROM checks c JOIN projects p ON p.project=c.project AND p.latest=c.snapshot_id WHERE c.project=?1 AND c.status='pass' ORDER BY c.id DESC LIMIT 1",[project],|r|Ok((r.get(0)?,r.get(1)?))).optional()?.ok_or_else(||invalid("No valid published snapshot; run check first"))
}

impl Store {
    pub fn task_head(&self, project: &str) -> Result<Option<i64>> {
        head(&self.connection, project)
    }
    pub fn task_sequence(&self, project: &str) -> Result<i64> {
        watermark(&self.connection, project)
    }
    pub fn task_session(
        &self,
        project: &str,
        plan: Option<i64>,
        as_of: Option<i64>,
    ) -> Result<Session> {
        let tx = self.connection.unchecked_transaction()?;
        let result = session(&tx, project, plan, as_of)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn task_detail_session(
        &self,
        project: &str,
        task: i64,
        plan: Option<i64>,
        as_of: Option<i64>,
    ) -> Result<Session> {
        let tx = self.connection.unchecked_transaction()?;
        let mut selected = metadata_session(&tx, project, plan, as_of)?;
        let mut ids = std::collections::BTreeSet::from([task]);
        ids.extend(
            selected
                .related(task, false, true)?
                .into_iter()
                .map(|edge| edge.prerequisite_id),
        );
        let mut statement=tx.prepare("SELECT state_data FROM task_events WHERE project=?1 AND task_id=?2 AND state_data IS NOT NULL AND seq<=?3 ORDER BY seq DESC LIMIT 1")?;
        for id in ids {
            let raw: String =
                statement.query_row(params![project, id, selected.event_seq], |r| r.get(0))?;
            selected.states.insert(id, storage::load(&tx, &raw)?);
        }
        drop(statement);
        tx.commit()?;
        Ok(selected)
    }
    pub fn task_metadata_session(
        &self,
        project: &str,
        plan: Option<i64>,
        as_of: Option<i64>,
    ) -> Result<Session> {
        let tx = self.connection.unchecked_transaction()?;
        let result = metadata_session(&tx, project, plan, as_of)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn task_check(&self, project: &str) -> Result<(i64, String)> {
        latest_check(&self.connection, project)
    }
    // Occurrence-specific continuity: deletion ends lineage even on equal hashes.
    pub fn task_continuity(
        &self,
        project: &str,
        before: i64,
        after: i64,
    ) -> Result<BTreeMap<String, String>> {
        if before > after {
            return Err(invalid(
                "Task publication cannot move backwards in check history",
            ));
        }
        let mut statement=self.connection.prepare("SELECT id,snapshot_id FROM checks WHERE project=?1 AND status='pass' AND id>=?2 AND id<=?3 ORDER BY id")?;
        let timeline = statement
            .query_map(params![project, before, after], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if timeline.first().map(|r| r.0) != Some(before)
            || timeline.last().map(|r| r.0) != Some(after)
        {
            return Err(invalid("Unknown check occurrence for task continuity"));
        }
        let first = self.load(project, &timeline[0].1)?;
        let mut lineage = first
            .ir
            .symbols
            .keys()
            .map(|id| (id.clone(), id.clone()))
            .collect::<BTreeMap<_, _>>();
        for (run, snapshot) in timeline.into_iter().skip(1) {
            let mappings:Option<String>=self.connection.query_row("SELECT r.mappings FROM check_renames cr JOIN renames r ON r.id=cr.rename_id WHERE cr.check_id=?1 AND r.project=?2",params![run,project],|r|r.get(0)).optional()?;
            let mappings = mappings
                .map(|s| decode::<Vec<RenameMapping>>(&s))
                .transpose()?
                .unwrap_or_default();
            let renames = mappings
                .iter()
                .map(|m| (&m.old, &m.new))
                .collect::<BTreeMap<_, _>>();
            for id in lineage.values_mut() {
                if let Some(new) = renames.get(id) {
                    *id = (*new).clone();
                }
            }
            let next = self.load(project, &snapshot)?;
            lineage.retain(|_, id| next.ir.symbols.contains_key(id));
        }
        Ok(lineage)
    }
    pub fn task_membership(&self, project: &str, task: i64, as_of: Option<i64>) -> Result<i64> {
        let seq = as_of.unwrap_or(watermark(&self.connection, project)?);
        self.connection.query_row("SELECT p.id FROM task_plans p JOIN task_members m ON m.plan_id=p.id WHERE p.project=?1 AND m.task_id=?2 AND p.created_seq<=?3 ORDER BY p.id DESC LIMIT 1",params![project,task,seq],|r|r.get(0)).optional()?.ok_or_else(||dctx0_tasks::Error::Missing(task).into())
    }
}
