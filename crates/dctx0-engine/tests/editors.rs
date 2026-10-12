use dctx0_engine::{Editor, EditorInstallRequest, install_editor};
use std::{fs, io::Read};
#[test]
fn offline_editor_bundles_embed_server_preserve_edits_and_are_repeatable() {
    let root = std::env::temp_dir().join(format!("dctx0-editors-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let executable = root.join("dctx0");
    fs::write(&executable, b"fake native executable").unwrap();
    let mut request = EditorInstallRequest {
        editor: Editor::Vscode,
        output: root.join("exports"),
        executable,
        force: false,
        dry_run: true,
    };
    assert_eq!(install_editor(&request).unwrap().status, "planned");
    assert!(!request.output.exists());
    request.dry_run = false;
    let report = install_editor(&request).unwrap();
    assert_eq!(report.status, "prepared");
    let mut zip = zip::ZipArchive::new(fs::File::open(report.artifact).unwrap()).unwrap();
    let mut bytes = Vec::new();
    zip.by_name(&format!(
        "extension/server/dctx0{}",
        std::env::consts::EXE_SUFFIX
    ))
    .unwrap()
    .read_to_end(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"fake native executable");
    assert!(
        zip.by_name("extension/syntaxes/dctx0.tmLanguage.json")
            .is_ok()
    );
    assert_eq!(install_editor(&request).unwrap().status, "unchanged");
    fs::write(request.output.join("vscode/custom.txt"), "keep me").unwrap();
    assert!(install_editor(&request).is_err());
    assert!(request.output.join("vscode/custom.txt").exists());
    request.force = true;
    assert_eq!(install_editor(&request).unwrap().status, "prepared");
    request.force = false;
    request.editor = Editor::Zed;
    let report = install_editor(&request).unwrap();
    let manifest = fs::read_to_string(report.directory.join("extension.toml")).unwrap();
    assert!(manifest.contains("file://"));
    assert!(!manifest.contains("__GRAMMAR"));
    let launcher = fs::read_to_string(report.directory.join("src/lib.rs")).unwrap();
    assert!(!launcher.contains("__DCTX0"));
    assert!(launcher.contains("lsp"));
    assert_eq!(install_editor(&request).unwrap().status, "unchanged");
    #[cfg(unix)]
    {
        let malicious = root.join("symlink");
        std::os::unix::fs::symlink(&request.output, &malicious).unwrap();
        request.output = malicious;
        assert!(install_editor(&request).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}
