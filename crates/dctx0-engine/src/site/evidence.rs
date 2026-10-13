//! Browser-specific lossless case packing. Native history models remain unchanged.
use super::*;
use base64::Engine;
use dctx0_core::{
    VerificationArtifactRef, VerificationBudgets, VerificationCaseRecord, VerificationIssue,
    VerificationProvenance, VerificationResult, VerificationRunOrigin,
};
use std::collections::BTreeSet;
#[derive(Serialize)]
struct RunView<'a> {
    schema_version: u32,
    origin: &'a VerificationRunOrigin,
    started_at_unix_ns: &'a str,
    elapsed_us: u64,
    provenance: &'a VerificationProvenance,
    budgets: &'a VerificationBudgets,
    selected_subjects: &'a BTreeSet<String>,
    required_subjects: &'a BTreeSet<String>,
    not_selected: &'a [String],
    required_cases: &'static [String],
    cases: &'static [VerificationCaseRecord],
    case_count: usize,
    required_case_count: usize,
    case_inventory_complete: bool,
    case_subjects: BTreeSet<&'a str>,
    case_details_encoding: &'static str,
    case_details: String,
    open_obligations: &'a [VerificationIssue],
    bounded_obligations: &'a [String],
    artifacts: &'a [VerificationArtifactRef],
    result: &'a VerificationResult,
}
#[derive(Serialize)]
struct StoredRunView<'a> {
    id: &'a str,
    created_at: &'a str,
    run: RunView<'a>,
}
#[derive(Serialize)]
struct RecordsView<'a> {
    revision: &'a dctx0_store::SiteRevision,
    snapshot: &'a Snapshot,
    check: &'a HistoryEntry,
    tasks: &'a Option<dctx0_tasks::Session>,
    runs: Vec<StoredRunView<'a>>,
    history: &'a [HistoryEntry],
    renames: &'a [dctx0_changes::RenameEvent],
    changes: &'a Option<Changes>,
    before: &'a Option<Snapshot>,
    task_snapshot: &'a Option<Snapshot>,
    requirement_snapshots: &'a std::collections::BTreeMap<String, Snapshot>,
    plan_diff: &'a Option<dctx0_tasks::PlanDiff>,
}
#[derive(Serialize)]
struct CaseCorpus<'a> {
    cases: &'a [VerificationCaseRecord],
    required_cases: &'a BTreeSet<String>,
}
fn packed_cases(
    cases: &[VerificationCaseRecord],
    required_cases: &BTreeSet<String>,
) -> Result<String> {
    let mut value = serde_json::to_value(CaseCorpus {
        cases,
        required_cases,
    })?;
    decimal_integers(&mut value);
    let mut encoder = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::default());
    serde_json::to_writer(&mut encoder, &value)?;
    let bytes = encoder.finish()?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}
pub(super) fn records_value(records: &SiteRecords) -> Result<serde_json::Value> {
    let mut runs = Vec::with_capacity(records.runs.len());
    for stored in &records.runs {
        let run = &stored.run;
        runs.push(StoredRunView {
            id: &stored.id,
            created_at: &stored.created_at,
            run: RunView {
                schema_version: run.schema_version,
                origin: &run.origin,
                started_at_unix_ns: &run.started_at_unix_ns,
                elapsed_us: run.elapsed_us,
                provenance: &run.provenance,
                budgets: &run.budgets,
                selected_subjects: &run.selected_subjects,
                required_subjects: &run.required_subjects,
                not_selected: &run.not_selected,
                required_cases: &[],
                cases: &[],
                case_count: run.cases.len(),
                required_case_count: run.required_cases.len(),
                case_inventory_complete: run
                    .cases
                    .iter()
                    .map(|c| c.id.clone())
                    .collect::<BTreeSet<_>>()
                    == run.required_cases,
                case_subjects: run.cases.iter().map(|c| c.subject.as_str()).collect(),
                case_details_encoding: "gzip-base64-json-decimal-integers-v2",
                case_details: packed_cases(&run.cases, &run.required_cases)?,
                open_obligations: &run.open_obligations,
                bounded_obligations: &run.bounded_obligations,
                artifacts: &run.artifacts,
                result: &run.result,
            },
        });
    }
    Ok(serde_json::to_value(RecordsView {
        revision: &records.revision,
        snapshot: &records.snapshot,
        check: &records.check,
        tasks: &records.tasks,
        runs,
        history: &records.history,
        renames: &records.renames,
        changes: &records.changes,
        before: &records.before,
        task_snapshot: &records.task_snapshot,
        requirement_snapshots: &records.requirement_snapshots,
        plan_diff: &records.plan_diff,
    })?)
}
