//! Compact read-only project status; never prepares query tables or exports.
use clap::Args;
use dctx0_engine::Project;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(crate) struct StatusArgs {
    input: PathBuf,
    #[arg(long)]
    project: Option<PathBuf>,
    #[arg(long, requires = "since_event")]
    since_plan: Option<i64>,
    #[arg(long, requires = "since_plan")]
    since_event: Option<i64>,
}

pub(crate) fn run(
    args: StatusArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let project = Project::new(&args.input, database).map_err(crate::error_message)?;
    let report = project
        .status(
            args.project.as_deref(),
            args.since_plan.zip(args.since_event),
        )
        .map_err(crate::error_message)?;
    if json {
        crate::render::bounded_json(output, &report, 131_072)?;
    } else {
        writeln!(
            output,
            "Tasks: {} Done/PASS; {} Done without PASS; {} Open; {} Progress; {} blocked",
            report.tasks.done_pass,
            report.tasks.done_without_pass,
            report.tasks.open,
            report.tasks.in_progress,
            report.tasks.blocked
        )
        .map_err(crate::error_message)?;
        writeln!(
            output,
            "Done/PASS source binding: {} current; {} stale; {} unknown",
            report.tasks.evidence_current,
            report.tasks.evidence_stale,
            report.tasks.evidence_unknown
        )
        .map_err(crate::error_message)?;
        if report.tasks.inspected_current + report.tasks.inspected_stale > 0 {
            writeln!(output, "INSPECTED + actual native: {} current; {} stale (finite cases and authored findings)", report.tasks.inspected_current, report.tasks.inspected_stale)
                .map_err(crate::error_message)?;
        }
        if report.tasks.agent_current + report.tasks.agent_stale > 0 {
            writeln!(output, "Agent inspection + actual native: {} current; {} stale (checked source/requirement/profile binding, not automatic proof)", report.tasks.agent_current, report.tasks.agent_stale)
                .map_err(crate::error_message)?;
        }
        writeln!(
            output,
            "Specification current: {:?}; task plan current: {:?}",
            report.spec_current, report.task_plan_current
        )
        .map_err(crate::error_message)?;
        if let Some(pin) = &report.task_pin {
            writeln!(
                output,
                "Task pin: P{} at event {} / check {}",
                pin.plan_id, pin.event_seq, pin.check_occurrence
            )
            .map_err(crate::error_message)?;
        }
        if let Some(run) = report.effective_run.as_ref().or(report.latest_run.as_ref()) {
            writeln!(
                output,
                "Recorded diagnostic causes: {:?}",
                run.summary.causes
            )
            .map_err(crate::error_message)?;
            writeln!(output, "{}: {} ({}) {:?}\nCases: {}/{} recorded; {} missing; PASS/FAIL/UNKNOWN {:?}; {} open obligations\nSources current: {:?}; requirements current: {:?}; task pin present: {:?}", if report.effective_run.is_some() { "Effective native evidence" } else { "Latest run" }, run.summary.id, run.summary.target, run.summary.result, run.summary.case_count, run.summary.required_case_count, run.summary.missing_case_count, run.summary.outcomes, run.summary.open_obligation_count, run.sources_current, run.requirements_current, run.task_pin_present).map_err(crate::error_message)?;
        } else {
            writeln!(output, "Latest run: none").map_err(crate::error_message)?;
        }
        if let Some(effective) = &report.effective_run {
            if let Some(latest) = &report.latest_run {
                if latest.summary.id != effective.summary.id {
                    writeln!(output, "Last attempt (history): {}; current acceptance uses the effective evidence above", latest.summary.id).map_err(crate::error_message)?;
                }
            }
        }
        if report.effective_run.is_none()
            && let Some(baseline) = &report.historical_complete_baseline
        {
            writeln!(output, "Historical complete native baseline: {} with {} passing cases; current compatibility requires input validation", baseline.id, baseline.required_case_count).map_err(crate::error_message)?;
        }
        if let Some(progress) = &report.progress {
            writeln!(output,"Since P{} / event {}: {} newly Done; {} newly PASS; {} added Done; {} added PASS; {} lost PASS",progress.baseline.plan_id,progress.baseline.event_seq,progress.newly_done,progress.newly_pass,progress.added_done,progress.added_pass,progress.lost_pass).map_err(crate::error_message)?;
        } else {
            writeln!(output,"Progress delta: unknown; pass --since-plan and --since-event for an explicit baseline").map_err(crate::error_message)?;
        }
        for cause in &report.next_causes {
            writeln!(
                output,
                "- {} ({}): {}",
                cause.code, cause.affected, cause.next_step
            )
            .map_err(crate::error_message)?;
        }
        for issue in &report.issues {
            writeln!(output, "Unknown {}: {}", issue.code, issue.message)
                .map_err(crate::error_message)?;
        }
        writeln!(
            output,
            "Read-only status; historical Done/PASS remains historical evidence. No task adoption."
        )
        .map_err(crate::error_message)?;
    }
    Ok(true)
}
