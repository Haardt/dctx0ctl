use clap::{Args, Subcommand};
use dctx0_core::TargetProfileProvider;
use std::{io::Write, path::PathBuf};
#[derive(Args)]
pub(crate) struct ProfileArgs {
    target: String,
    #[command(subcommand)]
    action: ProfileAction,
}
#[derive(Subcommand)]
enum ProfileAction {
    Create {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        default: bool,
        #[arg(long)]
        agents: bool,
        #[arg(long)]
        dry_run: bool,
    },
}
pub(crate) fn provider(target: &str) -> Result<Box<dyn TargetProfileProvider>, String> {
    match target {
        #[cfg(feature = "typescript")]
        "typescript" => Ok(Box::new(dctx0_target_typescript::Provider)),
        #[cfg(feature = "java")]
        "java" => Ok(Box::new(dctx0_target_java::Provider)),
        #[cfg(feature = "go")]
        "go" => Ok(Box::new(dctx0_target_go::Provider)),
        #[cfg(feature = "rust")]
        "rust" => Ok(Box::new(dctx0_target_rust::Provider)),
        _ => Err(format!("Target {target} is not registered in this build")),
    }
}
pub(crate) fn run(args: ProfileArgs, json: bool, output: &mut dyn Write) -> Result<(), String> {
    let provider = provider(&args.target)?;
    let ProfileAction::Create {
        project,
        default,
        agents,
        dry_run,
    } = args.action;
    let root = project
        .map_or_else(std::env::current_dir, Ok)
        .map_err(crate::error_message)?;
    let report = dctx0_engine::create_profiles(&root, provider.as_ref(), default, agents, dry_run)
        .map_err(crate::error_message)?;
    if json {
        crate::render::json(output, &report)?;
    } else {
        writeln!(
            output,
            "{} {} profiles; default {}",
            if dry_run { "PREVIEW" } else { "CREATED" },
            report.target,
            report.default_target
        )
        .map_err(crate::error_message)?;
        for path in report.files {
            writeln!(output, "{path}").map_err(crate::error_message)?;
        }
    }
    Ok(())
}
