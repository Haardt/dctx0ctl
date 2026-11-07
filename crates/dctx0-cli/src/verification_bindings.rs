//! Read-only requirements before application implementation or discovery.
use clap::{Args, Subcommand};
use dctx0_engine::{Project, VerificationSelection};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(crate) struct BindingArgs {
    #[command(subcommand)]
    action: BindingAction,
}

#[derive(Subcommand)]
enum BindingAction {
    /// List exact native roles and the complete type closure from published requirements.
    Plan {
        input: PathBuf,
        /// Optional application configuration supplies scope, fixtures and budgets.
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = positive_task)]
        task: Vec<i64>,
        #[arg(long)]
        fixtures: Option<PathBuf>,
    },
    /// Generate editable native tests over the existing central assessment engine.
    Scaffold {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = positive_task)]
        task: Vec<i64>,
        #[arg(long)]
        fixtures: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        /// Save inert comparison files, never active source files.
        #[arg(long)]
        candidate: Option<String>,
    },
    /// Execute the pinned finite plan without saving evidence or adopting tasks.
    Test {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = positive_task)]
        task: Vec<i64>,
        #[arg(long)]
        expected_snapshot: String,
        #[arg(long)]
        expected_check: u64,
        #[arg(long)]
        expected_plan: String,
    },
}

fn positive_task(value: &str) -> Result<i64, String> {
    value
        .strip_prefix('T')
        .unwrap_or(value)
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| "Task ID must be a positive integer".into())
}

pub(crate) fn run(
    args: BindingArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let action = match args.action {
        BindingAction::Scaffold {
            input,
            project,
            target,
            subject,
            task,
            fixtures,
            dry_run,
            candidate,
        } => {
            return scaffold(
                input, project, target, subject, task, fixtures, dry_run, candidate, database,
                json, output,
            );
        }
        BindingAction::Test {
            input,
            project: application,
            target,
            subject,
            task,
            expected_snapshot,
            expected_check,
            expected_plan,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let (_, generator) = super::verification::execution_target(&target)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let report = runtime
                .block_on(project.test_verification(
                    &application,
                    VerificationSelection {
                        subjects: subject.into_iter().collect(),
                        tasks: task.into_iter().collect(),
                        ..Default::default()
                    },
                    generator,
                    &dctx0_engine::NativeCancellation::default(),
                    dctx0_engine::BindingTestPin {
                        snapshot: expected_snapshot,
                        check_occurrence: expected_check,
                        plan_hash: expected_plan,
                    },
                ))
                .map_err(crate::error_message)?;
            let passed = matches!(report.run.result, dctx0_core::VerificationResult::Pass);
            if json {
                crate::render::json(
                    output,
                    &serde_json::json!({
                        "published": false,
                        "run": {"result":report.run.result,"snapshot":report.run.provenance.snapshot,"check_occurrence":report.run.provenance.check_occurrence,"required_cases":report.run.required_cases.len(),"executed_cases":report.run.cases.len(),"open_obligations":report.run.open_obligations,"provenance_issues":report.run.provenance.incomplete},
                        "cases":report.run.cases.iter().map(|case|serde_json::json!({"id":case.id,"subject":case.subject,"result":case.result})).collect::<Vec<_>>()
                    }),
                )?;
            } else {
                writeln!(output,"Central binding test: {:?}; {} actual cases. Evidence was not published; no tasks adopted.",report.run.result,report.run.cases.len()).map_err(crate::error_message)?;
            }
            return Ok(passed);
        }
        action => action,
    };
    let BindingAction::Plan {
        input,
        project: application,
        subject,
        task,
        fixtures,
    } = action
    else {
        return Err("Expected binding plan".into());
    };
    let project = Project::new(&input, database).map_err(crate::error_message)?;
    let mut selection = VerificationSelection {
        subjects: subject.into_iter().collect(),
        tasks: task.into_iter().collect(),
        ..Default::default()
    };
    let mut budgets = Default::default();
    let mut fixture_path = fixtures;
    if let Some(application) = application {
        let (root, configuration) = project
            .verification_configuration(Some(&application))
            .map_err(crate::error_message)?;
        if selection.subjects.is_empty() && selection.tasks.is_empty() {
            selection.subjects = configuration.scope.subjects;
            selection.tasks = configuration.scope.tasks;
        }
        budgets = configuration.budgets;
        if fixture_path.is_none() {
            fixture_path = Some(root.join(configuration.fixtures));
        }
    }
    let fixtures = match fixture_path {
        Some(path) => dctx0_engine::read_review_input(&path, 524_288, "Binding plan fixtures")
            .map_err(crate::error_message)?,
        None => Default::default(),
    };
    let report = project
        .verification_binding_requirements(selection, &fixtures, budgets)
        .map_err(crate::error_message)?;
    if json {
        crate::render::json(output, &report)?;
    } else {
        writeln!(output, "Binding requirements for snapshot {} / check {}\n{} native roles; {} data declarations.\nPlanning does not confirm bindings or runtime readiness.\n", report.snapshot, report.check_occurrence, report.requirements.len(), report.declarations.len()).map_err(crate::error_message)?;
        for requirement in &report.requirements {
            writeln!(
                output,
                "- {:?}: {}{}",
                requirement.role,
                requirement.subject,
                requirement
                    .span
                    .as_ref()
                    .map(|s| format!(" ({}:{}:{})", s.file, s.start.line, s.start.column))
                    .unwrap_or_default()
            )
            .map_err(crate::error_message)?;
        }
        for issue in &report.issues {
            writeln!(output, "- {}: {}", issue.code, issue.message)
                .map_err(crate::error_message)?;
        }
    }
    Ok(report.issues.is_empty())
}

