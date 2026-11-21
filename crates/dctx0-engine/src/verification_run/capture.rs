//! Bounded source/dependency capture. Workers execute the captured tree.
use super::{code_selection::CodeSelection, *};
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) const MATERIALIZED_INPUTS: &str = "application/vnd.dctx0.materialized-capture+json";
pub(crate) struct Capture {
    pub root: PathBuf,
    pub files: BTreeMap<String, String>,
    pub materialized: BTreeMap<String, String>,
    remaining: u64,
    limits: Limits,
    copy_files: bool,
    entries_remaining: usize,
    helper_remaining: u64,
    application_tests: bool,
    code_selection: CodeSelection,
}
#[derive(Clone, Copy)]
struct Limits {
    file: u64,
    tree: u64,
    count: usize,
}
impl Capture {
    pub(super) fn reuse_inventory_matches(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
        run: &VerificationRun,
        prior: &BTreeMap<String, String>,
    ) -> Result<bool> {
        if run
            .artifacts
            .iter()
            .any(|artifact| artifact.media_type == MATERIALIZED_INPUTS)
        {
            use dctx0_verification::incremental::code_inputs;
            let current = Self::bound_inventory(application, budgets, prior)?;
            return Ok(code_inputs(prior) == code_inputs(&current));
        }
        Ok(Self::code_inventory(application, prior)?
            == Self::proof_inventory(application, budgets, run)?)
    }

    pub(super) fn code_inventory(
        application: &Path,
        inputs: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>> {
        let mut selection = super::code_selection::CodeSelection::new(application)?;
        let mut code = BTreeMap::new();
        for (path, hash) in inputs {
            if selection.contains(Path::new(path))? {
                code.insert(path.clone(), hash.clone());
            }
        }
        Ok(code)
    }
    pub fn proof_inventory(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
        run: &VerificationRun,
    ) -> Result<BTreeMap<String, String>> {
        if run
            .artifacts
            .iter()
            .any(|reference| reference.media_type == MATERIALIZED_INPUTS)
        {
            Self::bound_inventory(
                application,
                budgets,
                &run.provenance
                    .implementation
                    .iter()
                    .chain(run.provenance.dependencies.iter())
                    .map(|(p, h)| (p.clone(), h.clone()))
                    .collect(),
            )
        } else if run.artifacts.iter().any(|reference| {
            reference.media_type
                == dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE
        }) {
            Self::application_inventory(application, budgets)
        } else {
            Self::inventory_with_budget(application, budgets)
        }
    }
    pub fn application_tests(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
    ) -> Result<Self> {
        // Dependencies are materialized by the frozen native build in the new
        // capture. An ambient node_modules (including executable symlinks) is
        // neither executed nor mistaken for a project source dependency.
        Self::scan_application(
            application,
            Limits {
                file: budgets.max_file_bytes,
                tree: budgets.max_tree_bytes,
                count: 20_000,
            },
            true,
            true,
        )
    }
    pub fn application_inventory(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
    ) -> Result<BTreeMap<String, String>> {
        Ok(Self::scan_application(
            application,
            Limits {
                file: budgets.max_file_bytes,
                tree: budgets.max_tree_bytes,
                count: 20_000,
            },
            false,
            true,
        )?
        .files
        .clone())
    }
    pub(crate) fn reviewed_go_inputs(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
    ) -> Result<String> {
        let capture = Self::scan_application(
            application,
            Limits {
                file: budgets.max_file_bytes,
                tree: budgets.max_tree_bytes,
                count: 20_000,
            },
            false,
            true,
        )?;
        Self::go_review_hash(application, &capture.materialized)
    }

    pub(super) fn go_review_hash(root: &Path, inputs: &BTreeMap<String, String>) -> Result<String> {
        let mut asset_roots = BTreeSet::new();
        for name in inputs.keys() {
            let path = Path::new(name);
            let directory = path.parent().expect("relative Go input has parent");
            if name.ends_with(".go") {
                let source = fs::read_to_string(root.join(path))?;
                if source.lines().any(|line| {
                    line.trim_start()
                        .strip_prefix("//go:embed")
                        .is_some_and(|args| args.starts_with([' ', '\t']))
                }) {
                    // Include the package subtree conservatively for wildcard embeds.
                    asset_roots.insert(directory);
                }
            } else if matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("go.mod" | "go.work")
            ) {
                let manifest = fs::read_to_string(root.join(path))?;
                let mut replacement_block = false;
                for line in manifest.lines().map(str::trim) {
                    if line.starts_with("//") {
                        continue;
                    }
                    if line.starts_with(')') {
                        replacement_block = false;
                    }
                    let declaration = line
                        .strip_prefix("replace")
                        .filter(|rest| rest.starts_with([' ', '\t']))
                        .map(str::trim_start);
                    if declaration.is_some_and(|rest| rest.starts_with('(')) {
                        replacement_block = true;
                    }
                    if let Some((_, target)) = declaration
                        .or_else(|| replacement_block.then_some(line))
                        .and_then(|line| line.split_once("=>"))
                    {
                        Self::check_go_review_replacement(root, directory, target, inputs)?;
                    }
                }
            }
        }
        let inputs: BTreeMap<_, _> = inputs
            .iter()
            .filter(|(name, _)| {
                let path = Path::new(name);
                asset_roots
                    .iter()
                    .any(|directory| path.starts_with(directory))
                    || matches!(
                        path.file_name().and_then(|name| name.to_str()),
                        Some("go.mod" | "go.sum" | "go.work" | "go.work.sum" | "modules.txt")
                    )
                    || matches!(
                        path.extension().and_then(|extension| extension.to_str()),
                        Some(
                            "go" | "s"
                                | "S"
                                | "syso"
                                | "h"
                                | "c"
                                | "cc"
                                | "cpp"
                                | "m"
                                | "f"
                                | "F"
                                | "f90"
                                | "swig"
                                | "swigcxx"
                        )
                    )
            })
            .collect();
        Ok(digest(&serde_json::to_vec(&inputs)?))
    }

