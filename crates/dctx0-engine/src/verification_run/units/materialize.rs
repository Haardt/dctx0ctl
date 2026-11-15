//! Pure recipes first; managed workspaces are acquired only for fresh cohorts.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(in crate::verification_run) struct UnitRecipe {
    pub configuration: VerificationConfiguration,
    pub files: BTreeMap<String, Vec<u8>>,
    pub go: PathBuf,
}

pub(in crate::verification_run) fn controlled_environment(
    root: &Path,
    config: &VerificationConfiguration,
) -> Result<crate::NativeEnvironment> {
    let mut environment = config
        .environment
        .clone()
        .ok_or_else(|| Error::Invalid("Controlled environment required".into()))?;
    let crate::NativeEnvironment::Controlled { values, .. } = &mut environment else {
        return Err(Error::Invalid("Controlled environment required".into()));
    };
    let mut allowed: Vec<String> = values
        .get("DCTX0_GO_READONLY_MODULE_ROOTS_V1")
        .map(|v| serde_json::from_str(v))
        .transpose()?
        .unwrap_or_default();
    allowed.push(root.to_string_lossy().into_owned());
    allowed.sort();
    allowed.dedup();
    values.insert(
        "DCTX0_GO_READONLY_MODULE_ROOTS_V1".into(),
        serde_json::to_string(&allowed)?,
    );
    let entries = environment
        .entries()
        .map_err(|e| Error::Invalid(e.to_string()))?;
    for (name, required) in [
        ("GOENV", "off"),
        ("GOWORK", "off"),
        ("GOTOOLCHAIN", "local"),
        ("CGO_ENABLED", "0"),
    ] {
        if crate::native::environment_value(&entries, name).is_none_or(|value| value != required) {
            return Err(Error::Invalid(format!(
                "Decoder unit environment requires {name}={required}"
            )));
        }
    }
    Ok(environment)
}

pub(in crate::verification_run) fn recipe(
    root: &Path,
    config: &VerificationConfiguration,
    group: &DecoderUnitGroup,
    generator: &dyn HarnessGenerator,
) -> Result<UnitRecipe> {
    let mut config = config.clone();
    config.adapter_scaffold_hash = None;
    config.adapter = ".dctx0/testing/go/adapter.go".into();
    config.fixtures = ".dctx0/testing/go/examples.json".into();
    config.scope.subjects = group.subjects.clone();
    config.scope.tasks.clear();
    config.environment = Some(controlled_environment(root, &config)?);
    let entries = config
        .environment
        .as_ref()
        .ok_or_else(|| Error::Invalid("Controlled environment required".into()))?
        .entries()
        .map_err(|error| Error::Invalid(error.to_string()))?;
    let go = crate::native::identity::resolve_program_with_path(
        &config.runtime,
        root,
        crate::native::environment_value(&entries, "PATH"),
    )
    .map_err(|e| Error::Invalid(e.to_string()))?;
    let adapter = generator
        .contract_decoder_adapter(&group.decoders)
        .map_err(|e| Error::Invalid(e.to_string()))?;
    let (_, path) = crate::verification_setup::safe_file(root, "go.mod")?;
    let module = fs::read_to_string(path)?;
    let name = module
        .lines()
        .find_map(|line| line.trim().strip_prefix("module ").map(str::trim))
        .ok_or_else(|| Error::Invalid("Go module declaration missing".into()))?;
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err(Error::Invalid("Unsupported module identity".into()));
    }
    let mut managed = module.replacen(
        &format!("module {name}"),
        &format!("module {name}/dctx0-units/{}", group.id),
        1,
    );
    if group
        .decoders
        .iter()
        .any(|d| d.import_path == name || d.import_path.starts_with(&format!("{name}/")))
    {
        managed.push_str(&format!(
            "\nrequire {name} v0.0.0\nreplace {name} => {}\n",
            serde_json::to_string(&root.to_string_lossy())?
        ));
    }
    let mut files = BTreeMap::from([
        ("go.mod".into(), managed.into_bytes()),
        (config.adapter.clone(), adapter.contents.into_bytes()),
        (config.fixtures.clone(), b"{\"examples\":{}}\n".to_vec()),
        (
            ".dctx0/testing/verification.json".into(),
            serde_json::to_vec(&config)?,
        ),
    ]);
    if root.join("go.sum").try_exists()? {
        files.insert(
            "go.sum".into(),
            fs::read(crate::verification_setup::safe_file(root, "go.sum")?.1)?,
        );
    }
    Ok(UnitRecipe {
        configuration: config,
        files,
        go,
    })
}

pub(super) fn write(root: &Path, group: &DecoderUnitGroup, recipe: &UnitRecipe) -> Result<PathBuf> {
    let relative = format!(
        ".dctx0ctl/unit-workspaces/{}-{}-{}",
        group.id,
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let (parent, path) =
        crate::verification_setup::safe_file(root, &format!("{relative}/owner.json"))?;
    fs::create_dir_all(&parent)?;
    // Exclusive ownership; an existing workspace is never repurposed as input.
    OpenOptions::new().write(true).create_new(true).open(path)?;
    for (relative, bytes) in &recipe.files {
        let (directory, file) = crate::verification_setup::safe_file(&parent, relative)?;
        fs::create_dir_all(directory)?;
        atomic_write(&file, bytes)?;
    }
    Ok(parent)
}
