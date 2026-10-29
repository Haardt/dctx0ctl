//! Opt-in collection, import and queries for test-impact research.
use clap::{Args, Subcommand};
use dctx0_engine::{AccessMode, Project};
use dctx0_store::CoverageExecution;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Args)]
pub(crate) struct ExperimentalArgs {
    #[command(subcommand)]
    command: Experiment,
}
#[derive(Subcommand)]
enum Experiment {
    TestAnalysis {
        #[command(subcommand)]
        command: TestAnalysis,
    },
    /// Observed project-test candidates; never skips tests or publishes task PASS.
    Coverage {
        #[command(subcommand)]
        command: Coverage,
    },
}
#[derive(Subcommand)]
enum Coverage {
    Catalog {
        input: PathBuf,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    Collection {
        input: PathBuf,
        id: String,
    },
    /// Suggest project tests overlapping Git changes. Read-only; never skips tests.
    Affected {
        input: PathBuf,
        #[arg(long, default_value = "HEAD")]
        base: String,
    },
    /// Collect Linux Rust libtest executions using llvm-tools-preview and Python 3.
    Collect {
        input: PathBuf,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Import collector JSON atomically; all original LLVM export data is retained.
    Import {
        input: PathBuf,
        report: PathBuf,
    },
    /// List historical executions, optionally matching an exact LLVM source filename/line.
    List {
        input: PathBuf,
        #[arg(long)]
        file: Option<String>,
        #[arg(long, requires = "file")]
        line: Option<u32>,
        #[arg(long, default_value="20", value_parser=clap::value_parser!(u32).range(1..=1000))]
        limit: u32,
    },
}
pub(crate) fn run(
    args: ExperimentalArgs,
    database: Option<&Path>,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let command = match args.command {
        Experiment::TestAnalysis { command } => return analysis(command, database, output),
        Experiment::Coverage { command } => command,
    };
    match command {
        Coverage::Affected { input, base } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = dctx0_engine::affected_project_tests(&project, &base)?;
            crate::render::json(output, &report)?;
            Ok(true)
        }
        Coverage::List {
            input,
            file,
            line,
            limit,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let records = project
                .store(AccessMode::ReadOnly)
                .map_err(crate::error_message)?
                .coverage_records(&project.key(), file.as_deref(), line, limit)
                .map_err(crate::error_message)?;
            crate::render::json(output, &records)?;
            Ok(true)
        }
        Coverage::Import { input, report } => {
            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(report).map_err(crate::error_message)?)
                    .map_err(crate::error_message)?;
            if report.get("execution_files").is_some() {
                let project = Project::new(&input, database).map_err(crate::error_message)?;
                publish_files(&project, report, output)
            } else {
                publish(&input, database, report, output)
            }
        }
        Coverage::Catalog { input, scope } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let timeout = scope.timeout_secs;
            let report =
                dctx0_engine::research_collect(&project, scope.into_scope(), timeout, true)?;
            crate::render::json(output, &report)?;
            Ok(true)
        }
        Coverage::Collection { input, id } => show(&input, database, &id, output),
        Coverage::Collect { input, scope } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let timeout = scope.timeout_secs;
            let report =
                dctx0_engine::research_collect(&project, scope.into_scope(), timeout, false)?;
            let passed = report["all_passed"].as_bool().unwrap_or(false);
            crate::render::json(output, &report)?;
            Ok(passed)
        }
    }
}
fn publish(
    input: &Path,
    database: Option<&Path>,
    report: serde_json::Value,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let executions: Vec<CoverageExecution> =
        serde_json::from_value(report["executions"].clone()).map_err(crate::error_message)?;
    if executions.is_empty() {
        return Err("Coverage report contains no executions".into());
    }
    let passed = executions.iter().all(|e| e.outcome == "pass");
    let project = Project::new(input, database).map_err(crate::error_message)?;
    let ids = project
        .store(AccessMode::ReadWrite)
        .map_err(crate::error_message)?
        .record_coverage(&project.key(), &executions)
        .map_err(crate::error_message)?;
    crate::render::json(
        output,
        &serde_json::json!({"experimental":true,"execution_ids":ids,"all_passed":passed,
        "collector_children_peak_rss_bytes":report["collector_children_peak_rss_bytes"],"measurement_scope":report["measurement_scope"]}),
    )?;
    Ok(passed)
}

