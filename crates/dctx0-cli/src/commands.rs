use crate::{
    args::{Cli, Command, Phase},
    benchmark::{bench_parse, benchmark},
    error_message,
    render::{display_changes, json},
    sources::sources,
};
use clap::Parser;
use dctx0_engine::Project;
use std::{fs, io::Write, path::Path};

pub(crate) fn run(args: &[String], output: &mut dyn Write) -> Result<bool, String> {
    // Keep the established parser microbenchmark independent of the richer
    // command registry; end-to-end check benchmarks include normal dispatch.
    if args.first().is_some_and(|a| a == "bench-parse")
        && (2..=3).contains(&args.len())
        && !args.iter().any(|a| matches!(a.as_str(), "-h" | "--help"))
    {
        let iterations = args
            .get(2)
            .map_or(Ok(1usize), |s| s.parse())
            .map_err(|_| "Iterations must be a positive integer")?;
        return bench_parse(Path::new(&args[1]), iterations, output);
    }
    let cli = match Cli::try_parse_from(
        std::iter::once("dctx0".to_owned()).chain(args.iter().cloned()),
    ) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayVersion | clap::error::ErrorKind::DisplayHelp
            ) =>
        {
            write!(output, "{error}").map_err(error_message)?;
            return Ok(true);
        }
        Err(error) => return Err(error_message(error)),
    };
    let project = |path: &Path| Project::new(path, cli.db.as_deref()).map_err(error_message);
    let mut valid = true;
    match cli.command {
        Command::Lsp { project, stdio: _ } => {
            dctx0_lsp::serve_stdio(dctx0_lsp::ServerOptions { project })?
        }
        Command::Editor(args) => crate::editors::run(args, cli.json, output)?,
        Command::Skills(args) => crate::skills::run(args, cli.json, output)?,
        Command::Install(args) => crate::skills::install(args, cli.json, output)?,
        Command::Site(args) => crate::site::run(args, cli.db.as_deref(), output)?,
        Command::Verify(args) => {
            valid = crate::verification::run(args, cli.db.as_deref(), cli.json, output)?
        }
        Command::Profile(args) => crate::profiles::run(args, cli.json, output)?,
        Command::Tasks(args) => crate::tasks::run(*args, cli.db.as_deref(), cli.json, output)?,
        Command::Parse { input } => {
            let mut files = Vec::new();
            sources(&input, &mut files).map_err(error_message)?;
            files.sort();
            if files.is_empty() {
                return Err("No .dctx0 sources found".into());
            }
            for path in files {
                let file = path.display().to_string();
                let source = fs::read_to_string(&path).map_err(error_message)?;
                let result = dctx0_language::parse(&file, &source);
                valid &= result.diagnostics.is_empty();
                json(output, &result.ast)?;
                for diagnostic in &result.diagnostics {
                    writeln!(
                        output,
                        "{}:{}:{}: {}: {}",
                        file,
                        diagnostic.span.start.line,
                        diagnostic.span.start.column,
                        diagnostic.code,
                        diagnostic.message
                    )
                    .map_err(error_message)?;
                }
            }
        }
        Command::BenchParse { input, iterations } => {
            valid = bench_parse(&input, iterations, output)?;
        }
        Command::BenchCompile { input, iterations } => {
            valid = benchmark(&project(&input)?, Phase::Compile, iterations, output)?;
        }
        Command::BenchProfiles {
            input,
            target,
            phase,
            iterations,
        } => {
            valid = crate::profile_benchmark::run(
                &input,
                cli.db.as_deref(),
                &target,
                phase,
                iterations,
                output,
            )?;
        }
        Command::Bench {
            input,
            phase,
            iterations,
        } => {
            valid = benchmark(&project(&input)?, phase, iterations, output)?;
        }
        Command::Check { input } => {
            let report = project(&input)?.check().map_err(error_message)?;
            valid = report.ok;
            if cli.json {
                json(output, &report)?;
            } else {
                writeln!(
                    output,
                    "{}: {} symbols, {} operations; snapshot {}",
                    if report.ok { "PASS" } else { "FAIL" },
                    report.symbols,
                    report.operations,
                    report.snapshot_id.as_deref().unwrap_or("none")
                )
                .map_err(error_message)?;
                if let Some(changes) = &report.changes {
                    display_changes(output, changes)?;
                }
                for diagnostic in report.diagnostics {
                    writeln!(
                        output,
                        "{}:{}:{}: {}: {}",
                        diagnostic.span.file,
                        diagnostic.span.start.line,
                        diagnostic.span.start.column,
                        diagnostic.code,
                        diagnostic.message
                    )
                    .map_err(error_message)?;
                }
            }
        }
        Command::History { input } => {
            let history = project(&input)?.history().map_err(error_message)?;
            if cli.json {
                json(output, &history)?;
            } else {
                for run in history {
                    writeln!(
                        output,
                        "{} {} {} {}",
                        run.run_id,
                        run.timestamp,
                        run.status,
                        run.snapshot_id.as_deref().unwrap_or("none")
                    )
                    .map_err(error_message)?;
                }
            }
        }
        Command::Diff {
            input,
            before,
            after,
        } => {
            let changes = project(&input)?
                .diff(&before, &after)
                .map_err(error_message)?;
            if cli.json {
                json(output, &changes)?;
            } else {
                display_changes(output, &changes)?;
            }
        }
        Command::Impact {
            input,
            symbol,
            snapshot,
        } => {
            let snapshot = project(&input)?.load(&snapshot).map_err(error_message)?;
            if !snapshot.graph.nodes.contains_key(&symbol) {
                return Err(format!("Unknown symbol {symbol}"));
            }
            let impacts = snapshot.graph.impact(&symbol);
            if cli.json {
                json(output, &impacts)?;
            } else {
                for impact in impacts {
                    writeln!(
                        output,
                        "{} via {}",
                        impact.subject,
                        impact.path.join(" -> ")
                    )
                    .map_err(error_message)?;
                }
            }
        }
        Command::Rename {
            input,
            symbol,
            new_name,
            dry_run,
        } => {
            let report = project(&input)?
                .rename(&symbol, &new_name, dry_run)
                .map_err(error_message)?;
            if cli.json {
                json(output, &report)?;
            } else {
                writeln!(
                    output,
                    "{} {} -> {}: {} source edits",
                    if dry_run { "PREVIEW" } else { "RENAMED" },
                    report.symbol,
                    report.new_symbol,
                    report.edits.len()
                )
                .map_err(error_message)?;
                for edit in &report.edits {
                    writeln!(
                        output,
                        "{}:{}..{} {:?} -> {:?}",
                        edit.file, edit.start, edit.end, edit.old, edit.new
                    )
                    .map_err(error_message)?;
                }
                display_changes(output, &report.changes)?;
            }
        }
    }
    output.flush().map_err(error_message)?;
    Ok(valid)
}
