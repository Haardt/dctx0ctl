//! Bounded Cargo input inventory and compiler/executable provenance.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RustBuildReceipt {
    pub schema_version: u32,
    pub runtime_version: String,
    pub files: BTreeMap<String, String>,
    pub packages: Vec<String>,
    pub binary_hash: String,
    pub target_directory: String,
    #[serde(skip)]
    cache_guard: Option<fs::File>,
}
async fn output(
    program: &str,
    args: &[&str],
    directory: &Path,
    cancel: &NativeCancellation,
) -> Result<String> {
    let recipe = ExecutionRecipe {
        program: program.into(),
        arguments: args.iter().map(|s| (*s).into()).collect(),
    };
    let limits = NativeLimits {
        log_bytes: 4_194_304,
        initialization: std::time::Duration::from_secs(300),
        run: std::time::Duration::from_secs(300),
        ..Default::default()
    };
    let mut process = crate::NativeCommand::spawn(&recipe, directory, limits)
        .await
        .map_err(|e| Error::Invalid(format!("Cargo inspection: {e}")))?;
    Ok(process
        .wait(cancel)
        .await
        .map_err(|e| Error::Invalid(format!("Cargo inspection: {e}")))?
        .stdout)
}
fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0; 65536];
    let mut digest = Sha256::new();
    let mut total = 0u64;
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        total += length as u64;
        if total > 268_435_456 {
            return Err(Error::Invalid("Rust build file exceeds 256 MiB".into()));
        }
        digest.update(&buffer[..length]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn add(path: &Path, files: &mut BTreeMap<String, String>) -> Result<()> {
    let path = fs::canonicalize(path)
        .map_err(|error| Error::Invalid(format!("Rust build input {}: {error}", path.display())))?;
    if files.len() >= 30_000 {
        return Err(Error::Invalid(
            "Rust build input count exceeds 30000".into(),
        ));
    }
    files.insert(path.to_string_lossy().into_owned(), hash(&path)?);
    Ok(())
}
fn walk(root: &Path, files: &mut BTreeMap<String, String>, depth: usize) -> Result<()> {
    if depth > 64 {
        return Err(Error::Invalid(
            "Rust build inventory exceeds directory depth".into(),
        ));
    }
    let mut entries = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if matches!(
            entry.file_name().to_str(),
            Some("target" | ".bin" | ".git" | ".dctx0ctl" | "__pycache__")
        ) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            walk(&entry.path(), files, depth + 1)?;
        } else if metadata.is_file() {
            add(&entry.path(), files)?;
        } else if metadata.file_type().is_symlink() {
            let actual = fs::canonicalize(entry.path())?;
            if actual.is_file() {
                add(&actual, files)?;
            } else {
                return Err(Error::Invalid(format!(
                    "Rust dependency directory symlink unsupported: {}",
                    entry.path().display()
                )));
            }
        } else {
            return Err(Error::Invalid("Nonregular Rust dependency input".into()));
        }
    }
    Ok(())
}
/// SDK/toolchain directories may contain aliases (notably Xcode headers).
/// Follow physical directories once, retaining strict application capture rules.
fn walk_toolchain(root: &Path, files: &mut BTreeMap<String, String>) -> Result<()> {
    fn visit(
        root: &Path,
        files: &mut BTreeMap<String, String>,
        visited: &mut BTreeSet<PathBuf>,
        depth: usize,
    ) -> Result<()> {
        let root = fs::canonicalize(root).map_err(|error| {
            Error::Invalid(format!("Toolchain input {}: {error}", root.display()))
        })?;
        if !visited.insert(root.clone()) {
            return Ok(());
        }
        if depth > 64 || visited.len() > 30_000 {
            return Err(Error::Invalid(
                "Toolchain directory inventory exceeds bounds".into(),
            ));
        }
        let mut entries = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let metadata = fs::metadata(entry.path()).map_err(|error| {
                Error::Invalid(format!(
                    "Toolchain input {}: {error}",
                    entry.path().display()
                ))
            })?;
            if metadata.is_dir() {
                visit(&entry.path(), files, visited, depth + 1)?;
            } else if metadata.is_file() {
                add(&entry.path(), files)?;
            } else {
                return Err(Error::Invalid(format!(
                    "Nonregular toolchain input {}",
                    entry.path().display()
                )));
            }
        }
        Ok(())
    }
    visit(root, files, &mut BTreeSet::new(), 0)
}
/// Validate Cargo's required package identities and capture boundaries before I/O.
fn package_roots(
    metadata: &serde_json::Value,
    application: &Path,
) -> Result<Vec<(String, PathBuf)>> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| Error::Invalid("Cargo metadata lacks packages".into()))?;
    if packages.is_empty() {
        return Err(Error::Invalid("Cargo metadata contains no packages".into()));
    }
    let mut roots = Vec::new();
    for package in packages {
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or_else(|| Error::Invalid("Cargo package lacks manifest".into()))?,
        );
        let root = manifest
            .parent()
            .ok_or_else(|| Error::Invalid("Cargo package has no directory".into()))?;
        if !manifest.is_absolute()
            || manifest
                .components()
                .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(Error::Invalid(
                "Cargo manifest must be a normalized absolute path".into(),
            ));
        }
        let source = package
            .get("source")
            .ok_or_else(|| Error::Invalid("Cargo package lacks source origin".into()))?;
        if source.is_null() && !root.starts_with(application) {
            return Err(Error::Invalid(
                "Rust local dependency escapes captured application".into(),
            ));
        }
        if !source.is_null() && !source.as_str().is_some_and(|s| !s.is_empty()) {
            return Err(Error::Invalid("Invalid Cargo package source origin".into()));
        }
        let identity = package["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Error::Invalid("Cargo package lacks identity".into()))?;
        roots.push((identity.into(), root.to_path_buf()));
    }
    Ok(roots)
}

