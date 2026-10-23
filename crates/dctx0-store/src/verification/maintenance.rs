//! Transactional retention and bounded-memory rebuilding of existing histories.
use super::*;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct VerificationGcReport {
    pub dry_run: bool,
    pub full_runs_before: u64,
    pub full_runs_after: u64,
    pub pruned_runs: u64,
    pub artifacts_before: u64,
    pub artifacts_after: u64,
    /// Stored bytes in run bodies, corpora and artifacts; excludes SQLite overhead.
    pub logical_bytes_before: u64,
    pub logical_bytes_after: u64,
}
pub(super) fn store_body(connection: &Connection, id: &str, run: &VerificationRun) -> Result<()> {
    cases::store(connection, id, run)?;
    connection.execute(
        "UPDATE verification_runs SET data=?2,corpus_hash=NULL,summary=?3 WHERE id=?1",
        params![
            id,
            serde_json::to_string(&codec::Header(run))?,
            serde_json::to_string(&VerificationRunSummary::new(id, run)?)?
        ],
    )?;
    Ok(())
}

fn prunable(connection: &Connection, project: &str) -> Result<Vec<String>> {
    let ids = {
        let sql = if crate::storage::version(connection)? >= 7 {
            "WITH newest(id) AS (SELECT id FROM verification_runs WHERE project=?1 ORDER BY rowid DESC LIMIT 1), problem(id) AS (SELECT id FROM verification_runs WHERE project=?1 AND json_extract(result,'$.status') IN ('FAIL','UNKNOWN') ORDER BY rowid DESC LIMIT 1), pending(id) AS (SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL AND (id IN (SELECT l.run_id FROM verification_task_links l JOIN task_states s ON s.task_id=l.task_id WHERE s.work!='Done') OR EXISTS(SELECT 1 FROM json_each(CASE WHEN json_valid(data) THEN data ELSE '{}' END,'$.provenance.task_pin.requirements') required JOIN task_states s ON s.task_id=CAST(required.key AS INTEGER) WHERE s.work='Progress' OR (s.work='Open' AND s.actor IS NOT NULL)))), seeds(id) AS (SELECT id FROM newest UNION SELECT id FROM problem UNION SELECT id FROM pending), kept(id) AS (SELECT id FROM seeds UNION SELECT p.parent FROM verification_run_parents p JOIN seeds s ON p.child=s.id) SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL AND id NOT IN (SELECT id FROM kept) ORDER BY rowid"
        } else {
            "SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL AND id NOT IN (SELECT id FROM verification_runs WHERE project=?1 ORDER BY rowid DESC LIMIT 3) ORDER BY rowid"
        };
        let mut statement = connection.prepare(sql)?;
        statement
            .query_map([project], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    Ok(ids)
}
pub(crate) fn prune(connection: &Connection, project: &str) -> Result<u64> {
    let ids = prunable(connection, project)?;
    for id in &ids {
        let mut summary = summaries::summary(connection, project, id)?;
        summary.details_available = false;
        let compact = crate::storage::version(connection)? >= 7;
        connection.execute(if compact {"UPDATE verification_runs SET corpus_hash=NULL,summary=?2,pruned_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1"} else {"UPDATE verification_runs SET data='',corpus_hash=NULL,summary=?2,pruned_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1"}, params![id,serde_json::to_string(&summary)?])?;
        if !compact {
            connection.execute("DELETE FROM verification_case_index WHERE run_id=?1", [id])?;
            connection.execute(
                "DELETE FROM verification_requirements WHERE run_id=?1",
                [id],
            )?;
            connection.execute(
                "DELETE FROM verification_case_projections WHERE run_id=?1",
                [id],
            )?;
            connection.execute(
                "DELETE FROM verification_run_artifacts WHERE run_id=?1",
                [id],
            )?;
        }
    }
    if crate::storage::version(connection)? >= 7 {
        connection.execute("UPDATE verification_artifacts SET bytes=x'',encoding='pruned' WHERE media_type NOT IN ('application/vnd.dctx0.runtime-metrics+json','application/vnd.dctx0.runtime-identity+json') AND NOT EXISTS(SELECT 1 FROM verification_run_artifacts l JOIN verification_runs r ON r.id=l.run_id WHERE l.artifact_hash=verification_artifacts.hash AND r.pruned_at IS NULL)",[])?;
    }
    connection.execute("DELETE FROM verification_artifacts WHERE NOT EXISTS(SELECT 1 FROM verification_run_artifacts r WHERE r.artifact_hash=verification_artifacts.hash)", [])?;
    connection.execute("DELETE FROM verification_case_bodies WHERE NOT EXISTS(SELECT 1 FROM verification_case_index i JOIN verification_runs r ON r.id=i.run_id WHERE i.body_hash=verification_case_bodies.hash AND r.pruned_at IS NULL)", [])?;
    connection.execute("DELETE FROM verification_corpora WHERE NOT EXISTS(SELECT 1 FROM verification_runs r WHERE r.corpus_hash=verification_corpora.hash)", [])?;
    connection.execute("INSERT INTO verification_revisions(project,revision) VALUES(?1,1) ON CONFLICT(project) DO UPDATE SET revision=revision+1",[project])?;
    Ok(ids.len() as u64)
}
fn totals(connection: &Connection, project: &str) -> Result<(u64, u64, u64)> {
    let schema: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if schema < 4 {
        return Ok((0, 0, 0));
    }
    let runs = connection.query_row(
        if schema >= 5 {
            "SELECT count(*) FROM verification_runs WHERE project=?1 AND pruned_at IS NULL"
        } else {
            "SELECT count(*) FROM verification_runs WHERE project=?1"
        },
        [project],
        |r| r.get::<_, i64>(0),
    )? as u64;
    let artifacts =
        connection.query_row("SELECT count(*) FROM verification_artifacts", [], |r| {
            r.get::<_, i64>(0)
        })? as u64;
    let mut bytes: u64 = connection.query_row(
        "SELECT coalesce(sum(length(CAST(data AS BLOB))),0) FROM verification_runs",
        [],
        |r| r.get::<_, i64>(0),
    )? as u64;
    bytes += connection.query_row(
        "SELECT coalesce(sum(length(bytes)),0) FROM verification_artifacts",
        [],
        |r| r.get::<_, i64>(0).map(|n| n as u64),
    )?;
    if schema >= 5 {
        bytes += connection.query_row(
            "SELECT coalesce(sum(length(bytes)),0) FROM verification_corpora",
            [],
            |r| r.get::<_, i64>(0).map(|n| n as u64),
        )?;
    }
    if schema >= 6 {
        bytes += connection.query_row(
            "SELECT coalesce(sum(length(bytes)),0) FROM verification_case_bodies",
            [],
            |r| r.get::<_, i64>(0),
        )? as u64;
    }
    Ok((runs, artifacts, bytes))
}
fn copy_tables(tx: &Connection, tables: &[String]) -> Result<()> {
    let mut pending = tables
        .iter()
        .filter(|n| !n.starts_with("verification_"))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut copied = BTreeSet::new();
    while !pending.is_empty() {
        let mut progress = false;
        for name in pending.clone() {
            let quoted = name.replace('"', "\"\"");
            let parents = {
                let mut s = tx.prepare(&format!("PRAGMA source.foreign_key_list(\"{quoted}\")"))?;
                s.query_map([], |r| r.get::<_, String>(2))?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            };
            if parents
                .iter()
                .any(|parent| parent != &name && !copied.contains(parent))
            {
                continue;
            }
            // Explicit FK order also avoids SQLite's deferred-counter/transfer
            // optimization interaction when copying between attached databases.
            tx.execute(
                &format!(
                    "INSERT INTO main.\"{quoted}\" SELECT * FROM source.\"{quoted}\" ORDER BY rowid"
                ),
                [],
            )
            .map_err(|e| invalid(&format!("Copying {name}: {e}")))?;
            copied.insert(name.clone());
            pending.remove(&name);
            progress = true;
        }
        if !progress {
            return Err(invalid("Unsupported cyclic history table dependencies"));
        }
    }
    Ok(())
}
impl Store {
    pub fn verification_storage_version(&self) -> Result<i64> {
        Ok(self
            .connection
            .pragma_query_value(None, "data_version", |r| r.get(0))?)
    }
    /// Checkpoint the old file before replacing it. Holding an exclusive rollback-
    /// journal lock prevents another writer from changing the checked source.
    pub fn install_verification_copy(
        mut self,
        destination: &Path,
        expected_version: i64,
    ) -> Result<()> {
        self.require_write()?;
        let path = self
            .connection
            .path()
            .filter(|path| !path.is_empty())
            .ok_or_else(|| invalid("Compaction requires a file database"))?
            .to_owned();
        let mut lock_path = std::ffi::OsString::from(&path);
        lock_path.push(".lock");
        let guard = match self.maintenance_lock.take() {
            Some(file) => file,
            None => std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(lock_path)?,
        };
        let signature = crate::storage_maintenance::source_signature(&self.connection)?;
        crate::storage::lock_exclusive(&guard)?;
        if crate::storage_maintenance::source_signature(&self.connection)? != signature
            || self.verification_storage_version()? != expected_version
        {
            return Err(invalid("History changed during compaction; rerun gc"));
        }
        self.connection.execute_batch("PRAGMA locking_mode=EXCLUSIVE; PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Exclusive)?;
        let version: i64 = tx.pragma_query_value(None, "data_version", |r| r.get(0))?;
        if version != expected_version {
            return Err(invalid("History changed during compaction; rerun gc"));
        }
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(destination)?
            .sync_all()?;
        tx.rollback()?;
        // The independent maintenance lock survives closing SQLite, including on
        // Windows where an open SQLite handle prevents replacing the database.
        self.connection
            .close()
            .map_err(|(_, error)| Error::Sql(error))?;
        std::fs::rename(destination, &path)?;
        drop(guard);
        Ok(())
    }
    pub fn verification_gc_preview(&self, project: &str) -> Result<VerificationGcReport> {
        let (runs, artifacts, bytes) = totals(&self.connection, project)?;
        let after = if crate::storage::version(&self.connection)? >= 7 {
            runs.saturating_sub(prunable(&self.connection, project)?.len() as u64)
        } else {
            runs.min(3)
        };
        Ok(VerificationGcReport {
            dry_run: true,
            full_runs_before: runs,
            full_runs_after: after,
            pruned_runs: runs.saturating_sub(after),
            artifacts_before: artifacts,
            artifacts_after: artifacts,
            logical_bytes_before: bytes,
            logical_bytes_after: bytes,
        })
    }
    pub fn verification_gc(&mut self, project: &str) -> Result<VerificationGcReport> {
        self.require_write()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (runs, artifacts, bytes) = totals(&tx, project)?;
        let pruned_runs = prune(&tx, project)?;
        let ids = {
            let mut s = tx.prepare("SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL AND corpus_hash IS NULL")?;
            s.query_map([project], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for id in ids {
            let run = codec::read_run(&tx, project, &id)?.run;
            store_body(&tx, &id, &run)?;
        }
        let hashes = {
            let mut s = tx.prepare("SELECT DISTINCT a.hash FROM verification_artifacts a JOIN verification_run_artifacts l ON l.artifact_hash=a.hash JOIN verification_runs r ON r.id=l.run_id WHERE r.project=?1 AND a.raw_length IS NULL")?;
            s.query_map([project], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for hash in hashes {
            let bytes: Vec<u8> = tx.query_row(
                "SELECT bytes FROM verification_artifacts WHERE hash=?1",
                [&hash],
                |r| r.get(0),
            )?;
            if format!("{:x}", Sha256::digest(&bytes)) != hash {
                return Err(invalid("Stored artifact digest is corrupt"));
            }
            let (encoding, packed) = codec::encode(&bytes)?;
            tx.execute("UPDATE verification_artifacts SET bytes=?2,encoding=?3,raw_length=?4 WHERE hash=?1",params![hash,packed,encoding,bytes.len() as i64])?;
        }
        let (after, artifacts_after, logical_bytes_after) = totals(&tx, project)?;
        tx.commit()?;
        Ok(VerificationGcReport {
            dry_run: false,
            full_runs_before: runs,
            full_runs_after: after,
            pruned_runs,
            artifacts_before: artifacts,
            artifacts_after,
            logical_bytes_before: bytes,
            logical_bytes_after,
        })
    }

    /// Build a compact replacement without growing the source WAL or copying discarded blobs.
    /// Caller must own the project lock and install the verified result atomically.
    pub fn verification_compact_copy(
        &self,
        project: &str,
        destination: &Path,
    ) -> Result<VerificationGcReport> {
        if crate::storage::version(&self.connection)? >= 7 {
            let names=self.connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'verification_%'")?.query_map([],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let known = [
                "verification_corpora",
                "verification_artifacts",
                "verification_runs",
                "verification_run_artifacts",
                "verification_task_links",
                "verification_adoptions",
                "verification_revisions",
                "verification_case_bodies",
                "verification_case_index",
                "verification_case_projections",
                "verification_requirements",
                "verification_run_parents",
            ];
            if names.iter().any(|name| !known.contains(&name.as_str())) {
                return Err(invalid(
                    "Unrecognized history table; refusing verification compaction",
                ));
            }
            let (runs, artifacts, bytes) = totals(&self.connection, project)?;
            self.storage_compact_project_copy(Some(project), destination)?;
            let target = Store::open(destination, AccessMode::ReadOnly)?;
            let (after, artifacts_after, logical_bytes_after) =
                totals(&target.connection, project)?;
            return Ok(VerificationGcReport {
                dry_run: false,
                full_runs_before: runs,
                full_runs_after: after,
                pruned_runs: runs.saturating_sub(after),
                artifacts_before: artifacts,
                artifacts_after,
                logical_bytes_before: bytes,
                logical_bytes_after,
            });
        }
        let source = self
            .connection
            .path()
            .filter(|path| !path.is_empty())
            .ok_or_else(|| invalid("Compaction requires a file database"))?;
        if destination.exists() {
            return Err(invalid("Compaction destination already exists"));
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let connection = Connection::open(destination)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        for migration in [
            crate::SCHEMA,
            crate::tasks::MIGRATION,
            crate::tasks::PROFILE_MIGRATION,
            MIGRATION,
            STORAGE_MIGRATION,
            cases::MIGRATION,
        ] {
            connection.execute_batch(migration)?;
        }
        let mut target = Store {
            connection,
            mode: AccessMode::ReadWrite,
            maintenance_lock: None,
        };
        target
            .connection
            .execute_batch("PRAGMA journal_mode=DELETE;")?;
        target
            .connection
            .execute("ATTACH DATABASE ?1 AS source", [source])?;
        let tx = target
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("CREATE TEMP TABLE retained_artifact_links(run_id TEXT NOT NULL,artifact_hash TEXT NOT NULL,PRIMARY KEY(run_id,artifact_hash));")?;
        let schema: i64 = tx.query_row("PRAGMA source.user_version", [], |r| r.get(0))?;
        if schema < 4 {
            return Err(invalid("No verification history to compact"));
        }
        let (runs, artifacts, bytes) = totals(&self.connection, project)?;
        let tables = {
            let mut s = tx.prepare("SELECT name FROM source.sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
            s.query_map([], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for table in &tables {
            let known: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM main.sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )?;
            if !known {
                return Err(invalid(
                    "Unrecognized history table; refusing to discard it",
                ));
            }
        }
        copy_tables(&tx, &tables)?;
        // AUTOINCREMENT high-water marks may exceed the greatest retained ID.
        tx.execute_batch("DELETE FROM main.sqlite_sequence; INSERT INTO main.sqlite_sequence SELECT * FROM source.sqlite_sequence;")?;
        let kept = {
            let mut s = tx.prepare("SELECT id FROM source.verification_runs WHERE project=?1 ORDER BY rowid DESC LIMIT 3")?;
            s.query_map([project], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<BTreeSet<_>, _>>()?
        };
        let mut s = tx.prepare("SELECT rowid,id,project,created_at,origin,result,snapshot_id,check_id FROM source.verification_runs ORDER BY rowid")?;
        let mut rows = s.query([])?;
        let mut pruned_runs = 0;
        while let Some(row) = rows.next()? {
            let id: String = row.get(1)?;
            let owner: String = row.get(2)?;
            let mut summary = summaries::summary(&self.connection, &owner, &id)?;
            let keep = summary.details_available && (owner != project || kept.contains(&id));
            if !keep && summary.details_available {
                pruned_runs += 1;
            }
            summary.details_available = keep;
            let pruned_at = if keep {
                None
            } else {
                Some(
                    summary
                        .pruned_at
                        .clone()
                        .unwrap_or_else(|| "retention".into()),
                )
            };
            tx.execute("INSERT INTO verification_runs(rowid,id,project,created_at,origin,result,snapshot_id,check_id,data,summary,pruned_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'',?9,CASE WHEN ?10='retention' THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE ?10 END)", params![row.get::<_,i64>(0)?,id,owner,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,i64>(7)?,serde_json::to_string(&summary)?,pruned_at])?;
            if keep {
                let run = codec::read_run(&self.connection, &owner, &id)?.run;
                if codec::json_hash(&run)? != id {
                    return Err(invalid("Stored run identity is corrupt"));
                }
                store_body(&tx, &id, &run)?;
                tx.execute("INSERT INTO temp.retained_artifact_links SELECT * FROM source.verification_run_artifacts WHERE run_id=?1",[&id])?;
            }
        }
        drop(rows);
        drop(s);
        for table in ["verification_task_links", "verification_adoptions"] {
            tx.execute(
                &format!("INSERT INTO main.{table} SELECT * FROM source.{table}"),
                [],
            )?;
        }
        if schema >= 5 {
            tx.execute(
                "INSERT INTO verification_revisions SELECT * FROM source.verification_revisions",
                [],
            )?;
        }
        tx.execute("INSERT INTO verification_revisions(project,revision) VALUES(?1,1) ON CONFLICT(project) DO UPDATE SET revision=revision+1",[project])?;
        let columns = if schema >= 5 {
            "a.encoding,a.raw_length"
        } else {
            "'identity',NULL"
        };
        let mut s = tx.prepare(&format!("SELECT a.hash,a.media_type,a.bytes,{columns} FROM source.verification_artifacts a WHERE EXISTS(SELECT 1 FROM temp.retained_artifact_links l WHERE l.artifact_hash=a.hash)"))?;
        let mut rows = s.query([])?;
        while let Some(row) = rows.next()? {
            let hash: String = row.get(0)?;
            let bytes = codec::decode_bytes(&row.get::<_, String>(3)?, row.get(2)?, row.get(4)?)?;
            if format!("{:x}", Sha256::digest(&bytes)) != hash {
                return Err(invalid("Stored artifact digest is corrupt"));
            }
            let (encoding, packed) = codec::encode(&bytes)?;
            tx.execute("INSERT INTO verification_artifacts(hash,media_type,bytes,encoding,raw_length) VALUES(?1,?2,?3,?4,?5)",params![hash,row.get::<_,String>(1)?,packed,encoding,bytes.len() as i64])?;
        }
        drop(rows);
        drop(s);
        tx.execute("INSERT INTO main.verification_run_artifacts SELECT * FROM temp.retained_artifact_links",[])?;
        if tx
            .prepare("PRAGMA main.foreign_key_check")?
            .query([])?
            .next()?
            .is_some()
        {
            return Err(invalid("Compacted history has invalid foreign keys"));
        }
        let (after, artifacts_after, logical_bytes_after) = totals(&tx, project)?;
        tx.commit()
            .map_err(|e| invalid(&format!("Committing compact history: {e}")))?;
        target.connection.execute("DETACH DATABASE source", [])?;
        let integrity: String = target
            .connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if integrity != "ok" {
            return Err(invalid("Compacted history failed integrity check"));
        }
        target
            .connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")?;
        Ok(VerificationGcReport {
            dry_run: false,
            full_runs_before: runs,
            full_runs_after: after,
            pruned_runs,
            artifacts_before: artifacts,
            artifacts_after,
            logical_bytes_before: bytes,
            logical_bytes_after,
        })
    }
}
