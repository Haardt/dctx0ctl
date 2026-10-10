//! Isolated parser driver: does not link the history/CLI dispatch implementations.
mod metrics;
mod sources;
use metrics::peak_rss;
use sources::sources;
use std::{fs, path::Path, process::ExitCode};
fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn run(args: &[String]) -> Result<serde_json::Value, String> {
    if args.len() < 2 || args[0] != "bench-parse" {
        return Err("Usage: dctx0-parse-bench bench-parse <source> [iterations]".into());
    }
    let iterations = args
        .get(2)
        .map_or(Ok(1usize), |s| s.parse())
        .map_err(|_| "Invalid iteration count")?;
    if iterations == 0 {
        return Err("Invalid iteration count".into());
    }
    let mut files = Vec::new();
    sources(Path::new(&args[1]), &mut files).map_err(message)?;
    files.sort();
    if files.is_empty() {
        return Err("No .dctx0 sources".into());
    }
    let sources = files
        .into_iter()
        .map(|p| fs::read_to_string(&p).map(|text| (p.display().to_string(), text)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(message)?;
    let start = std::time::Instant::now();
    let mut valid = true;
    for _ in 0..iterations {
        for (file, text) in &sources {
            let result = dctx0_language::parse(file, text);
            valid &= result.diagnostics.is_empty();
            std::hint::black_box(result);
        }
    }
    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    let rss = peak_rss(&status);
    Ok(
        serde_json::json!({"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"iterations":iterations,"files":sources.len(),"valid":valid,"peak_rss_bytes":rss}),
    )
}
fn main() -> ExitCode {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(value) => {
            println!("{value}");
            if value["valid"] == true {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
