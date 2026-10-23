//! Storage conversion uses an independent copy and preserves a source backup.
use crate::*;
#[derive(Debug, Serialize)]
pub struct StorageCompaction {
    pub before: dctx0_store::StorageReport,
    pub after: dctx0_store::StorageReport,
    pub destination: PathBuf,
    pub installed: bool,
    pub backup: Option<PathBuf>,
}
impl Project {
    pub fn storage_recover(&self) -> Result<serde_json::Value> {
        let _lock = self.lock()?;
        let journal = self.database.with_extension("storage-install.json");
        if !journal.exists() {
            return Ok(serde_json::json!({"recovered":false,"database":self.database}));
        }
        let record: serde_json::Value = serde_json::from_slice(&fs::read(&journal)?)?;
        let backup = record["backup"]
            .as_str()
            .map(PathBuf::from)
            .ok_or_else(|| Error::Invalid("Invalid storage recovery journal".into()))?;
        let backups = self
            .database
            .parent()
            .unwrap_or(Path::new("."))
            .join("backups");
        if record["source"] != self.database.to_string_lossy().as_ref()
            || backup.parent() != Some(backups.as_path())
        {
            return Err(Error::Invalid(
                "Storage journal paths do not match this project".into(),
            ));
        }
        let restored = Store::storage_recover_file(&self.database, &backup)?;
        fs::remove_file(journal)?;
        Ok(
            serde_json::json!({"recovered":true,"restored_backup":restored,"database":self.database,"backup":backup}),
        )
    }

    pub fn storage_install_copy(&self, copy: &Path) -> Result<StorageCompaction> {
        let _lock = self.lock()?;
        let source = self.store(AccessMode::ReadOnly)?;
        let version = source.verification_storage_version()?;
        let before = source.storage_inspect()?;
        source.storage_verify_copy(copy)?;
        let after = Store::open(copy, AccessMode::ReadOnly)?.storage_inspect()?;
        let backups = self
            .database
            .parent()
            .unwrap_or(Path::new("."))
            .join("backups");
        fs::create_dir_all(&backups)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::Invalid(e.to_string()))?
            .as_nanos();
        let backup = backups.join(format!("state-before-schema7-{stamp}.sqlite3"));
        let journal = self.database.with_extension("storage-install.json");
        atomic_write(
            &journal,
            &serde_json::to_vec(
                &serde_json::json!({"source":self.database,"destination":copy,"backup":backup}),
            )?,
        )?;
        source.install_storage_copy(copy, version, &backup)?;
        fs::remove_file(journal)?;
        #[cfg(unix)]
        std::fs::File::open(self.database.parent().unwrap_or(Path::new(".")))?.sync_all()?;
        Ok(StorageCompaction {
            before,
            after,
            destination: self.database.clone(),
            installed: true,
            backup: Some(backup),
        })
    }

    pub fn storage_compact(&self, output: Option<&Path>) -> Result<StorageCompaction> {
        let _lock = self.lock()?;
        let source = self.store(AccessMode::ReadOnly)?;
        let before = source.storage_inspect()?;
        let destination = output
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.database.with_extension("compact.sqlite3"));
        let after = source.storage_compact_project_copy(Some(&self.key()), &destination)?;
        Ok(StorageCompaction {
            before,
            after,
            destination,
            installed: false,
            backup: None,
        })
    }
}
