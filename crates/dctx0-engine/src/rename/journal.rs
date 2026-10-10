//! Durable file replacement and crash recovery; all writes require the project lock.
use super::*;
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct FileChange {
    pub(super) file: String,
    pub(super) before: String,
    pub(super) after: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Journal {
    pub(super) schema_version: u32,
    pub(super) event_id: String,
    pub(super) database: String,
    pub(super) files: Vec<FileChange>,
}
impl Project {
    pub(super) fn verify_sources(&self, expected: &[dctx0_core::Source]) -> Result<()> {
        if self.sources()? != expected {
            return Err(RenameError::SourcesChanged.into());
        }
        Ok(())
    }
    pub(super) fn write_rename_files(&self, journal: &Journal) -> Result<()> {
        for change in &journal.files {
            if self.read_source(&change.file)? != change.before {
                return Err(RenameError::ConcurrentEdit.into());
            }
            self.replace_source(&change.file, &change.after)?;
        }
        Ok(())
    }
    pub(crate) fn journal_path(&self) -> std::path::PathBuf {
        self.root.join(".dctx0ctl/rename-journal.json")
    }
    pub(super) fn remove_journal(&self) -> Result<()> {
        fs::remove_file(self.journal_path())?;
        fs::File::open(self.root.join(".dctx0ctl"))?.sync_all()?;
        Ok(())
    }
    pub(crate) fn recover(&self) -> Result<()> {
        if !self.journal_path().exists() {
            return Ok(());
        }
        let journal: Journal = serde_json::from_slice(&fs::read(self.journal_path())?)?;
        if journal.schema_version != 1 || Path::new(&journal.database) != self.database {
            return Err(RenameError::UnsupportedJournal.into());
        }
        let committed = self
            .store(AccessMode::ReadOnly)?
            .rename_committed(&journal.event_id)?;
        let mut seen = BTreeSet::new();
        // Validate every file before changing any, preserving intervening edits.
        for change in &journal.files {
            if !seen.insert(&change.file) {
                return Err(RenameError::DuplicateSource.into());
            }
            let current = self.read_source(&change.file)?;
            if current != change.before && current != change.after {
                return Err(RenameError::InterveningEdit {
                    file: change.file.clone(),
                }
                .into());
            }
        }
        for change in &journal.files {
            let expected = if committed {
                &change.after
            } else {
                &change.before
            };
            if self.read_source(&change.file)? != *expected {
                self.replace_source(&change.file, expected)?;
            }
        }
        self.remove_journal()
    }
}
