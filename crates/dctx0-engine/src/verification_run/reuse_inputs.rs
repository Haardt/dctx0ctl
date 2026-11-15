//! Explicit input bindings for historical reuse. These declarations do not sandbox a test.
use super::*;
use dctx0_verification::incremental::{
    ExternalInputReceipt, INPUT_RECEIPT_SCHEMA_VERSION, InputGap,
};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::time::Duration;

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
    revisions: BTreeMap<String, Revision>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    executable: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}
fn default_timeout() -> u64 {
    5_000
}

fn runtime_hash(
    application: &Path,
    runtime: &str,
    policy: &crate::NativeEnvironment,
) -> Result<String> {
    let entries = policy
        .entries()
        .map_err(|error| Error::Invalid(error.to_string()))?;
    let runtime = crate::native::identity::resolve_program_with_path(
        runtime,
        application,
        crate::native::environment_value(&entries, "PATH"),
    )
    .map_err(|_| Error::Invalid("Unable to resolve configured runtime".into()))?;
    let mut input = fs::File::open(runtime)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 16384];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > 268_435_456 {
            return Err(Error::Invalid(
                "Configured runtime exceeds identity limit".into(),
            ));
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn revision_inputs(
    application: &Path,
    revision: Revision,
    policy: &crate::NativeEnvironment,
    cancellation: &NativeCancellation,
) -> std::result::Result<(String, String), &'static str> {
    let recipe = ExecutionRecipe {
        program: revision.executable,
        arguments: revision.args,
    };
    let limits = NativeLimits {
        environment: policy.clone(),
        run: Duration::from_millis(revision.timeout_ms),
        log_bytes: 65_536,
        ..NativeLimits::default()
    };
    let mut command = NativeCommand::spawn(&recipe, application, limits)
        .await
        .map_err(|_| "REVISION_LAUNCH_UNAVAILABLE")?;
    let identity = command.runtime_identity();
    let audit = command
        .wait_status(cancellation)
        .await
        .map_err(|_| "REVISION_EXECUTION_INCOMPLETE")?;
    if audit.exit_code != Some(0) {
        return Err("REVISION_EXECUTION_FAILED");
    }
    let output = command.stdout_bytes();
    if output.is_empty() {
        return Err("REVISION_OUTPUT_MISSING");
    }
    let identity = serde_json::to_vec(&identity).map_err(|_| "REVISION_IDENTITY_UNAVAILABLE")?;
    Ok((digest(&identity), digest(&output)))
}

/// Query live revisions both before and after execution. Only digests are persisted.
pub(super) async fn capture(
    application: &Path,
    inputs: &str,
    cancellation: &NativeCancellation,
) -> Result<ExternalInputReceipt> {
    capture_mode(application, inputs, cancellation, true).await
}

