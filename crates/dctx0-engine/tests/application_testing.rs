#![cfg(unix)]
use dctx0_core::*;
use dctx0_engine::*;
use dctx0_target_sdk::application_protocol::{AssessmentFrame, decode, encode};
use dctx0_verification::{application_plan::TestForm, application_session::*};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dctx0-app-{}-{}",
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
        fs::write(path.join(".dctx0/testing/verification.json"),serde_json::to_vec(&serde_json::json!({"schema_version":1,"target":"go","runtime":"go","adapter":"adapter.go","fixtures":"fixtures.json","scope":{"subjects":["W.Year"],"tasks":[]},"budgets":VerificationBudgets::default()})).unwrap()).unwrap();
        Self(path)
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
    fn prepare(&self) -> ApplicationTestingBundle {
        self.project()
            .prepare_application_tests(
                &self.0,
                &ApplicationTestingPreparation {
                    plan: "plan.json",
                    group: "native",
                    test: "TestYear",
                    form: TestForm::Decoder,
                    group_seconds: None,
                    boundary: ApplicationValueBoundary::ValueConstructor,
                    subjects: Default::default(),
                },
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn exported_bundle_revalidates_inputs_models_sources_versions_and_portable_paths() {
    let fixture = Fixture::new();
    let project = fixture.project();
    let config = fs::read(fixture.0.join(".dctx0/testing/verification.json")).unwrap();
    let original = fixture.prepare();
    let write = |bundle: &ApplicationTestingBundle| {
        fs::write(
            fixture.0.join("plan.json"),
            serde_json::to_vec(bundle).unwrap(),
        )
        .unwrap()
    };
    assert!(
        project
            .read_application_testing_bundle(&fixture.0, "plan.json")
            .is_ok()
    );
    assert_eq!(
        config,
        fs::read(fixture.0.join(".dctx0/testing/verification.json")).unwrap()
    );
    for mutation in 0..4 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.schema_version = 99,
            1 => {
                changed.plan.program.contracts[0].corpus.values[0] =
                    VerificationValue::Boolean(false)
            }
            2 => changed.model.data.clear(),
            _ => changed.spec_source_hash = "foreign".into(),
        };
        write(&changed);
        assert!(
            project
                .read_application_testing_bundle(&fixture.0, "plan.json")
                .is_err()
        );
    }
    write(&original);
    fs::write(
        fixture.0.join("spec.dctx0"),
        "workspace W { value Year = Int }",
    )
    .unwrap();
    assert!(
        project
            .read_application_testing_bundle(&fixture.0, "plan.json")
            .is_err()
    );
    assert!(
        project
            .prepare_application_tests(
                &fixture.0,
                &ApplicationTestingPreparation {
                    plan: "../outside",
                    group: "native",
                    test: "TestYear",
                    form: TestForm::Decoder,
                    group_seconds: None,
                    boundary: ApplicationValueBoundary::JsonDecoder,
                    subjects: Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn prepared_plan_hash_rejects_a_valid_assignment_swap_before_execution() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new();
    let project = fixture.project();
    let original = fixture.prepare();
    let bytes = fs::read(fixture.0.join("plan.json")).unwrap();
    let expected = format!("{:x}", Sha256::digest(&bytes));
    assert!(
        project
            .read_application_testing_bundle_with_hash(&fixture.0, "plan.json", &expected)
            .is_ok()
    );
    let mut changed = original;
    changed.plan.configuration.groups[0]
        .resources
        .timeout_seconds += 1;
    fs::write(
        fixture.0.join("plan.json"),
        serde_json::to_vec(&changed).unwrap(),
    )
    .unwrap();
    // Rebuilt program, manifest and ownership are still internally valid, so
    // the legacy unpinned reader accepts it; the captured byte pin refuses it.
    assert!(
        project
            .read_application_testing_bundle(&fixture.0, "plan.json")
            .is_ok()
    );
    assert!(
        project
            .read_application_testing_bundle_with_hash(&fixture.0, "plan.json", &expected)
            .is_err()
    );
    for invalid in [
        "not-a-sha256".to_owned(),
        "g".repeat(64),
        "A".repeat(64),
        "0".repeat(64),
    ] {
        let error = project
            .read_application_testing_bundle_with_hash(&fixture.0, "plan.json", &invalid)
            .unwrap_err();
        assert!(error.to_string().contains("SHA-256 changed"), "{error}");
    }
    fs::write(fixture.0.join("plan.json"), bytes).unwrap();
    fs::write(
        fixture.0.join("spec.dctx0"),
        "workspace W { value Year = Int }",
    )
    .unwrap();
    assert!(
        project
            .read_application_testing_bundle_with_hash(&fixture.0, "plan.json", &expected)
            .is_err(),
        "An unchanged plan hash must not bypass source freshness"
    );
}

async fn request(
    socket: &mut BufReader<tokio::net::UnixStream>,
    id: u64,
    request: ApplicationRequest,
) -> ApplicationReply {
    socket
        .get_mut()
        .write_all(&encode(&AssessmentFrame::new(id, request), 1_048_576).unwrap())
        .await
        .unwrap();
    let mut bytes = vec![];
    socket.read_until(b'\n', &mut bytes).await.unwrap();
    decode::<ApplicationReply>(&bytes, 1_048_576)
        .unwrap()
        .payload
}
async fn connect(path: &std::path::Path) -> BufReader<tokio::net::UnixStream> {
    for _ in 0..200 {
        if path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    BufReader::new(tokio::net::UnixStream::connect(path).await.unwrap())
}
#[tokio::test]
async fn local_channel_assesses_actual_values_and_preserves_late_or_malformed_failures() {
    for mutation in 0..3 {
        let fixture = Fixture::new();
        fixture.prepare();
        let project = fixture.project();
        let root = fixture.0.clone();
        let socket = fixture.0.join("s");
        let endpoint = socket.to_str().unwrap().to_owned();
        let database_before = fs::read(&project.database).unwrap();
        let service = tokio::spawn(async move {
            project
                .serve_application_tests(&root, "plan.json", "native", &endpoint, "report.json")
                .await
        });
        let mut client = connect(&socket).await;
        if mutation == 2 {
            client.get_mut().write_all(b"not-json\n").await.unwrap();
            drop(client);
            let report = service.await.unwrap().unwrap();
            assert!(matches!(report.result, VerificationResult::Unknown { .. }));
            assert!(!report.issues.is_empty());
            continue;
        }
        let mut id = 1;
        assert!(matches!(
            request(
                &mut client,
                id,
                ApplicationRequest::Register {
                    test: "TestYear".into()
                }
            )
            .await,
            ApplicationReply::Registered
        ));
        loop {
            id += 1;
            let (context, value) = match request(
                &mut client,
                id,
                ApplicationRequest::Next {
                    test: "TestYear".into(),
                },
            )
            .await
            {
                ApplicationReply::Case {
                    context,
                    directive: ApplicationDirective::Construct { value, .. },
                    ..
                } => (context, value),
                ApplicationReply::NoMoreCases => break,
                reply => panic!("{reply:?}"),
            };
            let accepted = matches!(&value,VerificationValue::Integer(n) if n=="3"||n=="4");
            let outcome = if accepted {
                DecoderOutcome::Accepted {
                    value: value.clone(),
                }
            } else {
                DecoderOutcome::Rejected {
                    reason: "out of range".into(),
                }
            };
            id += 1;
            request(
                &mut client,
                id,
                ApplicationRequest::Observe {
                    context,
                    observation: ApplicationObservation::Constructed {
                        input: value,
                        outcome,
                    },
                },
            )
            .await;
            id += 1;
            request(
                &mut client,
                id,
                ApplicationRequest::CloseCase {
                    context,
                    cleanup_error: None,
                },
            )
            .await;
        }
        id += 1;
        request(
            &mut client,
            id,
            ApplicationRequest::CloseTest {
                test: "TestYear".into(),
            },
        )
        .await;
        id += 1;
        assert!(matches!(
            request(&mut client, id, ApplicationRequest::Finish).await,
            ApplicationReply::Finished {
                summary: ApplicationAssessmentSummary {
                    status: ApplicationAssessmentStatus::Pass,
                    ..
                }
            }
        ));
        if mutation == 1 {
            id += 1;
            assert!(matches!(
                request(
                    &mut client,
                    id,
                    ApplicationRequest::Next {
                        test: "TestYear".into()
                    }
                )
                .await,
                ApplicationReply::Rejected { .. }
            ));
        }
        drop(client);
        let report = service.await.unwrap().unwrap();
        assert_eq!(
            matches!(report.result, VerificationResult::Pass),
            mutation == 0
        );
        assert!(!socket.exists());
        assert!(fixture.0.join("report.json").is_file());
        assert_eq!(
            database_before,
            fs::read(fixture.project().database).unwrap()
        );
    }
}

#[tokio::test]
async fn bounded_channel_rejects_oversize_frames_and_preserves_existing_socket_paths() {
    let fixture = Fixture::new();
    let mut bundle = fixture.prepare();
    bundle.plan.configuration.groups[0]
        .resources
        .max_message_bytes = 128;
    fs::write(
        fixture.0.join("plan.json"),
        serde_json::to_vec(&bundle).unwrap(),
    )
    .unwrap();
    let occupied = fixture.0.join("occupied");
    fs::write(&occupied, b"owned by another application").unwrap();
    assert!(
        fixture
            .project()
            .serve_application_tests(
                &fixture.0,
                "plan.json",
                "native",
                occupied.to_str().unwrap(),
                "unwritten.json"
            )
            .await
            .is_err()
    );
    assert_eq!(fs::read(occupied).unwrap(), b"owned by another application");
    let project = fixture.project();
    let root = fixture.0.clone();
    let socket = fixture.0.join("s");
    let endpoint = socket.to_str().unwrap().to_string();
    let server = tokio::spawn(async move {
        project
            .serve_application_tests(&root, "plan.json", "native", &endpoint, "report.json")
            .await
    });
    let mut client = connect(&socket).await;
    client.get_mut().write_all(&[b'x'; 129]).await.unwrap();
    drop(client);
    let report = server.await.unwrap().unwrap();
    assert!(matches!(report.result, VerificationResult::Unknown { .. }));
    assert!(report.issues.iter().any(|i| i.message.contains("budget")));
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn abstract_local_socket_requires_no_filesystem_entry_and_retains_partial_eof() {
    use std::os::linux::net::SocketAddrExt;
    let fixture = Fixture::new();
    fixture.prepare();
    let project = fixture.project();
    let root = fixture.0.clone();
    let name = format!(
        "dctx0-app-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let endpoint = format!("@{name}");
    let server = tokio::spawn(async move {
        project
            .serve_application_tests(&root, "plan.json", "native", &endpoint, "report.json")
            .await
    });
    let address = std::os::unix::net::SocketAddr::from_abstract_name(name).unwrap();
    let socket = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match std::os::unix::net::UnixStream::connect_addr(&address) {
                Ok(socket) => break socket,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(5)).await,
            }
        }
    })
    .await
    .unwrap();
    socket.set_nonblocking(true).unwrap();
    let mut socket = tokio::net::UnixStream::from_std(socket).unwrap();
    socket.write_all(b"{\"unfinished\":").await.unwrap();
    drop(socket);
    let report = server.await.unwrap().unwrap();
    assert!(matches!(report.result, VerificationResult::Unknown { .. }));
    assert!(!report.finalized);
    assert!(!report.missing_cases.is_empty());
}

#[tokio::test]
async fn managed_native_recipe_rejects_runtime_scope_paths_and_unbounded_configuration_before_build()
 {
    let fixture = Fixture::new();
    fixture.prepare();
    let project = fixture.project();
    let base = serde_json::json!({"schema_version":1,"runtime":"go","root_test":"TestYear","test_directory":"root","prepare":[],"build":[],"catalog":null,"run":{"program":"go","arguments":[]},"junit":null});
    for mutation in 0..8 {
        let mut recipe = base.clone();
        match mutation {
            0 => recipe["schema_version"] = 2.into(),
            1 => recipe["runtime"] = "bun".into(),
            2 => recipe["root_test"] = "Other".into(),
            3 => recipe["test_directory"] = "../outside".into(),
            4 => recipe["root_test"] = "".into(),
            5 => recipe["root_test"] = "bad\nroot".into(),
            6 => {
                recipe["prepare"] =
                    serde_json::json!(vec![serde_json::json!({"program":"go","arguments":[]}); 17])
            }
            _ => recipe["unknown"] = true.into(),
        }
        fs::write(
            fixture.0.join("native-recipe.json"),
            serde_json::to_vec(&recipe).unwrap(),
        )
        .unwrap();
        let result = project
            .run_application_tests(
                &fixture.0,
                "plan.json",
                "native",
                "native-recipe.json",
                ".dctx0ctl/application-tests/report.json",
                &NativeCancellation::default(),
            )
            .await;
        assert!(result.is_err(), "mutation {mutation}");
    }
    fs::write(
        fixture.0.join("native-recipe.json"),
        serde_json::to_vec(&base).unwrap(),
    )
    .unwrap();
    assert!(
        project
            .run_application_tests(
                &fixture.0,
                "plan.json",
                "native",
                "native-recipe.json",
                "../report.json",
                &NativeCancellation::default()
            )
            .await
            .is_err()
    );
    assert!(
        project
            .run_application_tests(
                &fixture.0,
                "plan.json",
                "other",
                "native-recipe.json",
                ".dctx0ctl/application-tests/report.json",
                &NativeCancellation::default()
            )
            .await
            .is_err()
    );
    fs::OpenOptions::new()
        .write(true)
        .open(fixture.0.join("native-recipe.json"))
        .unwrap()
        .set_len(65_537)
        .unwrap();
    assert!(
        project
            .run_application_tests(
                &fixture.0,
                "plan.json",
                "native",
                "native-recipe.json",
                ".dctx0ctl/application-tests/report.json",
                &NativeCancellation::default()
            )
            .await
            .is_err()
    );
}

#[test]
fn application_preparation_and_bundle_bounds_reject_invalid_specs_empty_scopes_and_large_inputs() {
    let fixture = Fixture::new();
    let project = fixture.project();
    let options = |group| ApplicationTestingPreparation {
        plan: "plan.json",
        group,
        test: "TestYear",
        form: TestForm::Decoder,
        group_seconds: None,
        boundary: ApplicationValueBoundary::ValueConstructor,
        subjects: Default::default(),
    };
    assert!(
        project
            .prepare_application_tests(&fixture.0, &options("legacy"))
            .is_err()
    );
    let mut empty = options("native");
    empty.form = TestForm::Lifecycle;
    assert!(
        project
            .prepare_application_tests(&fixture.0, &empty)
            .is_err()
    );
    fixture.prepare();
    fs::OpenOptions::new()
        .write(true)
        .open(fixture.0.join("plan.json"))
        .unwrap()
        .set_len(33_554_433)
        .unwrap();
    assert!(
        project
            .read_application_testing_bundle(&fixture.0, "plan.json")
            .is_err()
    );
    fs::write(fixture.0.join("spec.dctx0"), "workspace W { broken").unwrap();
    assert!(
        project
            .prepare_application_tests(&fixture.0, &options("native"))
            .is_err()
    );
}

#[test]
fn library_publication_checks_all_paths_and_old_hashes_before_replacing_owned_files() {
    use dctx0_target_sdk::{ApplicationTestLibrary, Artifact};
    let fixture = Fixture::new();
    let library = ApplicationTestLibrary {
        target: "go".into(),
        version: "0.2.0".into(),
        protocol_version: 2,
        artifacts: vec![Artifact {
            path: "helper.go".into(),
            contents: "package helper\n".into(),
        }],
    };
    let publish =
        |library: &ApplicationTestLibrary| publish_application_test_library(&fixture.0, library);
    for mutation in 0..6 {
        let mut invalid = library.clone();
        match mutation {
            0 => invalid.target = "java".into(),
            1 => invalid.protocol_version = 1,
            2 => invalid.artifacts[0].path = "../authored.go".into(),
            3 => invalid.artifacts.push(invalid.artifacts[0].clone()),
            4 => invalid.artifacts[0].contents = "x".repeat(1_048_577),
            _ => invalid.artifacts.clear(),
        }
        assert!(publish(&invalid).is_err(), "mutation {mutation}");
        assert!(
            !fixture
                .0
                .join(".dctx0/application-tests/library/helper.go")
                .exists()
        );
    }
    publish(&library).unwrap();
    let directory = fixture.0.join(".dctx0/application-tests/library");
    let pin = fixture.0.join(".dctx0/application-tests/library-pin.json");
    let original_pin = fs::read(&pin).unwrap();
    let mut upgraded = library.clone();
    upgraded.version = "0.2.1".into();
    upgraded.artifacts[0].contents = "package updated\n".into();
    publish(&upgraded).unwrap();
    assert_eq!(
        fs::read(directory.join("helper.go")).unwrap(),
        b"package updated\n"
    );
    let upgrade_pin = fs::read(&pin).unwrap();
    fs::write(directory.join("helper.go"), "edited").unwrap();
    assert!(publish(&library).is_err());
    assert_eq!(fs::read(&pin).unwrap(), upgrade_pin);
    fs::write(&pin, r#"{"files":{"../outside":"fake"}}"#).unwrap();
    assert!(publish(&library).is_err());
    fs::write(&pin, r#"{"files":{"missing.go":"fake"}}"#).unwrap();
    assert!(publish(&library).is_err());
    fs::write(&pin, r#"{"files":{"helper.go":false}}"#).unwrap();
    assert!(publish(&library).is_err());
    fs::write(&pin, "{}").unwrap();
    assert!(publish(&library).is_err());
    fs::write(&pin, &original_pin).unwrap();
    fs::write(directory.join("helper.go"), &library.artifacts[0].contents).unwrap();
    fs::write(directory.join("foreign.go"), "authored").unwrap();
    let mut foreign = library.clone();
    foreign.artifacts.push(Artifact {
        path: "foreign.go".into(),
        contents: "replacement".into(),
    });
    assert!(publish(&foreign).is_err());
    assert_eq!(fs::read(directory.join("foreign.go")).unwrap(), b"authored");
    fs::OpenOptions::new()
        .write(true)
        .open(&pin)
        .unwrap()
        .set_len(1_048_577)
        .unwrap();
    assert!(publish(&library).is_err());
}

#[tokio::test]
async fn managed_group_budget_and_external_cancellation_terminate_preparation_without_success_receipts()
 {
    for external in [false, true] {
        let fixture = Fixture::new();
        let mut bundle = fixture.prepare();
        bundle.plan.configuration.groups[0]
            .resources
            .timeout_seconds = 1;
        fs::write(
            fixture.0.join("plan.json"),
            serde_json::to_vec(&bundle).unwrap(),
        )
        .unwrap();
        let python = std::env::var("DCTX0_TEST_PYTHON").unwrap_or_else(|_| "python3".into());
        let recipe = serde_json::json!({"schema_version":1,"runtime":"go","root_test":"TestYear","test_directory":"root","prepare":[{"program":python,"arguments":["-c","import time; time.sleep(30)"]}],"build":[],"catalog":null,"run":{"program":"go","arguments":[]},"junit":null});
        fs::write(
            fixture.0.join("native-recipe.json"),
            serde_json::to_vec(&recipe).unwrap(),
        )
        .unwrap();
        let cancel = NativeCancellation::default();
        if external {
            cancel.cancel();
        }
        let report = fixture
            .project()
            .run_application_tests(
                &fixture.0,
                "plan.json",
                "native",
                "native-recipe.json",
                ".dctx0ctl/application-tests/report.json",
                &cancel,
            )
            .await
            .unwrap();
        assert!(!matches!(report.result, VerificationResult::Pass));
        assert!(!report.adoption_authority);
        assert!(report.assessment.is_none());
        assert!(!report.issues.is_empty());
        let saved: ApplicationNativeReport = serde_json::from_slice(
            &fs::read(fixture.0.join(".dctx0ctl/application-tests/report.json")).unwrap(),
        )
        .unwrap();
        assert!(!matches!(saved.result, VerificationResult::Pass));
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn case_deadlines_and_evidence_budgets_keep_unfinished_native_prefixes_incomplete() {
    for overflow in [false, true] {
        let fixture = Fixture::new();
        let config_path = fixture.0.join(".dctx0/testing/verification.json");
        let mut config: serde_json::Value =
            serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        config["execution"] = serde_json::json!({"initialization_seconds":30,"case_seconds":1,"run_seconds":30,"cleanup_seconds":5,"evidence_bytes":if overflow{100}else{4_194_304}});
        fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
        fixture.prepare();
        let project = fixture.project();
        let root = fixture.0.clone();
        let endpoint = format!(
            "@dctx0-case-limit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let peer = endpoint.clone();
        let service = tokio::spawn(async move {
            project
                .serve_application_tests(&root, "plan.json", "native", &endpoint, "report.json")
                .await
        });
        use std::os::linux::net::SocketAddrExt;
        let address =
            std::os::unix::net::SocketAddr::from_abstract_name(peer.trim_start_matches('@'))
                .unwrap();
        let mut socket = loop {
            match std::os::unix::net::UnixStream::connect_addr(&address) {
                Ok(socket) => {
                    socket.set_nonblocking(true).unwrap();
                    break BufReader::new(tokio::net::UnixStream::from_std(socket).unwrap());
                }
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(5)).await,
            }
        };
        request(
            &mut socket,
            1,
            ApplicationRequest::Register {
                test: "TestYear".into(),
            },
        )
        .await;
        let reply = request(
            &mut socket,
            2,
            ApplicationRequest::Next {
                test: "TestYear".into(),
            },
        )
        .await;
        let ApplicationReply::Case { context, .. } = reply else {
            panic!("Expected active case")
        };
        if overflow {
            let frame = AssessmentFrame::new(
                3,
                ApplicationRequest::Observe {
                    context,
                    observation: ApplicationObservation::BindingError {
                        message: "x".repeat(200),
                    },
                },
            );
            socket
                .get_mut()
                .write_all(&encode(&frame, 1_048_576).unwrap())
                .await
                .unwrap();
        }
        let report = tokio::time::timeout(std::time::Duration::from_secs(10), service)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!matches!(report.result, VerificationResult::Pass));
        assert!(!report.finalized);
        assert!(!report.missing_cases.is_empty());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains(if overflow {
                    "evidence budget"
                } else {
                    "deadline exceeded"
                })),
            "{:?}",
            report.issues
        );
    }
}

#[tokio::test]
async fn small_valid_request_cannot_emit_case_reply_beyond_channel_budget() {
    let fixture = Fixture::new();
    let mut bundle = fixture.prepare();
    bundle.plan.configuration.groups[0]
        .resources
        .max_message_bytes = 128;
    fs::write(
        fixture.0.join("plan.json"),
        serde_json::to_vec(&bundle).unwrap(),
    )
    .unwrap();
    let project = fixture.project();
    let root = fixture.0.clone();
    let socket = fixture.0.join("reply-budget");
    let endpoint = socket.to_str().unwrap().to_owned();
    let service = tokio::spawn(async move {
        project
            .serve_application_tests(&root, "plan.json", "native", &endpoint, "reply-budget.json")
            .await
    });
    let mut client = connect(&socket).await;
    assert!(matches!(
        request(
            &mut client,
            1,
            ApplicationRequest::Register {
                test: "TestYear".into()
            }
        )
        .await,
        ApplicationReply::Registered
    ));
    let next = encode(
        &AssessmentFrame::new(
            2,
            ApplicationRequest::Next {
                test: "TestYear".into(),
            },
        ),
        128,
    )
    .unwrap();
    assert!(next.len() <= 128);
    client.get_mut().write_all(&next).await.unwrap();
    let mut reply = vec![];
    assert_eq!(
        client.read_until(b'\n', &mut reply).await.unwrap(),
        0,
        "Oversize replies must not be partially sent"
    );
    let report = service.await.unwrap().unwrap();
    assert!(!report.finalized);
    assert!(!report.missing_cases.is_empty());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "APPLICATION_TRANSPORT" && issue.message.contains("budget")),
        "{:?}",
        report.issues
    );
    assert!(!matches!(report.result, VerificationResult::Pass));
}

#[tokio::test]
async fn protocol_rejects_foreign_versions_unsafe_ids_unknown_requests_and_repeated_correlations() {
    for mutation in 0..5 {
        let fixture = Fixture::new();
        fixture.prepare();
        let project = fixture.project();
        let root = fixture.0.clone();
        let socket = fixture.0.join("invalid-frame");
        let endpoint = socket.to_str().unwrap().to_owned();
        let service = tokio::spawn(async move {
            project
                .serve_application_tests(
                    &root,
                    "plan.json",
                    "native",
                    &endpoint,
                    "invalid-frame.json",
                )
                .await
        });
        let mut client = connect(&socket).await;
        let mut frame = serde_json::to_value(AssessmentFrame::new(
            1,
            ApplicationRequest::Register {
                test: "TestYear".into(),
            },
        ))
        .unwrap();
        match mutation {
            0 => frame["protocol_version"] = 99.into(),
            1 => frame["id"] = 0.into(),
            2 => frame["id"] = 9_007_199_254_740_992u64.into(),
            3 => frame["payload"] = serde_json::json!({"kind":"invented"}),
            _ => {
                assert!(matches!(
                    request(
                        &mut client,
                        1,
                        ApplicationRequest::Register {
                            test: "TestYear".into()
                        }
                    )
                    .await,
                    ApplicationReply::Registered
                ));
            }
        }
        let mut bytes = serde_json::to_vec(&frame).unwrap();
        bytes.push(b'\n');
        client.get_mut().write_all(&bytes).await.unwrap();
        drop(client);
        let report = service.await.unwrap().unwrap();
        assert!(!report.finalized);
        assert!(!report.missing_cases.is_empty());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "APPLICATION_TRANSPORT"),
            "mutation {mutation}: {:?}",
            report.issues
        );
        if mutation == 4 {
            assert!(
                report
                    .issues
                    .iter()
                    .any(|issue| issue.message.contains("consecutive"))
            );
        }
        assert!(!matches!(report.result, VerificationResult::Pass));
    }
}
