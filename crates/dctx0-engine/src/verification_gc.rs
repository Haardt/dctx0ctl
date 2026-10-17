//! Explicit maintenance, with no backup and no implicit filesystem traversal.
use crate::*;
use dctx0_store::{VerificationGcReport, VerificationRunSummary};
use std::io::BufReader;
mod exports;

#[derive(Debug, Serialize)]
pub struct VerificationMaintenanceReport {
    pub database: PathBuf,
    pub compacted: bool,
    pub file_bytes_before: u64,
    pub file_bytes_after: u64,
    pub storage: VerificationGcReport,
    pub exports: Vec<exports::ExportGcResult>,
}
impl Project {
    pub fn verification_evidence_summaries(
        &self,
        evidence: &[dctx0_tasks::CheckEvidence],
    ) -> Result<Vec<VerificationRunSummary>> {
        let store = self.store(AccessMode::ReadOnly)?;
        let mut seen = std::collections::BTreeSet::new();
        let mut summaries = Vec::new();
        for check in evidence {
            let parsed = serde_json::from_str::<serde_json::Value>(&check.report).ok();
            let id = parsed
                .as_ref()
                .and_then(|v| v.get("native_run"))
                .and_then(|v| v.as_str())
                .unwrap_or(&check.report);
            if id.len() != 64
                || !id.bytes().all(|b| b.is_ascii_hexdigit())
                || !seen.insert(id.to_owned())
            {
                continue;
            }
            match store.verification_run_summary(&self.key(), id) {
                Ok(summary) => summaries.push(summary),
                Err(dctx0_store::Error::Missing(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(summaries)
    }
    pub fn verification_history(
        &self,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<VerificationRunSummary>> {
        Ok(self
            .store(AccessMode::ReadOnly)?
            .verification_run_summaries(&self.key(), offset, limit)?)
    }
    pub fn verification_summary(&self, id: &str) -> Result<VerificationRunSummary> {
        Ok(self
            .store(AccessMode::ReadOnly)?
            .verification_run_summary(&self.key(), id)?)
    }
    pub fn verification_gc(
        &self,
        dry_run: bool,
        compact: bool,
        exports: Option<&Path>,
    ) -> Result<VerificationMaintenanceReport> {
        exports::validate(exports)?;
        let before = fs::metadata(&self.database)?.len();
        // A preview never creates locks, migrates, checkpoints or recovers files.
        if dry_run {
            let store = self.store(AccessMode::ReadOnly)?;
            return Ok(VerificationMaintenanceReport {
                database: self.database.clone(),
                compacted: false,
                file_bytes_before: before,
                file_bytes_after: before,
                storage: store.verification_gc_preview(&self.key())?,
                exports: exports::clean(self, exports, true)?,
            });
        }
        let _lock = self.lock()?;
        let storage = if compact {
            self.compact_verification_history()?
        } else {
            self.store(AccessMode::ReadWrite)?
                .verification_gc(&self.key())?
        };
        Ok(VerificationMaintenanceReport {
            database: self.database.clone(),
            compacted: compact,
            file_bytes_before: before,
            file_bytes_after: fs::metadata(&self.database)?.len(),
            storage,
            exports: exports::clean(self, exports, false)?,
        })
    }
    fn compact_verification_history(&self) -> Result<VerificationGcReport> {
        let candidate = self.database.with_extension("gc.sqlite3");
        let journal = self.database.with_extension("gc.json");
        if journal.exists() {
            // The journal authorizes removing only this operation's derived files.
            let owner: String = serde_json::from_reader(BufReader::new(File::open(&journal)?))?;
            if owner != self.key() {
                return Err(Error::Invalid(
                    "Compaction journal belongs to another project".into(),
                ));
            }
            for path in [
                &candidate,
                candidate.with_extension("sqlite3-wal").as_path(),
                candidate.with_extension("sqlite3-shm").as_path(),
                candidate.with_extension("sqlite3-journal").as_path(),
            ] {
                if path.exists() {
                    fs::remove_file(path)?;
                }
            }
        } else if candidate.exists() {
            return Err(Error::Invalid(format!(
                "Compaction destination already exists: {}",
                candidate.display()
            )));
        }
        atomic_write(&journal, &serde_json::to_vec(&self.key())?)?;
        let source = self.store(AccessMode::ReadWrite)?;
        let version = source.verification_storage_version()?;
        let report = source.verification_compact_copy(&self.key(), &candidate)?;
        fs::set_permissions(&candidate, fs::metadata(&self.database)?.permissions())?;
        source.install_verification_copy(&candidate, version)?;
        #[cfg(unix)]
        File::open(
            self.database
                .parent()
                .ok_or_else(|| Error::Invalid("Missing database directory".into()))?,
        )?
        .sync_all()?;
        fs::remove_file(journal)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests;
