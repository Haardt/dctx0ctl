//! Local profile I/O, separate from specification roots and task semantics.
use super::*;
use dctx0_core::{ProfileDocument, ProfileKey, ProfileRole, TargetManifest, TargetProfileProvider};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileConfig {
    pub format_version: u32,
    pub default_target: String,
}
#[derive(Debug, Serialize)]
pub struct ProfileCreation {
    pub project: PathBuf,
    pub target: String,
    pub default_target: String,
    pub files: Vec<String>,
    pub dry_run: bool,
}
#[derive(Debug, Clone)]
pub struct LoadedProfiles {
    pub project_root: PathBuf,
    pub manifest: TargetManifest,
    pub documents: Vec<ProfileDocument>,
}
fn component(value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(Error::Invalid(format!(
            "Invalid profile component {value:?}"
        )));
    }
    Ok(())
}
fn safe(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = root.to_owned();
    let parts = Path::new(relative).components().collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        let std::path::Component::Normal(part) = part else {
            return Err(Error::Invalid("Profile path escapes project".into()));
        };
        path.push(part);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if index + 1 < parts.len() && !meta.is_dir() {
                return Err(Error::Invalid(format!(
                    "Profile parent is not a directory: {}",
                    path.display()
                )));
            }
            if meta.file_type().is_symlink() {
                return Err(Error::Invalid(format!(
                    "Profile symlink: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(path)
}
fn config(path: &Path) -> Result<ProfileConfig> {
    let value: ProfileConfig = serde_json::from_slice(&fs::read(path)?)?;
    if value.format_version != 1 {
        return Err(Error::Invalid(
            "Unsupported profile configuration version".into(),
        ));
    }
    component(&value.default_target)?;
    Ok(value)
}
const BEGIN: &str = "<!-- dctx0 profiles begin -->";
const END: &str = "<!-- dctx0 profiles end -->";
fn agents_text(old: &str, target: &str) -> Result<String> {
    let block = format!(
        "{BEGIN}\nUse the editable [implementation profiles](.dctx0/profiles/{target}/common.md) through `dctx0 tasks show`, `next`, or `context`. Existing project conventions take precedence. Claims pin the selected profile texts. Preview changes with `tasks profile-changes` and explicitly select completed work for adaptation.\n{END}"
    );
    let begins = old.match_indices(BEGIN).collect::<Vec<_>>();
    let ends = old.match_indices(END).collect::<Vec<_>>();
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => Ok(format!(
            "{}{block}\n",
            if old.is_empty() {
                String::new()
            } else {
                format!("{old}\n\n")
            }
        )),
        ([(start, _)], [(end, _)]) if start < end => Ok(format!(
            "{}{}{}",
            &old[..*start],
            block,
            &old[end + END.len()..]
        )),
        _ => Err(Error::Invalid("Ambiguous AGENTS.md profile markers".into())),
    }
}
fn validate_manifest(manifest: &TargetManifest) -> Result<()> {
    component(&manifest.target)?;
    let keys = manifest.standard_keys.iter().collect::<BTreeSet<_>>();
    if manifest.format_version != 1
        || manifest.template_version == 0
        || keys.len() != manifest.standard_keys.len()
        || dctx0_core::standard_profile_keys()
            .iter()
            .any(|key| !keys.contains(key))
    {
        return Err(Error::Invalid(
            "Invalid profile manifest version or standard keys".into(),
        ));
    }
    for key in &manifest.standard_keys {
        if dctx0_core::StatementKind::from_keyword(&key.semantic).is_none() {
            return Err(Error::Invalid(format!(
                "Unknown profile semantic {}",
                key.semantic
            )));
        }
    }
    Ok(())
}
pub fn create_profiles(
    root: &Path,
    provider: &dyn TargetProfileProvider,
    make_default: bool,
    agents: bool,
    dry_run: bool,
) -> Result<ProfileCreation> {
    let root = fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(Error::Invalid("Project must be a directory".into()));
    }
    let manifest = provider.manifest();
    validate_manifest(&manifest)?;
    let base = format!(".dctx0/profiles/{}", manifest.target);
    // Validate every planned path before creating directories or taking a lock.
    let templates = provider.templates();
    let mut unique = BTreeSet::new();
    for template in &templates {
        if Path::new(&template.path).is_absolute() {
            return Err(Error::Invalid("Profile path escapes project".into()));
        }
        if !unique.insert(&template.path) {
            return Err(Error::Invalid("Duplicate profile template".into()));
        }
        safe(&root, &format!("{base}/{}", template.path))?;
    }
    if !unique.contains(&"common.md".to_owned())
        || manifest
            .standard_keys
            .iter()
            .any(|key| !unique.contains(&key.path()))
    {
        return Err(Error::Invalid(
            "Provider is missing standard templates".into(),
        ));
    }
    let config_path = safe(&root, ".dctx0/profiles/config.json")?;
    let manifest_path = safe(&root, &format!("{base}/manifest.json"))?;
    let agents_path = safe(&root, "AGENTS.md")?;
    let lock_path = safe(&root, ".dctx0/profiles/profiles.lock")?;
    let _lock = if dry_run {
        None
    } else {
        fs::create_dir_all(lock_path.parent().unwrap())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        fs2::FileExt::lock_exclusive(&file)?;
        Some(file)
    };
    let current = if config_path.exists() {
        Some(config(&config_path)?)
    } else {
        None
    };
    let default_target = match &current {
        Some(config) if !make_default => config.default_target.clone(),
        _ => manifest.target.clone(),
    };
    if manifest_path.exists() {
        let existing: TargetManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        if existing != manifest {
            return Err(Error::Invalid(
                "Existing manifest differs; local profiles are not automatically upgraded".into(),
            ));
        }
    }
    let mut writes = Vec::new();
    for template in templates {
        let relative = format!("{base}/{}", template.path);
        let path = safe(&root, &relative)?;
        if path.exists() {
            if !path.is_file() {
                return Err(Error::Invalid(format!(
                    "Profile is not a file: {}",
                    path.display()
                )));
            }
        } else {
            writes.push((relative, template.text.into_bytes()));
        }
    }
    if !manifest_path.exists() {
        writes.push((
            format!("{base}/manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        ));
    }
    if agents {
        let old = if agents_path.exists() {
            fs::read_to_string(&agents_path)?
        } else {
            String::new()
        };
        let new = agents_text(&old, &default_target)?;
        if new != old {
            writes.push(("AGENTS.md".into(), new.into_bytes()));
        }
    }
    if current
        .as_ref()
        .is_none_or(|c| c.default_target != default_target)
    {
        writes.push((
            ".dctx0/profiles/config.json".into(),
            serde_json::to_vec_pretty(&ProfileConfig {
                format_version: 1,
                default_target: default_target.clone(),
            })?,
        ));
    }
    let files = writes.iter().map(|(path, _)| path.clone()).collect();
    if !dry_run {
        for (relative, bytes) in writes {
            let path = safe(&root, &relative)?;
            fs::create_dir_all(path.parent().unwrap())?;
            if relative == "AGENTS.md" || relative == ".dctx0/profiles/config.json" {
                atomic_write(&path, &bytes)?;
            } else {
                filesystem::atomic_write_new(&path, &bytes)?;
            }
        }
    }
    Ok(ProfileCreation {
        project: root,
        target: manifest.target,
        default_target,
        files,
        dry_run,
    })
}
impl Project {
    pub fn profiles(
        &self,
        project: Option<&Path>,
        target: Option<&str>,
    ) -> Result<Option<LoadedProfiles>> {
        let root = if let Some(root) = project {
            Some(fs::canonicalize(root)?)
        } else {
            self.root
                .ancestors()
                .find(|p| p.join(".dctx0/profiles/config.json").exists())
                .map(Path::to_owned)
        };
        let Some(root) = root else {
            if target.is_some() {
                return Err(Error::Invalid(
                    "Explicit target has no configured profiles".into(),
                ));
            }
            return Ok(None);
        };
        let config_path = safe(&root, ".dctx0/profiles/config.json")?;
        if !config_path.exists() {
            if target.is_some() {
                return Err(Error::Invalid(
                    "Explicit target has no configured profiles".into(),
                ));
            }
            return Ok(None);
        }
        let config = config(&config_path)?;
        let target = target.unwrap_or(&config.default_target);
        component(target)?;
        let base = format!(".dctx0/profiles/{target}");
        let manifest: TargetManifest = serde_json::from_slice(
            &fs::read(safe(&root, &format!("{base}/manifest.json"))?)
                .map_err(|e| Error::Invalid(format!("Target {target} is not configured: {e}")))?,
        )?;
        validate_manifest(&manifest)?;
        if manifest.target != target {
            return Err(Error::Invalid("Profile manifest mismatch".into()));
        }
        let mut documents = vec![];
        let mut load = |relative: String, key: Option<ProfileKey>| -> Result<()> {
            let path = safe(&root, &format!("{base}/{relative}"))?;
            if path.exists() {
                let text = fs::read_to_string(&path)?;
                documents.push(ProfileDocument {
                    target: target.into(),
                    key,
                    path: format!("{base}/{relative}"),
                    content_hash: dctx0_changes::hash(&text),
                    text,
                    subjects: vec![],
                });
            }
            Ok(())
        };
        load("common.md".into(), None)?;
        for role in [ProfileRole::Implement, ProfileRole::Use] {
            let dir = safe(&root, &format!("{base}/{}", role.as_str()))?;
            if dir.exists() {
                let mut entries = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
                entries.sort_by_key(|e| e.file_name());
                for entry in entries {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "md") {
                        let semantic = path.file_stem().unwrap().to_string_lossy().into_owned();
                        component(&semantic)?;
                        if dctx0_core::StatementKind::from_keyword(&semantic).is_none() {
                            return Err(Error::Invalid(format!(
                                "Unknown profile semantic {semantic}"
                            )));
                        }
                        let key = ProfileKey { role, semantic };
                        load(key.path(), Some(key))?;
                    }
                }
            }
        }
        Ok(Some(LoadedProfiles {
            project_root: root,
            manifest,
            documents,
        }))
    }
}
