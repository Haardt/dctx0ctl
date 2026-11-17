//! Content-addressed run/artifact history. Retention never changes task state.
use super::*;
use dctx0_core::{VerificationArtifactRef, VerificationResult, VerificationRun};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
pub(crate) mod cases;
pub(crate) mod codec;
mod context;
pub(crate) mod maintenance;
mod reuse_observations;
mod summaries;
pub(crate) mod units;
pub use context::*;
pub use maintenance::*;
pub use summaries::*;
/// An immutable retention hold grants no testing or task authority. Once linked
/// to a saved run, publication and GC preserve every detail in that database.
pub const HISTORY_PRESERVATION_MEDIA_TYPE: &str = "application/vnd.dctx0.history-preservation+json";

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

pub(crate) fn validate_incremental(
    c: &Connection,
    project: &str,
    run: &VerificationRun,
    artifacts: &[VerificationArtifact],
) -> Result<Option<String>> {
    use dctx0_verification::incremental::{self, ExternalInputReceipt, IncrementalProof};
    fn unique<'a>(
        artifacts: &'a [VerificationArtifact],
        media: &str,
    ) -> Result<Option<&'a VerificationArtifact>> {
        let mut matching = artifacts.iter().filter(|a| a.reference.media_type == media);
        let first = matching.next();
        if matching.next().is_some() {
            return Err(invalid("Duplicate incremental evidence artifact"));
        }
        Ok(first)
    }
    let receipt_artifact = unique(artifacts, incremental::INCREMENTAL_INPUTS_MEDIA_TYPE)?;
    let proof_artifact = unique(artifacts, incremental::INCREMENTAL_PROOF_MEDIA_TYPE)?;
    if receipt_artifact.is_none() && proof_artifact.is_none() {
        return Ok(None);
    }
    // Imported receipt-only copies remain untrusted reader fixtures. Native
    // authority, including every reuse proof, still requires all guards below.
    if run.origin != dctx0_core::VerificationRunOrigin::Native && proof_artifact.is_none() {
        return Ok(None);
    }
    let receipt_artifact = receipt_artifact
        .ok_or_else(|| invalid("Incremental proof requires an external-input receipt"))?;
    let receipt: ExternalInputReceipt = serde_json::from_slice(&receipt_artifact.contents)?;
    let plan_artifact = unique(artifacts, "application/vnd.dctx0.plan+json")?
        .ok_or_else(|| invalid("Incremental evidence requires a program plan"))?;
    if plan_artifact.reference.hash != run.provenance.plan_hash {
        return Err(invalid("Incremental plan is not the run's pinned plan"));
    }
    let plan: dctx0_verification::ProgramPlan = serde_json::from_slice(&plan_artifact.contents)?;
    let required = dctx0_verification::case_manifest(&plan).required_cases;
    // Assigned adapter cohorts retain available inputs for diagnosis, but they
    // are not a complete execution and cannot publish a reuse proof.
    let diagnostic_cohort = proof_artifact.is_none()
        && run.result != dctx0_core::VerificationResult::Pass
        && run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "HYBRID_COHORT_REQUIRED")
        && run.required_cases.is_subset(&required);
    if plan.snapshot != run.provenance.snapshot
        || plan.check_occurrence as i64 != run.provenance.check_occurrence
        || plan.budgets != run.budgets
        || (required != run.required_cases && !diagnostic_cohort)
        || plan.scope.subjects != run.selected_subjects
        || plan.closure != run.required_subjects
    {
        return Err(invalid("Incremental plan does not match run provenance"));
    }
    let units = incremental::receipt_units(&plan, &receipt)
        .ok_or_else(|| invalid("Invalid deterministic input receipt or units"))?;
    let Some(proof_artifact) = proof_artifact else {
        return Ok(None);
    };
    let proof: IncrementalProof = serde_json::from_slice(&proof_artifact.contents)?;
    let contrary = reuse_observations::later_nonpass(c, project, &proof.parent_run)?;
    if !proof.plan.reused.is_disjoint(&contrary) {
        return Err(invalid(
            "LATER_NONPASS_OBSERVATION: historical PASS is contradicted by a later native observation",
        ));
    }
    let parent = codec::read_run(c, project, &proof.parent_run)?;
    let old = &parent.run;
    let old_plan_ref = old
        .artifacts
        .iter()
        .find(|a| {
            a.hash == old.provenance.plan_hash && a.media_type == "application/vnd.dctx0.plan+json"
        })
        .ok_or_else(|| invalid("Parent has no pinned program plan"))?;
    let old_receipts: Vec<_> = old
        .artifacts
        .iter()
        .filter(|a| a.media_type == incremental::INCREMENTAL_INPUTS_MEDIA_TYPE)
        .collect();
    if old_receipts.len() != 1 {
        return Err(invalid("Parent and current input receipts differ"));
    }
    let old_plan_artifact = codec::read_artifact(c, &parent.id, &old_plan_ref.hash)?;
    let old_receipt_artifact = codec::read_artifact(c, &parent.id, &old_receipts[0].hash)?;
    let prior_receipt: ExternalInputReceipt =
        serde_json::from_slice(&old_receipt_artifact.contents)?;
    if old_plan_artifact.reference != *old_plan_ref
        || old_receipt_artifact.reference != *old_receipts[0]
        || incremental::code_receipt(&prior_receipt) != incremental::code_receipt(&receipt)
    {
        return Err(invalid("Parent incremental artifacts differ"));
    }
    let old_plan: dctx0_verification::ProgramPlan =
        serde_json::from_slice(&old_plan_artifact.contents)?;
    if old_plan.snapshot != old.provenance.snapshot
        || old_plan.check_occurrence as i64 != old.provenance.check_occurrence
        || old_plan.budgets != old.budgets
        || dctx0_verification::case_manifest(&old_plan).required_cases != old.required_cases
        || old_plan.scope.subjects != old.selected_subjects
        || old_plan.closure != old.required_subjects
        || incremental::receipt_units(&old_plan, &receipt).as_ref() != Some(&units)
    {
        return Err(invalid("Parent incremental plan or units differ"));
    }
    const APP_RECEIPT: &str = "application/vnd.dctx0.application-native-receipt+json";
    let current_app = unique(artifacts, APP_RECEIPT)?;
    let old_app: Vec<_> = old
        .artifacts
        .iter()
        .filter(|a| a.media_type == APP_RECEIPT)
        .collect();
    let application_protocol =
        dctx0_verification::application_evidence::APPLICATION_EXECUTION_PROTOCOL_VERSION;
    let old_application = old.provenance.protocol_version == application_protocol;
    let current_application = run.provenance.protocol_version == application_protocol;
    let (prior, current) = if !old_application && !current_application {
        (
            incremental::binding(
                &old_plan,
                old.provenance.clone(),
                receipt_artifact.reference.hash.clone(),
            )?,
            incremental::binding(
                &plan,
                run.provenance.clone(),
                receipt_artifact.reference.hash.clone(),
            )?,
        )
    } else {
        match (
            old_application,
            current_application,
            old_app.as_slice(),
            current_app,
        ) {
            (true, true, [old_ref], Some(current_app)) => {
                let old_app = codec::read_artifact(c, &parent.id, &old_ref.hash)?;
                if old_app.reference != **old_ref {
                    return Err(invalid(
                        "Parent application receipt differs from its reference",
                    ));
                }
                let bundle_media =
                    dctx0_verification::application_reuse::APPLICATION_BUNDLE_MEDIA_TYPE;
                let current_bundle = unique(artifacts, bundle_media)?;
                let old_bundles: Vec<_> = old
                    .artifacts
                    .iter()
                    .filter(|a| a.media_type == bundle_media)
                    .collect();
                let (prior, current) = match (old_bundles.as_slice(), current_bundle) {
                    ([], None) => (
                        dctx0_verification::application_reuse::binding(
                            &old_plan,
                            old,
                            receipt_artifact.reference.hash.clone(),
                            &old_app.contents,
                        ),
                        dctx0_verification::application_reuse::binding(
                            &plan,
                            run,
                            receipt_artifact.reference.hash.clone(),
                            &current_app.contents,
                        ),
                    ),
                    ([old_bundle_ref], Some(current_bundle)) => {
                        let old_bundle = codec::read_artifact(c, &parent.id, &old_bundle_ref.hash)?;
                        if old_bundle.reference != **old_bundle_ref {
                            return Err(invalid(
                                "Parent application bundle differs from its reference",
                            ));
                        }
                        (
                            dctx0_verification::application_reuse::binding_with_bundle(
                                &old_plan,
                                old,
                                receipt_artifact.reference.hash.clone(),
                                &old_app.contents,
                                &old_bundle.contents,
                            ),
                            dctx0_verification::application_reuse::binding_with_bundle(
                                &plan,
                                run,
                                receipt_artifact.reference.hash.clone(),
                                &current_app.contents,
                                &current_bundle.contents,
                            ),
                        )
                    }
                    _ => {
                        return Err(invalid(
                            "Application bundle receipts must be present exactly once on both incremental runs",
                        ));
                    }
                };
                let prior = prior.ok_or_else(|| {
                    invalid("Parent application receipt does not bind its provenance")
                })?;
                let current = current.ok_or_else(|| {
                    invalid("Current application receipt does not bind its provenance")
                })?;
                (prior, current)
            }
            _ => {
                return Err(invalid(
                    "Application receipts must be present exactly once on both incremental runs",
                ));
            }
        }
    };
    if proof.plan.reused.is_empty()
        || !incremental::validate_proof_projected(
            &proof,
            &parent.id,
            &prior,
            &current,
            old,
            &units,
            run,
            &old.provenance,
            &run.provenance,
        )
    {
        return Err(invalid("Invalid incremental proof or copied case evidence"));
    }
    Ok(Some(parent.id))
}
pub(crate) fn register_parents(
    c: &Connection,
    project: &str,
    id: &str,
    artifacts: &[VerificationArtifact],
    parent: Option<&str>,
) -> Result<()> {
    if crate::storage::version(c)? >= 7 {
        let inferred = artifacts
            .iter()
            .filter(|a| a.reference.media_type == "application/vnd.dctx0.native-parent+json")
            .filter_map(|a| serde_json::from_slice::<serde_json::Value>(&a.contents).ok())
            .filter_map(|v| v["native_run"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        for parent in parent
            .into_iter()
            .chain(inferred.iter().map(String::as_str))
        {
            let same: bool = c.query_row(
                "SELECT EXISTS(SELECT 1 FROM verification_runs WHERE id=?1 AND project=?2)",
                params![parent, project],
                |r| r.get(0),
            )?;
            if !same {
                return Err(invalid("Derived run parent is absent from this project"));
            }
            if parent != id {
                c.execute(
                    "INSERT OR IGNORE INTO verification_run_parents VALUES(?1,?2)",
                    params![id, parent],
                )?;
            }
        }
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
        let (id, created_at) = self.persist_verification_run(project, run, artifacts, None)?;
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
        self.record_verification_run_owned_from(project, run, artifacts, None)
    }
    pub fn record_verification_run_owned_from(
        &mut self,
        project: &str,
        run: VerificationRun,
        artifacts: &[VerificationArtifact],
        parent: Option<&str>,
    ) -> Result<StoredVerificationRun> {
        let (id, created_at) = self.persist_verification_run(project, &run, artifacts, parent)?;
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
        parent: Option<&str>,
    ) -> Result<(String, String)> {
        self.require_write()?;
        validate(run, artifacts)?;
        let id = codec::json_hash(run)?;
        let data = serde_json::to_string(&codec::Header(run))?;
        let summary = serde_json::to_string(&VerificationRunSummary::new(&id, run)?)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let incremental_parent = validate_incremental(&tx, project, run, artifacts)?;
        if incremental_parent
            .as_deref()
            .is_some_and(|p| parent.is_some_and(|given| given != p))
        {
            return Err(invalid("Incremental proof conflicts with supplied parent"));
        }
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
                if encoding == "pruned"
                    && media == artifact.reference.media_type
                    && length == Some(artifact.reference.bytes as i64)
                {
                    let (encoding, bytes) = codec::encode(&artifact.contents)?;
                    tx.execute(
                        "UPDATE verification_artifacts SET bytes=?2,encoding=?3 WHERE hash=?1",
                        params![artifact.reference.hash, bytes, encoding],
                    )?;
                } else if media != artifact.reference.media_type
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
        tx.execute("INSERT INTO verification_runs(id,project,snapshot_id,check_id,origin,result,data,corpus_hash,summary) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![id,project,run.provenance.snapshot,run.provenance.check_occurrence,serde_json::to_string(&run.origin)?,serde_json::to_string(&run.result)?,data,Option::<String>::None,summary])?;
        for artifact in artifacts {
            tx.execute(
                "INSERT INTO verification_run_artifacts(run_id,artifact_hash) VALUES(?1,?2)",
                params![id, artifact.reference.hash],
            )?;
        }
        cases::store(&tx, &id, run)?;
        // A unit source must be protected in this publication transaction,
        // before retention runs or another writer can collect its artifacts.
        // This hold grants no PASS or task authority and does not preserve
        // unrelated history globally.
        if storage::version(&tx)? >= 8
            && run.origin == dctx0_core::VerificationRunOrigin::Native
            && artifacts.iter().any(|artifact| {
                artifact.reference.media_type == dctx0_core::UNIT_BINDING_MEDIA_TYPE
            })
        {
            tx.execute(
                "INSERT OR IGNORE INTO verification_active_sources VALUES(?1,?2,?3)",
                params![format!("native-unit:{id}"), project, id],
            )?;
        }
        register_parents(
            &tx,
            project,
            &id,
            artifacts,
            incremental_parent.as_deref().or(parent),
        )?;
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
        let tx = self.connection.unchecked_transaction()?;
        let run = codec::read_run(&tx, project, id)?;
        tx.commit()?;
        Ok(run)
    }
    pub fn verification_runs(&self, project: &str) -> Result<Vec<StoredVerificationRun>> {
        if !self.has_verification_history()? {
            return Ok(vec![]);
        }
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        let sql = if version >= 7 {
            "SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL ORDER BY rowid DESC"
        } else if version >= 5 {
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
        if storage::version(&self.connection)? < 5 {
            return codec::read_artifact(&self.connection, run_id, hash);
        }
        let available: bool = self
            .connection
            .query_row(
                "SELECT pruned_at IS NULL FROM verification_runs WHERE id=?1",
                [run_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(false);
        if !available {
            let metadata:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM verification_artifacts WHERE hash=?1 AND encoding!='pruned' AND media_type IN ('application/vnd.dctx0.runtime-metrics+json','application/vnd.dctx0.runtime-identity+json'))",[hash],|r|r.get(0))?;
            if !metadata {
                return Err(Error::DetailsPruned(run_id.into()));
            }
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
pub(crate) mod tests;
