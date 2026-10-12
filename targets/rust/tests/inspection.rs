use dctx0_target_rust::RustInspector;
use dctx0_target_sdk::*;
#[test]
fn rust_inventory_retains_public_items_imports_and_unresolved_macros() {
    let sources=vec![InspectionSource{path:"src/service.rs".into(),language:InspectionLanguage::Rust,text:"use crate::ports::Port; pub struct Service; impl Service { pub fn call(&self) {} fn hidden() {} } pub(crate) type Local = u64; pub enum Choice { A, B } domain!();".into()}];
    let report = RustInspector.inspect(&sources).unwrap();
    assert!(!report.sources[0].syntax_errors);
    let declarations = &report.sources[0].declarations;
    assert!(
        declarations
            .iter()
            .any(|d| d.name == "Service" && d.exported)
    );
    assert!(declarations.iter().any(|d| d.name == "call" && d.exported));
    assert!(
        declarations
            .iter()
            .any(|d| d.name == "hidden" && !d.exported)
    );
    assert!(
        declarations
            .iter()
            .any(|d| d.name == "Local" && !d.exported)
    );
    assert_eq!(
        RustInspector
            .dependencies(&sources)
            .unwrap()
            .references
            .len(),
        1
    );
}
#[test]
fn malformed_rust_is_reported_and_other_languages_are_ignored() {
    let sources = vec![
        InspectionSource {
            path: "bad.rs".into(),
            language: InspectionLanguage::Rust,
            text: "pub fn (".into(),
        },
        InspectionSource {
            path: "x.go".into(),
            language: InspectionLanguage::Go,
            text: "package x".into(),
        },
    ];
    assert_eq!(RustInspector.inspect(&sources).unwrap().sources.len(), 1);
    assert_eq!(
        RustInspector.dependencies(&sources).unwrap().syntax_errors,
        ["bad.rs"]
    );
}
