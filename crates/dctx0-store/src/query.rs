//! Public query version 1. SQL executes against isolated metadata, never the history connection.
use crate::*;
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    types::Value,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const QUERY_VIEW_VERSION: u32 = 2;
pub const QUERY_VIEWS: &[(&str, &str)] = &[
    ("dctx0_run_links", "run_id TEXT,parent_run_id TEXT"),
    (
        "dctx0_task_evidence",
        "task_id INTEGER,requirements_hash TEXT,implementation TEXT,status TEXT,verifier TEXT,verifier_version TEXT,started_at TEXT,finished_at TEXT,report_hash TEXT,details_available INTEGER",
    ),
    (
        "dctx0_tasks",
        "task_id INTEGER,title TEXT,kind TEXT,wave INTEGER,work TEXT,check_status TEXT,ready INTEGER,delivered INTEGER,actor TEXT,blockers TEXT",
    ),
    ("dctx0_task_subjects", "task_id INTEGER,subject TEXT"),
    (
        "dctx0_task_dependencies",
        "task_id INTEGER,prerequisite_id INTEGER,condition TEXT,reason TEXT",
    ),
    (
        "dctx0_task_links",
        "from_task INTEGER,to_task INTEGER,kind TEXT,reason TEXT",
    ),
    (
        "dctx0_task_events",
        "sequence INTEGER,task_id INTEGER,kind TEXT,message TEXT",
    ),
    (
        "dctx0_checks",
        "check_occurrence INTEGER,snapshot TEXT,status TEXT,created_at TEXT",
    ),
    (
        "dctx0_runs",
        "run_id TEXT,snapshot TEXT,check_occurrence INTEGER,result TEXT,created_at TEXT,required_cases INTEGER,recorded_cases INTEGER,pass_cases INTEGER,fail_cases INTEGER,unknown_cases INTEGER,missing_cases INTEGER,open_obligations INTEGER,open_obligations_available INTEGER,details_available INTEGER",
    ),
    ("dctx0_run_issues", "run_id TEXT,code TEXT,message TEXT"),
    (
        "dctx0_cases",
        "run_id TEXT,case_id TEXT,ordinal INTEGER,subject TEXT,kind TEXT,outcome TEXT,issue_code TEXT,observations INTEGER,body_hash TEXT,body_available INTEGER",
    ),
    (
        "dctx0_run_requirements",
        "run_id TEXT,case_id TEXT,recorded INTEGER",
    ),
    (
        "dctx0_artifacts",
        "run_id TEXT,hash TEXT,media_type TEXT,bytes INTEGER,available INTEGER",
    ),
    (
        "dctx0_subjects",
        "snapshot TEXT,subject TEXT,kind TEXT,text TEXT",
    ),
    (
        "dctx0_relations",
        "snapshot TEXT,source TEXT,target TEXT,kind TEXT",
    ),
    (
        "dctx0_facts",
        "snapshot TEXT,subject TEXT,owner TEXT,kind TEXT,text TEXT",
    ),
    (
        "dctx0_source_refs",
        "snapshot TEXT,subject TEXT,task_id INTEGER,run_id TEXT,origin TEXT,file TEXT,start_line INTEGER,end_line INTEGER,source_hash TEXT",
    ),
];
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryPins {
    pub plan: Option<i64>,
    pub as_of: Option<i64>,
    pub snapshot: Option<String>,
}
#[derive(Debug, Clone, Copy)]
pub struct QueryLimits {
    pub max_rows: usize,
    pub max_bytes: usize,
    pub timeout_ms: u64,
}
impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            max_rows: 100,
            max_bytes: 1048576,
            timeout_ms: 5000,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct QueryReport {
    pub view_version: u32,
    pub pins: QueryPins,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub complete: bool,
    pub availability: Vec<String>,
}
fn invalid(message: impl Into<String>) -> Error {
    Error::Verification(message.into())
}
pub(crate) fn put(c: &Connection, view: &str, value: serde_json::Value) -> Result<()> {
    let Some((index, (_, columns))) = QUERY_VIEWS
        .iter()
        .enumerate()
        .find(|(_, (name, _))| *name == view)
    else {
        return Err(invalid("Unknown public query view"));
    };
    let keys = columns
        .split(',')
        .map(|v| v.split_whitespace().next().unwrap_or(v))
        .collect::<Vec<_>>();
    let values = keys
        .iter()
        .map(|key| match &value[*key] {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(v) => Value::Integer(i64::from(*v)),
            serde_json::Value::Number(v) => v
                .as_i64()
                .map(Value::Integer)
                .unwrap_or_else(|| Value::Real(v.as_f64().unwrap_or(0.0))),
            serde_json::Value::String(v) => Value::Text(v.clone()),
            v => Value::Text(v.to_string()),
        })
        .collect::<Vec<_>>();
    let slots = std::iter::repeat_n("?", keys.len())
        .collect::<Vec<_>>()
        .join(",");
    c.execute(
        &format!("INSERT INTO query_rows_{index} VALUES({slots})"),
        rusqlite::params_from_iter(values),
    )?;
    Ok(())
}
fn outcome(result: &dctx0_core::VerificationResult) -> &'static str {
    match result {
        dctx0_core::VerificationResult::Pass => "PASS",
        dctx0_core::VerificationResult::Fail { .. } => "FAIL",
        _ => "UNKNOWN",
    }
}
pub(crate) fn project_snapshot(c: &Connection, snapshot: &Snapshot) -> Result<()> {
    if storage::version(c)? >= 7 {
        return storage_spec::project(c, snapshot);
    }
    let exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM query_snapshots WHERE snapshot=?1)",
        [&snapshot.id],
        |r| r.get(0),
    )?;
    if exists {
        return Ok(());
    }
    c.execute(
        "DELETE FROM query_spec_rows WHERE snapshot=?1",
        [&snapshot.id],
    )?;
    for node in &snapshot.ir.nodes {
        let source = snapshot.sources.iter().find(|s| s.file == node.span.file);
        let text = source
            .and_then(|s| s.text.get(node.span.start.offset..node.span.end.offset))
            .unwrap_or("");
        let kind = serde_json::to_value(node.kind)?;
        for (view, value) in [
            (
                "dctx0_subjects",
                json!({"snapshot":snapshot.id,"subject":node.id,"kind":kind,"text":text}),
            ),
            (
                "dctx0_facts",
                json!({"snapshot":snapshot.id,"subject":node.id,"owner":node.owner,"kind":kind,"text":text}),
            ),
            (
                "dctx0_source_refs",
                json!({"snapshot":snapshot.id,"subject":node.id,"origin":"pinned-spec","file":node.span.file,"start_line":node.span.start.line,"end_line":node.span.end.line,"source_hash":source.map(|s|format!("{:x}",Sha256::digest(s.text.as_bytes())))}),
            ),
        ] {
            c.execute(
                "INSERT INTO query_spec_rows VALUES(?1,?2,?3)",
                params![snapshot.id, view, value.to_string()],
            )?;
        }
    }
    for edge in &snapshot.graph.edges {
        c.execute("INSERT INTO query_spec_rows VALUES(?1,'dctx0_relations',?2)",params![snapshot.id,json!({"snapshot":snapshot.id,"source":edge.from,"target":edge.to,"kind":edge.kind}).to_string()])?;
    }
    c.execute(
        "INSERT OR REPLACE INTO query_snapshots VALUES(?1,1)",
        [&snapshot.id],
    )?;
    Ok(())
}
pub(crate) fn preparation_command(project: &str, snapshot: Option<&str>) -> String {
    let quoted = format!("'{}'", project.replace('\'', "'\"'\"'"));
    format!(
        "dctx0 query prepare {quoted}{}",
        snapshot
            .map(|s| format!(" --snapshot {s}"))
            .unwrap_or_default()
    )
}
impl Store {
    /// Explicitly materialize pinned spec text; no working-tree contents are used.
    pub fn prepare_query_snapshot(&mut self, project: &str, snapshot: &str) -> Result<()> {
        self.require_write()?;
        if storage::version(&self.connection)? >= 7 {
            let id = self.snapshot_id(project, snapshot)?;
            let ready:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM storage_snapshot_keys k JOIN query_snapshots q ON q.snapshot=k.snapshot WHERE k.snapshot=?1)",[&id],|r|r.get(0))?;
            if ready {
                return Ok(());
            }
        }
        let snapshot = self.load(project, snapshot)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        project_snapshot(&tx, &snapshot)?;
        tx.commit()?;
        Ok(())
    }
    pub fn query_sql(
        &self,
        project: &str,
        pins: QueryPins,
        sql: &str,
        parameters: &BTreeMap<String, serde_json::Value>,
        limits: QueryLimits,
    ) -> Result<QueryReport> {
        let started = std::time::Instant::now();
        let result = self.query_sql_inner(project, pins, sql, parameters, limits);
        if started.elapsed() >= Duration::from_millis(limits.timeout_ms) {
            return Err(invalid(format!(
                "QUERY_TIMEOUT: query preparation and SQL exceeded {} ms",
                limits.timeout_ms
            )));
        }
        result
    }
    fn query_sql_inner(
        &self,
        project: &str,
        mut pins: QueryPins,
        sql: &str,
        parameters: &BTreeMap<String, serde_json::Value>,
        limits: QueryLimits,
    ) -> Result<QueryReport> {
        let QueryLimits {
            max_rows,
            max_bytes,
            timeout_ms,
        } = limits;
        if !(1..=10000).contains(&max_rows)
            || !(1..=67108864).contains(&max_bytes)
            || !(1..=60000).contains(&timeout_ms)
        {
            return Err(invalid("Invalid query limits"));
        }
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        self.connection
            .progress_handler(1000, Some(move || std::time::Instant::now() >= deadline))?;
        let _guard = SourceProgress(&self.connection);
        let tx = self.connection.unchecked_transaction()?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < 6 {
            return Err(invalid(format!(
                "QUERY_INDEX_MISSING: run `{}`",
                preparation_command(project, None)
            )));
        }
        let c = Connection::open_in_memory()?;
        c.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 1048576)?;
        c.set_limit(Limit::SQLITE_LIMIT_COLUMN, 256)?;
        c.set_limit(Limit::SQLITE_LIMIT_EXPR_DEPTH, 100)?;
        c.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
        for (index, (name, columns)) in QUERY_VIEWS.iter().enumerate() {
            let first = columns.split_whitespace().next().unwrap_or("task_id");
            c.execute_batch(&format!("CREATE TABLE query_rows_{index}({columns}); CREATE VIEW {name} AS SELECT * FROM query_rows_{index} WHERE typeof({first}) != '';"))?;
            for column in columns
                .split(',')
                .filter_map(|v| v.split_whitespace().next())
                .filter(|v| {
                    matches!(
                        *v,
                        "task_id"
                            | "run_id"
                            | "subject"
                            | "snapshot"
                            | "source"
                            | "target"
                            | "outcome"
                            | "case_id"
                            | "work"
                            | "prerequisite_id"
                    )
                })
            {
                c.execute_batch(&format!(
                    "CREATE INDEX query_index_{index}_{column} ON query_rows_{index}({column});"
                ))?;
            }
        }
        reject_private_identifier(sql)?;
        let needed = std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let collector = needed.clone();
        c.authorizer(Some(move |context: AuthContext<'_>| {
            if let Some(name) = context.accessor {
                if QUERY_VIEWS.iter().any(|(view, _)| *view == name) {
                    if let Ok(mut set) = collector.lock() {
                        set.insert(name.to_owned());
                    }
                }
            }
            if let AuthAction::Read { table_name, .. } = context.action {
                if QUERY_VIEWS.iter().any(|(name, _)| *name == table_name) {
                    if let Ok(mut set) = collector.lock() {
                        set.insert(table_name.to_owned());
                    }
                }
            }
            Authorization::Allow
        }))?;
        {
            let statement = c.prepare(sql)?;
            if !statement.readonly() || statement.column_count() == 0 {
                return Err(invalid("Exactly one read-only SELECT statement required"));
            }
        }
        c.authorizer(None::<fn(AuthContext<'_>) -> Authorization>)?;
        let needed = needed
            .lock()
            .map_err(|_| invalid("Query dependency collection failed"))?
            .clone();
        let task_views = needed.iter().any(|name| name.starts_with("dctx0_task"));
        if pins.plan.is_some() || tasks::head(&tx, project)?.is_some() {
            let selected = pins
                .plan
                .or(tasks::head(&tx, project)?)
                .ok_or_else(|| invalid("Missing task plan"))?;
            let seq = pins.as_of.unwrap_or(tasks::watermark(&tx, project)?);
            if seq < 0 || seq > tasks::watermark(&tx, project)? {
                return Err(invalid("Unknown task event sequence"));
            }
            let snapshot: String = tx.query_row(
                "SELECT snapshot_id FROM task_plans WHERE id=?1 AND project=?2 AND created_seq<=?3",
                params![selected, project, seq],
                |r| r.get(0),
            )?;
            pins.plan = Some(selected);
            pins.as_of = Some(seq);
            if pins.snapshot.is_none() {
                pins.snapshot = Some(snapshot);
            }
            if task_views {
                let session = tasks::metadata_session(&tx, project, pins.plan, pins.as_of)?;
                pins.plan = Some(session.plan.id);
                pins.as_of = Some(session.event_seq);
                if pins.snapshot.is_none() {
                    pins.snapshot = Some(session.plan.snapshot_id.clone());
                }
                if needed.contains("dctx0_tasks") || needed.contains("dctx0_task_subjects") {
                    for row in session.overviews() {
                        put(
                            &c,
                            "dctx0_tasks",
                            json!({"task_id":row.task_id,"title":row.title,"kind":row.kind,"wave":row.wave,"work":format!("{:?}",row.work),"check_status":format!("{:?}",row.check).to_uppercase(),"ready":row.ready,"delivered":row.delivered,"actor":row.actor,"blockers":serde_json::to_string(&row.blockers)?}),
                        )?;
                        for subject in row.subjects {
                            put(
                                &c,
                                "dctx0_task_subjects",
                                json!({"task_id":row.task_id,"subject":subject}),
                            )?;
                        }
                    }
                }
                if needed.contains("dctx0_task_dependencies") {
                    for row in session.dependencies() {
                        put(&c, "dctx0_task_dependencies", serde_json::to_value(row)?)?;
                    }
                }
                if needed.contains("dctx0_task_links") {
                    for row in session.links {
                        put(
                            &c,
                            "dctx0_task_links",
                            json!({"from_task":row.from,"to_task":row.to,"kind":row.kind,"reason":row.reason}),
                        )?;
                    }
                }
                if needed.contains("dctx0_task_events") {
                    for row in session.events {
                        put(
                            &c,
                            "dctx0_task_events",
                            json!({"sequence":row.seq,"task_id":row.task_id,"kind":row.kind,"message":row.message}),
                        )?;
                    }
                }
                if needed.contains("dctx0_task_evidence") && version < 7 {
                    for (task, state) in &session.states {
                        for e in &state.evidence {
                            put(
                                &c,
                                "dctx0_task_evidence",
                                storage::evidence_metadata(*task, e),
                            )?;
                        }
                    }
                }
                if needed.contains("dctx0_task_evidence") && version >= 7 {
                    let mut q=tx.prepare("SELECT data FROM storage_evidence_metadata WHERE event_seq IN (SELECT max(event_seq) FROM storage_state_metadata WHERE event_seq<=?2 AND task_id IN (SELECT id FROM tasks WHERE project=?1) GROUP BY task_id)")?;
                    for data in q.query_map(params![project, seq], |r| r.get::<_, String>(0))? {
                        put(&c, "dctx0_task_evidence", decode(&data?)?)?;
                    }
                }
            }
        } else if pins.as_of.is_some() {
            return Err(invalid("No task plan for --as-of"));
        }
        let snapshot_id =
            self.snapshot_id(project, pins.snapshot.as_deref().unwrap_or("latest"))?;
        pins.snapshot = Some(snapshot_id.clone());
        let spec_views = needed.iter().any(|v| {
            matches!(
                v.as_str(),
                "dctx0_subjects" | "dctx0_relations" | "dctx0_facts" | "dctx0_source_refs"
            )
        });
        if spec_views {
            let ready: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM query_snapshots WHERE snapshot=?1 AND version=1)",
                [&snapshot_id],
                |r| r.get(0),
            )?;
            if !ready {
                return Err(invalid(format!(
                    "QUERY_INDEX_MISSING: run `{}`",
                    preparation_command(project, Some(&snapshot_id))
                )));
            }
            if version >= 7 {
                storage_spec::populate(&tx, &c, &snapshot_id, &needed)?;
            } else {
                for row in tx
                    .prepare("SELECT view,data FROM query_spec_rows WHERE snapshot=?1")?
                    .query_map([&snapshot_id], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })?
                {
                    let (view, data) = row?;
                    if needed.contains(&view) {
                        put(&c, &view, decode(&data)?)?;
                    }
                }
            }
        }
        if needed.contains("dctx0_checks") {
            for row in self.history(project)? {
                put(
                    &c,
                    "dctx0_checks",
                    json!({"check_occurrence":row.run_id,"snapshot":row.snapshot_id,"status":row.status,"created_at":row.timestamp}),
                )?;
            }
        }
        if needed.contains("dctx0_run_links") && version >= 7 {
            let mut q=tx.prepare("SELECT p.child,p.parent FROM verification_run_parents p JOIN verification_runs c ON c.id=p.child JOIN verification_runs r ON r.id=p.parent WHERE c.project=?1 AND r.project=?1 ORDER BY p.child,p.parent")?;
            for row in q.query_map([project], |r| {
                Ok(json!({"run_id":r.get::<_,String>(0)?,"parent_run_id":r.get::<_,String>(1)?}))
            })? {
                put(&c, "dctx0_run_links", row?)?;
            }
        }
        let availability=vec!["Historical pins; no working-tree freshness. Historical case metadata can outlive its body; check body_available. Older already-pruned runs may have no case metadata. Implementation references are limited to captured evidence.".into()];
        if needed.iter().any(|v| {
            matches!(
                v.as_str(),
                "dctx0_runs"
                    | "dctx0_cases"
                    | "dctx0_run_issues"
                    | "dctx0_run_requirements"
                    | "dctx0_artifacts"
                    | "dctx0_source_refs"
            )
        }) {
            let mut stmt = tx.prepare(
                "SELECT id,data,pruned_at FROM verification_runs WHERE project=?1 ORDER BY rowid",
            )?;
            for row in stmt.query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })? {
                let (id, data, pruned) = row?;
                let summary = self.verification_run_summary(project, &id)?;
                put(
                    &c,
                    "dctx0_runs",
                    json!({"run_id":id,"snapshot":summary.snapshot,"check_occurrence":summary.check_occurrence,"result":outcome(&summary.result),"created_at":summary.created_at,"required_cases":summary.required_case_count,"recorded_cases":summary.case_count,"pass_cases":summary.outcomes[0],"fail_cases":summary.outcomes[1],"unknown_cases":summary.outcomes[2],"missing_cases":summary.missing_case_count,"open_obligations":summary.open_obligation_count,"open_obligations_available":summary.open_obligation_count_available,"details_available":summary.details_available}),
                )?;
                if data.is_empty() || (pruned.is_some() && version < 7) {
                    continue;
                }
                if !verification::cases::prepared(&tx, &id)? {
                    return Err(invalid(format!(
                        "QUERY_INDEX_MISSING: {id}; run `{}`",
                        preparation_command(project, None)
                    )));
                }
                if needed.iter().all(|v| {
                    !matches!(
                        v.as_str(),
                        "dctx0_cases"
                            | "dctx0_run_issues"
                            | "dctx0_run_requirements"
                            | "dctx0_artifacts"
                            | "dctx0_source_refs"
                    )
                }) {
                    continue;
                }
                let header: dctx0_core::VerificationRun = decode(&data)?;
                if needed.contains("dctx0_source_refs") {
                    for (file, hash) in &header.provenance.implementation {
                        put(
                            &c,
                            "dctx0_source_refs",
                            json!({"snapshot":header.provenance.snapshot,"run_id":id,"origin":"captured-implementation","file":file,"source_hash":hash}),
                        )?;
                    }
                }
                if needed.contains("dctx0_run_issues") {
                    for issue in header.open_obligations {
                        put(
                            &c,
                            "dctx0_run_issues",
                            json!({"run_id":id,"code":issue.code,"message":issue.message}),
                        )?;
                    }
                }
                if needed.contains("dctx0_artifacts") {
                    for a in header.artifacts {
                        let metadata = matches!(
                            a.media_type.as_str(),
                            "application/vnd.dctx0.runtime-metrics+json"
                                | "application/vnd.dctx0.runtime-identity+json"
                        );
                        put(
                            &c,
                            "dctx0_artifacts",
                            json!({"run_id":id,"hash":a.hash,"media_type":a.media_type,"bytes":a.bytes,"available":pruned.is_none() || metadata}),
                        )?;
                    }
                }
                if needed.contains("dctx0_cases") {
                    for row in tx.prepare("SELECT case_id,ordinal,subject,kind,outcome,issue_code,observations,body_hash,EXISTS(SELECT 1 FROM verification_case_bodies b WHERE b.hash=i.body_hash) FROM verification_case_index i WHERE run_id=?1 ORDER BY ordinal")?.query_map([&id],|r|Ok(json!({"run_id":id,"case_id":r.get::<_,String>(0)?,"ordinal":r.get::<_,i64>(1)?,"subject":r.get::<_,String>(2)?,"kind":r.get::<_,String>(3)?,"outcome":r.get::<_,String>(4)?,"issue_code":r.get::<_,Option<String>>(5)?,"observations":r.get::<_,i64>(6)?,"body_hash":r.get::<_,String>(7)?,"body_available":r.get::<_,bool>(8)? && pruned.is_none()})))?{put(&c,"dctx0_cases",row?)?;}
                }
                if needed.contains("dctx0_run_requirements") {
                    for row in tx.prepare("SELECT case_id,EXISTS(SELECT 1 FROM verification_case_index i WHERE i.run_id=r.run_id AND i.case_id=r.case_id) FROM verification_requirements r WHERE run_id=?1")?.query_map([&id],|r|Ok(json!({"run_id":id,"case_id":r.get::<_,String>(0)?,"recorded":r.get::<_,bool>(1)?})))?{put(&c,"dctx0_run_requirements",row?)?;}
                }
            }
            drop(stmt);
        }
        tx.commit()?;
        let remaining = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_millis() as u64;
        if remaining == 0 {
            return Err(invalid(
                "QUERY_TIMEOUT: query preparation exhausted the time limit",
            ));
        }
        execute(
            c,
            pins,
            sql,
            parameters,
            QueryLimits {
                timeout_ms: remaining,
                ..limits
            },
            availability,
        )
    }
}
/// SQLite reports a CTE name as its authorizer accessor too. Reject the private
/// backing identifier in user SQL before preparing, including SQLite's quoted
/// table-name forms. Comments are skipped; scalar values can always be bound.
fn reject_private_identifier(sql: &str) -> Result<()> {
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"--") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            i += 2;
            while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        let mut token = Vec::new();
        if matches!(bytes[i], b'\'' | b'"' | b'`' | b'[') {
            let open = bytes[i];
            let close = if open == b'[' { b']' } else { open };
            i += 1;
            while i < bytes.len() {
                if bytes[i] == close {
                    i += 1;
                    if open != b'[' && bytes.get(i) == Some(&close) {
                        token.push(close);
                        i += 1;
                        continue;
                    }
                    break;
                }
                token.push(bytes[i]);
                i += 1;
            }
        } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' || bytes[i] >= 128 {
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'$'
                    || bytes[i] >= 128)
            {
                token.push(bytes[i]);
                i += 1;
            }
        } else {
            i += 1;
        }
        if token.to_ascii_lowercase().starts_with(b"query_rows") {
            return Err(invalid(
                "QUERY_PRIVATE_ACCESS: internal query storage identifiers are reserved; select public dctx0_* views",
            ));
        }
    }
    Ok(())
}

