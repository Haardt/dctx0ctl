//! Verification setup and read-only planning; native results remain centrally assessed.
use clap::{Args, Subcommand};
use dctx0_engine::{Project, VerificationSelection};
use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
};
fn outcome_summary(
    output: &mut dyn Write,
    run: &dctx0_core::VerificationRun,
) -> Result<(), String> {
    use dctx0_core::VerificationResult;
    let mut groups = std::collections::BTreeMap::new();
    let mut subjects = std::collections::BTreeMap::new();
    let (mut pass, mut fail, mut unknown) = (0, 0, 0);
    for case in &run.cases {
        let counts = subjects.entry(case.subject.as_str()).or_insert([0usize; 3]);
        match &case.result {
            VerificationResult::Pass => {
                pass += 1;
                counts[0] += 1;
            }
            VerificationResult::Fail { issue } => {
                fail += 1;
                counts[1] += 1;
                *groups
                    .entry(("FAIL", issue.code.as_str()))
                    .or_insert(0usize) += 1;
            }
            VerificationResult::Unknown { issue } => {
                unknown += 1;
                counts[2] += 1;
                *groups
                    .entry(("UNKNOWN", issue.code.as_str()))
                    .or_insert(0usize) += 1;
            }
        }
    }
    writeln!(output, "Recorded outcomes: {pass} PASS, {fail} FAIL, {unknown} UNKNOWN.\n\n{} required subjects; {} subjects outside this scope. Seed: {}; generated-case budget: {} per obligation.\n", run.required_subjects.len(), run.not_selected.len(), run.budgets.seed, run.budgets.generated_cases).map_err(crate::error_message)?;
    let (mut returned, mut threw, mut unavailable) = (0usize, 0usize, 0usize);
    for observation in run.cases.iter().flat_map(|case| &case.observations) {
        match observation.observation {
            dctx0_core::Observation::Returned { .. } => returned += 1,
            dctx0_core::Observation::Threw { .. } => threw += 1,
            dctx0_core::Observation::Unavailable { .. } => unavailable += 1,
        }
    }
    let without_invocations = run
        .cases
        .iter()
        .filter(|case| case.observations.is_empty())
        .count();
    writeln!(output, "Recorded invocation observations: {returned} returned, {threw} exceptions, {unavailable} unavailable. Cases without invocation observations: {without_invocations}. Value checks and central assessments have separate evidence.\n").map_err(crate::error_message)?;
    let (mut recorded_phases, mut unrecorded_phases) = (0usize, 0usize);
    for state in run.cases.iter().flat_map(|case| &case.states) {
        if state.phase_recorded == Some(true) {
            recorded_phases += 1;
        } else {
            unrecorded_phases += 1;
        }
    }
    if recorded_phases + unrecorded_phases > 0 {
        writeln!(output,"State phase provenance: {recorded_phases} recorded at observation, {unrecorded_phases} without recorded timing. Older inferred labels do not establish the preparation/transition boundary.\n").map_err(crate::error_message)?;
    }
    if !groups.is_empty() {
        writeln!(output, "| Outcome | Reason | Cases |\n| --- | --- | ---: |")
            .map_err(crate::error_message)?;
        for ((outcome, code), count) in groups {
            writeln!(
                output,
                "| {outcome} | `{}` | {count} |",
                code.replace('|', "\\|").replace(['\n', '\r', '`'], " ")
            )
            .map_err(crate::error_message)?;
        }
    }
    if !subjects.is_empty() {
        writeln!(
            output,
            "\n| Subject | PASS cases | FAIL cases | UNKNOWN cases |\n| --- | ---: | ---: | ---: |"
        )
        .map_err(crate::error_message)?;
        for (subject, [passed, failed, unknown]) in subjects {
            writeln!(
                output,
                "| `{}` | {passed} | {failed} | {unknown} |",
                subject.replace('|', "\\|").replace(['\n', '\r', '`'], " ")
            )
            .map_err(crate::error_message)?;
        }
        writeln!(output, "\nCase counts do not establish complete subject verification; mandatory obligations and provenance also apply.\n")
            .map_err(crate::error_message)?;
    }
    writeln!(
        output,
        "\nIncomplete provenance: {}.\n",
        run.provenance.incomplete.len()
    )
    .map_err(crate::error_message)?;
    for issue in &run.provenance.incomplete {
        writeln!(output, "- {}: {}", issue.code, issue.message).map_err(crate::error_message)?;
    }
    Ok(())
}

