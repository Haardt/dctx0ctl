use std::{fs, process::Command};
#[test]
fn editor_installation_preview_is_read_only_and_distinct_from_agent_skills() {
    let root = std::env::temp_dir().join(format!("dctx0-editor-cli-{}", std::process::id()));
    let binary = env!("CARGO_BIN_EXE_dctx0");
    for editor in ["vscode", "zed"] {
        let output = Command::new(binary)
            .args([
                "--json",
                "editor",
                "install",
                editor,
                "--dry-run",
                "--output",
            ])
            .arg(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], "planned");
    }
    assert!(!root.exists());
    let output = Command::new(binary)
        .args([
            "editor",
            "install",
            "vscode",
            "--activate",
            "--dry-run",
            "--output",
        ])
        .arg(&root)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("code --install-extension"));
    assert!(!root.exists());
    assert!(
        !Command::new(binary)
            .args(["editor", "install", "zed", "--activate", "--dry-run"])
            .status()
            .unwrap()
            .success()
    );
    assert!(!fs::exists(root).unwrap());
}
#[cfg(unix)]
#[test]
fn explicit_activation_uses_local_code_command_and_retains_vsix_on_failure() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("dctx0-editor-activate-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let code = root.join("code");
    let marker = root.join("args");
    fs::write(
        &code,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$DCTX0_TEST_INSTALL_ARGS\"\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&code, fs::Permissions::from_mode(0o755)).unwrap();
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args([
                "--json",
                "editor",
                "install",
                "vscode",
                "--activate",
                "--output",
            ])
            .arg(root.join("bundles"))
            .env("PATH", &root)
            .env("DCTX0_TEST_INSTALL_ARGS", &marker)
            .output()
            .unwrap()
    };
    let result = invoke();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["status"], "installed-in-vscode");
    assert!(
        fs::read_to_string(&marker)
            .unwrap()
            .contains("--install-extension")
    );
    fs::write(&code, "#!/bin/sh\nexit 7\n").unwrap();
    let result = invoke();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("VS Code installation failed"));
    assert!(root.join("bundles/vscode/dctx0.vsix").exists());
    fs::remove_file(&code).unwrap();
    let result = invoke();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Could not start VS Code"));
    fs::remove_dir_all(root).unwrap();
}
