//! CLI options for editable native representation scaffolds.
use clap::Args;
use dctx0_core::{CodecOptions, CodecRepresentation};
use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(crate) struct CodecScaffoldArgs {
    input: PathBuf,
    #[arg(long)]
    project: PathBuf,
    #[arg(long, default_value = "typescript")]
    target: String,
    #[arg(long)]
    subject: Vec<String>,
    #[arg(long, value_parser = ["number", "bigint"])]
    integer: Option<String>,
    #[arg(long, value_parser = ["null", "undefined"])]
    option: Option<String>,
    #[arg(long = "enum", value_parser = ["string", "tagged"])]
    enumeration: Option<String>,
    /// Per-qualified-type representation: NAME=number/bigint/null/undefined/string/tagged.
    #[arg(long = "type")]
    types: Vec<String>,
    #[arg(long)]
    config: Option<PathBuf>,
    /// Separate native basename beside the contract; existing files are preserved.
    #[arg(long)]
    output: Option<String>,
    /// Save an inert comparison proposal with a .txt suffix.
    #[arg(long)]
    candidate: Option<String>,
    #[arg(long)]
    dry_run: bool,
}
fn representation(text: &str) -> Result<CodecRepresentation, String> {
    serde_json::from_value(serde_json::Value::String(text.into())).map_err(crate::error_message)
}
impl CodecScaffoldArgs {
    fn configuration(&self) -> Result<CodecOptions, String> {
        let mut options = match &self.config {
            Some(path) => dctx0_engine::read_review_input(path, 1_048_576, "Codec configuration")
                .map_err(crate::error_message)?,
            None => CodecOptions::default(),
        };
        for (text, slot) in [
            (&self.integer, &mut options.integer),
            (&self.option, &mut options.option),
            (&self.enumeration, &mut options.r#enum),
        ] {
            if let Some(text) = text {
                *slot = representation(text)?;
            }
        }
        let mut seen = BTreeSet::new();
        for override_ in &self.types {
            let (name, value) = override_
                .split_once('=')
                .filter(|(name, value)| !name.is_empty() && !value.is_empty())
                .ok_or("Use --type QUALIFIED_NAME=REPRESENTATION")?;
            if !seen.insert(name) {
                return Err(format!("Duplicate codec type override: {name}"));
            }
            options.types.insert(name.into(), representation(value)?);
        }
        Ok(options)
    }
}
pub(crate) fn run(
    args: CodecScaffoldArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let options = args.configuration()?;
    let enabled = (args.target == "typescript" && cfg!(feature = "typescript"))
        || (args.target == "go" && cfg!(feature = "go"))
        || (args.target == "rust" && cfg!(feature = "rust"));
    if !enabled {
        return Err(format!("No codec generator enabled for {}", args.target));
    }
    let project =
        dctx0_engine::Project::new(&args.input, database).map_err(crate::error_message)?;
    let inputs = project
        .codec_inputs(&args.subject.into_iter().collect())
        .map_err(crate::error_message)?;
    let (artifact, version) = generate(
        &args.target,
        &inputs.declarations,
        &options,
        &inputs.snapshot,
    )?;
    let filename = args.output.unwrap_or_else(|| artifact.path.clone());
    let filename = match args.candidate {
        Some(name) => format!("{name}.{filename}.txt"),
        None => filename,
    };
    let report = dctx0_engine::publish_target_codec_scaffold(
        &args.project,
        &args.target,
        &filename,
        artifact,
        &inputs,
        options,
        version,
        args.dry_run,
    )
    .map_err(crate::error_message)?;
    if json {
        crate::render::json(output, &report)?;
    } else {
        writeln!(output, "Codec scaffold: {} ({})\nSnapshot: {}\nRepresentation conversion only; production validation and native proof remain separate.", report.path, report.status, report.snapshot).map_err(crate::error_message)?;
        if report.dry_run {
            writeln!(output, "\n{}", report.artifact.contents).map_err(crate::error_message)?;
        }
    }
    Ok(true)
}

pub(crate) fn benchmark(
    ir: &dctx0_core::WorkspaceIr,
    iterations: usize,
    files: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    #[cfg(feature = "typescript")]
    {
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let declarations = dctx0_core::codec_declarations(ir, &BTreeSet::new())
                .map_err(crate::error_message)?;
            std::hint::black_box(
                dctx0_target_typescript::generate_codecs(
                    &declarations,
                    &CodecOptions::default(),
                    "benchmark",
                )
                .map_err(crate::error_message)?,
            );
        }
        crate::benchmark::benchmark_report(output, start, iterations, files)
    }
    #[cfg(not(feature = "typescript"))]
    {
        let _ = (ir, iterations, files, output);
        Err("Codec scaffolding requires the TypeScript target feature".into())
    }
}

/// Target dispatch keeps the command and binding scaffolds on the same generators.
pub(crate) fn generate(
    target: &str,
    declarations: &std::collections::BTreeMap<String, dctx0_core::DataIr>,
    options: &CodecOptions,
    snapshot: &str,
) -> Result<(dctx0_target_sdk::Artifact, &'static str), String> {
    let _ = (declarations, options, snapshot);
    match target {
        #[cfg(feature = "typescript")]
        "typescript" => dctx0_target_typescript::generate_codecs(declarations, options, snapshot)
            .map(|a| (a, dctx0_target_typescript::CODEC_GENERATOR_VERSION))
            .map_err(crate::error_message),
        #[cfg(feature = "go")]
        "go" => dctx0_target_go::generate_codecs(declarations, options, snapshot)
            .map(|a| (a, dctx0_target_go::CODEC_GENERATOR_VERSION))
            .map_err(crate::error_message),
        #[cfg(feature = "rust")]
        "rust" => dctx0_target_rust::generate_codecs(declarations, options, snapshot)
            .map(|a| (a, dctx0_target_rust::CODEC_GENERATOR_VERSION))
            .map_err(crate::error_message),
        _ => Err(format!("No codec generator enabled for {target}")),
    }
}
