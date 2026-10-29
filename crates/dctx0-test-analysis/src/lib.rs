//! Pure, versioned inputs and labels for isolated libtest selection research.
//! No result here authorizes skipping tests, task PASS or result reuse.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const IMPLEMENTATION_VERSION: &str = "isolated-coverage-v1";

/// A deterministic content identifier; adapters own publication and timestamps.
pub fn content_id<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub repository: String,
    pub project_prefix: String,
    pub commit: Option<String>,
    pub files: BTreeMap<String, String>,
    pub snapshot_hash: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunScope {
    pub package: Option<String>,
    pub filter: Option<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub features: BTreeSet<String>,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub toolchain: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalTestKey {
    pub project: String,
    pub package: String,
    pub target_kind: String,
    pub target_name: String,
    pub test_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestVersion {
    pub key: LogicalTestKey,
    pub snapshot: String,
    pub definition_hash: Option<String>,
    /// Unknown hashes must explain why. An executable hash is not a definition hash.
    pub definition_hash_provenance: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogTest {
    pub version: TestVersion,
    pub ignored: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSupport {
    pub package: String,
    pub kind: String,
    pub name: String,
    pub supported: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestCatalog {
    pub id: String,
    pub snapshot: String,
    pub scope: RunScope,
    pub tests: Vec<CatalogTest>,
    pub targets: Vec<TargetSupport>,
    pub enumeration_complete: bool,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestOutcome {
    Pass,
    TestFailure,
    Ignored,
    Timeout,
    Crash,
    InfrastructureError,
    NotRun,
    Unsupported,
}
impl TestOutcome {
    pub fn evaluable(self) -> bool {
        matches!(self, Self::Pass | Self::TestFailure)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub sha256: String,
    pub bytes: u64,
    pub media_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObservation {
    pub id: String,
    pub collection: String,
    pub version: TestVersion,
    pub attempt: u32,
    pub outcome: TestOutcome,
    pub started_at: String,
    pub ended_at: String,
    pub duration_ms: f64,
    pub peak_rss_bytes: Option<u64>,
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub coverage_artifact: Option<Artifact>,
    pub coverage_available: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchEvent {
    pub sequence: u64,
    pub at: String,
    pub kind: String,
    pub subject: String,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionRun {
    pub id: String,
    pub catalog: String,
    pub recipe: Vec<String>,
    pub environment_fingerprint: String,
    pub collector_version: String,
    pub timeout_secs: u64,
    pub started_at: String,
    pub ended_at: Option<String>,
    #[serde(default)]
    pub legacy_execution_ids: Vec<i64>,
    #[serde(default)]
    pub collector_peak_rss_bytes: Option<u64>,
    pub build_ms: f64,
    pub build_peak_rss_bytes: Option<u64>,
    pub events: Vec<ResearchEvent>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completeness {
    pub catalog_complete: bool,
    pub all_executable_tests_observed: bool,
    pub reference_evaluable: bool,
    pub missing: Vec<LogicalTestKey>,
    pub unknown: Vec<LogicalTestKey>,
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnalysisError {
    #[error("Invalid research input: {0}")]
    Invalid(String),
    #[error("BASELINE_REQUIRED: {0}")]
    BaselineRequired(String),
}

/// Validate the reference without turning infrastructure failures into test labels.
/// Version 1 requires exactly one isolated attempt for every executable test.
pub fn completeness(
    catalog: &TestCatalog,
    collection: &CollectionRun,
    observations: &[TestObservation],
) -> Result<Completeness, AnalysisError> {
    if collection.catalog != catalog.id
        || collection.timeout_secs == 0
        || !collection.build_ms.is_finite()
        || collection.build_ms < 0.0
    {
        return Err(AnalysisError::Invalid(
            "collection/catalog mismatch or timeout".into(),
        ));
    }
    let mut expected = BTreeMap::new();
    for test in &catalog.tests {
        if test.version.snapshot != catalog.snapshot
            || expected.insert(&test.version.key, test).is_some()
        {
            return Err(AnalysisError::Invalid(
                "duplicate test or snapshot mismatch".into(),
            ));
        }
    }
    let mut seen = BTreeMap::new();
    for observation in observations {
        if observation.collection != collection.id
            || observation.attempt != 1
            || !observation.duration_ms.is_finite()
            || observation.duration_ms < 0.0
            || expected.get(&observation.version.key).map(|t| &t.version)
                != Some(&observation.version)
            || seen.insert(&observation.version.key, observation).is_some()
        {
            return Err(AnalysisError::Invalid(
                "invalid or duplicate observation".into(),
            ));
        }
        if expected[&observation.version.key].ignored && observation.outcome != TestOutcome::Ignored
        {
            return Err(AnalysisError::Invalid(
                "ignored test executed implicitly".into(),
            ));
        }
    }
    let mut missing = Vec::new();
    let mut unknown = Vec::new();
    for test in &catalog.tests {
        if test.ignored {
            continue;
        }
        match seen.get(&test.version.key) {
            None => missing.push(test.version.key.clone()),
            Some(o) if o.outcome == TestOutcome::NotRun => missing.push(test.version.key.clone()),
            Some(o) if !o.outcome.evaluable() => unknown.push(test.version.key.clone()),
            _ => (),
        }
    }
    let catalog_complete = catalog.enumeration_complete;
    Ok(Completeness {
        catalog_complete,
        all_executable_tests_observed: catalog_complete && missing.is_empty(),
        reference_evaluable: catalog_complete
            && missing.is_empty()
            && unknown.is_empty()
            && collection.ended_at.is_some(),
        missing,
        unknown,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedRange {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
}
/// Old-coordinate insertion anchors shared with the legacy observational query.
pub fn old_interval(range: &ChangedRange) -> (u32, u32, bool) {
    let insertion = range.old_count == 0;
    (
        range.old_start.max(1),
        if insertion {
            range.old_start.saturating_add(1).max(1)
        } else {
            range
                .old_start
                .saturating_add(range.old_count)
                .saturating_sub(1)
        },
        insertion,
    )
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
    pub binary: bool,
    pub ranges: Vec<ChangedRange>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoverageInterval {
    pub test: LogicalTestKey,
    pub file: String,
    pub start: u32,
    pub end: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionReason {
    CoverageHit { file: String, start: u32, end: u32 },
    NewOrUnmappedTest,
    ConservativeFallback,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedTest {
    pub test: LogicalTestKey,
    pub reasons: Vec<SelectionReason>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionPrediction {
    pub id: String,
    #[serde(default)]
    pub occurrence: Option<String>,
    pub base_snapshot: String,
    pub target_snapshot: String,
    pub baseline_collection: String,
    pub base_catalog: String,
    pub target_catalog: String,
    pub strategy_version: String,
    pub available_observation_ids: Vec<String>,
    pub selected: Vec<SelectedTest>,
    pub unknowns: BTreeSet<String>,
    pub changed_files: Vec<ChangedFile>,
    pub coverage_intervals: Vec<CoverageInterval>,
    pub selection_ms: f64,
    pub can_skip_other_tests: bool,
}

pub struct PredictionInputs<'a> {
    pub base: &'a TestCatalog,
    pub target: &'a TestCatalog,
    pub baseline: &'a CollectionRun,
    pub observations: &'a [TestObservation],
    pub files: &'a [ChangedFile],
    pub coverage: &'a [CoverageInterval],
}
/// Uses only frozen baseline observations, catalogs and old-coordinate changes.
/// Target labels cannot be supplied to this API.
pub fn predict(inputs: PredictionInputs<'_>) -> Result<SelectionPrediction, AnalysisError> {
    let PredictionInputs {
        base,
        target,
        baseline,
        observations,
        files,
        coverage,
    } = inputs;
    if base.scope != target.scope
        || !completeness(base, baseline, observations)?.reference_evaluable
        || baseline.collector_version != IMPLEMENTATION_VERSION
    {
        return Err(AnalysisError::BaselineRequired(
            "complete compatible collection required".into(),
        ));
    }
    let mut canonical_coverage = coverage.to_vec();
    canonical_coverage.sort();
    canonical_coverage.dedup();
    let coverage = canonical_coverage.as_slice();
    let known: BTreeSet<_> = base
        .tests
        .iter()
        .filter(|t| !t.ignored)
        .map(|t| &t.version.key)
        .collect();
    let covered: BTreeSet<_> = observations
        .iter()
        .filter(|o| o.coverage_available)
        .map(|o| &o.version.key)
        .collect();
    if coverage
        .iter()
        .any(|c| !covered.contains(&c.test) || c.start == 0 || c.end < c.start)
    {
        return Err(AnalysisError::Invalid(
            "coverage not tied to available baseline observation".into(),
        ));
    }
    let mut unknowns = BTreeSet::new();
    if known.iter().any(|k| !covered.contains(k)) && !files.is_empty() {
        unknowns.insert("baseline_coverage_unavailable".into());
    }
    for file in files {
        if matches!(file.status.as_str(), "A" | "untracked") {
            unknowns.insert(format!("new_file:{}", file.path));
        }
        if file.binary
            || file.ranges.is_empty()
            || !file.path.ends_with(".rs")
            || file.path == "build.rs"
            || file.path.ends_with("/build.rs")
        {
            unknowns.insert(format!("binary_metadata_or_build_input:{}", file.path));
        }
        for range in &file.ranges {
            let (start, end, insertion) = old_interval(range);
            if insertion {
                unknowns.insert(format!("inserted_lines:{}", file.path));
            }
            if !coverage
                .iter()
                .any(|c| c.file == file.path && c.start <= end && start <= c.end)
            {
                unknowns.insert(format!("unobserved_old_range:{}:{start}", file.path));
            }
        }
    }
    let mut selected = BTreeMap::<_, Vec<_>>::new();
    for test in target.tests.iter().filter(|t| !t.ignored) {
        let mut reasons = Vec::new();
        if !known.contains(&test.version.key) {
            reasons.push(SelectionReason::NewOrUnmappedTest);
        }
        if !unknowns.is_empty() {
            reasons.push(SelectionReason::ConservativeFallback);
        }
        for file in files {
            for range in &file.ranges {
                let (start, end, _) = old_interval(range);
                if coverage.iter().any(|c| {
                    c.test == test.version.key
                        && c.file == file.path
                        && c.start <= end
                        && start <= c.end
                }) {
                    reasons.push(SelectionReason::CoverageHit {
                        file: file.path.clone(),
                        start,
                        end,
                    });
                }
            }
        }
        if !reasons.is_empty() {
            selected.insert(test.version.key.clone(), reasons);
        }
    }
    let mut available_observation_ids: Vec<_> = observations.iter().map(|o| o.id.clone()).collect();
    available_observation_ids.sort();
    let mut prediction = SelectionPrediction {
        id: String::new(),
        occurrence: None,
        base_snapshot: base.snapshot.clone(),
        target_snapshot: target.snapshot.clone(),
        baseline_collection: baseline.id.clone(),
        base_catalog: base.id.clone(),
        target_catalog: target.id.clone(),
        strategy_version: IMPLEMENTATION_VERSION.into(),
        available_observation_ids,
        selected: selected
            .into_iter()
            .map(|(test, reasons)| SelectedTest { test, reasons })
            .collect(),
        unknowns,
        changed_files: files.to_vec(),
        coverage_intervals: coverage.to_vec(),
        selection_ms: 0.0,
        can_skip_other_tests: false,
    };
    prediction.id = content_id(&prediction).map_err(|e| AnalysisError::Invalid(e.to_string()))?;
    Ok(prediction)
}

/// Logical identity and definition changes are independent of observed outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestIdentityRelation {
    New,
    Known,
    Unmapped,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestDefinitionChange {
    Changed,
    Unchanged,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureClassification {
    pub test: LogicalTestKey,
    pub identity: TestIdentityRelation,
    pub definition: TestDefinitionChange,
    pub previous_outcome: Option<TestOutcome>,
    pub previously_ignored: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionEvaluation {
    pub prediction: String,
    pub reference_collection: String,
    pub completeness: Completeness,
    pub selected_fraction: Option<f64>,
    pub detected_failures: Vec<LogicalTestKey>,
    pub missed_failures: Vec<LogicalTestKey>,
    pub pass_to_failure: Vec<LogicalTestKey>,
    pub existing_failures: Vec<LogicalTestKey>,
    pub new_or_unmapped_failures: Vec<LogicalTestKey>,
    /// None for historical reports without independently classified labels.
    #[serde(default)]
    pub failure_classifications: Option<Vec<FailureClassification>>,
    /// None when the complete reference contains no failures, or is incomplete.
    pub failure_recall: Option<f64>,
    pub reference_test_ms: f64,
    pub hypothetical_selected_test_ms: f64,
    pub shared_build_ms: f64,
    pub peak_test_rss_bytes: Option<u64>,
    pub artifact_bytes: u64,
    pub reproducibility_confirmed: bool,
    pub causal_regression_confirmed: bool,
}
pub fn evaluate(
    prediction: &SelectionPrediction,
    base: &TestCatalog,
    target: &TestCatalog,
    reference: &CollectionRun,
    observations: &[TestObservation],
    baseline: &[TestObservation],
) -> Result<SelectionEvaluation, AnalysisError> {
    if prediction.base_catalog != base.id
        || prediction.base_snapshot != base.snapshot
        || prediction.target_catalog != target.id
        || prediction.target_snapshot != target.snapshot
        || prediction.can_skip_other_tests
        || baseline.iter().any(|o| {
            o.collection != prediction.baseline_collection
                || !prediction.available_observation_ids.contains(&o.id)
        })
    {
        return Err(AnalysisError::Invalid(
            "prediction/reference provenance mismatch".into(),
        ));
    }
    let completeness = completeness(target, reference, observations)?;
    let executable: BTreeSet<_> = target
        .tests
        .iter()
        .filter(|t| !t.ignored)
        .map(|t| &t.version.key)
        .collect();
    let selected: BTreeSet<_> = prediction.selected.iter().map(|s| &s.test).collect();
    if selected.len() != prediction.selected.len() || !selected.is_subset(&executable) {
        return Err(AnalysisError::Invalid(
            "selection outside target catalog or duplicate".into(),
        ));
    }
    let previous: BTreeMap<_, _> = baseline
        .iter()
        .map(|o| (&o.version.key, o.outcome))
        .collect();
    let mut result = SelectionEvaluation {
        prediction: prediction.id.clone(),
        reference_collection: reference.id.clone(),
        selected_fraction: if completeness.reference_evaluable && !executable.is_empty() {
            Some(selected.len() as f64 / executable.len() as f64)
        } else {
            None
        },
        completeness,
        detected_failures: vec![],
        missed_failures: vec![],
        pass_to_failure: vec![],
        existing_failures: vec![],
        new_or_unmapped_failures: vec![],
        failure_classifications: None,
        failure_recall: None,
        reference_test_ms: 0.0,
        hypothetical_selected_test_ms: 0.0,
        shared_build_ms: reference.build_ms,
        peak_test_rss_bytes: None,
        artifact_bytes: 0,
        reproducibility_confirmed: false,
        causal_regression_confirmed: false,
    };
    let baseline_tests: BTreeMap<_, _> = base
        .tests
        .iter()
        .map(|test| (&test.version.key, test))
        .collect();
    let mut classifications = Vec::new();
    let mut ordered: Vec<_> = observations.iter().collect();
    ordered.sort_by(|a, b| a.version.key.cmp(&b.version.key));
    let mut artifact_sizes = BTreeMap::new();
    let mut missing_rss = false;
    for o in ordered {
        if o.outcome != TestOutcome::Ignored && o.peak_rss_bytes.is_none() {
            missing_rss = true;
        }
        result.reference_test_ms += o.duration_ms;
        result.peak_test_rss_bytes = result.peak_test_rss_bytes.max(o.peak_rss_bytes);
        for artifact in &o.artifacts {
            if artifact_sizes
                .insert(&artifact.sha256, artifact.bytes)
                .is_some_and(|previous| previous != artifact.bytes)
            {
                return Err(AnalysisError::Invalid(
                    "inconsistent content-addressed artifact sizes".into(),
                ));
            }
        }
        if selected.contains(&o.version.key) {
            result.hypothetical_selected_test_ms += o.duration_ms;
        }
        if o.outcome != TestOutcome::TestFailure {
            continue;
        }
        let old = baseline_tests.get(&o.version.key);
        let unknown_target = base.targets.iter().any(|target| {
            !target.supported
                && target.package == o.version.key.package
                && target.kind == o.version.key.target_kind
                && target.name == o.version.key.target_name
        });
        let identity = if old.is_some() {
            TestIdentityRelation::Known
        } else if base.enumeration_complete && !unknown_target {
            TestIdentityRelation::New
        } else {
            TestIdentityRelation::Unmapped
        };
        let definition = match old {
            Some(old)
                if old.version.definition_hash_provenance
                    == o.version.definition_hash_provenance =>
            {
                match (&old.version.definition_hash, &o.version.definition_hash) {
                    (Some(before), Some(after)) if before == after => {
                        TestDefinitionChange::Unchanged
                    }
                    (Some(_), Some(_)) => TestDefinitionChange::Changed,
                    _ => TestDefinitionChange::Unknown,
                }
            }
            _ => TestDefinitionChange::Unknown,
        };
        classifications.push(FailureClassification {
            test: o.version.key.clone(),
            identity,
            definition,
            previous_outcome: previous.get(&o.version.key).copied(),
            previously_ignored: old.is_some_and(|test| test.ignored),
        });
        if selected.contains(&o.version.key) {
            result.detected_failures.push(o.version.key.clone());
        } else {
            result.missed_failures.push(o.version.key.clone());
        }
        match previous.get(&o.version.key) {
            Some(TestOutcome::Pass) => result.pass_to_failure.push(o.version.key.clone()),
            Some(TestOutcome::TestFailure) => result.existing_failures.push(o.version.key.clone()),
            _ => result.new_or_unmapped_failures.push(o.version.key.clone()),
        }
    }
    for bytes in artifact_sizes.values() {
        result.artifact_bytes = result
            .artifact_bytes
            .checked_add(*bytes)
            .ok_or_else(|| AnalysisError::Invalid("artifact byte count overflow".into()))?;
    }
    if !result.reference_test_ms.is_finite() || !result.hypothetical_selected_test_ms.is_finite() {
        return Err(AnalysisError::Invalid(
            "test duration total overflow".into(),
        ));
    }
    result.failure_classifications = Some(classifications);
    if missing_rss {
        result.peak_test_rss_bytes = None;
    }
    let failures = result.detected_failures.len() + result.missed_failures.len();
    if result.completeness.reference_evaluable && failures > 0 {
        result.failure_recall = Some(result.detected_failures.len() as f64 / failures as f64);
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayPair {
    pub base: String,
    pub head: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayManifest {
    pub schema_version: u32,
    pub scope: RunScope,
    pub timeout_secs: u64,
    pub strategy: String,
    pub pairs: Vec<ReplayPair>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRun {
    pub id: String,
    pub manifest: ReplayManifest,
    pub implementation_version: String,
    #[serde(default)]
    pub environment_fingerprint: String,
    pub events: Vec<ResearchEvent>,
}
