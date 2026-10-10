use dctx0_target_sdk::*;
use dctx0_target_typescript::TypeScriptInspector;

#[test]
fn typescript_and_tsx_inventory_preserves_exports_imports_names_and_syntax_errors() {
    let sources = vec![
        InspectionSource { path: "z.tsx".into(), language: InspectionLanguage::Tsx, text: "export const View = () => <div>Hello</div>;".into() },
        InspectionSource { path: "a.ts".into(), language: InspectionLanguage::TypeScript, text: "import { service } from './service'; export function save(id: number) { return service(id); } export interface Item { id: number } export type Id = number; export class Store {} const hidden = 1; export { hidden as visible }; export { other } from './other';".into() },
        InspectionSource { path: "bad.ts".into(), language: InspectionLanguage::TypeScript, text: "export function broken( {".into() },
    ];
    let inspector = TypeScriptInspector;
    let report = inspector.inspect(&sources).unwrap();
    assert_eq!(report, inspector.inspect(&sources).unwrap());
    assert_eq!(
        report
            .sources
            .iter()
            .map(|s| s.path.as_str())
            .collect::<Vec<_>>(),
        ["a.ts", "bad.ts", "z.tsx"]
    );
    let ts = &report.sources[0];
    assert!(!ts.syntax_errors);
    assert_eq!(ts.imports.len(), 1);
    assert_eq!(ts.exports.len(), 6);
    for name in ["save", "Item", "Id", "Store"] {
        assert!(ts.declarations.iter().any(|d| d.name == name && d.exported));
    }
    assert!(
        ts.declarations
            .iter()
            .any(|d| d.name == "hidden" && !d.exported)
    );
    assert!(
        !ts.declarations
            .iter()
            .any(|d| d.name == "visible" || d.name == "other")
    );
    for source in &report.sources {
        let original = sources.iter().find(|s| s.path == source.path).unwrap();
        for statement in source.imports.iter().chain(&source.exports) {
            assert_eq!(
                statement.text,
                original.text[statement.start_byte..statement.end_byte]
            );
        }
    }
    assert!(report.sources[1].syntax_errors);
    assert!(!report.sources[2].syntax_errors);
    assert_eq!(report.sources[2].declarations[0].name, "View");
}

#[test]
fn class_members_are_candidates_without_resolving_instances_or_private_access() {
    let report = TypeScriptInspector.inspect(&[InspectionSource { path: "service.ts".into(), language: InspectionLanguage::TypeScript, text: "export class Service { constructor() {} public save() {} private hidden() {} protected inherited() {} #secret() {} readonly load = () => 1; }".into() }]).unwrap();
    assert!(!report.sources[0].syntax_errors);
    assert_eq!(
        report.sources[0]
            .declarations
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        ["Service", "Service.save", "Service.load"]
    );
}
