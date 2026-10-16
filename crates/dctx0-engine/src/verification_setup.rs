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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_resolution: Option<DependencyResolutionBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<VerificationExecutionBudgets>,
    pub schema_version: u32,
    pub target: String,
    pub runtime: String,
    pub adapter: String,
    pub fixtures: String,
    pub scope: ConfiguredVerificationScope,
    pub budgets: VerificationBudgets,
}
/// Native literal-reference budget, independent of generated test corpora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyResolutionBudget {
    pub max_references: usize,
}
impl Default for DependencyResolutionBudget {
    fn default() -> Self {
        Self {
            max_references: 10_000,
        }
    }
}
impl DependencyResolutionBudget {
    pub(crate) fn validate(self) -> Result<()> {
        if !(1..=1_000_000).contains(&self.max_references) {
            return Err(Error::Invalid(
                "Native dependency max_references must be between 1 and 1000000".into(),
            ));
        }
        Ok(())
    }
}
/// Per-application process budgets; omitted configurations retain global defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationExecutionBudgets {
    pub initialization_seconds: u64,
    pub case_seconds: u64,
    pub run_seconds: u64,
    pub cleanup_seconds: u64,
}
impl VerificationExecutionBudgets {
    pub fn native_limits(&self) -> crate::NativeLimits {
        crate::NativeLimits {
            initialization: std::time::Duration::from_secs(self.initialization_seconds),
            case: std::time::Duration::from_secs(self.case_seconds),
            run: std::time::Duration::from_secs(self.run_seconds),
            cleanup: std::time::Duration::from_secs(self.cleanup_seconds),
            ..Default::default()
        }
    }
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
pub(crate) fn safe_file(root: &Path, relative: &str) -> Result<(PathBuf, PathBuf)> {
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
    config
        .dependency_resolution
        .unwrap_or_default()
        .validate()?;
    if let Some(execution) = &config.execution {
        execution
            .native_limits()
            .validate()
            .map_err(|error| Error::Invalid(error.to_string()))?;
    }
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
        let scaffold = generator.scaffold();
        let bundle = generator
            .generate(&HarnessRequest {
                adapter_import: format!("./{}", scaffold.path),
                subjects: scope.subjects.clone(),
            })
            .map_err(|error| Error::Invalid(error.to_string()))?;
        if bundle.sdk_version != SDK_VERSION
            || bundle.target.is_empty()
            || bundle.generator_version.is_empty()
            || bundle.recipe.program.is_empty()
            || bundle
                .build
                .as_ref()
                .is_some_and(|recipe| recipe.program.is_empty())
            || scaffold.path.contains('/')
            || scaffold.path.contains('\\')
            || !scaffold.path.starts_with("adapter.")
        {
            return Err(Error::Invalid(
                "Unsupported or invalid testing harness bundle".into(),
            ));
        }
        let base = format!(".dctx0/testing/{}", bundle.target);
        let config = VerificationConfiguration {
            dependency_resolution: Some(Default::default()),
            execution: None,
            schema_version: 1,
            target: bundle.target.clone(),
            runtime: generator.runtime().into(),
            adapter: format!("{base}/{}", scaffold.path),
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
        files.extend(generator.setup_artifacts().into_iter().map(|artifact| {
            (
                format!("{base}/{}", artifact.path),
                artifact.contents.into_bytes(),
            )
        }));
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

#[cfg(test)]
mod execution_budget_tests {
    use super::*;
    #[test]
    fn dependency_reference_budgets_are_explicit_bounded_and_strict() {
        assert_eq!(DependencyResolutionBudget::default().max_references, 10_000);
        for limit in [1, 10_000, 1_000_000] {
            assert!(
                DependencyResolutionBudget {
                    max_references: limit
                }
                .validate()
                .is_ok()
            );
        }
        for limit in [0, 1_000_001, usize::MAX] {
            assert!(
                DependencyResolutionBudget {
                    max_references: limit
                }
                .validate()
                .is_err()
            );
        }
        for json in [
            r#"{"max_references":-1}"#,
            r#"{"max_references":1.5}"#,
            r#"{"max_references":18446744073709551616}"#,
            r#"{"max_references":1,"unknown":true}"#,
            "{}",
        ] {
            assert!(
                serde_json::from_str::<DependencyResolutionBudget>(json).is_err(),
                "{json}"
            );
        }
    }
    #[test]
    fn scoped_execution_budgets_are_optional_and_reject_zero_or_overflowing_deadlines() {
        let text = r#"{"schema_version":1,"target":"rust","runtime":"cargo","adapter":".dctx0/testing/rust/adapter.rs","fixtures":".dctx0/testing/rust/examples.json","scope":{"subjects":["W"],"tasks":[]},"budgets":{"seed":42,"generated_cases":16,"collection_size":2,"recursion_depth":8,"value_bytes":65536,"shrink_steps":100,"evaluation_steps":100000}}"#;
        let mut config: VerificationConfiguration = serde_json::from_str(text).unwrap();
        assert!(config.execution.is_none());
        assert!(config.dependency_resolution.is_none());
        assert!(validate(&config).is_ok());
        config.execution = Some(VerificationExecutionBudgets {
            initialization_seconds: 60,
            case_seconds: 60,
            run_seconds: 900,
            cleanup_seconds: 5,
        });
        assert!(validate(&config).is_ok());
        assert_eq!(
            config.execution.as_ref().unwrap().native_limits().case,
            std::time::Duration::from_secs(60)
        );
        config.execution.as_mut().unwrap().case_seconds = 0;
        assert!(validate(&config).is_err());
        config.execution.as_mut().unwrap().case_seconds = u64::MAX;
        assert!(validate(&config).is_err());
    }
}
