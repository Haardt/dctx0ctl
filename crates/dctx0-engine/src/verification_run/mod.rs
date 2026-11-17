//! Managed native execution from a published snapshot, with immutable reports.
mod adoption;
mod continuation;
pub use continuation::VerificationContinuationPlan;
mod application_run;
pub use application_run::*;
mod adoption_preview;
pub use adoption_preview::{AdoptionPreviewTask, VerificationAdoptionPreview};
mod attestations;
mod build;
mod capture;
mod code_selection;
mod composition_attestations;
pub use composition_attestations::CompositionTaskAttestation;
mod dependencies;
mod execution;
mod hybrid_run;
pub use hybrid_run::HybridVerificationOptions;
mod incremental;
mod reconciliation;
mod reuse_execution;
mod reuse_inputs;
mod reuse_preflight;
pub use reconciliation::{NativeTaskReconciliation, ReconciliationCheck};
mod unit_plan;
mod units;
pub use incremental::{IncrementalComparison, VerificationIncrementalReport};
pub use reuse_execution::{NativeReuseMode, NativeReuseOptions, NativeReuseSummary};
pub use units::{DecoderUnitConfiguration, DecoderUnitGroup, UnitVerificationReport};
mod pins;
mod preflight;
mod status;
mod status_agent;
pub use preflight::{VerificationIntent, VerificationPreflight, VerificationPreflightPin};
pub use status::{
    ProjectStatus, StatusCause, StatusProgress, StatusRun, StatusRunSummary, StatusTaskCounts,
};
mod program_inspection;
mod replay;
mod review_changes;
pub(crate) mod rust_build;
mod shrink;
mod task_reviews;
use crate::*;
pub use attestations::TaskAttestation;
use capture::Capture;
use dctx0_core::*;
use dctx0_store::{StoredVerificationRun, VerificationArtifact};
use dctx0_target_sdk::{
    ExecutionRecipe, HarnessGenerator, HarnessRequest, PROTOCOL_VERSION, SDK_VERSION, ValueCheck,
    WorkerReply, WorkerRequest,
};
use dctx0_verification::{LifecycleCaseInput, ProgramPlan, ProjectionFixtures};
pub use program_inspection::InspectedVerificationReport;
pub use replay::{ReplayChange, VerificationReplayReport};
pub use review_changes::ReviewChangesReport;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

