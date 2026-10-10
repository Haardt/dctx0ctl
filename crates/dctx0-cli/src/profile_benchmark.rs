use crate::{benchmark::benchmark_report, error_message, profiles::provider};
use clap::ValueEnum;
use dctx0_engine::{Project, create_profiles};
use dctx0_tasks::{Action, Filters};
use std::{fs, io::Write, path::Path};
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ProfilePhase {
    Create,
    Load,
    ContextMd,
    ContextJson,
    Claim,
}
pub(crate) fn benchmark(
    project: &Project,
    target: &str,
    phase: ProfilePhase,
    iterations: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    if iterations == 0 {
        return Err("Iterations must be a positive integer".into());
    }
    if project.database == project.root.join(".dctx0ctl/state.sqlite3") {
        return Err("Profile benchmarks require --db pointing to a temporary database".into());
    }
    let provider = provider(target)?;
    let parent = project.database.parent().unwrap();
    let app = parent.join(format!("profile-benchmark-{}", std::process::id()));
    fs::create_dir_all(&app).map_err(error_message)?;
    create_profiles(&app, provider.as_ref(), false, false, false).map_err(error_message)?;
    if matches!(phase, ProfilePhase::Create | ProfilePhase::Load) {
        let start = std::time::Instant::now();
        for i in 0..iterations {
            if matches!(phase, ProfilePhase::Create) {
                let root = app.join(format!("create-{i}"));
                fs::create_dir_all(&root).map_err(error_message)?;
                std::hint::black_box(
                    create_profiles(&root, provider.as_ref(), false, false, false)
                        .map_err(error_message)?,
                );
            } else {
                std::hint::black_box(
                    project
                        .profiles(Some(&app), Some(target))
                        .map_err(error_message)?,
                );
            }
        }
        return benchmark_report(output, start, iterations, 0);
    }
    project.check().map_err(error_message)?;
    project.task_plan("latest").map_err(error_message)?;
    let profiles = project
        .profiles(Some(&app), Some(target))
        .map_err(error_message)?
        .unwrap();
    let session = project.task_session(None, None).map_err(error_message)?;
    let snapshot = project
        .load(&session.plan.snapshot_id)
        .map_err(error_message)?;
    let ready = session
        .next(&Filters::default(), None)
        .ok_or("No benchmark task available")?;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        if matches!(phase, ProfilePhase::Claim) {
            let receipt = project
                .task_action_with_profiles(
                    Action::ClaimNext {
                        actor: "benchmark".into(),
                        filters: Filters::default(),
                    },
                    Some(&profiles),
                )
                .map_err(error_message)?;
            let state = receipt.state.ok_or("No benchmark claim available")?;
            project
                .task_action(Action::Release {
                    task: receipt.task_id.unwrap(),
                    actor: "benchmark".into(),
                    claim: state.claim.unwrap(),
                    version: state.version,
                    reason: "Repeat benchmark".into(),
                })
                .map_err(error_message)?;
        } else {
            let selection = dctx0_tasks::select_profiles(
                &snapshot,
                &session
                    .member(ready.task_id)
                    .map_err(error_message)?
                    .candidate,
                &profiles.manifest,
                &profiles.documents,
            );
            let context = dctx0_tasks::context_with_profiles(
                &session,
                &snapshot,
                ready.task_id,
                "core",
                usize::MAX,
                None,
                Some(selection),
            )
            .map_err(error_message)?;
            if matches!(phase, ProfilePhase::ContextMd) {
                std::hint::black_box(dctx0_tasks::context_markdown(&context));
            } else {
                std::hint::black_box(serde_json::to_string(&context).map_err(error_message)?);
            }
        }
    }
    benchmark_report(output, start, iterations, snapshot.sources.len())
}
// Keep the caller responsible for a disposable database and application root.
pub(crate) fn run(
    input: &Path,
    database: Option<&Path>,
    target: &str,
    phase: ProfilePhase,
    iterations: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let project = Project::new(input, database).map_err(error_message)?;
    benchmark(&project, target, phase, iterations, output)
}
