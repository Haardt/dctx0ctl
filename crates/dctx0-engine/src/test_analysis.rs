//! Opt-in isolated libtest research orchestration. Never publishes task evidence.
use crate::{AccessMode, Project};
use dctx0_store::AnalysisRecord;
use dctx0_test_analysis::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::experimental_coverage::error_text;

fn adapter(root: &Path, mut options: Value) -> Result<Value, String> {
    options["root"] = json!(root);
    let collector = include_str!("coverage_collect.py")
        .split("if __name__ == '__main__':")
        .next()
        .ok_or("Collector missing")?;
    let script = format!(
        "{collector}\n{}\n{}\n{}",
        include_str!("coverage_diff.py"),
        include_str!("analysis_adapter.py"),
        include_str!("analysis_main.py")
    );
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(options.to_string())
        .output()
        .map_err(error_text)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    serde_json::from_slice(&output.stdout).map_err(error_text)
}
fn implementation_fingerprint() -> String {
    format!(
        "{}:{:x}",
        IMPLEMENTATION_VERSION,
        Sha256::digest(
            concat!(
                include_str!("test_analysis.rs"),
                include_str!("coverage_collect.py"),
                include_str!("analysis_adapter.py"),
                include_str!("coverage_diff.py"),
                include_str!("experimental_coverage.rs"),
                include_str!("analysis_main.py"),
                include_str!("../../dctx0-store/src/test_analysis.rs"),
                include_str!("../../../Cargo.lock"),
                include_str!("../../dctx0-test-analysis/src/lib.rs")
            )
            .as_bytes()
        )
    )
}
fn nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
fn save(
    project: &Project,
    records: &[AnalysisRecord],
    owner: &str,
    kind: &str,
    diagnostic: Option<String>,
) -> Result<(), String> {
    let mut store = project.store(AccessMode::ReadWrite).map_err(error_text)?;
    let sequence = store
        .analysis_events(&project.key(), owner)
        .map_err(error_text)?
        .len() as u64
        + 1;
    store
        .publish_analysis(
            &project.key(),
            records,
            owner,
            &ResearchEvent {
                sequence,
                at: now(),
                kind: kind.into(),
                subject: owner.into(),
                diagnostic,
            },
        )
        .map_err(|error| format!("Research write for {owner} failed: {}", error_text(error)))
}
pub fn research_records(project: &Project) -> Result<Vec<AnalysisRecord>, String> {
    if !project.database.exists() {
        return Ok(vec![]);
    }
    project
        .store(AccessMode::ReadOnly)
        .map_err(error_text)?
        .analysis_records(&project.key())
        .map_err(error_text)
}
fn snapshot(project: &Project, root: &Path) -> Result<SourceSnapshot, String> {
    let mut snapshot: SourceSnapshot = serde_json::from_value(adapter(
        root,
        json!({"action":"snapshot", "repository":project.key(), "project_root":project.root,"database":project.database}),
    )?)
    .map_err(error_text)?;
    snapshot.snapshot_hash = content_id(&snapshot).map_err(error_text)?;
    Ok(snapshot)
}
fn artifact(project: &Project, path: &Path) -> Result<Artifact, String> {
    let bytes = std::fs::read(path).map_err(error_text)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let directory = project.root.join(".dctx0ctl/analysis/artifacts");
    std::fs::create_dir_all(&directory).map_err(error_text)?;
    let destination = directory.join(&hash);
    if destination.exists() && std::fs::read(&destination).map_err(error_text)? != bytes {
        return Err("Existing content-addressed artifact is corrupt".into());
    }
    if !destination.exists() {
        let temporary = directory.join(nonce());
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(error_text)?;
        file.write_all(&bytes).map_err(error_text)?;
        file.sync_all().map_err(error_text)?;
        std::fs::rename(temporary, destination).map_err(error_text)?;
        std::fs::File::open(&directory)
            .and_then(|directory| directory.sync_all())
            .map_err(error_text)?;
    }
    Ok(Artifact {
        sha256: hash,
        bytes: bytes.len() as u64,
        media_type: "application/octet-stream".into(),
    })
}
#[derive(serde::Serialize)]
pub struct ResearchCollection {
    pub collection_id: String,
    pub catalog_id: String,
    pub completeness: Completeness,
    pub all_passed: bool,
    pub experimental: bool,
    pub execution_ids: Vec<i64>,
    pub collector_children_peak_rss_bytes: Option<u64>,
    pub measurement_scope: String,
}
/// Catalog-only builds and enumerates; it never invokes a test body.
pub fn research_collect(
    project: &Project,
    scope: RunScope,
    timeout_secs: u64,
    catalog_only: bool,
) -> Result<Value, String> {
    let _lock = project.lock().map_err(error_text)?;
    collect_at(project, &project.root, scope, timeout_secs, catalog_only).and_then(
        |(catalog, collection, observations)| {
            if catalog_only {
                return Ok(json!(catalog));
            }
            Ok(json!(ResearchCollection {
                experimental: true,
                execution_ids: collection.legacy_execution_ids.clone(),
                collector_children_peak_rss_bytes: collection.collector_peak_rss_bytes,
                measurement_scope:
                    "Linux cumulative maximum child RSS, includes build; not per-test RSS".into(),
                collection_id: collection.id.clone(),
                catalog_id: catalog.id.clone(),
                completeness: completeness(&catalog, &collection, &observations)
                    .map_err(error_text)?,
                all_passed: !catalog.tests.is_empty()
                    && observations
                        .iter()
                        .all(|o| matches!(o.outcome, TestOutcome::Pass | TestOutcome::Ignored))
            }))
        },
    )
}
fn collect_at(
    project: &Project,
    root: &Path,
    scope: RunScope,
    timeout_secs: u64,
    catalog_only: bool,
) -> Result<(TestCatalog, CollectionRun, Vec<TestObservation>), String> {
    collect_step(project, root, scope, timeout_secs, catalog_only, None)
}
fn catalog_from_report(
    snapshot: &SourceSnapshot,
    mut scope: RunScope,
    report: &Value,
) -> Result<TestCatalog, String> {
    scope.profile = "test".into();
    scope.toolchain = report["provenance"]["rustc"]
        .as_str()
        .ok_or("Missing toolchain")?
        .into();
    let mut tests = Vec::new();
    let mut targets = BTreeMap::new();
    for test in report["catalog"].as_array().ok_or("Missing catalog")? {
        let get = |name: &str| test[name].as_str().ok_or_else(|| format!("Missing {name}"));
        let key = LogicalTestKey {
            project: content_id(&(&snapshot.repository, &snapshot.project_prefix))
                .map_err(error_text)?,
            package: get("package")?.into(),
            target_kind: get("target_kind")?.into(),
            target_name: get("target_name")?.into(),
            test_name: get("test_name")?.into(),
        };
        targets.insert(
            (
                key.package.clone(),
                key.target_kind.clone(),
                key.target_name.clone(),
            ),
            TargetSupport {
                package: key.package.clone(),
                kind: key.target_kind.clone(),
                name: key.target_name.clone(),
                supported: true,
                reason: None,
            },
        );
        tests.push(CatalogTest { version: TestVersion { key, snapshot: snapshot.snapshot_hash.clone(), definition_hash: None, definition_hash_provenance:"individual definition not resolved; executable hash retained only in raw provenance".into() }, ignored:test["ignored"].as_bool().unwrap_or(false) });
    }
    tests.sort_by(|a, b| a.version.key.cmp(&b.version.key));
    let mut targets: Vec<_> = targets.into_values().collect();
    targets.extend(
        serde_json::from_value::<Vec<TargetSupport>>(report["supported_targets"].clone())
            .map_err(error_text)?,
    );
    targets.sort_by(|a, b| (&a.package, &a.kind, &a.name).cmp(&(&b.package, &b.kind, &b.name)));
    targets.dedup();
    targets.extend(
        serde_json::from_value::<Vec<TargetSupport>>(report["unsupported"].clone())
            .map_err(error_text)?,
    );
    targets.push(TargetSupport {
        package: "scope".into(),
        kind: "doctest".into(),
        name: "doctests".into(),
        supported: false,
        reason: Some("doctests outside isolated libtest scope".into()),
    });
    let mut catalog = TestCatalog {
        id: String::new(),
        snapshot: snapshot.snapshot_hash.clone(),
        scope,
        tests,
        targets,
        enumeration_complete: true,
        diagnostics: vec![],
    };
    catalog.id = content_id(&catalog).map_err(error_text)?;
    Ok(catalog)
}
fn collect_step(
    project: &Project,
    root: &Path,
    scope: RunScope,
    timeout_secs: u64,
    catalog_only: bool,
    step: Option<&str>,
) -> Result<(TestCatalog, CollectionRun, Vec<TestObservation>), String> {
    if timeout_secs == 0 {
        return Err("Timeout must be positive".into());
    }
    let snapshot = snapshot(project, root)?;
    let id = match step {
        Some(step) => content_id(&step).map_err(error_text)?,
        None => nonce(),
    };
    if step.is_some() {
        let records = research_records(project)?;
        if let Some(collection) = records.iter().find_map(|r| match r {
            AnalysisRecord::Collection(c) if c.id == id => Some(c.clone()),
            _ => None,
        }) {
            let catalog = records
                .iter()
                .find_map(|r| match r {
                    AnalysisRecord::Catalog(c) if c.id == collection.catalog => Some(c.clone()),
                    _ => None,
                })
                .ok_or("Resume catalog missing")?;
            if snapshot.snapshot_hash != catalog.snapshot {
                return Err("Resume snapshot changed".into());
            }
            let observations = records
                .iter()
                .filter_map(|r| match r {
                    AnalysisRecord::Observation(o) if o.collection == id => Some(o.clone()),
                    _ => None,
                })
                .collect();
            return Ok((catalog, collection, observations));
        }
    }
    let run_artifacts = project.root.join(".dctx0ctl/analysis/runs").join(&id);
    let started_at = now();
    save(project, &[], &id, "collection_started", None)?;
    let mut options = json!({"action":"collect", "artifacts":run_artifacts, "package":scope.package, "filter":scope.filter, "all_features":scope.all_features, "no_default_features":scope.no_default_features, "features":scope.features, "timeout_secs":timeout_secs, "catalog_only":catalog_only,"resume":step.is_some()});
    let mut frozen = None;
    let report_result = (|| -> Result<Value, String> {
        if !catalog_only {
            options["catalog_only"] = json!(true);
            let enumerated = adapter(root, options.clone())?;
            if snapshot != self::snapshot(project, root)? {
                return Err("Source changed during enumeration".into());
            }
            let catalog = catalog_from_report(&snapshot, scope.clone(), &enumerated)?;
            save(
                project,
                &[
                    AnalysisRecord::Snapshot(snapshot.clone()),
                    AnalysisRecord::Catalog(catalog.clone()),
                ],
                &id,
                "catalog_published",
                Some(catalog.id.clone()),
            )?;
            options["catalog_only"] = json!(false);
            options["resume"] = json!(true);
            let frozen_path = run_artifacts.join("frozen-catalog.json");
            let frozen_bytes=serde_json::to_vec(&json!({"catalog":enumerated["catalog"],"toolchain":enumerated["provenance"]["rustc"]})).map_err(error_text)?;
            if frozen_path.exists() {
                if std::fs::read(&frozen_path).map_err(error_text)? != frozen_bytes {
                    return Err("Frozen collection inputs changed during resume".into());
                }
            } else {
                let partial = run_artifacts.join(format!("frozen-{}.partial", nonce()));
                std::fs::write(&partial, &frozen_bytes).map_err(error_text)?;
                std::fs::rename(partial, &frozen_path).map_err(error_text)?;
            }
            options["expected_catalog_path"] = json!(frozen_path);
            options["expected_toolchain"] = enumerated["provenance"]["rustc"].clone();
            options["catalog_build_ms"] = enumerated["provenance"]["build_ms"].clone();
            options["catalog_build_peak_rss_bytes"] =
                enumerated["provenance"]["build_peak_rss_bytes"].clone();
            frozen = Some(catalog);
        }
        adapter(root, options)
    })();
    let report = match report_result {
        Ok(report) => report,
        Err(error) => {
            save(project, &[], &id, "collection_blocked", Some(error.clone()))?;
            return Err(format!("COLLECTION_BLOCKED {id}: {error}"));
        }
    };
    if snapshot != self::snapshot(project, root)? {
        return Err("Source changed during research collection".into());
    }
    let catalog = catalog_from_report(&snapshot, scope, &report)?;
    if frozen.as_ref().is_some_and(|c| c != &catalog) {
        return Err("Catalog changed after publication".into());
    }
    let mut collection = CollectionRun {
        id: id.clone(),
        catalog: catalog.id.clone(),
        recipe: serde_json::from_value(report["provenance"]["build_argv"].clone())
            .map_err(error_text)?,
        environment_fingerprint: report["provenance"]["environment_hash"]
            .as_str()
            .unwrap_or_default()
            .into(),
        collector_version: IMPLEMENTATION_VERSION.into(),
        timeout_secs,
        started_at,
        ended_at: Some(now()),
        legacy_execution_ids: vec![],
        collector_peak_rss_bytes: report["collector_children_peak_rss_bytes"].as_u64(),
        build_ms: report["provenance"]["build_ms"]
            .as_f64()
            .unwrap_or_default(),
        build_peak_rss_bytes: report["provenance"]["build_peak_rss_bytes"].as_u64(),
        events: vec![],
    };
    let mut source_artifacts = Vec::new();
    let mut repository_root = root.to_owned();
    for _ in Path::new(&snapshot.project_prefix).components() {
        repository_root.pop();
    }
    for (name, hash) in &snapshot.files {
        let a = artifact(project, &repository_root.join(name))?;
        if &a.sha256 != hash {
            return Err("Source changed before artifact publication".into());
        }
        source_artifacts.push(AnalysisRecord::Artifact(a));
    }
    let mut records = vec![
        AnalysisRecord::Snapshot(snapshot),
        AnalysisRecord::Catalog(catalog.clone()),
    ];
    records.extend(source_artifacts);
    let mut observations = Vec::new();
    if !catalog_only {
        let mut legacy = Vec::new();
        for path in report["execution_files"]
            .as_array()
            .ok_or("Missing execution files")?
        {
            let path = Path::new(path.as_str().ok_or("Invalid execution path")?);
            let raw: Value = serde_json::from_slice(&std::fs::read(path).map_err(error_text)?)
                .map_err(error_text)?;
            legacy.push(
                serde_json::from_value::<dctx0_store::CoverageExecution>(raw.clone())
                    .map_err(error_text)?,
            );
            let p = &raw["provenance"];
            let test = catalog
                .tests
                .iter()
                .find(|t| {
                    p["test_name"] == t.version.key.test_name
                        && p["package_name"] == t.version.key.package
                        && p["test_target_kind"] == t.version.key.target_kind
                        && p["test_target_name"] == t.version.key.target_name
                })
                .ok_or("Uncataloged execution")?;
            let mut artifacts = Vec::new();
            for file in [
                path.to_owned(),
                path.with_file_name("stdout"),
                path.with_file_name("stderr"),
                path.with_file_name("llvm.json"),
                run_artifacts.join("frozen-catalog.json"),
            ] {
                let a = artifact(project, &file)?;
                records.push(AnalysisRecord::Artifact(a.clone()));
                artifacts.push(a);
            }
            let outcome = if raw["outcome"] == "timeout" {
                TestOutcome::Timeout
            } else if p["exit_code"].as_i64().is_some_and(|c| c != 0 && c != 101) {
                TestOutcome::Crash
            } else if raw["outcome"] == "pass" {
                TestOutcome::Pass
            } else {
                TestOutcome::TestFailure
            };
            let coverage_artifact = if p["coverage_available"] == true {
                artifacts.first().cloned()
            } else {
                None
            };
            observations.push(TestObservation {
                id: format!("{id}:{}", observations.len()),
                collection: id.clone(),
                version: test.version.clone(),
                attempt: 1,
                outcome,
                started_at: p["started_at_unix_ms"]
                    .as_u64()
                    .ok_or("Missing execution start time")?
                    .to_string(),
                ended_at: p["ended_at_unix_ms"]
                    .as_u64()
                    .ok_or("Missing execution end time")?
                    .to_string(),
                duration_ms: raw["duration_ms"].as_f64().unwrap_or_default(),
                peak_rss_bytes: raw["peak_rss_bytes"].as_u64(),
                artifacts,
                coverage_artifact,
                coverage_available: p["coverage_available"].as_bool().unwrap_or(false),
            });
        }
        if !legacy.is_empty() {
            collection.legacy_execution_ids = project
                .store(AccessMode::ReadWrite)
                .map_err(error_text)?
                .record_coverage(&project.key(), &legacy)
                .map_err(error_text)?;
        }
        for test in catalog.tests.iter().filter(|t| t.ignored) {
            observations.push(TestObservation {
                id: format!("{id}:{}", observations.len()),
                collection: id.clone(),
                version: test.version.clone(),
                attempt: 1,
                outcome: TestOutcome::Ignored,
                started_at: now(),
                ended_at: now(),
                duration_ms: 0.0,
                peak_rss_bytes: None,
                artifacts: vec![],
                coverage_artifact: None,
                coverage_available: false,
            });
        }
        completeness(&catalog, &collection, &observations).map_err(error_text)?;
        records.push(AnalysisRecord::Collection(collection.clone()));
        records.extend(
            observations
                .iter()
                .cloned()
                .map(AnalysisRecord::Observation),
        );
    }
    save(
        project,
        &records,
        &id,
        if catalog_only {
            "catalog_published"
        } else {
            "collection_published"
        },
        None,
    )?;
    Ok((catalog, collection, observations))
}

