//! Inspect captured implementation inputs; unresolved dependencies block adoption.
use super::*;
use dctx0_target_sdk::{InspectionLanguage, InspectionSource};
fn relative(source: &str, specifier: &str) -> Option<String> {
    let mut segments = source.split('/').collect::<Vec<_>>();
    segments.pop();
    for segment in specifier.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            _ => segments.push(segment),
        }
    }
    Some(segments.join("/"))
}
pub(super) fn inspect(
    capture: &Capture,
    generator: &dyn HarnessGenerator,
    generated: &BTreeSet<String>,
    artifacts: &mut Vec<VerificationArtifact>,
) -> Result<Vec<VerificationIssue>> {
    let mut sources = vec![];
    for path in capture.files.keys() {
        let language = match Path::new(path).extension().and_then(|value| value.to_str()) {
            Some("ts" | "js" | "mjs" | "cjs") => InspectionLanguage::TypeScript,
            Some("tsx" | "jsx") => InspectionLanguage::Tsx,
            _ => continue,
        };
        sources.push(InspectionSource {
            path: path.clone(),
            language,
            text: fs::read_to_string(capture.root.join(path))?,
        });
    }
    let report = match generator.inspect_dependencies(&sources) {
        Ok(report) => report,
        Err(error) => {
            return Ok(vec![issue(
                "DEPENDENCY_INSPECTION_UNAVAILABLE",
                error.to_string(),
            )]);
        }
    };
    let expected_sources = sources
        .iter()
        .map(|source| source.path.as_str())
        .collect::<BTreeSet<_>>();
    let reported_sources = report
        .sources
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let report_valid = report.schema_version == 1
        && !report.analyzer_version.trim().is_empty()
        && reported_sources == expected_sources
        && reported_sources.len() == report.sources.len()
        && report
            .syntax_errors
            .iter()
            .all(|path| expected_sources.contains(path.as_str()))
        && report.references.iter().all(|reference| {
            sources.iter().any(|source| {
                source.path == reference.source
                    && reference.start_byte < reference.end_byte
                    && reference.end_byte <= source.text.len()
                    && source.text.is_char_boundary(reference.start_byte)
                    && source.text.is_char_boundary(reference.end_byte)
                    && reference
                        .specifier
                        .as_ref()
                        .is_none_or(|specifier| !specifier.is_empty())
            })
        });
    if !report_valid {
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.dependencies+json",
            serde_json::to_vec(&report)?,
        ));
        return Ok(vec![issue(
            "DEPENDENCY_REPORT_INVALID",
            "Dependency inspection does not cover the actual source inventory or contains invalid source ranges",
        )]);
    }
    let mut issues = report
        .syntax_errors
        .iter()
        .map(|path| issue("DEPENDENCY_SYNTAX_UNRESOLVED", path))
        .collect::<Vec<_>>();
    for reference in &report.references {
        let Some(specifier) = &reference.specifier else {
            issues.push(issue(
                "DYNAMIC_DEPENDENCY_UNRESOLVED",
                format!("{}:{}", reference.source, reference.start_byte),
            ));
            continue;
        };
        if specifier.starts_with("node:") || specifier.starts_with("bun:") {
            continue;
        }
        if !specifier.starts_with('.') {
            issues.push(issue(
                "PACKAGE_DEPENDENCY_UNASSESSED",
                format!(
                    "{} imports {specifier}; package resolution is not yet verified",
                    reference.source
                ),
            ));
            continue;
        }
        let Some(path) = relative(&reference.source, specifier) else {
            issues.push(issue(
                "DEPENDENCY_OUTSIDE_CAPTURE",
                format!("{} imports {specifier}", reference.source),
            ));
            continue;
        };
        // Resolve only exact captured paths. Extension/package guessing is not
        // a proof of the runtime resolver's selection.
        if !reference.source.starts_with(".dctx0/testing/") && path.starts_with(".dctx0/testing/") {
            issues.push(issue(
                "PRODUCTION_TESTING_DEPENDENCY",
                format!("{} imports testing artifact {path}", reference.source),
            ));
        }
        if !capture.files.contains_key(&path) && !generated.contains(&path) {
            issues.push(issue(
                "DEPENDENCY_NOT_CAPTURED",
                format!("{} imports {specifier}", reference.source),
            ));
        }
    }
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.dependencies+json",
        serde_json::to_vec(&report)?,
    ));
    Ok(issues)
}