struct SourceProgress<'a>(&'a Connection);
impl Drop for SourceProgress<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

fn execute(
    c: Connection,
    pins: QueryPins,
    sql: &str,
    parameters: &BTreeMap<String, serde_json::Value>,
    limits: QueryLimits,
    availability: Vec<String>,
) -> Result<QueryReport> {
    let QueryLimits {
        max_rows,
        max_bytes,
        timeout_ms,
    } = limits;
    reject_private_identifier(sql)?;
    c.set_limit(Limit::SQLITE_LIMIT_LENGTH, max_bytes as i32)?;
    c.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 1048576)?;
    c.set_limit(Limit::SQLITE_LIMIT_COLUMN, 256)?;
    c.set_limit(Limit::SQLITE_LIMIT_EXPR_DEPTH, 100)?;
    c.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    c.pragma_update(None, "query_only", true)?;
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    c.progress_handler(1000, Some(move || std::time::Instant::now() >= deadline))?;
    c.authorizer(Some(|context: AuthContext<'_>| match context.action {
        AuthAction::Select | AuthAction::Recursive => Authorization::Allow,
        AuthAction::Read { table_name, .. }
            if QUERY_VIEWS.iter().any(|(v, _)| *v == table_name)
                || (table_name.starts_with("query_rows_")
                    && context
                        .accessor
                        .is_some_and(|a| QUERY_VIEWS.iter().any(|(v, _)| *v == a))) =>
        {
            Authorization::Allow
        }
        AuthAction::Function { function_name }
            if [
                "json_extract",
                "count",
                "sum",
                "avg",
                "min",
                "max",
                "total",
                "coalesce",
                "ifnull",
                "nullif",
                "lower",
                "upper",
                "length",
                "substr",
                "substring",
                "instr",
                "like",
                "glob",
                "abs",
                "round",
                "typeof",
                "hex",
                "trim",
                "ltrim",
                "rtrim",
                "replace",
                "group_concat",
                "row_number",
                "rank",
                "dense_rank",
            ]
            .iter()
            .any(|f| function_name.eq_ignore_ascii_case(f)) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }))?;
    let mut stmt = c.prepare(sql)?;
    if !stmt.readonly() || stmt.column_count() == 0 {
        return Err(invalid("Exactly one read-only SELECT statement required"));
    }
    // prepare() rejects trailing statements through rusqlite's extra_check feature.
    for (name, value) in parameters {
        let index = stmt
            .parameter_index(&format!(":{name}"))?
            .ok_or_else(|| invalid(format!("Unknown SQL parameter :{name}")))?;
        let value = match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Integer(i64::from(*b)),
            serde_json::Value::Number(n) => {
                if let Some(n) = n.as_i64() {
                    Value::Integer(n)
                } else {
                    Value::Real(
                        n.as_f64()
                            .ok_or_else(|| invalid("Invalid numeric parameter"))?,
                    )
                }
            }
            serde_json::Value::String(s) => Value::Text(s.clone()),
            _ => return Err(invalid("SQL parameters must be scalar JSON")),
        };
        stmt.raw_bind_parameter(index, value)?;
    }
    for index in 1..=stmt.parameter_count() {
        let name = stmt
            .parameter_name(index)
            .ok_or_else(|| invalid("Use named :parameters"))?;
        if !name.starts_with(':') || !parameters.contains_key(&name[1..]) {
            return Err(invalid(format!("Missing SQL parameter {name}")));
        }
    }
    let columns = stmt
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut report = QueryReport {
        view_version: QUERY_VIEW_VERSION,
        pins,
        columns,
        rows: vec![],
        complete: true,
        availability,
    };
    let mut rows = stmt.raw_query();
    let mut bytes = serde_json::to_vec(&report)?.len();
    while let Some(row) = rows.next().map_err(|error| {
        if matches!(&error,rusqlite::Error::SqliteFailure(failure,_) if failure.code==rusqlite::ErrorCode::OperationInterrupted) {
            invalid(format!("QUERY_TIMEOUT: SQL exceeded {timeout_ms} ms"))
        } else {Error::Sql(error)}
    })? {
        if report.rows.len() == max_rows {
            report.complete = false;
            break;
        }
        let values = (0..report.columns.len())
            .map(|i| -> Result<_> {
                Ok(match row.get_ref(i)? {
                    rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => json!(n),
                    rusqlite::types::ValueRef::Real(n) => json!(n),
                    rusqlite::types::ValueRef::Text(s) => {
                        json!(std::str::from_utf8(s).map_err(|_| invalid("Invalid UTF-8 value"))?)
                    }
                    rusqlite::types::ValueRef::Blob(_) => {
                        return Err(invalid("Binary values are not public query results"));
                    }
                })
            })
            .collect::<Result<Vec<_>>>()?;
        bytes += serde_json::to_vec(&values)?.len() + 1;
        if bytes > max_bytes {
            return Err(invalid(
                "QUERY_OUTPUT_LIMIT_EXCEEDED: raise --max-output-bytes or select smaller values",
            ));
        }
        report.rows.push(values);
    }
    Ok(report)
}