#[derive(Args)]
pub(crate) struct VerifyArgs {
    #[command(subcommand)]
    action: VerifyAction,
}
#[derive(Subcommand)]
enum VerifyAction {
    /// Check, execute, apply authored source inspections and require zero unfinished tasks.
    Complete {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        /// Authored source-bound inspection notes; defaults to the testing adapter directory.
        #[arg(long)]
        inspections: Option<PathBuf>,
    },
    /// Store effective suite/review/Release evidence; no native execution or adoption.
    Inspect {
        input: PathBuf,
        run_id: String,
        #[arg(long)]
        project: Option<PathBuf>,
        /// JSON array of bound ProgramInspection reports.
        #[arg(long)]
        file: PathBuf,
    },
    /// Explicitly deliver inspected agent reports; preserves automatic evidence.
    Attest {
        input: PathBuf,
        run_id: String,
        #[arg(long)]
        project: Option<PathBuf>,
        /// JSON array of TaskAttestation reports with exact source bindings.
        #[arg(long)]
        file: PathBuf,
    },
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
        /// Replay saved minimized operation or lifecycle inputs.
        #[arg(long)]
        minimized: bool,
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
        VerifyAction::Complete {
            input,
            project: application,
            inspections,
        } => {
            #[cfg(feature = "typescript")]
            {
                if let Some(pass) = super::verification_complete::memory_guard()? {
                    return Ok(pass);
                }
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                let notes = inspections.unwrap_or_else(|| {
                    application.join(".dctx0/testing/typescript/inspection-notes.json")
                });
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error_message)?;
                eprintln!(
                    "Complete verification: checking sources, executing configured native cases, then binding authored inspections."
                );
                let report = runtime
                    .block_on(project.complete_verification(
                        &application,
                        &notes,
                        &dctx0_target_typescript::Provider,
                        &dctx0_target_typescript::BunHarness,
                        &dctx0_engine::NativeCancellation::default(),
                    ))
                    .map_err(crate::error_message)?;
                if json {
                    crate::render::json(output, &report)?;
                } else {
                    writeln!(output, "# Complete verification\n\nResult: PASS\nMandatory native cases: {}\nTasks: {}; unfinished: {}; without PASS: {}\nNative result: {:?}\nNative run: `{}`\nEffective run: `{}`\nBasis: {}", report.mandatory_cases, report.tasks, report.open_tasks, report.non_pass_tasks, report.native_result, report.native_run, report.inspected_run, report.basis).map_err(crate::error_message)?;
                }
                return Ok(true);
            }
            #[cfg(not(feature = "typescript"))]
            {
                let _ = (input, application, inspections);
                return Err("No executable TypeScript testing target is enabled".into());
            }
        }
        VerifyAction::Inspect {
            input,
            run_id,
            project: application,
            file,
        } => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(file)
                .map_err(crate::error_message)?
                .take(1_048_577)
                .read_to_end(&mut bytes)
                .map_err(crate::error_message)?;
            if bytes.len() > 1_048_576 {
                return Err("Inspection input exceeds 1 MiB".into());
            }
            let inspections: Vec<dctx0_verification::program_inspection::ProgramInspection> =
                serde_json::from_slice(&bytes).map_err(crate::error_message)?;
            let report = Project::new(&input, database)
                .map_err(crate::error_message)?
                .publish_program_inspections(application.as_deref(), &run_id, &inspections)
                .map_err(crate::error_message)?;
            let pass = report.assessment.result == dctx0_core::VerificationResult::Pass;
            if json {
                crate::render::json(output, &report)?;
            } else {
                writeln!(output,"# Effective verification\n\nNative source: `{run_id}`\nInspected report: `{}`\nResult: {:?}\nOpen obligations: {}\nAutomatic and agent results are retained separately. No new execution or task adoption.",report.evidence.id,report.assessment.result,report.assessment.open_obligations.len()).map_err(crate::error_message)?;
            }
            return Ok(pass);
        }
        VerifyAction::Attest {
            input,
            run_id,
            project: application,
            file,
        } => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(&file)
                .map_err(crate::error_message)?
                .take(1_048_577)
                .read_to_end(&mut bytes)
                .map_err(crate::error_message)?;
            if bytes.len() > 1_048_576 {
                return Err("Attestation input exceeds 1 MiB".into());
            }
            let reports: Vec<dctx0_engine::TaskAttestation> =
                serde_json::from_slice(&bytes).map_err(crate::error_message)?;
            let receipt = Project::new(&input, database)
                .map_err(crate::error_message)?
                .publish_task_attestations(application.as_deref(), &run_id, &reports)
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &receipt)?;
            } else {
                writeln!(output, "# Agent inspection delivery\n\n{} tasks delivered with agent evidence. Native run `{run_id}` remains unchanged. Event sequence: {}.", receipt.deliveries.len(), receipt.event_sequence).map_err(crate::error_message)?;
            }
            return Ok(true);
        }
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
            minimized,
            project: application,
        } => {
            #[cfg(feature = "typescript")]
            {
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error_message)?;
                let cancel = dctx0_engine::NativeCancellation::default();
                let report = runtime
                    .block_on(async {
                        if minimized {
                            project
                                .replay_minimized_verification(
                                    application.as_deref(),
                                    &run_id,
                                    &case,
                                    &dctx0_target_typescript::BunHarness,
                                    &cancel,
                                )
                                .await
                        } else {
                            project
                                .replay_verification(
                                    application.as_deref(),
                                    &run_id,
                                    &case,
                                    &dctx0_target_typescript::BunHarness,
                                    &cancel,
                                )
                                .await
                        }
                    })
                    .map_err(crate::error_message)?;
                let pass = matches!(
                    report.case_result,
                    Some(dctx0_core::VerificationResult::Pass)
                );
                if json {
                    crate::render::json(output, &report)?;
                } else {
                    writeln!(output,"# Verification replay\n\nOriginal run: `{}`; case: `{}`\nNew run: `{}`\nMinimized input: {}\nSelected case result: {:?}\n{} provenance changes. This is a new case comparison; no task adoption or whole-application claim.",report.original_run,report.original_case,report.evidence.evidence.id,report.minimized_input,report.case_result,report.changes.len()).map_err(crate::error_message)?;
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
                let _ = (input, run_id, case, minimized, application);
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
                    outcome_summary(output, &report.evidence.run)?;
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
                outcome_summary(output, &saved.run)?;
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
        .as_ref()
        .map(|(_, config)| config.budgets.clone())
        .unwrap_or_default();
    let mut plan = project
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
    if let Some((root, config)) = &configured {
        plan.issues.extend(
            project
                .verification_lifecycle_input_issues(root, &config.adapter, &plan)
                .map_err(crate::error_message)?,
        );
    }
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
        let explanatory = plan
            .manual_checks
            .iter()
            .filter(|check| check.kind == dctx0_core::StatementKind::Intent)
            .count();
        let mandatory = plan.manual_checks.len() - explanatory;
        writeln!(output,"\n{} subjects outside the selected closure.\n{} manual requirements, {} explanatory intents and {} suites retained in the JSON plan.",plan.not_selected.len(),mandatory,explanatory,plan.suites.len()).map_err(crate::error_message)?;
    }
    Ok(valid)
}
