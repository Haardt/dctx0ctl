use dctx0_target_go::GoInspector;
use dctx0_target_sdk::*;
#[test]
fn go_inventory_retains_alias_imports_methods_types_and_ambiguity() {
    let sources=vec![InspectionSource{path:"service.go".into(),language:InspectionLanguage::Go,text:"package cinema\nimport (http \"net/http\"; _ `modernc.org/sqlite`)\ntype Service struct {}\ntype Port interface { Call() }\nfunc (s *Service) Call() {}\nfunc hidden() {}\n".into()}];
    let report = GoInspector.inspect(&sources).unwrap();
    assert!(!report.sources[0].syntax_errors);
    assert!(
        report.sources[0]
            .declarations
            .iter()
            .any(|d| d.name == "Service" && d.exported)
    );
    assert!(
        report.sources[0]
            .declarations
            .iter()
            .any(|d| d.name == "Call" && d.exported)
    );
    assert!(
        report.sources[0]
            .declarations
            .iter()
            .any(|d| d.name == "hidden" && !d.exported)
    );
    let dependencies = GoInspector.dependencies(&sources).unwrap();
    assert_eq!(dependencies.references.len(), 2);
    assert_eq!(
        dependencies.references[1].specifier.as_deref(),
        Some("modernc.org/sqlite")
    );
}
#[test]
fn malformed_go_remains_a_syntax_issue_and_non_go_sources_are_ignored() {
    let sources = vec![
        InspectionSource {
            path: "bad.go".into(),
            language: InspectionLanguage::Go,
            text: "package x\nfunc (".into(),
        },
        InspectionSource {
            path: "view.ts".into(),
            language: InspectionLanguage::TypeScript,
            text: "not Go".into(),
        },
    ];
    let report = GoInspector.inspect(&sources).unwrap();
    assert_eq!(report.sources.len(), 1);
    assert!(report.sources[0].syntax_errors);
    assert_eq!(
        GoInspector.dependencies(&sources).unwrap().syntax_errors,
        ["bad.go"]
    );
}