#[derive(Debug, Serialize, Deserialize)]
struct ResolvedDependency {
    source: String,
    specifier: String,
    mode: dctx0_target_sdk::DependencyResolution,
    path: Option<String>,
    captured: bool,
}
/// Native resolution is additional evidence; it does not erase unresolved
/// expressions or establish transitive package closure by itself.
pub(super) async fn resolve_native(
    worker: &mut NativeWorker,
    root: &Path,
    adapter: &str,
    generated: &BTreeSet<String>,
    run: &mut VerificationRun,
    artifacts: &mut Vec<VerificationArtifact>,
    cancel: &NativeCancellation,
) -> Result<()> {
    if run
        .provenance
        .incomplete
        .iter()
        .any(|issue| issue.code == "DEPENDENCY_REPORT_INVALID")
    {
        return Ok(());
    }
    let Some(artifact) = artifacts.iter().find(|artifact| {
        artifact.reference.media_type == "application/vnd.dctx0.dependencies+json"
    }) else {
        return Ok(());
    };
    let report: dctx0_target_sdk::DependencyInspection =
        serde_json::from_slice(&artifact.contents)?;
    let Some(description) = artifacts.iter().find(|artifact| {
        artifact.reference.media_type == "application/vnd.dctx0.adapter-description+json"
    }) else {
        return Ok(());
    };
    let description: dctx0_target_sdk::AdapterDescription =
        serde_json::from_slice(&description.contents)?;
    let roots = description
        .artifacts
        .iter()
        .cloned()
        .chain(
            description
                .subjects
                .iter()
                .map(|subject| subject.artifact.clone()),
        )
        .chain(std::iter::once(adapter.to_owned()))
        .collect::<BTreeSet<_>>();
    if roots.iter().any(|path| {
        !run.provenance.implementation.contains_key(path)
            && !run.provenance.dependencies.contains_key(path)
            && !generated.contains(path)
    }) {
        run.provenance.incomplete.push(issue(
            "DEPENDENCY_ROOT_NOT_CAPTURED",
            "Adapter dependency roots must belong to the captured inventory",
        ));
        return Ok(());
    }
    // Rebuild dependency diagnostics from the evidenced execution roots. The
    // original complete structural inspection remains an immutable artifact.
    run.provenance.incomplete.retain(|issue| {
        !matches!(
            issue.code.as_str(),
            "DEPENDENCY_SYNTAX_UNRESOLVED"
                | "DYNAMIC_DEPENDENCY_UNRESOLVED"
                | "PACKAGE_DEPENDENCY_UNASSESSED"
                | "DEPENDENCY_OUTSIDE_CAPTURE"
                | "PRODUCTION_TESTING_DEPENDENCY"
                | "DEPENDENCY_NOT_CAPTURED"
        )
    });
    let mut pending = roots.clone();
    let mut visited = BTreeSet::new();
    let mut resolved = Vec::new();
    'closure: while let Some(source) = pending.pop_first() {
        if !visited.insert(source.clone()) {
            continue;
        }
        if report.syntax_errors.contains(&source) {
            run.provenance
                .incomplete
                .push(issue("DEPENDENCY_SYNTAX_UNRESOLVED", source.clone()));
        }
        for reference in report
            .references
            .iter()
            .filter(|reference| reference.source == source)
        {
            let Some(specifier) = &reference.specifier else {
                run.provenance.incomplete.push(issue(
                    "DYNAMIC_DEPENDENCY_UNRESOLVED",
                    format!("{}:{}", reference.source, reference.start_byte),
                ));
                continue;
            };
            if resolved.len() >= 1000 {
                run.provenance.incomplete.push(issue(
                    "DEPENDENCY_RESOLUTION_BUDGET",
                    "Native dependency resolution exceeded 1000 references",
                ));
                break 'closure;
            }
            let mode = if reference.kind == dctx0_target_sdk::DependencyKind::Require {
                dctx0_target_sdk::DependencyResolution::Require
            } else {
                dctx0_target_sdk::DependencyResolution::Import
            };
            let importer = root.join(&reference.source).to_string_lossy().into_owned();
            let reply = worker
                .request(
                    WorkerRequest::ResolveDependency {
                        mode,
                        specifier: specifier.clone(),
                        importer,
                    },
                    cancel,
                    &mut |_: &Invocation| {
                        Ok(Observation::Unavailable {
                            reason: "Dependency resolution cannot invoke collaborators".into(),
                        })
                    },
                )
                .await;
            match reply {
                Ok(WorkerReply::DependencyResolved { path }) => {
                    if (path.starts_with("node:")
                        && (path == *specifier || path == format!("node:{specifier}")))
                        || (path.starts_with("bun:") && path == *specifier)
                        || (path == "bun" && specifier == "bun")
                    {
                        resolved.push(ResolvedDependency {
                            source: reference.source.clone(),
                            specifier: specifier.clone(),
                            mode,
                            path: Some(path),
                            captured: false,
                        });
                        continue;
                    }
                    let captured = Path::new(&path)
                        .strip_prefix(root)
                        .ok()
                        .and_then(|path| path.to_str())
                        .filter(|path| {
                            run.provenance.implementation.contains_key(*path)
                                || run.provenance.dependencies.contains_key(*path)
                                || generated.contains(*path)
                        });
                    if let Some(path) = captured {
                        if path.starts_with("node_modules/") {
                            let mut directory = Path::new(path).parent();
                            let mut manifest = false;
                            while let Some(parent) = directory {
                                if parent == Path::new("node_modules")
                                    || parent.as_os_str().is_empty()
                                {
                                    break;
                                }
                                let candidate = parent
                                    .join("package.json")
                                    .to_string_lossy()
                                    .replace('\\', "/");
                                if run.provenance.dependencies.contains_key(&candidate) {
                                    manifest = true;
                                    break;
                                }
                                directory = parent.parent();
                            }
                            if !manifest {
                                run.provenance
                                    .incomplete
                                    .push(issue("PACKAGE_MANIFEST_NOT_CAPTURED", path));
                            }
                        }
                        if !reference.source.starts_with(".dctx0/testing/")
                            && path.starts_with(".dctx0/testing/")
                        {
                            run.provenance.incomplete.push(issue(
                                "PRODUCTION_TESTING_DEPENDENCY",
                                format!("{} imports testing artifact {path}", reference.source),
                            ));
                        }
                        if report.sources.iter().any(|source| source == path) {
                            pending.insert(path.to_owned());
                        } else if !generated.contains(path)
                            && Path::new(path).extension().and_then(|value| value.to_str())
                                != Some("json")
                        {
                            run.provenance
                                .incomplete
                                .push(issue("DEPENDENCY_SOURCE_UNASSESSED", path));
                        }
                    }
                    if captured.is_none() {
                        run.provenance.incomplete.push(issue(
                            "RESOLVED_DEPENDENCY_NOT_CAPTURED",
                            format!(
                                "{} resolves {specifier} outside the recorded input inventory",
                                reference.source
                            ),
                        ));
                    }
                    resolved.push(ResolvedDependency {
                        source: reference.source.clone(),
                        specifier: specifier.clone(),
                        mode,
                        path: Some(captured.map(str::to_owned).unwrap_or(path.clone())),
                        captured: captured.is_some(),
                    });
                }
                Ok(WorkerReply::DependencyUnresolved { reason }) => {
                    run.provenance.incomplete.push(issue(
                        "NATIVE_DEPENDENCY_UNRESOLVED",
                        format!("{} imports {specifier}: {reason}", reference.source),
                    ));
                    resolved.push(ResolvedDependency {
                        source: reference.source.clone(),
                        specifier: specifier.clone(),
                        mode,
                        path: None,
                        captured: false,
                    });
                }
                other => {
                    run.provenance.incomplete.push(issue(
                        "NATIVE_DEPENDENCY_RESOLUTION_UNAVAILABLE",
                        format!("{} imports {specifier}: {other:?}", reference.source),
                    ));
                    resolved.push(ResolvedDependency {
                        source: reference.source.clone(),
                        specifier: specifier.clone(),
                        mode,
                        path: None,
                        captured: false,
                    });
                    break 'closure;
                }
            }
        }
    }
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.dependency-scope+json",
        serde_json::to_vec(&serde_json::json!({"schema_version":1,"roots":roots,"visited":visited,"not_selected":report.sources.iter().filter(|source|!visited.contains(*source)).collect::<Vec<_>>()}))?,
    ));
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.resolved-dependencies+json",
        serde_json::to_vec(&resolved)?,
    ));
    Ok(())
}

