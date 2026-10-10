//! Verification setup and read-only planning; native results remain centrally assessed.
use clap::{Args, Subcommand};
use dctx0_engine::{Project, VerificationSelection};
use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Args)]
pub(crate) struct VerifyArgs {
    #[command(subcommand)]
    action: VerifyAction,
}
#[derive(Subcommand)]
enum VerifyAction {
    /// Inspect TypeScript/TSX syntax; never execute or adopt candidates.
    Discover {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
    },
    /// Execute saved case inputs as a new comparison; never adopt tasks.
    Replay {
        input: PathBuf,
        run_id: String,
        #[arg(long)]
        case: String,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Execute the selected published plan and store actual observations.
    Run {
        input: PathBuf,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = task_id)]
        task: Vec<i64>,
        #[arg(long)]
        historical_check: Option<i64>,
        #[arg(long)]
        no_adopt: bool,
    },
    /// Read immutable saved evidence without claiming working-tree freshness.
    Show { input: PathBuf, run_id: String },
    /// Preserve existing testing files and create an unavailable adapter scaffold.
    Init {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = task_id)]
        task: Vec<i64>,
    },
    /// Project published requirements without executing the application.
    Plan {
        input: PathBuf,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = task_id)]
        task: Vec<i64>,
        #[arg(long)]
        historical_check: Option<i64>,
        /// Reviewed symbolic inputs in ProjectionFixtures JSON format.
        #[arg(long)]
        fixtures: Option<PathBuf>,
    },
}
fn task_id(text: &str) -> Result<i64, String> {
    let id = text
        .strip_prefix('T')
        .unwrap_or(text)
        .parse::<i64>()
        .map_err(|_| "Task must be a positive ID such as T31".to_string())?;
    if id <= 0 {
        return Err("Task must be a positive ID such as T31".into());
    }
    Ok(id)
}
pub(crate) fn run(
    args: VerifyArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let (input, application, subject, task, historical_check, fixtures) = match args.action {
        VerifyAction::Discover {
            input,
            project: application,
        } => {
            #[cfg(feature = "typescript")]
            {
                let report = Project::new(&input, database)
                    .map_err(crate::error_message)?
                    .discover_verification(
                        &application,
                        &dctx0_target_typescript::TypeScriptInspector,
                    )
                    .map_err(crate::error_message)?;
                if json {
                    crate::render::json(output, &report)?;
                } else {
                    writeln!(output, "# Structural discovery\n\nSnapshot: `{}`; check occurrence: {}\n{} source files. No execution or adoption.\n", report.snapshot, report.check_occurrence, report.inspection.sources.len()).map_err(crate::error_message)?;
                    for subject in &report.subjects {
                        if !subject.candidates.is_empty() {
                            writeln!(
                                output,
                                "- `{}`: {} unconfirmed candidates{}",
                                subject.subject,
                                subject.candidates.len(),
                                if subject.ambiguous {
                                    " (ambiguous)"
                                } else {
                                    ""
                                }
                            )
                            .map_err(crate::error_message)?;
                            for candidate in &subject.candidates {
                                writeln!(
                                    output,
                                    "  - `{}`: `{}`",
                                    candidate.path, candidate.declaration
                                )
                                .map_err(crate::error_message)?;
                            }
                        }
                    }
                    for limitation in &report.limitations {
                        writeln!(output, "\n{limitation}").map_err(crate::error_message)?;
                    }
                    for source in report.inspection.sources.iter().filter(|s| s.syntax_errors) {
                        writeln!(
                            output,
                            "\nSyntax errors in `{}`; excluded from suggestions.",
                            source.path
                        )
                        .map_err(crate::error_message)?;
                    }
                }
                return Ok(true);
            }
            #[cfg(not(feature = "typescript"))]
            {
                let _ = (input, application);
                return Err("No TypeScript structural inspector is enabled".into());
            }
        }
        VerifyAction::Replay {
            input,
            run_id,
            case,
            project: application,
        } => {
            #[cfg(feature = "typescript")]
            {
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error_message)?;
                let report = runtime
                    .block_on(project.replay_verification(
                        application.as_deref(),
                        &run_id,
                        &case,
                        &dctx0_target_typescript::BunHarness,
                        &dctx0_engine::NativeCancellation::default(),
                    ))
                    .map_err(crate::error_message)?;
                let pass = matches!(
                    report.case_result,
                    Some(dctx0_core::VerificationResult::Pass)
                );
                if json {
                    crate::render::json(output, &report)?;
                } else {
                    writeln!(output,"# Verification replay\n\nOriginal run: `{}`; case: `{}`\nNew run: `{}`\nSelected case result: {:?}\n{} provenance changes. This is a new case comparison; no task adoption or whole-application claim.",report.original_run,report.original_case,report.evidence.evidence.id,report.case_result,report.changes.len()).map_err(crate::error_message)?;
                    for change in &report.changes {
                        writeln!(
                            output,
                            "- {}: {:?} → {:?}",
                            change.component, change.before, change.after
                        )
                        .map_err(crate::error_message)?;
                    }
                }
                return Ok(pass);
            }
            #[cfg(not(feature = "typescript"))]
            {
                let _ = (input, run_id, case, application);
                return Err("No executable TypeScript testing target is enabled".into());
            }
        }
        VerifyAction::Run {
            input,
            project: application,
            subject,
            task,
            historical_check,
            no_adopt,
        } => {
            #[cfg(feature = "typescript")]
            {
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error_message)?;
                let report = runtime
                    .block_on(project.run_verification(
                        application.as_deref(),
                        VerificationSelection {
                            subjects: BTreeSet::from_iter(subject),
                            tasks: BTreeSet::from_iter(task),
                            historical_check,
                        },
                        &dctx0_target_typescript::BunHarness,
                        &dctx0_engine::NativeCancellation::default(),
                        no_adopt,
                    ))
                    .map_err(crate::error_message)?;
                let pass = matches!(
                    report.evidence.run.result,
                    dctx0_core::VerificationResult::Pass
                );
                if json {
                    crate::render::json(output, &report)?;
                } else {
                    writeln!(output,"# Native verification\n\nRun: `{}`\nSnapshot: `{}`; check occurrence: {}\nResult: {:?}\n\n{} mandatory cases; {} recorded cases; {} open obligations.\n",report.evidence.id,report.evidence.run.provenance.snapshot,report.evidence.run.provenance.check_occurrence,report.evidence.run.result,report.evidence.run.required_cases.len(),report.evidence.run.cases.len(),report.evidence.run.open_obligations.len()).map_err(crate::error_message)?;
                    for issue in report
                        .evidence
                        .run
                        .open_obligations
                        .iter()
                        .chain(&report.adoption_issues)
                    {
                        writeln!(output, "- {}: {}", issue.code, issue.message)
                            .map_err(crate::error_message)?;
                    }
                }
                return Ok(pass);
            }
            #[cfg(not(feature = "typescript"))]
            {
                let _ = (
                    input,
                    application,
                    subject,
                    task,
                    historical_check,
                    no_adopt,
                );
                return Err("No executable TypeScript testing target is enabled".into());
            }
        }
        VerifyAction::Show { input, run_id } => {
            let saved = Project::new(&input, database)
                .map_err(crate::error_message)?
                .saved_verification_run(&run_id)
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &saved)?;
            } else {
                writeln!(output,"# Stored verification evidence\n\nRun: `{}`\nStored: {}\nSnapshot: `{}`; check occurrence: {}\nResult: {:?}\n\nSaved evidence only; no working-tree freshness or task-adoption claim.\n",saved.id,saved.created_at,saved.run.provenance.snapshot,saved.run.provenance.check_occurrence,saved.run.result).map_err(crate::error_message)?;
                writeln!(output,"{} selected subjects; {} mandatory cases; {} recorded cases; {} open obligations.\n",saved.run.selected_subjects.len(),saved.run.required_cases.len(),saved.run.cases.len(),saved.run.open_obligations.len()).map_err(crate::error_message)?;
                for case in &saved.run.cases {
                    writeln!(
                        output,
                        "- `{}` / `{}`: {:?}",
                        case.id, case.subject, case.result
                    )
                    .map_err(crate::error_message)?;
                }
                for issue in &saved.run.open_obligations {
                    writeln!(output, "- {}: {}", issue.code, issue.message)
                        .map_err(crate::error_message)?;
                }
            }
            return Ok(true);
        }

        VerifyAction::Init {
            input,
            project,
            target,
            subject,
            task,
        } => {
            #[cfg(feature = "typescript")]
            if target == "typescript" {
                let report = Project::new(&input, database)
                    .map_err(crate::error_message)?
                    .initialize_verification(
                        &project,
                        &dctx0_target_typescript::BunHarness,
                        dctx0_engine::ConfiguredVerificationScope {
                            subjects: BTreeSet::from_iter(subject),
                            tasks: BTreeSet::from_iter(task),
                        },
                    )
                    .map_err(crate::error_message)?;
                if json {
                    crate::render::json(output, &report)?;
                } else {
                    writeln!(output,"# Verification setup\n\nProject: `{}`\n\n{} created files; {} preserved files.\nConfigured scope: {} subjects, {} tasks.\nNo application execution or task adoption. Implement and review the testing adapter before running verification.",report.project.display(),report.created.len(),report.preserved.len(),report.configuration.scope.subjects.len(),report.configuration.scope.tasks.len()).map_err(crate::error_message)?;
                }
                return Ok(true);
            }
            let _ = (input, project, subject, task);
            return Err(format!(
                "No executable testing harness is registered for target {target}; profiles alone do not provide native verification"
            ));
        }
        VerifyAction::Plan {
            input,
            project,
            subject,
            task,
            historical_check,
            fixtures,
        } => (input, project, subject, task, historical_check, fixtures),
    };
    let project = Project::new(&input, database).map_err(crate::error_message)?;
    let configured = project
        .optional_verification_configuration(application.as_deref())
        .map_err(crate::error_message)?;
    let (subject, task) = if subject.is_empty() && task.is_empty() {
        configured
            .as_ref()
            .map(|(_, config)| (config.scope.subjects.clone(), config.scope.tasks.clone()))
            .unwrap_or_default()
    } else {
        (BTreeSet::from_iter(subject), BTreeSet::from_iter(task))
    };
    let fixtures = match fixtures {
        Some(path) => serde_json::from_slice(&std::fs::read(path).map_err(crate::error_message)?)
            .map_err(crate::error_message)?,
        None => match &configured {
            Some((root, config)) => serde_json::from_slice(
                &std::fs::read(root.join(&config.fixtures)).map_err(crate::error_message)?,
            )
            .map_err(crate::error_message)?,
            None => Default::default(),
        },
    };
    let budgets = configured
        .map(|(_, config)| config.budgets)
        .unwrap_or_default();
    let plan = project
        .verification_plan(
            VerificationSelection {
                subjects: subject,
                tasks: task,
                historical_check,
            },
            &fixtures,
            budgets,
        )
        .map_err(crate::error_message)?;
    let issues = plan
        .issues
        .iter()
        .chain(plan.contracts.iter().flat_map(|c| &c.corpus.issues))
        .chain(
            plan.contracts
                .iter()
                .flat_map(|c| &c.rejection_corpus.issues),
        )
        .chain(plan.operations.iter().flat_map(|o| &o.inputs.issues))
        .chain(
            plan.properties
                .iter()
                .flat_map(|p| p.assertions.iter().flat_map(|a| &a.issues)),
        )
        .chain(plan.examples.iter().flat_map(|e| &e.issues))
        .chain(plan.lifecycles.iter().flat_map(|l| &l.issues))
        .collect::<Vec<_>>();
    let valid = plan.issues.is_empty();
    if json {
        crate::render::json(output, &plan)?;
    } else {
        writeln!(output,"# Verification plan\n\nSnapshot: `{}`; check occurrence: {}\n\nPlanning only; no execution or task adoption.\n",plan.snapshot,plan.check_occurrence).map_err(crate::error_message)?;
        writeln!(
            output,
            "{} contracts, {} operations, {} examples, {} properties, {} lifecycles.\n",
            plan.contracts.len(),
            plan.operations.len(),
            plan.examples.len(),
            plan.properties.len(),
            plan.lifecycles.len()
        )
        .map_err(crate::error_message)?;
        for subject in &plan.closure {
            writeln!(output, "- `{subject}`").map_err(crate::error_message)?;
        }
        writeln!(
            output,
            "\nRequired capabilities: {:?}\n\nOpen execution points: {}",
            plan.capabilities,
            issues.len()
        )
        .map_err(crate::error_message)?;
        for issue in issues {
            writeln!(output, "- {}: {}", issue.code, issue.message)
                .map_err(crate::error_message)?;
        }
        writeln!(output,"\n{} subjects outside the selected closure.\n{} manual requirements and {} suites retained in the JSON plan.",plan.not_selected.len(),plan.manual_checks.len(),plan.suites.len()).map_err(crate::error_message)?;
    }
    Ok(valid)
}
