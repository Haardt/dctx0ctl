//! Transactional local history; malformed or newer databases are never reset.
use dctx0_changes::{Changes, RenameMapping, Snapshot, diff};
pub use dctx0_changes::{HistoryEntry, HistoryReader, HistoryWriter, RenameEvent};
use dctx0_changes::{LineagePath, RenameLineage};
use dctx0_core::Diagnostic;
mod tasks;
pub use tasks::{TaskDelivery, TaskDeliveryPublication, TaskDeliveryPublicationReceipt};
mod verification;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use std::{path::Path, time::Duration};
pub use verification::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Snapshot JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unsupported database schema {0}; expected 1 through 4")]
    Schema(i64),
    #[error("Snapshot not found or ambiguous: {0}")]
    Missing(String),
    #[error(transparent)]
    Task(#[from] dctx0_tasks::Error),
    #[error("Verification history: {0}")]
    Verification(String),
    #[error("History is read-only")]
    ReadOnly,
}
pub type Result<T> = std::result::Result<T, Error>;
/// Opening intent is explicit at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}
pub struct Store {
    connection: Connection,
    mode: AccessMode,
}
const SCHEMA:&str="
CREATE TABLE snapshots(id TEXT PRIMARY KEY,source_hash TEXT NOT NULL,semantic_hash TEXT NOT NULL,data TEXT NOT NULL);
CREATE TABLE projects(project TEXT PRIMARY KEY,latest TEXT REFERENCES snapshots(id));
CREATE TABLE checks(id INTEGER PRIMARY KEY,project TEXT NOT NULL REFERENCES projects(project),snapshot_id TEXT REFERENCES snapshots(id),status TEXT NOT NULL CHECK(status IN ('pass','fail')),source_hash TEXT NOT NULL,diagnostics TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')));
CREATE INDEX checks_project ON checks(project,id);
CREATE TABLE nodes(snapshot_id TEXT NOT NULL REFERENCES snapshots(id),id TEXT NOT NULL,kind TEXT NOT NULL,contract_hash TEXT NOT NULL,behavior_hash TEXT NOT NULL,facts_hash TEXT NOT NULL,PRIMARY KEY(snapshot_id,id));
CREATE TABLE edges(snapshot_id TEXT NOT NULL,source TEXT NOT NULL,target TEXT NOT NULL,kind TEXT NOT NULL,origins TEXT NOT NULL,FOREIGN KEY(snapshot_id,source) REFERENCES nodes(snapshot_id,id),FOREIGN KEY(snapshot_id,target) REFERENCES nodes(snapshot_id,id));
CREATE INDEX edges_target ON edges(snapshot_id,target);
CREATE TABLE renames(id TEXT PRIMARY KEY,project TEXT NOT NULL REFERENCES projects(project),before_id TEXT NOT NULL REFERENCES snapshots(id),after_id TEXT NOT NULL REFERENCES snapshots(id),mappings TEXT NOT NULL);
PRAGMA user_version=1;";
// DSL nesting limits exceed serde_json's default JSON depth because every IR
// level has object/array wrappers. Grow the deserializer stack explicitly.
fn decode<T: serde::de::DeserializeOwned>(json: &str) -> std::result::Result<T, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(json);
    deserializer.disable_recursion_limit();
    let value = T::deserialize(serde_stacker::Deserializer::new(&mut deserializer))?;
    deserializer.end()?;
    Ok(value)
}
fn validate_version(version: i64) -> Result<()> {
    if !matches!(version, 1..=4) {
        return Err(Error::Schema(version));
    }
    Ok(())
}
fn initialize(connection: &mut Connection, version: i64, mode: AccessMode) -> Result<()> {
    if version < 4 && mode == AccessMode::ReadWrite {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Recheck under the write lock: another process may have initialized.
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == 0 {
            tx.execute_batch(SCHEMA)?;
        }
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == 1 {
            tx.execute_batch(tasks::MIGRATION)?;
        }
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == 2 {
            tx.execute_batch(tasks::PROFILE_MIGRATION)?;
        }
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == 3 {
            tx.execute_batch(verification::MIGRATION)?;
        }
        let version = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        validate_version(version)?;
        tx.commit()?;
    } else {
        validate_version(version)?;
    }
    Ok(())
}
impl Store {
    pub fn open(path: &Path, mode: AccessMode) -> Result<Self> {
        let flags = if mode == AccessMode::ReadWrite {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        };
        let mut connection = Connection::open_with_flags(path, flags)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        initialize(&mut connection, version, mode)?;
        if mode == AccessMode::ReadWrite {
            let journal: String =
                connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
            if journal != "wal" {
                connection.pragma_update(None, "journal_mode", "WAL")?;
            }
            connection.pragma_update(None, "synchronous", "FULL")?;
        }
        Ok(Self { connection, mode })
    }
    fn require_write(&self) -> Result<()> {
        if self.mode == AccessMode::ReadWrite {
            Ok(())
        } else {
            Err(Error::ReadOnly)
        }
    }
}