/// Static preflight never launches a revision provider. Unknown live inputs
/// remain explicit gaps rather than being silently assumed unchanged.
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
    let verifier = crate::native::VerifierIdentity::capture()
        .map_err(|_| Error::Invalid("Cannot capture verifier identity".into()))?;
    let mut files = BTreeMap::from([
        (format!("declaration:{inputs}"), digest(&bytes)),
        ("verifier".into(), digest(&serde_json::to_vec(&verifier)?)),
    ]);
    let mut incomplete = Vec::new();
    let config_path = application.join(".dctx0/testing/verification.json");
    let mut policy = crate::NativeEnvironment::default();
    let has_config = match fs::symlink_metadata(&config_path) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if has_config {
        let (_, path) =
            crate::verification_setup::safe_file(application, ".dctx0/testing/verification.json")?;
        let config: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
        if let Some(value) = config.get("environment").filter(|value| !value.is_null()) {
            policy = serde_json::from_value(value.clone())?;
        }
        let runtime = config
            .get("runtime")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::Invalid("Missing configured runtime".into()))?;
        match runtime_hash(application, runtime, &policy) {
            Ok(hash) => {
                files.insert("configured-runtime".into(), hash);
            }
            Err(_) => incomplete.push(InputGap {
                code: "CONFIGURED_RUNTIME_UNAVAILABLE".into(),
                input: "configured-runtime".into(),
            }),
        }
    }
    for name in declaration.files {
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
    let mut environment = BTreeMap::new();
    for (name, value) in policy
        .entries()
        .map_err(|error| Error::Invalid(error.to_string()))?
    {
        let name = name
            .into_string()
            .map_err(|_| Error::Invalid("Non-UTF8 reuse environment name".into()))?;
        environment.insert(name, Some(digest(value.as_encoded_bytes())));
    }
    for name in declaration.environment {
        environment.entry(name).or_insert(None);
    }
    for (name, revision) in declaration.revisions {
        if name.is_empty()
            || name.len() > 256
            || name.chars().any(char::is_control)
            || revision.executable.is_empty()
            || revision.args.len() > 256
            || !(1..=30_000).contains(&revision.timeout_ms)
        {
            return Err(Error::Invalid("Invalid live revision query".into()));
        }
        if !live {
            incomplete.push(InputGap {
                code: "REVISION_REQUIRES_LIVE_PLAN".into(),
                input: name,
            });
            continue;
        }
        match revision_inputs(application, revision, &policy, cancellation).await {
            Ok((identity, output)) => {
                files.insert(format!("revision-identity:{name}"), identity);
                files.insert(format!("revision-output:{name}"), output);
            }
            Err(code) => incomplete.push(InputGap {
                code: code.into(),
                input: name,
            }),
        }
    }
    Ok(ExternalInputReceipt {
        schema_version: INPUT_RECEIPT_SCHEMA_VERSION,
        deterministic_units: declaration.deterministic_units,
        files,
        environment,
        incomplete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "dctx0-reuse-input-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn snapshots_and_live_revisions_bind_observed_inputs_without_disclosing_them() {
        let root = Directory::new();
        fs::write(root.0.join("snapshot"), b"private snapshot").unwrap();
        fs::write(root.0.join("revision"), b"private revision one").unwrap();
        fs::write(root.0.join("inputs.json"), r#"{"schema_version":1,"deterministic_units":["unit"],"files":["snapshot"],"environment":["DCTX0_ABSENT_TEST_VARIABLE_9428"],"revisions":{"database":{"executable":"/bin/cat","args":["revision"]}}}"#).unwrap();
        let first = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(
            first,
            capture(&root.0, "inputs.json", &NativeCancellation::default())
                .await
                .unwrap()
        );
        assert_eq!(
            first.environment.get("DCTX0_ABSENT_TEST_VARIABLE_9428"),
            Some(&None)
        );
        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains("private revision"));
        assert!(!serialized.contains("private snapshot"));
        fs::write(root.0.join("revision"), b"private revision two").unwrap();
        let changed = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_ne!(
            first.files["revision-output:database"],
            changed.files["revision-output:database"]
        );
        fs::write(root.0.join("snapshot"), b"changed").unwrap();
        let changed_file = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_ne!(
            changed.files["file:snapshot"],
            changed_file.files["file:snapshot"]
        );
        fs::create_dir_all(root.0.join(".dctx0/testing")).unwrap();
        fs::write(
            root.0.join(".dctx0/testing/verification.json"),
            r#"{"runtime":"./runtime"}"#,
        )
        .unwrap();
        fs::write(root.0.join("runtime"), b"runtime one").unwrap();
        let runtime_one = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        fs::write(
            root.0.join(".dctx0/testing/verification.json"),
            r#"{"runtime":"./runtime","environment":null}"#,
        )
        .unwrap();
        assert_eq!(
            runtime_one,
            capture(&root.0, "inputs.json", &NativeCancellation::default())
                .await
                .unwrap()
        );
        fs::write(root.0.join("runtime"), b"runtime two").unwrap();
        let runtime_two = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_ne!(
            runtime_one.files["configured-runtime"],
            runtime_two.files["configured-runtime"]
        );
        fs::remove_file(root.0.join("snapshot")).unwrap();
        let missing = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(
            missing.incomplete,
            vec![InputGap {
                code: "INPUT_FILE_UNAVAILABLE".into(),
                input: "snapshot".into(),
            }]
        );
        assert!(!missing.files.contains_key("file:snapshot"));
    }

    #[tokio::test]
    async fn revision_query_failures_and_timeouts_remain_explicit_incomplete_receipts() {
        let root = Directory::new();
        fs::write(root.0.join("inputs.json"), r#"{"schema_version":1,"deterministic_units":["unit"],"revisions":{"bad":{"executable":"/bin/sh","args":["-c","sleep 5"],"timeout_ms":30}}}"#).unwrap();
        let start = Instant::now();
        let timed_out = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(timed_out.schema_version, INPUT_RECEIPT_SCHEMA_VERSION);
        assert_eq!(
            timed_out.incomplete,
            vec![InputGap {
                code: "REVISION_EXECUTION_INCOMPLETE".into(),
                input: "bad".into(),
            }]
        );
        assert!(start.elapsed() < Duration::from_secs(4));
        fs::write(root.0.join("inputs.json"), r#"{"schema_version":1,"deterministic_units":["unit"],"revisions":{"bad":{"executable":"/bin/false"}}}"#).unwrap();
        let failed = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(failed.incomplete[0].code, "REVISION_EXECUTION_FAILED");
        assert!(!failed.files.contains_key("revision-output:bad"));
        fs::create_dir_all(root.0.join(".dctx0/testing")).unwrap();
        fs::write(
            root.0.join(".dctx0/testing/verification.json"),
            r#"{"runtime":"/dctx0-absent-runtime-22901"}"#,
        )
        .unwrap();
        let unavailable = capture(&root.0, "inputs.json", &NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(
            unavailable.incomplete[0].code,
            "CONFIGURED_RUNTIME_UNAVAILABLE"
        );
        assert!(!unavailable.files.contains_key("configured-runtime"));
    }
}
