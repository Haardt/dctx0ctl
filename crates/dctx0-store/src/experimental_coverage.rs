//! Opt-in research data, independent of specification checks and task evidence.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageExecution {
    pub test_id: String,
    pub outcome: String,
    pub duration_ms: f64,
    pub peak_rss_bytes: Option<u64>,
    pub provenance: serde_json::Value,
    pub llvm_export: serde_json::Value,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CoverageRecord {
    pub id: i64,
    pub execution: CoverageExecution,
}
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS experimental_coverage_executions_v1(
 id INTEGER PRIMARY KEY, project TEXT NOT NULL, test_id TEXT NOT NULL,
 data TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')));
CREATE INDEX IF NOT EXISTS experimental_coverage_project_v1 ON experimental_coverage_executions_v1(project,id);
CREATE TABLE IF NOT EXISTS experimental_coverage_lines_v1(
 execution INTEGER NOT NULL REFERENCES experimental_coverage_executions_v1(id),
 file TEXT NOT NULL, line INTEGER NOT NULL, PRIMARY KEY(execution,file,line));
CREATE INDEX IF NOT EXISTS experimental_coverage_lookup_v1 ON experimental_coverage_lines_v1(file,line,execution);";
impl Store {
    /// Additive extension schema, created only by this explicit experimental write.
    pub fn record_coverage(
        &mut self,
        project: &str,
        executions: &[CoverageExecution],
    ) -> Result<Vec<i64>> {
        self.record_coverage_stream(project, executions.iter().cloned().map(Ok))
    }
    /// Stream a batch within one transaction; an invalid record rolls back every row.
    pub fn record_coverage_stream(
        &mut self,
        project: &str,
        executions: impl IntoIterator<Item = Result<CoverageExecution>>,
    ) -> Result<Vec<i64>> {
        self.require_write()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(SCHEMA)?;
        let mut ids = Vec::new();
        for execution in executions {
            let execution = execution?;
            if execution.test_id.is_empty()
                || !matches!(execution.outcome.as_str(), "pass" | "fail" | "timeout")
                || !execution.duration_ms.is_finite()
                || execution.duration_ms < 0.0
                || execution.llvm_export.get("type").and_then(|v| v.as_str())
                    != Some("llvm.coverage.json.export")
            {
                return Err(Error::Verification(
                    "Invalid experimental coverage execution".into(),
                ));
            }
            tx.execute("INSERT INTO experimental_coverage_executions_v1(project,test_id,data) VALUES(?1,?2,?3)", params![project, execution.test_id, serde_json::to_string(&execution)?])?;
            let id = tx.last_insert_rowid();
            // LLVM segments describe intervals. Expand positive intervals, not just their starts.
            if let Some(data) = execution.llvm_export["data"].as_array() {
                for unit in data {
                    if let Some(files) = unit["files"].as_array() {
                        for file in files {
                            let Some(name) = file["filename"].as_str() else {
                                continue;
                            };
                            if let Some(segments) = file["segments"].as_array() {
                                for pair in segments.windows(2) {
                                    let a = &pair[0];
                                    let b = &pair[1];
                                    if a[3].as_bool() == Some(true)
                                        && a[2].as_u64().unwrap_or(0) > 0
                                    {
                                        let start = a[0].as_u64().unwrap_or(0);
                                        let end = b[0]
                                            .as_u64()
                                            .unwrap_or(start)
                                            .saturating_sub(u64::from(b[1].as_u64() == Some(1)));
                                        if start > 0
                                            && end >= start
                                            && end - start < 1_000_000
                                            && end <= u32::MAX as u64
                                        {
                                            for line in start..=end {
                                                tx.execute("INSERT OR IGNORE INTO experimental_coverage_lines_v1 VALUES(?1,?2,?3)", params![id,name,line as i64])?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    }
    pub fn coverage_records(
        &self,
        project: &str,
        file: Option<&str>,
        line: Option<u32>,
        limit: u32,
    ) -> Result<Vec<CoverageRecord>> {
        let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='experimental_coverage_executions_v1')", [], |r| r.get(0))?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut query = self.connection.prepare("SELECT id,data FROM experimental_coverage_executions_v1 e WHERE project=?1 AND (?2 IS NULL OR EXISTS(SELECT 1 FROM experimental_coverage_lines_v1 l WHERE l.execution=e.id AND l.file=?2 AND (?3 IS NULL OR l.line=?3))) ORDER BY id DESC LIMIT ?4")?;
        let rows = query.query_map(params![project, file, line, limit], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, data) = row?;
            Ok(CoverageRecord {
                id,
                execution: serde_json::from_str(&data)?,
            })
        })
        .collect()
    }
}

/// Metadata projection; raw exports and binary profiles are not returned to Rust.
#[derive(Debug)]
pub struct CoverageBaseline {
    pub id: i64,
    pub test_id: String,
    pub revision: Option<String>,
    pub dirty: Option<bool>,
    pub source_hash: Option<String>,
    pub environment_hash: Option<String>,
    pub root: Option<String>,
    pub available: Option<bool>,
    pub toolchain: Option<String>,
    pub outcome: String,
    pub test_name: Option<String>,
    pub package_name: Option<String>,
    pub target_name: Option<String>,
    pub target_kind: Option<String>,
}
impl Store {
    pub fn coverage_baselines(&self, project: &str) -> Result<Vec<CoverageBaseline>> {
        let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='experimental_coverage_executions_v1')", [], |r| r.get(0))?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut query = self.connection.prepare("SELECT id,test_id,json_extract(data,'$.provenance.revision'),json_extract(data,'$.provenance.dirty_worktree'),json_extract(data,'$.provenance.source_fingerprint'),json_extract(data,'$.provenance.environment_hash'),json_extract(data,'$.provenance.project_root'),json_extract(data,'$.provenance.coverage_available'),json_extract(data,'$.provenance.rustc'),json_extract(data,'$.outcome'),json_extract(data,'$.provenance.test_name'),json_extract(data,'$.provenance.package_name'),json_extract(data,'$.provenance.test_target_name'),json_extract(data,'$.provenance.test_target_kind') FROM experimental_coverage_executions_v1 WHERE project=?1 ORDER BY id DESC")?;
        Ok(query
            .query_map([project], |row| {
                Ok(CoverageBaseline {
                    id: row.get(0)?,
                    test_id: row.get(1)?,
                    revision: row.get(2)?,
                    dirty: row.get(3)?,
                    source_hash: row.get(4)?,
                    environment_hash: row.get(5)?,
                    root: row.get(6)?,
                    available: row.get(7)?,
                    toolchain: row.get(8)?,
                    outcome: row.get(9)?,
                    test_name: row.get(10)?,
                    package_name: row.get(11)?,
                    target_name: row.get(12)?,
                    target_kind: row.get(13)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }
    pub fn coverage_range_executions(&self, file: &str, start: u32, end: u32) -> Result<Vec<i64>> {
        let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='experimental_coverage_lines_v1')", [], |r| r.get(0))?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut query = self.connection.prepare("SELECT DISTINCT execution FROM experimental_coverage_lines_v1 WHERE file=?1 AND line BETWEEN ?2 AND ?3 ORDER BY execution")?;
        Ok(query
            .query_map(params![file, start, end], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?)
    }
}
