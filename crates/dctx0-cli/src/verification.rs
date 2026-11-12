//! Verification setup and read-only planning; native results remain centrally assessed.
use clap::{Args, Subcommand};
use dctx0_engine::{DEFAULT_REVIEW_INPUT_BYTES, Project, VerificationSelection, read_review_input};
use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
};
pub(crate) fn execution_target(
    target: &str,
) -> Result<
    (
        &'static dyn dctx0_core::TargetProfileProvider,
        &'static dyn dctx0_target_sdk::HarnessGenerator,
    ),
    String,
> {
    match target {
        #[cfg(feature = "typescript")]
        "typescript" => Ok((
            &dctx0_target_typescript::Provider,
            &dctx0_target_typescript::BunHarness,
        )),
        #[cfg(feature = "go")]
        "go" => Ok((&dctx0_target_go::Provider, &dctx0_target_go::GoHarness)),
        #[cfg(feature = "rust")]
        "rust" => Ok((
            &dctx0_target_rust::Provider,
            &dctx0_target_rust::RustHarness,
        )),
        _ => Err(format!(
            "No executable testing target is enabled for {target}"
        )),
    }
}
#[cfg(any(feature = "typescript", feature = "go", feature = "rust"))]
fn configured_target(project: &Project, application: Option<&Path>) -> Result<String, String> {
    project
        .verification_configuration(application)
        .map(|(_, config)| config.target)
        .map_err(crate::error_message)
}
fn outcome_summary(
    output: &mut dyn Write,
    run: &dctx0_core::VerificationRun,
) -> Result<(), String> {
    use dctx0_core::VerificationResult;
    writeln!(
        output,
        "Causes: {:?}\n",
        dctx0_verification::diagnostics::cause_counts(run)
    )
    .map_err(crate::error_message)?;
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
    /// Preserve the complete legacy JSON report (explicit opt-in).
    #[arg(long, global = true)]
    full: bool,
    #[arg(long, global = true, default_value_t = 1_048_576, value_parser = clap::value_parser!(u64).range(1..=67_108_864))]
    max_output_bytes: u64,
}
#[derive(serde::Serialize)]
#[cfg(any(feature = "typescript", feature = "go", feature = "rust"))]
struct PreflightedRun<'a> {
    #[serde(flatten)]
    report: &'a dctx0_engine::NativeVerificationReport,
    preflight: &'a serde_json::Value,
}
#[derive(Subcommand)]
enum VerifyAction {
    /// Diagnostic application-owned tests with central Rust assessment.
    ApplicationTests(crate::application_testing::ApplicationTestingArgs),
    /// Check scope, task pins, planned cases and declared bindings without execution.
    Preflight {
        input: PathBuf,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = task_id)]
        task: Vec<i64>,
        #[arg(long)]
        historical_check: Option<i64>,
        /// Plan a diagnostic run; never promises or attempts task delivery.
        #[arg(long, alias = "no-adopt")]
        diagnostic: bool,
        /// Authored schema-1 BindingDescription, relative to the application.
        #[arg(long)]
        bindings: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long, default_value_t = 0)]
        offset: usize,
    },
    /// Plan reviewed native connections from the selected published requirements.
    Bindings(crate::verification_bindings::BindingArgs),
    /// Generate editable testing codecs from published IR and explicit representations.
    ScaffoldCodecs(crate::codec_scaffold::CodecScaffoldArgs),
    /// Compare the latest task review to current sources; no renewal or approval.
    ReviewChanges {
        input: PathBuf,
        #[arg(long, value_parser = task_id)]
        task: i64,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Build captured Rust production and execute the repository self specification.
    #[command(name = "self")]
    SelfCheck {
        input: PathBuf,
        #[arg(long)]
        subject: Vec<String>,
    },
    /// Read-only source-bound report template for an assigned conformance review task.
    ReviewTemplate {
        input: PathBuf,
        #[arg(long, value_parser = task_id)]
        task: i64,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Check, execute, apply authored source inspections and require zero unfinished tasks.
    Complete {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        /// Authored source-bound inspection notes; defaults to the testing adapter directory.
        #[arg(long)]
        inspections: Option<PathBuf>,
        /// Maximum authored inspection JSON bytes.
        #[arg(long, default_value_t = DEFAULT_REVIEW_INPUT_BYTES, value_parser = clap::value_parser!(u64).range(1..))]
        max_input_bytes: u64,
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
        /// Maximum JSON input bytes; read incrementally without a second full buffer.
        #[arg(long, default_value_t = DEFAULT_REVIEW_INPUT_BYTES, value_parser = clap::value_parser!(u64).range(1..))]
        max_input_bytes: u64,
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
        /// Maximum JSON input bytes; read incrementally without a second full buffer.
        #[arg(long, default_value_t = DEFAULT_REVIEW_INPUT_BYTES, value_parser = clap::value_parser!(u64).range(1..))]
        max_input_bytes: u64,
    },
    /// Inspect target syntax; never execute or adopt candidates.
    Discover {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long, default_value = "typescript")]
        target: String,
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
    /// Compare saved evidence with current inputs without executing or adopting tests.
    IncrementalPlan {
        input: PathBuf,
        run_id: String,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_parser = task_id)]
        task: Vec<i64>,
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
        #[arg(long, alias = "diagnostic")]
        no_adopt: bool,
        #[arg(long)]
        bindings: Option<String>,
        /// Explicit deterministic scopes and snapshot/revision inputs for historical reuse.
        #[arg(long)]
        reuse_inputs: Option<String>,
        /// Same-project saved native run; requires the same input declaration.
        #[arg(long, requires = "reuse_inputs")]
        reuse_from: Option<String>,
    },
    /// List compact historical outcomes; never loads case corpora.
    History {
        input: PathBuf,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// Prune old verification details; compact rebuilds without a backup.
    Gc {
        input: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        compact: bool,
        /// Reduce embedded reports only in this explicitly selected directory.
        #[arg(long)]
        exports: Option<PathBuf>,
    },
    /// Atomically export complete saved evidence or one exact case.
    Export {
        input: PathBuf,
        run_id: String,
        #[arg(long)]
        case: Option<String>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Read immutable saved evidence without claiming working-tree freshness.
    Show {
        input: PathBuf,
        run_id: String,
        /// Compact aggregate without individual case rows.
        #[arg(long)]
        summary: bool,
        /// Filter case rows; the full run assessment stays visible.
        #[arg(long, value_delimiter = ',', value_parser = ["PASS", "FAIL", "UNKNOWN"])]
        outcome: Vec<String>,
        /// Filter by structured diagnostic cause. Never changes the saved result.
        #[arg(long, value_delimiter = ',', value_parser = ["contract-violation", "adapter", "environment", "evaluator-limit", "missing-evidence", "inspection-required", "unclassified"])]
        cause: Vec<String>,
        #[arg(long)]
        subject: Vec<String>,
        /// Exact case ID, including actual observations and bindings.
        #[arg(long)]
        case: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Compare exact saved case identities; removed cases never count as repaired.
    Compare {
        input: PathBuf,
        before_run: String,
        after_run: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long, default_value_t = 0)]
        offset: usize,
    },
    /// Explain current adoption eligibility without execution or task changes.
    AdoptionPreview {
        input: PathBuf,
        run_id: String,
        #[arg(long)]
        project: Option<PathBuf>,
    },
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
        /// Export diagnostic application-test ownership using schema-2 groups.
        #[arg(long, requires = "test_catalog")]
        application_tests: Option<PathBuf>,
        /// Expected TestRegistration array; does not prove native execution.
        #[arg(long, requires = "application_tests")]
        test_catalog: Option<PathBuf>,
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
    if !args.full
        && matches!(
            &args.action,
            VerifyAction::Run { .. }
                | VerifyAction::Complete { .. }
                | VerifyAction::Inspect { .. }
                | VerifyAction::Attest { .. }
                | VerifyAction::Replay { .. }
                | VerifyAction::Show { .. }
        )
    {
        crate::render::with_output_limit(output, args.max_output_bytes, |output| {
            run_inner(args, database, json, output)
        })
    } else {
        run_inner(args, database, json, output)
    }
}
fn run_inner(
    args: VerifyArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let full = args.full;
    let max_output_bytes = args.max_output_bytes;
    let (
        input,
        application,
        subject,
        task,
        historical_check,
        fixtures,
        application_tests,
        test_catalog,
    ) = match args.action {
        VerifyAction::IncrementalPlan {
            input,
            run_id,
            project: application,
            subject,
            task,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .verification_incremental_plan(
                    application.as_deref(),
                    VerificationSelection {
                        subjects: subject.into_iter().collect(),
                        tasks: task.into_iter().collect(),
                        historical_check: None,
                    },
                    &run_id,
                )
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &report)?;
            } else {
                writeln!(
                    output,
                    "# Incremental verification comparison\n\nParent: `{}`\nRequired: {}; reused: {}; fresh execution required: {}.\n\nThis comparison grants no test PASS or task delivery.\n\n| Condition | Match |\n| --- | --- |",
                    report.parent_run, report.required_cases, report.actual_reused_cases, report.rerun_cases
                )
                .map_err(crate::error_message)?;
                for comparison in &report.comparisons {
                    writeln!(
                        output,
                        "| {} | {} |",
                        comparison.component,
                        match comparison.matches {
                            Some(true) => "yes",
                            Some(false) => "no",
                            None => "unknown",
                        }
                    )
                    .map_err(crate::error_message)?;
                }
                for reason in &report.reasons {
                    writeln!(output, "\n- {}: {}", reason.code, reason.message)
                        .map_err(crate::error_message)?;
                }
            }
            return Ok(true);
        }
        VerifyAction::ApplicationTests(args) => {
            return crate::application_testing::run(args, database, json, output);
        }
        VerifyAction::Preflight {
            input,
            project: application,
            subject,
            task,
            historical_check,
            diagnostic,
            bindings,
            limit,
            offset,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let intent = if diagnostic {
                dctx0_engine::VerificationIntent::Diagnostic
            } else {
                dctx0_engine::VerificationIntent::Delivery
            };
            let report = project
                .verification_preflight(
                    application.as_deref(),
                    VerificationSelection {
                        subjects: subject.into_iter().collect(),
                        tasks: task.into_iter().collect(),
                        historical_check,
                    },
                    intent,
                    bindings.as_deref(),
                )
                .map_err(crate::error_message)?;
            crate::verification_preflight::render(
                &report,
                json,
                full,
                limit,
                offset,
                max_output_bytes,
                output,
            )?;
            return Ok(report.permits_execution());
        }
        VerifyAction::Bindings(args) => {
            return crate::verification_bindings::run(args, database, json, output);
        }
        VerifyAction::ScaffoldCodecs(args) => {
            return crate::codec_scaffold::run(args, database, json, output);
        }
        VerifyAction::ReviewChanges {
            input,
            task,
            project: application,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .review_changes(application.as_deref(), task)
                .map_err(crate::error_message)?;
            crate::render::json(output, &report)?;
            return Ok(true);
        }
        VerifyAction::SelfCheck { input, subject } => {
            #[cfg(feature = "rust")]
            {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error_message)?;
                let report = runtime
                    .block_on(dctx0_engine::verify_self(
                        &input,
                        subject.into_iter().collect(),
                        &dctx0_target_rust::RustHarness,
                        &dctx0_target_rust::Provider,
                        &dctx0_engine::NativeCancellation::default(),
                    ))
                    .map_err(crate::error_message)?;
                let pass = report.selected_machine_complete;
                if json {
                    crate::render::json(output, &report)?;
                } else {
                    writeln!(output, "# Rust self-verification\n\nMachine complete: {}; source current: {}; partial: {}\n{} mandatory cases; {} recorded cases; {} review obligations.\nReport: `{}`\nReview approval remains separate from native evidence.", report.machine_complete, report.source_current, report.partial, report.native.evidence.run.required_cases.len(), report.native.evidence.run.cases.len(), report.review_obligations.len(), report.report_path.display()).map_err(crate::error_message)?;
                }
                return Ok(pass);
            }
            #[cfg(not(feature = "rust"))]
            {
                let _ = (input, subject);
                return Err("Self-verification requires the Rust target feature".into());
            }
        }
        VerifyAction::ReviewTemplate {
            input,
            task,
            project: application,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .conformance_review_template(application.as_deref(), task)
                .map_err(crate::error_message)?;
            crate::render::json(output, &report)?;
            return Ok(true);
        }
        VerifyAction::Complete {
            input,
            project: application,
            inspections,
            max_input_bytes,
        } => {
            #[cfg(any(feature = "typescript", feature = "go", feature = "rust"))]
            {
                if let Some(pass) = super::verification_complete::memory_guard()? {
                    return Ok(pass);
                }
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                let target = configured_target(&project, Some(&application))?;
                let (provider, generator) = execution_target(&target)?;
                let notes = inspections.unwrap_or_else(|| {
                    application.join(format!(".dctx0/testing/{target}/inspection-notes.json"))
                });
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error_message)?;
                eprintln!(
                    "Complete verification: checking sources, executing configured native cases, then binding authored inspections."
                );
                let report = runtime
                    .block_on(project.complete_verification_with_input_limit(
                        &application,
                        &notes,
                        provider,
                        generator,
                        &dctx0_engine::NativeCancellation::default(),
                        max_input_bytes,
                    ))
                    .map_err(crate::error_message)?;
                if json {
                    if full {
                        crate::render::json(output, &report)?;
                    } else {
                        let summary = project
                            .verification_summary(&report.inspected_run)
                            .map_err(crate::error_message)?;
                        crate::render::bounded_json(
                            output,
                            &{
                                let mut compact =
                                    serde_json::to_value(&report).map_err(crate::error_message)?;
                                compact["summary"] =
                                    serde_json::to_value(summary).map_err(crate::error_message)?;
                                compact
                            },
                            max_output_bytes,
                        )?;
                    }
                } else {
                    writeln!(output, "# Complete verification\n\nResult: PASS\nMandatory native cases: {}\nTasks: {}; unfinished: {}; without PASS: {}\nSource-bound review tasks: {:?}\nNative result: {:?}\nNative run: `{}`\nEffective run: `{}`\nBasis: {}", report.mandatory_cases, report.tasks, report.open_tasks, report.non_pass_tasks, report.reviewed_tasks, report.native_result, report.native_run, report.inspected_run, report.basis).map_err(crate::error_message)?;
                }
                return Ok(true);
            }
            #[cfg(not(any(feature = "typescript", feature = "go", feature = "rust")))]
            {
                let _ = (input, application, inspections, max_input_bytes);
                return Err("No executable TypeScript testing target is enabled".into());
            }
        }
        VerifyAction::Inspect {
            input,
            run_id,
            project: application,
            file,
            max_input_bytes,
        } => {
            let inspections: Vec<dctx0_verification::program_inspection::ProgramInspection> =
                read_review_input(&file, max_input_bytes, "Inspection")
                    .map_err(crate::error_message)?;
            let report = Project::new(&input, database)
                .map_err(crate::error_message)?
                .publish_program_inspections(application.as_deref(), &run_id, &inspections)
                .map_err(crate::error_message)?;
            let pass = report.assessment.result == dctx0_core::VerificationResult::Pass;
            if json {
                if full {
                    crate::render::json(output, &report)?;
                } else {
                    compact_run(
                        output,
                        &report.evidence,
                        serde_json::json!({}),
                        max_output_bytes,
                    )?;
                }
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
            max_input_bytes,
        } => {
            let reports: Vec<dctx0_engine::TaskAttestation> =
                read_review_input(&file, max_input_bytes, "Attestation")
                    .map_err(crate::error_message)?;
            let receipt = Project::new(&input, database)
                .map_err(crate::error_message)?
                .publish_task_attestations(application.as_deref(), &run_id, &reports)
                .map_err(crate::error_message)?;
            if json {
                if full {
                    crate::render::json(output, &receipt)?;
                } else {
                    let summary = Project::new(&input, database)
                        .map_err(crate::error_message)?
                        .verification_summary(&run_id)
                        .map_err(crate::error_message)?;
                    crate::render::bounded_json(
                        output,
                        &serde_json::json!({"schema_version":1,"summary":summary,"attestation":{"run_id":receipt.run_id,"request_hash":receipt.request_hash,"event_sequence":receipt.event_sequence,"delivered_tasks":receipt.deliveries.iter().filter_map(|r|r.task_id).collect::<Vec<_>>()}}),
                        max_output_bytes,
                    )?;
                }
            } else {
                writeln!(output, "# Agent inspection delivery\n\n{} tasks delivered with agent evidence. Native run `{run_id}` remains unchanged. Event sequence: {}.", receipt.deliveries.len(), receipt.event_sequence).map_err(crate::error_message)?;
            }
            return Ok(true);
        }
        VerifyAction::Discover {
            input,
            project: application,
            target,
        } => {
            #[cfg(any(feature = "typescript", feature = "go", feature = "rust"))]
            {
                let inspector: &dyn dctx0_target_sdk::ImplementationInspector = match target
                    .as_str()
                {
                    #[cfg(feature = "typescript")]
                    "typescript" => &dctx0_target_typescript::TypeScriptInspector,
                    #[cfg(feature = "go")]
                    "go" => &dctx0_target_go::GoInspector,
                    #[cfg(feature = "rust")]
                    "rust" => &dctx0_target_rust::RustInspector,
                    _ => return Err(format!("No structural inspector is enabled for {target}")),
                };
                let report = Project::new(&input, database)
                    .map_err(crate::error_message)?
                    .discover_verification(&application, inspector)
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
            #[cfg(not(any(feature = "typescript", feature = "go", feature = "rust")))]
            {
                let _ = (input, application, target);
                return Err("No structural inspector is enabled".into());
            }
        }
        VerifyAction::Replay {
            input,
            run_id,
            case,
            minimized,
            project: application,
        } => {
            #[cfg(any(feature = "typescript", feature = "go", feature = "rust"))]
            {
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                let target = configured_target(&project, application.as_deref())?;
                let (_, generator) = execution_target(&target)?;
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
                                    generator,
                                    &cancel,
                                )
                                .await
                        } else {
                            project
                                .replay_verification(
                                    application.as_deref(),
                                    &run_id,
                                    &case,
                                    generator,
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
                    if full {
                        crate::render::json(output, &report)?;
                    } else {
                        compact_run(
                            output,
                            &report.evidence.evidence,
                            serde_json::json!({"original_run":report.original_run,"original_case":report.original_case,"case_result":report.case_result}),
                            max_output_bytes,
                        )?;
                    }
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
            #[cfg(not(any(feature = "typescript", feature = "go", feature = "rust")))]
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
            bindings,
            reuse_inputs,
            reuse_from,
        } => {
            #[cfg(any(feature = "typescript", feature = "go", feature = "rust"))]
            {
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                let target = configured_target(&project, application.as_deref())?;
                let (_, generator) = execution_target(&target)?;
                let selection = VerificationSelection {
                    subjects: BTreeSet::from_iter(subject),
                    tasks: BTreeSet::from_iter(task),
                    historical_check,
                };
                let intent = if no_adopt || historical_check.is_some() {
                    dctx0_engine::VerificationIntent::Diagnostic
                } else {
                    dctx0_engine::VerificationIntent::Delivery
                };
                let preflight = project
                    .verification_preflight(
                        application.as_deref(),
                        selection.clone(),
                        intent,
                        bindings.as_deref(),
                    )
                    .map_err(crate::error_message)?;
                if !preflight.permits_execution() {
                    crate::verification_preflight::render(
                        &preflight,
                        json,
                        full,
                        20,
                        0,
                        max_output_bytes,
                        output,
                    )?;
                    return Ok(false);
                }
                let preflight_summary = crate::verification_preflight::summary(&preflight, 20, 0);
                if !json {
                    crate::verification_preflight::render(
                        &preflight,
                        false,
                        false,
                        20,
                        0,
                        max_output_bytes,
                        output,
                    )?;
                    output.flush().map_err(crate::error_message)?;
                }
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error_message)?;
                let cancellation = dctx0_engine::NativeCancellation::default();
                let report = if let Some(inputs) = reuse_inputs {
                    runtime.block_on(
                        project.run_verification_incremental_preflighted_with_adoption(
                            application.as_deref(),
                            selection,
                            generator,
                            &cancellation,
                            &dctx0_engine::NativeReuseOptions {
                                inputs,
                                parent: reuse_from,
                            },
                            preflight,
                            no_adopt,
                        ),
                    )
                } else {
                    runtime.block_on(project.run_verification_preflighted(
                        application.as_deref(),
                        selection,
                        generator,
                        &cancellation,
                        preflight,
                    ))
                }
                .map_err(crate::error_message)?;
                let pass = matches!(
                    report.evidence.run.result,
                    dctx0_core::VerificationResult::Pass
                );
                if json {
                    if full {
                        crate::render::json(
                            output,
                            &PreflightedRun {
                                report: &report,
                                preflight: &preflight_summary,
                            },
                        )?;
                    } else {
                        compact_run(
                            output,
                            &report.evidence,
                            serde_json::json!({"adopted_tasks":report.adopted_tasks,"adoption_issues":report.adoption_issues,"preflight":preflight_summary,"incremental":report.incremental}),
                            max_output_bytes,
                        )?;
                    }
                } else {
                    writeln!(output,"# Native verification\n\nRun: `{}`\nSnapshot: `{}`; check occurrence: {}\nResult: {:?}\n\n{} mandatory cases; {} recorded cases; {} open obligations.\n",report.evidence.id,report.evidence.run.provenance.snapshot,report.evidence.run.provenance.check_occurrence,report.evidence.run.result,report.evidence.run.required_cases.len(),report.evidence.run.cases.len(),report.evidence.run.open_obligations.len()).map_err(crate::error_message)?;
                    if let Some(supervision) = &report.supervision {
                        writeln!(output, "Process-tree supervision: {}; detached descendants: {}; collective memory limit: {}.\n", supervision.process_tree, supervision.detached_descendants, supervision.collective_memory_limit).map_err(crate::error_message)?;
                    }
                    if let Some(summary) = &report.incremental {
                        writeln!(output,"Historical reuse: {} cases; fresh execution: {} cases. Parent: {:?}. No task delivery authority.",summary.reused_cases,summary.executed_cases,summary.parent_run).map_err(crate::error_message)?;
                    }
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
            #[cfg(not(any(feature = "typescript", feature = "go", feature = "rust")))]
            {
                let _ = (
                    input,
                    application,
                    subject,
                    task,
                    historical_check,
                    no_adopt,
                    bindings,
                    reuse_inputs,
                    reuse_from,
                );
                return Err("No executable TypeScript testing target is enabled".into());
            }
        }
        VerifyAction::History {
            input,
            offset,
            limit,
        } => {
            let page = Project::new(&input, database)
                .map_err(crate::error_message)?
                .verification_history(offset, limit)
                .map_err(crate::error_message)?;
            if json {
                crate::render::bounded_json(output, &page, max_output_bytes)?;
            } else {
                for item in page {
                    writeln!(
                        output,
                        "{} {:?} · {} · {}",
                        item.id,
                        item.result,
                        item.created_at,
                        if item.details_available {
                            "full details"
                        } else {
                            "details pruned; historical result only"
                        }
                    )
                    .map_err(crate::error_message)?;
                }
            }
            return Ok(true);
        }
        VerifyAction::Gc {
            input,
            dry_run,
            compact,
            exports,
        } => {
            let report = Project::new(&input, database)
                .map_err(crate::error_message)?
                .verification_gc(dry_run, compact, exports.as_deref())
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &report)?;
            } else {
                writeln!(output,"{}: {} full runs → {}; {} runs pruned.\nDatabase: {} → {} bytes. Stored run/artifact payloads: {} → {} bytes.\nNo backup created.",if dry_run {"Preview"} else {"Verification cleanup"},report.storage.full_runs_before,report.storage.full_runs_after,report.storage.pruned_runs,report.file_bytes_before,report.file_bytes_after,report.storage.logical_bytes_before,report.storage.logical_bytes_after).map_err(crate::error_message)?;
                for file in report.exports {
                    writeln!(
                        output,
                        "{}: {} ({} → {} bytes)",
                        file.path.display(),
                        file.status,
                        file.bytes_before,
                        file.bytes_after
                    )
                    .map_err(crate::error_message)?;
                }
            }
            return Ok(true);
        }
        VerifyAction::AdoptionPreview {
            input,
            run_id,
            project: application,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .verification_adoption_preview(application.as_deref(), &run_id)
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &report)?;
            } else {
                writeln!(output, "# Adoption preview\n\nRun: {}. Read-only; eligibility must be rechecked transactionally at publication.\n", report.run_id).map_err(crate::error_message)?;
                for task in &report.tasks {
                    writeln!(output, "- T{}: eligible: {}", task.task_id, task.eligible)
                        .map_err(crate::error_message)?;
                    for issue in &task.blockers {
                        writeln!(output, "  - {}: {}", issue.code, issue.message)
                            .map_err(crate::error_message)?;
                    }
                }
                for issue in &report.issues {
                    writeln!(output, "- {}: {}", issue.code, issue.message)
                        .map_err(crate::error_message)?;
                }
            }
            return Ok(true);
        }
        VerifyAction::Export {
            input,
            run_id,
            case,
            output: destination,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let store = project
                .store(dctx0_store::AccessMode::ReadOnly)
                .map_err(crate::error_message)?;
            store
                .export_verification(
                    &project.root.display().to_string(),
                    &run_id,
                    case.as_deref(),
                    &destination,
                )
                .map_err(crate::error_message)?;
            return Ok(true);
        }
        VerifyAction::Compare {
            input,
            before_run,
            after_run,
            limit,
            offset,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .store(dctx0_store::AccessMode::ReadOnly)
                .map_err(crate::error_message)?
                .verification_comparison(
                    &project.root.display().to_string(),
                    &before_run,
                    &after_run,
                    offset,
                    limit,
                )
                .map_err(crate::error_message)?;
            if json {
                crate::render::bounded_json(output, &report, max_output_bytes)?;
            } else {
                writeln!(output, "New: {}; persisting: {}; changed: {}; resolved: {}; unproven: {}. Added cases: {}; removed cases: {}.\nSaved evidence comparison; no current-source or task-delivery claim.", report.new, report.persisting, report.changed, report.resolved, report.unproven, report.added_cases.len(), report.removed_cases.len()).map_err(crate::error_message)?;
                for item in &report.issues {
                    writeln!(
                        output,
                        "- {} / {}: {:?}",
                        item.case_id, item.subject, item.change
                    )
                    .map_err(crate::error_message)?;
                }
                if let Some(next) = report.next_offset {
                    writeln!(output, "More issues: --offset {next} --limit {limit}")
                        .map_err(crate::error_message)?;
                }
            }
            return Ok(true);
        }
        VerifyAction::Show {
            input,
            run_id,
            summary: summary_only,
            outcome,
            cause,
            subject,
            case,
            limit,
            cursor,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let summary = project
                .verification_summary(&run_id)
                .map_err(crate::error_message)?;
            if !summary.details_available {
                if json {
                    crate::render::json(output, &summary)?;
                } else {
                    writeln!(output,"Run: {}\nHistorical result: {:?}\nDetails pruned at {}. Replay and new evidence publication are unavailable.",summary.id,summary.result,summary.pruned_at.as_deref().unwrap_or("unknown")).map_err(crate::error_message)?;
                }
                return Ok(true);
            }
            if !full
                && outcome.is_empty()
                && cause.is_empty()
                && subject.is_empty()
                && case.is_none()
                && limit.is_none()
                && cursor.is_none()
            {
                if json {
                    crate::render::bounded_json(
                        output,
                        &serde_json::json!({"schema_version":1,"summary":summary}),
                        max_output_bytes,
                    )?;
                } else {
                    writeln!(output,"# Stored verification summary\n\nRun: `{}`\nSnapshot: `{}`; check occurrence: {}\nResult: {:?}\nMandatory: {}; recorded: {}; missing: {}\nOutcomes (PASS/FAIL/UNKNOWN): {:?}\nOpen obligations: {}\nDetails available: {}\nSaved evidence; no working-tree freshness.\n",summary.id,summary.snapshot,summary.check_occurrence,summary.result,summary.required_case_count,summary.case_count,summary.missing_case_count,summary.outcomes,summary.open_obligation_count,summary.details_available).map_err(crate::error_message)?;
                    writeln!(
                        output,
                        "Causes: {:?} (absent historical counts are unmeasured).",
                        summary.causes
                    )
                    .map_err(crate::error_message)?;
                }
                return Ok(true);
            }
            if summary_only
                || !outcome.is_empty()
                || !cause.is_empty()
                || !subject.is_empty()
                || case.is_some()
                || limit.is_some()
                || cursor.is_some()
            {
                use dctx0_verification::diagnostics::*;
                let cursor = cursor
                    .as_deref()
                    .map(serde_json::from_str::<DiagnosticCursor>)
                    .transpose()
                    .map_err(crate::error_message)?;
                let filter = DiagnosticFilter {
                    causes: cause
                        .into_iter()
                        .map(|c| serde_json::from_value(serde_json::Value::String(c)))
                        .collect::<Result<_, _>>()
                        .map_err(crate::error_message)?,
                    outcomes: outcome.into_iter().collect(),
                    subjects: subject.into_iter().collect(),
                    case,
                };
                let page = project
                    .store(dctx0_store::AccessMode::ReadOnly)
                    .map_err(crate::error_message)?
                    .verification_diagnostics(
                        &project.root.display().to_string(),
                        &run_id,
                        filter,
                        limit.unwrap_or(50),
                        cursor.as_ref(),
                        summary_only,
                    )
                    .map_err(crate::error_message)?;
                if json {
                    crate::render::bounded_json(output, &page, max_output_bytes)?;
                } else {
                    writeln!(output, "# Stored verification diagnostics\n\nRun: {}\nResult: {:?}\nMandatory: {}; recorded: {}; matching: {}\nOutcomes: {:?}\n\nSaved evidence; no current-source or adoption claim.\n", page.run_id, page.summary.result, page.summary.mandatory_cases, page.summary.recorded_cases, page.matching_cases, page.summary.outcomes).map_err(crate::error_message)?;
                    writeln!(output, "Causes: {:?}", page.summary.causes)
                        .map_err(crate::error_message)?;
                    for case in &page.cases {
                        writeln!(
                            output,
                            "- {} / {}: {:?}",
                            case.id, case.subject, case.result
                        )
                        .map_err(crate::error_message)?;
                    }
                    for issue in page
                        .summary
                        .open_obligations
                        .iter()
                        .chain(&page.summary.incomplete_provenance)
                    {
                        writeln!(output, "- {}: {}", issue.code, issue.message)
                            .map_err(crate::error_message)?;
                    }
                    if let Some(detail) = &page.detail {
                        crate::render::bounded_json(output, detail, max_output_bytes)?;
                    }
                    if let Some(cursor) = &page.next_cursor {
                        writeln!(
                            output,
                            "\nMore: --cursor '{}'",
                            serde_json::to_string(cursor).map_err(crate::error_message)?
                        )
                        .map_err(crate::error_message)?;
                    }
                }
                return Ok(true);
            }
            let saved = project
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
            if let Ok((_, generator)) = execution_target(&target) {
                let report = Project::new(&input, database)
                    .map_err(crate::error_message)?
                    .initialize_verification(
                        &project,
                        generator,
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
            application_tests,
            test_catalog,
        } => (
            input,
            project,
            subject,
            task,
            historical_check,
            fixtures,
            application_tests,
            test_catalog,
        ),
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
    if let (Some(configuration), Some(catalog)) = (application_tests, test_catalog) {
        return crate::application_test_plan::render(&plan, &configuration, &catalog, json, output);
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

fn compact_run(
    output: &mut dyn Write,
    saved: &dctx0_store::StoredVerificationRun,
    extra: serde_json::Value,
    max: u64,
) -> Result<(), String> {
    let mut summary = dctx0_store::VerificationRunSummary::new(&saved.id, &saved.run)
        .map_err(crate::error_message)?;
    summary.created_at = saved.created_at.clone();
    crate::render::bounded_json(
        output,
        &serde_json::json!({"schema_version":1,"summary":summary,"created_at":saved.created_at,"missing_cases":saved.run.required_cases.len().saturating_sub(saved.run.cases.len()),"open_obligation_count":saved.run.open_obligations.len(),"extra":extra,"details":{"show":format!("verify show <spec> {} --case CASE_ID",saved.id),"export":format!("verify export <spec> {} --output report.json",saved.id)}}),
        max,
    )
}
