//! Published codec inputs and non-overwriting testing-artifact publication.
use crate::*;
use dctx0_core::{CodecOptions, DataIr, codec_declarations};
use dctx0_target_sdk::Artifact;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct CodecInputs {
    pub snapshot: String,
    pub declarations: BTreeMap<String, DataIr>,
}
#[derive(Debug, Serialize)]
pub struct CodecScaffoldReport {
    pub snapshot: String,
    pub generator_version: String,
    pub configuration: CodecOptions,
    pub path: String,
    pub status: String,
    pub dry_run: bool,
    pub artifact: Artifact,
}
impl Project {
    pub fn codec_inputs(&self, subjects: &BTreeSet<String>) -> Result<CodecInputs> {
        let store = self.store(AccessMode::ReadOnly)?;
        let (_, snapshot) = store.task_check(&self.key())?;
        let snapshot = store.load(&self.key(), &snapshot)?;
        if dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash {
            return Err(Error::Invalid(
                "Spec files changed since the published check; run check first".into(),
            ));
        }
        Ok(CodecInputs {
            declarations: codec_declarations(&snapshot.ir, subjects).map_err(Error::Invalid)?,
            snapshot: snapshot.id,
        })
    }
}

/// Only a basename is accepted: generated imports stay beside the existing contract.
pub fn publish_codec_scaffold(
    application: &Path,
    filename: &str,
    artifact: Artifact,
    inputs: &CodecInputs,
    options: CodecOptions,
    generator_version: &str,
    dry_run: bool,
) -> Result<CodecScaffoldReport> {
    if filename.is_empty()
        || filename.contains(['/', '\\'])
        || filename.chars().any(char::is_control)
        || !filename.ends_with(".ts")
        || matches!(
            filename,
            "contract.ts" | "worker.ts" | "adapter.ts" | "module-observer.ts"
        )
    {
        return Err(Error::Invalid(
            "Codec output must be a separate .ts basename beside contract.ts".into(),
        ));
    }
    let root = fs::canonicalize(application)?;
    let relative = format!(".dctx0/testing/typescript/{filename}");
    let (parent, path) = crate::verification_setup::safe_file(&root, &relative)?;
    let exists = path.try_exists()?;
    let status = if exists {
        if !path.is_file() {
            return Err(Error::Invalid(
                "Existing codec output is not a regular file".into(),
            ));
        }
        "preserved"
    } else if dry_run {
        "proposed"
    } else {
        fs::create_dir_all(parent)?;
        if crate::filesystem::atomic_write_new(&path, artifact.contents.as_bytes())? {
            "created"
        } else {
            "preserved"
        }
    };
    Ok(CodecScaffoldReport {
        snapshot: inputs.snapshot.clone(),
        generator_version: generator_version.into(),
        configuration: options,
        path: relative,
        status: status.into(),
        dry_run,
        artifact: Artifact {
            path: filename.into(),
            ..artifact
        },
    })
}
