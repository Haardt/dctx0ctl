//! Explicit storage inspection and copy conversion.
use clap::{Args, Subcommand};
use dctx0_engine::{AccessMode, Project};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Args)]
pub(crate) struct StorageArgs {
    #[command(subcommand)]
    action: Action,
}
#[derive(Subcommand)]
enum Action {
    Recover {
        input: PathBuf,
    },
    InstallCopy {
        input: PathBuf,
        #[arg(long)]
        copy: PathBuf,
        #[arg(long)]
        evidence: PathBuf,
    },
    VerifyCopy {
        input: PathBuf,
        #[arg(long)]
        copy: PathBuf,
    },
    Inspect {
        input: PathBuf,
    },
    Compact {
        input: PathBuf,
        #[arg(long)]
        dry_run: bool,
        /// Build a verified copy without replacing the live database.
        #[arg(long)]
        output: Option<PathBuf>,
    },
}
pub(crate) fn run(
    args: StorageArgs,
    database: Option<&Path>,
    output: &mut dyn Write,
) -> Result<(), String> {
    match args.action {
        Action::Recover { input } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            crate::render::json(
                output,
                &project.storage_recover().map_err(crate::error_message)?,
            )
        }
        Action::InstallCopy {
            input,
            copy,
            evidence,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(evidence).map_err(crate::error_message)?)
                    .map_err(crate::error_message)?;
            let expected = std::fs::canonicalize(&copy).map_err(crate::error_message)?;
            let recorded = report["copy"]
                .as_str()
                .map(PathBuf::from)
                .ok_or("Missing comparison copy path")?;
            let binary = std::env::current_exe().map_err(crate::error_message)?;
            let binary_hash = hash_file(&binary)?;
            let copy_hash = hash_file(&copy)?;
            if report["status"] != "complete"
                || report["copy_sha256"] != copy_hash
                || report["binary_hashes"]["new"] != binary_hash
                || std::fs::canonicalize(recorded).map_err(crate::error_message)? != expected
                || report["source"] != project.database.to_string_lossy().as_ref()
            {
                return Err("Storage installation requires a successful unchanged copy/binary comparison report".into());
            }
            let result = project
                .storage_install_copy(&copy)
                .map_err(crate::error_message)?;
            crate::render::json(output, &result)
        }
        Action::VerifyCopy { input, copy } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .store(AccessMode::ReadOnly)
                .map_err(crate::error_message)?
                .storage_verify_copy(&copy)
                .map_err(crate::error_message)?;
            crate::render::json(output, &report)
        }
        Action::Inspect { input } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            let report = project
                .store(AccessMode::ReadOnly)
                .map_err(crate::error_message)?
                .storage_inspect()
                .map_err(crate::error_message)?;
            crate::render::json(output, &report)
        }
        Action::Compact {
            input,
            dry_run,
            output: destination,
        } => {
            let project = Project::new(&input, database).map_err(crate::error_message)?;
            if dry_run {
                let report = project
                    .store(AccessMode::ReadOnly)
                    .map_err(crate::error_message)?
                    .storage_inspect()
                    .map_err(crate::error_message)?;
                return crate::render::json(
                    output,
                    &serde_json::json!({"dry_run":true,"source":project.database,"before":report,"target_schema":7,"retention":"latest run and latest FAIL/UNKNOWN; pending task publications protected"}),
                );
            }
            let report = project
                .storage_compact(destination.as_deref())
                .map_err(crate::error_message)?;
            crate::render::json(output, &report)
        }
    }
}

fn hash_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(crate::error_message)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(crate::error_message)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
