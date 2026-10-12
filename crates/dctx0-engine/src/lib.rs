//! Filesystem and history orchestration. Semantic compilation remains pure.
mod editors;
use dctx0_changes::HistoryReader;
pub use editors::{
    Editor, EditorInstallReport, EditorInstallRequest, activate_vscode, install_editor,
};
mod check;
mod native;
pub use native::*;
mod filesystem;
mod profiles;
mod skills;
pub use skills::*;
mod rename;
mod tasks;
mod verification;
mod verification_complete;
pub use verification_complete::*;
mod verification_discovery;
mod verification_run;
mod verification_setup;
use dctx0_changes::{Changes, Snapshot};
use dctx0_core::{Compilation, Diagnostic, Source};
pub use dctx0_store::AccessMode;
use dctx0_store::{HistoryEntry, Store};
use filesystem::atomic_write;
pub use filesystem::{SourceReader, SourceWriter};
pub use profiles::*;
pub use rename::{Edit, RenameError, RenamePlan, RenameReport, plan_rename};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};
pub use verification::VerificationSelection;
pub use verification_discovery::*;
pub use verification_run::{
    InspectedVerificationReport, NativeVerificationReport, ReplayChange, TaskAttestation,
    VerificationLifecycleFixtures, VerificationReplayReport,
};
pub use verification_setup::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Rename(#[from] RenameError),
    #[error(transparent)]
    Task(#[from] dctx0_tasks::Error),
    #[error("Filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("History: {0}")]
    Store(#[from] dctx0_store::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckReport {
    pub ok: bool,
    pub source_hash: String,
    pub snapshot_id: Option<String>,
    pub changes: Option<Changes>,
    pub diagnostics: Vec<Diagnostic>,
    pub symbols: usize,
    pub operations: usize,
}
#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub database: PathBuf,
}
impl Project {
    pub fn new(root: &Path, database: Option<&Path>) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(Error::Invalid("Spec root must be a directory".into()));
        }
        let database = match database {
            Some(path) if path.is_absolute() => path.to_owned(),
            Some(path) => std::env::current_dir()?.join(path),
            None => root.join(".dctx0ctl/state.sqlite3"),
        };
        Ok(Self { root, database })
    }
    pub fn key(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
    pub fn compile(&self) -> Result<(Vec<Source>, Compilation)> {
        let sources = self.sources()?;
        let compilation = dctx0_language::compile(&sources);
        Ok((sources, compilation))
    }
    pub fn lock(&self) -> Result<File> {
        let state = self.root.join(".dctx0ctl");
        fs::create_dir_all(&state)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(state.join("project.lock"))?;
        fs2::FileExt::lock_exclusive(&lock)?;
        Ok(lock)
    }
    pub fn store(&self, mode: AccessMode) -> Result<Store> {
        if mode == AccessMode::ReadWrite {
            let parent = self
                .database
                .parent()
                .ok_or_else(|| Error::Invalid("Database path has no parent directory".into()))?;
            fs::create_dir_all(parent)?;
        }
        Ok(Store::open(&self.database, mode)?)
    }
    pub fn check(&self) -> Result<CheckReport> {
        let _lock = self.lock()?;
        self.recover()?;
        let (sources, compilation) = self.compile()?;
        let mut store = self.store(AccessMode::ReadWrite)?;
        check::record_check(&self.key(), &sources, compilation, &mut store)
    }
    pub fn history(&self) -> Result<Vec<HistoryEntry>> {
        self.store(AccessMode::ReadOnly)?
            .history(&self.key())
            .map_err(Error::from)
    }
    pub fn load(&self, id: &str) -> Result<Snapshot> {
        self.store(AccessMode::ReadOnly)?
            .load(&self.key(), id)
            .map_err(Error::from)
    }
    pub fn diff(&self, before: &str, after: &str) -> Result<Changes> {
        let store = self.store(AccessMode::ReadOnly)?;
        let old = store.load(&self.key(), before)?;
        let new = store.load(&self.key(), after)?;
        let mappings = store.rename_mappings(&self.key(), &old.id, &new.id)?;
        Ok(dctx0_changes::diff(Some(&old), &new, &mappings))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_files_and_database_paths_reject_missing_parent_directories() {
        assert!(atomic_write(Path::new("/"), b"unchanged").is_err());
        let project =
            Project::new(Path::new(env!("CARGO_MANIFEST_DIR")), Some(Path::new("/"))).unwrap();
        assert!(project.store(AccessMode::ReadWrite).is_err());
    }
}

#[cfg(test)]
mod profile_publication_tests {
    use super::*;
    #[test]
    fn new_profile_publication_preserves_concurrent_edits_and_cleans_failed_temporaries() {
        let root =
            std::env::temp_dir().join(format!("dctx0-profile-publication-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("guide.md");
        assert!(filesystem::atomic_write_new(&file, b"Default").unwrap());
        fs::write(&file, b"Architect's concurrent edit").unwrap();
        assert!(!filesystem::atomic_write_new(&file, b"Replacement").unwrap());
        assert_eq!(fs::read(&file).unwrap(), b"Architect's concurrent edit");
        // Empty names cannot be published as files; the temporary file is removed.
        assert!(filesystem::atomic_write_new(&root, b"Invalid").is_err());
        assert!(filesystem::atomic_write_new(&root.join("bad\0name"), b"Invalid").is_err());
        assert!(atomic_write(&root, b"Invalid").is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
