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
    /// Separate .ts basename beside contract.ts; existing files are preserved.
    #[arg(long, default_value = "wire-codecs.ts")]
    output: String,
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
    if args.target != "typescript" {
        return Err("Codec scaffolding currently supports only typescript".into());
    }
    #[cfg(feature = "typescript")]
    {
        let project =
            dctx0_engine::Project::new(&args.input, database).map_err(crate::error_message)?;
        let inputs = project
            .codec_inputs(&args.subject.into_iter().collect())
            .map_err(crate::error_message)?;
        let artifact = dctx0_target_typescript::generate_codecs(
            &inputs.declarations,
            &options,
            &inputs.snapshot,
        )
        .map_err(crate::error_message)?;
        let report = dctx0_engine::publish_codec_scaffold(
            &args.project,
            &args.output,
            artifact,
            &inputs,
            options,
            dctx0_target_typescript::CODEC_GENERATOR_VERSION,
            args.dry_run,
        )
        .map_err(crate::error_message)?;
        if json {
            crate::render::json(output, &report)?;
        } else {
            writeln!(output, "Codec scaffold: {} ({})\nSnapshot: {}\nTesting representation conversion only; production validation and native proof remain separate.", report.path, report.status, report.snapshot).map_err(crate::error_message)?;
            if report.dry_run {
                writeln!(output, "\n{}", report.artifact.contents).map_err(crate::error_message)?;
            }
        }
        Ok(true)
    }
    #[cfg(not(feature = "typescript"))]
    {
        let _ = (args, database, json, output, options);
        Err("Codec scaffolding requires the TypeScript target feature".into())
    }
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
