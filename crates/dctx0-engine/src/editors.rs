//! Offline, testing-independent local editor distributions embedded in the CLI.
use crate::{Error, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use zip::{ZipWriter, write::SimpleFileOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Editor {
    Vscode,
    Zed,
}
impl Editor {
    fn name(self) -> &'static str {
        match self {
            Self::Vscode => "vscode",
            Self::Zed => "zed",
        }
    }
}
pub struct EditorInstallRequest {
    pub editor: Editor,
    pub output: PathBuf,
    pub executable: PathBuf,
    pub force: bool,
    pub dry_run: bool,
}
#[derive(Debug, Serialize)]
pub struct EditorInstallReport {
    pub schema_version: u32,
    pub editor: Editor,
    pub directory: PathBuf,
    pub artifact: PathBuf,
    pub status: String,
    pub instructions: String,
}
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const VSCODE: &[(&str, &[u8])] = &[
    (
        "package.json",
        include_bytes!("../../../editors/vscode/package.json"),
    ),
    (
        "extension.js",
        include_bytes!("../../../editors/vscode/extension.js"),
    ),
    (
        "language-configuration.json",
        include_bytes!("../../../editors/vscode/language-configuration.json"),
    ),
    (
        "syntaxes/dctx0.tmLanguage.json",
        include_bytes!("../../../editors/vscode/syntaxes/dctx0.tmLanguage.json"),
    ),
];
const ZED: &[(&str, &[u8])] = &[
    (
        "Cargo.lock",
        include_bytes!("../../../editors/zed/Cargo.lock"),
    ),
    (
        "Cargo.toml",
        include_bytes!("../../../editors/zed/Cargo.toml"),
    ),
    (
        "languages/dctx0/config.toml",
        include_bytes!("../../../editors/zed/languages/dctx0/config.toml"),
    ),
    (
        "languages/dctx0/highlights.scm",
        include_bytes!("../../../editors/zed/languages/dctx0/highlights.scm"),
    ),
    (
        "languages/dctx0/brackets.scm",
        include_bytes!("../../../editors/zed/languages/dctx0/brackets.scm"),
    ),
    (
        "languages/dctx0/outline.scm",
        include_bytes!("../../../editors/zed/languages/dctx0/outline.scm"),
    ),
    (
        "languages/dctx0/semantic_token_rules.json",
        include_bytes!("../../../editors/zed/languages/dctx0/semantic_token_rules.json"),
    ),
    (
        "tree-sitter-dctx0/grammar.js",
        include_bytes!("../../../editors/tree-sitter-dctx0/grammar.js"),
    ),
    (
        "tree-sitter-dctx0/tree-sitter.json",
        include_bytes!("../../../editors/tree-sitter-dctx0/tree-sitter.json"),
    ),
    (
        "tree-sitter-dctx0/src/parser.c",
        include_bytes!("../../../editors/tree-sitter-dctx0/src/parser.c"),
    ),
    (
        "tree-sitter-dctx0/src/grammar.json",
        include_bytes!("../../../editors/tree-sitter-dctx0/src/grammar.json"),
    ),
    (
        "tree-sitter-dctx0/src/node-types.json",
        include_bytes!("../../../editors/tree-sitter-dctx0/src/node-types.json"),
    ),
    (
        "tree-sitter-dctx0/src/tree_sitter/parser.h",
        include_bytes!("../../../editors/tree-sitter-dctx0/src/tree_sitter/parser.h"),
    ),
    (
        "tree-sitter-dctx0/src/tree_sitter/alloc.h",
        include_bytes!("../../../editors/tree-sitter-dctx0/src/tree_sitter/alloc.h"),
    ),
    (
        "tree-sitter-dctx0/src/tree_sitter/array.h",
        include_bytes!("../../../editors/tree-sitter-dctx0/src/tree_sitter/array.h"),
    ),
];
fn problem(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
/// Stage a platform-local VSIX or Zed development extension without modifying editor stores.
pub fn install_editor(request: &EditorInstallRequest) -> Result<EditorInstallReport> {
    let output = if request.output.is_absolute() {
        request.output.clone()
    } else {
        std::env::current_dir()?.join(&request.output)
    };
    check_path(&output)?;
    let directory = output.join(request.editor.name());
    check_path(&directory)?;
    let artifact = match request.editor {
        Editor::Vscode => directory.join("dctx0.vsix"),
        Editor::Zed => directory.clone(),
    };
    let instructions = match request.editor {
        Editor::Vscode => format!(
            "Install with: code --install-extension {}. The VSIX bundles this platform's native server.",
            artifact.display()
        ),
        Editor::Zed => format!(
            "In Zed run ‘zed: install dev extension’ and select {}. Keep this directory in place; Zed compiles its Rust/Wasm launcher.",
            directory.display()
        ),
    };
    let mut report = EditorInstallReport {
        schema_version: 1,
        editor: request.editor,
        directory: directory.clone(),
        artifact,
        status: "planned".into(),
        instructions,
    };
    if request.dry_run {
        return Ok(report);
    }
    fs::create_dir_all(&output)?;
    let lockpath = output.join(".dctx0-editor.lock");
    check_path(&lockpath)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lockpath)?;
    lock.lock_exclusive()?;
    let stage = output.join(format!(
        ".dctx0-editor-stage-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&stage)?;
    let replacement = stage.join("new");
    fs::create_dir(&replacement)?;
    let result = (|| -> Result<String> {
        match request.editor {
            Editor::Vscode => vscode(&replacement, &request.executable)?,
            Editor::Zed => zed(&replacement, &directory, &request.executable)?,
        }
        let generated = tree_hash(&replacement)?;
        if directory.exists() {
            if tree_hash(&directory)? == generated {
                return Ok("unchanged".into());
            }
            if !request.force {
                return Err(problem(format!(
                    "{} contains different files; preserved. Use --force to replace this editor bundle",
                    directory.display()
                )));
            }
        }
        let backup = stage.join("old");
        let had_old = directory.exists();
        if had_old {
            fs::rename(&directory, &backup)?;
        }
        if let Err(error) = fs::rename(&replacement, &directory) {
            if had_old {
                if let Err(rollback) = fs::rename(&backup, &directory) {
                    return Err(problem(format!(
                        "Install failed: {error}; rollback failed: {rollback}; backup retained at {}",
                        backup.display()
                    )));
                }
            }
            return Err(error.into());
        }
        Ok("prepared".into())
    })();
    // Never remove a backup after a failed restoration.
    if result.is_err() && stage.join("old").exists() {
        return result.map(|_| report);
    }
    if let Err(cleanup) = fs::remove_dir_all(&stage) {
        return Err(problem(format!(
            "Editor preparation result: {result:?}; cleanup failed: {cleanup}; staging retained at {}",
            stage.display()
        )));
    }
    report.status = result?;
    Ok(report)
}
fn check_path(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        if ancestor
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(problem("Editor output may not contain parent traversal"));
        }
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(problem(format!(
                    "Symlink editor destination rejected: {}",
                    ancestor.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn zip_error(error: zip::result::ZipError) -> Error {
    problem(error.to_string())
}

fn vscode(root: &Path, executable: &Path) -> Result<()> {
    let mut zip = ZipWriter::new(fs::File::create(root.join("dctx0.vsix"))?);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);
    for (name, bytes) in VSCODE {
        zip.start_file(format!("extension/{name}"), options)
            .map_err(zip_error)?;
        zip.write_all(bytes)?;
    }
    zip.start_file(
        format!("extension/server/dctx0{}", std::env::consts::EXE_SUFFIX),
        options.unix_permissions(0o755),
    )
    .map_err(zip_error)?;
    std::io::copy(&mut fs::File::open(executable)?, &mut zip)?;
    zip.start_file("extension.vsixmanifest", options)
        .map_err(zip_error)?;
    zip.write_all(br#"<?xml version="1.0" encoding="utf-8"?><PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011"><Metadata><Identity Language="en-US" Id="dctx0" Version="0.1.0" Publisher="dctx0"/><DisplayName>DCTX0</DisplayName><Description xml:space="preserve">DCTX0 colors and native Rust language server</Description><Categories>Programming Languages</Categories><Properties><Property Id="Microsoft.VisualStudio.Code.Engine" Value="^1.95.0"/><Property Id="Microsoft.VisualStudio.Code.ExecutesCode" Value="true"/></Properties></Metadata><Installation><InstallationTarget Id="Microsoft.VisualStudio.Code"/></Installation><Dependencies/><Assets><Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true"/></Assets></PackageManifest>"#)?;
    zip.start_file("[Content_Types].xml", options)
        .map_err(zip_error)?;
    zip.write_all(br#"<?xml version="1.0" encoding="utf-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="json" ContentType="application/json"/><Default Extension="js" ContentType="application/javascript"/><Default Extension="xml" ContentType="text/xml"/><Default Extension="vsixmanifest" ContentType="text/xml"/><Default Extension="exe" ContentType="application/octet-stream"/><Override PartName="/extension/server/dctx0" ContentType="application/octet-stream"/></Types>"#)?;
    zip.finish().map_err(zip_error)?;
    Ok(())
}
fn zed(root: &Path, destination: &Path, executable: &Path) -> Result<()> {
    for (name, bytes) in ZED {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
    }
    fs::create_dir(root.join("server"))?;
    let binary = destination.join(format!("server/dctx0{}", std::env::consts::EXE_SUFFIX));
    fs::copy(
        executable,
        root.join(format!("server/dctx0{}", std::env::consts::EXE_SUFFIX)),
    )?;
    fs::create_dir(root.join("src"))?;
    let launcher = include_str!("../../../editors/zed/src/lib.rs").replace(
        "__DCTX0_SERVER_PATH__",
        &format!(
            "{:?}",
            binary
                .to_str()
                .ok_or_else(|| problem("Zed server path must be valid UTF-8"))?
        ),
    );
    fs::write(root.join("src/lib.rs"), launcher)?;
    let grammar = root.join("tree-sitter-dctx0");
    git(
        &grammar,
        &[
            "-c",
            "init.templateDir=",
            "init",
            "--quiet",
            "--initial-branch=main",
        ],
    )?;
    git(&grammar, &["add", "."])?;
    git(
        &grammar,
        &[
            "-c",
            "user.name=dctx0",
            "-c",
            "user.email=build@dctx0.local",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "DCTX0 grammar 0.1.0",
        ],
    )?;
    let revision = git(&grammar, &["rev-parse", "HEAD"])?;
    let repository = url_for_path(&destination.join("tree-sitter-dctx0"))?;
    let manifest = include_str!("../../../editors/zed/extension.template.toml")
        .replace("__GRAMMAR_REPOSITORY__", &repository)
        .replace("__GRAMMAR_REVISION__", revision.trim());
    fs::write(root.join("extension.toml"), manifest)?;
    Ok(())
}
fn url_for_path(path: &Path) -> Result<String> {
    url::Url::from_file_path(path)
        .map(String::from)
        .map_err(|_| problem("Invalid local grammar repository path"))
}
fn git(root: &Path, args: &[&str]) -> Result<String> {
    let result = Command::new("git")
        .arg("-c")
        .arg(format!(
            "core.hooksPath={}",
            root.join(".git/no-hooks").display()
        ))
        .arg("-c")
        .arg("core.autocrlf=false")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .output()?;
    if !result.status.success() {
        return Err(problem(format!(
            "Local grammar Git preparation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}
fn tree_hash(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    fn visit(
        root: &Path,
        path: &Path,
        out: &mut BTreeMap<PathBuf, String>,
        depth: usize,
    ) -> Result<()> {
        if depth > 32 || out.len() > 4096 {
            return Err(problem("Editor bundle exceeds comparison limits"));
        }
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() {
            return Err(problem("Symlinks inside editor bundles are rejected"));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|e| problem(e.to_string()))?
            .to_owned();
        if meta.is_dir() {
            out.insert(relative, "directory".into());
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if entry.file_name() != ".git" {
                    visit(root, &entry.path(), out, depth + 1)?;
                }
            }
        } else if meta.is_file() {
            if meta.len() > 512 * 1024 * 1024 {
                return Err(problem("Oversized editor bundle file"));
            }
            let mut reader = fs::File::open(path)?;
            let mut hash = Sha256::new();
            let mut buffer = [0; 65536];
            loop {
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            out.insert(relative, format!("{:x}", hash.finalize()));
        } else {
            return Err(problem("Special editor bundle file rejected"));
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out, 0)?;
    Ok(out)
}
/// Explicitly install a prepared VSIX using the user's local VS Code command.
pub fn activate_vscode(artifact: &Path, force: bool) -> Result<()> {
    let mut command = Command::new("code");
    command
        .arg("--install-extension")
        .arg(artifact)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit());
    if force {
        command.arg("--force");
    }
    let mut child = command.spawn().map_err(|e| {
        problem(format!(
            "Could not start VS Code's `code` installer: {e}. Prepared VSIX remains at {}",
            artifact.display()
        ))
    })?;
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(problem(format!(
                    "VS Code installation failed ({status}); prepared VSIX remains at {}",
                    artifact.display()
                )))
            };
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(problem("VS Code installer timed out after 30 seconds"));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staging_failures_and_private_bundle_writes_preserve_existing_destinations() {
        let root = std::env::temp_dir().join(format!("dctx0-editor-errors-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let binary = root.join("native");
        fs::write(&binary, "native").unwrap();
        let request = EditorInstallRequest {
            editor: Editor::Vscode,
            output: root.join("output"),
            executable: binary.clone(),
            force: false,
            dry_run: false,
        };
        let report = install_editor(&request).unwrap();
        assert_eq!(report.status, "prepared");
        let before = fs::read(&report.artifact).unwrap();
        fs::remove_file(&binary).unwrap();
        assert!(install_editor(&request).is_err());
        assert_eq!(fs::read(&report.artifact).unwrap(), before);
        fs::write(&binary, "native").unwrap();
        let mut request = request;
        request.editor = Editor::Zed;
        assert_eq!(install_editor(&request).unwrap().status, "prepared");
        assert!(url_for_path(Path::new("relative")).is_err());
        let bad = root.join("not-a-directory");
        fs::write(&bad, "keep").unwrap();
        request.output = bad.clone();
        assert!(install_editor(&request).is_err());
        assert_eq!(fs::read_to_string(bad).unwrap(), "keep");
        #[cfg(target_os = "linux")]
        {
            let failing = root.join("disk-failure");
            fs::create_dir(&failing).unwrap();
            std::os::unix::fs::symlink("/dev/full", failing.join("dctx0.vsix")).unwrap();
            assert!(vscode(&failing, &binary).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
#[cfg(all(test, unix))]
mod path_tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;
    #[test]
    fn zed_requires_representable_server_paths_and_emits_real_rust_literals() {
        let root = std::env::temp_dir().join(format!("dctx0-zed-paths-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let binary = root.join("native");
        fs::write(&binary, "native").unwrap();
        let stage = root.join("stage");
        fs::create_dir(&stage).unwrap();
        let invalid = root.join(std::ffi::OsString::from_vec(vec![255]));
        assert!(
            zed(&stage, &invalid, &binary)
                .unwrap_err()
                .to_string()
                .contains("UTF-8")
        );
        let request = EditorInstallRequest {
            editor: Editor::Zed,
            output: root.join("quotes-\"-and-controls-\u{000c}"),
            executable: binary,
            force: false,
            dry_run: false,
        };
        let report = install_editor(&request).unwrap();
        let source = fs::read_to_string(report.directory.join("src/lib.rs")).unwrap();
        assert!(source.contains("\\u{c}"));
        assert!(source.contains("\\\""));
        fs::remove_dir_all(root).unwrap();
    }
}