fn plan_hash(plan: &dctx0_verification::ProgramPlan) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(plan).map_err(crate::error_message)?)
    ))
}
fn relative_spec(application: &Path, input: &Path) -> Result<String, String> {
    let application = std::fs::canonicalize(application).map_err(crate::error_message)?;
    let input = std::fs::canonicalize(input).map_err(crate::error_message)?;
    let mut base = application.as_path();
    let mut prefix = PathBuf::new();
    loop {
        if let Ok(suffix) = input.strip_prefix(base) {
            prefix.push(suffix);
            return Ok(prefix.to_string_lossy().replace('\\', "/"));
        }
        base = base
            .parent()
            .ok_or("Spec and project must share a filesystem root")?;
        prefix.push("..");
    }
}
#[allow(clippy::too_many_arguments)]
fn scaffold(
    input: PathBuf,
    application: PathBuf,
    target: String,
    subjects: Vec<String>,
    tasks: Vec<i64>,
    fixtures: Option<PathBuf>,
    dry_run: bool,
    candidate: Option<String>,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let project = Project::new(&input, database).map_err(crate::error_message)?;
    let (root, config) = project
        .verification_configuration(Some(&application))
        .map_err(crate::error_message)?;
    if config.target != target {
        return Err("Scaffold target differs from the project verification target".into());
    }
    let mut selection = VerificationSelection {
        subjects: subjects.into_iter().collect(),
        tasks: tasks.into_iter().collect(),
        ..Default::default()
    };
    if selection.subjects.is_empty() && selection.tasks.is_empty() {
        selection.subjects = config.scope.subjects.clone();
        selection.tasks = config.scope.tasks.clone();
    }
    if fixtures.is_some() {
        return Err("Use the project's configured fixture path so generated tests and central execution select identical inputs".into());
    }
    let fixtures = dctx0_engine::read_review_input(
        &root.join(&config.fixtures),
        dctx0_engine::DEFAULT_REVIEW_INPUT_BYTES,
        "Scaffold fixtures",
    )
    .map_err(crate::error_message)?;
    let plan = project
        .verification_plan(selection.clone(), &fixtures, config.budgets.clone())
        .map_err(crate::error_message)?;
    let requirements = project
        .verification_binding_requirements(selection.clone(), &fixtures, config.budgets)
        .map_err(crate::error_message)?;
    let mut arguments = vec![
        "verify".into(),
        "bindings".into(),
        "test".into(),
        relative_spec(&application, &input)?,
        "--project".into(),
        ".".into(),
        "--target".into(),
        target.clone(),
        "--expected-snapshot".into(),
        plan.snapshot.clone(),
        "--expected-check".into(),
        plan.check_occurrence.to_string(),
        "--expected-plan".into(),
        plan_hash(&plan)?,
        "--json".into(),
    ];
    for subject in &plan.scope.subjects {
        arguments.extend(["--subject".into(), subject.clone()]);
    }
    for task in &plan.scope.tasks {
        arguments.extend(["--task".into(), task.clone()]);
    }
    let options = dctx0_core::CodecOptions::default();
    let (codec, version) = crate::codec_scaffold::generate(
        &target,
        &requirements.declarations,
        &options,
        &plan.snapshot,
    )?;
    let mut artifacts = test_artifacts(&target, &requirements, &arguments)?;
    artifacts.push(codec);
    let inputs = dctx0_engine::CodecInputs {
        snapshot: plan.snapshot,
        declarations: requirements.declarations,
    };
    let mut reports = Vec::new();
    // Validate every filename before publishing any member of the bundle.
    for artifact in &artifacts {
        let filename = match &candidate {
            Some(name) => format!("{name}.{}.txt", artifact.path),
            None => artifact.path.clone(),
        };
        reports.push(
            dctx0_engine::publish_target_codec_scaffold(
                &application,
                &target,
                &filename,
                artifact.clone(),
                &inputs,
                options.clone(),
                version,
                true,
            )
            .map_err(crate::error_message)?,
        );
    }
    if !dry_run {
        for report in &mut reports {
            *report = dctx0_engine::publish_target_codec_scaffold(
                &application,
                &target,
                &report.artifact.path,
                report.artifact.clone(),
                &inputs,
                options.clone(),
                version,
                false,
            )
            .map_err(crate::error_message)?;
        }
    }
    if json {
        crate::render::json(output, &reports)?;
    } else {
        for report in reports {
            writeln!(output, "{} ({})", report.path, report.status)
                .map_err(crate::error_message)?;
            if dry_run {
                writeln!(output, "{}", report.artifact.contents).map_err(crate::error_message)?;
            }
        }
    }
    Ok(true)
}

fn test_artifacts(
    target: &str,
    requirements: &dctx0_core::BindingRequirements,
    arguments: &[String],
) -> Result<Vec<dctx0_target_sdk::Artifact>, String> {
    let _ = (requirements, arguments);
    match target {
        #[cfg(feature = "go")]
        "go" => Ok(dctx0_target_go::generate_test_scaffold(
            requirements,
            arguments,
        )),
        #[cfg(feature = "rust")]
        "rust" => Ok(dctx0_target_rust::generate_test_scaffold(
            requirements,
            arguments,
        )),
        #[cfg(feature = "typescript")]
        "typescript" => Ok(dctx0_target_typescript::generate_test_scaffold(
            requirements,
            arguments,
        )),
        _ => Err(format!("No test scaffold generator enabled for {target}")),
    }
}