fn baseline(
    project: &Project,
    id: &str,
    base: &str,
) -> Result<(TestCatalog, CollectionRun, Vec<TestObservation>), String> {
    let records = research_records(project)?;
    let collection = records
        .iter()
        .find_map(|r| match r {
            AnalysisRecord::Collection(c) if c.id == id => Some(c.clone()),
            _ => None,
        })
        .ok_or("BASELINE_REQUIRED: collection not found")?;
    let catalog = records
        .iter()
        .find_map(|r| match r {
            AnalysisRecord::Catalog(c) if c.id == collection.catalog => Some(c.clone()),
            _ => None,
        })
        .ok_or("BASELINE_REQUIRED: catalog missing")?;
    let snapshot = records
        .iter()
        .find_map(|r| match r {
            AnalysisRecord::Snapshot(s) if s.snapshot_hash == catalog.snapshot => Some(s),
            _ => None,
        })
        .ok_or("BASELINE_REQUIRED: snapshot missing")?;
    if snapshot.commit.as_deref() != Some(base) {
        return Err("BASELINE_REQUIRED: wrong base commit".into());
    }
    let observations: Vec<_> = records
        .iter()
        .filter_map(|r| match r {
            AnalysisRecord::Observation(o) if o.collection == id => Some(o.clone()),
            _ => None,
        })
        .collect();
    if !completeness(&catalog, &collection, &observations)
        .map_err(error_text)?
        .reference_evaluable
    {
        return Err("BASELINE_REQUIRED: incomplete reference".into());
    }
    Ok((catalog, collection, observations))
}
fn resolve(root: &Path, revision: &str) -> Result<String, String> {
    adapter(root, json!({"action":"resolve","revision":revision}))?
        .as_str()
        .map(str::to_owned)
        .ok_or("Invalid revision".into())
}
fn materialized(project: &Project, commit: &str, overlay: bool) -> Result<PathBuf, String> {
    let destination = project
        .root
        .join(".dctx0ctl/analysis/worktrees")
        .join(nonce());
    std::fs::create_dir_all(destination.parent().ok_or("Invalid destination")?)
        .map_err(error_text)?;
    adapter(
        &project.root,
        json!({"action":"materialize","project_root":project.root,"database":project.database,"destination":destination,"commit":commit,"overlay":overlay}),
    )?
    .as_str()
    .map(PathBuf::from)
    .ok_or("Invalid materialization".into())
}
fn intervals(
    project: &Project,
    observations: &[TestObservation],
) -> Result<Vec<CoverageInterval>, String> {
    let mut intervals = Vec::new();
    for observation in observations.iter().filter(|o| o.coverage_available) {
        for a in observation.coverage_artifact.iter() {
            let bytes = std::fs::read(
                project
                    .root
                    .join(".dctx0ctl/analysis/artifacts")
                    .join(&a.sha256),
            )
            .map_err(error_text)?;
            if format!("{:x}", Sha256::digest(&bytes)) != a.sha256 || bytes.len() as u64 != a.bytes
            {
                return Err("Artifact integrity check failed".into());
            }
            let raw: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let Some(root) = raw["provenance"]["repository_root"].as_str() else {
                continue;
            };
            for unit in raw["llvm_export"]["data"].as_array().into_iter().flatten() {
                for file in unit["files"].as_array().into_iter().flatten() {
                    let Some(path) = file["filename"]
                        .as_str()
                        .and_then(|p| Path::new(p).strip_prefix(root).ok())
                    else {
                        continue;
                    };
                    for pair in file["segments"]
                        .as_array()
                        .into_iter()
                        .flat_map(|s| s.windows(2))
                    {
                        if pair[0][3] == true && pair[0][2].as_u64().unwrap_or(0) > 0 {
                            let start = pair[0][0].as_u64().unwrap_or(0);
                            let end = pair[1][0]
                                .as_u64()
                                .unwrap_or(0)
                                .saturating_sub(u64::from(pair[1][1].as_u64() == Some(1)));
                            if start > 0 && end >= start && end <= u32::MAX as u64 {
                                intervals.push(CoverageInterval {
                                    test: observation.version.key.clone(),
                                    file: path.to_string_lossy().into_owned(),
                                    start: start as u32,
                                    end: end as u32,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(intervals)
}
/// Freeze the decision before collecting any target test observations.
pub fn research_shadow(
    project: &Project,
    base: &str,
    baseline_id: &str,
) -> Result<SelectionEvaluation, String> {
    let _lock = project.lock().map_err(error_text)?;
    let base = resolve(&project.root, base)?;
    let (base_catalog, collection, observations) = baseline(project, baseline_id, &base)?;
    let pristine = materialized(project, &base, false)?;
    if snapshot(project, &pristine)?.snapshot_hash != base_catalog.snapshot {
        return Err("BASELINE_REQUIRED: baseline does not match exact base files".into());
    }
    let target = materialized(project, &resolve(&project.root, "HEAD")?, true)?;
    shadow_at(
        project,
        &target,
        &base,
        base_catalog,
        collection,
        observations,
    )
}
fn shadow_at(
    project: &Project,
    root: &Path,
    base: &str,
    base_catalog: TestCatalog,
    collection: CollectionRun,
    observations: Vec<TestObservation>,
) -> Result<SelectionEvaluation, String> {
    shadow_step(
        project,
        root,
        base,
        base_catalog,
        collection,
        observations,
        None,
    )
}
fn prediction_occurrence(
    mut prediction: SelectionPrediction,
    occurrence: Option<&str>,
) -> Result<SelectionPrediction, String> {
    if let Some(occurrence) = occurrence {
        prediction.id = content_id(&(&prediction.id, occurrence)).map_err(error_text)?;
        prediction.occurrence = Some(occurrence.into());
    }
    Ok(prediction)
}
fn shadow_step(
    project: &Project,
    root: &Path,
    base: &str,
    base_catalog: TestCatalog,
    collection: CollectionRun,
    observations: Vec<TestObservation>,
    step: Option<&str>,
) -> Result<SelectionEvaluation, String> {
    let (target, _, _) = collect_at(
        project,
        root,
        base_catalog.scope.clone(),
        collection.timeout_secs,
        true,
    )?;
    let diff = adapter(
        root,
        json!({"action":"diff","base":base,"project_root":project.root,"database":project.database}),
    )?;
    let files: Vec<ChangedFile> =
        serde_json::from_value(diff["files"].clone()).map_err(error_text)?;
    let coverage = intervals(project, &observations)?;
    let started = std::time::Instant::now();
    let mut prediction = predict(PredictionInputs {
        base: &base_catalog,
        target: &target,
        baseline: &collection,
        observations: &observations,
        files: &files,
        coverage: &coverage,
    })
    .map_err(error_text)?;
    let occurrence = step.map(str::to_owned).unwrap_or_else(nonce);
    prediction = prediction_occurrence(prediction, Some(&occurrence))?;
    prediction.selection_ms = started.elapsed().as_secs_f64() * 1000.0;
    let prior = research_records(project)?;
    if let Some(existing) = prior.iter().find_map(|r| match r {
        AnalysisRecord::Prediction(p) if p.id == prediction.id => Some(p.clone()),
        _ => None,
    }) {
        prediction = existing;
        if let Some(evaluation) = prior.iter().find_map(|r| match r {
            AnalysisRecord::Evaluation(e) if e.prediction == prediction.id => Some(e.clone()),
            _ => None,
        }) {
            return Ok(evaluation);
        }
    } else {
        // This transaction completes before the target collector is invoked.
        save(
            project,
            &[AnalysisRecord::Prediction(prediction.clone())],
            &prediction.id,
            "prediction_frozen",
            None,
        )?;
    }
    let (reference_catalog, reference, labels) = collect_step(
        project,
        root,
        target.scope.clone(),
        collection.timeout_secs,
        false,
        step,
    )?;
    if target != reference_catalog {
        return Err("Target catalog changed after prediction publication".into());
    }
    let evaluation = evaluate(
        &prediction,
        &base_catalog,
        &target,
        &reference,
        &labels,
        &observations,
    )
    .map_err(error_text)?;
    save(
        project,
        &[AnalysisRecord::Evaluation(evaluation.clone())],
        &prediction.id,
        "evaluation_published",
        None,
    )?;
    Ok(evaluation)
}
/// Resolve and freeze all pairs before executing builds. Strict chronological
/// chains are deliberately required in v1 to prevent future-label leakage.
pub fn research_replay(
    project: &Project,
    mut manifest: ReplayManifest,
    resume: Option<&str>,
) -> Result<ReplayRun, String> {
    let _lock = project.lock().map_err(error_text)?;
    if manifest.schema_version != 1
        || manifest.strategy != "coverage"
        || manifest.timeout_secs == 0
        || manifest.pairs.is_empty()
    {
        return Err("Unsupported replay manifest".into());
    }
    for pair in &mut manifest.pairs {
        pair.base = resolve(&project.root, &pair.base)?;
        pair.head = resolve(&project.root, &pair.head)?;
        if adapter(
            &project.root,
            json!({"action":"ancestor","base":pair.base,"head":pair.head}),
        )? != true
        {
            return Err("Replay base must be an ancestor of head".into());
        }
    }
    for pairs in manifest.pairs.windows(2) {
        if adapter(
            &project.root,
            json!({"action":"ancestor","base":pairs[0].head,"head":pairs[1].base}),
        )? != true
        {
            return Err("Nonchronological replay rejected".into());
        }
    }
    let environment: BTreeMap<_, _> = std::env::vars_os()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let environment_fingerprint = content_id(&environment).map_err(error_text)?;
    let run = if let Some(id) = resume {
        let run = research_records(project)?
            .into_iter()
            .find_map(|r| match r {
                AnalysisRecord::Replay(r) if r.id == id => Some(r),
                _ => None,
            })
            .ok_or("Replay not found")?;
        if run.manifest != manifest
            || run.implementation_version != implementation_fingerprint()
            || run.environment_fingerprint != environment_fingerprint
        {
            return Err("Resume inputs or implementation changed".into());
        }
        run
    } else {
        let run = ReplayRun {
            id: nonce(),
            manifest,
            implementation_version: implementation_fingerprint(),
            environment_fingerprint,
            events: vec![],
        };
        save(
            project,
            &[AnalysisRecord::Replay(run.clone())],
            &run.id,
            "replay_frozen",
            None,
        )?;
        run
    };
    let mut events = project
        .store(AccessMode::ReadOnly)
        .map_err(error_text)?
        .analysis_events(&project.key(), &run.id)
        .map_err(error_text)?;
    for (index, pair) in run.manifest.pairs.iter().enumerate() {
        let completed = format!("pair_{index}_complete");
        let blocked = format!("pair_{index}_blocked");
        if events.iter().any(|e| e.kind == completed) {
            continue;
        }
        let result = (|| -> Result<String, String> {
            let ready = format!("pair_{index}_base_ready");
            let previous = events
                .iter()
                .find(|e| e.kind == ready)
                .and_then(|e| e.diagnostic.as_deref());
            let reusable = events
                .iter()
                .rev()
                .filter(|e| e.kind.ends_with("_complete") || e.kind.ends_with("_base_ready"))
                .filter_map(|e| e.diagnostic.as_deref())
                .find_map(|id| {
                    let values = baseline(project, id, &pair.base).ok()?;
                    let mut requested = run.manifest.scope.clone();
                    requested.profile = values.0.scope.profile.clone();
                    requested.toolchain = values.0.scope.toolchain.clone();
                    (requested == values.0.scope
                        && values.1.timeout_secs == run.manifest.timeout_secs)
                        .then_some(values)
                });
            let (catalog, collection, observations) = if let Some(id) = previous {
                baseline(project, id, &pair.base)?
            } else if let Some(values) = reusable {
                save(project, &[], &run.id, &ready, Some(values.1.id.clone()))?;
                values
            } else {
                let root = materialized(project, &pair.base, false)?;
                let values = collect_step(
                    project,
                    &root,
                    run.manifest.scope.clone(),
                    run.manifest.timeout_secs,
                    false,
                    Some(&format!("{}:{index}:base", run.id)),
                )?;
                save(project, &[], &run.id, &ready, Some(values.1.id.clone()))?;
                values
            };
            let head = materialized(project, &pair.head, false)?;
            let evaluation = shadow_step(
                project,
                &head,
                &pair.base,
                catalog,
                collection,
                observations,
                Some(&format!("{}:{index}:reference", run.id)),
            )?;
            if !evaluation.completeness.reference_evaluable {
                return Err("Incomplete target reference".into());
            }
            Ok(evaluation.reference_collection)
        })();
        save(
            project,
            &[],
            &run.id,
            if result.is_ok() { &completed } else { &blocked },
            Some(result.unwrap_or_else(|error| error)),
        )?;
        events = project
            .store(AccessMode::ReadOnly)
            .map_err(error_text)?
            .analysis_events(&project.key(), &run.id)
            .map_err(error_text)?;
    }
    let mut report = run;
    report.events = project
        .store(AccessMode::ReadOnly)
        .map_err(error_text)?
        .analysis_events(&project.key(), &report.id)
        .map_err(error_text)?;
    Ok(report)
}
/// NDJSON records keep frozen prediction inputs distinct from later labels.
pub fn research_export(project: &Project, replay_id: &str, output: &Path) -> Result<(), String> {
    let records = research_records(project)?;
    let replay = records
        .iter()
        .find_map(|record| match record {
            AnalysisRecord::Replay(run) if run.id == replay_id => Some(run),
            _ => None,
        })
        .ok_or("Replay not found")?;
    let store = project.store(AccessMode::ReadOnly).map_err(error_text)?;
    let replay_events = store
        .analysis_events(&project.key(), replay_id)
        .map_err(error_text)?;
    let occurrences: BTreeSet<_> = (0..replay.manifest.pairs.len())
        .map(|index| format!("{replay_id}:{index}:reference"))
        .collect();
    let predictions: Vec<_> = records
        .iter()
        .filter_map(|record| match record {
            AnalysisRecord::Prediction(p)
                if p.occurrence
                    .as_ref()
                    .is_some_and(|o| occurrences.contains(o)) =>
            {
                Some(p)
            }
            _ => None,
        })
        .collect();
    let prediction_ids: BTreeSet<_> = predictions.iter().map(|p| p.id.clone()).collect();
    let mut collection_ids = BTreeSet::new();
    for index in 0..replay.manifest.pairs.len() {
        for phase in ["base", "reference"] {
            collection_ids
                .insert(content_id(&format!("{replay_id}:{index}:{phase}")).map_err(error_text)?);
        }
    }
    for p in &predictions {
        collection_ids.insert(p.baseline_collection.clone());
    }
    for record in &records {
        if let AnalysisRecord::Evaluation(e) = record {
            if prediction_ids.contains(&e.prediction) {
                collection_ids.insert(e.reference_collection.clone());
            }
        }
    }
    for event in &replay_events {
        if event.kind.ends_with("_base_ready") {
            if let Some(id) = &event.diagnostic {
                collection_ids.insert(id.clone());
            }
        }
    }
    let available_ids: BTreeSet<_> = predictions
        .iter()
        .flat_map(|p| p.available_observation_ids.iter().cloned())
        .collect();
    for record in &records {
        if let AnalysisRecord::Observation(o) = record {
            if available_ids.contains(&o.id) {
                collection_ids.insert(o.collection.clone());
            }
        }
    }
    let mut catalog_ids: BTreeSet<_> = predictions
        .iter()
        .flat_map(|p| [p.base_catalog.clone(), p.target_catalog.clone()])
        .collect();
    for record in &records {
        if let AnalysisRecord::Collection(c) = record {
            if collection_ids.contains(&c.id) {
                catalog_ids.insert(c.catalog.clone());
            }
        }
    }
    let mut owner_events = BTreeMap::new();
    for owner in collection_ids.iter().chain(prediction_ids.iter()) {
        let events = store
            .analysis_events(&project.key(), owner)
            .map_err(error_text)?;
        for event in &events {
            if event.kind == "catalog_published" {
                if let Some(id) = &event.diagnostic {
                    catalog_ids.insert(id.clone());
                }
            }
        }
        owner_events.insert(owner.clone(), events);
    }
    let mut snapshot_ids: BTreeSet<_> = predictions
        .iter()
        .flat_map(|p| [p.base_snapshot.clone(), p.target_snapshot.clone()])
        .collect();
    let mut artifact_ids = BTreeSet::new();
    for record in &records {
        match record {
            AnalysisRecord::Catalog(c) if catalog_ids.contains(&c.id) => {
                snapshot_ids.insert(c.snapshot.clone());
            }
            AnalysisRecord::Observation(o)
                if collection_ids.contains(&o.collection) || available_ids.contains(&o.id) =>
            {
                snapshot_ids.insert(o.version.snapshot.clone());
                for a in o.artifacts.iter().chain(o.coverage_artifact.iter()) {
                    artifact_ids.insert(a.sha256.clone());
                }
            }
            _ => (),
        }
    }
    for record in &records {
        if let AnalysisRecord::Snapshot(snapshot) = record {
            if snapshot_ids.contains(&snapshot.snapshot_hash) {
                artifact_ids.extend(snapshot.files.values().cloned());
            }
        }
    }
    let temporary = output.with_extension(format!("{}.partial", nonce()));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(error_text)?;
    writeln!(file,"{}",json!({"schema_version":1,"replay_id":replay_id,"scope":"selected replay dependency closure; frozen predictions and later labels are separate tagged records"})).map_err(error_text)?;
    for record in records {
        let selected = match &record {
            AnalysisRecord::Replay(r) => r.id == replay_id,
            AnalysisRecord::Prediction(p) => prediction_ids.contains(&p.id),
            AnalysisRecord::Evaluation(e) => prediction_ids.contains(&e.prediction),
            AnalysisRecord::Collection(c) => collection_ids.contains(&c.id),
            AnalysisRecord::Observation(o) => {
                collection_ids.contains(&o.collection) || available_ids.contains(&o.id)
            }
            AnalysisRecord::Catalog(c) => catalog_ids.contains(&c.id),
            AnalysisRecord::Snapshot(s) => snapshot_ids.contains(&s.snapshot_hash),
            AnalysisRecord::Artifact(a) => artifact_ids.contains(&a.sha256),
        };
        if selected {
            serde_json::to_writer(&mut file, &record).map_err(error_text)?;
            writeln!(file).map_err(error_text)?;
        }
    }
    for event in replay_events {
        writeln!(file, "{}", json!({"kind":"ReplayEvent","data":event})).map_err(error_text)?;
    }
    for (owner, events) in owner_events {
        for event in events {
            writeln!(
                file,
                "{}",
                json!({"kind":"ResearchEvent","owner":owner,"data":event})
            )
            .map_err(error_text)?;
        }
    }
    file.sync_all().map_err(error_text)?;
    std::fs::rename(temporary, output).map_err(error_text)
}
/// Offline recomputation consumes exported inputs, never runs Cargo or reads Git.
pub fn research_recompute(dataset: &Path, stage: &str) -> Result<Vec<Value>, String> {
    let data = std::fs::read_to_string(dataset).map_err(error_text)?;
    let header: Value = serde_json::from_str(data.lines().next().ok_or("Missing dataset header")?)
        .map_err(error_text)?;
    if header["schema_version"] != 1 {
        return Err("Unsupported dataset schema".into());
    }
    let mut records = Vec::new();
    for line in data.lines().skip(1) {
        let value: Value = serde_json::from_str(line).map_err(error_text)?;
        if value["kind"] == "ReplayEvent" || value["kind"] == "ResearchEvent" {
            continue;
        }
        records.push(serde_json::from_value::<AnalysisRecord>(value).map_err(error_text)?);
    }
    let catalog = |id: &str| {
        records
            .iter()
            .find_map(|r| match r {
                AnalysisRecord::Catalog(c) if c.id == id => Some(c),
                _ => None,
            })
            .ok_or_else(|| format!("Missing catalog {id}"))
    };
    let collection = |id: &str| {
        records
            .iter()
            .find_map(|r| match r {
                AnalysisRecord::Collection(c) if c.id == id => Some(c),
                _ => None,
            })
            .ok_or_else(|| format!("Missing collection {id}"))
    };
    let mut result = Vec::new();
    for prediction in records.iter().filter_map(|r| match r {
        AnalysisRecord::Prediction(p) => Some(p),
        _ => None,
    }) {
        let mut baseline = Vec::new();
        for id in &prediction.available_observation_ids {
            baseline.push(
                records
                    .iter()
                    .find_map(|r| match r {
                        AnalysisRecord::Observation(o) if &o.id == id => Some(o.clone()),
                        _ => None,
                    })
                    .ok_or("Missing frozen baseline observation")?,
            );
        }
        if stage == "prediction" {
            let recomputed = predict(PredictionInputs {
                base: catalog(&prediction.base_catalog)?,
                target: catalog(&prediction.target_catalog)?,
                baseline: collection(&prediction.baseline_collection)?,
                observations: &baseline,
                files: &prediction.changed_files,
                coverage: &prediction.coverage_intervals,
            })
            .map_err(error_text)?;
            result.push(json!(prediction_occurrence(
                recomputed,
                prediction.occurrence.as_deref()
            )?));
        } else if stage == "evaluation" {
            for evaluation in records.iter().filter_map(|r| match r {
                AnalysisRecord::Evaluation(e) if e.prediction == prediction.id => Some(e),
                _ => None,
            }) {
                let mut labels: Vec<_> = records
                    .iter()
                    .filter_map(|r| match r {
                        AnalysisRecord::Observation(o)
                            if o.collection == evaluation.reference_collection =>
                        {
                            Some(o.clone())
                        }
                        _ => None,
                    })
                    .collect();
                labels.sort_by(|a, b| a.version.key.cmp(&b.version.key));
                result.push(json!(
                    evaluate(
                        prediction,
                        catalog(&prediction.base_catalog)?,
                        catalog(&prediction.target_catalog)?,
                        collection(&evaluation.reference_collection)?,
                        &labels,
                        &baseline
                    )
                    .map_err(error_text)?
                ));
            }
        } else {
            return Err("Unknown offline stage".into());
        }
    }
    Ok(result)
}
