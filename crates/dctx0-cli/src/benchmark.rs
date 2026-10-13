use crate::{args::Phase, error_message, metrics::peak_rss, render::json, sources::sources};
use dctx0_changes::HistoryWriter;
use dctx0_engine::SourceReader;
use dctx0_engine::{AccessMode, Project};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

pub(crate) fn benchmark(
    project: &Project,
    phase: Phase,
    iterations: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    if iterations == 0 {
        return Err("Iterations must be a positive integer".into());
    }
    if matches!(phase, Phase::SiteSnapshot) {
        let start = std::time::Instant::now();
        let mut files = 0;
        for _ in 0..iterations {
            let data = project
                .site_snapshot(&dctx0_store::SiteSelection::default())
                .map_err(error_message)?;
            files = data.records.snapshot.sources.len();
            std::hint::black_box(data);
        }
        return benchmark_report(output, start, iterations, files);
    }
    let sources = project.sources().map_err(error_message)?;
    let compiled = dctx0_language::compile(&sources);
    let Some(ir) = compiled.ir else {
        json(
            output,
            &serde_json::json!({"valid":false,"diagnostics":compiled.diagnostics}),
        )?;
        return Ok(false);
    };
    if matches!(
        phase,
        Phase::VerificationProject
            | Phase::VerificationGenerate
            | Phase::VerificationEvaluateUnbound
            | Phase::VerificationAssessUnbound
    ) {
        return crate::verification_benchmark::benchmark(
            &ir,
            phase,
            iterations,
            sources.len(),
            output,
        );
    }
    if matches!(
        phase,
        Phase::VerificationHistoryPublish | Phase::VerificationHistoryRead
    ) {
        return crate::verification_history_benchmark::benchmark(
            &sources, &ir, phase, iterations, output,
        );
    }
    if matches!(phase, Phase::VerificationInit) {
        return crate::verification_setup_benchmark::benchmark(&sources, iterations, output);
    }
    if matches!(phase, Phase::VerificationDiscover) {
        return crate::verification_discovery_benchmark::benchmark(project, iterations, output);
    }
    if matches!(phase, Phase::VerificationHarness) {
        return crate::harness_benchmark::benchmark(&ir, iterations, sources.len(), output);
    }
    if matches!(phase, Phase::Compile) {
        drop(ir);
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(dctx0_language::compile(&sources));
        }
        return benchmark_report(output, start, iterations, sources.len());
    }
    if matches!(phase, Phase::Graph) {
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(dctx0_graph::build(&ir));
        }
        return benchmark_report(output, start, iterations, sources.len());
    }
    let graph = dctx0_graph::build(&ir);
    if matches!(phase, Phase::Snapshot) {
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(dctx0_changes::snapshot(&sources, ir.clone(), graph.clone()));
        }
        return benchmark_report(output, start, iterations, sources.len());
    }
    let baseline = dctx0_changes::snapshot(&sources, ir, graph);
    if matches!(
        phase,
        Phase::TasksPlan
            | Phase::TasksReconcile
            | Phase::TasksPublish
            | Phase::TasksList
            | Phase::TasksContextMd
            | Phase::TasksContextJson
            | Phase::TasksSlice
            | Phase::TasksClaim
    ) {
        return task_benchmark(project, phase, iterations, output, &baseline);
    }

    let persists = matches!(phase, Phase::Persist | Phase::Check | Phase::CheckChanged);
    if persists && project.database == project.root.join(".dctx0ctl/state.sqlite3") {
        return Err("Persistence benchmarks require --db pointing to a temporary database".into());
    }
    let mut store = if persists {
        Some(
            project
                .store(AccessMode::ReadWrite)
                .map_err(error_message)?,
        )
    } else {
        None
    };
    let mut changed_sources = sources.clone();
    let changed = if matches!(phase, Phase::DiffChanged | Phase::CheckChanged) {
        changed_sources.push(dctx0_core::Source {
            file: "__benchmark_change.dctx0".into(),
            text: format!(
                "fragment {} {{ domain BenchmarkDelta {{ type Added = Int }} }}",
                baseline.ir.workspace
            ),
        });
        let result = dctx0_language::compile(&changed_sources);
        let changed_ir = result
            .ir
            .ok_or_else(|| format!("Benchmark scenario invalid: {:?}", result.diagnostics))?;
        let changed_graph = dctx0_graph::build(&changed_ir);
        Some(dctx0_changes::snapshot(
            &changed_sources,
            changed_ir,
            changed_graph,
        ))
    } else {
        None
    };
    if let Some(store) = &mut store {
        store
            .publish(&project.key(), &baseline, None)
            .map_err(error_message)?;
    }
    let start = std::time::Instant::now();
    for iteration in 0..iterations {
        if matches!(phase, Phase::Diff) {
            std::hint::black_box(dctx0_changes::diff(Some(&baseline), &baseline, &[]));
        } else if matches!(phase, Phase::DiffChanged) {
            std::hint::black_box(dctx0_changes::diff(
                Some(&baseline),
                changed.as_ref().unwrap(),
                &[],
            ));
        } else if matches!(phase, Phase::Persist) {
            std::hint::black_box(
                store
                    .as_mut()
                    .unwrap()
                    .publish(&project.key(), &baseline, None)
                    .map_err(error_message)?,
            );
        } else {
            let input = if matches!(phase, Phase::CheckChanged) && iteration % 2 == 0 {
                &changed_sources
            } else {
                &sources
            };
            let ir = dctx0_language::compile(input)
                .ir
                .expect("benchmark inputs prevalidated");
            let graph = dctx0_graph::build(&ir);
            let snapshot = dctx0_changes::snapshot(input, ir, graph);
            std::hint::black_box(
                store
                    .as_mut()
                    .unwrap()
                    .publish(&project.key(), &snapshot, None)
                    .map_err(error_message)?,
            );
        }
    }
    benchmark_report(output, start, iterations, sources.len())
}
pub(crate) fn benchmark_report(
    output: &mut dyn Write,
    start: std::time::Instant,
    iterations: usize,
    files: usize,
) -> Result<bool, String> {
    json(
        output,
        &serde_json::json!({"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"iterations":iterations,"files":files,"valid":true,"peak_rss_bytes":peak_rss(&fs::read_to_string("/proc/self/status").unwrap_or_default())}),
    )?;
    Ok(true)
}

pub(crate) fn bench_parse(
    input: &Path,
    iterations: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let mut valid = true;
    if iterations == 0 {
        return Err("Iterations must be a positive integer".into());
    }
    let mut files = Vec::new();
    sources(input, &mut files).map_err(error_message)?;
    files.sort();
    if files.is_empty() {
        return Err("No .dctx0 sources found".into());
    }
    let sources = files
        .iter()
        .map(|p| fs::read_to_string(p).map(|text| (p.display().to_string(), text)))
        .collect::<io::Result<Vec<_>>>()
        .map_err(error_message)?;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        for (file, text) in &sources {
            let result = dctx0_language::parse(file, text);
            valid &= result.diagnostics.is_empty();
            std::hint::black_box(result);
        }
    }
    json(
        output,
        &serde_json::json!({"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"iterations":iterations,"files":sources.len(),"valid":valid,"peak_rss_bytes":peak_rss(&fs::read_to_string("/proc/self/status").unwrap_or_default())}),
    )?;

    output.flush().map_err(error_message)?;
    Ok(valid)
}

fn task_benchmark(
    project: &Project,
    phase: Phase,
    iterations: usize,
    output: &mut dyn Write,
    snapshot: &dctx0_changes::Snapshot,
) -> Result<bool, String> {
    use dctx0_tasks::{Action, Filters};
    if project.database == project.root.join(".dctx0ctl/state.sqlite3") {
        return Err("Task benchmarks require --db pointing to a temporary database".into());
    }
    let candidate = dctx0_tasks::plan(snapshot).map_err(error_message)?;
    let mut store = project
        .store(AccessMode::ReadWrite)
        .map_err(error_message)?;
    store
        .publish(&project.key(), snapshot, None)
        .map_err(error_message)?;
    let (run, _) = store.task_check(&project.key()).map_err(error_message)?;
    let head = store.task_head(&project.key()).map_err(error_message)?;
    let seq = store.task_sequence(&project.key()).map_err(error_message)?;
    let continuity = snapshot
        .ir
        .symbols
        .keys()
        .map(|s| (s.clone(), s.clone()))
        .collect();
    let plan = store
        .publish_tasks(&project.key(), &candidate, run, head, seq, &continuity)
        .map_err(error_message)?;
    let session = store
        .task_session(&project.key(), Some(plan.id), None)
        .map_err(error_message)?;
    let anchor = session
        .plan
        .members
        .iter()
        .filter(|m| m.candidate.kind == dctx0_tasks::TaskKind::Operation)
        .max_by_key(|m| (m.candidate.types.len(), m.candidate.wave))
        .or_else(|| session.plan.members.first())
        .ok_or("Task benchmark requires at least one task")?
        .task_id;
    let mut changed = snapshot.clone();
    if let Some(node) = changed
        .ir
        .nodes
        .iter_mut()
        .find(|n| n.kind == dctx0_core::StatementKind::Intent)
    {
        if let Some(term) = node.terms.first_mut() {
            term.text.push_str(" Benchmark delta");
        }
    }
    let delta = dctx0_tasks::plan(&changed).map_err(error_message)?;
    let mut elapsed = std::time::Duration::ZERO;
    for iteration in 0..iterations {
        let mut published_candidate = candidate.clone();
        let publication = if matches!(phase, Phase::TasksPublish) {
            let mut sources = snapshot.sources.clone();
            sources[0]
                .text
                .push_str(&format!("\n// task publication sample {iteration}\n"));
            let variant =
                dctx0_changes::snapshot(&sources, snapshot.ir.clone(), snapshot.graph.clone());
            store
                .publish(&project.key(), &variant, None)
                .map_err(error_message)?;
            published_candidate.snapshot_id = variant.id.clone();
            Some(store.task_check(&project.key()).map_err(error_message)?.0)
        } else {
            None
        };
        let start = std::time::Instant::now();
        match phase {
            Phase::TasksPlan => {
                std::hint::black_box(dctx0_tasks::plan(snapshot).map_err(error_message)?);
            }
            Phase::TasksReconcile => {
                std::hint::black_box(
                    dctx0_tasks::reconcile(Some(&session), &delta, &continuity)
                        .map_err(error_message)?,
                );
            }
            Phase::TasksPublish => {
                let seq = store.task_sequence(&project.key()).map_err(error_message)?;
                std::hint::black_box(
                    store
                        .publish_tasks(
                            &project.key(),
                            &published_candidate,
                            publication.unwrap(),
                            store.task_head(&project.key()).map_err(error_message)?,
                            seq,
                            &continuity,
                        )
                        .map_err(error_message)?,
                );
            }
            Phase::TasksList => {
                std::hint::black_box(
                    session
                        .list(&Filters::default(), 50, None)
                        .map_err(error_message)?,
                );
            }
            Phase::TasksContextMd | Phase::TasksContextJson => {
                let context = dctx0_tasks::context(&session, snapshot, anchor, "core", 24576, None)
                    .map_err(error_message)?;
                if matches!(phase, Phase::TasksContextMd) {
                    std::hint::black_box(dctx0_tasks::context_markdown(&context));
                } else {
                    std::hint::black_box(serde_json::to_vec(&context).map_err(error_message)?);
                }
            }
            Phase::TasksSlice => {
                std::hint::black_box(session.slice(anchor, None, None).map_err(error_message)?);
            }
            _ => {
                let receipt = store
                    .task_action(
                        &project.key(),
                        Action::ClaimNext {
                            actor: "benchmark".into(),
                            filters: Filters::default(),
                        },
                    )
                    .map_err(error_message)?;
                let state = receipt.state.ok_or("Task benchmark has no ready task")?;
                store
                    .task_action(
                        &project.key(),
                        Action::Release {
                            task: receipt.task_id.unwrap(),
                            actor: "benchmark".into(),
                            claim: state.claim.unwrap(),
                            version: state.version,
                            reason: "Benchmark release".into(),
                        },
                    )
                    .map_err(error_message)?;
            }
        }
        elapsed += start.elapsed();
    }
    json(
        output,
        &serde_json::json!({"elapsed_ms":elapsed.as_secs_f64()*1000.0,"iterations":iterations,"files":snapshot.sources.len(),"valid":true,"peak_rss_bytes":peak_rss(&fs::read_to_string("/proc/self/status").unwrap_or_default())}),
    )?;
    Ok(true)
}
