//! Managed native execution from a published snapshot, with immutable reports.
mod adoption;
mod adoption_preview;
pub use adoption_preview::{AdoptionPreviewTask, VerificationAdoptionPreview};
mod attestations;
mod build;
mod capture;
mod dependencies;
mod execution;
mod pins;
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
    pub supervision: Option<crate::NativeSupervisionCapabilities>,
    pub evidence: StoredVerificationRun,
    pub adopted_tasks: Vec<i64>,
    pub adoption_issues: Vec<VerificationIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adoption_proposal: Option<dctx0_tasks::AdoptionProposal>,
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
        mut selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
        pinned: Option<(ProgramPlan, VerificationLifecycleFixtures, Option<String>)>,
        limits: NativeLimits,
    ) -> Result<NativeVerificationReport> {
        let parent = pinned.as_ref().and_then(|(_, _, parent)| parent.clone());
        let supervision = limits.supervision_capabilities();
        let historical = selection.historical_check.is_some() || pinned.is_some();
        let no_adopt = no_adopt || historical;
        let (application, config) = self.verification_configuration(application)?;
        if selection.subjects.is_empty() && selection.tasks.is_empty() {
            selection.subjects = config.scope.subjects.clone();
            selection.tasks = config.scope.tasks.clone();
        }
        let mut capture = Capture::new(&application)?;
        let captured_inputs = capture.files.clone();
        let fixtures: ProjectionFixtures = if pinned.is_none() {
            serde_json::from_slice(&fs::read(capture.root.join(&config.fixtures))?)?
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
        let (task_pin, mut pin_issues) =
            pins::capture(self, &application, &config.target, &plan, &snapshot)?;
        let (directory, adapter_name) = config
            .adapter
            .rsplit_once('/')
            .unwrap_or(("", &config.adapter));
        let directory = Path::new(directory);
        let lifecycle_path = capture.root.join(directory).join("lifecycles.json");
        if pinned_lifecycles.is_none() {
            plan.issues.extend(self.verification_lifecycle_input_issues(
                &capture.root,
                &config.adapter,
                &plan,
            )?);
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
                rust_build::prepare(&capture.root.join(directory), &capture.root, cancellation)
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
                                capture
                                    .root
                                    .join(directory)
                                    .join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX)),
                                &executable,
                            )?;
                        }
                        bundle.recipe.program = executable.to_string_lossy().into_owned();
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
                                capture
                                    .root
                                    .join(directory)
                                    .join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX)),
                                &executable,
                            )?;
                        }
                        bundle.recipe.program = executable.to_string_lossy().into_owned();
                        go_receipt = Some(receipt);
                    }
                    Err(error) => {
                        pin_issues.push(issue("GO_BUILD_INPUTS_UNAVAILABLE", error.to_string()));
                        build_ready = false;
                    }
                }
            }
        }
        let execution_inputs = Capture::inventory(&capture.root)?;
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
            implementation: capture
                .files
                .iter()
                .filter(|(p, _)| !p.starts_with("node_modules/"))
                .map(|(p, h)| (p.clone(), h.clone()))
                .collect(),
            dependencies: capture
                .files
                .iter()
                .filter(|(p, _)| p.starts_with("node_modules/"))
                .map(|(p, h)| (p.clone(), h.clone()))
                .collect(),
            adapter_hash: capture.files[&config.adapter].clone(),
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
            cases: vec![],
            open_obligations: plan.issues.clone(),
            bounded_obligations: vec![],
            artifacts: vec![],
            result: VerificationResult::Pass,
        };
        if !build_ready {
            let manifest = dctx0_verification::case_manifest(&plan);
            run.required_cases = manifest.required_cases;
            run.open_obligations.extend(manifest.open_obligations);
            run.bounded_obligations.extend(manifest.bounded_obligations);
        }
        if build_ready {
            execution::execute(
                execution::Input {
                    ir: &snapshot.ir,
                    root: &capture.root,
                    adapter: &config.adapter,
                    generated: &generated,
                    plan: &plan,
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
        match Capture::inventory(&capture.root) {
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
        let _lock = self.lock()?;
        match Capture::inventory(&application) {
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
        if !historical {
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
        let saved = self
            .store(AccessMode::ReadWrite)?
            .record_verification_run_owned_from(&self.key(), run, &artifacts, parent.as_deref())?;
        let (adopted_tasks, adoption_issues, adoption_proposal) = if no_adopt {
            (vec![], vec![], None)
        } else {
            adoption::publish(self, &application, &plan, &saved, &artifacts)?
        };
        Ok(NativeVerificationReport {
            supervision: Some(supervision),
            evidence: saved,
            adopted_tasks,
            adoption_issues,
            adoption_proposal,
        })
    }
}
