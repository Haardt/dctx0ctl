//! Explicit input bindings for historical reuse. These declarations do not sandbox a test.
use super::*;
use dctx0_verification::incremental::{
    ExternalInputReceipt, INPUT_RECEIPT_SCHEMA_VERSION, InputGap,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    schema_version: u32,
    deterministic_units: BTreeSet<String>,
    #[serde(default)]
    files: BTreeSet<String>,
    #[serde(default)]
    environment: BTreeSet<String>,
    #[serde(default)]
    revisions: BTreeMap<String, serde_json::Value>,
}

/// Capture explicitly declared code inputs without querying runtime providers.
pub(super) async fn capture(
    application: &Path,
    inputs: &str,
    cancellation: &NativeCancellation,
) -> Result<ExternalInputReceipt> {
    capture_mode(application, inputs, cancellation, true).await
}

/// Static preflight uses the same code selection and never launches providers.
pub(super) async fn capture_static(
    application: &Path,
    inputs: &str,
    cancellation: &NativeCancellation,
) -> Result<ExternalInputReceipt> {
    capture_mode(application, inputs, cancellation, false).await
}

async fn capture_mode(
    application: &Path,
    inputs: &str,
    cancellation: &NativeCancellation,
    live: bool,
) -> Result<ExternalInputReceipt> {
    let bytes = if inputs.is_empty() {
        br#"{"schema_version":1,"deterministic_units":[]}"#.to_vec()
    } else {
        let (_, path) = crate::verification_setup::safe_file(application, inputs)?;
        fs::read(path)?
    };
    if bytes.len() > 65_536 {
        return Err(Error::Invalid("Reuse declaration exceeds 64 KiB".into()));
    }
    let declaration: Declaration = serde_json::from_slice(&bytes)?;
    if declaration.schema_version != 1
        || declaration.files.len() > 1024
        || declaration.environment.len() > 1024
        || declaration.revisions.len() > 128
    {
        return Err(Error::Invalid(
            "Invalid native reuse input declaration".into(),
        ));
    }
    // Code-only policy: runtime binaries, revisions and inherited environment
    // remain execution metadata, not change-detection inputs.
    let _ = (cancellation, live);
    let mut files = BTreeMap::new();
    let mut incomplete = Vec::new();
    let mut selection = super::code_selection::CodeSelection::new(application)?;
    for name in declaration.files {
        if !selection.contains(Path::new(&name))? {
            continue;
        }
        let file = if Path::new(&name).is_absolute() {
            PathBuf::from(&name)
        } else {
            crate::verification_setup::safe_file(application, &name)?.1
        };
        let metadata = match fs::symlink_metadata(&file) {
            Ok(metadata) => metadata,
            Err(_) => {
                incomplete.push(InputGap {
                    code: "INPUT_FILE_UNAVAILABLE".into(),
                    input: name,
                });
                continue;
            }
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 67_108_864 {
            return Err(Error::Invalid(format!(
                "Reuse input must be a regular file at most 64 MiB: {name}"
            )));
        }
        match fs::read(file) {
            Ok(bytes) => {
                files.insert(format!("file:{name}"), digest(&bytes));
            }
            Err(_) => incomplete.push(InputGap {
                code: "INPUT_FILE_UNAVAILABLE".into(),
                input: name,
            }),
        }
    }
    Ok(ExternalInputReceipt {
        schema_version: INPUT_RECEIPT_SCHEMA_VERSION,
        deterministic_units: declaration.deterministic_units,
        files,
        environment: BTreeMap::new(),
        incomplete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn code_receipts_ignore_environment_runtime_and_revision_commands() {
        let root = std::env::temp_dir().join(format!("dctx0-code-inputs-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("source.go"), "package source").unwrap();
        fs::write(
            root.join("inputs.json"),
            br#"{
          "schema_version":1,"deterministic_units":[],"files":["source.go","missing.json"],
          "environment":["CODEX_TOOL_CALL_ID"],
          "revisions":{"runtime":{"executable":"nonexistent-provider"}}
        }"#,
        )
        .unwrap();
        let cancel = NativeCancellation::default();
        let live = capture(&root, "inputs.json", &cancel).await.unwrap();
        let planned = capture_static(&root, "inputs.json", &cancel).await.unwrap();
        assert_eq!(live, planned);
        assert!(live.environment.is_empty());
        assert!(live.incomplete.is_empty());
        assert_eq!(
            live.files,
            [("file:source.go".into(), digest(b"package source"))].into()
        );
        fs::remove_file(root.join("source.go")).unwrap();
        let missing = capture_static(&root, "inputs.json", &cancel).await.unwrap();
        assert_eq!(missing.incomplete[0].code, "INPUT_FILE_UNAVAILABLE");
        fs::remove_dir_all(root).unwrap();
    }
}
