//! Bounded, read-only source inventory. No adapter evaluation or filesystem writes.
use crate::*;
use dctx0_target_sdk::{
    ImplementationInspector, InspectionLanguage, InspectionReport, InspectionSource,
};
use std::collections::BTreeMap;
use std::io::Read;

const MAX_FILES: usize = 10_000;
const MAX_FILE: u64 = 4 * 1024 * 1024;
const MAX_TOTAL: u64 = 128 * 1024 * 1024;

#[cfg(all(test, unix))]
mod path_tests {
    use super::*;
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    #[test]
    fn non_utf8_source_names_are_rejected_without_lossy_identity_collisions() {
        let root =
            std::env::temp_dir().join(format!("dctx0-discovery-utf8-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(OsString::from_vec(vec![0xff, b'.', b'r', b's'])),
            "pub fn actual() {}",
        )
        .unwrap();
        let result = inventory(
            &root,
            &root,
            &mut Vec::new(),
            &mut BTreeMap::new(),
            &mut 0,
            &mut 0,
        );
        assert!(matches!(result, Err(Error::Invalid(message)) if message.contains("UTF-8")));
        fs::remove_dir_all(root).unwrap();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryCandidate {
    pub path: String,
    pub declaration: String,
    pub start_byte: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverySubject {
    pub subject: String,
    pub candidates: Vec<DiscoveryCandidate>,
    pub ambiguous: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDiscovery {
    pub schema_version: u32,
    pub snapshot: String,
    pub check_occurrence: i64,
    pub application: String,
    pub source_hashes: BTreeMap<String, String>,
    pub inspection: InspectionReport,
    pub subjects: Vec<DiscoverySubject>,
    pub limitations: Vec<String>,
}

fn inventory(
    root: &Path,
    directory: &Path,
    sources: &mut Vec<InspectionSource>,
    hashes: &mut BTreeMap<String, String>,
    total: &mut u64,
    visited: &mut usize,
) -> Result<()> {
    if directory.components().count() - root.components().count() > 64 {
        return Err(Error::Invalid(
            "Discovery inventory exceeds directory depth limit".into(),
        ));
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        if *visited >= 100_000 {
            return Err(Error::Invalid(
                "Discovery inventory exceeds directory entry limit".into(),
            ));
        }
        *visited += 1;
        entries.push(entry?);
    }
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(
                "node_modules"
                    | ".git"
                    | ".dctx0"
                    | ".dctx0ctl"
                    | ".runatron"
                    | "target"
                    | "dist"
                    | "coverage"
            )
        ) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::Invalid(format!(
                "Discovery refuses symlink {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            inventory(root, &path, sources, hashes, total, visited)?;
            continue;
        }
        let language = match path.extension().and_then(|x| x.to_str()) {
            Some("ts" | "mts" | "cts") => InspectionLanguage::TypeScript,
            Some("tsx") => InspectionLanguage::Tsx,
            Some("go") => InspectionLanguage::Go,
            Some("rs") => InspectionLanguage::Rust,
            _ => continue,
        };
        if !metadata.is_file() || metadata.len() > MAX_FILE || sources.len() >= MAX_FILES {
            return Err(Error::Invalid(
                "Discovery inventory exceeds file/count limits or contains a nonregular source"
                    .into(),
            ));
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)?
            .take(MAX_FILE + 1)
            .read_to_end(&mut bytes)?;
        *total += bytes.len() as u64;
        if bytes.len() as u64 > MAX_FILE || *total > MAX_TOTAL {
            return Err(Error::Invalid(
                "Discovery inventory exceeds source byte limits".into(),
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|e| Error::Invalid(e.to_string()))?
            .to_str()
            .ok_or_else(|| Error::Invalid("Discovery requires UTF-8 source paths".into()))?
            .to_string();
        let hash = dctx0_store::VerificationArtifact::new("text/typescript", bytes.clone())
            .reference
            .hash;
        let text = String::from_utf8(bytes).map_err(|e| {
            Error::Invalid(format!("Discovery source {relative} is not UTF-8: {e}"))
        })?;
        hashes.insert(relative.clone(), hash);
        sources.push(InspectionSource {
            path: relative,
            language,
            text,
        });
    }
    Ok(())
}

impl Project {
    pub fn discover_verification(
        &self,
        application: &Path,
        inspector: &dyn ImplementationInspector,
    ) -> Result<VerificationDiscovery> {
        let root = fs::canonicalize(application)?;
        if !root.is_dir() {
            return Err(Error::Invalid(
                "Discovery application root must be a directory".into(),
            ));
        }
        let store = self.store(AccessMode::ReadOnly)?;
        let (check_occurrence, snapshot) = store.task_check(&self.key())?;
        let snapshot = store.load(&self.key(), &snapshot)?;
        let mut sources = Vec::new();
        let mut hashes = BTreeMap::new();
        inventory(&root, &root, &mut sources, &mut hashes, &mut 0, &mut 0)?;
        let inspection = inspector
            .inspect(&sources)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let subjects = snapshot
            .ir
            .symbols
            .keys()
            .map(|subject| {
                let name = subject.rsplit('.').next().unwrap_or(subject);
                let candidates = inspection
                    .sources
                    .iter()
                    .filter(|s| !s.syntax_errors)
                    .flat_map(|s| {
                        s.declarations
                            .iter()
                            .filter(move |d| d.exported && d.name.rsplit('.').next() == Some(name))
                            .map(move |d| DiscoveryCandidate {
                                path: s.path.clone(),
                                declaration: d.name.clone(),
                                start_byte: d.start_byte,
                            })
                    })
                    .collect::<Vec<_>>();
                DiscoverySubject {
                    subject: subject.clone(),
                    ambiguous: candidates.len() > 1,
                    candidates,
                }
            })
            .collect();
        Ok(VerificationDiscovery { schema_version: 1, snapshot: snapshot.id, check_occurrence, application: root.to_string_lossy().into_owned(), source_hashes: hashes, inspection, subjects, limitations: vec![
            "Static syntax inspection only: no application or adapter was executed and no task was adopted".into(),
            "Name matches are unconfirmed candidates, including single matches; renamed/DI/HTTP relationships require a reviewed adapter".into(),
            "No semantic type proof, runtime call-count proof, complete dependency graph or production import-boundary proof".into(),
            "Published snapshot requirements are reported; discovery does not publish or assert working-tree spec freshness".into(),
        ] })
    }
}
