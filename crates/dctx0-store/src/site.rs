//! One consistent, read-only view for browser projection. No schema writes.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteSelection {
    pub snapshot: Option<String>,
    pub plan: Option<i64>,
    pub as_of: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SiteRevision {
    pub schema: i64,
    pub check: i64,
    pub snapshot: Option<String>,
    pub plan: Option<i64>,
    pub events: i64,
    pub runs: i64,
    pub renames: i64,
}
#[derive(Debug, Serialize)]
pub struct SiteRecords {
    pub revision: SiteRevision,
    pub snapshot: Snapshot,
    pub check: HistoryEntry,
    pub tasks: Option<dctx0_tasks::Session>,
    pub runs: Vec<SiteVerificationRun>,
    pub run_summaries: Vec<VerificationRunSummary>,
    pub history: Vec<HistoryEntry>,
    pub renames: Vec<RenameEvent>,
    pub changes: Option<Changes>,
    pub before: Option<Snapshot>,
    pub task_snapshot: Option<Snapshot>,
    pub requirement_snapshots: std::collections::BTreeMap<String, Snapshot>,
    pub plan_diff: Option<dctx0_tasks::PlanDiff>,
}
/// Header and compressed corpus keep export memory bounded to one decoded run.
#[derive(Debug, Serialize)]
pub struct SiteVerificationRun {
    pub id: String,
    pub created_at: String,
    pub header: dctx0_core::VerificationRun,
    pub summary: VerificationRunSummary,
    #[serde(skip)]
    pub corpus: Vec<u8>,
}
impl Store {
    /// An immutable-history watermark, read consistently without loading case bodies.
    pub fn site_revision(&self, project: &str) -> Result<SiteRevision> {
        let tx = self.connection.unchecked_transaction()?;
        let result = revision(&tx, project)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn site_records(&self, project: &str, selection: &SiteSelection) -> Result<SiteRecords> {
        let tx = self.connection.unchecked_transaction()?;
        let revision = revision(&tx, project)?;
        let schema: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let history = self.history(project)?;
        let requested = selection.snapshot.as_deref().unwrap_or("latest");
        let snapshot = self.load(project, requested).map_err(|error| match error {
            Error::Missing(_) if requested == "latest" => {
                Error::Missing("No valid published spec; run dctx0 check first".into())
            }
            other => other,
        })?;
        let check = history
            .iter()
            .find(|c| c.status == "pass" && c.snapshot_id.as_ref() == Some(&snapshot.id))
            .cloned()
            .ok_or_else(|| Error::Missing("No valid published check; run check first".into()))?;
        let tasks = if selection.plan.is_some() || tasks::head(&tx, project)?.is_some() {
            Some(tasks::session(
                &tx,
                project,
                selection.plan,
                selection.as_of,
            )?)
        } else {
            if selection.as_of.is_some() {
                return Err(Error::Missing("No task plan for --as-of".into()));
            }
            None
        };
        let mut runs = Vec::new();
        let mut run_summaries = Vec::new();
        if schema >= 4 {
            let mut offset = 0;
            loop {
                let page = self.verification_run_summaries(project, offset, 200)?;
                let count = page.len();
                run_summaries.extend(page);
                if count < 200 {
                    break;
                }
                offset += 200;
            }
            for summary in run_summaries.iter().filter(|s| s.details_available).take(3) {
                let (mut header, corpus) = if schema >= 5 {
                    let (data, hash): (String, Option<String>) = tx.query_row(
                        "SELECT data,corpus_hash FROM verification_runs WHERE project=?1 AND id=?2",
                        params![project, summary.id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )?;
                    if let Some(hash) = hash {
                        let (bytes, length): (Vec<u8>, i64) = tx.query_row(
                            "SELECT bytes,raw_length FROM verification_corpora WHERE hash=?1",
                            [&hash],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )?;
                        verification::codec::validate_corpus(&bytes, length, &hash)?;
                        (decode(&data)?, bytes)
                    } else {
                        site_legacy_run(&tx, project, &summary.id)?
                    }
                } else {
                    site_legacy_run(&tx, project, &summary.id)?
                };
                header.cases.clear();
                header.required_cases.clear();
                runs.push(SiteVerificationRun {
                    id: summary.id.clone(),
                    created_at: summary.created_at.clone(),
                    header,
                    summary: summary.clone(),
                    corpus,
                });
            }
        }
        let mut renames = Vec::new();
        let mut statement = tx.prepare(
            "SELECT id,before_id,after_id,mappings FROM renames WHERE project=?1 ORDER BY rowid",
        )?;
        for row in statement.query_map([project], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })? {
            let (id, before, after, mappings) = row?;
            renames.push(RenameEvent {
                id,
                before,
                after,
                mappings: decode(&mappings)?,
            });
        }
        drop(statement);
        let previous = history
            .iter()
            .filter(|h| h.run_id < check.run_id && h.status == "pass")
            .find_map(|h| h.snapshot_id.as_ref());
        let before = previous.map(|id| self.load(project, id)).transpose()?;
        let changes = before
            .as_ref()
            .map(|old| {
                let mappings = self.rename_mappings(project, &old.id, &snapshot.id)?;
                Ok::<_, Error>(diff(Some(old), &snapshot, &mappings))
            })
            .transpose()?;
        let task_snapshot = tasks
            .as_ref()
            .map(|s| self.load(project, &s.plan.snapshot_id))
            .transpose()?;
        let plan_diff = tasks
            .as_ref()
            .and_then(|s| s.plan.parent.map(|parent| (s, parent)))
            .map(|(s, parent)| {
                let old = tasks::session(&tx, project, Some(parent), selection.as_of)?;
                Ok::<_, Error>(dctx0_tasks::diff_plans(&old.plan, &s.plan, &s.links))
            })
            .transpose()?;
        let mut requirement_snapshots = std::collections::BTreeMap::new();
        if let Some(session) = &tasks {
            for definition in session.definitions.values() {
                for id in std::iter::once(&definition.origin_snapshot)
                    .chain(definition.before_snapshot.iter())
                {
                    if id != &snapshot.id
                        && before.as_ref().is_none_or(|s| &s.id != id)
                        && task_snapshot.as_ref().is_none_or(|s| &s.id != id)
                        && !requirement_snapshots.contains_key(id)
                    {
                        requirement_snapshots.insert(id.clone(), self.load(project, id)?);
                    }
                }
            }
        }
        tx.commit()?;
        Ok(SiteRecords {
            revision,
            snapshot,
            check,
            tasks,
            runs,
            run_summaries,
            history,
            renames,
            changes,
            before,
            task_snapshot,
            requirement_snapshots,
            plan_diff,
        })
    }
}

fn revision(connection: &Connection, project: &str) -> Result<SiteRevision> {
    let schema: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let check = connection.query_row(
        "SELECT coalesce(max(id),0) FROM checks WHERE project=?1",
        [project],
        |r| r.get(0),
    )?;
    let snapshot = connection
        .query_row(
            "SELECT latest FROM projects WHERE project=?1",
            [project],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    let plan = tasks::head(connection, project)?;
    let events = tasks::watermark(connection, project)?;
    let runs = if schema >= 4 {
        connection.query_row(
            "SELECT coalesce(max(rowid),0) FROM verification_runs WHERE project=?1",
            [project],
            |r| r.get(0),
        )?
    } else {
        0
    };
    let runs = if schema >= 5 {
        runs + connection.query_row(
            "SELECT coalesce((SELECT revision FROM verification_revisions WHERE project=?1),0)",
            [project],
            |r| r.get::<_, i64>(0),
        )?
    } else {
        runs
    };
    let renames = connection.query_row(
        "SELECT coalesce(max(rowid),0) FROM renames WHERE project=?1",
        [project],
        |r| r.get(0),
    )?;
    Ok(SiteRevision {
        schema,
        check,
        snapshot,
        plan,
        events,
        runs,
        renames,
    })
}

fn site_legacy_run(
    connection: &Connection,
    project: &str,
    id: &str,
) -> Result<(dctx0_core::VerificationRun, Vec<u8>)> {
    let mut run = verification::codec::read_run(connection, project, id)?.run;
    let (_, _, corpus) = verification::codec::compress_json(&verification::codec::Corpus {
        cases: &run.cases,
        required_cases: &run.required_cases,
    })?;
    run.cases.clear();
    run.required_cases.clear();
    Ok((run, corpus))
}