fn digest(bytes: &[u8]) -> String {
    VerificationArtifact::new("application/octet-stream", bytes.to_vec())
        .reference
        .hash
}
fn issue(code: &str, message: impl Into<String>) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: None,
    }
}
fn unknown(code: &str, message: impl Into<String>) -> VerificationResult {
    VerificationResult::Unknown {
        issue: issue(code, message),
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationLifecycleFixtures {
    pub paths: BTreeMap<String, LifecycleCaseInput>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct NativeVerificationReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incremental: Option<Box<NativeReuseSummary>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervision: Option<crate::NativeSupervisionCapabilities>,
    pub evidence: StoredVerificationRun,
    pub adopted_tasks: Vec<i64>,
    pub adoption_issues: Vec<VerificationIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adoption_proposal: Option<dctx0_tasks::AdoptionProposal>,
}
/// Exact published requirements and generated inputs selected by an editable test.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingTestPin {
    pub snapshot: String,
    pub check_occurrence: u64,
    pub plan_hash: String,
}
/// Ephemeral central assessment. It has no saved run ID or task-delivery authority.
#[derive(Debug, Serialize)]
pub struct BindingTestReport {
    pub published: bool,
    pub run: VerificationRun,
}
enum ExecutionReport {
    Published(NativeVerificationReport),
    Transient(BindingTestReport),
}
impl Project {
    pub async fn run_verification(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
    ) -> Result<NativeVerificationReport> {
        self.run_verification_plan(
            application,
            selection,
            generator,
            cancellation,
            no_adopt,
            None,
        )
        .await
    }
    /// Explicit historical reuse; execution and reused records are reported separately.
    /// This diagnostic path does not grant task-delivery authority.
    pub async fn run_verification_incremental(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        options: &NativeReuseOptions,
    ) -> Result<NativeVerificationReport> {
        self.run_incremental_with_pin(
            application,
            selection,
            generator,
            cancellation,
            options,
            None,
            true,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)] // Current source/task pin is a separate execution boundary.
    pub async fn run_verification_incremental_preflighted(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        options: &NativeReuseOptions,
        preflight: VerificationPreflight,
    ) -> Result<NativeVerificationReport> {
        if !preflight.permits_execution() {
            return Err(Error::Invalid(
                "PREFLIGHT_BLOCKED: incremental execution requires a permitted diagnostic scope"
                    .into(),
            ));
        }
        self.run_incremental_with_pin(
            application,
            selection,
            generator,
            cancellation,
            options,
            Some(&preflight.pin),
            true,
        )
        .await
    }
    /// Execute with explicit reuse and optionally deliver eligible task results.
    pub async fn run_verification_incremental_with_adoption(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        options: &NativeReuseOptions,
        no_adopt: bool,
    ) -> Result<NativeVerificationReport> {
        self.run_incremental_with_pin(
            application,
            selection,
            generator,
            cancellation,
            options,
            None,
            no_adopt,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn run_verification_incremental_preflighted_with_adoption(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        options: &NativeReuseOptions,
        preflight: VerificationPreflight,
        no_adopt: bool,
    ) -> Result<NativeVerificationReport> {
        if !preflight.permits_execution() {
            return Err(Error::Invalid(
                "PREFLIGHT_BLOCKED: incremental execution requires a permitted scope".into(),
            ));
        }
        let no_adopt = no_adopt || preflight.intent == VerificationIntent::Diagnostic;
        self.run_incremental_with_pin(
            application,
            selection,
            generator,
            cancellation,
            options,
            Some(&preflight.pin),
            no_adopt,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)] // Shared implementation retains the optional preflight boundary.
    async fn run_incremental_with_pin(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        options: &NativeReuseOptions,
        preflight: Option<&VerificationPreflightPin>,
        no_adopt: bool,
    ) -> Result<NativeVerificationReport> {
        let (_, config) = self.verification_configuration(application)?;
        let limits = config
            .execution
            .as_ref()
            .map(VerificationExecutionBudgets::native_limits)
            .unwrap_or_default();
        match self
            .execute_verification(
                application,
                selection,
                generator,
                cancellation,
                no_adopt,
                None,
                limits,
                None,
                preflight,
                None,
                None,
                Some(options),
                None,
                None,
            )
            .await?
        {
            ExecutionReport::Published(report) => Ok(report),
            ExecutionReport::Transient(_) => Err(Error::Invalid(
                "Unexpected transient incremental verification".into(),
            )),
        }
    }
    pub(super) async fn run_verification_plan(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
        pinned: Option<(ProgramPlan, VerificationLifecycleFixtures, Option<String>)>,
    ) -> Result<NativeVerificationReport> {
        let (_, config) = self.verification_configuration(application)?;
        let limits = config
            .execution
            .as_ref()
            .map(VerificationExecutionBudgets::native_limits)
            .unwrap_or_default();
        self.run_verification_with_limits(
            application,
            selection,
            generator,
            cancellation,
            no_adopt,
            pinned,
            limits,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    /// Execute current requirements with explicit process budgets and supervision.
    pub async fn run_verification_with_native_limits(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
        limits: NativeLimits,
    ) -> Result<NativeVerificationReport> {
        self.run_verification_with_limits(
            application,
            selection,
            generator,
            cancellation,
            no_adopt,
            None,
            limits,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_verification_with_limits(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
        pinned: Option<(ProgramPlan, VerificationLifecycleFixtures, Option<String>)>,
        limits: NativeLimits,
    ) -> Result<NativeVerificationReport> {
        match self
            .execute_verification(
                application,
                selection,
                generator,
                cancellation,
                no_adopt,
                pinned,
                limits,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?
        {
            ExecutionReport::Published(report) => Ok(report),
            ExecutionReport::Transient(_) => {
                Err(Error::Invalid("Unexpected transient verification".into()))
            }
        }
    }
    /// Execute the full selected finite plan without publishing or adopting evidence.
    pub async fn test_verification(
        &self,
        application: &Path,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        expected: BindingTestPin,
    ) -> Result<BindingTestReport> {
        let (_, config) = self.verification_configuration(Some(application))?;
        let limits = config
            .execution
            .as_ref()
            .map(VerificationExecutionBudgets::native_limits)
            .unwrap_or_default();
        match self
            .execute_verification(
                Some(application),
                selection,
                generator,
                cancellation,
                true,
                None,
                limits,
                Some(expected),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?
        {
            ExecutionReport::Transient(report) => Ok(report),
            ExecutionReport::Published(_) => {
                Err(Error::Invalid("Unexpected published verification".into()))
            }
        }
    }
    /// Execute only the scope and sources inspected by the shared preflight.
    /// Existing low-level run APIs remain available for diagnostic consumers.
    pub async fn run_verification_preflighted(
        &self,
        application: Option<&Path>,
        selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        preflight: VerificationPreflight,
    ) -> Result<NativeVerificationReport> {
        if !preflight.permits_execution() {
            return Err(Error::Invalid("PREFLIGHT_BLOCKED: resolve prerequisites or explicitly select diagnostic execution".into()));
        }
        let (_, config) = self.verification_configuration(application)?;
        let limits = config
            .execution
            .as_ref()
            .map(VerificationExecutionBudgets::native_limits)
            .unwrap_or_default();
        match self
            .execute_verification(
                application,
                selection,
                generator,
                cancellation,
                preflight.intent == VerificationIntent::Diagnostic,
                None,
                limits,
                None,
                Some(&preflight.pin),
                None,
                None,
                None,
                None,
                None,
            )
            .await?
        {
            ExecutionReport::Published(report) => Ok(report),
            ExecutionReport::Transient(_) => {
                Err(Error::Invalid("Unexpected transient verification".into()))
            }
        }
    }
    /// Execute only centrally assigned adapter cases. This diagnostic entry
    /// cannot adopt tasks; a partial backend is not a complete project proof.
    pub async fn run_adapter_execution_group(
        &self,
        application: &Path,
        assignment: &dctx0_verification::application_plan::ApplicationTestPlan,
        group: &str,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
    ) -> Result<NativeVerificationReport> {
        let (_, config) = self.verification_configuration(Some(application))?;
        let limits = config
            .execution
            .as_ref()
            .map(VerificationExecutionBudgets::native_limits)
            .unwrap_or_default();
        match self
            .execute_verification(
                Some(application),
                VerificationSelection::default(),
                generator,
                cancellation,
                true,
                None,
                limits,
                None,
                None,
                Some((assignment, group)),
                None,
                None,
                None,
                None,
            )
            .await?
        {
            ExecutionReport::Published(report) => Ok(report),
            ExecutionReport::Transient(_) => {
                Err(Error::Invalid("Unexpected transient adapter group".into()))
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    async fn execute_verification(
        &self,
        application: Option<&Path>,
        mut selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
        pinned: Option<(ProgramPlan, VerificationLifecycleFixtures, Option<String>)>,
        limits: NativeLimits,
        transient: Option<BindingTestPin>,
        preflight: Option<&VerificationPreflightPin>,
        assigned_group: Option<(
            &dctx0_verification::application_plan::ApplicationTestPlan,
            &str,
        )>,
        hybrid: Option<&hybrid_run::HybridExecution>,
        reuse_options: Option<&NativeReuseOptions>,
        continuation: Option<&continuation::ContinuationSelection>,
        mut unit_context: Option<&mut units::UnitNativeContext>,
    ) -> Result<ExecutionReport> {
        let parent = pinned.as_ref().and_then(|(_, _, parent)| parent.clone());
        let supervision = limits.supervision_capabilities();
        let historical = selection.historical_check.is_some() || pinned.is_some();
        let no_adopt = no_adopt || historical;
        let automatic_options = NativeReuseOptions::default();
        let reuse_options = reuse_options
            .or_else(|| (transient.is_none() && !historical).then_some(&automatic_options));
        let (application, config) = self.verification_configuration(application)?;
        let mut limits = limits;
        limits.environment = config.environment.clone().unwrap_or_default();
        if selection.subjects.is_empty() && selection.tasks.is_empty() {
            selection.subjects = config.scope.subjects.clone();
            selection.tasks = config.scope.tasks.clone();
        }
        let projection_bytes = if pinned.is_none() {
            Some(fs::read(
                crate::verification_setup::safe_file(&application, &config.fixtures)?.1,
            )?)
        } else {
            None
        };
        let fixtures: ProjectionFixtures = if let Some(bytes) = &projection_bytes {
            serde_json::from_slice(bytes)?
        } else {
            Default::default()
        };
        let (mut plan, pinned_lifecycles) = match pinned {
            Some((plan, lifecycles, _)) => (plan, Some(lifecycles)),
            None => (
                self.verification_plan(selection, &fixtures, config.budgets.clone())?,
                None,
            ),
        };
        if hybrid.is_none()
            && continuation.is_none()
            && let Some(options) = reuse_options
        {
            reuse_preflight::guard(self, &application, &plan, options, cancellation).await?;
        }
        let mut capture = Capture::new_with_budget(&application, config.capture_budgets())?;
        let captured_inputs = capture.files.clone();
        if let Some(bytes) = projection_bytes
            && fs::read(capture.root.join(&config.fixtures))? != bytes
        {
            return Err(Error::Invalid("PREFLIGHT_FIXTURES_CHANGED: projection inputs changed during planning; no native execution started".into()));
        }
        let assigned_cases = if let Some(context) = continuation {
            let required = dctx0_verification::case_manifest(&plan).required_cases;
            if assigned_group.is_some()
                || hybrid.is_some()
                || reuse_options.is_some()
                || !context.plan.selected_cases.is_subset(&required)
            {
                return Err(Error::Invalid(
                    "Continuation cannot expand or mix execution cohorts".into(),
                ));
            }
            Some(context.plan.selected_cases.clone())
        } else if let Some((assignment, group)) = assigned_group {
            assignment.validate().map_err(|issues| {
                Error::Invalid(format!("Invalid hybrid assignment: {issues:?}"))
            })?;
            let valid_group = assignment.configuration.groups.iter().any(|entry| {
                entry.id == group
                    && entry.backend == dctx0_verification::application_plan::TestBackend::Adapter
                    && entry.target == config.target
            });
            if assignment.program != plan || !valid_group {
                return Err(Error::Invalid(
                    "Adapter group must match the current complete program and configured target"
                        .into(),
                ));
            }
            Some(
                assignment
                    .case_groups
                    .iter()
                    .filter(|(_, owner)| owner.as_str() == group)
                    .map(|(id, _)| id.clone())
                    .collect::<BTreeSet<_>>(),
            )
        } else {
            None
        };
        if let Some(expected) = preflight {
            if plan.snapshot != expected.snapshot
                || plan.check_occurrence != expected.check_occurrence
                || digest(&serde_json::to_vec(&plan)?) != expected.plan_hash
                || digest(&serde_json::to_vec(&captured_inputs)?) != expected.inputs_hash
            {
                return Err(Error::Invalid("PREFLIGHT_INPUTS_CHANGED: source inventory, configuration or published plan changed; repeat preflight before native execution".into()));
            }
        }
        if let Some(expected) = &transient {
            if plan.snapshot != expected.snapshot
                || plan.check_occurrence != expected.check_occurrence
                || digest(&serde_json::to_vec(&plan)?) != expected.plan_hash
            {
                return Err(Error::Invalid(
                    "Generated binding test pins are stale; regenerate a candidate".into(),
                ));
            }
        }
        if plan
            .issues
            .iter()
            .any(|i| i.code == "INVALID_OPERATION_CASE")
        {
            return Err(Error::Invalid(
                "Invalid reviewed operation cases; inspect verify plan before native execution"
                    .into(),
            ));
        }
        let snapshot = self
            .store(AccessMode::ReadOnly)?
            .load(&self.key(), &plan.snapshot)?;
        let (mut task_pin, mut pin_issues) = if unit_context.is_some() {
            (None, Vec::new())
        } else {
            pins::capture(self, &application, &config.target, &plan, &snapshot)?
        };
        let incremental_done_versions = if reuse_options.is_some() && hybrid.is_none() && !no_adopt
        {
            Some(adoption::capture_done_versions(self, task_pin.as_ref())?)
        } else {
            None
        };
        if let Some(context) = hybrid {
            if captured_inputs != context.source_inputs
                || captured_inputs != context.report.source_inputs
            {
                pin_issues.push(issue(
                    "HYBRID_INPUTS_CHANGED",
                    "Backend groups did not execute the same source inventory",
                ));
            }
            if task_pin != context.task_pin {
                pin_issues.push(issue(
                    "HYBRID_TASK_PIN_CHANGED",
                    "Task/profile requirements changed between backend groups",
                ));
            }
            task_pin = context.task_pin.clone();
            pin_issues.extend(context.pin_issues.clone());
        }
        if preflight.is_some_and(|expected| expected.task_pin != task_pin) {
            return Err(Error::Invalid("PREFLIGHT_TASK_PIN_CHANGED: task events, requirements or selected profiles changed; repeat preflight before native execution".into()));
        }
        let (directory, adapter_name) = config
            .adapter
            .rsplit_once('/')
            .unwrap_or(("", &config.adapter));
        let directory = Path::new(directory);
        let lifecycle_path = capture.root.join(directory).join("lifecycles.json");
        if pinned_lifecycles.is_none() {
            let issues =
                self.verification_lifecycle_input_issues(&capture.root, &config.adapter, &plan)?;
            if assigned_group.is_none() {
                plan.issues.extend(issues);
            } else {
                pin_issues.extend(issues);
            }
        }
        let lifecycles: VerificationLifecycleFixtures = if let Some(lifecycles) = pinned_lifecycles
        {
            lifecycles
        } else if lifecycle_path.try_exists()? {
            serde_json::from_slice(&fs::read(lifecycle_path)?)?
        } else {
            Default::default()
        };
        // Setup grants invocation access, not a verification claim for its operation.
        let mut harness_subjects = plan.closure.clone();
        for invocation in plan
            .operations
            .iter()
            .flat_map(|operation| &operation.cases)
            .flat_map(|case| &case.setup)
            .chain(
                plan.lifecycles
                    .iter()
                    .flat_map(|lifecycle| &lifecycle.paths)
                    .filter_map(|path| lifecycles.paths.get(&path.id))
                    .flat_map(|input| &input.setup),
            )
        {
            if snapshot.ir.operations.contains_key(&invocation.subject) {
                harness_subjects.insert(invocation.subject.clone());
            }
        }
        let mut bundle = generator
            .generate(&HarnessRequest {
                adapter_import: format!("./{adapter_name}"),
                subjects: harness_subjects,
            })
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let mut observer_sources = Vec::new();
        for path in capture.files.keys() {
            let language = match Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
            {
                Some("ts" | "js" | "mjs" | "cjs") => {
                    dctx0_target_sdk::InspectionLanguage::TypeScript
                }
                Some("tsx" | "jsx") => dctx0_target_sdk::InspectionLanguage::Tsx,
                Some("go") => dctx0_target_sdk::InspectionLanguage::Go,
                Some("rs") => dctx0_target_sdk::InspectionLanguage::Rust,
                _ => continue,
            };
            observer_sources.push(dctx0_target_sdk::InspectionSource {
                path: path.clone(),
                language,
                text: fs::read_to_string(capture.root.join(path))?,
            });
        }
        let depth = directory
            .components()
            .filter(|component| matches!(component, std::path::Component::Normal(_)))
            .count();
        let root_relative = if depth == 0 {
            "./".into()
        } else {
            "../".repeat(depth)
        };
        bundle.artifacts.extend(
            generator
                .module_observer_artifacts(&observer_sources, &root_relative)
                .map_err(|error| Error::Invalid(error.to_string()))?,
        );
        if bundle.sdk_version != SDK_VERSION
            || bundle.target != config.target
            || (bundle.build.is_none() && bundle.recipe.program != config.runtime)
            || (bundle.build.is_some() && generator.runtime() != config.runtime)
        {
            return Err(Error::Invalid(
                "Configured target/runtime does not match the generated harness".into(),
            ));
        }
        let mut artifacts = vec![VerificationArtifact::new(
            "application/vnd.dctx0.plan+json",
            serde_json::to_vec(&plan)?,
        )];
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.captured-input-inventory+json",
            serde_json::to_vec(&captured_inputs)?,
        ));
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.assessment-version+json",
            serde_json::to_vec(&serde_json::json!({"schema_version":1,"version":dctx0_verification::ASSESSMENT_VERSION}))?,
        ));
        match crate::native::VerifierIdentity::capture() {
            Ok(identity) => artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.verifier-identity+json",
                serde_json::to_vec(&identity)?,
            )),
            Err(error) => {
                pin_issues.push(issue("VERIFIER_IDENTITY_UNAVAILABLE", error.to_string()))
            }
        }
        artifacts.push(dependencies::limits_artifact(
            config.dependency_resolution.unwrap_or_default(),
        )?);
        let mut artifact_names = BTreeSet::new();
        for artifact in &bundle.artifacts {
            let path = Path::new(&artifact.path);
            if artifact.path == adapter_name
                || !artifact_names.insert(artifact.path.clone())
                || path.components().count() != 1
                || !matches!(
                    path.components().next(),
                    Some(std::path::Component::Normal(_))
                )
            {
                return Err(Error::Invalid(
                    "Generated harness artifact must have a local filename".into(),
                ));
            }
            fs::write(capture.root.join(directory).join(path), &artifact.contents)?;
            capture
                .files
                .remove(&directory.join(path).to_string_lossy().into_owned());
            artifacts.push(VerificationArtifact::new(
                if artifact.path == "module-observer.json" {
                    "application/vnd.dctx0.module-observer-manifest+json"
                } else {
                    if bundle.target == "rust" {
                        "text/x-rust"
                    } else if bundle.target == "go" {
                        "text/x-go"
                    } else {
                        "text/typescript"
                    }
                },
                artifact.contents.as_bytes().to_vec(),
            ));
        }
        let model = serde_json::to_vec(
            &serde_json::json!({"data":snapshot.ir.data,"operations":snapshot.ir.operations}),
        )?;
        fs::write(capture.root.join(directory).join("model.json"), &model)?;
        capture
            .files
            .remove(&directory.join("model.json").to_string_lossy().into_owned());
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.model+json",
            model.clone(),
        ));
        let generated = artifact_names
            .iter()
            .chain(std::iter::once(&"model.json".to_string()))
            .map(|name| directory.join(name).to_string_lossy().replace('\\', "/"))
            .collect::<BTreeSet<_>>();
        pin_issues.extend(dependencies::inspect(
            &capture,
            generator,
            &generated,
            &mut artifacts,
        )?);
        let mut go_receipt = None;
        let mut rust_receipt = None;
        let rust_pending = if config.target == "rust" {
            Some(
                rust_build::prepare_with_environment(
                    &capture.root.join(directory),
                    &capture.root,
                    cancellation,
                    &limits.environment,
                )
                .await,
            )
        } else {
            None
        };
        let mut build_ready = true;
        if let Some(Ok(receipt)) = &rust_pending {
            if let Some(recipe) = bundle.build.as_mut() {
                if let Some(position) = recipe
                    .arguments
                    .iter()
                    .position(|arg| arg == "--target-dir")
                {
                    if let Some(value) = recipe.arguments.get_mut(position + 1) {
                        *value = receipt.target_directory.clone();
                    }
                }
            }
        }
        if let Some(Err(error)) = &rust_pending {
            pin_issues.push(issue("RUST_BUILD_INPUTS_UNAVAILABLE", error.to_string()));
            build_ready = false;
        }
        if let Some(recipe) = bundle.build.as_ref().filter(|_| build_ready) {
            if let Err(error) = build::execute(
                recipe,
                &capture.root.join(directory),
                cancellation,
                &mut artifacts,
                &limits.environment,
            )
            .await
            {
                pin_issues.push(issue("NATIVE_BUILD", error.to_string()));
                build_ready = false;
            } else if config.target == "rust" {
                match rust_pending
                    .and_then(|pending| pending.ok())
                    .ok_or_else(|| Error::Invalid("Missing Rust build input receipt".into()))
                    .and_then(|receipt| rust_build::finish(receipt, &capture.root.join(directory)))
                {
                    Ok(receipt) => {
                        artifacts.push(VerificationArtifact::new(
                            "application/vnd.dctx0.rust-build-inputs+json",
                            serde_json::to_vec(&receipt)?,
                        ));
                        if transient.is_some() {
                            bundle.recipe.program = capture
                                .root
                                .join(directory)
                                .join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX))
                                .to_string_lossy()
                                .into_owned();
                        } else {
                            let relative = format!(
                                ".dctx0ctl/native/rust/{}/worker{}",
                                receipt.binary_hash,
                                std::env::consts::EXE_SUFFIX
                            );
                            let (parent, executable) =
                                crate::verification_setup::safe_file(&application, &relative)?;
                            fs::create_dir_all(parent)?;
                            if executable.try_exists()? {
                                if digest(&fs::read(&executable)?) != receipt.binary_hash {
                                    return Err(Error::Invalid(
                                        "Stored Rust worker hash mismatch".into(),
                                    ));
                                }
                            } else {
                                fs::copy(
                                    capture.root.join(directory).join(format!(
                                        ".bin/worker{}",
                                        std::env::consts::EXE_SUFFIX
                                    )),
                                    &executable,
                                )?;
                            }
                            bundle.recipe.program = executable.to_string_lossy().into_owned();
                        }
                        rust_receipt = Some(receipt);
                    }
                    Err(error) => {
                        pin_issues.push(issue("RUST_BUILD_INPUTS_UNAVAILABLE", error.to_string()));
                        build_ready = false;
                    }
                }
            } else if config.target == "go" {
                match build::go_receipt(&capture.root.join(directory)) {
                    Ok(receipt) => {
                        artifacts.push(VerificationArtifact::new(
                            "application/vnd.dctx0.go-build-inputs+json",
                            serde_json::to_vec(&receipt)?,
                        ));
                        if let Some(context) = unit_context.as_deref_mut() {
                            context.bind_compilation(self, &receipt, generator, &mut artifacts)?;
                        }
                        if transient.is_some() {
                            bundle.recipe.program = capture
                                .root
                                .join(directory)
                                .join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX))
                                .to_string_lossy()
                                .into_owned();
                        } else {
                            let relative = format!(
                                ".dctx0ctl/native/go/{}/worker{}",
                                receipt.binary_hash,
                                std::env::consts::EXE_SUFFIX
                            );
                            let (parent, executable) =
                                crate::verification_setup::safe_file(&application, &relative)?;
                            fs::create_dir_all(parent)?;
                            if executable.try_exists()? {
                                if digest(&fs::read(&executable)?) != receipt.binary_hash {
                                    return Err(Error::Invalid(
                                        "Stored Go worker hash mismatch".into(),
                                    ));
                                }
                            } else {
                                fs::copy(
                                    capture.root.join(directory).join(format!(
                                        ".bin/worker{}",
                                        std::env::consts::EXE_SUFFIX
                                    )),
                                    &executable,
                                )?;
                            }
                            bundle.recipe.program = executable.to_string_lossy().into_owned();
                        }
                        go_receipt = Some(receipt);
                    }
                    Err(error) => {
                        pin_issues.push(issue("GO_BUILD_INPUTS_UNAVAILABLE", error.to_string()));
                        build_ready = false;
                    }
                }
            }
        }
        let execution_inputs =
            Capture::inventory_with_budget(&capture.root, config.capture_budgets())?;
        let started_at_unix_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| Error::Invalid(e.to_string()))?
            .as_nanos()
            .to_string();
        let started = Instant::now();
        let provenance = VerificationProvenance {
            snapshot: plan.snapshot.clone(),
            check_occurrence: plan.check_occurrence as i64,
            plan_hash: artifacts[0].reference.hash.clone(),
            // Both hybrid backends bind the same original source tree. The
            // adapter's deterministic generated replacements remain separately
            // bound by its harness/build receipts and execution-input inventory.
            implementation: if hybrid.is_some() {
                captured_inputs.clone()
            } else {
                capture
                    .files
                    .iter()
                    .filter(|(p, _)| !p.starts_with("node_modules/"))
                    .map(|(p, h)| (p.clone(), h.clone()))
                    .collect()
            },
            dependencies: capture
                .files
                .iter()
                .filter(|(p, _)| p.starts_with("node_modules/"))
                .map(|(p, h)| (p.clone(), h.clone()))
                .collect(),
            adapter_hash: capture.files.get(&config.adapter).cloned().ok_or_else(|| {
                Error::Invalid(
                    "Configured adapter is excluded by code selection or .gitignore".into(),
                )
            })?,
            configuration_hash: digest(&serde_json::to_vec(&config)?),
            harness_hash: digest(&serde_json::to_vec(
                &serde_json::json!({"bundle":bundle,"model_hash":digest(&model)}),
            )?),
            interpreter_version: dctx0_interpreter::INTERPRETER_VERSION.into(),
            generator_version: plan.generator_version.clone(),
            protocol_version: PROTOCOL_VERSION,
            target: bundle.target.clone(),
            runtime_version: String::new(),
            build_configuration: capture
                .files
                .iter()
                .filter(|(path, _)| {
                    matches!(
                        path.as_str(),
                        "package.json"
                            | "bun.lock"
                            | "bun.lockb"
                            | "tsconfig.json"
                            | "go.mod"
                            | "go.sum"
                            | "Cargo.toml"
                            | "Cargo.lock"
                    ) || path.starts_with("tsconfig.") && path.ends_with(".json")
                })
                .map(|(path, hash)| (path.clone(), hash.clone()))
                .collect(),
            task_pin,
            incomplete: pin_issues,
        };
        if let Some(context) = continuation {
            artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.continuation+json",
                serde_json::to_vec(&context.plan)?,
            ));
            if let Some(inputs) = &context.lifecycle_inputs {
                let artifact = VerificationArtifact::new(
                    "application/vnd.dctx0.continuation-lifecycle-inputs+json",
                    serde_json::to_vec(inputs)?,
                );
                if context.plan.lifecycle_inputs_hash.as_ref() != Some(&artifact.reference.hash) {
                    return Err(Error::Invalid(
                        "Continuation lifecycle input digest differs".into(),
                    ));
                }
                artifacts.push(artifact);
            }
        }
        let mut run = VerificationRun {
            schema_version: 1,
            origin: VerificationRunOrigin::Native,
            started_at_unix_ns,
            elapsed_us: 0,
            provenance,
            budgets: plan.budgets.clone(),
            selected_subjects: plan.scope.subjects.clone(),
            required_subjects: plan.closure.clone(),
            not_selected: plan.not_selected.clone(),
            required_cases: BTreeSet::new(),
            cases: hybrid.map(|context| context.cases()).unwrap_or_default(),
            open_obligations: plan.issues.clone(),
            bounded_obligations: vec![],
            artifacts: vec![],
            result: VerificationResult::Pass,
        };
        if !build_ready {
            let manifest = dctx0_verification::case_manifest(&plan);
            run.required_cases = if hybrid.is_some() {
                manifest.required_cases.clone()
            } else {
                assigned_cases
                    .as_ref()
                    .unwrap_or(&manifest.required_cases)
                    .clone()
            };
            run.open_obligations.extend(manifest.open_obligations);
            run.bounded_obligations.extend(manifest.bounded_obligations);
        }
        let mut reuse_state = if let Some(options) = reuse_options {
            Some(
                reuse_execution::PreparedReuse::capture(
                    self,
                    &application,
                    &plan,
                    options,
                    cancellation,
                )
                .await?,
            )
        } else {
            None
        };
        if let Some(state) = &reuse_state {
            run.provenance.incomplete.extend(state.input_issues());
        }
        if let (Some(state), Some(assigned)) = (&reuse_state, &assigned_cases) {
            if state.enabled()
                && state
                    .units
                    .iter()
                    .any(|unit| unit.reusable && !unit.cases.is_subset(assigned))
            {
                return Err(Error::Invalid("Hybrid reuse declarations must select complete adapter-owned units; native group remains fresh".into()));
            }
        }
        if build_ready {
            execution::execute(
                execution::Input {
                    ir: &snapshot.ir,
                    root: &capture.root,
                    adapter: &config.adapter,
                    generated: &generated,
                    plan: &plan,
                    assigned_cases: assigned_cases.as_ref(),
                    reuse: reuse_state.as_mut(),
                    complete_cohort: hybrid.is_some(),
                    identity_context: continuation
                        .map(|context| context.identity_context.as_slice())
                        .unwrap_or_default(),
                    lifecycles: &lifecycles,
                    cancel: cancellation,
                    limits,
                    dependency_budget: config.dependency_resolution.unwrap_or_default(),
                },
                &bundle.recipe,
                &capture.root.join(directory),
                &mut run,
                &mut artifacts,
            )
            .await?;
        }
        let resolved =
            dctx0_verification::resolved_state_preconditions(&snapshot.ir, &plan, &run.cases);
        if !resolved.is_empty() {
            run.open_obligations
                .retain(|issue| !resolved.contains(issue));
            artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.resolved-preconditions+json",
                serde_json::to_vec(&resolved)?,
            ));
        }
        let resolved_suites = dctx0_verification::suites::resolved_native_suite_policy_obligations(
            &snapshot.ir,
            &plan,
            &run.cases,
        );
        run.open_obligations
            .retain(|issue| !resolved_suites.contains(issue));
        run.elapsed_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        run.artifacts = artifacts.iter().map(|a| a.reference.clone()).collect();
        let policy_failure = dctx0_verification::suites::native_suite_policy_assessment(
            &snapshot.ir,
            &plan,
            &run.cases,
        )
        .policies
        .into_iter()
        .find_map(|policy| {
            matches!(policy.result, VerificationResult::Fail { .. }).then_some(policy.result)
        });
        if let Some((_, group)) = assigned_group.filter(|_| hybrid.is_none()) {
            let incomplete = issue(
                "HYBRID_COHORT_REQUIRED",
                format!(
                    "Adapter group {group} is diagnostic until the complete fresh backend cohort is verified"
                ),
            );
            run.provenance.incomplete.push(incomplete.clone());
            run.open_obligations.push(incomplete);
        }
        if continuation.is_some() {
            let gap = issue(
                "CONTINUATION_COMPOSITION_REQUIRED",
                "Selected pending cases are diagnostic native observations; the full inventory and historical producer bindings require separate validated composition",
            );
            run.provenance.incomplete.push(gap.clone());
            run.open_obligations.push(gap);
        }
        run.result = run
            .cases
            .iter()
            .find_map(|c| {
                if matches!(c.result, VerificationResult::Fail { .. }) {
                    Some(c.result.clone())
                } else {
                    None
                }
            })
            .or(policy_failure)
            .unwrap_or_else(|| {
                if let Some(problem) = run.open_obligations.first() {
                    VerificationResult::Unknown {
                        issue: problem.clone(),
                    }
                } else if let Some(case) = run
                    .cases
                    .iter()
                    .find(|c| matches!(c.result, VerificationResult::Unknown { .. }))
                {
                    case.result.clone()
                } else if run.required_cases.is_empty()
                    || run.required_cases.len() != run.cases.len()
                {
                    unknown("INCOMPLETE_RUN", "Not every mandatory case was executed")
                } else {
                    VerificationResult::Pass
                }
            });
        match Capture::inventory_with_budget(&capture.root, config.capture_budgets()) {
            Ok(current) if current == execution_inputs => {}
            Ok(_) => run.provenance.incomplete.push(issue(
                "CAPTURED_INPUTS_CHANGED",
                "Executed sources, adapter, harness or dependencies changed inside the captured tree",
            )),
            Err(error) => run.provenance.incomplete.push(issue(
                "CAPTURED_INPUTS_UNAVAILABLE",
                format!("Cannot reassess captured execution inputs: {error}"),
            )),
        }
        if let Some(receipt) = &rust_receipt {
            if let Err(error) = rust_build::validate(receipt) {
                run.provenance
                    .incomplete
                    .push(issue("RUST_BUILD_INPUTS_CHANGED", error.to_string()));
            }
        }
        if let Some(receipt) = &go_receipt {
            if let Err(error) = build::validate(receipt) {
                run.provenance
                    .incomplete
                    .push(issue("GO_BUILD_INPUTS_CHANGED", error.to_string()));
            }
        }
        let _lock = if transient.is_none() {
            Some(self.lock()?)
        } else {
            None
        };
        match Capture::inventory_with_budget(&application, config.capture_budgets()) {
            Ok(current) if current == captured_inputs => {}
            Ok(_) => run.provenance.incomplete.push(issue(
                "APPLICATION_INPUTS_CHANGED",
                "Application, adapter, configuration or dependencies changed since execution capture",
            )),
            Err(error) => run.provenance.incomplete.push(issue(
                "APPLICATION_INPUTS_UNAVAILABLE",
                format!("Cannot reassess execution input freshness: {error}"),
            )),
        }
        if !historical || continuation.is_some() {
            match self.sources() {
                Ok(sources) if dctx0_changes::source_hash(&sources) == snapshot.source_hash => {},
                Ok(_) => run.provenance.incomplete.push(issue("SPEC_INPUTS_CHANGED",
                    "Spec sources changed during native execution; published requirements remain pinned but current tasks cannot be adopted")),
                Err(error) => run.provenance.incomplete.push(issue("SPEC_INPUTS_UNAVAILABLE", error.to_string())),
            }
        }
        let identity = artifacts
            .iter()
            .find(|artifact| {
                artifact.reference.media_type == "application/vnd.dctx0.runtime-identity+json"
            })
            .map(|artifact| {
                serde_json::from_slice::<crate::NativeRuntimeIdentity>(&artifact.contents)
            })
            .transpose()?;
        if let (Some(receipt), Some(identity)) = (&rust_receipt, &identity) {
            if receipt.binary_hash != identity.executable_hash {
                run.provenance.incomplete.push(issue(
                    "RUST_EXECUTABLE_MISMATCH",
                    "Actual worker differs from compiled artifact",
                ));
            }
        }
        if let (Some(receipt), Some(identity)) = (&go_receipt, &identity) {
            if receipt.binary_hash != identity.executable_hash {
                run.provenance.incomplete.push(issue(
                    "GO_EXECUTABLE_MISMATCH",
                    "Actual worker differs from compiled artifact",
                ));
            }
        }
        match identity.map(|identity| identity.still_current()) {
            Some(Ok(true)) => {},
            other => run.provenance.incomplete.push(issue("RUNTIME_IDENTITY_CHANGED_OR_UNAVAILABLE",
                format!("Actual runtime executable or launch environment cannot be pinned at publication: {other:?}"))),
        }
        if let Some(context) = hybrid {
            let adapter_started = run.started_at_unix_ns.clone();
            let adapter_elapsed = run.elapsed_us;
            run.started_at_unix_ns = context.started_at_unix_ns.clone();
            run.elapsed_us = context.started.elapsed().as_micros().min(u64::MAX as u128) as u64;
            if crate::native::VerifierIdentity::capture().ok().as_ref() != Some(&context.verifier) {
                run.provenance.incomplete.push(issue(
                    "HYBRID_VERIFIER_CHANGED",
                    "Managing verifier changed during the cohort",
                ));
            }
            context.attach(
                &mut run,
                &mut artifacts,
                adapter_started,
                adapter_elapsed,
                reuse_state
                    .as_ref()
                    .map(|state| state.reused_cases())
                    .unwrap_or(&BTreeSet::new()),
            )?;
            if let Some(notes) = &context.inspections {
                program_inspection::apply_fresh_hybrid_inspections(
                    self,
                    &application,
                    &snapshot,
                    &plan,
                    &mut run,
                    &mut artifacts,
                    notes,
                )?;
            }
            run.artifacts = artifacts
                .iter()
                .map(|artifact| artifact.reference.clone())
                .collect();
            if matches!(run.result, VerificationResult::Pass) {
                if let Some(problem) = run.provenance.incomplete.first() {
                    run.result = VerificationResult::Unknown {
                        issue: problem.clone(),
                    };
                }
            }
        }
        if transient.is_some() {
            if matches!(run.result, VerificationResult::Pass) {
                if let Some(problem) = run.provenance.incomplete.first() {
                    run.result = VerificationResult::Unknown {
                        issue: problem.clone(),
                    };
                }
            }
            return Ok(ExecutionReport::Transient(BindingTestReport {
                published: false,
                run,
            }));
        }
        if let Some(state) = &mut reuse_state {
            state
                .finish(&application, &plan, &mut run, &mut artifacts, cancellation)
                .await?;
            run.artifacts = artifacts.iter().map(|a| a.reference.clone()).collect();
        }
        let mut incremental = reuse_state
            .as_ref()
            .map(|state| Box::new(state.summary(&plan, &run)));
        if let Some(context) = unit_context {
            context.complete(&run, &mut artifacts)?;
            run.artifacts = artifacts.iter().map(|a| a.reference.clone()).collect();
        }
        preserve_verification_history(&config, &mut run, &mut artifacts);
        let saved = self
            .store(AccessMode::ReadWrite)?
            .record_verification_run_owned_from(&self.key(), run, &artifacts, parent.as_deref())?;
        let (adopted_tasks, adoption_issues, adoption_proposal) = if no_adopt {
            (vec![], vec![], None)
        } else if let Some(context) = hybrid {
            adoption::publish_hybrid(
                self,
                &application,
                &plan,
                &saved,
                &artifacts,
                &context.done_versions,
            )?
        } else if let Some(done_versions) = &incremental_done_versions {
            adoption::publish_incremental(
                self,
                &application,
                &plan,
                &saved,
                &artifacts,
                done_versions,
            )?
        } else {
            adoption::publish(self, &application, &plan, &saved, &artifacts)?
        };
        if let Some(summary) = &mut incremental {
            summary.adoption_authority = !adopted_tasks.is_empty();
        }
        Ok(ExecutionReport::Published(NativeVerificationReport {
            incremental,
            supervision: Some(supervision),
            evidence: saved,
            adopted_tasks,
            adoption_issues,
            adoption_proposal,
        }))
    }
}

// The hold is content-addressed in the new run. The Store sees it in the same
// transaction before retention, independently of verdicts or task eligibility.
fn preserve_verification_history(
    config: &VerificationConfiguration,
    run: &mut VerificationRun,
    artifacts: &mut Vec<VerificationArtifact>,
) {
    if config.history_retention.is_some()
        && !artifacts.iter().any(|artifact| {
            artifact.reference.media_type == dctx0_store::HISTORY_PRESERVATION_MEDIA_TYPE
        })
    {
        let artifact = VerificationArtifact::new(
            dctx0_store::HISTORY_PRESERVATION_MEDIA_TYPE,
            br#"{"schema_version":1,"retention":"preserve-all"}"#.to_vec(),
        );
        run.artifacts.push(artifact.reference.clone());
        artifacts.push(artifact);
    }
}
