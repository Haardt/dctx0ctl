//! Explicit preparation and bounded read-only public SQL.
use clap::{Args, Subcommand};
use dctx0_engine::Project;
use dctx0_store::{AccessMode, QueryPins};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Args)]
pub(crate) struct QueryArgs {
    #[command(subcommand)]
    action: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Explicitly migrate/index retained verification and pinned spec history.
    Prepare {
        input: PathBuf,
        #[arg(long)]
        snapshot: Option<String>,
    },
    /// Describe the stable public view contract and examples.
    Schema { input: PathBuf },
    /// Execute exactly one read-only SELECT, including CTEs.
    Sql {
        input: PathBuf,
        #[arg(long, required_unless_present = "file", conflicts_with = "file")]
        sql: Option<String>,
        #[arg(long, required_unless_present = "sql")]
        file: Option<PathBuf>,
        #[arg(long)]
        param: Vec<String>,
        #[arg(long)]
        plan: Option<i64>,
        #[arg(long)]
        as_of: Option<i64>,
        #[arg(long)]
        snapshot: Option<String>,
        #[arg(long,default_value_t=100,value_parser=clap::value_parser!(u32).range(1..=10000))]
        max_rows: u32,
        #[arg(long,default_value_t=1048576,value_parser=clap::value_parser!(u64).range(1..=67108864))]
        max_output_bytes: u64,
        #[arg(long,default_value_t=5,value_parser=clap::value_parser!(u64).range(1..=60))]
        timeout_seconds: u64,
    },
}
pub(crate) fn run(
    args: QueryArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    let limit = match &args.action {
        Action::Sql {
            max_output_bytes, ..
        } => *max_output_bytes,
        _ => 1048576,
    };
    crate::render::with_output_limit(output, limit, |output| {
        run_inner(args, database, json, output)
    })
}
fn run_inner(
    args: QueryArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    match args.action {
        Action::Prepare { input, snapshot } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let _lock = project.lock().map_err(crate::error_message)?;
            let mut store = project
                .store(AccessMode::ReadWrite)
                .map_err(crate::error_message)?;
            let converted = store
                .prepare_verification_queries(&project.key())
                .map_err(crate::error_message)?;
            let mut snapshots = std::collections::BTreeSet::new();
            if let Some(snapshot) = snapshot {
                snapshots.insert(snapshot);
            }
            {
                snapshots.insert("latest".into());
                if store
                    .task_head(&project.key())
                    .map_err(crate::error_message)?
                    .is_some()
                {
                    let session = store
                        .task_session(&project.key(), None, None)
                        .map_err(crate::error_message)?;
                    snapshots.insert(session.plan.snapshot_id);
                }
                let mut offset = 0;
                loop {
                    let page = store
                        .verification_run_summaries(&project.key(), offset, 200)
                        .map_err(crate::error_message)?;
                    let count = page.len();
                    for run in page.into_iter().filter(|s| s.details_available) {
                        snapshots.insert(run.snapshot);
                    }
                    if count < 200 {
                        break;
                    }
                    offset += 200;
                }
            }
            for snapshot in &snapshots {
                store
                    .prepare_query_snapshot(&project.key(), snapshot)
                    .map_err(crate::error_message)?;
            }
            crate::render::json(
                output,
                &serde_json::json!({"view_version":dctx0_store::QUERY_VIEW_VERSION,"converted_runs":converted,"snapshots":snapshots}),
            )?;
        }
        Action::Schema { input } => {
            let _project = Project::new(&input, database).map_err(crate::error_message)?;
            let views = dctx0_store::QUERY_VIEWS
                .iter()
                .map(|(name, columns)| serde_json::json!({"name":name,"columns":columns}))
                .collect::<Vec<_>>();
            let schema = serde_json::json!({"view_version":dctx0_store::QUERY_VIEW_VERSION,"views":views,"relationships":["task_id joins task views","run_id joins runs, cases, requirements, artifacts, issues","snapshot and subject join spec views and source refs"],"availability":"Pinned history; pruned runs retain counts only. No working-tree freshness. Missing indexes require query prepare.","examples":["SELECT task_id,title,ready,check_status FROM dctx0_tasks WHERE work = 'Open' AND ready = 1 ORDER BY wave,task_id","SELECT subject,outcome,COUNT(*) AS cases FROM dctx0_cases WHERE run_id = :run_id AND outcome IN ('FAIL','UNKNOWN') GROUP BY subject,outcome ORDER BY subject,outcome"]});
            if json {
                crate::render::json(output, &schema)?;
            } else {
                writeln!(output,"Public query views v1 (prepare existing history with `dctx0 query prepare <spec>`)").map_err(crate::error_message)?;
                for (name, columns) in dctx0_store::QUERY_VIEWS {
                    writeln!(output, "\n{name}: {columns}").map_err(crate::error_message)?;
                }
            }
        }
        Action::Sql {
            input,
            sql,
            file,
            param,
            plan,
            as_of,
            snapshot,
            max_rows,
            max_output_bytes,
            timeout_seconds,
        } => {
            let sql = if let Some(sql) = sql {
                sql
            } else {
                let file = file.ok_or("Missing --sql or --file")?;
                let mut text = String::new();
                let mut reader: Box<dyn Read> = if file == Path::new("-") {
                    Box::new(std::io::stdin())
                } else {
                    Box::new(std::fs::File::open(file).map_err(crate::error_message)?)
                };
                reader
                    .by_ref()
                    .take(1048577)
                    .read_to_string(&mut text)
                    .map_err(crate::error_message)?;
                if text.len() > 1048576 {
                    return Err("SQL input exceeds 1 MiB".into());
                }
                text
            };
            let mut parameters = BTreeMap::new();
            for param in param {
                let (name, value) = param.split_once('=').ok_or("Expected --param name=JSON")?;
                if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err("Invalid parameter name".into());
                }
                if parameters
                    .insert(
                        name.into(),
                        serde_json::from_str(value).map_err(crate::error_message)?,
                    )
                    .is_some()
                {
                    return Err(format!("Duplicate parameter {name}"));
                }
            }
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .store(AccessMode::ReadOnly)
                .map_err(crate::error_message)?
                .query_sql(
                    &project.key(),
                    QueryPins {
                        plan,
                        as_of,
                        snapshot,
                    },
                    &sql,
                    &parameters,
                    dctx0_store::QueryLimits {
                        max_rows: max_rows as usize,
                        max_bytes: max_output_bytes as usize,
                        timeout_ms: timeout_seconds * 1000,
                    },
                )
                .map_err(crate::error_message)?;
            if json {
                crate::render::bounded_json(output, &report, max_output_bytes)?;
            } else {
                writeln!(
                    output,
                    "Views v{}; pins: {:?}; complete: {}",
                    report.view_version, report.pins, report.complete
                )
                .map_err(crate::error_message)?;
                writeln!(output, "{}", report.columns.join(" | ")).map_err(crate::error_message)?;
                for availability in &report.availability {
                    writeln!(output, "{availability}").map_err(crate::error_message)?;
                }
                for row in report.rows {
                    writeln!(
                        output,
                        "{}",
                        row.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" | ")
                    )
                    .map_err(crate::error_message)?;
                }
            }
        }
    }
    Ok(())
}
