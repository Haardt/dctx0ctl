//! Bounded source/dependency capture. Workers execute the captured tree.
use super::*;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Capture {
    pub root: PathBuf,
    pub files: BTreeMap<String, String>,
    remaining: u64,
    limits: Limits,
    copy_files: bool,
    entries_remaining: usize,
    helper_remaining: u64,
}
#[derive(Clone, Copy)]
struct Limits {
    file: u64,
    tree: u64,
    count: usize,
}
impl Capture {
    pub fn new(application: &Path) -> Result<Self> {
        Self::bounded(
            application,
            Limits {
                file: 16_777_216,
                tree: 536_870_912,
                count: 20_000,
            },
        )
    }
    fn bounded(application: &Path, limits: Limits) -> Result<Self> {
        Self::scan(application, limits, true)
    }
    pub fn inventory(application: &Path) -> Result<BTreeMap<String, String>> {
        let capture = Self::scan(
            application,
            Limits {
                file: 16_777_216,
                tree: 536_870_912,
                count: 20_000,
            },
            false,
        )?;
        Ok(capture.files.clone())
    }
    fn scan(application: &Path, limits: Limits, copy_files: bool) -> Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "dctx0-verification-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root)?;
        let mut capture = Self {
            root,
            files: BTreeMap::new(),
            remaining: limits.tree,
            limits,
            copy_files,
            entries_remaining: 40_000,
            helper_remaining: 268_435_456,
        };
        capture.directory(application, Path::new(""))?;
        Ok(capture)
    }
    fn directory(&mut self, application: &Path, relative: &Path) -> Result<()> {
        if relative.components().count() > 64 {
            return Err(Error::Invalid(
                "Verification capture exceeds its directory depth budget".into(),
            ));
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(application.join(relative))? {
            if self.entries_remaining == 0 {
                return Err(Error::Invalid(
                    "Verification capture exceeds its directory entry budget".into(),
                ));
            }
            self.entries_remaining -= 1;
            entries.push(entry?);
        }
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | ".runatron" | ".dctx0ctl" | "target" | ".bin")
            ) {
                continue;
            }
            let path = relative.join(name);
            let kind = entry.file_type()?;
            if kind.is_dir() {
                if self.copy_files {
                    fs::create_dir_all(self.root.join(&path))?;
                }
                self.directory(application, &path)?;
            } else if kind.is_file() {
                self.file(&entry.path(), &path)?;
            } else {
                return Err(Error::Invalid(format!(
                    "Verification capture needs regular files; unsupported path {}",
                    entry.path().display()
                )));
            }
        }
        Ok(())
    }
    fn file(&mut self, source: &Path, relative: &Path) -> Result<()> {
        if self.files.len() >= self.limits.count {
            return Err(Error::Invalid(
                "Verification capture exceeds 20000 files".into(),
            ));
        }
        let helper = relative
            == Path::new(&format!(
                ".dctx0/testing/rust/executables/dctx0{}",
                std::env::consts::EXE_SUFFIX
            ));
        let file_limit = if helper {
            268_435_456
        } else {
            self.limits.file
        };
        let remaining = if helper {
            &mut self.helper_remaining
        } else {
            &mut self.remaining
        };
        let mut bytes = Vec::new();
        fs::File::open(source)?
            .take(file_limit + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > file_limit || bytes.len() as u64 > *remaining {
            return Err(Error::Invalid(
                "Verification capture exceeds its 16 MiB file or 512 MiB tree budget".into(),
            ));
        }
        let name = relative
            .to_str()
            .ok_or_else(|| Error::Invalid("Verification paths must be UTF-8".into()))?;
        if std::path::MAIN_SEPARATOR != '\\' && name.contains('\\') {
            return Err(Error::Invalid(
                "Verification paths must use unambiguous forward-slash separators".into(),
            ));
        }
        let name = name.replace('\\', "/");
        *remaining -= bytes.len() as u64;
        self.files.insert(name, digest(&bytes));
        if self.copy_files {
            fs::write(self.root.join(relative), bytes)?;
            if helper {
                fs::set_permissions(
                    self.root.join(relative),
                    fs::metadata(source)?.permissions(),
                )?;
            }
        }
        Ok(())
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_rejects_symlinks_and_enforces_file_tree_and_count_budgets() {
        let root =
            std::env::temp_dir().join(format!("dctx0-capture-limits-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("a.ts"), b"ab").unwrap();
        fs::write(root.join("b.ts"), b"cd").unwrap();
        for limits in [
            Limits {
                file: 1,
                tree: 8,
                count: 2,
            },
            Limits {
                file: 4,
                tree: 3,
                count: 2,
            },
            Limits {
                file: 4,
                tree: 8,
                count: 1,
            },
        ] {
            assert!(Capture::bounded(&root, limits).is_err());
        }
        let capture = Capture::bounded(
            &root,
            Limits {
                file: 2,
                tree: 4,
                count: 2,
            },
        )
        .unwrap();
        assert_eq!(capture.files.len(), 2);
        assert_eq!(fs::read(capture.root.join("a.ts")).unwrap(), b"ab");
        let captured = capture.root.clone();
        drop(capture);
        assert!(!captured.exists());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("a.ts"), root.join("link.ts")).unwrap();
            assert!(Capture::new(&root).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn non_utf8_source_paths_cannot_enter_capture_or_freshness_evidence() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let root = std::env::temp_dir().join(format!("dctx0-capture-utf8-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(OsString::from_vec(vec![0xff])), "actual source").unwrap();
        assert!(
            matches!(Capture::inventory(&root), Err(Error::Invalid(message)) if message.contains("UTF-8"))
        );
        assert!(
            matches!(Capture::new(&root), Err(Error::Invalid(message)) if message.contains("UTF-8"))
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn distinct_native_paths_cannot_collide_in_verification_provenance() {
        let root =
            std::env::temp_dir().join(format!("dctx0-path-collision-{}", std::process::id()));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/app.ts"), "actual implementation").unwrap();
        fs::write(root.join("src\\app.ts"), "different native path").unwrap();
        assert!(
            matches!(Capture::inventory(&root), Err(Error::Invalid(message)) if message.contains("unambiguous"))
        );
        assert!(
            matches!(Capture::new(&root), Err(Error::Invalid(message)) if message.contains("unambiguous"))
        );
        assert_eq!(
            fs::read_to_string(root.join("src/app.ts")).unwrap(),
            "actual implementation"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn freshness_inventory_detects_edits_additions_removals_and_dependency_changes_without_writes()
    {
        let root =
            std::env::temp_dir().join(format!("dctx0-inventory-freshness-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("app.ts"), b"original").unwrap();
        fs::write(root.join("node_modules/dep.js"), b"dependency").unwrap();
        let baseline = Capture::inventory(&root).unwrap();
        assert_eq!(fs::read(root.join("app.ts")).unwrap(), b"original");
        assert_eq!(baseline, Capture::inventory(&root).unwrap());
        fs::write(root.join("app.ts"), b"changed").unwrap();
        assert_ne!(baseline, Capture::inventory(&root).unwrap());
        fs::write(root.join("app.ts"), b"original").unwrap();
        fs::write(root.join("added.ts"), b"new").unwrap();
        assert_ne!(baseline, Capture::inventory(&root).unwrap());
        fs::remove_file(root.join("added.ts")).unwrap();
        fs::write(root.join("node_modules/dep.js"), b"changed dependency").unwrap();
        assert_ne!(baseline, Capture::inventory(&root).unwrap());
        fs::remove_file(root.join("node_modules/dep.js")).unwrap();
        assert_ne!(baseline, Capture::inventory(&root).unwrap());
        fs::remove_dir_all(&root).unwrap();
        assert!(Capture::inventory(&root).is_err());
    }
}

#[cfg(test)]
mod directory_budget_tests {
    use super::*;
    #[test]
    fn directory_entries_and_depth_are_bounded_even_without_files() {
        let application = std::env::temp_dir().join(format!(
            "dctx0-directory-budget-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&application).unwrap();
        fs::create_dir(application.join("empty")).unwrap();
        let mut capture = Capture::new(&application).unwrap();
        capture.entries_remaining = 0;
        assert!(
            matches!(capture.directory(&application, Path::new("")), Err(Error::Invalid(message)) if message.contains("entry budget"))
        );
        let too_deep = (0..65).fold(PathBuf::new(), |path, _| path.join("d"));
        assert!(
            matches!(capture.directory(&application, &too_deep), Err(Error::Invalid(message)) if message.contains("depth budget"))
        );
        drop(capture);
        fs::remove_dir_all(application).unwrap();
    }
}
