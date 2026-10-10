mod error;
mod journal;
mod plan;
use crate::{AccessMode, Project, Result, SourceReader, SourceWriter, atomic_write};
use dctx0_changes::{Changes, RenameMapping, Snapshot, diff, hash, snapshot};
use dctx0_changes::{HistoryReader, HistoryWriter};
use dctx0_core::StatementKind;
use dctx0_store::RenameEvent;
pub use error::RenameError;
use journal::{FileChange, Journal};
pub use plan::plan_rename;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edit {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub old: String,
    pub new: String,
}
pub struct RenamePlan {
    pub before: Snapshot,
    pub after: Snapshot,
    pub edits: Vec<Edit>,
    pub mappings: Vec<RenameMapping>,
    pub symbol: String,
    pub new_symbol: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct RenameReport {
    pub dry_run: bool,
    pub symbol: String,
    pub new_symbol: String,
    pub before: String,
    pub after: String,
    pub edits: Vec<Edit>,
    pub mappings: Vec<RenameMapping>,
    pub changes: Changes,
}
impl Project {
    pub fn plan_rename(&self, symbol: &str, new_name: &str) -> Result<RenamePlan> {
        if self.journal_path().exists() {
            return Err(RenameError::PendingJournal.into());
        }
        plan::plan_rename(&self.sources()?, symbol, new_name)
    }
    pub fn rename(&self, symbol: &str, new_name: &str, dry_run: bool) -> Result<RenameReport> {
        if dry_run {
            return Ok(self.plan_rename(symbol, new_name)?.report(true));
        }
        let _lock = self.lock()?;
        self.recover()?;
        let plan = self.plan_rename(symbol, new_name)?;
        self.verify_sources(&plan.before.sources)?;
        let mut store = self.store(AccessMode::ReadWrite)?;
        if store
            .latest(&self.key())?
            .as_ref()
            .is_none_or(|s| s.id != plan.before.id)
        {
            store.publish(&self.key(), &plan.before, None)?;
        }
        let unique = format!(
            "{}:{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let event = RenameEvent {
            id: hash(&(&plan.before.id, &plan.after.id, &unique)),
            before: plan.before.id.clone(),
            after: plan.after.id.clone(),
            mappings: plan.mappings.clone(),
        };
        let files = plan
            .before
            .sources
            .iter()
            .zip(&plan.after.sources)
            .filter(|(a, b)| a.text != b.text)
            .map(|(a, b)| FileChange {
                file: a.file.clone(),
                before: a.text.clone(),
                after: b.text.clone(),
            })
            .collect::<Vec<_>>();
        let journal = Journal {
            schema_version: 1,
            event_id: event.id.clone(),
            database: self.database.to_string_lossy().into_owned(),
            files,
        };
        atomic_write(&self.journal_path(), &serde_json::to_vec(&journal)?)?;
        self.write_rename_files(&journal)?;
        // Include untouched and newly discovered sources in the race check.
        self.verify_sources(&plan.after.sources)?;
        store.publish(&self.key(), &plan.after, Some(&event))?;
        self.remove_journal()?;
        Ok(plan.report(false))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use plan::{reference_component, validate_edits, validate_reference};
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("dctx0-recovery-{name}-{}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("workspace.dctx0"), "workspace W { type A = Int }").unwrap();
            Self(path)
        }
        fn project(&self) -> Project {
            Project::new(&self.0, None).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn journal(project: &Project, plan: &RenamePlan, event: &str) -> Journal {
        Journal {
            schema_version: 1,
            event_id: event.into(),
            database: project.database.to_string_lossy().into_owned(),
            files: vec![FileChange {
                file: "workspace.dctx0".into(),
                before: plan.before.sources[0].text.clone(),
                after: plan.after.sources[0].text.clone(),
            }],
        }
    }
    #[test]
    fn interrupted_rename_rolls_back_or_finishes_committed_publication() {
        for committed in [false, true] {
            let temp = Temp::new(if committed { "committed" } else { "rollback" });
            let project = temp.project();
            let plan = project.plan_rename("W.A", "B").unwrap();
            let _lock = project.lock().unwrap();
            let mut store = project.store(AccessMode::ReadWrite).unwrap();
            store.publish(&project.key(), &plan.before, None).unwrap();
            let event = RenameEvent {
                id: "event".into(),
                before: plan.before.id.clone(),
                after: plan.after.id.clone(),
                mappings: plan.mappings.clone(),
            };
            let journal = journal(&project, &plan, "event");
            atomic_write(
                &project.journal_path(),
                &serde_json::to_vec(&journal).unwrap(),
            )
            .unwrap();
            if committed {
                store
                    .publish(&project.key(), &plan.after, Some(&event))
                    .unwrap();
            } else {
                atomic_write(
                    &project.root.join("workspace.dctx0"),
                    journal.files[0].after.as_bytes(),
                )
                .unwrap();
            }
            assert!(project.plan_rename("W.A", "B").is_err());
            project.recover().unwrap();
            assert!(!project.journal_path().exists());
            assert_eq!(
                fs::read_to_string(project.root.join("workspace.dctx0")).unwrap(),
                if committed {
                    &journal.files[0].after
                } else {
                    &journal.files[0].before
                }
                .as_str()
            );
        }
    }
    #[test]
    fn recovery_preserves_intervening_edits_and_rejects_unsafe_journals() {
        let temp = Temp::new("invalid");
        let project = temp.project();
        let plan = project.plan_rename("W.A", "B").unwrap();
        let _lock = project.lock().unwrap();
        let mut store = project.store(AccessMode::ReadWrite).unwrap();
        store.publish(&project.key(), &plan.before, None).unwrap();
        let mut journal = journal(&project, &plan, "event");
        fs::write(
            project.root.join("workspace.dctx0"),
            "workspace W { type C = Int }",
        )
        .unwrap();
        atomic_write(
            &project.journal_path(),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        assert!(project.recover().is_err());
        assert!(project.journal_path().exists());
        assert!(
            fs::read_to_string(project.root.join("workspace.dctx0"))
                .unwrap()
                .contains("type C")
        );
        fs::write(
            project.root.join("workspace.dctx0"),
            &journal.files[0].before,
        )
        .unwrap();
        journal.schema_version = 99;
        atomic_write(
            &project.journal_path(),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        assert!(project.recover().is_err());
        journal.schema_version = 1;
        journal.files[0].file = "../escape.dctx0".into();
        atomic_write(
            &project.journal_path(),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        assert!(project.recover().is_err());
        for file in ["../escape.dctx0", "bad.txt"] {
            assert!(project.safe_source(file).is_err());
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                project.root.join("workspace.dctx0"),
                project.root.join("link.dctx0"),
            )
            .unwrap();
            assert!(project.safe_source("link.dctx0").is_err());
        }
    }
    #[test]
    fn race_checks_and_recovery_reject_edits_without_overwriting_them() {
        let temp = Temp::new("races");
        let project = temp.project();
        let plan = project.plan_rename("W.A", "B").unwrap();
        let mut pending = journal(&project, &plan, "event");
        let _lock = project.lock().unwrap();
        let mut store = project.store(AccessMode::ReadWrite).unwrap();
        store.publish(&project.key(), &plan.before, None).unwrap();
        project.write_rename_files(&pending).unwrap();
        assert!(project.verify_sources(&plan.before.sources).is_err());
        assert!(project.write_rename_files(&pending).is_err());
        assert!(project.verify_sources(&plan.after.sources).is_ok());
        pending.files.push(FileChange {
            file: pending.files[0].file.clone(),
            before: pending.files[0].before.clone(),
            after: pending.files[0].after.clone(),
        });
        atomic_write(
            &project.journal_path(),
            &serde_json::to_vec(&pending).unwrap(),
        )
        .unwrap();
        assert!(
            project
                .recover()
                .unwrap_err()
                .to_string()
                .contains("Duplicate journal")
        );
        pending.files.pop();
        atomic_write(
            &project.journal_path(),
            &serde_json::to_vec(&pending).unwrap(),
        )
        .unwrap();
        project.recover().unwrap();
        assert_eq!(project.sources().unwrap(), plan.before.sources);
        // Recovery also handles files already restored by an earlier attempt.
        atomic_write(
            &project.journal_path(),
            &serde_json::to_vec(&pending).unwrap(),
        )
        .unwrap();
        project.recover().unwrap();
    }
    #[test]
    fn source_edits_require_exact_utf8_ranges_and_nonoverlapping_spans() {
        assert!(validate_reference("éA", 2, 3, "A").is_ok());
        assert!(validate_reference("éA", 1, 3, "A").is_err());
        assert!(validate_reference("A", 0, 1, "B").is_err());
        assert_eq!(reference_component("D . /* note */ M.Api", 2), Some(17));
        assert_eq!(reference_component("D", 1), None);
        let edit = |start, end| Edit {
            file: "spec.dctx0".into(),
            start,
            end,
            old: "A".into(),
            new: "B".into(),
        };
        assert!(validate_edits(&[edit(0, 2), edit(1, 3)]).is_err());
        assert!(validate_edits(&[edit(0, 1), edit(1, 2)]).is_ok());
    }
}
