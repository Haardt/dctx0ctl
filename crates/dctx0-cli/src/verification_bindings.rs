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
    let BindingAction::Plan {
        input,
        project: application,
        subject,
        task,
        fixtures,
    } = args.action;
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
