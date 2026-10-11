use dctx0_target_sdk::*;
use dctx0_target_typescript::TypeScriptDependencies;

#[test]
fn dependencies_preserve_literal_reexports_and_unresolved_dynamic_expressions() {
    let source = InspectionSource { path: "src/app.ts".into(), language: InspectionLanguage::TypeScript,
        text: "import {service} from './service.ts'; export {value} from './value.ts'; import('./lazy.ts'); import(name); require('package'); require(name); eval(code); new Function(code);".into() };
    let report = TypeScriptDependencies
        .dependencies(std::slice::from_ref(&source))
        .unwrap();
    assert!(report.syntax_errors.is_empty());
    assert_eq!(
        report
            .references
            .iter()
            .map(|reference| (reference.kind.clone(), reference.specifier.clone()))
            .collect::<Vec<_>>(),
        vec![
            (DependencyKind::Import, Some("./service.ts".into())),
            (DependencyKind::Reexport, Some("./value.ts".into())),
            (DependencyKind::DynamicImport, Some("./lazy.ts".into())),
            (DependencyKind::DynamicImport, None),
            (DependencyKind::Require, Some("package".into())),
            (DependencyKind::Require, None),
            (DependencyKind::DynamicCode, None),
            (DependencyKind::DynamicCode, None),
        ]
    );
    for reference in &report.references {
        assert_eq!(reference.source, source.path);
        assert!(reference.start_byte < reference.end_byte);
        assert!(reference.end_byte <= source.text.len());
    }
    assert_eq!(
        report,
        TypeScriptDependencies.dependencies(&[source]).unwrap()
    );
}

#[test]
fn dependency_sources_are_sorted_and_syntax_errors_are_explicit() {
    let sources = [
        InspectionSource {
            path: "z.tsx".into(),
            language: InspectionLanguage::Tsx,
            text: "export const view = <div/>; import(unknown)".into(),
        },
        InspectionSource {
            path: "a.ts".into(),
            language: InspectionLanguage::TypeScript,
            text: "import broken {".into(),
        },
    ];
    let report = TypeScriptDependencies.dependencies(&sources).unwrap();
    assert_eq!(report.syntax_errors, vec!["a.ts"]);
    assert_eq!(report.references.last().unwrap().source, "z.tsx");
    assert_eq!(report.references.last().unwrap().specifier, None);
}

#[test]
fn commonjs_member_loaders_preserve_literal_and_unknown_dependencies() {
    let source = InspectionSource {
        path: "app.cjs".into(),
        language: InspectionLanguage::TypeScript,
        text: "module.require('./actual.cjs'); module . require(name); require.resolve('./located.cjs'); module['require']('./indexed.cjs');".into(),
    };
    let report = TypeScriptDependencies.dependencies(&[source]).unwrap();
    assert!(report.syntax_errors.is_empty());
    assert_eq!(
        report
            .references
            .iter()
            .map(|r| (r.kind.clone(), r.specifier.clone()))
            .collect::<Vec<_>>(),
        vec![
            (DependencyKind::Require, Some("./actual.cjs".into())),
            (DependencyKind::Require, None),
            (DependencyKind::Require, Some("./located.cjs".into())),
            (DependencyKind::Require, Some("./indexed.cjs".into())),
        ]
    );
}

#[test]
fn escaped_commonjs_loaders_remain_unknown_without_inventing_their_calls() {
    let source = InspectionSource {
        path: "app.cjs".into(), language: InspectionLanguage::TypeScript,
        text: "const load = module.require; load(name); const resolve = require['resolve']; consume(resolve);".into(),
    };
    let report = TypeScriptDependencies.dependencies(&[source]).unwrap();
    assert_eq!(report.references.len(), 2);
    assert!(
        report
            .references
            .iter()
            .all(|reference| reference.kind == DependencyKind::DynamicCode
                && reference.specifier.is_none())
    );
}

#[test]
fn custom_loader_members_and_require_factories_do_not_claim_a_closed_dependency_set() {
    let source = InspectionSource {
        path: "loader.ts".into(), language: InspectionLanguage::TypeScript,
        text: "dependencies.require(path); createRequire(import.meta.url); modules.createRequire(file); container.resolve('service');".into(),
    };
    let report = TypeScriptDependencies.dependencies(&[source]).unwrap();
    assert_eq!(report.references.len(), 3);
    assert!(
        report
            .references
            .iter()
            .all(|r| r.kind == DependencyKind::DynamicCode && r.specifier.is_none())
    );
}