    fn check_go_review_replacement(
        root: &Path,
        directory: &Path,
        target: &str,
        inputs: &BTreeMap<String, String>,
    ) -> Result<()> {
        // Go module replacements use a path or a module path followed by a version.
        let target = target.trim_start();
        let target = if target.starts_with('"') {
            serde_json::Deserializer::from_str(target)
                .into_iter::<String>()
                .next()
                .transpose()
                .map_err(|_| Error::Invalid("Unrecognized quoted Go replacement path".into()))?
                .unwrap_or_default()
        } else {
            target.split_whitespace().next().unwrap_or_default().into()
        };
        if Path::new(&target).is_absolute() {
            return Err(Error::Invalid("DECODER_REVIEW_EXTERNAL_SOURCE: absolute Go replacement cannot follow the reviewed source capture".into()));
        }
        if target.starts_with("./")
            || target.starts_with("../")
            || matches!(target.as_str(), "." | "..")
        {
            let replacement = fs::canonicalize(root.join(directory).join(target))?;
            // The built artifact must use files covered by this project's review.
            if replacement
                .strip_prefix(fs::canonicalize(root)?)
                .ok()
                .and_then(|relative| {
                    relative
                        .join("go.mod")
                        .to_str()
                        .map(|name| inputs.contains_key(name))
                })
                != Some(true)
            {
                return Err(Error::Invalid("DECODER_REVIEW_EXTERNAL_SOURCE: local Go replacement is outside the reviewed inputs".into()));
            }
        }
        Ok(())
    }