impl HistoryReader for Store {
    type Error = Error;
    fn latest(&self, project: &str) -> Result<Option<Snapshot>> {
        let data:Option<String>=self.connection.query_row("SELECT s.data FROM projects p JOIN snapshots s ON s.id=p.latest WHERE p.project=?1",[project],|r|r.get(0)).optional()?;
        data.map(|s| decode(&s).map_err(Error::from)).transpose()
    }
    fn load(&self, project: &str, id: &str) -> Result<Snapshot> {
        if id == "latest" {
            return self
                .latest(project)?
                .ok_or_else(|| Error::Missing(id.into()));
        }
        if id.len() < 8 || id.len() > 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::Missing(id.into()));
        }
        let mut statement=self.connection.prepare("SELECT DISTINCT s.data FROM snapshots s JOIN checks c ON c.snapshot_id=s.id WHERE c.project=?1 AND s.id LIKE ?2")?;
        let records = statement
            .query_map(params![project, format!("{id}%")], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if records.len() != 1 {
            return Err(Error::Missing(id.into()));
        }
        Ok(decode(&records[0])?)
    }
    fn history(&self, project: &str) -> Result<Vec<HistoryEntry>> {
        let mut statement=self.connection.prepare("SELECT id,snapshot_id,status,source_hash,created_at,diagnostics FROM checks WHERE project=?1 ORDER BY id DESC")?;
        let rows = statement
            .query_map([project], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(run_id, snapshot_id, status, source_hash, timestamp, diagnostics)| {
                    Ok(HistoryEntry {
                        run_id,
                        snapshot_id,
                        status,
                        source_hash,
                        timestamp,
                        diagnostics: decode(&diagnostics)?,
                    })
                },
            )
            .collect()
    }
    fn rename_committed(&self, id: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row("SELECT 1 FROM renames WHERE id=?1", [id], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?
            .is_some())
    }
    fn rename_mappings(
        &self,
        project: &str,
        before: &str,
        after: &str,
    ) -> Result<Vec<RenameMapping>> {
        if before == after {
            return Ok(Vec::new());
        }
        let path =
            LineagePath::between(&self.history(project)?, before, after).map_err(Error::Missing)?;
        let initial = self.load(project, &path.snapshots[0])?;
        let mut lineage = RenameLineage::new(&initial.fingerprints);
        for pair in path.snapshots.windows(2) {
            let recorded: Option<String> = self.connection.query_row(
                "SELECT mappings FROM renames WHERE project=?1 AND before_id=?2 AND after_id=?3 ORDER BY rowid DESC LIMIT 1",
                params![project, pair[0], pair[1]], |row| row.get(0)).optional()?;
            let mappings = recorded
                .map(|json| decode::<Vec<RenameMapping>>(&json))
                .transpose()?
                .unwrap_or_default();
            let current = self.load(project, &pair[1])?;
            lineage.advance(&mappings, &current.fingerprints);
        }
        Ok(lineage.finish(path.reverse))
    }
}

