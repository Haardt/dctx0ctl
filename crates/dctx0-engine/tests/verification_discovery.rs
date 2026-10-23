use dctx0_engine::Project;
use dctx0_target_typescript::TypeScriptInspector;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-discovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("spec")).unwrap();
        fs::create_dir(root.join("app")).unwrap();
        fs::write(
            root.join("spec/w.dctx0"),
            "workspace W { value Item = Int }",
        )
        .unwrap();
        Self(root)
    }
    fn project(&self) -> Project {
        Project::new(&self.0.join("spec"), None).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn unavailable_structural_inspection_preserves_history_and_does_not_publish_a_check() {
    struct Unavailable;
    impl dctx0_target_sdk::ImplementationInspector for Unavailable {
        fn inspect(
            &self,
            _: &[dctx0_target_sdk::InspectionSource],
        ) -> Result<dctx0_target_sdk::InspectionReport, dctx0_target_sdk::Error> {
            Err(dctx0_target_sdk::Error::Invalid(
                "structural inspection unavailable".into(),
            ))
        }
    }
    let fixture = Fixture::new();
    let project = fixture.project();
    project.check().unwrap();
    let before = fs::read(&project.database).unwrap();
    let error = project
        .discover_verification(&fixture.0.join("app"), &Unavailable)
        .unwrap_err()
        .to_string();
    assert!(error.contains("structural inspection unavailable"));
    assert_eq!(fs::read(&project.database).unwrap(), before);
}

#[test]
fn discovery_is_read_only_does_not_execute_sources_and_keeps_ambiguity_visible() {
    let f = Fixture::new();
    let p = f.project();
    let app = f.0.join("app");
    assert!(p.discover_verification(&app, &TypeScriptInspector).is_err());
    assert!(!p.database.exists());
    p.check().unwrap();
    fs::write(
        app.join("one.ts"),
        "throw new Error('must never execute'); export type Item = number;",
    )
    .unwrap();
    fs::write(app.join("two.tsx"), "export const Item = () => <div/>;").unwrap();
    fs::write(app.join("bad.ts"), "export function Item( {").unwrap();
    fs::create_dir(app.join("node_modules")).unwrap();
    fs::write(app.join("node_modules/ignored.ts"), "export const Item=1;").unwrap();
    let database = fs::read(&p.database).unwrap();
    let report = p.discover_verification(&app, &TypeScriptInspector).unwrap();
    let item = report
        .subjects
        .iter()
        .find(|s| s.subject == "W.Item")
        .unwrap();
    assert_eq!(item.candidates.len(), 2);
    assert!(item.ambiguous);
    assert_eq!(report.inspection.sources.len(), 3);
    assert_eq!(report.source_hashes.len(), 3);
    assert!(
        report
            .inspection
            .sources
            .iter()
            .any(|s| s.path == "bad.ts" && s.syntax_errors)
    );
    assert_eq!(database, fs::read(&p.database).unwrap());
    assert!(!app.join(".dctx0").exists());
    fs::write(app.join("one.ts"), "export type Item = string;").unwrap();
    let changed = p.discover_verification(&app, &TypeScriptInspector).unwrap();
    assert_ne!(
        changed.source_hashes["one.ts"],
        report.source_hashes["one.ts"]
    );
    assert_eq!(report.snapshot, changed.snapshot);
    fs::write(app.join("invalid.ts"), [255]).unwrap();
    assert!(
        p.discover_verification(&app, &TypeScriptInspector)
            .unwrap_err()
            .to_string()
            .contains("UTF-8")
    );
}

#[test]
fn discovery_rejects_escaping_symlinks_large_files_and_non_directory_roots() {
    let f = Fixture::new();
    let p = f.project();
    p.check().unwrap();
    let app = f.0.join("app");
    fs::write(app.join("file.ts"), "export const Item = 1;").unwrap();
    assert!(
        p.discover_verification(&app.join("file.ts"), &TypeScriptInspector)
            .is_err()
    );
    let large = fs::File::create(app.join("large.ts")).unwrap();
    large.set_len(4 * 1024 * 1024 + 1).unwrap();
    assert!(
        p.discover_verification(&app, &TypeScriptInspector)
            .unwrap_err()
            .to_string()
            .contains("limits")
    );
    large.set_len(0).unwrap();
    let mut deep = app.clone();
    for _ in 0..65 {
        deep = deep.join("nested");
    }
    fs::create_dir_all(&deep).unwrap();
    assert!(
        p.discover_verification(&app, &TypeScriptInspector)
            .unwrap_err()
            .to_string()
            .contains("depth")
    );
    fs::remove_dir_all(app.join("nested")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(f.0.join("spec"), app.join("outside")).unwrap();
        assert!(
            p.discover_verification(&app, &TypeScriptInspector)
                .unwrap_err()
                .to_string()
                .contains("symlink")
        );
    }
}
