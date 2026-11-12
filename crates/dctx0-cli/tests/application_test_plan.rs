use dctx0_verification::{application_plan::*, *};
use std::{fs, process::Command};

#[test]
fn diagnostic_export_validates_ownership_without_changing_history_or_legacy_configuration() {
    let root = std::env::temp_dir().join(format!("dctx0-application-plan-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let specs = root.join("spec");
    fs::create_dir_all(&specs).unwrap();
    fs::write(
        specs.join("w.dctx0"),
        "workspace W { value Year = Int { range 1..2 } }",
    )
    .unwrap();
    let config_path = root.join("groups.json");
    let catalog_path = root.join("catalog.json");
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let call = |args: &[&str]| Command::new(binary).args(args).output().unwrap();
    assert!(call(&["check", specs.to_str().unwrap()]).status.success());
    let base = call(&[
        "verify",
        "plan",
        specs.to_str().unwrap(),
        "--subject",
        "W",
        "--json",
    ]);
    assert!(
        base.status.success(),
        "{}",
        String::from_utf8_lossy(&base.stderr)
    );
    let program: ProgramPlan = serde_json::from_slice(&base.stdout).unwrap();
    let configuration = ApplicationTestConfiguration {
        schema_version: 2,
        groups: vec![ExecutionGroup {
            id: "direct".into(),
            backend: TestBackend::ApplicationTests,
            target: "go".into(),
            selection: ["TestYear".into()].into(),
            area: TestArea {
                cases: case_manifest(&program).required_cases,
                ..Default::default()
            },
            resources: GroupResources {
                namespace: "year".into(),
                timeout_seconds: 30,
                max_message_bytes: 1_048_576,
            },
        }],
    };
    let catalog = vec![TestRegistration {
        group: "direct".into(),
        test: "TestYear".into(),
        form: TestForm::Decoder,
        cases: configuration.groups[0].area.cases.clone(),
    }];
    fs::write(&config_path, serde_json::to_vec(&configuration).unwrap()).unwrap();
    fs::write(&catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let history_before = call(&["history", specs.to_str().unwrap(), "--json"]);
    let args = [
        "verify",
        "plan",
        specs.to_str().unwrap(),
        "--subject",
        "W",
        "--application-tests",
        config_path.to_str().unwrap(),
        "--test-catalog",
        catalog_path.to_str().unwrap(),
    ];
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let exported = call(&json_args);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let export: ApplicationTestPlan = serde_json::from_slice(&exported.stdout).unwrap();
    assert_eq!(export.program, program);
    export.validate().unwrap();
    let markdown = call(&args);
    assert!(markdown.status.success());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.contains("Diagnostic planning only"));
    assert!(markdown.contains("direct | ApplicationTests | go"));
    assert_eq!(
        history_before.stdout,
        call(&["history", specs.to_str().unwrap(), "--json"]).stdout
    );
    assert!(!root.join(".dctx0/testing/verification.json").exists());
    fs::write(&catalog_path, "[]").unwrap();
    let missing = call(&json_args);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("MISSING_TEST_REGISTRATION"));
    fs::write(&catalog_path, " ".repeat(524_289)).unwrap();
    let oversized = call(&json_args);
    assert!(!oversized.status.success());
    assert!(String::from_utf8_lossy(&oversized.stderr).contains("exceeds 512 KiB"));
    assert!(!call(&args[..7]).status.success());
    fs::remove_dir_all(root).unwrap();
}
