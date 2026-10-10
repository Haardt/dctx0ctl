//! Managed native execution from a published snapshot, with immutable reports.
mod capture;
mod execution;
mod pins;
mod replay;
use crate::*;
use capture::Capture;
use dctx0_core::*;
use dctx0_store::{StoredVerificationRun, VerificationArtifact};
use dctx0_target_sdk::{
    ExecutionRecipe, HarnessGenerator, HarnessRequest, SDK_VERSION, ValueCheck, WorkerReply,
    WorkerRequest,
};
use dctx0_verification::{LifecycleCaseInput, ProgramPlan, ProjectionFixtures};
pub use replay::{ReplayChange, VerificationReplayReport};
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
    pub evidence: StoredVerificationRun,
    pub adopted_tasks: Vec<i64>,
    pub adoption_issues: Vec<VerificationIssue>,
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
        mut selection: VerificationSelection,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
        pinned: Option<(ProgramPlan, VerificationLifecycleFixtures)>,
    ) -> Result<NativeVerificationReport> {
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
        let (plan, pinned_lifecycles) = match pinned {
            Some((plan, lifecycles)) => (plan, Some(lifecycles)),
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
        let bundle = generator
            .generate(&HarnessRequest {
                adapter_import: format!("./{adapter_name}"),
                subjects: plan.closure.clone(),
            })
            .map_err(|e| Error::Invalid(e.to_string()))?;
        if bundle.sdk_version != SDK_VERSION
            || bundle.target != config.target
            || bundle.recipe.program != config.runtime
        {
            return Err(Error::Invalid(
                "Configured target/runtime does not match the generated harness".into(),
            ));
        }
        let mut artifacts = vec![VerificationArtifact::new(
            "application/vnd.dctx0.plan+json",
            serde_json::to_vec(&plan)?,
        )];
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
                "text/typescript",
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
        let lifecycle_path = capture.root.join(directory).join("lifecycles.json");
        let lifecycles: VerificationLifecycleFixtures = if let Some(lifecycles) = pinned_lifecycles
        {
            lifecycles
        } else if lifecycle_path.try_exists()? {
            serde_json::from_slice(&fs::read(lifecycle_path)?)?
        } else {
            Default::default()
        };
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
            protocol_version: SDK_VERSION,
            target: bundle.target.clone(),
            runtime_version: String::new(),
            build_configuration: capture
                .files
                .iter()
                .filter(|(path, _)| {
                    matches!(
                        path.as_str(),
                        "package.json" | "bun.lock" | "bun.lockb" | "tsconfig.json"
                    ) || path.starts_with("tsconfig.") && path.ends_with(".json")
                })
                .map(|(path, hash)| (path.clone(), hash.clone()))
                .collect(),
            task_pin,
            incomplete: {
                pin_issues.push(issue(
                "ADOPTION_PROVENANCE_PENDING",
                "Native evidence still requires dependency closure and guarded publication before adoption",
                ));
                pin_issues
            },
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
        execution::execute(
            execution::Input {
                ir: &snapshot.ir,
                plan: &plan,
                lifecycles: &lifecycles,
                cancel: cancellation,
            },
            &bundle.recipe,
            &capture.root.join(directory),
            &mut run,
            &mut artifacts,
        )
        .await?;
        run.elapsed_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        run.artifacts = artifacts.iter().map(|a| a.reference.clone()).collect();
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
        let saved = self.store(AccessMode::ReadWrite)?.record_verification_run(
            &self.key(),
            &run,
            &artifacts,
        )?;
        Ok(NativeVerificationReport {
            evidence: saved,
            adopted_tasks: vec![],
            adoption_issues: if no_adopt {
                vec![]
            } else {
                vec![issue(
                    "ADOPTION_UNAVAILABLE",
                    "Run evidence is stored; automatic guarded adoption is not yet implemented",
                )]
            },
        })
    }
}