#[test]
fn escaped_custom_loaders_retain_uncertainty_without_inventing_runtime_calls() {
    let source = InspectionSource {
        path: "escaped-loader.mjs".into(),
        language: InspectionLanguage::TypeScript,
        text: "const load = dependencies.require; load(path); const create = modules['createRequire']; create(url); const ordinary = container.resolve; ordinary('service'); dependencies.require = load;".into(),
    };
    let report = TypeScriptDependencies
        .dependencies(std::slice::from_ref(&source))
        .unwrap();
    assert!(report.syntax_errors.is_empty());
    assert_eq!(report.references.len(), 2);
    assert!(
        report
            .references
            .iter()
            .all(|reference| reference.kind == DependencyKind::DynamicCode
                && reference.specifier.is_none())
    );
    assert_eq!(
        report
            .references
            .iter()
            .map(|reference| &source.text[reference.start_byte..reference.end_byte])
            .collect::<Vec<_>>(),
        ["dependencies.require", "modules['createRequire']"]
    );
    let artifacts = dctx0_target_typescript::BunHarness
        .module_observer_artifacts(&[source], "./")
        .unwrap();
    let manifest: ModuleObserverManifest = serde_json::from_str(&artifacts[0].contents).unwrap();
    assert_eq!(manifest.observer_version, "dctx0-esm-observer-2");
    assert_eq!(manifest.sources[0].sites.len(), 2);
    // Whole-expression sites observe access, without claiming a loader call.
    assert!(
        manifest.sources[0].sites.iter().all(
            |site| site.start_byte == site.argument_start && site.end_byte == site.argument_end
        )
    );
}

#[test]
fn esm_observer_manifests_pin_only_supported_actual_argument_ranges() {
    use dctx0_target_typescript::BunHarness;
    let sources = [
        InspectionSource { path: "library.mjs".into(), language: InspectionLanguage::TypeScript, text: "export function load(path){return import(path)}; export function required(path){return dependencies.require(path)}; export function unsafe(code){return eval(code)}; require(...args); require.resolve(name);".into() },
        InspectionSource { path: "legacy.cjs".into(), language: InspectionLanguage::TypeScript, text: "require(name)".into() },
        InspectionSource { path: "nested.mjs".into(), language: InspectionLanguage::TypeScript, text: "export function nested(path){return import(resolve(import(path)))}".into() },
        InspectionSource { path: "broken.mjs".into(), language: InspectionLanguage::TypeScript, text: "import {".into() },
    ];
    let artifacts = BunHarness
        .module_observer_artifacts(&sources, "../../")
        .unwrap();
    assert_eq!(artifacts.len(), 1);
    let manifest: ModuleObserverManifest = serde_json::from_str(&artifacts[0].contents).unwrap();
    assert_eq!(manifest.project_root_relative, "../../");
    assert_eq!(manifest.sources.len(), 1);
    let source = &manifest.sources[0];
    assert_eq!(source.source, "library.mjs");
    assert_eq!(source.sha256.len(), 64);
    assert_eq!(source.sites.len(), 3);
    for site in &source.sites {
        assert!(matches!(
            &sources[0].text[site.argument_start..site.argument_end],
            "path" | "name"
        ));
    }
    assert_eq!(
        artifacts,
        BunHarness
            .module_observer_artifacts(&sources, "../../")
            .unwrap()
    );
    assert!(
        BunHarness
            .module_observer_artifacts(&[], "./")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn observer_manifests_cover_multiple_modules_in_canonical_order_without_static_import_sites() {
    let sources = [
        InspectionSource {
            path: "z.mjs".into(),
            language: InspectionLanguage::TypeScript,
            text: "import('./static.mjs'); import(second); import(first);".into(),
        },
        InspectionSource {
            path: "a.mjs".into(),
            language: InspectionLanguage::TypeScript,
            text: "export function load(name) { return import(name); }".into(),
        },
    ];
    let artifacts = dctx0_target_typescript::BunHarness
        .module_observer_artifacts(&sources, "./")
        .unwrap();
    let manifest: ModuleObserverManifest = serde_json::from_str(&artifacts[0].contents).unwrap();
    assert_eq!(
        manifest
            .sources
            .iter()
            .map(|source| source.source.as_str())
            .collect::<Vec<_>>(),
        ["a.mjs", "z.mjs"]
    );
    assert_eq!(manifest.sources[0].sites.len(), 1);
    assert_eq!(manifest.sources[1].sites.len(), 2);
    assert_eq!(
        manifest.sources[1]
            .sites
            .iter()
            .map(|site| &sources[0].text[site.argument_start..site.argument_end])
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    let mut reversed = sources.clone();
    reversed.reverse();
    assert_eq!(
        artifacts,
        dctx0_target_typescript::BunHarness
            .module_observer_artifacts(&reversed, "./")
            .unwrap()
    );
}
