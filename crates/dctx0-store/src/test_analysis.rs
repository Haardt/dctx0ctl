//! Append-only experimental research extension, independent of main schema versions.
use crate::*;
use dctx0_test_analysis::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AnalysisRecord {
    Snapshot(SourceSnapshot),
    Catalog(TestCatalog),
    Collection(CollectionRun),
    Observation(TestObservation),
    Prediction(SelectionPrediction),
    Evaluation(SelectionEvaluation),
    Replay(ReplayRun),
    Artifact(Artifact),
}
impl AnalysisRecord {
    fn table(&self) -> &'static str {
        match self {
            Self::Snapshot(_) => "source_snapshots",
            Self::Catalog(_) => "catalogs",
            Self::Collection(_) => "collections",
            Self::Observation(_) => "observations",
            Self::Prediction(_) => "predictions",
            Self::Evaluation(_) => "evaluations",
            Self::Replay(_) => "replays",
            Self::Artifact(_) => "artifacts",
        }
    }
    pub fn id(&self) -> String {
        match self {
            Self::Snapshot(s) => s.snapshot_hash.clone(),
            Self::Catalog(s) => s.id.clone(),
            Self::Collection(s) => s.id.clone(),
            Self::Observation(s) => s.id.clone(),
            Self::Prediction(s) => s.id.clone(),
            Self::Evaluation(s) => format!("{}:{}", s.prediction, s.reference_collection),
            Self::Replay(s) => s.id.clone(),
            Self::Artifact(s) => s.sha256.clone(),
        }
    }
}
const TABLES: &[&str] = &[
    "source_snapshots",
    "catalogs",
    "collections",
    "observations",
    "predictions",
    "evaluations",
    "replays",
    "artifacts",
];
impl Store {
    /// Definitions never update. An identical retry is idempotent; conflicting IDs fail.
    /// Events and the derived latest-event cache commit in the same transaction.
    pub fn publish_analysis(
        &mut self,
        project: &str,
        records: &[AnalysisRecord],
        owner: &str,
        event: &ResearchEvent,
    ) -> Result<()> {
        self.require_write()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS experimental_analysis_metadata_v1(version INTEGER PRIMARY KEY CHECK(version=1)); INSERT OR IGNORE INTO experimental_analysis_metadata_v1 VALUES(1);
        CREATE TABLE IF NOT EXISTS experimental_analysis_events_v1(project TEXT NOT NULL,owner TEXT NOT NULL,sequence INTEGER NOT NULL,data TEXT NOT NULL,PRIMARY KEY(project,owner,sequence));
        CREATE TABLE IF NOT EXISTS experimental_analysis_progress_v1(project TEXT NOT NULL,owner TEXT NOT NULL,sequence INTEGER NOT NULL,PRIMARY KEY(project,owner));")?;
        for table in TABLES {
            tx.execute_batch(&format!("CREATE TABLE IF NOT EXISTS experimental_analysis_{table}_v1(project TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(project,id));"))?;
        }
        for record in records {
            let id = record.id();
            if id.is_empty() {
                return Err(Error::Verification("Empty research ID".into()));
            }
            let table = record.table();
            let data = serde_json::to_string(record)?;
            let previous: Option<String> = tx.query_row(
                &format!("SELECT data FROM experimental_analysis_{table}_v1 WHERE project=?1 AND id=?2"),
                params![project, id], |r| r.get(0),
            ).optional()?;
            match previous {
                Some(previous) if previous != data => {
                    return Err(Error::Verification("Immutable research ID conflict".into()));
                }
                Some(_) => (),
                None => {
                    tx.execute(
                        &format!("INSERT INTO experimental_analysis_{table}_v1 VALUES(?1,?2,?3)"),
                        params![project, id, data],
                    )?;
                }
            }
        }
        let current: Option<i64> = tx.query_row("SELECT sequence FROM experimental_analysis_progress_v1 WHERE project=?1 AND owner=?2", params![project,owner], |r| r.get(0)).optional()?;
        let sequence = i64::try_from(event.sequence)
            .map_err(|_| Error::Verification("Event sequence overflow".into()))?;
        if sequence != current.unwrap_or(0) + 1 {
            return Err(Error::Verification(
                "Research event sequence conflict".into(),
            ));
        }
        tx.execute(
            "INSERT INTO experimental_analysis_events_v1 VALUES(?1,?2,?3,?4)",
            params![project, owner, sequence, serde_json::to_string(event)?],
        )?;
        tx.execute("INSERT INTO experimental_analysis_progress_v1 VALUES(?1,?2,?3) ON CONFLICT(project,owner) DO UPDATE SET sequence=excluded.sequence",params![project,owner,sequence])?;
        tx.commit()?;
        Ok(())
    }
    /// Read-only; an absent extension is an empty dataset, never a migration.
    pub fn analysis_records(&self, project: &str) -> Result<Vec<AnalysisRecord>> {
        let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='experimental_analysis_metadata_v1')", [], |r| r.get(0))?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for table in TABLES {
            let mut query = self.connection.prepare(&format!(
                "SELECT data FROM experimental_analysis_{table}_v1 WHERE project=?1 ORDER BY id"
            ))?;
            for row in query.query_map([project], |r| r.get::<_, String>(0))? {
                result.push(serde_json::from_str(&row?)?);
            }
        }
        Ok(result)
    }
    pub fn analysis_events(&self, project: &str, owner: &str) -> Result<Vec<ResearchEvent>> {
        let exists: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='experimental_analysis_events_v1')", [], |r| r.get(0))?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut query = self.connection.prepare("SELECT data FROM experimental_analysis_events_v1 WHERE project=?1 AND owner=?2 ORDER BY sequence")?;
        query
            .query_map(params![project, owner], |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect()
    }
}
