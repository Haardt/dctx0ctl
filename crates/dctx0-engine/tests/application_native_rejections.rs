#![cfg(unix)]
use dctx0_core::*;
use dctx0_engine::*;
use dctx0_verification::{application_plan::TestForm, application_session::*};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dctx0-app-rejections-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join(".dctx0/testing")).unwrap();
        fs::write(
            path.join("spec.dctx0"),
            "workspace W { value Year = Int { range 3..4 } }",
        )
        .unwrap();
        fs::write(path.join("fixtures.json"), r#"{"examples":{}}"#).unwrap();
        fs::write(path.join(".dctx0/testing/verification.json"),json!({"schema_version":1,"target":"typescript","runtime":"bun","adapter":"adapter.ts","fixtures":"fixtures.json","scope":{"subjects":["W.Year"],"tasks":[]},"budgets":VerificationBudgets::default()}).to_string()).unwrap();
        Self(path)
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
    fn prepare(&self) {
        self.project()
            .prepare_application_tests(
                &self.0,
                &ApplicationTestingPreparation {
                    plan: "plan.json",
                    group: "native",
                    test: "TestYear",
                    form: TestForm::Decoder,
                    group_seconds: Some(3),
                    stateless_decoders: Default::default(),
                    boundary: ApplicationValueBoundary::ValueConstructor,
                    subjects: Default::default(),
                },
            )
            .unwrap();
    }
    fn write(&self, name: &str, value: &Value) {
        fs::write(self.0.join(name), serde_json::to_vec(value).unwrap()).unwrap();
    }
    async fn failed_native(&self) -> ApplicationNativeReport {
        self.project()
            .run_application_tests(
                &self.0,
                "plan.json",
                "native",
                "recipe.json",
                ".dctx0ctl/application-tests/report.json",
                &NativeCancellation::default(),
            )
            .await
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[tokio::test]
async fn bun_catalog_requires_entrypoint_and_captured_test_file_before_native_start() {
    for entrypoint in [None, Some("missing.test.ts"), Some("entry.test.ts")] {
        let f = Fixture::new();
        f.prepare();
        let catalog = json!({"schema_version":1,"collector_version":"dctx0-bun-registration-1","library_version":"0.2.0","complete":true,"tests":[{"entrypoint":"entry.test.ts","name":"TestYear","skipped":false,"focused":false}],"limitations":["root registrations only"]});
        fs::write(f.0.join("catalog.json"), catalog.to_string()).unwrap();
        let mut recipe = json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","test_directory":"root","prepare":[],"build":[],"catalog":{"program":"cat","arguments":["catalog.json"]},"run":{"program":"false","arguments":[]},"junit":null});
        if let Some(entrypoint) = entrypoint {
            recipe["entrypoint"] = entrypoint.into();
        }
        if entrypoint == Some("entry.test.ts") {
            fs::write(f.0.join("entry.test.ts"), "// captured test").unwrap();
            // A valid catalog cannot turn a nonzero native exit into PASS.
        }
        f.write("recipe.json", &recipe);
        let report = f.failed_native().await;
        assert!(!report.adoption_authority);
        assert!(!report.issues.is_empty());
        assert!(!matches!(report.result, VerificationResult::Pass));
        let message = &report.issues[0].message;
        match entrypoint {
            None => assert!(message.contains("entrypoint is required"), "{message}"),
            Some("missing.test.ts") => {
                assert!(message.contains("absent from captured"), "{message}")
            }
            _ => assert!(message.contains("native-tests exited"), "{message}"),
        }
    }
}

#[tokio::test]
async fn native_replay_rejects_bad_parent_missing_case_and_unbound_identity_sources() {
    let f = Fixture::new();
    f.prepare();
    f.write("recipe.json",&json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","test_directory":"root","prepare":[],"build":[],"catalog":null,"run":{"program":"false","arguments":[]},"junit":null}));
    let parent = f.failed_native().await;
    let original = serde_json::to_value(parent).unwrap();
    // Parent hashes and group are checked before any native process is launched.
    for mutation in 0..8 {
        let mut changed = original.clone();
        match mutation {
            0 => changed["schema_version"] = 2.into(),
            1 => changed["group"] = "foreign".into(),
            2 => changed["plan_hash"] = "foreign".into(),
            3 => {}
            _ => {
                changed["assessment"] = json!({"assessment_version":"dctx0-application-assessment-2","group":"native","result":{"status":"PASS"},"required_cases":[],"cases":[],"missing_cases":[],"missing_tests":[],"issues":[],"finalized":true});
                if mutation > 4 {
                    let bindings = match mutation {
                        5 => json!({}),
                        6 => {
                            json!({"source_cases":{"kind":"list","value":[{"kind":"boolean","value":true}]}})
                        }
                        _ => {
                            json!({"source_cases":{"kind":"list","value":[{"kind":"string","value":"absent"}]}})
                        }
                    };
                    changed["assessment"]["cases"] = json!([{"id":"identity","subject":"W.Year","kind":"entity-identity","bindings":bindings,"setup":[],"arguments":[],"observations":[],"events":null,"resources":null,"states":[],"result":{"status":"PASS"},"elapsed_us":0}]);
                }
            }
        }
        f.write("parent.json", &changed);
        let result = f
            .project()
            .replay_application_test(
                &f.0,
                "plan.json",
                "native",
                "recipe.json",
                "parent.json",
                if mutation > 4 { "identity" } else { "absent" },
                ".dctx0ctl/application-tests/replay.json",
                &NativeCancellation::default(),
            )
            .await;
        let error = result.unwrap_err().to_string();
        let expected = match mutation {
            0..=2 => "plan does not match",
            3 => "needs saved case",
            4 => "case is absent",
            5 => "actual source case IDs",
            6 => "Invalid identity source",
            _ => "source case is missing",
        };
        assert!(error.contains(expected), "mutation {mutation}: {error}");
    }
    // Sparse evidence cannot exhaust the JSON reader before its explicit cap.
    let file = fs::File::create(f.0.join("parent.json")).unwrap();
    file.set_len(67_108_865).unwrap();
    assert!(
        f.project()
            .replay_application_test(
                &f.0,
                "plan.json",
                "native",
                "recipe.json",
                "parent.json",
                "absent",
                ".dctx0ctl/application-tests/replay.json",
                &NativeCancellation::default()
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("64 MiB")
    );
}

#[tokio::test]
async fn successful_native_exit_still_requires_independent_bun_junit_evidence() {
    for junit in [None, Some(".dctx0ctl/application-tests/missing.xml")] {
        let f = Fixture::new();
        f.prepare();
        fs::write(f.0.join("entry.test.ts"), "// captured native root").unwrap();
        fs::write(f.0.join("catalog.json"), json!({"schema_version":1,"collector_version":"dctx0-bun-registration-1","library_version":"0.2.0","complete":true,"tests":[{"entrypoint":"entry.test.ts","name":"TestYear","skipped":false,"focused":false}],"limitations":["root registrations only"]}).to_string()).unwrap();
        f.write("recipe.json", &json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","entrypoint":"entry.test.ts","test_directory":"root","prepare":[],"build":[],"catalog":{"program":"cat","arguments":["catalog.json"]},"run":{"program":"true","arguments":[]},"junit":junit}));
        let report = f.failed_native().await;
        assert!(
            report
                .phases
                .iter()
                .any(|phase| phase.phase == "native-tests" && phase.audit.exit_code == Some(0))
        );
        assert!(!report.adoption_authority);
        assert!(!matches!(report.result, VerificationResult::Pass));
        assert!(
            report
                .assessment
                .as_ref()
                .is_some_and(|assessment| !assessment.finalized)
        );
        let error = &report.issues[0].message;
        if junit.is_none() {
            assert!(error.contains("JUnit report is required"), "{error}");
        } else {
            assert!(error.contains("No such file"), "{error}");
        }
    }
}

#[test]
fn preparation_and_bundle_reader_reject_invalid_group_resources_and_case_ownership() {
    let f = Fixture::new();
    for (group, test, seconds, expected) in [
        ("", "TestYear", 3, "INVALID_EXECUTION_GROUP"),
        ("native", "", 3, "INVALID_GROUP_SELECTION"),
        ("native", "TestYear", 0, "INVALID_GROUP_RESOURCES"),
    ] {
        let error = f
            .project()
            .prepare_application_tests(
                &f.0,
                &ApplicationTestingPreparation {
                    plan: "invalid-plan.json",
                    group,
                    test,
                    form: TestForm::Decoder,
                    group_seconds: Some(seconds),
                    stateless_decoders: Default::default(),
                    boundary: ApplicationValueBoundary::ValueConstructor,
                    subjects: Default::default(),
                },
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
        assert!(!f.0.join("invalid-plan.json").exists());
    }
    f.prepare();
    let original: Value =
        serde_json::from_slice(&fs::read(f.0.join("plan.json")).unwrap()).unwrap();
    for mutation in 0..2 {
        let mut changed = original.clone();
        if mutation == 0 {
            changed["plan"]["configuration"]["groups"][0]["resources"]["timeout_seconds"] =
                0.into();
        } else {
            changed["plan"]["case_groups"] = json!({});
        }
        f.write("plan.json", &changed);
        assert!(
            f.project()
                .read_application_testing_bundle(&f.0, "plan.json")
                .is_err()
        );
    }
}

#[tokio::test]
async fn missing_catalog_executable_retains_failed_start_without_native_or_adoption() {
    let f = Fixture::new();
    f.prepare();
    f.write("recipe.json",&json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","test_directory":"root","prepare":[],"build":[],"catalog":{"program":"/dctx0-absent-test-tool","arguments":[]},"run":{"program":"true","arguments":[]},"junit":null}));
    let report = f.failed_native().await;
    assert!(!report.issues.is_empty());
    assert!(report.phases.is_empty());
    assert!(report.assessment.is_none());
    assert!(!report.adoption_authority);
    assert!(!matches!(report.result, VerificationResult::Pass));
}

#[tokio::test]
async fn replay_parent_cannot_invent_group_or_cases_outside_central_manifest() {
    let f = Fixture::new();
    f.prepare();
    f.write("recipe.json",&json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","test_directory":"root","prepare":[],"build":[],"catalog":null,"run":{"program":"false","arguments":[]},"junit":null}));
    let mut parent = serde_json::to_value(f.failed_native().await).unwrap();
    parent["assessment"] = json!({"assessment_version":"dctx0-application-assessment-2","group":"native","result":{"status":"PASS"},"required_cases":[],"cases":[{"id":"invented","subject":"W.Year","kind":"contract","bindings":{},"setup":[],"arguments":[],"observations":[],"events":null,"resources":null,"states":[],"result":{"status":"PASS"},"elapsed_us":0}],"missing_cases":[],"missing_tests":[],"issues":[],"finalized":true});
    for group in ["foreign", "native"] {
        parent["group"] = group.into();
        f.write("parent.json", &parent);
        let error = f
            .project()
            .replay_application_test(
                &f.0,
                "plan.json",
                group,
                "recipe.json",
                "parent.json",
                "invented",
                ".dctx0ctl/application-tests/replay.json",
                &NativeCancellation::default(),
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if group == "foreign" {
                "Replay group is unavailable"
            } else {
                "Invalid replay ownership"
            }),
            "{error}"
        );
    }
}

#[tokio::test]
async fn native_finish_cannot_authorize_delivery_after_late_protocol_corruption() {
    let f = Fixture::new();
    f.prepare();
    fs::write(f.0.join("entry.test.ts"), "// captured native root").unwrap();
    fs::write(f.0.join("catalog.json"), json!({"schema_version":1,"collector_version":"dctx0-bun-registration-1","library_version":"0.2.0","complete":true,"tests":[{"entrypoint":"entry.test.ts","name":"TestYear","skipped":false,"focused":false}],"limitations":["root registrations only"]}).to_string()).unwrap();
    fs::write(
        f.0.join("late.py"),
        r#"import json,os,socket
s=socket.socket(socket.AF_UNIX)
s.connect(os.environ['DCTX0_APPLICATION_SOCKET'])
s.sendall((json.dumps({'protocol_version':2,'id':1,'payload':{'kind':'finish'}})+'\n').encode())
assert s.recv(65536)
s.sendall(b'broken frame after Finish\n')
while s.recv(65536): pass
s.close()
"#,
    )
    .unwrap();
    let python = std::env::var("DCTX0_TEST_PYTHON").unwrap_or_else(|_| "/usr/bin/python3".into());
    f.write("recipe.json", &json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","entrypoint":"entry.test.ts","test_directory":"root","prepare":[],"build":[],"catalog":{"program":"cat","arguments":["catalog.json"]},"run":{"program":python,"arguments":["late.py"]},"junit":null}));
    let report = f.failed_native().await;
    let assessment = report
        .assessment
        .as_ref()
        .expect("Service evidence must survive native cancellation");
    assert!(assessment.finalized, "{assessment:?}");
    assert!(
        assessment
            .issues
            .iter()
            .any(|issue| issue.code == "APPLICATION_TRANSPORT")
    );
    assert!(!assessment.missing_cases.is_empty());
    assert!(!matches!(assessment.result, VerificationResult::Pass));
    assert!(!report.adoption_authority);
    assert!(report.evidence.is_none());
    assert!(report.adopted_tasks.is_empty());
}

#[tokio::test]
async fn excessive_native_catalog_output_is_bounded_and_never_delivers_tasks() {
    let f = Fixture::new();
    f.prepare();
    let python = std::env::var("DCTX0_TEST_PYTHON").unwrap_or_else(|_| "/usr/bin/python3".into());
    f.write("recipe.json", &json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","entrypoint":"entry.test.ts","test_directory":"root","prepare":[],"build":[],"catalog":{"program":python,"arguments":["-c","import sys; sys.stdout.write('x' * 33554433); sys.stdout.flush()"]},"run":{"program":"false","arguments":[]},"junit":null}));
    let report = f.failed_native().await;
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.message.contains("log byte limit exceeded")),
        "{:?}",
        report.issues
    );
    assert!(report.assessment.is_none());
    assert!(!matches!(report.result, VerificationResult::Pass));
    assert!(!report.adoption_authority);
    assert!(report.evidence.is_none());
    assert!(report.adopted_tasks.is_empty());
}

#[tokio::test]
async fn scoped_execution_limits_are_retained_and_missing_test_directory_cannot_start() {
    for directory in ["root", "missing-tests"] {
        let f = Fixture::new();
        let config_path = f.0.join(".dctx0/testing/verification.json");
        let mut config: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        config["execution"] = json!({"initialization_seconds":4,"case_seconds":2,"run_seconds":13,"cleanup_seconds":1,"message_bytes":4096,"evidence_bytes":8192});
        fs::write(config_path, config.to_string()).unwrap();
        f.prepare();
        f.write("recipe.json", &json!({"schema_version":1,"runtime":"bun","root_test":"TestYear","entrypoint":"entry.test.ts","test_directory":directory,"prepare":[],"build":[],"catalog":{"program":"false","arguments":[]},"run":{"program":"false","arguments":[]},"junit":null}));
        if directory == "missing-tests" {
            let error = f
                .project()
                .run_application_tests(
                    &f.0,
                    "plan.json",
                    "native",
                    "recipe.json",
                    ".dctx0ctl/application-tests/report.json",
                    &NativeCancellation::default(),
                )
                .await
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("Native test directory is missing"),
                "{error}"
            );
            assert!(!f.0.join(".dctx0ctl/application-tests/report.json").exists());
        } else {
            let report = f.failed_native().await;
            let limits = report.limits.unwrap();
            assert_eq!(
                (
                    limits.initialization_seconds,
                    limits.case_seconds,
                    limits.cleanup_seconds
                ),
                (4, 2, 1)
            );
            assert_eq!((limits.message_bytes, limits.evidence_bytes), (4096, 8192));
            assert_eq!(
                limits.group_seconds, 3,
                "The explicit group budget remains authoritative"
            );
            assert!(!report.adoption_authority);
            assert!(report.evidence.is_none());
        }
    }
}

#[test]
fn explicit_plan_budget_guards_preparation_and_read_before_replacing_existing_plan() {
    let fixture = Fixture::new();
    let config_path = fixture.0.join(".dctx0/testing/verification.json");
    let mut config: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["application_tests"] = json!({"plan_bytes":1});
    fs::write(&config_path, config.to_string()).unwrap();
    fs::write(
        fixture.0.join("budget-plan.json"),
        b"preserved earlier model",
    )
    .unwrap();
    let options = ApplicationTestingPreparation {
        plan: "budget-plan.json",
        group: "native",
        test: "TestYear",
        form: TestForm::Decoder,
        group_seconds: Some(3),
        stateless_decoders: Default::default(),
        boundary: ApplicationValueBoundary::ValueConstructor,
        subjects: Default::default(),
    };
    let error = fixture
        .project()
        .prepare_application_tests(&fixture.0, &options)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("configured plan budget of 1 bytes"),
        "{error}"
    );
    assert_eq!(
        fs::read(fixture.0.join("budget-plan.json")).unwrap(),
        b"preserved earlier model"
    );
    config["application_tests"]["plan_bytes"] = 1_048_576.into();
    fs::write(&config_path, config.to_string()).unwrap();
    fixture
        .project()
        .prepare_application_tests(&fixture.0, &options)
        .unwrap();
    fixture
        .project()
        .read_application_testing_bundle(&fixture.0, "budget-plan.json")
        .unwrap();
    let prepared = fs::read(fixture.0.join("budget-plan.json")).unwrap();
    config["application_tests"]["plan_bytes"] = 1.into();
    fs::write(&config_path, config.to_string()).unwrap();
    let error = fixture
        .project()
        .read_application_testing_bundle(&fixture.0, "budget-plan.json")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("configured plan budget of 1 bytes"),
        "{error}"
    );
    assert_eq!(
        fs::read(fixture.0.join("budget-plan.json")).unwrap(),
        prepared
    );
}

#[test]
fn older_native_limits_without_plan_bytes_roundtrip_without_hash_changing_fields() {
    let old = json!({"initialization_seconds":30,"case_seconds":5,"group_seconds":120,"cleanup_seconds":5,"message_bytes":1048576,"evidence_bytes":4194304,"log_bytes":33554432});
    let limits: ApplicationNativeLimits = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(limits.plan_bytes, None);
    assert_eq!(serde_json::to_value(limits).unwrap(), old);
}
