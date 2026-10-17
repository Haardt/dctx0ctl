//! Content-addressed run/artifact history. Retention never changes task state.
use super::*;
use dctx0_core::{VerificationArtifactRef, VerificationResult, VerificationRun};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
pub(crate) mod codec;
mod maintenance;
mod summaries;
pub use maintenance::*;
pub use summaries::*;
pub(crate) const STORAGE_MIGRATION: &str = "
CREATE TABLE verification_corpora(hash TEXT PRIMARY KEY,bytes BLOB NOT NULL,raw_length INTEGER NOT NULL);
ALTER TABLE verification_runs ADD COLUMN corpus_hash TEXT REFERENCES verification_corpora(hash);
ALTER TABLE verification_runs ADD COLUMN summary TEXT;
ALTER TABLE verification_runs ADD COLUMN pruned_at TEXT;
ALTER TABLE verification_artifacts ADD COLUMN encoding TEXT NOT NULL DEFAULT 'identity';
ALTER TABLE verification_artifacts ADD COLUMN raw_length INTEGER;
CREATE INDEX verification_artifact_users ON verification_run_artifacts(artifact_hash);
CREATE TABLE verification_revisions(project TEXT PRIMARY KEY REFERENCES projects(project),revision INTEGER NOT NULL);
PRAGMA user_version=5;";

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
        if artifact.reference.hash != format!("{:x}", Sha256::digest(&artifact.contents))
            || artifact.reference.bytes != artifact.contents.len() as u64
            || artifact.reference.media_type.is_empty()
            || !hashes.insert(artifact.reference.hash.clone())
        {
            return Err(invalid(
                "Artifact digest, length, media type or uniqueness is invalid",
            ));
        }
    }
    let supplied = artifacts
        .iter()
        .map(|a| (a.reference.hash.as_str(), &a.reference))
        .collect::<std::collections::BTreeMap<_, _>>();
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
            .any(|a| supplied.get(a.hash.as_str()).copied() != Some(a))
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
        let (id, created_at) = self.persist_verification_run(project, run, artifacts)?;
        Ok(StoredVerificationRun {
            id,
            created_at,
            run: run.clone(),
        })
    }
    /// Transfer execution results without cloning or re-decoding the complete case corpus.
    pub fn record_verification_run_owned(
        &mut self,
        project: &str,
        run: VerificationRun,
        artifacts: &[VerificationArtifact],
    ) -> Result<StoredVerificationRun> {
        let (id, created_at) = self.persist_verification_run(project, &run, artifacts)?;
        Ok(StoredVerificationRun {
            id,
            created_at,
            run,
        })
    }
    fn persist_verification_run(
        &mut self,
        project: &str,
        run: &VerificationRun,
        artifacts: &[VerificationArtifact],
    ) -> Result<(String, String)> {
        self.require_write()?;
        validate(run, artifacts)?;
        let id = codec::json_hash(run)?;
        let data = serde_json::to_string(&codec::Header(run))?;
        let summary = serde_json::to_string(&VerificationRunSummary::new(&id, run)?)?;
        let (corpus_hash, length, bytes) = codec::compress_json(&codec::Corpus {
            cases: &run.cases,
            required_cases: &run.required_cases,
        })?;
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
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM verification_runs WHERE id=?1)",
            [&id],
            |r| r.get(0),
        )?;
        if exists {
            let pruned: Option<String> = tx.query_row(
                "SELECT pruned_at FROM verification_runs WHERE project=?1 AND id=?2",
                params![project, id],
                |r| r.get(0),
            )?;
            if pruned.is_some() {
                return Err(Error::DetailsPruned(id));
            }
        }
        for artifact in artifacts {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM verification_artifacts WHERE hash=?1)",
                [&artifact.reference.hash],
                |r| r.get(0),
            )?;
            if exists {
                // Artifact may belong to another run; validate it without another content clone.
                let (media, stored, encoding, length): (String, Vec<u8>, String, Option<i64>) = tx.query_row(
                    "SELECT media_type,bytes,encoding,raw_length FROM verification_artifacts WHERE hash=?1", [&artifact.reference.hash],
                    |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
                if media != artifact.reference.media_type
                    || codec::decode_bytes(&encoding, stored, length)? != artifact.contents
                {
                    return Err(invalid(
                        "Existing artifact content/metadata conflicts with its immutable digest",
                    ));
                }
            } else {
                let (encoding, bytes) = codec::encode(&artifact.contents)?;
                tx.execute("INSERT INTO verification_artifacts(hash,media_type,bytes,encoding,raw_length) VALUES(?1,?2,?3,?4,?5)",params![artifact.reference.hash,artifact.reference.media_type,bytes,encoding,artifact.reference.bytes as i64])?;
            }
        }
        if exists {
            tx.commit()?;
            let saved = self.verification_run(project, &id)?;
            return Ok((id, saved.created_at));
        }
        maintenance::insert_corpus(&tx, &corpus_hash, length, &bytes)?;
        tx.execute("INSERT INTO verification_runs(id,project,snapshot_id,check_id,origin,result,data,corpus_hash,summary) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![id,project,run.provenance.snapshot,run.provenance.check_occurrence,serde_json::to_string(&run.origin)?,serde_json::to_string(&run.result)?,data,corpus_hash,summary])?;
        for artifact in artifacts {
            tx.execute(
                "INSERT INTO verification_run_artifacts(run_id,artifact_hash) VALUES(?1,?2)",
                params![id, artifact.reference.hash],
            )?;
        }
        maintenance::prune(&tx, project)?;
        tx.commit()?;
        let created_at = self.connection.query_row(
            "SELECT created_at FROM verification_runs WHERE id=?1",
            [&id],
            |r| r.get(0),
        )?;
        Ok((id, created_at))
    }
    pub fn verification_run(&self, project: &str, id: &str) -> Result<StoredVerificationRun> {
        if !self.has_verification_history()? {
            return Err(Error::Missing(format!("Verification run {id}")));
        }
        codec::read_run(&self.connection, project, id)
    }
    pub fn verification_runs(&self, project: &str) -> Result<Vec<StoredVerificationRun>> {
        if !self.has_verification_history()? {
            return Ok(vec![]);
        }
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        let sql = if version >= 5 {
            "SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL ORDER BY rowid DESC LIMIT 3"
        } else {
            "SELECT id FROM verification_runs WHERE project=?1 ORDER BY rowid DESC LIMIT 3"
        };
        let mut statement = self.connection.prepare(sql)?;
        statement
            .query_map([project], |r| r.get::<_, String>(0))?
            .map(|id| self.verification_run(project, &id?))
            .collect()
    }
    pub fn verification_artifact(&self, run_id: &str, hash: &str) -> Result<VerificationArtifact> {
        if !self.has_verification_history()? {
            return Err(Error::Missing(format!("Verification artifact {hash}")));
        }
        codec::read_artifact(&self.connection, run_id, hash)
    }
    fn has_verification_history(&self) -> Result<bool> {
        Ok(self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))?
            >= 4)
    }
}

#[cfg(test)]
mod tests;
