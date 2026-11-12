//! Publish only embedded, pinned testing artifacts. Authored tests and production
//! dependency declarations belong to the application and are never overwritten.
use crate::*;
use dctx0_target_sdk::{
    ApplicationTestLibrary, application_protocol::APPLICATION_PROTOCOL_VERSION,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct ApplicationLibraryPublication {
    pub target: String,
    pub version: String,
    pub protocol_version: u32,
    pub files: BTreeMap<String, String>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn existing(path: &Path) -> Result<Option<Vec<u8>>> {
    if !path.try_exists()? {
        return Ok(None);
    }
    if fs::metadata(path)?.len() > 1_048_576 {
        return Err(Error::Invalid(
            "Application library artifact exceeds 1 MiB".into(),
        ));
    }
    Ok(Some(fs::read(path)?))
}
pub fn publish_application_test_library(
    application: &Path,
    library: &ApplicationTestLibrary,
) -> Result<ApplicationLibraryPublication> {
    if !matches!(library.target.as_str(), "go" | "typescript")
        || library.protocol_version != APPLICATION_PROTOCOL_VERSION
        || library.version.is_empty()
        || library.artifacts.is_empty()
        || library.artifacts.len() > 64
    {
        return Err(Error::Invalid(
            "Unsupported application test library".into(),
        ));
    }
    let root = fs::canonicalize(application)?;
    let (_, lock) = verification_setup::safe_file(&root, ".dctx0-install.lock")?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock)?;
    fs2::FileExt::lock_exclusive(&lock)?;
    let (_, pin_path) =
        verification_setup::safe_file(&root, ".dctx0/application-tests/library-pin.json")?;
    let mut owned = BTreeMap::<String, String>::new();
    if let Some(bytes) = existing(&pin_path)? {
        let old: serde_json::Value = serde_json::from_slice(&bytes)?;
        let files = old
            .get("files")
            .or_else(|| old.get("sha256"))
            .and_then(|files| files.as_object())
            .ok_or_else(|| {
                Error::Invalid("Existing application library pin has no file hashes".into())
            })?;
        for (name, value) in files {
            if name.contains(['/', '\\']) || name.is_empty() {
                return Err(Error::Invalid("Invalid pinned library filename".into()));
            }
            let expected = value
                .as_str()
                .ok_or_else(|| Error::Invalid("Invalid library artifact hash".into()))?;
            let (_, path) = verification_setup::safe_file(
                &root,
                &format!(".dctx0/application-tests/library/{name}"),
            )?;
            let actual = existing(&path)?.ok_or_else(|| {
                Error::Invalid(format!("Pinned library artifact is missing: {name}"))
            })?;
            if hash(&actual) != expected {
                return Err(Error::Invalid(format!(
                    "Library artifact was edited; preserve and review it before upgrading: {name}"
                )));
            }
            owned.insert(name.clone(), expected.into());
        }
    }
    let mut files = BTreeMap::new();
    for artifact in &library.artifacts {
        if artifact.path.contains(['/', '\\'])
            || artifact.contents.len() > 1_048_576
            || files.contains_key(&artifact.path)
        {
            return Err(Error::Invalid(
                "Invalid or duplicate embedded library artifact".into(),
            ));
        }
        let (_, path) = verification_setup::safe_file(
            &root,
            &format!(".dctx0/application-tests/library/{}", artifact.path),
        )?;
        if !owned.contains_key(&artifact.path)
            && let Some(bytes) = existing(&path)?
            && bytes != artifact.contents.as_bytes()
        {
            return Err(Error::Invalid(format!(
                "Refusing to overwrite unowned application file {}",
                artifact.path
            )));
        }
        files.insert(artifact.path.clone(), hash(artifact.contents.as_bytes()));
    }
    // Validate all destinations first. A crash before the final pin leaves a
    // visible mismatch rather than a falsely certified partial publication.
    for artifact in &library.artifacts {
        let (directory, path) = verification_setup::safe_file(
            &root,
            &format!(".dctx0/application-tests/library/{}", artifact.path),
        )?;
        fs::create_dir_all(directory)?;
        atomic_write(&path, artifact.contents.as_bytes())?;
    }
    let module = if library.target == "go" {
        "github.com/Haardt/dctx0ctl/applicationtests/go"
    } else {
        "@dctx0/application-tests"
    };
    atomic_write(
        &pin_path,
        &serde_json::to_vec_pretty(
            &serde_json::json!({"schema_version":1,"target":library.target,"module":module,"version":library.version,"protocol_version":library.protocol_version,"application_protocol":library.protocol_version,"files":files,"sha256":files}),
        )?,
    )?;
    Ok(ApplicationLibraryPublication {
        target: library.target.clone(),
        version: library.version.clone(),
        protocol_version: library.protocol_version,
        files,
    })
}
