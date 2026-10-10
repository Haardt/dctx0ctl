use dctx0_engine::Project;
use dctx0_engine::SourceReader;
use std::{
    fs,
    path::{Path, PathBuf},
};
struct Temp(PathBuf);
impl Temp {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("dctx0-engine-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
    fn write(&self, file: &str, text: &str) {
        let path = self.0.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
const SPEC: &str = "workspace W { domain D { value Id = UUID { stableId \"kept-id\" } type Item { id: Id } module M { owns { Item } surface Api { operation get(id: Id) -> Item { pure } } example \"sample\" { when result = Api.get(sampleId); expect Success } constraint \"Id and Item in narrative stay unchanged\" } } }";
#[test]
fn check_history_diff_impact_and_invalid_checks_preserve_valid_head() {
    let temp = Temp::new("history");
    temp.write("workspace.dctx0", SPEC);
    temp.write(".runatron/ignored.dctx0", "invalid");
    let p = temp.project();
    let first = p.check().unwrap();
    assert!(first.ok);
    let initial = p.load("latest").unwrap();
    assert_eq!(p.history().unwrap().len(), 1);
    let same = p.check().unwrap();
    assert_eq!(first.snapshot_id, same.snapshot_id);
    assert!(same.changes.unwrap().changes.is_empty());
    temp.write("workspace.dctx0", &SPEC.replace("kept-id", "changed-id"));
    let changed = p.check().unwrap();
    assert!(
        changed
            .changes
            .unwrap()
            .changes
            .iter()
            .any(|c| c.new.as_deref() == Some("W.D.Id"))
    );
    assert!(!p.diff(&initial.id, "latest").unwrap().changes.is_empty());
    let latest = p.load("latest").unwrap();
    temp.write("workspace.dctx0", "workspace W { type T = Missing }");
    assert!(!p.check().unwrap().ok);
    assert_eq!(p.load("latest").unwrap().id, latest.id);
    assert_eq!(p.history().unwrap()[0].status, "fail");
    assert!(p.load("unknown").is_err());
}
#[test]
fn dry_run_has_no_side_effects_and_rename_updates_only_resolved_symbols() {
    let temp = Temp::new("rename");
    temp.write("workspace.dctx0", SPEC);
    temp.write(
        "fragment.dctx0",
        "fragment D.M.Api { operation another() -> W.D.Item { pure } }",
    );
    let p = temp.project();
    let preview = p.rename("W.D.Item", "Entry", true).unwrap();
    assert!(preview.dry_run);
    assert!(preview.edits.len() >= 3);
    assert!(!temp.0.join(".dctx0ctl").exists());
    assert_eq!(
        fs::read_to_string(temp.0.join("workspace.dctx0")).unwrap(),
        SPEC
    );
    let renamed = p.rename("W.D.Item", "Entry", false).unwrap();
    assert!(!renamed.dry_run);
    let text = fs::read_to_string(temp.0.join("workspace.dctx0")).unwrap();
    assert!(text.contains("type Entry"));
    assert!(text.contains("Id and Item in narrative"));
    assert!(text.contains("stableId \"kept-id\""));
    let latest = p.load("latest").unwrap();
    assert!(latest.ir.data.contains_key("W.D.Entry"));
    assert!(
        p.diff(&renamed.before, &renamed.after)
            .unwrap()
            .changes
            .iter()
            .any(|c| matches!(c.kind, dctx0_changes::ChangeKind::Renamed))
    );
    let container = p.rename("W.D.M", "RenamedModule", false).unwrap();
    assert!(
        container
            .mappings
            .iter()
            .any(|m| m.old == "W.D.M.Api.get" && m.new == "W.D.RenamedModule.Api.get")
    );
    assert!(
        fs::read_to_string(temp.0.join("fragment.dctx0"))
            .unwrap()
            .contains("fragment D.RenamedModule.Api")
    );
    assert!(
        p.rename("W.D.RenamedModule.Api.get", "fetch", false)
            .is_ok()
    );
    assert!(
        p.rename("W.D.RenamedModule.sample", "renamedSample", false)
            .is_ok()
    );
    for (old, new) in [
        ("W.D.Id", "Id"),
        ("W.D.Id", "Entry"),
        ("Unknown", "X"),
        ("W.D.Id", "invalid.name"),
        ("W.D.Id", "type"),
        ("W.D.Id", "Int"),
        ("W.D.Id", ""),
    ] {
        assert!(p.rename(old, new, true).is_err(), "{old}->{new}");
    }
}
#[test]
fn projects_validate_roots_sources_and_database_paths() {
    let temp = Temp::new("roots");
    let project = temp.project();
    assert!(project.sources().is_err());
    temp.write("file.dctx0", "workspace W {}");
    assert!(Project::new(&temp.0.join("file.dctx0"), None).is_err());
    assert!(Project::new(Path::new("/does-not-exist/dctx0ctl"), None).is_err());
    assert!(
        Project::new(&temp.0, Some(Path::new("relative.sqlite3")))
            .unwrap()
            .database
            .is_absolute()
    );
    assert!(project.history().is_err());
    temp.write("file.dctx0", "workspace W {");
    assert!(project.rename("W", "Changed", true).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&temp.0, temp.0.join("loop")).unwrap();
        assert_eq!(project.sources().unwrap().len(), 1);
    }
}
#[test]
fn rename_chain_records_repeated_and_reversed_operations() {
    let temp = Temp::new("chain");
    temp.write("workspace.dctx0", "workspace W { type A = Int }");
    let project = temp.project();
    let a = project.rename("W.A", "B", false).unwrap();
    let b = project.rename("W.B", "A", false).unwrap();
    let c = project.rename("W.A", "B", false).unwrap();
    assert_eq!(a.after, b.before);
    assert_eq!(b.after, c.before);
    assert_eq!(a.after, c.after);
}

#[test]
fn explicit_rename_lineage_composes_across_checks_and_reverses_without_guessing() {
    let temp = Temp::new("lineage");
    temp.write(
        "spec.dctx0",
        "workspace W { type A = Int; domain D { module M {} } }",
    );
    temp.write(
        "fragment.dctx0",
        "fragment D . /* keep */ M { surface Api { operation get() -> A } }",
    );
    let project = temp.project();
    let first = project.check().unwrap().snapshot_id.unwrap();
    let rename = project.rename("W.A", "B", false).unwrap();
    temp.write(
        "spec.dctx0",
        "workspace W { type B = Int; domain D { module M {} } } // formatting",
    );
    project.check().unwrap();
    let last = project.rename("W.B", "C", false).unwrap();
    let forward = project.diff(&first, &last.after).unwrap();
    assert!(
        forward
            .changes
            .iter()
            .any(|c| c.old.as_deref() == Some("W.A")
                && c.new.as_deref() == Some("W.C")
                && c.kind == dctx0_changes::ChangeKind::Renamed)
    );
    let reverse = project.diff(&last.after, &first).unwrap();
    assert!(
        reverse
            .changes
            .iter()
            .any(|c| c.old.as_deref() == Some("W.C") && c.new.as_deref() == Some("W.A"))
    );
    assert!(
        project
            .diff(&rename.after, &rename.after)
            .unwrap()
            .changes
            .is_empty()
    );
    project.rename("W.D.M", "Changed", false).unwrap();
    assert!(
        fs::read_to_string(temp.0.join("fragment.dctx0"))
            .unwrap()
            .contains("D . /* keep */ Changed")
    );
}

#[test]
fn rename_rejects_new_ambiguities_and_preserves_unqualified_variants() {
    let temp = Temp::new("ambiguity");
    temp.write("spec.dctx0", "workspace W { type E = On | Off; domain A { type T = Int } domain B { type Other = Int } surface Api { operation get() -> T } invariant I { assert On != Off } }");
    let project = temp.project();
    assert!(project.rename("W.B.Other", "T", true).is_err());
    assert!(!temp.0.join(".dctx0ctl").exists());
    project.rename("W.E", "State", false).unwrap();
    assert!(
        fs::read_to_string(temp.0.join("spec.dctx0"))
            .unwrap()
            .contains("assert On != Off")
    );
    // Scope rename affects declarations but short references outside that scope
    // remain short when their resolution is unchanged.
    project.rename("W.A", "Changed", false).unwrap();
    assert!(
        project
            .load("latest")
            .unwrap()
            .ir
            .symbols
            .contains_key("W.Changed.T")
    );
}

#[test]
fn nested_sources_and_custom_databases_are_discovered_without_generated_inputs() {
    let temp = Temp::new("nested");
    temp.write("nested/spec.dctx0", "workspace W {}");
    temp.write("ignored.txt", "ignored");
    let project = Project::new(&temp.0, Some(&temp.0.join("state.sqlite3"))).unwrap();
    assert!(project.check().unwrap().ok);
    assert_eq!(project.sources().unwrap()[0].file, "nested/spec.dctx0");
}

#[test]
fn workspace_rename_updates_root_qualified_fragments() {
    let temp = Temp::new("workspace-rename");
    temp.write("spec.dctx0", "workspace W { domain D {} }");
    temp.write("fragment.dctx0", "fragment W.D { type T = Int }");
    let project = temp.project();
    project.rename("W", "Changed", false).unwrap();
    assert_eq!(project.load("latest").unwrap().ir.workspace, "Changed");
    assert!(
        project
            .load("latest")
            .unwrap()
            .ir
            .data
            .contains_key("Changed.D.T")
    );
}

#[test]
fn pure_rename_planning_returns_structured_diagnostics_and_preserves_inputs() {
    use dctx0_engine::{Error, RenameError, plan_rename};
    let sources = |text: &str| {
        vec![dctx0_core::Source {
            file: "spec.dctx0".into(),
            text: text.into(),
        }]
    };
    let invalid = sources("workspace W { type A = Missing }");
    let error = match plan_rename(&invalid, "W.A", "B") {
        Err(error) => error,
        Ok(_) => panic!("invalid workspace accepted"),
    };
    assert!(
        error
            .to_string()
            .starts_with("Cannot rename an invalid workspace: [")
    );
    assert!(
        matches!(error, Error::Rename(RenameError::InvalidWorkspace { diagnostics }) if diagnostics.iter().any(|d| d.code == "DCTX0_UNKNOWN_REFERENCE"))
    );
    let ambiguous = sources(
        "workspace W { domain A { type T = Int } domain B { type Other = Int } surface Api { operation get() -> T } }",
    );
    let error = match plan_rename(&ambiguous, "W.B.Other", "T") {
        Err(error) => error,
        Ok(_) => panic!("ambiguous rename accepted"),
    };
    assert!(
        error
            .to_string()
            .starts_with("Rename would invalidate the workspace: [")
    );
    assert!(
        matches!(error, Error::Rename(RenameError::InvalidResult { diagnostics }) if diagnostics.iter().any(|d| d.code == "DCTX0_AMBIGUOUS_REFERENCE"))
    );
    let valid = sources("workspace W { type A = Int; surface Api { operation get() -> A } }");
    let original = valid.clone();
    let plan = plan_rename(&valid, "W.A", "B").unwrap();
    assert_eq!(valid, original);
    assert!(plan.after.ir.data.contains_key("W.B"));
    assert_eq!(plan.before.sources, original);
    assert!(plan.after.ir.references.iter().any(|r| r.target == "W.B"));
}
