//! Explicit copy migration. The source connection never writes during conversion.
use crate::*;
use serde::Serialize;
use std::path::Path;

const PAYLOADS: &[(&str, &str, &str)] = &[
    ("snapshots", "data", "id"),
    ("tasks", "data", "id"),
    ("task_plans", "data", "id"),
    ("task_events", "state_data", "seq"),
    ("task_states", "data", "task_id"),
    ("execution_bundles", "data", "hash"),
    ("task_groups", "data", "id"),
    ("task_profile_updates", "data", "hash"),
    ("verification_adoptions", "data", "run_id"),
];
#[derive(Debug, Serialize)]
pub struct StorageTable {
    pub name: String,
    pub bytes: u64,
    pub payload_bytes: u64,
}
#[derive(Debug, Serialize)]
pub struct StorageReport {
    pub schema_version: i64,
    pub file_bytes: u64,
    pub free_bytes: u64,
    pub tables: Vec<StorageTable>,
}
#[derive(Debug, Serialize)]
pub struct StorageVerification {
    pub compared_rows: u64,
    pub compared_tables: usize,
    pub integrity_ok: bool,
}
impl Store {
    pub fn storage_recover_file(path: &Path, backup: &Path) -> Result<bool> {
        let mut lock = path.as_os_str().to_os_string();
        lock.push(".lock");
        let guard = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock)?;
        crate::storage::lock_exclusive(&guard)?;
        let restored = !path.exists();
        if restored {
            let source = Store::open(backup, AccessMode::ReadOnly)?;
            let result: String =
                source
                    .connection
                    .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            if result != "ok" {
                return Err(Error::Verification(
                    "Recovery backup failed integrity validation".into(),
                ));
            }
            let temporary = path.with_extension("recover.sqlite3");
            source.storage_backup(&temporary)?;
            std::fs::rename(temporary, path)?;
        }
        let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let result: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if result != "ok" {
            return Err(Error::Verification(
                "Recovery source failed integrity validation".into(),
            ));
        }
        #[cfg(unix)]
        std::fs::File::open(path.parent().unwrap_or(Path::new(".")))?.sync_all()?;
        Ok(restored)
    }
    pub fn storage_verify_copy(&self, destination: &Path) -> Result<StorageVerification> {
        let tx = self.connection.unchecked_transaction()?;
        let target = Store::open(destination, AccessMode::ReadOnly)?;
        let target_tx = target.connection.unchecked_transaction()?;
        let mut compared_rows = 0;
        let mut compared_tables = 0;
        for &(table, payload, key) in PAYLOADS {
            let present: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )?;
            if !present {
                let rows: i64 =
                    target_tx
                        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
                if rows != 0 {
                    return Err(Error::Verification(format!(
                        "Unexpected new rows in {table}"
                    )));
                }
                continue;
            }
            compared_tables += 1;
            let mut source = tx.prepare(&format!("SELECT * FROM {table} ORDER BY {key}"))?;
            let names = source
                .column_names()
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let mut source_rows = source.query([])?;
            let mut other = target_tx.prepare(&format!("SELECT * FROM {table} ORDER BY {key}"))?;
            let mut target_rows = other.query([])?;
            loop {
                match (source_rows.next()?, target_rows.next()?) {
                    (None, None) => break,
                    (Some(a), Some(b)) => {
                        for (i, name) in names.iter().enumerate() {
                            let left: rusqlite::types::Value = a.get(i)?;
                            let right: rusqlite::types::Value = b.get(i)?;
                            let equal = if name == payload {
                                match (&left, &right) {
                                    (
                                        rusqlite::types::Value::Text(a),
                                        rusqlite::types::Value::Text(b),
                                    ) => {
                                        storage::load::<serde_json::Value>(&tx, a)?
                                            == storage::load::<serde_json::Value>(&target_tx, b)?
                                    }
                                    _ => left == right,
                                }
                            } else {
                                left == right
                            };
                            if !equal {
                                return Err(Error::Verification(format!(
                                    "Storage copy differs: {table}/{:?} column {name}",
                                    a.get_ref(0)?
                                )));
                            }
                        }
                        compared_rows += 1;
                    }
                    _ => {
                        return Err(Error::Verification(format!(
                            "Storage copy row count differs: {table}"
                        )));
                    }
                }
            }
        }
        let result: String = target_tx.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if result != "ok" || target_tx.prepare("PRAGMA foreign_key_check")?.exists([])? {
            return Err(Error::Verification(
                "Storage copy integrity check failed".into(),
            ));
        }
        target_tx.commit()?;
        tx.commit()?;
        Ok(StorageVerification {
            compared_rows,
            compared_tables,
            integrity_ok: true,
        })
    }

    pub fn install_storage_copy(
        mut self,
        destination: &Path,
        expected_version: i64,
        backup: &Path,
    ) -> Result<()> {
        let guard = self.maintenance_lock.take().ok_or_else(|| {
            Error::Verification("File storage requires a maintenance lock".into())
        })?;
        let signature = source_signature(&self.connection)?;
        crate::storage::lock_exclusive(&guard)?;
        if source_signature(&self.connection)? != signature
            || self.verification_storage_version()? != expected_version
        {
            return Err(Error::Verification(
                "Storage changed during conversion".into(),
            ));
        }
        self.storage_backup(backup)?;
        let path = self
            .connection
            .path()
            .ok_or_else(|| Error::Verification("Missing database path".into()))?
            .to_owned();
        self.connection
            .close()
            .map_err(|(_, error)| Error::Sql(error))?;
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let writer = Store {
            connection,
            mode: AccessMode::ReadWrite,
            maintenance_lock: Some(guard),
        };
        let version = writer.verification_storage_version()?;
        writer.install_verification_copy(destination, version)
    }
    pub fn storage_backup(&self, destination: &Path) -> Result<()> {
        if destination.exists() {
            return Err(Error::Verification(
                "Backup destination already exists".into(),
            ));
        }
        let mut copy = Connection::open(destination)?;
        let backup = rusqlite::backup::Backup::new(&self.connection, &mut copy)?;
        backup.run_to_completion(256, Duration::ZERO, None)?;
        drop(backup);
        drop(copy);
        std::fs::File::open(destination)?.sync_all()?;
        #[cfg(unix)]
        std::fs::File::open(destination.parent().unwrap_or(Path::new(".")))?.sync_all()?;
        Ok(())
    }
    pub fn storage_inspect(&self) -> Result<StorageReport> {
        let c = &self.connection;
        let page: i64 = c.pragma_query_value(None, "page_size", |r| r.get(0))?;
        let pages: i64 = c.pragma_query_value(None, "page_count", |r| r.get(0))?;
        let free: i64 = c.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
        let tables=c.prepare("SELECT name,sum(pgsize),sum(payload) FROM dbstat GROUP BY name ORDER BY sum(pgsize) DESC")?.query_map([],|r|Ok(StorageTable{name:r.get(0)?,bytes:r.get::<_,i64>(1)? as u64,payload_bytes:r.get::<_,i64>(2)? as u64}))?.collect::<std::result::Result<_,_>>()?;
        Ok(StorageReport {
            schema_version: storage::version(c)?,
            file_bytes: (page * pages) as u64,
            free_bytes: (page * free) as u64,
            tables,
        })
    }
    /// A consistent SQLite backup, then per-row transactional conversion. A failed
    /// destination is retained for diagnosis; the source is never replaced here.
    pub fn storage_compact_copy(&self, destination: &Path) -> Result<StorageReport> {
        self.storage_compact_project_copy(None, destination)
    }
    pub fn storage_compact_project_copy(
        &self,
        retention_project: Option<&str>,
        destination: &Path,
    ) -> Result<StorageReport> {
        let signature = format!(
            "{}:{retention_project:?}",
            source_signature(&self.connection)?
        );
        let resumed = destination.exists();
        if resumed {
            let existing =
                Connection::open_with_flags(destination, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let state: Option<(String, String)> = existing
                .query_row(
                    "SELECT source_signature,status FROM storage_conversion WHERE id=1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if state
                .as_ref()
                .is_none_or(|(old, status)| old != &signature || status != "incomplete")
            {
                return Err(Error::Verification(
                    "Storage destination already exists or its source changed".into(),
                ));
            }
        } else {
            self.storage_backup(destination)?;
        }
        let mut target = Store::open(destination, AccessMode::ReadWrite)?;
        target
            .connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        if storage::version(&target.connection)? < 7 {
            let tx = target
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(storage::MIGRATION)?;
            tx.commit()?;
        }
        target.connection.execute_batch(storage::chunks::SCHEMA)?;
        target.connection.execute_batch(storage::PARENT_SCHEMA)?;
        target.connection.execute_batch("CREATE TABLE IF NOT EXISTS storage_conversion(id INTEGER PRIMARY KEY CHECK(id=1),source_signature TEXT NOT NULL,status TEXT NOT NULL);")?;
        target.connection.execute(
            "INSERT INTO storage_conversion VALUES(1,?1,'incomplete') ON CONFLICT(id) DO UPDATE SET source_signature=excluded.source_signature,status=excluded.status",
            [&signature],
        )?;

        for &(table, column, _) in PAYLOADS {
            convert(&mut target.connection, table, column)?;
        }
        let mut seq = 0;
        loop {
            let row:Option<(i64,i64,String)>=target.connection.query_row("SELECT seq,task_id,state_data FROM task_events WHERE seq>?1 AND state_data IS NOT NULL ORDER BY seq LIMIT 1",[seq],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            let Some((next, task, data)) = row else { break };
            let ready: bool = target.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM storage_state_metadata WHERE event_seq=?1)",
                [next],
                |r| r.get(0),
            )?;
            if ready {
                seq = next;
                continue;
            }
            let state: dctx0_tasks::State = storage::load(&target.connection, &data)?;
            let tx = target.connection.transaction()?;
            storage::state_metadata(&tx, next, task, &state)?;
            tx.commit()?;
            seq = next;
        }
        let projects = target
            .connection
            .prepare("SELECT project FROM projects")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for project in projects {
            target.prepare_verification_queries(&project)?;
            let snapshots=target.connection.prepare("SELECT DISTINCT snapshot_id FROM checks WHERE project=?1 AND snapshot_id IS NOT NULL")?.query_map([&project],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
            for snapshot in snapshots {
                target.prepare_query_snapshot(&project, &snapshot)?;
            }
            let retained = target
                .connection
                .prepare("SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL")?
                .query_map([&project], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for id in retained {
                let header: String = target.connection.query_row(
                    "SELECT data FROM verification_runs WHERE id=?1",
                    [&id],
                    |r| r.get(0),
                )?;
                let header: dctx0_core::VerificationRun = decode(&header)?;
                let mut parents = Vec::new();
                for reference in &header.artifacts {
                    if reference.media_type == "application/vnd.dctx0.native-parent+json" {
                        parents.push(target.verification_artifact(&id, &reference.hash)?);
                    }
                }
                let tx = target.connection.transaction()?;
                verification::register_parents(&tx, &project, &id, &parents, None)?;
                tx.commit()?;
            }
            let tx = target.connection.transaction()?;
            if retention_project.is_none_or(|selected| selected == project) {
                verification::maintenance::prune(&tx, &project)?;
            }
            tx.commit()?;
            let retained = target
                .connection
                .prepare("SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL")?
                .query_map([&project], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for id in retained {
                let stored = target.verification_run(&project, &id)?;
                for expected in &stored.run.artifacts {
                    let artifact = target.verification_artifact(&id, &expected.hash)?;
                    if artifact.reference != *expected {
                        return Err(Error::Verification(
                            "Retained artifact metadata differs from its run".into(),
                        ));
                    }
                }
            }
        }
        target.connection.execute_batch(
            "DELETE FROM edges; DELETE FROM nodes; DELETE FROM task_dependencies;",
        )?;
        storage_gc(&target.connection)?;
        let integrity: String = target
            .connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if integrity != "ok" {
            return Err(Error::Verification(format!(
                "Storage integrity check: {integrity}"
            )));
        }
        if target
            .connection
            .prepare("PRAGMA foreign_key_check")?
            .exists([])?
        {
            return Err(Error::Verification(
                "Storage foreign key check failed".into(),
            ));
        }
        target.connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; VACUUM; PRAGMA optimize;")?;
        if format!(
            "{}:{retention_project:?}",
            source_signature(&self.connection)?
        ) != signature
        {
            return Err(Error::Verification(
                "Storage source changed during conversion".into(),
            ));
        }
        target.connection.execute(
            "UPDATE storage_conversion SET status='complete' WHERE id=1",
            [],
        )?;
        target.storage_inspect()
    }
}
fn convert(c: &mut Connection, table: &str, column: &str) -> Result<()> {
    let mut cursor = 0;
    loop {
        let row:Option<(i64,String)>=c.query_row(&format!("SELECT rowid,{column} FROM {table} WHERE rowid>?1 AND {column} IS NOT NULL ORDER BY rowid LIMIT 1"),[cursor],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let Some((id, raw)) = row else { break };
        let value: serde_json::Value = storage::load(c, &raw)?;
        if raw.starts_with("@dctx0:7:") {
            cursor = id;
            continue;
        }
        let tx = c.transaction()?;
        let packed = storage::store(&tx, &value)?;
        let restored: serde_json::Value = storage::load(&tx, &packed)?;
        if restored != value {
            return Err(Error::Verification(format!(
                "Storage reconstruction differs: {table}/{id}"
            )));
        }
        tx.execute(
            &format!("UPDATE {table} SET {column}=?1 WHERE rowid=?2"),
            params![packed, id],
        )?;
        tx.commit()?;
        cursor = id;
    }
    Ok(())
}
fn storage_gc(c: &Connection) -> Result<()> {
    c.execute_batch("CREATE TEMP TABLE storage_roots(id INTEGER PRIMARY KEY);")?;
    for &(table, column, _) in PAYLOADS {
        c.execute(&format!("INSERT OR IGNORE INTO storage_roots SELECT CAST(substr({column},10) AS INTEGER) FROM {table} WHERE {column} LIKE '@dctx0:7:%'"),[])?;
    }
    c.execute_batch("CREATE TEMP TABLE storage_reachable(id INTEGER PRIMARY KEY); WITH RECURSIVE reach(id) AS (SELECT id FROM storage_roots UNION SELECT e.child FROM storage_edges e JOIN reach r ON e.parent=r.id) INSERT INTO storage_reachable SELECT id FROM reach; DELETE FROM storage_edges WHERE parent NOT IN (SELECT id FROM storage_reachable); DELETE FROM storage_chunk_refs WHERE parent NOT IN (SELECT id FROM storage_reachable); DELETE FROM storage_objects WHERE id NOT IN (SELECT id FROM storage_reachable); DELETE FROM storage_chunks WHERE NOT EXISTS(SELECT 1 FROM storage_chunk_refs r WHERE r.chunk=storage_chunks.id); DROP TABLE storage_roots; DROP TABLE storage_reachable;")?;
    Ok(())
}

pub(crate) fn source_signature(c: &Connection) -> Result<String> {
    use sha2::{Digest, Sha256};
    let path = c
        .path()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Error::Verification("Storage conversion requires a file database".into()))?;
    let mut hash = Sha256::new();
    for name in [path.to_owned(), format!("{path}-wal")] {
        hash.update(name.as_bytes());
        match std::fs::metadata(&name) {
            Ok(meta) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    hash.update(meta.dev().to_le_bytes());
                    hash.update(meta.ino().to_le_bytes());
                }
                hash.update(meta.len().to_le_bytes());
                hash.update(
                    meta.modified()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| Error::Verification(e.to_string()))?
                        .as_nanos()
                        .to_le_bytes(),
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => hash.update(b"missing"),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}
