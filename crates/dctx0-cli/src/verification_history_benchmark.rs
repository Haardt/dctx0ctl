//! Warm immutable report persistence/queries, not native execution evidence.
use crate::args::Phase;
use dctx0_changes::HistoryWriter;
use dctx0_core::*;
use dctx0_store::{AccessMode, Store, VerificationArtifact};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(crate) fn benchmark(
    sources: &[Source],
    ir: &WorkspaceIr,
    phase: Phase,
    iterations: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let root = std::env::temp_dir().join(format!(
        "dctx0-verification-history-bench-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).map_err(crate::error_message)?;
    let result = (|| {
        let snapshot = dctx0_changes::snapshot(sources, ir.clone(), dctx0_graph::build(ir));
        let mut store = Store::open(&root.join("state.sqlite3"), AccessMode::ReadWrite)
            .map_err(crate::error_message)?;
        let _publication = store
            .publish("benchmark", &snapshot, None)
            .map_err(crate::error_message)?;
        let (occurrence, _) = store
            .task_check("benchmark")
            .map_err(crate::error_message)?;
        let subject = ir
            .symbols
            .keys()
            .next()
            .ok_or("Benchmark needs a declared subject")?;
        let plan = dctx0_verification::project_program(
            ir,
            &snapshot.id,
            occurrence as u64,
            dctx0_verification::VerificationScope {
                subjects: BTreeSet::from([subject.clone()]),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let artifact = VerificationArtifact::new(
            "application/json",
            serde_json::to_vec(&plan).map_err(crate::error_message)?,
        );
        let issue = serde_json::json!({"code":"BENCHMARK_NOT_EXECUTION","message":"Only report persistence is measured; no native application invocation","span":null});
        let unknown = serde_json::json!({"status":"UNKNOWN","issue":issue});
        let mut run:VerificationRun=serde_json::from_value(serde_json::json!({"schema_version":1,"origin":"imported","started_at_unix_ns":"0","elapsed_us":0,"provenance":{"snapshot":snapshot.id,"check_occurrence":occurrence,"plan_hash":artifact.reference.hash,"implementation":{},"dependencies":{},"adapter_hash":"","configuration_hash":"","harness_hash":"","interpreter_version":"unexecuted","generator_version":plan.generator_version,"protocol_version":1,"target":"typescript","runtime_version":"not-executed","build_configuration":{},"task_pin":null,"incomplete":[issue]},"budgets":VerificationBudgets::default(),"selected_subjects":[subject],"required_subjects":[subject],"not_selected":[],"required_cases":["not-executed"],"cases":[],"open_obligations":[issue],"bounded_obligations":[],"artifacts":[artifact.reference],"result":unknown})).map_err(crate::error_message)?;
        if matches!(
            phase,
            Phase::VerificationHistoryLargePublish
                | Phase::VerificationHistoryLargeRead
                | Phase::VerificationHistorySummary
        ) {
            // Synthetic storage pressure only; no claim of executed application checks.
            for index in 0..10_000 {
                let id = format!("storage-{index}");
                run.required_cases.insert(id.clone());
                run.cases.push(VerificationCaseRecord {
                    id,
                    subject: subject.clone(),
                    kind: VerificationCaseKind::Contract,
                    bindings: [(
                        "payload".into(),
                        VerificationValue::String("storage-fixture-".repeat(1500)),
                    )]
                    .into(),
                    setup: vec![],
                    arguments: vec![],
                    observations: vec![],
                    events: None,
                    resources: None,
                    states: vec![],
                    result: serde_json::from_value(unknown.clone())
                        .map_err(crate::error_message)?,
                    elapsed_us: 0,
                });
            }
        }
        let saved = store
            .record_verification_run_owned("benchmark", run, std::slice::from_ref(&artifact))
            .map_err(crate::error_message)?;
        let saved_id = saved.id;
        let mut corpus = Some(saved.run);
        let publish = matches!(
            phase,
            Phase::VerificationHistoryPublish | Phase::VerificationHistoryLargePublish
        );
        if !publish {
            drop(corpus.take());
        }
        let start = std::time::Instant::now();
        for index in 0..iterations {
            if publish {
                let mut next = corpus.take().ok_or("Missing benchmark corpus")?;
                next.started_at_unix_ns = (index as u128 + 1).to_string();
                let saved = store
                    .record_verification_run_owned(
                        "benchmark",
                        next,
                        std::slice::from_ref(&artifact),
                    )
                    .map_err(crate::error_message)?;
                corpus = Some(saved.run);
            } else if matches!(phase, Phase::VerificationHistorySummary) {
                std::hint::black_box(
                    store
                        .verification_run_summaries("benchmark", 0, 50)
                        .map_err(crate::error_message)?,
                );
            } else {
                std::hint::black_box(
                    store
                        .verification_run("benchmark", &saved_id)
                        .map_err(crate::error_message)?,
                );
            }
        }
        crate::benchmark::benchmark_report(output, start, iterations, sources.len())
    })();
    let cleanup = fs::remove_dir_all(root).map_err(crate::error_message);
    result.and_then(|value| cleanup.map(|()| value))
}
