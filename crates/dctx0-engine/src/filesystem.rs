//! Filesystem adapter: source discovery, safe paths and durable replacement.
use crate::{Error, Project, RenameError, Result};
use dctx0_core::Source;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub trait SourceReader {
    fn sources(&self) -> Result<Vec<Source>>;
    fn read_source(&self, file: &str) -> Result<String>;
}
pub trait SourceWriter: SourceReader {
    fn replace_source(&self, file: &str, text: &str) -> Result<()>;
}
impl SourceReader for Project {
    fn sources(&self) -> Result<Vec<Source>> {
        let mut files = Vec::new();
        discover(&self.root, &mut files)?;
        files.sort();
        if files.is_empty() {
            return Err(Error::Invalid("No .dctx0 files found".into()));
        }
        files
            .into_iter()
            .map(|path| {
                Ok(Source {
                    file: path
                        .strip_prefix(&self.root)
                        .expect("discovery returns paths rooted in project")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    text: fs::read_to_string(path)?,
                })
            })
            .collect()
    }
    fn read_source(&self, file: &str) -> Result<String> {
        Ok(fs::read_to_string(self.safe_source(file)?)?)
    }
}
impl SourceWriter for Project {
    fn replace_source(&self, file: &str, text: &str) -> Result<()> {
        atomic_write(&self.safe_source(file)?, text.as_bytes())
    }
}
impl Project {
    pub(crate) fn safe_source(&self, file: &str) -> Result<std::path::PathBuf> {
        let relative = Path::new(file);
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
            || relative.extension().is_none_or(|e| e != "dctx0")
        {
            return Err(RenameError::UnsafeSource.into());
        }
        let path = self.root.join(relative);
        let canonical = fs::canonicalize(&path)?;
        if !canonical.starts_with(&self.root)
            || fs::symlink_metadata(&path)?.file_type().is_symlink()
        {
            return Err(RenameError::EscapedSource.into());
        }
        Ok(path)
    }
}
fn discover(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if matches!(
                entry.file_name().to_str(),
                Some(".dctx0ctl" | ".runatron" | ".git" | "node_modules" | "target")
            ) {
                continue;
            }
            discover(&path, files)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "dctx0") {
            files.push(path);
        }
    }
    Ok(())
}
static TEMPORARY: AtomicU64 = AtomicU64::new(0);
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    publish_file(path, bytes, true).map(|_| ())
}
/// Link publication cannot replace a concurrently created profile document.
pub(crate) fn atomic_write_new(path: &Path, bytes: &[u8]) -> Result<bool> {
    publish_file(path, bytes, false)
}
fn publish_file(path: &Path, bytes: &[u8], replace: bool) -> Result<bool> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::Invalid("Path has no parent".into()))?;
    let temporary = parent.join(format!(
        ".dctx0ctl-tmp-{}-{}",
        std::process::id(),
        TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let publication = if replace {
        fs::rename(&temporary, path).map(|_| true)
    } else {
        match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::symlink_metadata(path).and_then(|metadata| {
                    if metadata.is_file() {
                        Ok(false)
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            format!("Not a regular profile file: {}", path.display()),
                        ))
                    }
                })
            }
            Err(error) => Err(error),
        }
    };
    if !replace || publication.is_err() {
        fs::remove_file(&temporary)?;
    }
    let published = publication?;
    // Unix supports syncing directory metadata. Windows does not permit
    // opening directories through File::open; the file itself was synced above.
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(published)
}