fn physical_package_roots(
    mut metadata: serde_json::Value,
    application: &Path,
) -> Result<Vec<(String, PathBuf)>> {
    // Cargo reports physical paths (/private/var on macOS; expanded/verbatim
    // paths on Windows). Compare both boundaries in that same namespace.
    let application = fs::canonicalize(application)?;
    for package in metadata["packages"]
        .as_array_mut()
        .ok_or_else(|| Error::Invalid("Cargo metadata lacks packages".into()))?
    {
        let manifest = package["manifest_path"]
            .as_str()
            .ok_or_else(|| Error::Invalid("Cargo package lacks manifest".into()))?;
        let manifest = fs::canonicalize(manifest)?;
        package["manifest_path"] =
            serde_json::Value::String(manifest.to_string_lossy().into_owned());
    }
    package_roots(&metadata, &application)
}

#[cfg(target_os = "linux")]
async fn capture_c_toolchain(
    directory: &Path,
    cancel: &NativeCancellation,
    files: &mut BTreeMap<String, String>,
) -> Result<()> {
    // Bundled SQLite's host C toolchain and standard system header closure.
    for tool in ["/usr/bin/cc", "/usr/bin/as", "/usr/bin/ld", "/usr/bin/ar"] {
        add(Path::new(tool), files)?;
    }
    for name in ["cc1", "collect2"] {
        let argument = format!("-print-prog-name={name}");
        let tool = output("cc", &[&argument], directory, cancel).await?;
        add(Path::new(tool.trim()), files)?;
    }
    let multiarch = output("cc", &["-print-multiarch"], directory, cancel).await?;
    let multiarch = multiarch.trim();
    if !multiarch.is_empty() {
        if multiarch.contains('/') || multiarch.contains('\\') {
            return Err(Error::Invalid(
                "Invalid C compiler multiarch include directory".into(),
            ));
        }
        let path = Path::new("/usr/include").join(multiarch);
        if path.is_dir() {
            walk_toolchain(&path, files)?;
        }
    }
    let compiler_include = output("cc", &["-print-file-name=include"], directory, cancel).await?;
    walk_toolchain(Path::new(compiler_include.trim()), files)?;
    for entry in fs::read_dir("/usr/include")? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            add(&entry.path(), files)?;
        }
    }
    for name in ["bits", "sys", "gnu", "asm", "asm-generic", "linux"] {
        let path = Path::new("/usr/include").join(name);
        if path.is_dir() {
            walk_toolchain(&path, files)?;
        }
    }
    Ok(())
}
#[cfg(target_os = "macos")]
async fn capture_c_toolchain(
    directory: &Path,
    cancel: &NativeCancellation,
    files: &mut BTreeMap<String, String>,
) -> Result<()> {
    for name in ["clang", "as", "ld", "ar"] {
        let path = output("xcrun", &["--find", name], directory, cancel).await?;
        add(Path::new(path.trim()), files)?;
    }
    let sdk = output("xcrun", &["--show-sdk-path"], directory, cancel).await?;
    walk_toolchain(&Path::new(sdk.trim()).join("usr/include"), files)?;
    let resource = output("clang", &["-print-resource-dir"], directory, cancel).await?;
    walk_toolchain(&Path::new(resource.trim()).join("include"), files)?;
    Ok(())
}
#[cfg(windows)]
async fn capture_c_toolchain(
    _directory: &Path,
    _cancel: &NativeCancellation,
    files: &mut BTreeMap<String, String>,
) -> Result<()> {
    // The native runner initializes the Visual Studio developer environment.
    // Capture the selected compiler binaries and effective header search roots.
    for name in ["cl.exe", "link.exe", "lib.exe"] {
        let path = crate::native::identity::resolve_program(name, _directory)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        add(&path, files)?;
        if name == "cl.exe" {
            walk_toolchain(
                path.parent()
                    .ok_or_else(|| Error::Invalid("MSVC compiler lacks parent".into()))?,
                files,
            )?;
        }
    }
    let include = std::env::var_os("INCLUDE").ok_or_else(|| {
        Error::Invalid(
            "MSVC INCLUDE is required; initialize the Visual Studio developer environment".into(),
        )
    })?;
    for root in std::env::split_paths(&include) {
        walk_toolchain(&root, files)?;
    }
    Ok(())
}
pub(crate) async fn prepare(
    directory: &Path,
    application: &Path,
    cancel: &NativeCancellation,
) -> Result<RustBuildReceipt> {
    for name in [
        "CC",
        "CXX",
        "AR",
        "CFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "RUSTC",
        "CARGO_BUILD_RUSTC",
    ] {
        if std::env::var_os(name).is_some() {
            return Err(Error::Invalid(format!(
                "Custom native toolchain override {name} is outside the pinned Rust build capability"
            )));
        }
    }
    let runtime_version = output("rustc", &["-vV"], directory, cancel)
        .await?
        .trim()
        .to_owned();
    fs::write(directory.join("build-version.txt"), &runtime_version)?;
    let text = output(
        "cargo",
        &["metadata", "--offline", "--locked", "--format-version", "1"],
        directory,
        cancel,
    )
    .await?;
    let metadata: serde_json::Value = serde_json::from_str(&text)?;
    let application = fs::canonicalize(application)?;
    let mut files = BTreeMap::new();
    let mut names = Vec::new();
    // Workspace-level embedded assets are real compiler inputs outside package roots.
    walk(&application, &mut files, 0)?;
    for (name, root) in physical_package_roots(metadata, &application)? {
        if !root.starts_with(&application) {
            walk(&root, &mut files, 0)?;
        }
        names.push(name);
    }
    names.sort();
    names.dedup();
    let sysroot = output("rustc", &["--print", "sysroot"], directory, cancel).await?;
    let sysroot = Path::new(sysroot.trim());
    // A system toolchain may use /usr as its sysroot. Inventory Rust tools,
    // not every unrelated program and library installed on the host.
    for name in ["rustc", "cargo"] {
        let tool = sysroot
            .join("bin")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        add(&tool, &mut files)?;
    }
    for entry in fs::read_dir(sysroot.join("bin"))? {
        let entry = entry?;
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "dll")
        {
            add(&entry.path(), &mut files)?;
        }
    }
    // Compiler dynamic libraries and the complete selected standard-library directory.
    for entry in fs::read_dir(sysroot.join("lib"))? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.path().is_file()
            && (name.starts_with("librustc_driver") || name.starts_with("libLLVM"))
        {
            add(&entry.path(), &mut files)?;
        }
    }
    let targetlib = output("rustc", &["--print", "target-libdir"], directory, cancel).await?;
    let targetlib = Path::new(targetlib.trim());
    walk_toolchain(targetlib, &mut files)?;
    if let Some(parent) = targetlib.parent() {
        let tools = parent.join("bin");
        if tools.is_dir() {
            walk_toolchain(&tools, &mut files)?;
        }
    }
    capture_c_toolchain(directory, cancel, &mut files).await?;
    let cache_root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .ok_or_else(|| {
            Error::Invalid("Rust build cache needs LOCALAPPDATA, HOME or XDG_CACHE_HOME".into())
        })?;
    fs::create_dir_all(&cache_root)?;
    let (cache_parent, cache_path) =
        crate::verification_setup::safe_file(&cache_root, "dctx0ctl/native-rust-v1/cache-marker")?;
    fs::create_dir_all(&cache_parent)?;
    // safe_file verifies a regular file; the target directory is created after its parent validation.
    let cache_directory = cache_path
        .parent()
        .ok_or_else(|| Error::Invalid("Rust cache has no parent".into()))?
        .join("target");
    if cache_directory.try_exists()? {
        let metadata = fs::symlink_metadata(&cache_directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::Invalid(
                "Rust cache target is not a real directory".into(),
            ));
        }
    }
    let target_directory = cache_directory.to_string_lossy().into_owned();
    let guard = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(cache_parent.join("build.lock"))?;
    let deadline = Instant::now() + std::time::Duration::from_secs(300);
    loop {
        match fs2::FileExt::try_lock_exclusive(&guard) {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(Error::Invalid("Rust build cache lock timed out".into()));
                }
                tokio::select! { _=cancel.cancelled()=>return Err(Error::Invalid("Rust build cache wait cancelled".into())), _=tokio::time::sleep(std::time::Duration::from_millis(50))=>{} }
            }
            Err(error) => return Err(error.into()),
        }
    }

    Ok(RustBuildReceipt {
        schema_version: 1,
        runtime_version,
        files,
        packages: names,
        binary_hash: String::new(),
        target_directory,
        cache_guard: Some(guard),
    })
}
pub(super) fn finish(receipt: RustBuildReceipt, directory: &Path) -> Result<RustBuildReceipt> {
    finish_named(
        receipt,
        directory,
        "dctx0-worker",
        &format!(".bin/worker{}", std::env::consts::EXE_SUFFIX),
    )
}
pub(crate) fn finish_named(
    mut receipt: RustBuildReceipt,
    directory: &Path,
    name: &str,
    destination: &str,
) -> Result<RustBuildReceipt> {
    validate(&receipt)?;
    let depinfo = Path::new(&receipt.target_directory).join(format!("release/{name}.d"));
    let metadata = fs::metadata(&depinfo)?;
    if metadata.len() > 4_194_304 {
        return Err(Error::Invalid("Rust dependency info exceeds budget".into()));
    }
    let text = fs::read_to_string(depinfo)?.replace("\\\n", "");
    let line = text
        .lines()
        .next()
        .ok_or_else(|| Error::Invalid("Empty Rust compiler dependency info".into()))?;
    let (_, dependencies) = line
        .split_once(": ")
        .ok_or_else(|| Error::Invalid("Invalid Rust compiler dependency info".into()))?;
    for path in dependency_paths(dependencies)? {
        let path = fs::canonicalize(path)?;
        if !receipt
            .files
            .contains_key(&path.to_string_lossy().into_owned())
        {
            return Err(Error::Invalid(format!(
                "Compiler used an input outside the pinned Rust closure: {}",
                path.display()
            )));
        }
    }
    let executable = Path::new(&receipt.target_directory)
        .join(format!("release/{name}{}", std::env::consts::EXE_SUFFIX));
    receipt.binary_hash = hash(&executable)?;
    fs::create_dir_all(
        directory
            .join(destination)
            .parent()
            .ok_or_else(|| Error::Invalid("Executable destination lacks parent".into()))?,
    )?;
    fs::copy(executable, directory.join(destination))?;
    if hash(&directory.join(destination))? != receipt.binary_hash {
        return Err(Error::Invalid("Rust worker changed while copying".into()));
    }
    receipt.cache_guard.take();
    Ok(receipt)
}
pub(crate) fn validate(receipt: &RustBuildReceipt) -> Result<()> {
    if receipt.schema_version != 1
        || receipt.files.is_empty()
        || receipt.packages.is_empty()
        || receipt.runtime_version.is_empty()
    {
        return Err(Error::Invalid("Incomplete Rust build receipt".into()));
    }
    for (path, expected) in &receipt.files {
        if !Path::new(path).is_absolute() || hash(Path::new(path))? != *expected {
            return Err(Error::Invalid(format!("Rust build input changed: {path}")));
        }
    }
    Ok(())
}

