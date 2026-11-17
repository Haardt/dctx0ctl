//! Code selection is independent of build materialization and execution receipts.
use super::*;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::sync::Arc;

/// Resolve repository and nested .gitignore rules, including deleted source paths.
pub(super) struct CodeSelection {
    root: PathBuf,
    directories: BTreeMap<PathBuf, Option<Vec<Arc<Gitignore>>>>,
}

impl CodeSelection {
    pub fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            root: fs::canonicalize(root)?,
            directories: BTreeMap::new(),
        })
    }

    fn rules(&mut self, directory: &Path) -> Result<Option<Vec<Arc<Gitignore>>>> {
        if let Some(rules) = self.directories.get(directory) {
            return Ok(rules.clone());
        }
        let parent = if directory.join(".git").exists() {
            Some(Vec::new())
        } else if let Some(parent) = directory.parent() {
            self.rules(parent)?
        } else {
            Some(Vec::new())
        };
        let mut rules = match parent {
            Some(rules) if !Self::ignored(&rules, directory, true) => rules,
            _ => {
                self.directories.insert(directory.into(), None);
                return Ok(None);
            }
        };
        let file = directory.join(".gitignore");
        if file.is_file() {
            let mut builder = GitignoreBuilder::new(directory);
            if let Some(error) = builder.add(&file) {
                return Err(Error::Invalid(format!("Invalid .gitignore: {error}")));
            }
            rules.push(Arc::new(builder.build().map_err(|error| {
                Error::Invalid(format!("Invalid .gitignore: {error}"))
            })?));
        }
        self.directories
            .insert(directory.into(), Some(rules.clone()));
        Ok(Some(rules))
    }

    fn ignored(rules: &[Arc<Gitignore>], path: &Path, directory: bool) -> bool {
        let mut ignored = false;
        for rule in rules {
            let matched = rule.matched(path, directory);
            if !matched.is_none() {
                ignored = matched.is_ignore();
            }
        }
        ignored
    }

    pub fn ignored_path(&mut self, relative: &Path, directory: bool) -> Result<bool> {
        let path = self.root.join(relative);
        let parent = path.parent().unwrap_or(&self.root).to_path_buf();
        Ok(self
            .rules(&parent)?
            .is_none_or(|rules| Self::ignored(&rules, &path, directory)))
    }

    pub fn contains(&mut self, relative: &Path) -> Result<bool> {
        Ok(
            dctx0_verification::incremental::is_code_path(&relative.to_string_lossy())
                && !self.ignored_path(relative, false)?,
        )
    }
}
