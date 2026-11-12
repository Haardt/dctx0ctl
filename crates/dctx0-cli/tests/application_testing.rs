#![cfg(unix)]
use dctx0_core::*;
use dctx0_target_sdk::application_protocol::{AssessmentFrame, decode, encode};
use dctx0_verification::application_session::*;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn application_testing_cli_publishes_explicitly_and_returns_nonzero_for_incomplete_native_groups() {
    assert_eq!(
        dctx0_target_sdk::application_protocol::APPLICATION_PROTOCOL_VERSION,
        dctx0_verification::application_evidence::APPLICATION_EXECUTION_PROTOCOL_VERSION
    );
    let root = std::env::temp_dir().join(format!("dctx0-app-cli-{}", std::process::id()));
    fs::create_dir_all(root.join(".dctx0/testing")).unwrap();
    fs::write(
        root.join("spec.dctx0"),
        "workspace W { value Year = Int { range 3..4 } }",
    )
    .unwrap();
    fs::write(root.join("fixtures.json"), r#"{"examples":{}}"#).unwrap();
    let config=serde_json::to_vec(&serde_json::json!({"schema_version":1,"target":"go","runtime":"go","adapter":"adapter.go","fixtures":"fixtures.json","scope":{"subjects":["W.Year"],"tasks":[]},"budgets":VerificationBudgets::default()})).unwrap();
    fs::write(root.join(".dctx0/testing/verification.json"), &config).unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let path = root.to_str().unwrap();
    for complete in [true, false] {
        let mut prepare = Command::new(binary);
        prepare.args([
            "verify",
            "application-tests",
            "prepare",
            path,
            "--project",
            path,
            "--plan",
            "plan.json",
            "--group",
            "native",
            "--test-name",
            "TestYear",
            "--boundary",
            "value-constructor",
        ]);
        if complete {
            prepare.arg("--json");
        }
        let prepared = prepare.output().unwrap();
        assert!(
            prepared.status.success(),
            "{}",
            String::from_utf8_lossy(&prepared.stderr)
        );
        if complete {
            let json: serde_json::Value = serde_json::from_slice(&prepared.stdout).unwrap();
            assert_eq!(json["boundary"], "value-constructor");
        } else {
            assert!(String::from_utf8_lossy(&prepared.stdout).contains("No execution"));
        }
        let socket = root.join("s");
        let mut command = Command::new(binary);
        command.args([
            "verify",
            "application-tests",
            "serve",
            path,
            "--project",
            path,
            "--plan",
            "plan.json",
            "--group",
            "native",
            "--socket",
            socket.to_str().unwrap(),
            "--report",
            "report.json",
        ]);
        if !complete {
            command.arg("--json");
        }
        let mut child = command
            .stdout(fs::File::create(root.join("service.stdout")).unwrap())
            .stderr(fs::File::create(root.join("service.stderr")).unwrap())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while !socket.exists() && started.elapsed() < Duration::from_secs(10) {
            assert!(child.try_wait().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut client = BufReader::new(UnixStream::connect(&socket).unwrap());
        client
            .get_mut()
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut id = 0;
        let mut request = |request: ApplicationRequest| -> ApplicationReply {
            id += 1;
            client
                .get_mut()
                .write_all(&encode(&AssessmentFrame::new(id, request), 1_048_576).unwrap())
                .unwrap();
            let mut bytes = vec![];
            client.read_until(b'\n', &mut bytes).unwrap();
            decode::<ApplicationReply>(&bytes, 1_048_576)
                .unwrap()
                .payload
        };
        request(ApplicationRequest::Register {
            test: "TestYear".into(),
        });
        if complete {
            loop {
                let (context, input) = match request(ApplicationRequest::Next {
                    test: "TestYear".into(),
                }) {
                    ApplicationReply::Case {
                        context,
                        directive: ApplicationDirective::Construct { value, .. },
                        ..
                    } => (context, value),
                    ApplicationReply::NoMoreCases => break,
                    reply => panic!("{reply:?}"),
                };
                let accepted = matches!(&input,VerificationValue::Integer(n) if n=="3"||n=="4");
                let outcome = if accepted {
                    DecoderOutcome::Accepted {
                        value: input.clone(),
                    }
                } else {
                    DecoderOutcome::Rejected {
                        reason: "outside range".into(),
                    }
                };
                request(ApplicationRequest::Observe {
                    context,
                    observation: ApplicationObservation::Constructed { input, outcome },
                });
                request(ApplicationRequest::CloseCase {
                    context,
                    cleanup_error: None,
                });
            }
            request(ApplicationRequest::CloseTest {
                test: "TestYear".into(),
            });
        }
        request(ApplicationRequest::Finish);
        drop(client);
        let started = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if started.elapsed() > Duration::from_secs(10) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("assessment child did not terminate")
            };
            std::thread::sleep(Duration::from_millis(5));
        }
        let status = child.wait().unwrap();
        assert_eq!(
            status.success(),
            complete,
            "{}",
            fs::read_to_string(root.join("service.stderr")).unwrap()
        );
        let report: ApplicationAssessmentReport =
            serde_json::from_slice(&fs::read(root.join("report.json")).unwrap()).unwrap();
        assert_eq!(matches!(report.result, VerificationResult::Pass), complete);
        if !complete {
            let report: ApplicationAssessmentReport =
                serde_json::from_slice(&fs::read(root.join("service.stdout")).unwrap()).unwrap();
            assert!(!report.missing_cases.is_empty());
        }
    }
    assert_eq!(
        fs::read(root.join(".dctx0/testing/verification.json")).unwrap(),
        config
    );
    fs::remove_dir_all(root).unwrap();
}

// These real native peers also execute under cargo-llvm-cov. Running only the
// outer Python validation stages would leave their Rust protocol paths unmeasured.
#[cfg(all(feature = "go", feature = "typescript", target_os = "linux"))]
#[test]
fn actual_go_and_bun_application_pilots_preserve_the_original_finite_inputs() {
    use std::path::Path;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let directory =
        std::env::temp_dir().join(format!("dctx0-real-app-pilots-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    for (target, forms) in [
        (
            "go",
            ["decoder", "operation", "state-change", "assertion", "all"],
        ),
        (
            "typescript",
            [
                "constructor",
                "operation",
                "state-change",
                "assertion",
                "all",
            ],
        ),
    ] {
        for form in forms {
            let output = directory.join(format!("{target}-{form}"));
            let mut command = Command::new(
                std::env::var("DCTX0_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
            );
            command.current_dir(repository);
            if target == "go" {
                command
                    .args([
                        "scripts/application_test_acceptance.py",
                        "--binary",
                        binary,
                        "--form",
                        form,
                        "--report",
                    ])
                    .arg(output.join("result.json"));
            } else {
                command
                    .args([
                        "examples/cinema/scripts/application-acceptance.py",
                        "--cli",
                        binary,
                        "--form",
                        form,
                        "--output",
                    ])
                    .arg(&output);
            }
            let result = command.output().unwrap();
            assert!(
                result.status.success(),
                "{target}/{form}: {}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            let receipt: serde_json::Value =
                serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
            if target == "go" {
                assert_eq!(receipt["status"], "pass");
                assert_eq!(receipt["comparison"]["required_cases"], 5766);
            } else {
                assert_eq!(receipt["exit_code"], 0);
                assert_eq!(receipt["comparison"]["current_required_cases"], 5766);
            }
        }
    }
    for target in ["go", "typescript"] {
        let output = directory.join(format!("{target}-managed"));
        let result =
            Command::new(std::env::var("DCTX0_TEST_PYTHON").unwrap_or_else(|_| "python3".into()))
                .current_dir(repository)
                .args([
                    "scripts/application_test_managed_acceptance.py",
                    "--replay",
                    "--binary",
                    binary,
                    "--target",
                    target,
                    "--report",
                ])
                .arg(output.join("result.json"))
                .output()
                .unwrap();
        assert!(
            result.status.success(),
            "managed {target}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    // Runner failure after central PASS must not deliver any task. The other
    // probes reject missing registration/build evidence before production runs.
    for (target, probes) in [
        (
            "go",
            vec!["native-assertion", "missing-test", "build-failure"],
        ),
        (
            "typescript",
            vec!["missing-test", "build-failure", "source-drift"],
        ),
    ] {
        for probe in probes {
            let output = directory.join(format!("{target}-managed-probe-{probe}"));
            let result = Command::new(
                std::env::var("DCTX0_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
            )
            .current_dir(repository)
            .args([
                "scripts/application_test_managed_acceptance.py",
                "--binary",
                binary,
                "--target",
                target,
                "--probe",
                probe,
                "--report",
            ])
            .arg(output.join("result.json"))
            .output()
            .unwrap();
            assert!(
                result.status.success(),
                "managed {target}/{probe}: {}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
    fs::remove_dir_all(directory).unwrap();
}

#[cfg(all(feature = "go", feature = "typescript"))]
#[test]
fn shipped_testing_libraries_publish_offline_and_preserve_authored_or_edited_files() {
    let root =
        std::env::temp_dir().join(format!("dctx0-library-publication-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    for target in ["go", "typescript"] {
        let app = root.join(target);
        fs::create_dir_all(app.join(".dctx0/application-tests")).unwrap();
        let authored = app.join(".dctx0/application-tests/authored.test.txt");
        fs::write(&authored, "authored test must remain").unwrap();
        let publish = || {
            Command::new(env!("CARGO_BIN_EXE_dctx0"))
                .args(["verify", "application-tests", "library", "--project"])
                .arg(&app)
                .args(["--target", target, "--json"])
                .output()
                .unwrap()
        };
        let first = publish();
        assert!(
            first.status.success(),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        let pin = app.join(".dctx0/application-tests/library-pin.json");
        let before = fs::read(&pin).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(value["version"], "0.2.0");
        assert_eq!(value["protocol_version"], 2);
        assert!(publish().status.success());
        assert_eq!(fs::read(&pin).unwrap(), before);
        let library = app.join(".dctx0/application-tests/library");
        let modified = library.join(if target == "go" {
            "client.go"
        } else {
            "client.ts"
        });
        fs::write(&modified, "hand edited library").unwrap();
        assert!(!publish().status.success());
        assert_eq!(fs::read(&modified).unwrap(), b"hand edited library");
        assert_eq!(fs::read(&pin).unwrap(), before);
        assert_eq!(fs::read(&authored).unwrap(), b"authored test must remain");
    }
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_dctx0"))
            .args(["verify", "application-tests", "library", "--project"])
            .arg(&root)
            .args(["--target", "java"])
            .status()
            .unwrap()
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}