/// Actual instrumentation can discharge an unexecuted dynamic call site for this
/// run. Observed calls remain unknown until their native dependency closure is
/// separately established; a resolver candidate alone is insufficient.
pub(super) fn reconcile_observer(
    run: &mut VerificationRun,
    artifacts: &mut Vec<VerificationArtifact>,
    evidence: &dctx0_target_sdk::ModuleObserverEvidence,
) -> Result<()> {
    use dctx0_target_sdk::{ModuleObserverManifest, ModuleObserverSite};
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.module-observer-evidence+json",
        serde_json::to_vec(evidence)?,
    ));
    let Some(manifest) = artifacts.iter().find(|artifact| {
        artifact.reference.media_type == "application/vnd.dctx0.module-observer-manifest+json"
    }) else {
        run.provenance.incomplete.push(issue(
            "MODULE_OBSERVER_MANIFEST_REQUIRED",
            "Observed dependency evidence has no generated manifest",
        ));
        return Ok(());
    };
    let manifest: ModuleObserverManifest = serde_json::from_slice(&manifest.contents)?;
    let loaded = evidence.loaded_sources.iter().collect::<BTreeSet<_>>();
    let valid_site = |source: &str, start: usize| {
        manifest.sources.iter().any(|entry| {
            entry.source == source && entry.sites.iter().any(|site| site.start_byte == start)
        })
    };
    let source_hash = |source: &str| {
        run.provenance
            .implementation
            .get(source)
            .or_else(|| run.provenance.dependencies.get(source))
    };
    let valid_range = |site: &ModuleObserverSite| {
        site.start_byte <= site.argument_start
            && site.argument_start < site.argument_end
            && site.argument_end <= site.end_byte
    };
    let valid = manifest.schema_version == 1
        && evidence.schema_version == 1
        && matches!(
            manifest.observer_version.as_str(),
            "dctx0-esm-observer-1" | "dctx0-esm-observer-2"
        )
        && evidence.observer_version == manifest.observer_version
        && evidence.complete
        && loaded.len() == evidence.loaded_sources.len()
        && manifest
            .sources
            .iter()
            .map(|source| &source.source)
            .collect::<BTreeSet<_>>()
            .len()
            == manifest.sources.len()
        && manifest.sources.iter().all(|source| {
            source.source.ends_with(".mjs")
                && source_hash(&source.source) == Some(&source.sha256)
                && !source.sites.is_empty()
                && source.sites.iter().all(valid_range)
                && source
                    .sites
                    .iter()
                    .map(|site| site.start_byte)
                    .collect::<BTreeSet<_>>()
                    .len()
                    == source.sites.len()
        })
        && loaded.iter().all(|source| {
            manifest
                .sources
                .iter()
                .any(|entry| &entry.source == *source)
        })
        && evidence.attempts.iter().all(|attempt| {
            loaded.contains(&attempt.source) && valid_site(&attempt.source, attempt.start_byte)
        });
    if !valid {
        run.provenance.incomplete.push(issue(
            "MODULE_OBSERVER_INCOMPLETE",
            "Dynamic dependency trace/manifest is incomplete or mismatched",
        ));
        return Ok(());
    }
    let unexecuted = manifest
        .sources
        .iter()
        .filter(|source| loaded.contains(&source.source))
        .flat_map(|source| {
            source
                .sites
                .iter()
                .filter(|site| {
                    !evidence.attempts.iter().any(|attempt| {
                        attempt.source == source.source && attempt.start_byte == site.start_byte
                    })
                })
                .map(|site| format!("{}:{}", source.source, site.start_byte))
        })
        .collect::<BTreeSet<_>>();
    run.provenance.incomplete.retain(|issue| {
        issue.code != "DYNAMIC_DEPENDENCY_UNRESOLVED" || !unexecuted.contains(&issue.message)
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_esm_sites_require_matching_complete_evidence_and_actual_loading() {
        use dctx0_target_sdk::*;
        let hash = "a".repeat(64);
        let manifest = ModuleObserverManifest {
            schema_version: 1,
            observer_version: "dctx0-esm-observer-1".into(),
            project_root_relative: "./".into(),
            sources: vec![ModuleObserverSource {
                source: "library.mjs".into(),
                sha256: hash.clone(),
                sites: vec![ModuleObserverSite {
                    start_byte: 10,
                    end_byte: 30,
                    argument_start: 20,
                    argument_end: 25,
                }],
            }],
        };
        let base: VerificationRun = serde_json::from_value(serde_json::json!({"schema_version":1,"origin":"native","started_at_unix_ns":"1","elapsed_us":0,"provenance":{"snapshot":"s","check_occurrence":1,"plan_hash":"p","implementation":{"library.mjs":hash},"dependencies":{},"adapter_hash":"a","configuration_hash":"c","harness_hash":"h","interpreter_version":"i","generator_version":"g","protocol_version":1,"target":"typescript","runtime_version":"bun","build_configuration":{},"task_pin":null,"incomplete":[{"code":"DYNAMIC_DEPENDENCY_UNRESOLVED","message":"library.mjs:10","span":null},{"code":"OTHER_REQUIRED","message":"retain","span":null}]},"budgets":VerificationBudgets::default(),"selected_subjects":[],"required_subjects":[],"not_selected":[],"required_cases":[],"cases":[],"open_obligations":[],"bounded_obligations":[],"artifacts":[],"result":{"status":"PASS"}})).unwrap();
        let evidence = ModuleObserverEvidence {
            schema_version: 1,
            observer_version: manifest.observer_version.clone(),
            loaded_sources: vec!["library.mjs".into()],
            attempts: vec![],
            complete: true,
        };
        let artifact = |manifest: &ModuleObserverManifest| {
            VerificationArtifact::new(
                "application/vnd.dctx0.module-observer-manifest+json",
                serde_json::to_vec(manifest).unwrap(),
            )
        };
        let mut run = base.clone();
        reconcile_observer(&mut run, &mut vec![artifact(&manifest)], &evidence).unwrap();
        assert_eq!(run.provenance.incomplete.len(), 1);
        assert_eq!(run.provenance.incomplete[0].code, "OTHER_REQUIRED");
        for mutation in 0..9 {
            let mut changed = evidence.clone();
            let mut configured = manifest.clone();
            match mutation {
                0 => changed.complete = false,
                1 => changed.loaded_sources.clear(),
                2 => changed.loaded_sources.push("library.mjs".into()),
                3 => configured.sources[0].sha256 = "wrong".into(),
                4 => changed.attempts.push(ModuleLoadAttempt {
                    source: "library.mjs".into(),
                    start_byte: 10,
                    specifier: "./loaded.mjs".into(),
                    resolved: Some("file:///loaded.mjs".into()),
                }),
                5 => configured.sources[0].sites[0].argument_end = 99,
                6 => changed.loaded_sources = vec!["unknown.mjs".into()],
                7 => changed.observer_version = "other".into(),
                _ => changed.attempts.push(ModuleLoadAttempt {
                    source: "library.mjs".into(),
                    start_byte: 99,
                    specifier: "unknown".into(),
                    resolved: None,
                }),
            }
            let mut run = base.clone();
            reconcile_observer(&mut run, &mut vec![artifact(&configured)], &changed).unwrap();
            assert!(
                run.provenance
                    .incomplete
                    .iter()
                    .any(|issue| issue.code == "DYNAMIC_DEPENDENCY_UNRESOLVED"),
                "Mutation {mutation}"
            );
        }
        let mut run = base;
        reconcile_observer(&mut run, &mut vec![], &evidence).unwrap();
        assert!(
            run.provenance
                .incomplete
                .iter()
                .any(|issue| issue.code == "MODULE_OBSERVER_MANIFEST_REQUIRED")
        );
    }
    #[test]
    fn relative_dependencies_cannot_escape_the_capture_root() {
        assert_eq!(
            relative("src/app.ts", "./helper.ts"),
            Some("src/helper.ts".into())
        );
        assert_eq!(
            relative("src/app.ts", "../shared.ts"),
            Some("shared.ts".into())
        );
        assert_eq!(relative("src/app.ts", "../../outside.ts"), None);
        assert_eq!(relative("app.ts", "../outside.ts"), None);
    }
    #[test]
    fn captured_dependency_analysis_retains_all_unresolved_inputs_and_its_artifact() {
        let root =
            std::env::temp_dir().join(format!("dctx0-dependency-inputs-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("app.ts"), "import './helper.ts'; import './missing.ts'; import '../outside.ts'; import 'package'; import(name); import 'node:path';").unwrap();
        fs::write(root.join("helper.ts"), "export const value = 1;").unwrap();
        fs::write(root.join("view.tsx"), "export const view = <div/>;").unwrap();
        fs::write(root.join("data.json"), "{}").unwrap();
        let capture = Capture::new(&root).unwrap();
        let mut artifacts = vec![];
        let issues = inspect(
            &capture,
            &dctx0_target_typescript::BunHarness,
            &BTreeSet::new(),
            &mut artifacts,
        )
        .unwrap();
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "DEPENDENCY_NOT_CAPTURED",
                "DEPENDENCY_OUTSIDE_CAPTURE",
                "PACKAGE_DEPENDENCY_UNASSESSED",
                "DYNAMIC_DEPENDENCY_UNRESOLVED"
            ]
        );
        assert_eq!(artifacts.len(), 1);
        assert_eq!(
            artifacts[0].reference.media_type,
            "application/vnd.dctx0.dependencies+json"
        );
        let report: dctx0_target_sdk::DependencyInspection =
            serde_json::from_slice(&artifacts[0].contents).unwrap();
        assert_eq!(report.references.len(), 6);
        drop(capture);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn generated_artifacts_are_available_only_in_the_testing_boundary() {
        let root = std::env::temp_dir().join(format!(
            "dctx0-generated-dependencies-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir_all(root.join(".dctx0/testing/typescript")).unwrap();
        fs::write(
            root.join("app.ts"),
            "import './.dctx0/testing/typescript/contract.ts';",
        )
        .unwrap();
        fs::write(
            root.join(".dctx0/testing/typescript/adapter.ts"),
            "import './contract.ts';",
        )
        .unwrap();
        let capture = Capture::new(&root).unwrap();
        let generated = BTreeSet::from([".dctx0/testing/typescript/contract.ts".into()]);
        let mut artifacts = vec![];
        let issues = inspect(
            &capture,
            &dctx0_target_typescript::BunHarness,
            &generated,
            &mut artifacts,
        )
        .unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "PRODUCTION_TESTING_DEPENDENCY");
        drop(capture);
        fs::remove_dir_all(root).unwrap();
    }
    struct BrokenReport(dctx0_target_sdk::DependencyInspection);
    impl HarnessGenerator for BrokenReport {
        fn generate(
            &self,
            _: &HarnessRequest,
        ) -> std::result::Result<dctx0_target_sdk::HarnessBundle, dctx0_target_sdk::Error> {
            Err(dctx0_target_sdk::Error::Invalid(
                "No worker in report validation test".into(),
            ))
        }
        fn scaffold(&self) -> dctx0_target_sdk::Artifact {
            dctx0_target_sdk::Artifact {
                path: "adapter.ts".into(),
                contents: String::new(),
            }
        }
        fn inspect_dependencies(
            &self,
            _: &[InspectionSource],
        ) -> std::result::Result<dctx0_target_sdk::DependencyInspection, dctx0_target_sdk::Error>
        {
            Ok(self.0.clone())
        }
    }
    #[test]
    fn reports_cannot_omit_sources_or_invent_source_ranges() {
        let root = std::env::temp_dir().join(format!(
            "dctx0-invalid-dependency-report-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let source = "export const name = 'é';";
        fs::write(root.join("app.ts"), source).unwrap();
        let unicode = source.find('é').unwrap();
        let capture = Capture::new(&root).unwrap();
        let valid = dctx0_target_sdk::DependencyInspection {
            schema_version: 1,
            analyzer_version: "test".into(),
            sources: vec!["app.ts".into()],
            references: vec![],
            syntax_errors: vec![],
        };
        for reason in [
            "version",
            "analyzer",
            "omitted",
            "duplicate",
            "syntax",
            "range",
            "unknown-source",
            "empty-range",
            "unicode-start",
            "unicode-end",
            "empty-specifier",
        ] {
            let mut report = valid.clone();
            match reason {
                "version" => report.schema_version = 99,
                "analyzer" => report.analyzer_version.clear(),
                "omitted" => report.sources.clear(),
                "duplicate" => report.sources.push("app.ts".into()),
                "syntax" => report.syntax_errors.push("not-captured.ts".into()),
                "range" => report
                    .references
                    .push(dctx0_target_sdk::DependencyReference {
                        source: "app.ts".into(),
                        kind: dctx0_target_sdk::DependencyKind::Import,
                        specifier: Some("./value.ts".into()),
                        start_byte: 0,
                        end_byte: 999,
                    }),
                "unknown-source" | "empty-range" | "unicode-start" | "unicode-end"
                | "empty-specifier" => {
                    let mut reference = dctx0_target_sdk::DependencyReference {
                        source: "app.ts".into(),
                        kind: dctx0_target_sdk::DependencyKind::Import,
                        specifier: Some("./value.ts".into()),
                        start_byte: 0,
                        end_byte: 1,
                    };
                    match reason {
                        "unknown-source" => reference.source = "missing.ts".into(),
                        "empty-range" => reference.end_byte = 0,
                        "unicode-start" => {
                            reference.start_byte = unicode + 1;
                            reference.end_byte = unicode + 2;
                        }
                        "unicode-end" => {
                            reference.start_byte = unicode;
                            reference.end_byte = unicode + 1;
                        }
                        _ => reference.specifier = Some(String::new()),
                    }
                    report.references.push(reference);
                }
                _ => unreachable!(),
            }
            let mut artifacts = vec![];
            let issues = inspect(
                &capture,
                &BrokenReport(report),
                &BTreeSet::new(),
                &mut artifacts,
            )
            .unwrap();
            assert_eq!(issues[0].code, "DEPENDENCY_REPORT_INVALID", "{reason}");
            assert_eq!(artifacts.len(), 1);
        }
        drop(capture);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod native_resolution_tests {
    use super::*;
    use dctx0_target_sdk::{
        AdapterDescription, DependencyInspection, DependencyKind, DependencyReference,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "dctx0-resolver-boundary-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        async fn worker(&self) -> NativeWorker {
            let code = r#"import json,sys,pathlib
root=pathlib.Path.cwd()
for line in sys.stdin:
 p=json.loads(line);r=p['request'];kind=r['kind']
 if kind=='shutdown': reply={'kind':'bye'}
 else:
  (root/'attempted-resolution').write_text(kind)
  spec=r['specifier']
  if spec=='mock':
   print(json.dumps({'protocol_version':1,'message':{'kind':'mock-call','request_id':17,'invocation':{'subject':'W.Unconfigured.read','arguments':[]}}}),flush=True)
   answer=json.loads(next(sys.stdin))['request']
   (root/'mock-response').write_text(json.dumps(answer))
   reply={'kind':'dependency-unresolved','reason':'collaborator cannot resolve dependencies'}
  elif spec=='unresolved':reply={'kind':'dependency-unresolved','reason':'not installed'}
  elif spec=='wrong':reply={'kind':'value-checked','outcome':{'kind':'accepted','value':{'kind':'unit'}}}
  elif spec=='outside':reply={'kind':'dependency-resolved','path':'/outside/not-captured.js'}
  elif spec=='spoof-builtin':reply={'kind':'dependency-resolved','path':'node:fs'}
  else:reply={'kind':'dependency-resolved','path':str(root/spec)}
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':reply}}),flush=True)
 if kind=='shutdown':break
"#;
            NativeWorker::spawn(
                &ExecutionRecipe {
                    program: "python3".into(),
                    arguments: vec!["-u".into(), "-c".into(), code.into()],
                },
                &self.0,
                crate::NativeLimits::default(),
            )
            .await
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn run() -> VerificationRun {
        // Transport/provenance fixture only. This supplies no execution authority
        // and is never persisted or passed to automatic task publication.
        serde_json::from_value(serde_json::json!({"schema_version":1,"origin":"native","started_at_unix_ns":"1","elapsed_us":0,
        "provenance":{"snapshot":"s","check_occurrence":1,"plan_hash":"p","implementation":{"app.ts":"hash","adapter.ts":"hash"},"dependencies":{"library.wasm":"hash","value.json":"hash"},"adapter_hash":"a","configuration_hash":"c","harness_hash":"h","interpreter_version":"i","generator_version":"g","protocol_version":1,"target":"typescript","runtime_version":"python transport fixture","build_configuration":{},"task_pin":null,"incomplete":[]},
        "budgets":VerificationBudgets::default(),"selected_subjects":["W.A"],"required_subjects":["W.A"],"not_selected":[],"required_cases":[],"cases":[],"open_obligations":[],"bounded_obligations":[],"artifacts":[],"result":{"status":"PASS"}})).unwrap()
    }
    fn artifacts(specifiers: &[&str]) -> Vec<VerificationArtifact> {
        let report = DependencyInspection {
            schema_version: 1,
            analyzer_version: "transport-test".into(),
            sources: vec!["app.ts".into(), "adapter.ts".into()],
            syntax_errors: vec![],
            references: specifiers
                .iter()
                .map(|specifier| DependencyReference {
                    source: "app.ts".into(),
                    kind: DependencyKind::Import,
                    specifier: Some((*specifier).into()),
                    start_byte: 0,
                    end_byte: 1,
                })
                .collect(),
        };
        let description = AdapterDescription {
            protocol_version: 1,
            subjects: vec![],
            artifacts: vec!["app.ts".into()],
            capabilities: vec![],
            representations: BTreeMap::new(),
        };
        vec![
            VerificationArtifact::new(
                "application/vnd.dctx0.dependencies+json",
                serde_json::to_vec(&report).unwrap(),
            ),
            VerificationArtifact::new(
                "application/vnd.dctx0.adapter-description+json",
                serde_json::to_vec(&description).unwrap(),
            ),
        ]
    }
    async fn finish(worker: &mut NativeWorker, cancel: &NativeCancellation) {
        assert_eq!(
            worker
                .request(WorkerRequest::Shutdown, cancel, &mut |_| Ok(
                    Observation::Unavailable {
                        reason: "No collaborator".into()
                    }
                ))
                .await
                .unwrap(),
            WorkerReply::Bye
        );
        assert_eq!(worker.finish().await.unwrap().exit_code, Some(0));
    }
    #[tokio::test]
    async fn dependency_scope_guards_execute_no_resolution_without_valid_captured_roots() {
        for reason in [
            "invalid-report",
            "missing-report",
            "missing-description",
            "uncaptured-root",
        ] {
            let fixture = Fixture::new();
            let mut worker = fixture.worker().await;
            let mut run = run();
            let mut artifacts = artifacts(&["value.json"]);
            match reason {
                "invalid-report" => run.provenance.incomplete.push(issue(
                    "DEPENDENCY_REPORT_INVALID",
                    "incomplete source report",
                )),
                "missing-report" => {
                    artifacts.remove(0);
                }
                "missing-description" => {
                    artifacts.pop();
                }
                _ => {
                    run.provenance.implementation.remove("app.ts");
                }
            }
            let cancel = NativeCancellation::default();
            resolve_native(
                &mut worker,
                &fixture.0,
                "adapter.ts",
                &BTreeSet::new(),
                &mut run,
                &mut artifacts,
                &cancel,
            )
            .await
            .unwrap();
            assert!(!fixture.0.join("attempted-resolution").exists(), "{reason}");
            assert!(
                !artifacts
                    .iter()
                    .any(|artifact| artifact.reference.media_type
                        == "application/vnd.dctx0.resolved-dependencies+json")
            );
            if reason == "uncaptured-root" {
                assert_eq!(
                    run.provenance.incomplete[0].code,
                    "DEPENDENCY_ROOT_NOT_CAPTURED"
                );
            }
            finish(&mut worker, &cancel).await;
        }
    }
    #[tokio::test]
    async fn resolver_replies_cannot_invent_captured_sources_or_enter_the_testing_boundary() {
        for (specifier, expected) in [
            ("outside", "RESOLVED_DEPENDENCY_NOT_CAPTURED"),
            ("spoof-builtin", "RESOLVED_DEPENDENCY_NOT_CAPTURED"),
            ("library.wasm", "DEPENDENCY_SOURCE_UNASSESSED"),
            (
                ".dctx0/testing/contract.ts",
                "PRODUCTION_TESTING_DEPENDENCY",
            ),
            ("unresolved", "NATIVE_DEPENDENCY_UNRESOLVED"),
            ("wrong", "NATIVE_DEPENDENCY_RESOLUTION_UNAVAILABLE"),
            ("mock", "NATIVE_DEPENDENCY_UNRESOLVED"),
        ] {
            let fixture = Fixture::new();
            let mut worker = fixture.worker().await;
            let mut run = run();
            let mut artifacts = artifacts(&[specifier]);
            let generated = [".dctx0/testing/contract.ts".into()].into();
            let cancel = NativeCancellation::default();
            resolve_native(
                &mut worker,
                &fixture.0,
                "adapter.ts",
                &generated,
                &mut run,
                &mut artifacts,
                &cancel,
            )
            .await
            .unwrap();
            assert!(
                run.provenance
                    .incomplete
                    .iter()
                    .any(|issue| issue.code == expected),
                "{specifier}: {:?}",
                run.provenance.incomplete
            );
            let resolution: Vec<ResolvedDependency> =
                serde_json::from_slice(&artifacts.last().unwrap().contents).unwrap();
            assert_eq!(resolution.len(), 1);
            if specifier == "mock" {
                let reply: serde_json::Value =
                    serde_json::from_slice(&fs::read(fixture.0.join("mock-response")).unwrap())
                        .unwrap();
                assert_eq!(reply["observation"]["kind"], "unavailable");
                assert!(
                    reply["observation"]["reason"]
                        .as_str()
                        .unwrap()
                        .contains("cannot invoke collaborators")
                );
            }
            finish(&mut worker, &cancel).await;
        }
    }
    #[tokio::test]
    async fn syntax_failures_remain_open_without_inventing_native_resolution_attempts() {
        let fixture = Fixture::new();
        let mut worker = fixture.worker().await;
        let mut run = run();
        let mut artifacts = artifacts(&[]);
        let mut report: DependencyInspection =
            serde_json::from_slice(&artifacts[0].contents).unwrap();
        report.syntax_errors = vec!["app.ts".into()];
        artifacts[0] = VerificationArtifact::new(
            "application/vnd.dctx0.dependencies+json",
            serde_json::to_vec(&report).unwrap(),
        );
        let cancel = NativeCancellation::default();
        resolve_native(
            &mut worker,
            &fixture.0,
            "adapter.ts",
            &BTreeSet::new(),
            &mut run,
            &mut artifacts,
            &cancel,
        )
        .await
        .unwrap();
        assert!(
            run.provenance
                .incomplete
                .iter()
                .any(|issue| issue.code == "DEPENDENCY_SYNTAX_UNRESOLVED"
                    && issue.message == "app.ts")
        );
        let resolved: Vec<ResolvedDependency> =
            serde_json::from_slice(&artifacts.last().unwrap().contents).unwrap();
        assert!(resolved.is_empty());
        assert!(!fixture.0.join("attempted-resolution").exists());
        finish(&mut worker, &cancel).await;
    }

    #[tokio::test]
    async fn native_dependency_budget_retains_its_executed_prefix_and_reports_incompleteness() {
        let fixture = Fixture::new();
        let mut worker = fixture.worker().await;
        let mut run = run();
        let specifiers = vec!["value.json"; 1001];
        let mut artifacts = artifacts(&specifiers);
        let cancel = NativeCancellation::default();
        resolve_native(
            &mut worker,
            &fixture.0,
            "adapter.ts",
            &BTreeSet::new(),
            &mut run,
            &mut artifacts,
            &cancel,
        )
        .await
        .unwrap();
        assert!(
            run.provenance
                .incomplete
                .iter()
                .any(|issue| issue.code == "DEPENDENCY_RESOLUTION_BUDGET")
        );
        let prefix: Vec<ResolvedDependency> =
            serde_json::from_slice(&artifacts.last().unwrap().contents).unwrap();
        assert_eq!(prefix.len(), 1000);
        assert!(
            prefix
                .iter()
                .all(|entry| entry.captured && entry.path.as_deref() == Some("value.json"))
        );
        finish(&mut worker, &cancel).await;
    }
}
