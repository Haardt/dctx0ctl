//! Embedded agent guidance and staged filesystem installation; no network or spec writes.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};

pub const SKILL_AGENTS: &[&str] = &[
    "agents", "claude", "codex", "copilot", "cursor", "gemini", "opencode",
];
pub const SKILL_NAMES: &[&str] = &[
    "dctx0",
    "dctx0-language",
    "dctx0-implementation",
    "dctx0-config",
    "dctx0-delivery",
];
static SERIAL: AtomicU64 = AtomicU64::new(0);
#[derive(Debug, Clone, Serialize)]
pub struct SkillDescription {
    pub name: &'static str,
    pub version: u32,
    pub files: Vec<&'static str>,
}
pub fn skill_catalog() -> Vec<SkillDescription> {
    SKILL_NAMES
        .iter()
        .map(|name| SkillDescription {
            name,
            version: match *name {
                "dctx0-config" | "dctx0-delivery" => 1,
                "dctx0" | "dctx0-implementation" => 4,
                _ => 2,
            },
            files: bundle(name)
                .expect("embedded catalog")
                .keys()
                .copied()
                .collect(),
        })
        .collect()
}
fn bundle(name: &str) -> Result<BTreeMap<&'static str, &'static str>> {
    let entries = match name {
        "dctx0" => vec![
            ("SKILL.md", include_str!("../../../skills/dctx0/SKILL.md")),
            (
                "references/query.md",
                include_str!("../../../skills/query-guide.md"),
            ),
        ],
        "dctx0-language" => vec![
            (
                "SKILL.md",
                include_str!("../../../skills/dctx0-language/SKILL.md"),
            ),
            (
                "references/query.md",
                include_str!("../../../skills/query-guide.md"),
            ),
            (
                "references/language.md",
                include_str!("../../../skills/dctx0-language/references/language.md"),
            ),
        ],
        "dctx0-implementation" => vec![
            (
                "SKILL.md",
                include_str!("../../../skills/dctx0-implementation/SKILL.md"),
            ),
            (
                "references/query.md",
                include_str!("../../../skills/query-guide.md"),
            ),
            (
                "references/implementation.md",
                include_str!("../../../skills/dctx0-implementation/references/implementation.md"),
            ),
            (
                "references/work-packages.md",
                include_str!("../../../skills/dctx0-implementation/references/work-packages.md"),
            ),
        ],
        "dctx0-delivery" => vec![
            (
                "SKILL.md",
                include_str!("../../../skills/dctx0-delivery/SKILL.md"),
            ),
            (
                "references/delivery-measures.md",
                include_str!("../../../skills/dctx0-delivery/references/delivery-measures.md"),
            ),
            (
                "references/query.md",
                include_str!("../../../skills/query-guide.md"),
            ),
        ],
        "dctx0-config" => vec![
            (
                "SKILL.md",
                include_str!("../../../skills/dctx0-config/SKILL.md"),
            ),
            (
                "references/configuration.md",
                include_str!("../../../skills/dctx0-config/references/configuration.md"),
            ),
            (
                "references/query.md",
                include_str!("../../../skills/query-guide.md"),
            ),
        ],
        _ => return Err(Error::Invalid(format!("Unknown skill: {name}"))),
    };
    let mut entries = entries;
    entries.push((
        "references/workflow-measurement.md",
        include_str!("../../../skills/workflow-measurement.md"),
    ));
    Ok(entries.into_iter().collect())
}
#[derive(Debug, Clone)]
pub struct SkillLocations {
    pub home: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
    pub config_home: Option<PathBuf>,
}
impl SkillLocations {
    pub fn from_environment() -> Self {
        Self {
            home: std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))
                .map(PathBuf::from),
            codex_home: std::env::var_os("CODEX_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            config_home: std::env::var_os("XDG_CONFIG_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }
}
#[derive(Debug, Clone)]
pub struct SkillInstallRequest {
    pub project: PathBuf,
    pub agents: BTreeSet<String>,
    pub skills: BTreeSet<String>,
    pub global: bool,
    pub force: bool,
    pub dry_run: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillInstallStatus {
    Installed,
    Unchanged,
    Planned,
}
#[derive(Debug, Serialize)]
pub struct SkillInstallation {
    pub skill: String,
    pub agents: Vec<String>,
    pub directory: PathBuf,
    pub status: SkillInstallStatus,
}
#[derive(Debug, Serialize)]
pub struct SkillInstallReport {
    pub schema_version: u32,
    pub installations: Vec<SkillInstallation>,
    pub warnings: Vec<String>,
}
fn location(
    agent: &str,
    request: &SkillInstallRequest,
    locations: &SkillLocations,
) -> Result<PathBuf> {
    let project = match agent {
        "agents" | "codex" => ".agents/skills",
        "claude" => ".claude/skills",
        "copilot" => ".github/skills",
        "cursor" => ".cursor/skills",
        "gemini" => ".gemini/skills",
        "opencode" => ".opencode/skills",
        _ => return Err(Error::Invalid(format!("Unknown agent: {agent}"))),
    };
    if !request.global {
        return Ok(fs::canonicalize(&request.project)?.join(project));
    }
    if agent == "codex" {
        if let Some(root) = &locations.codex_home {
            return Ok(root.join("skills"));
        }
    }
    if agent == "opencode" {
        if let Some(root) = &locations.config_home {
            return Ok(root.join("opencode/skills"));
        }
    }
    let home=locations.home.as_ref().ok_or_else(||Error::Invalid("Global installation needs HOME/USERPROFILE or the selected agent's explicit configuration root".into()))?;
    Ok(home.join(match agent {
        "codex" => ".codex/skills",
        "copilot" => ".copilot/skills",
        "opencode" => ".config/opencode/skills",
        _ => project,
    }))
}
fn check_path(path: &Path) -> Result<()> {
    if path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(Error::Invalid(
            "Skill installation refuses parent traversal".into(),
        ));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(Error::Invalid(format!(
                    "Skill path is not a real directory: {}",
                    current.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn tree(
    root: &Path,
    path: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
    total: &mut usize,
) -> Result<()> {
    if path.components().count() - root.components().count() > 64 || out.len() > 1024 {
        return Err(Error::Invalid("Installed skill tree exceeds limits".into()));
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        if out.len() + entries.len() >= 1024 {
            return Err(Error::Invalid(
                "Installed skill tree exceeds file count".into(),
            ));
        }
        entries.push(entry?);
    }
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let metadata = fs::symlink_metadata(entry.path())?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| Error::Invalid(e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        if metadata.is_dir() {
            out.insert(format!("{relative}/"), vec![]);
            tree(root, &entry.path(), out, total)?;
        } else if metadata.is_file() && metadata.len() <= 1_048_576 {
            let mut bytes = Vec::new();
            fs::File::open(entry.path())?
                .take(1_048_577)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 1_048_576 {
                return Err(Error::Invalid("Installed skill file exceeds limit".into()));
            }
            *total += bytes.len();
            if *total > 8_388_608 {
                return Err(Error::Invalid("Installed skill exceeds byte limit".into()));
            }
            out.insert(relative, bytes);
        } else {
            return Err(Error::Invalid(format!(
                "Skill contains a symlink, special or oversized file: {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}
fn expected(name: &str) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut entries = BTreeMap::new();
    for (path, content) in bundle(name)? {
        entries.insert(path.into(), content.as_bytes().to_vec());
        let mut parent = Path::new(path).parent();
        while let Some(directory) = parent.filter(|p| !p.as_os_str().is_empty()) {
            entries.insert(format!("{}/", directory.display()), vec![]);
            parent = directory.parent();
        }
    }
    Ok(entries)
}
struct Stage {
    destination: PathBuf,
    temporary: PathBuf,
    existing: bool,
    applied: bool,
    backed_up: bool,
}
/// Preflight every destination, stage every tree, then apply with rollback on failure.
pub fn install_skills(
    request: &SkillInstallRequest,
    locations: &SkillLocations,
) -> Result<SkillInstallReport> {
    install_with_move(request, locations, |from, to| fs::rename(from, to))
}
fn install_with_move(
    request: &SkillInstallRequest,
    locations: &SkillLocations,
    mut move_directory: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<SkillInstallReport> {
    if request.agents.is_empty() || request.skills.is_empty() {
        return Err(Error::Invalid("Select at least one skill and agent".into()));
    }
    let mut destinations: BTreeMap<PathBuf, (String, Vec<String>)> = BTreeMap::new();
    for agent in &request.agents {
        let root = location(agent, request, locations)?;
        for name in &request.skills {
            bundle(name)?;
            let path = root.join(name);
            check_path(&path)?;
            destinations
                .entry(path)
                .or_insert_with(|| (name.clone(), vec![]))
                .1
                .push(agent.clone());
        }
    }
    let mut locks = Vec::new();
    if !request.dry_run {
        let roots = destinations
            .keys()
            .filter_map(|p| p.parent())
            .collect::<BTreeSet<_>>();
        for root in roots {
            check_path(root)?;
            fs::create_dir_all(root)?;
            let lock_path = root.join(".dctx0-install.lock");
            match fs::symlink_metadata(&lock_path) {
                Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
                    return Err(Error::Invalid("Skill lock is not a regular file".into()));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            let file = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(lock_path)?;
            fs2::FileExt::lock_exclusive(&file)?;
            locks.push(file);
        }
    }
    let mut report = SkillInstallReport {
        schema_version: 1,
        installations: vec![],
        warnings: vec![],
    };
    let mut pending = Vec::new();
    for (destination, (name, agents)) in destinations {
        let existing = destination.try_exists()?;
        let unchanged = if existing {
            let mut files = BTreeMap::new();
            tree(&destination, &destination, &mut files, &mut 0)?;
            files == expected(&name)?
        } else {
            false
        };
        if existing && !unchanged && !request.force {
            return Err(Error::Invalid(format!(
                "Skill differs at {}; use --force for explicit replacement",
                destination.display()
            )));
        }
        if !unchanged {
            pending.push((destination.clone(), name.clone(), existing));
        }
        report.installations.push(SkillInstallation {
            skill: name,
            agents,
            directory: destination,
            status: if unchanged {
                SkillInstallStatus::Unchanged
            } else if request.dry_run {
                SkillInstallStatus::Planned
            } else {
                SkillInstallStatus::Installed
            },
        });
    }
    if request.dry_run {
        return Ok(report);
    }
    let mut stages: Vec<Stage> = Vec::new();
    let result = (|| -> Result<()> {
        for (destination, name, existing) in pending {
            let parent = destination
                .parent()
                .ok_or_else(|| Error::Invalid("Skill destination has no parent".into()))?;
            check_path(parent)?;
            fs::create_dir_all(parent)?;
            let temporary = parent.join(format!(
                ".dctx0-skill-stage-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&temporary)?;
            stages.push(Stage {
                destination,
                temporary: temporary.clone(),
                existing,
                applied: false,
                backed_up: false,
            });
            let staged = temporary.join("new");
            fs::create_dir(&staged)?;
            for (path, content) in bundle(&name)? {
                let file = staged.join(path);
                if let Some(parent) = file.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(file, content)?;
            }
        }
        for stage in &mut stages {
            if stage.existing {
                move_directory(&stage.destination, &stage.temporary.join("old"))?;
                stage.backed_up = true;
            }
            move_directory(&stage.temporary.join("new"), &stage.destination)?;
            stage.applied = true;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut failures = vec![error.to_string()];
        for stage in stages.iter().rev() {
            if stage.applied {
                if let Err(error) = fs::remove_dir_all(&stage.destination) {
                    failures.push(error.to_string());
                    continue;
                }
            }
            if stage.backed_up {
                if let Err(error) = fs::rename(stage.temporary.join("old"), &stage.destination) {
                    failures.push(format!(
                        "Backup retained at {}: {error}",
                        stage.temporary.display()
                    ));
                    continue;
                }
            }
            if let Err(error) = fs::remove_dir_all(&stage.temporary) {
                failures.push(error.to_string());
            }
        }
        return Err(Error::Invalid(failures.join("; ")));
    }
    for stage in stages {
        if let Err(error) = fs::remove_dir_all(&stage.temporary) {
            report.warnings.push(format!(
                "Installed successfully; cleanup failed at {}: {error}",
                stage.temporary.display()
            ));
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(label: &str) -> (PathBuf, SkillInstallRequest, SkillLocations) {
        let root =
            std::env::temp_dir().join(format!("dctx0-skills-{label}-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let request = SkillInstallRequest {
            project: root.clone(),
            agents: ["agents".into(), "claude".into(), "codex".into()].into(),
            skills: SKILL_NAMES.iter().map(|s| (*s).into()).collect(),
            global: false,
            force: false,
            dry_run: false,
        };
        let locations = SkillLocations {
            home: Some(root.join("home")),
            codex_home: Some(root.join("codex-home")),
            config_home: Some(root.join("config")),
        };
        (root, request, locations)
    }
    #[test]
    fn installs_complete_embedded_trees_deduplicates_codex_and_preserves_edits() {
        let (root, mut request, locations) = fixture("install");
        let report = install_skills(&request, &locations).unwrap();
        assert_eq!(report.installations.len(), SKILL_NAMES.len() * 2);
        assert!(
            root.join(".agents/skills/dctx0-language/references/language.md")
                .is_file()
        );
        assert!(
            report
                .installations
                .iter()
                .any(|i| i.agents == ["agents", "codex"])
        );
        assert!(
            install_skills(&request, &locations)
                .unwrap()
                .installations
                .iter()
                .all(|i| matches!(i.status, SkillInstallStatus::Unchanged))
        );
        let edited = root.join(".claude/skills/dctx0/SKILL.md");
        fs::write(&edited, "user customization").unwrap();
        assert!(install_skills(&request, &locations).is_err());
        assert_eq!(fs::read_to_string(&edited).unwrap(), "user customization");
        request.force = true;
        install_skills(&request, &locations).unwrap();
        assert_eq!(
            fs::read_to_string(edited).unwrap(),
            bundle("dctx0").unwrap()["SKILL.md"]
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn preview_and_late_conflict_never_install_earlier_destinations() {
        let (root, mut request, locations) = fixture("preview");
        request.dry_run = true;
        assert!(
            install_skills(&request, &locations)
                .unwrap()
                .installations
                .iter()
                .all(|i| matches!(i.status, SkillInstallStatus::Planned))
        );
        assert!(!root.join(".agents").exists());
        request.dry_run = false;
        fs::create_dir_all(root.join(".claude/skills/dctx0")).unwrap();
        fs::write(root.join(".claude/skills/dctx0/SKILL.md"), "existing").unwrap();
        assert!(install_skills(&request, &locations).is_err());
        assert!(!root.join(".agents/skills/dctx0").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn global_roots_honor_agent_environment_without_changing_project_files() {
        let (root, mut request, mut locations) = fixture("global");
        request.global = true;
        request.skills = ["dctx0-language".into()].into();
        request.agents = SKILL_AGENTS.iter().map(|s| (*s).into()).collect();
        install_skills(&request, &locations).unwrap();
        assert!(
            root.join("codex-home/skills/dctx0-language/SKILL.md")
                .exists()
        );
        assert!(
            root.join("config/opencode/skills/dctx0-language/SKILL.md")
                .exists()
        );
        assert!(
            root.join("home/.copilot/skills/dctx0-language/SKILL.md")
                .exists()
        );
        assert!(!root.join(".agents").exists());
        locations.home = None;
        request.agents = ["codex".into()].into();
        assert!(install_skills(&request, &locations).is_ok());
        request.agents = ["claude".into()].into();
        assert!(install_skills(&request, &locations).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_application_rolls_back_all_skills_in_the_request() {
        let (root, mut request, locations) = fixture("rollback");
        request.skills = ["dctx0".into()].into();
        install_skills(&request, &locations).unwrap();
        let first = root.join(".agents/skills/dctx0/SKILL.md");
        let second = root.join(".claude/skills/dctx0/SKILL.md");
        fs::write(&first, "old first").unwrap();
        fs::write(&second, "old second").unwrap();
        request.force = true;
        let mut calls = 0;
        let result = install_with_move(&request, &locations, |from, to| {
            calls += 1;
            if calls == 4 {
                return Err(std::io::Error::other("simulated late move failure"));
            }
            fs::rename(from, to)
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(first).unwrap(), "old first");
        assert_eq!(fs::read_to_string(second).unwrap(), "old second");
        assert!(
            !fs::read_dir(root.join(".agents/skills")).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".dctx0-skill-stage"))
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unknown_selection_and_symlinks_are_rejected_even_when_forced() {
        let (root, mut request, locations) = fixture("reject");
        request.agents = ["unknown".into()].into();
        assert!(install_skills(&request, &locations).is_err());
        request.agents = ["agents".into()].into();
        request.skills = ["../outside".into()].into();
        assert!(install_skills(&request, &locations).is_err());
        #[cfg(unix)]
        {
            request.skills = ["dctx0".into()].into();
            request.force = true;
            fs::create_dir_all(root.join(".agents/skills/dctx0")).unwrap();
            std::os::unix::fs::symlink(
                root.join("missing"),
                root.join(".agents/skills/dctx0/SKILL.md"),
            )
            .unwrap();
            assert!(install_skills(&request, &locations).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
#[cfg(test)]
mod environment_tests {
    use super::*;
    #[test]
    fn global_preview_uses_the_live_agent_environment_without_writing_files() {
        let locations = SkillLocations::from_environment();
        let request = SkillInstallRequest {
            project: PathBuf::from("."),
            agents: ["codex".into()].into(),
            skills: ["dctx0".into()].into(),
            global: true,
            force: false,
            dry_run: true,
        };
        if locations.codex_home.is_none() && locations.home.is_none() {
            assert!(install_skills(&request, &locations).is_err());
            return;
        }
        let report = install_skills(&request, &locations).unwrap();
        assert_eq!(report.installations.len(), 1);
        let installation = &report.installations[0];
        assert!(matches!(installation.status, SkillInstallStatus::Planned));
        assert_eq!(installation.agents, vec!["codex"]);
        assert_eq!(installation.directory.file_name().unwrap(), "dctx0");
        assert_eq!(
            installation
                .directory
                .parent()
                .unwrap()
                .file_name()
                .unwrap(),
            "skills"
        );
        assert!(
            installation.directory.starts_with(
                locations
                    .codex_home
                    .as_ref()
                    .or(locations.home.as_ref())
                    .unwrap()
            )
        );
    }
}
