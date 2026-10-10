//! Immutable run/artifact history. Recording alone never changes task state.
use super::*;
use dctx0_core::{VerificationArtifactRef, VerificationResult, VerificationRun};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) const MIGRATION: &str = "
CREATE TABLE verification_artifacts(hash TEXT PRIMARY KEY,media_type TEXT NOT NULL,bytes BLOB NOT NULL);
CREATE TABLE verification_runs(id TEXT PRIMARY KEY,project TEXT NOT NULL REFERENCES projects(project),snapshot_id TEXT NOT NULL REFERENCES snapshots(id),check_id INTEGER NOT NULL REFERENCES checks(id),origin TEXT NOT NULL,result TEXT NOT NULL,data TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')));
CREATE INDEX verification_runs_project ON verification_runs(project,created_at,id);
CREATE TABLE verification_run_artifacts(run_id TEXT NOT NULL REFERENCES verification_runs(id),artifact_hash TEXT NOT NULL REFERENCES verification_artifacts(hash),PRIMARY KEY(run_id,artifact_hash));
CREATE TABLE verification_task_links(run_id TEXT NOT NULL REFERENCES verification_runs(id),task_id INTEGER NOT NULL REFERENCES tasks(id),requirements_hash TEXT NOT NULL,PRIMARY KEY(run_id,task_id));
CREATE TABLE verification_adoptions(run_id TEXT PRIMARY KEY REFERENCES verification_runs(id),data TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')));
PRAGMA user_version=4;";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationArtifact {
    pub reference: VerificationArtifactRef,
    pub contents: Vec<u8>,
}
impl VerificationArtifact {
    pub fn new(media_type: &str, contents: Vec<u8>) -> Self {
        Self {
            reference: VerificationArtifactRef {
                hash: format!("{:x}", Sha256::digest(&contents)),
                media_type: media_type.into(),
                bytes: contents.len() as u64,
            },
            contents,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredVerificationRun {
    pub id: String,
    pub created_at: String,
    pub run: VerificationRun,
}
fn invalid(message: &str) -> Error {
    Error::Verification(message.into())
}
fn validate(run: &VerificationRun, artifacts: &[VerificationArtifact]) -> Result<()> {
    if run.schema_version != 1
        || run.provenance.check_occurrence <= 0
        || run.selected_subjects.is_empty()
        || run.required_subjects.is_empty()
        || !run.selected_subjects.is_subset(&run.required_subjects)
        || run.started_at_unix_ns.is_empty()
        || !run.started_at_unix_ns.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid(
            "Run must have a versioned nonempty scope, occurrence and lossless execution time",
        ));
    }
    let mut seen = BTreeSet::new();
    for case in &run.cases {
        if case.id.is_empty()
            || !seen.insert(case.id.clone())
            || !run.required_cases.contains(&case.id)
            || !run.required_subjects.contains(&case.subject)
        {
            return Err(invalid("Duplicate, surplus or unscoped verification case"));
        }
    }
    let failed = run
        .cases
        .iter()
        .any(|c| matches!(c.result, VerificationResult::Fail { .. }));
    let incomplete = run.required_cases.is_empty()
        || seen != run.required_cases
        || !run.open_obligations.is_empty()
        || run
            .cases
            .iter()
            .any(|c| matches!(c.result, VerificationResult::Unknown { .. }));
    if matches!(run.result, VerificationResult::Pass) && (failed || incomplete)
        || matches!(run.result, VerificationResult::Unknown { .. }) && failed
    {
        return Err(invalid(
            "Verdict contradicts mandatory case coverage/outcomes",
        ));
    }
    let mut hashes = BTreeSet::new();
    for artifact in artifacts {
        if artifact.reference
            != VerificationArtifact::new(&artifact.reference.media_type, artifact.contents.clone())
                .reference
            || artifact.reference.media_type.is_empty()
            || !hashes.insert(artifact.reference.hash.clone())
        {
            return Err(invalid(
                "Artifact digest, length, media type or uniqueness is invalid",
            ));
        }
    }
    let references = run
        .artifacts
        .iter()
        .map(|a| a.hash.clone())
        .collect::<BTreeSet<_>>();
    if references != hashes
        || references.len() != run.artifacts.len()
        || run
            .artifacts
            .iter()
            .any(|a| !artifacts.iter().any(|artifact| &artifact.reference == a))
        || !references.contains(&run.provenance.plan_hash)
    {
        return Err(invalid(
            "Every immutable artifact, including the pinned plan, must be supplied exactly",
        ));
    }
    Ok(())
}
impl Store {
    pub fn record_verification_run(
        &mut self,
        project: &str,
        run: &VerificationRun,
        artifacts: &[VerificationArtifact],
    ) -> Result<StoredVerificationRun> {
        self.require_write()?;
        validate(run, artifacts)?;
        let data = serde_json::to_string(run)?;
        let id = format!("{:x}", Sha256::digest(data.as_bytes()));
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let check: Option<(String, Option<String>, String)> = tx
            .query_row(
                "SELECT project,snapshot_id,status FROM checks WHERE id=?1",
                [run.provenance.check_occurrence],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if check.as_ref().is_none_or(|(p, s, status)| {
            p != project
                || s.as_deref() != Some(run.provenance.snapshot.as_str())
                || status != "pass"
        }) {
            return Err(invalid(
                "Run does not match its exact successful project check occurrence",
            ));
        }
        for artifact in artifacts {
            let existing: Option<(String, Vec<u8>)> = tx
                .query_row(
                    "SELECT media_type,bytes FROM verification_artifacts WHERE hash=?1",
                    [&artifact.reference.hash],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if existing.is_some_and(|(media, bytes)| {
                media != artifact.reference.media_type || bytes != artifact.contents
            }) {
                return Err(invalid(
                    "Existing artifact content/metadata conflicts with its immutable digest",
                ));
            }
            tx.execute("INSERT OR IGNORE INTO verification_artifacts(hash,media_type,bytes) VALUES(?1,?2,?3)",params![artifact.reference.hash,artifact.reference.media_type,artifact.contents])?;
        }
        tx.execute("INSERT OR IGNORE INTO verification_runs(id,project,snapshot_id,check_id,origin,result,data) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,project,run.provenance.snapshot,run.provenance.check_occurrence,serde_json::to_string(&run.origin)?,serde_json::to_string(&run.result)?,data])?;
        for artifact in artifacts {
            tx.execute("INSERT OR IGNORE INTO verification_run_artifacts(run_id,artifact_hash) VALUES(?1,?2)",params![id,artifact.reference.hash])?;
        }
        tx.commit()?;
        self.verification_run(project, &id)
    }
    pub fn verification_run(&self, project: &str, id: &str) -> Result<StoredVerificationRun> {
        if !self.has_verification_history()? {
            return Err(Error::Missing(format!("Verification run {id}")));
        }
        let row: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT created_at,data FROM verification_runs WHERE project=?1 AND id=?2",
                params![project, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (created_at, data) =
            row.ok_or_else(|| Error::Missing(format!("Verification run {id}")))?;
        Ok(StoredVerificationRun {
            id: id.into(),
            created_at,
            run: decode(&data)?,
        })
    }
    pub fn verification_runs(&self, project: &str) -> Result<Vec<StoredVerificationRun>> {
        if !self.has_verification_history()? {
            return Ok(vec![]);
        }
        let mut statement=self.connection.prepare("SELECT id,created_at,data FROM verification_runs WHERE project=?1 ORDER BY created_at,id")?;
        let rows = statement.query_map([project], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, created_at, data) = row?;
            Ok(StoredVerificationRun {
                id,
                created_at,
                run: decode(&data)?,
            })
        })
        .collect()
    }
    pub fn verification_artifact(&self, run_id: &str, hash: &str) -> Result<VerificationArtifact> {
        if !self.has_verification_history()? {
            return Err(Error::Missing(format!("Verification artifact {hash}")));
        }
        let row:Option<(String,Vec<u8>)>=self.connection.query_row("SELECT a.media_type,a.bytes FROM verification_artifacts a JOIN verification_run_artifacts r ON r.artifact_hash=a.hash WHERE r.run_id=?1 AND a.hash=?2",params![run_id,hash],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let (media, bytes) =
            row.ok_or_else(|| Error::Missing(format!("Verification artifact {hash}")))?;
        let artifact = VerificationArtifact::new(&media, bytes);
        if artifact.reference.hash != hash {
            return Err(invalid("Stored artifact digest is corrupt"));
        }
        Ok(artifact)
    }
    fn has_verification_history(&self) -> Result<bool> {
        Ok(self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))?
            == 4)
    }
}