fn publish_files(
    project: &Project,
    report: serde_json::Value,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let paths: Vec<PathBuf> =
        serde_json::from_value(report["execution_files"].clone()).map_err(crate::error_message)?;
    if paths.is_empty() {
        return Err("Coverage report contains no executions".into());
    }
    let mut passed = true;
    let records = paths
        .iter()
        .map(|path| -> dctx0_store::Result<CoverageExecution> {
            let record: CoverageExecution = serde_json::from_slice(&std::fs::read(path)?)?;
            passed &= record.outcome == "pass";
            Ok(record)
        });
    let ids = project
        .store(AccessMode::ReadWrite)
        .map_err(crate::error_message)?
        .record_coverage_stream(&project.key(), records)
        .map_err(crate::error_message)?;
    crate::render::json(
        output,
        &serde_json::json!({"experimental":true,"execution_ids":ids,"all_passed":passed,
        "collector_children_peak_rss_bytes":report["collector_children_peak_rss_bytes"],"measurement_scope":report["measurement_scope"]}),
    )?;
    Ok(passed)
}

#[derive(Args)]
struct ScopeArgs {
    #[arg(long)]
    package: Option<String>,
    #[arg(long)]
    filter: Option<String>,
    #[arg(long)]
    all_features: bool,
    #[arg(long)]
    no_default_features: bool,
    #[arg(long, value_delimiter = ',')]
    features: Vec<String>,
    #[arg(long, default_value="300", value_parser=clap::value_parser!(u64).range(1..))]
    timeout_secs: u64,
}
impl ScopeArgs {
    fn into_scope(self) -> dctx0_test_analysis::RunScope {
        dctx0_test_analysis::RunScope {
            package: self.package,
            filter: self.filter,
            all_features: self.all_features,
            no_default_features: self.no_default_features,
            features: self.features.into_iter().collect(),
            ..Default::default()
        }
    }
}
#[derive(Subcommand)]
enum TestAnalysis {
    Recompute {
        dataset: PathBuf,
        #[arg(long,default_value="evaluation",value_parser=["prediction","evaluation"])]
        stage: String,
    },
    Shadow {
        input: PathBuf,
        #[arg(long)]
        base: String,
        #[arg(long)]
        baseline: String,
        #[arg(long, default_value="coverage", value_parser=["coverage"])]
        strategy: String,
    },
    Replay {
        input: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        resume: Option<String>,
    },
    Show {
        input: PathBuf,
        id: String,
    },
    Export {
        input: PathBuf,
        id: String,
        #[arg(long)]
        output: PathBuf,
    },
}
fn show(
    input: &Path,
    database: Option<&Path>,
    id: &str,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let project = Project::new(input, database).map_err(crate::error_message)?;
    let records = dctx0_engine::research_records(&project)?;
    let record = records.iter().find(|r| r.id() == id);
    let events = if project.database.exists() {
        project
            .store(AccessMode::ReadOnly)
            .map_err(crate::error_message)?
            .analysis_events(&project.key(), id)
            .map_err(crate::error_message)?
    } else {
        vec![]
    };
    if record.is_none() && events.is_empty() {
        return Err("Research record not found".into());
    }
    crate::render::json(
        output,
        &serde_json::json!({"record":record,"events":events,"records":records}),
    )?;
    Ok(true)
}
fn analysis(
    command: TestAnalysis,
    database: Option<&Path>,
    output: &mut dyn Write,
) -> Result<bool, String> {
    match command {
        TestAnalysis::Recompute { dataset, stage } => {
            let report = dctx0_engine::research_recompute(&dataset, &stage)?;
            crate::render::json(output, &report)?;
            Ok(true)
        }
        TestAnalysis::Show { input, id } => show(&input, database, &id, output),
        TestAnalysis::Shadow {
            input,
            base,
            baseline,
            strategy: _,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = dctx0_engine::research_shadow(&project, &base, &baseline)?;
            let complete = report.completeness.reference_evaluable;
            crate::render::json(output, &report)?;
            Ok(complete)
        }
        TestAnalysis::Replay {
            input,
            manifest,
            resume,
        } => {
            let manifest =
                serde_json::from_slice(&std::fs::read(manifest).map_err(crate::error_message)?)
                    .map_err(crate::error_message)?;
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = dctx0_engine::research_replay(&project, manifest, resume.as_deref())?;
            let complete = report
                .events
                .iter()
                .filter(|e| e.kind.ends_with("_complete"))
                .count()
                == report.manifest.pairs.len();
            crate::render::json(output, &report)?;
            Ok(complete)
        }
        TestAnalysis::Export {
            input,
            id,
            output: destination,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            dctx0_engine::research_export(&project, &id, &destination)?;
            crate::render::json(
                output,
                &serde_json::json!({"output":destination,"schema_version":1}),
            )?;
            Ok(true)
        }
    }
}
