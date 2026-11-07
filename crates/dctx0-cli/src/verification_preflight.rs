use dctx0_engine::VerificationPreflight;
use std::{collections::BTreeMap, io::Write};

pub(crate) fn summary(
    report: &VerificationPreflight,
    limit: usize,
    offset: usize,
) -> serde_json::Value {
    let counts = |issues: &[dctx0_core::VerificationIssue]| {
        let mut counts = BTreeMap::<String, usize>::new();
        for issue in issues {
            *counts.entry(issue.code.clone()).or_default() += 1;
        }
        counts
    };
    let issues = report
        .blockers
        .iter()
        .map(|issue| ("blocker", issue))
        .chain(report.warnings.iter().map(|issue| ("warning", issue)))
        .skip(offset)
        .take(limit)
        .map(|(kind, issue)| serde_json::json!({"kind":kind,"issue":issue}))
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version":1,"executed":false,"intent":report.intent,"permits_execution":report.permits_execution(),
        "snapshot":report.pin.snapshot,"check_occurrence":report.pin.check_occurrence,"plan_hash":report.pin.plan_hash,"inputs_hash":report.pin.inputs_hash,
        "task_pin":report.pin.task_pin.as_ref().map(|pin|serde_json::json!({"plan_id":pin.plan_id,"event_sequence":pin.event_sequence,"selected_tasks":pin.requirements.len()})),
        "required_cases":report.required_cases,"required_capabilities":report.required_capabilities,
        "potential_task_count":report.potential_tasks.len(),"potential_tasks":report.potential_tasks.iter().skip(offset).take(limit).collect::<Vec<_>>(),
        "bindings_checked":report.bindings_checked,"fixture_readiness":report.fixture_readiness,
        "blockers":counts(&report.blockers),"warnings":counts(&report.warnings),
        "issues":{"total":report.blockers.len()+report.warnings.len(),"offset":offset,"limit":limit,"items":issues,"complete":offset.saturating_add(limit)>=report.blockers.len()+report.warnings.len()}
    })
}

pub(crate) fn render(
    report: &VerificationPreflight,
    json: bool,
    full: bool,
    limit: usize,
    offset: usize,
    max_output_bytes: u64,
    output: &mut dyn Write,
) -> Result<(), String> {
    if !(1..=200).contains(&limit) {
        return Err("Preflight page limit must be 1..200".into());
    }
    if json {
        if full {
            crate::render::json(output, report)
        } else {
            crate::render::bounded_json(output, &summary(report, limit, offset), max_output_bytes)
        }
    } else {
        writeln!(output,"Preflight {:?}: execution {}; {} mandatory cases; {} potential task deliveries\nSnapshot {} / check {}\nBindings checked: {}; private fixtures: {}",report.intent,if report.permits_execution(){"permitted"}else{"blocked"},report.required_cases,report.potential_tasks.len(),report.pin.snapshot,report.pin.check_occurrence,report.bindings_checked,report.fixture_readiness).map_err(crate::error_message)?;
        if !report.potential_tasks.is_empty() {
            writeln!(
                output,
                "Potential tasks if all proof/review/transaction guards pass: {:?}",
                report
                    .potential_tasks
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .collect::<Vec<_>>()
            )
            .map_err(crate::error_message)?;
        }
        for (kind, issue) in report
            .blockers
            .iter()
            .map(|i| ("blocker", i))
            .chain(report.warnings.iter().map(|i| ("warning", i)))
            .skip(offset)
            .take(limit)
        {
            writeln!(output, "- {kind} {}: {}", issue.code, issue.message)
                .map_err(crate::error_message)?;
        }
        writeln!(output,"{} blockers; {} warnings. No execution, evidence publication or task adoption. Use --offset/--limit for remaining details.",report.blockers.len(),report.warnings.len()).map_err(crate::error_message)
    }
}
