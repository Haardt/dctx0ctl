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
struct PackedJsonView {
    encoding: &'static str,
    data: String,
}
#[derive(Serialize)]
struct StateView {
    #[serde(flatten)]
    summary: dctx0_tasks::State,
    details: PackedJsonView,
}
#[derive(Serialize)]
struct SessionView<'a> {
    project: &'a str,
    plan: &'a dctx0_tasks::Plan,
    event_seq: i64,
    states: std::collections::BTreeMap<i64, StateView>,
    definitions: &'a std::collections::BTreeMap<i64, dctx0_tasks::Definition>,
    links: &'a [dctx0_tasks::HistoryLink],
    events: &'a [dctx0_tasks::Event],
}
#[derive(Serialize)]
pub(super) struct RecordsView<'a> {
    revision: &'a dctx0_store::SiteRevision,
    snapshot: &'a Snapshot,
    check: &'a HistoryEntry,
    tasks: Option<SessionView<'a>>,
    runs: Vec<StoredRunView<'a>>,
    run_summaries: &'a [dctx0_store::VerificationRunSummary],
    history: &'a [HistoryEntry],
    renames: &'a [dctx0_changes::RenameEvent],
    changes: &'a Option<Changes>,
    before: &'a Option<Snapshot>,
    task_snapshot: &'a Option<Snapshot>,
    requirement_snapshots: std::collections::BTreeMap<&'a str, PackedJsonView>,
    history_scope: dctx0_store::SiteHistory,
    omitted_requirement_snapshots: &'a BTreeSet<String>,
    plan_diff: &'a Option<dctx0_tasks::PlanDiff>,
}
fn packed_browser_json(corpus: &[u8], compression: flate2::Compression) -> Result<String> {
    let reader = std::io::BufReader::new(flate2::read::GzDecoder::new(corpus));
    let mut de = serde_json::Deserializer::from_reader(reader);
    de.disable_recursion_limit();
    let value = serde_json::Value::deserialize(serde_stacker::Deserializer::new(&mut de))?;
    de.end()?;
    packed_value(&value, compression)
}
fn packed_value(value: &impl Serialize, compression: flate2::Compression) -> Result<String> {
    let encoder = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), compression);
    let mut writer = std::io::BufWriter::new(encoder);
    json::write(&mut writer, value)?;
    let encoder = writer.into_inner().map_err(|error| error.into_error())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(encoder.finish()?))
}
fn session_view<'a>(
    session: &'a dctx0_tasks::Session,
    progress: &mut dyn FnMut(SiteProgress),
) -> Result<SessionView<'a>> {
    let mut states = std::collections::BTreeMap::new();
    let total = session.states.len();
    progress(SiteProgress::PackingTasks {
        completed: 0,
        total,
    });
    for (id, state) in &session.states {
        let summary = dctx0_tasks::State {
            execution_bundle: None,
            work: state.work,
            version: state.version,
            actor: state.actor.clone(),
            claim: state.claim,
            reservation: state.reservation,
            result: state.result.clone(),
            check: state.check,
            evidence: Vec::new(),
            reviews: Vec::new(),
            ever_started: state.ever_started,
        };
        states.insert(
            *id,
            StateView {
                summary,
                details: PackedJsonView {
                    encoding: "gzip-base64-json-decimal-integers-v1",
                    data: packed_value(state, flate2::Compression::fast())?,
                },
            },
        );
        progress(SiteProgress::PackingTasks {
            completed: states.len(),
            total,
        });
    }
    Ok(SessionView {
        project: &session.project,
        plan: &session.plan,
        event_seq: session.event_seq,
        states,
        definitions: &session.definitions,
        links: &session.links,
        events: &session.events,
    })
}
pub(super) fn records_view<'a>(
    records: &'a SiteRecords,
    progress: &mut dyn FnMut(SiteProgress),
) -> Result<RecordsView<'a>> {
    // Convert exact browser integers one historical snapshot at a time. Keeping
    // these compressed also prevents the browser from parsing all old IRs at boot.
    let trace = std::env::var_os("DCTX0_SITE_TRACE").is_some();
    if trace {
        eprintln!(
            "site: packing {} historical snapshots",
            records.requirement_snapshots.len()
        );
    }
    let mut requirement_snapshots = std::collections::BTreeMap::new();
    let total = records.requirement_snapshots.len();
    progress(SiteProgress::PackingHistory {
        completed: 0,
        total,
    });
    for (id, compressed) in &records.requirement_snapshots {
        requirement_snapshots.insert(
            id.as_str(),
            PackedJsonView {
                encoding: "gzip-base64-json-decimal-integers-v1",
                data: packed_browser_json(compressed, flate2::Compression::fast())?,
            },
        );
        progress(SiteProgress::PackingHistory {
            completed: requirement_snapshots.len(),
            total,
        });
    }
    if trace {
        eprintln!(
            "site: historical snapshots packed ({} bytes)",
            requirement_snapshots
                .values()
                .map(|value| value.data.len())
                .sum::<usize>()
        );
    }
    let mut runs = Vec::with_capacity(records.runs.len());
    let total = records.runs.len();
    progress(SiteProgress::PackingRuns {
        completed: 0,
        total,
    });
    for stored in &records.runs {
        let run = &stored.header;
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
                case_count: stored.summary.case_count,
                required_case_count: stored.summary.required_case_count,
                case_inventory_complete: stored.summary.case_inventory_complete,
                case_subjects: stored
                    .summary
                    .case_subjects
                    .iter()
                    .map(String::as_str)
                    .collect(),
                case_details_encoding: "gzip-base64-json-decimal-integers-v2",
                case_details: packed_browser_json(&stored.corpus, flate2::Compression::default())?,
                open_obligations: &run.open_obligations,
                bounded_obligations: &run.bounded_obligations,
                artifacts: &run.artifacts,
                result: &run.result,
            },
        });
        progress(SiteProgress::PackingRuns {
            completed: runs.len(),
            total,
        });
    }
    if trace {
        eprintln!("site: packing task evidence");
    }
    let tasks = records
        .tasks
        .as_ref()
        .map(|session| session_view(session, progress))
        .transpose()?;
    if trace {
        eprintln!(
            "site: task evidence packed ({} bytes)",
            tasks.as_ref().map_or(0, |session| session
                .states
                .values()
                .map(|state| state.details.data.len())
                .sum::<usize>())
        );
    }
    Ok(RecordsView {
        revision: &records.revision,
        snapshot: &records.snapshot,
        check: &records.check,
        tasks,
        runs,
        run_summaries: &records.run_summaries,
        history: &records.history,
        renames: &records.renames,
        changes: &records.changes,
        before: &records.before,
        task_snapshot: &records.task_snapshot,
        requirement_snapshots,
        history_scope: records.history_scope,
        omitted_requirement_snapshots: &records.omitted_requirement_snapshots,
        plan_diff: &records.plan_diff,
    })
}
