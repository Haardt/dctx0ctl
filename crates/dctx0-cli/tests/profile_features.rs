use std::{fs, process::Command};
#[test]
fn shipped_target_features_control_profile_registration() {
    let root = std::env::temp_dir().join(format!("dctx0-profile-features-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    for (target, enabled) in [
        ("typescript", cfg!(feature = "typescript")),
        ("java", cfg!(feature = "java")),
        ("go", cfg!(feature = "go")),
        ("rust", cfg!(feature = "rust")),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args([
                "profile",
                target,
                "create",
                "--project",
                root.to_str().unwrap(),
                "--dry-run",
                "--json",
            ])
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            enabled,
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(!root.join(".dctx0").exists());
    fs::remove_dir_all(root).unwrap();
}
