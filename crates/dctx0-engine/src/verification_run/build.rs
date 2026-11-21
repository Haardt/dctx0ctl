//! Managed compilation and independently checked Go build input receipts.
use super::*;
use std::io::Read;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GoBuildReceipt {
    pub schema_version: u32,
    pub go_version: String,
    pub files: BTreeMap<String, String>,
    pub packages: Vec<String>,
    pub binary_hash: String,
    #[serde(default)]
    pub build_elapsed_us: u64,
    #[serde(default)]
    pub build_peak_rss_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub discovery: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub module_root: String,
}

pub(super) async fn execute(
    recipe: &ExecutionRecipe,
    directory: &Path,
    cancellation: &NativeCancellation,
    artifacts: &mut Vec<VerificationArtifact>,
    environment: &crate::NativeEnvironment,
) -> Result<()> {
    let started = Instant::now();
    let limits = NativeLimits {
        environment: environment.clone(),
        initialization: std::time::Duration::from_secs(300),
        run: std::time::Duration::from_secs(300),
        ..NativeLimits::default()
    };
    let mut command = crate::NativeCommand::spawn(recipe, directory, limits)
        .await
        .map_err(|e| Error::Invalid(format!("Native build: {e}")))?;
    let audit = command
        .wait(cancellation)
        .await
        .map_err(|e| Error::Invalid(format!("Native build: {e}")))?;
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.build-audit+json",
        serde_json::to_vec(&serde_json::json!({"schema_version":1,"elapsed_us":started.elapsed().as_micros(),"audit":audit}))?,
    ));
    Ok(())
}

fn file_hash(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(268_435_457)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 268_435_456 {
        return Err(Error::Invalid("Go build input exceeds file budget".into()));
    }
    Ok(digest(&bytes))
}

pub(super) fn go_receipt(directory: &Path) -> Result<GoBuildReceipt> {
    let path = directory.join(".bin/dependencies.json");
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(4_194_305)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4_194_304 {
        return Err(Error::Invalid(
            "Go build receipt exceeds size budget".into(),
        ));
    }
    let receipt: GoBuildReceipt = serde_json::from_slice(&bytes)?;
    if !matches!(receipt.schema_version, 1 | 2)
        || (receipt.schema_version == 1 && !receipt.discovery.is_empty())
        || (receipt.schema_version == 2 && receipt.discovery.is_empty())
        || receipt.files.is_empty()
        || receipt.files.len() > 20_000
        || receipt.packages.is_empty()
        || receipt.go_version.is_empty()
    {
        return Err(Error::Invalid("Incomplete Go build input receipt".into()));
    }
    validate(&receipt)?;
    if file_hash(&directory.join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX)))?
        != receipt.binary_hash
    {
        return Err(Error::Invalid(
            "Go worker does not match build receipt".into(),
        ));
    }
    Ok(receipt)
}

pub(super) fn validate(receipt: &GoBuildReceipt) -> Result<()> {
    for (path, expected) in &receipt.files {
        if !Path::new(path).is_absolute() || file_hash(Path::new(path))? != *expected {
            return Err(Error::Invalid(format!("Go build input changed: {path}")));
        }
    }
    for (directory, expected) in &receipt.discovery {
        if discovery_hash(directory)? != *expected {
            return Err(Error::Invalid(format!(
                "Go package discovery changed: {directory}"
            )));
        }
    }
    Ok(())
}

pub(super) fn discovery_hash(key: &str) -> Result<String> {
    if let Some(directory) = key.strip_prefix("module-scope:") {
        let mut bytes = Vec::new();
        for name in ["go.mod", "go.sum", "go.work", "go.work.sum"] {
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(0);
            let path = Path::new(directory).join(name);
            let hash = if path.try_exists()? {
                file_hash(&path)?
            } else {
                "missing".into()
            };
            bytes.extend_from_slice(hash.as_bytes());
            bytes.push(0);
        }
        return Ok(digest(&bytes));
    }
    let directory = key;
    let mut inputs = fs::read_dir(directory)?
        .map(|entry| {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                || !matches!(
                    entry.path().extension().and_then(|v| v.to_str()),
                    Some(
                        "go" | "s"
                            | "S"
                            | "syso"
                            | "h"
                            | "c"
                            | "cc"
                            | "cpp"
                            | "m"
                            | "f"
                            | "F"
                            | "f90"
                            | "swig"
                            | "swigcxx"
                    )
                )
            {
                return Ok(None);
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| std::io::Error::other("Non-UTF8 Go package entry"))?;
            Ok(Some((name, entry.path())))
        })
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    inputs.sort_by(|left, right| left.0.cmp(&right.0));
    if !Path::new(directory).is_absolute() {
        return Err(Error::Invalid(
            "Go discovery needs an absolute directory".into(),
        ));
    }
    let mut bytes = Vec::new();
    for (name, path) in inputs {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(file_hash(&path)?.as_bytes());
        bytes.push(0);
    }
    Ok(digest(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_bind_actual_inputs_and_binary_and_reject_changes() {
        let root = std::env::temp_dir().join(format!("dctx0-build-receipt-{}", std::process::id()));
        fs::create_dir_all(root.join(".bin")).unwrap();
        let source = root.join("source.go");
        fs::write(&source, b"package main").unwrap();
        fs::write(
            root.join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX)),
            b"native binary",
        )
        .unwrap();
        let receipt = GoBuildReceipt {
            schema_version: 1,
            go_version: "go test fixture".into(),
            files: BTreeMap::from([(
                source.to_string_lossy().into_owned(),
                digest(b"package main"),
            )]),
            packages: vec!["fixture".into()],
            binary_hash: digest(b"native binary"),
            build_elapsed_us: 1,
            build_peak_rss_bytes: None,
            discovery: BTreeMap::new(),
            module_root: String::new(),
        };
        fs::write(
            root.join(".bin/dependencies.json"),
            serde_json::to_vec(&receipt).unwrap(),
        )
        .unwrap();
        assert!(go_receipt(&root).is_ok());
        fs::write(&source, b"changed").unwrap();
        assert!(
            go_receipt(&root)
                .unwrap_err()
                .to_string()
                .contains("changed")
        );
        fs::write(&source, b"package main").unwrap();
        fs::write(
            root.join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX)),
            b"other binary",
        )
        .unwrap();
        assert!(
            go_receipt(&root)
                .unwrap_err()
                .to_string()
                .contains("worker")
        );
        fs::write(root.join(".bin/dependencies.json"), b"{}").unwrap();
        assert!(go_receipt(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