    /// Historical default limits; test and fixture paths only. Production paths
    /// pass the configured budgets explicitly through the *_with_budget forms.
    #[cfg(test)]
    pub fn new(application: &Path) -> Result<Self> {
        Self::new_with_budget(
            application,
            crate::verification_setup::VerificationCaptureBudgets::default(),
        )
    }
    /// Capture with the configured budgets; the defaults keep the historical
    /// 16 MiB per-file and 512 MiB tree limits.
    pub fn new_with_budget(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
    ) -> Result<Self> {
        Self::bounded(
            application,
            Limits {
                file: budgets.max_file_bytes,
                tree: budgets.max_tree_bytes,
                count: 20_000,
            },
        )
    }
    pub(super) fn unit_recipe(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
    ) -> Result<Self> {
        Self::scan_with_selection(
            application,
            Limits {
                file: budgets.max_file_bytes,
                tree: budgets.max_tree_bytes,
                count: 20_000,
            },
            true,
            false,
            CodeSelection::managed(application)?,
        )
    }
    fn bounded(application: &Path, limits: Limits) -> Result<Self> {
        Self::scan(application, limits, true)
    }
    #[cfg(test)]
    pub fn inventory(application: &Path) -> Result<BTreeMap<String, String>> {
        Self::inventory_with_budget(
            application,
            crate::verification_setup::VerificationCaptureBudgets::default(),
        )
    }
    /// Inventory with the configured budgets so stored and current inventories
    /// are always compared under identical limits.
    pub fn inventory_with_budget(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
    ) -> Result<BTreeMap<String, String>> {
        let capture = Self::scan(
            application,
            Limits {
                file: budgets.max_file_bytes,
                tree: budgets.max_tree_bytes,
                count: 20_000,
            },
            false,
        )?;
        Ok(capture.files.clone())
    }
    pub(super) fn materialized_inventory(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
    ) -> Result<BTreeMap<String, String>> {
        Ok(Self::scan(
            application,
            Limits {
                file: budgets.max_file_bytes,
                tree: budgets.max_tree_bytes,
                count: 20_000,
            },
            false,
        )?
        .materialized
        .clone())
    }
    /// Recheck every captured input and discover new executable/configuration inputs.
    /// Newly created runtime data is an output, not a replacement for an input.
    pub(super) fn bound_inventory(
        application: &Path,
        budgets: crate::verification_setup::VerificationCaptureBudgets,
        expected: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>> {
        let mut current = Self::materialized_inventory(application, budgets)?;
        current.retain(|path, _| {
            expected.contains_key(path)
                || dctx0_verification::incremental::is_code_path(path)
                || path.starts_with("node_modules/")
                || matches!(
                    path.as_str(),
                    "package.json"
                        | "bun.lock"
                        | "bun.lockb"
                        | "bunfig.toml"
                        | "go.mod"
                        | "go.sum"
                        | "Cargo.toml"
                        | "Cargo.lock"
                )
                || path.starts_with("tsconfig.") && path.ends_with(".json")
        });
        Ok(current)
    }
    fn scan(application: &Path, limits: Limits, copy_files: bool) -> Result<Self> {
        Self::scan_application(application, limits, copy_files, false)
    }
    fn scan_application(
        application: &Path,
        limits: Limits,
        copy_files: bool,
        application_tests: bool,
    ) -> Result<Self> {
        Self::scan_with_selection(
            application,
            limits,
            copy_files,
            application_tests,
            CodeSelection::new(application)?,
        )
    }
    fn scan_with_selection(
        application: &Path,
        limits: Limits,
        copy_files: bool,
        application_tests: bool,
        code_selection: CodeSelection,
    ) -> Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "dctx0-verification-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root)?;
        let mut capture = Self {
            root,
            files: BTreeMap::new(),
            materialized: BTreeMap::new(),
            remaining: limits.tree,
            limits,
            copy_files,
            entries_remaining: 40_000,
            helper_remaining: 268_435_456,
            application_tests,
            code_selection,
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
            if self.application_tests && relative.as_os_str().is_empty() && name == "node_modules" {
                continue;
            }
            if matches!(
                name.to_str(),
                Some(".git" | ".runatron" | ".dctx0ctl" | "target" | ".bin")
            ) {
                continue;
            }
            let path = relative.join(name);
            let kind = entry.file_type()?;
            if kind.is_dir()
                && !path.starts_with("node_modules")
                && matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("dist" | "coverage" | "reports")
                )
                && self.code_selection.ignored_path(&path, true)?
            {
                continue;
            }
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
        let name = relative
            .to_str()
            .ok_or_else(|| Error::Invalid("Verification paths must be UTF-8".into()))?;
        if std::path::MAIN_SEPARATOR != '\\' && name.contains('\\') {
            return Err(Error::Invalid(
                "Verification paths must use unambiguous forward-slash separators".into(),
            ));
        }
        let code = self.code_selection.contains(relative)?;
        if self.materialized.len() >= self.limits.count {
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
        // Reject over-budget files from their size before reading them, so a
        // large generated artifact cannot consume the capture tree budget or
        // the process memory while the limits are evaluated.
        let size = fs::metadata(source)?.len();
        if size > file_limit || size > *remaining {
            return Err(Error::Invalid(
                "Verification capture exceeds its 16 MiB file or 512 MiB tree budget".into(),
            ));
        }
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
        let hash = digest(&bytes);
        self.materialized.insert(name.clone(), hash.clone());
        if code {
            self.files.insert(name, hash);
        }
        if self.copy_files {
            fs::write(self.root.join(relative), bytes)?;
            fs::set_permissions(
                self.root.join(relative),
                fs::metadata(source)?.permissions(),
            )?;
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
    fn materialized_inputs_bind_ignored_code_dependencies_and_assets_separately() {
        let root = std::env::temp_dir().join(format!(
            "dctx0-materialized-inputs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("node_modules/dep")).unwrap();
        fs::create_dir_all(root.join("node_modules/dep/dist")).unwrap();
        fs::write(root.join(".gitignore"), "node_modules/\ncodec.ts\n").unwrap();
        fs::write(root.join("main.ts"), "import './codec'").unwrap();
        fs::write(root.join("codec.ts"), "export const value = 1").unwrap();
        fs::write(root.join("node_modules/dep/index.js"), "export default 1").unwrap();
        fs::write(root.join("node_modules/dep/package.json"), "{}").unwrap();
        fs::write(root.join("node_modules/dep/asset.bin"), b"actual asset").unwrap();
        fs::write(
            root.join("node_modules/dep/dist/index.js"),
            "export default 2",
        )
        .unwrap();
        let budgets = crate::verification_setup::VerificationCaptureBudgets::default();
        let capture = Capture::new(&root).unwrap();
        assert!(capture.files.contains_key("main.ts"));
        for path in [
            "codec.ts",
            "node_modules/dep/index.js",
            "node_modules/dep/package.json",
            "node_modules/dep/asset.bin",
            "node_modules/dep/dist/index.js",
        ] {
            assert!(!capture.files.contains_key(path));
            assert_eq!(
                capture.materialized[path],
                digest(&fs::read(root.join(path)).unwrap())
            );
        }
        assert_eq!(
            Capture::inventory_with_budget(&root, budgets).unwrap(),
            capture.files
        );
        assert_eq!(
            Capture::materialized_inventory(&root, budgets).unwrap(),
            capture.materialized
        );
        fs::write(root.join("runtime.sqlite"), b"new output").unwrap();
        assert_eq!(
            Capture::bound_inventory(&root, budgets, &capture.materialized).unwrap(),
            capture.materialized
        );
        fs::write(root.join("new-code.ts"), "export const added = 1").unwrap();
        assert_ne!(
            Capture::bound_inventory(&root, budgets, &capture.materialized).unwrap(),
            capture.materialized
        );
        fs::remove_file(root.join("new-code.ts")).unwrap();
        fs::write(root.join("codec.ts"), "export const value = 2").unwrap();
        assert_eq!(
            Capture::inventory_with_budget(&root, budgets).unwrap(),
            capture.files
        );
        assert_ne!(
            Capture::materialized_inventory(&root, budgets).unwrap(),
            capture.materialized
        );
        fs::write(root.join("codec.ts"), "export const value = 1").unwrap();
        fs::remove_file(root.join("node_modules/dep/asset.bin")).unwrap();
        assert_ne!(
            Capture::materialized_inventory(&root, budgets).unwrap(),
            capture.materialized
        );
        drop(capture);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn capture_preserves_executable_inputs_outside_helper_paths() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "dctx0-capture-executable-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let binary = root.join("git-lfs");
        fs::write(&binary, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o750)).unwrap();
        let capture = Capture::new(&root).unwrap();
        let copied = capture.root.join("git-lfs");
        assert_eq!(
            fs::metadata(&copied).unwrap().permissions().mode() & 0o777,
            0o750
        );
        assert!(
            std::process::Command::new(copied)
                .status()
                .unwrap()
                .success()
        );
        drop(capture);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn configured_capture_budgets_accept_larger_regular_files() {
        let root = std::env::temp_dir().join(format!(
            "dctx0-capture-budgets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        std::fs::File::create(root.join("generated.bin"))
            .unwrap()
            .set_len(16_777_217)
            .unwrap();
        assert!(Capture::new(&root).is_err());
        let budgets = crate::verification_setup::VerificationCaptureBudgets {
            max_file_bytes: 34_359_738_368,
            max_tree_bytes: 2_147_483_648,
        };
        let capture = Capture::new_with_budget(&root, budgets).unwrap();
        assert!(capture.root.join("generated.bin").is_file());
        assert!(!capture.files.contains_key("generated.bin"));
        assert_eq!(
            Capture::inventory_with_budget(&root, budgets).unwrap(),
            capture.files
        );
        let copied = capture.root.clone();
        drop(capture);
        assert!(!copied.exists());
        fs::remove_dir_all(&root).unwrap();
    }
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
    #[test]
    fn code_inventory_obeys_parent_nested_and_negated_gitignore_rules() {
        let root =
            std::env::temp_dir().join(format!("dctx0-code-selection-{}", std::process::id()));
        fs::create_dir_all(root.join("app/src")).unwrap();
        fs::create_dir_all(root.join("app/dist")).unwrap();
        fs::write(root.join(".gitignore"), "dist/\n*.go\n").unwrap();
        fs::write(root.join("app/src/.gitignore"), "!kept.go\n").unwrap();
        fs::write(root.join("app/src/kept.go"), "package src").unwrap();
        fs::write(root.join("app/src/ignored.go"), "package ignored").unwrap();
        fs::write(root.join("app/dist/generated.ts"), "export default 1").unwrap();
        fs::write(root.join("app/build.json"), "{}").unwrap();
        let inputs = Capture::inventory(&root.join("app")).unwrap();
        assert_eq!(inputs.keys().collect::<Vec<_>>(), vec!["src/kept.go"]);
        fs::write(root.join("app/build.json"), "{\"changed\":true}").unwrap();
        fs::write(root.join("app/dist/generated.ts"), "export default 2").unwrap();
        assert_eq!(inputs, Capture::inventory(&root.join("app")).unwrap());
        fs::write(root.join("app/src/kept.go"), "package changed").unwrap();
        assert_ne!(inputs, Capture::inventory(&root.join("app")).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
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
