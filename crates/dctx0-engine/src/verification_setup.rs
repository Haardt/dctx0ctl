//! Testing-only setup. No compiler publication, adapter execution or task changes.
use crate::{Error, Project, Result, filesystem::atomic_write_new};
use dctx0_core::VerificationBudgets;
use dctx0_target_sdk::{HarnessGenerator, HarnessRequest, SDK_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const CONFIG: &str = ".dctx0/testing/verification.json";
const POINTER: &str = ".dctx0ctl/verification.json";
const MAX_CONFIG_BYTES: u64 = 524_288;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredVerificationScope {
    pub subjects: BTreeSet<String>,
    pub tasks: BTreeSet<i64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationConfiguration {
    pub schema_version: u32,
    pub target: String,
    pub runtime: String,
    pub adapter: String,
    pub fixtures: String,
    pub scope: ConfiguredVerificationScope,
    pub budgets: VerificationBudgets,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectPointer {
    schema_version: u32,
    project: PathBuf,
}
#[derive(Debug, Serialize)]
pub struct VerificationSetupReport {
    pub project: PathBuf,
    pub configuration: VerificationConfiguration,
    pub created: Vec<String>,
    pub preserved: Vec<String>,
    pub executed: bool,
}
// Return the parent separately: publication never needs to infer a parent from
// unchecked target paths. Reject every symlink, including existing leaf files.
fn safe_file(root: &Path, relative: &str) -> Result<(PathBuf, PathBuf)> {
    let parts = relative.split('/').collect::<Vec<_>>();
    if relative.is_empty()
        || relative.contains(['\\', ':'])
        || relative.chars().any(char::is_control)
        || parts.iter().any(|part| matches!(*part, "" | "." | ".."))
    {
        return Err(Error::Invalid(
            "Verification path escapes project or is not portable".into(),
        ));
    }
    let mut directory = root.to_owned();
    for part in &parts[..parts.len() - 1] {
        directory.push(part);
        match fs::symlink_metadata(&directory) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(Error::Invalid(format!(
                    "Verification parent is not a real directory: {}",
                    directory.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let path = directory.join(parts[parts.len() - 1]);
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(Error::Invalid(format!(
                "Verification path is not a regular file: {}",
                path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok((directory, path))
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_CONFIG_BYTES {
        return Err(Error::Invalid(
            "Verification configuration exceeds 512 KiB".into(),
        ));
    }
    Ok(serde_json::from_reader(file)?)
}
fn validate(config: &VerificationConfiguration) -> Result<()> {
    if config.schema_version != 1
        || config.target.is_empty()
        || config.runtime.is_empty()
        || !config
            .target
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || config.scope.tasks.iter().any(|id| *id <= 0)
        || config.scope.subjects.iter().any(|s| s.trim().is_empty())
    {
        return Err(Error::Invalid(
            "Invalid or unsupported verification configuration".into(),
        ));
    }
    Ok(())
}
impl Project {
    pub fn optional_verification_configuration(
        &self,
        application: Option<&Path>,
    ) -> Result<Option<(PathBuf, VerificationConfiguration)>> {
        if application.is_none() {
            let (_, path) = safe_file(&self.root, POINTER)?;
            if !path.try_exists()? {
                return Ok(None);
            }
        }
        self.verification_configuration(application).map(Some)
    }
    /// A saved association is configuration, not evidence of source freshness.
    pub fn verification_configuration(
        &self,
        application: Option<&Path>,
    ) -> Result<(PathBuf, VerificationConfiguration)> {
        let root = if let Some(root) = application {
            fs::canonicalize(root)?
        } else {
            let (_, path) = safe_file(&self.root, POINTER)?;
            let pointer: ProjectPointer = read_json(&path)?;
            if pointer.schema_version != 1 {
                return Err(Error::Invalid(
                    "Unsupported verification project association".into(),
                ));
            }
            fs::canonicalize(pointer.project)?
        };
        let (_, path) = safe_file(&root, CONFIG)?;
        let config: VerificationConfiguration = read_json(&path)?;
        validate(&config)?;
        safe_file(&root, &config.adapter)?;
        safe_file(&root, &config.fixtures)?;
        Ok((root, config))
    }
    pub fn initialize_verification(
        &self,
        application: &Path,
        generator: &dyn HarnessGenerator,
        scope: ConfiguredVerificationScope,
    ) -> Result<VerificationSetupReport> {
        let root = fs::canonicalize(application)?;
        if !root.is_dir() {
            return Err(Error::Invalid(
                "Application root must be a directory".into(),
            ));
        }
        let bundle = generator
            .generate(&HarnessRequest {
                adapter_import: "./adapter.ts".into(),
                subjects: scope.subjects.clone(),
            })
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let scaffold = generator.scaffold();
        if bundle.sdk_version != SDK_VERSION
            || bundle.target.is_empty()
            || bundle.generator_version.is_empty()
            || scaffold.path != "adapter.ts"
        {
            return Err(Error::Invalid(
                "Unsupported or invalid testing harness bundle".into(),
            ));
        }
        let base = format!(".dctx0/testing/{}", bundle.target);
        let config = VerificationConfiguration {
            schema_version: 1,
            target: bundle.target.clone(),
            runtime: bundle.recipe.program.clone(),
            adapter: format!("{base}/adapter.ts"),
            fixtures: format!("{base}/examples.json"),
            scope,
            budgets: Default::default(),
        };
        validate(&config)?;
        let mut files = bundle
            .artifacts
            .into_iter()
            .map(|a| (format!("{base}/{}", a.path), a.contents.into_bytes()))
            .collect::<Vec<_>>();
        files.push((config.adapter.clone(), scaffold.contents.into_bytes()));
        files.push((
            config.fixtures.clone(),
            b"{\n  \"examples\": {}\n}\n".to_vec(),
        ));
        files.push((CONFIG.into(), serde_json::to_vec_pretty(&config)?));
        let mut unique = BTreeSet::new();
        for (file, _) in &files {
            if !unique.insert(file.clone()) {
                return Err(Error::Invalid(
                    "Duplicate verification artifact path".into(),
                ));
            }
            safe_file(&root, file)?;
        }
        safe_file(&self.root, ".dctx0ctl/project.lock")?;
        let (pointer_parent, pointer_path) = safe_file(&self.root, POINTER)?;
        let (lock_parent, lock_path) = safe_file(&root, ".dctx0/testing/setup.lock")?;
        let _spec_lock = self.lock()?;
        fs::create_dir_all(lock_parent)?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        fs2::FileExt::lock_exclusive(&lock)?;
        let actual = if root.join(CONFIG).exists() {
            let value: VerificationConfiguration = read_json(&root.join(CONFIG))?;
            validate(&value)?;
            safe_file(&root, &value.adapter)?;
            safe_file(&root, &value.fixtures)?;
            if value.target != config.target || value.runtime != config.runtime {
                return Err(Error::Invalid("Existing testing configuration selects a different target/runtime; edit it explicitly".into()));
            }
            value
        } else {
            config
        };
        if pointer_path.exists() {
            let pointer: ProjectPointer = read_json(&pointer_path)?;
            if pointer.schema_version != 1 || pointer.project != root {
                return Err(Error::Invalid(
                    "Spec is already associated with another verification project".into(),
                ));
            }
        }
        let mut report = VerificationSetupReport {
            project: root.clone(),
            configuration: actual,
            created: vec![],
            preserved: vec![],
            executed: false,
        };
        for (file, contents) in files {
            let (parent, path) = safe_file(&root, &file)?;
            fs::create_dir_all(parent)?;
            if atomic_write_new(&path, &contents)? {
                report.created.push(file);
            } else {
                report.preserved.push(file);
            }
        }
        fs::create_dir_all(pointer_parent)?;
        atomic_write_new(
            &pointer_path,
            &serde_json::to_vec_pretty(&ProjectPointer {
                schema_version: 1,
                project: root,
            })?,
        )?;
        // Static setup never certifies that preserved developer code is functional.
        Ok(report)
    }
}