impl HistoryWriter for Store {
    fn failure(
        &mut self,
        project: &str,
        source_hash: &str,
        diagnostics: &[Diagnostic],
    ) -> Result<()> {
        self.require_write()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT OR IGNORE INTO projects(project) VALUES(?1)",
            [project],
        )?;
        tx.execute(
            "INSERT INTO checks(project,status,source_hash,diagnostics) VALUES(?1,'fail',?2,?3)",
            params![project, source_hash, serde_json::to_string(diagnostics)?],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn publish(
        &mut self,
        project: &str,
        snapshot: &Snapshot,
        event: Option<&RenameEvent>,
    ) -> Result<Changes> {
        self.require_write()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before:Option<String>=tx.query_row("SELECT s.data FROM projects p JOIN snapshots s ON s.id=p.latest WHERE p.project=?1",[project],|r|r.get(0)).optional()?;
        let before: Option<Snapshot> = before.as_deref().map(decode).transpose()?;
        let changes = diff(
            before.as_ref(),
            snapshot,
            event.map_or(&[], |e| e.mappings.as_slice()),
        );
        tx.execute("INSERT OR IGNORE INTO snapshots(id,source_hash,semantic_hash,data) VALUES(?1,?2,?3,?4)",params![snapshot.id,snapshot.source_hash,snapshot.semantic_hash,serde_json::to_string(snapshot)?])?;
        for (id, node) in &snapshot.graph.nodes {
            let f = &snapshot.fingerprints[id];
            tx.execute("INSERT OR IGNORE INTO nodes(snapshot_id,id,kind,contract_hash,behavior_hash,facts_hash) VALUES(?1,?2,?3,?4,?5,?6)",params![snapshot.id,id,node.kind.as_str(),f.contract,f.behavior,f.facts])?;
        }
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM edges WHERE snapshot_id=?1",
            [&snapshot.id],
            |r| r.get(0),
        )?;
        if count == 0 {
            for edge in &snapshot.graph.edges {
                tx.execute("INSERT INTO edges(snapshot_id,source,target,kind,origins) VALUES(?1,?2,?3,?4,?5)",params![snapshot.id,edge.from,edge.to,serde_json::to_string(&edge.kind)?,serde_json::to_string(&edge.origins)?])?;
            }
        }
        tx.execute("INSERT INTO projects(project,latest) VALUES(?1,?2) ON CONFLICT(project) DO UPDATE SET latest=excluded.latest",params![project,snapshot.id])?;
        tx.execute("INSERT INTO checks(project,snapshot_id,status,source_hash,diagnostics) VALUES(?1,?2,'pass',?3,'[]')",params![project,snapshot.id,snapshot.source_hash])?;
        let check_id = tx.last_insert_rowid();
        if let Some(event) = event {
            tx.execute("INSERT INTO renames(id,project,before_id,after_id,mappings) VALUES(?1,?2,?3,?4,?5)",params![event.id,project,event.before,event.after,serde_json::to_string(&event.mappings)?])?;
            tx.execute(
                "INSERT INTO check_renames(check_id,rename_id) VALUES(?1,?2)",
                params![check_id, event.id],
            )?;
        }
        tx.commit()?;
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_schema_observation_rechecks_under_transaction_lock() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize(&mut connection, 0, AccessMode::ReadWrite).unwrap();
        connection
            .execute("INSERT INTO projects(project) VALUES('kept')", [])
            .unwrap();
        // Another opener may have observed version 0 before this initialization.
        initialize(&mut connection, 0, AccessMode::ReadWrite).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT project FROM projects", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "kept"
        );
        connection.pragma_update(None, "user_version", 99).unwrap();
        assert!(initialize(&mut connection, 0, AccessMode::ReadWrite).is_err());
    }
}