fn dependency_paths(text: &str) -> Result<Vec<String>> {
    let mut values = Vec::new();
    let mut item = String::new();
    let mut escaped = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if escaped {
            item.push(character);
            escaped = false;
        } else if character == '\\' {
            if characters
                .peek()
                .is_none_or(|c| matches!(c, ' ' | '\\' | '#'))
            {
                escaped = true;
            } else {
                item.push(character);
            }
        } else if character.is_whitespace() {
            if !item.is_empty() {
                values.push(std::mem::take(&mut item));
            }
        } else {
            item.push(character);
        }
    }
    if escaped {
        return Err(Error::Invalid(
            "Incomplete escape in Rust dependency info".into(),
        ));
    }
    if !item.is_empty() {
        values.push(item);
    }
    if values.is_empty() {
        return Err(Error::Invalid(
            "Rust compiler dependency info has no sources".into(),
        ));
    }
    Ok(values)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_dependency_paths_preserve_escaped_spaces_and_reject_incomplete_paths() {
        assert_eq!(
            dependency_paths("/a.rs /with\\ space/lib.rs").unwrap(),
            ["/a.rs", "/with space/lib.rs"]
        );
        assert_eq!(
            dependency_paths(r"C:\work\src\lib.rs C:\with\ space\a.rs").unwrap(),
            [r"C:\work\src\lib.rs", r"C:\with space\a.rs"]
        );
        assert!(dependency_paths("/incomplete\\").is_err());
        assert!(dependency_paths(" ").is_err());
    }
    #[test]
    fn cargo_package_metadata_rejects_missing_fields_and_escaping_local_sources() {
        use serde_json::json;
        let app = Path::new("/captured/app");
        for invalid in [
            json!({}),
            json!({"packages":[]}),
            json!({"packages":[{}]}),
            json!({"packages":[{"manifest_path":"/"}]}),
            json!({"packages":[{"manifest_path":"/captured/app/Cargo.toml","id":"local"}]}),
            json!({"packages":[{"manifest_path":"/captured/app/Cargo.toml","source":null}]}),
            json!({"packages":[{"manifest_path":"/outside/Cargo.toml","source":null,"id":"local"}]}),
            json!({"packages":[{"manifest_path":"/captured/app/../outside/Cargo.toml","source":null,"id":"local"}]}),
        ] {
            assert!(package_roots(&invalid, app).is_err(), "{invalid}");
        }
        let valid = json!({"packages":[{"manifest_path":"/captured/app/Cargo.toml","source":null,"id":"local"},{"manifest_path":"/registry/dependency/Cargo.toml","source":"registry+https://example.invalid/index","id":"dependency"}]});
        assert_eq!(package_roots(&valid, app).unwrap().len(), 2);
    }
    #[cfg(unix)]
    #[test]
    fn sdk_directory_aliases_bind_physical_files_terminate_cycles_and_reject_broken_inputs() {
        let root = std::env::temp_dir().join(format!("dctx0-sdk-aliases-{}", std::process::id()));
        let include = root.join("include");
        fs::create_dir_all(&include).unwrap();
        fs::write(include.join("actual.h"), "#define ACTUAL 1").unwrap();
        std::os::unix::fs::symlink(&include, root.join("alias")).unwrap();
        std::os::unix::fs::symlink(&root, include.join("cycle")).unwrap();
        let mut files = BTreeMap::new();
        assert!(
            walk_toolchain(&root.join("missing-sdk"), &mut files)
                .unwrap_err()
                .to_string()
                .contains("missing-sdk")
        );
        walk_toolchain(&root, &mut files).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[&fs::canonicalize(include.join("actual.h"))
                .unwrap()
                .to_string_lossy()
                .into_owned()],
            hash(&include.join("actual.h")).unwrap()
        );
        std::os::unix::fs::symlink(root.join("missing"), root.join("broken")).unwrap();
        assert!(
            walk_toolchain(&root, &mut files)
                .unwrap_err()
                .to_string()
                .contains("broken")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn cargo_physical_paths_accept_application_aliases_and_reject_escaping_local_aliases() {
        let root =
            std::env::temp_dir().join(format!("dctx0-cargo-physical-{}", std::process::id()));
        let app = root.join("application");
        let outside = root.join("external");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(app.join("Cargo.toml"), "fixture").unwrap();
        fs::write(outside.join("Cargo.toml"), "external").unwrap();
        let alias = root.join("application-alias");
        std::os::unix::fs::symlink(&app, &alias).unwrap();
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"packages":[]}),
            serde_json::json!({"packages":[{}]}),
            serde_json::json!({"packages":[{"manifest_path":app.join("missing.toml")} ]}),
        ] {
            assert!(physical_package_roots(invalid, &alias).is_err());
        }
        let metadata = |manifest: PathBuf| serde_json::json!({"packages":[{"manifest_path":manifest,"id":"local","source":null}]});
        let roots = physical_package_roots(
            metadata(fs::canonicalize(app.join("Cargo.toml")).unwrap()),
            &alias,
        )
        .unwrap();
        assert_eq!(roots[0].1, fs::canonicalize(&app).unwrap());
        std::os::unix::fs::symlink(&outside, app.join("escape")).unwrap();
        assert!(
            physical_package_roots(metadata(app.join("escape/Cargo.toml")), &alias)
                .unwrap_err()
                .to_string()
                .contains("escapes")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn unavailable_and_failed_inspection_tools_return_technical_errors() {
        let cancel = NativeCancellation::default();
        let root = std::env::temp_dir();
        assert!(
            output("/dctx0-no-such-inspection-tool", &[], &root, &cancel)
                .await
                .is_err()
        );
        assert!(
            output("sh", &["-c", "exit 1"], &root, &cancel)
                .await
                .is_err()
        );
    }
    #[test]
    fn invalid_compiler_depinfo_and_unpinned_includes_cannot_finish_a_receipt() {
        let root = std::env::temp_dir().join(format!("dctx0-rust-receipt-{}", std::process::id()));
        let target = root.join("target");
        fs::create_dir_all(target.join("release")).unwrap();
        let source = root.join("source.rs");
        fs::write(&source, "pub fn real() {}").unwrap();
        let make_receipt = || RustBuildReceipt {
            schema_version: 1,
            runtime_version: "test toolchain".into(),
            files: BTreeMap::from([(
                source.to_string_lossy().into_owned(),
                hash(&source).unwrap(),
            )]),
            packages: vec!["fixture".into()],
            binary_hash: String::new(),
            target_directory: target.to_string_lossy().into_owned(),
            cache_guard: None,
        };
        let depinfo = target.join("release/dctx0-worker.d");
        for invalid in ["", "not dependency info"] {
            fs::write(&depinfo, invalid).unwrap();
            assert!(finish(make_receipt(), &root).is_err());
        }
        let external = root.join("untracked.rs");
        fs::write(&external, "untracked input").unwrap();
        fs::write(&depinfo, format!("worker: {}", external.display())).unwrap();
        assert!(finish(make_receipt(), &root).is_err());
        fs::write(&depinfo, format!("worker: {}", source.display())).unwrap();
        fs::write(
            target.join(format!(
                "release/dctx0-worker{}",
                std::env::consts::EXE_SUFFIX
            )),
            b"fixture binary",
        )
        .unwrap();
        let receipt = finish(make_receipt(), &root).unwrap();
        assert_eq!(
            receipt.binary_hash,
            hash(&root.join(format!(".bin/worker{}", std::env::consts::EXE_SUFFIX))).unwrap()
        );
        fs::write(&source, "changed").unwrap();
        assert!(validate(&receipt).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
