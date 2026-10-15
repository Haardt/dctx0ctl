use dctx0_core::{
    Invocation, InvocationObservation, Observation, ResourceAccess, ResourceObservation,
    VerificationEvent, VerificationValue,
};
use dctx0_target_go::GoHarness;
use dctx0_target_sdk::*;
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    sync::mpsc::{Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn subjects() -> BTreeSet<String> {
    BTreeSet::from(["operation".into(), "value".into(), "Entity".into()])
}

fn integer(value: i64) -> VerificationValue {
    VerificationValue::Integer(value.to_string())
}

fn text(value: &str) -> VerificationValue {
    VerificationValue::String(value.into())
}

fn begin(case_id: &str) -> WorkerRequest {
    WorkerRequest::Begin {
        case_id: case_id.into(),
    }
}

fn invoke(invocation: Invocation) -> WorkerRequest {
    WorkerRequest::Invoke { invocation }
}

fn increment(amount: i64) -> Invocation {
    Invocation {
        subject: "operation".into(),
        arguments: vec![integer(amount)],
    }
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "dctx0-go-worker-{}-{}-{}",
        label,
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn generate(adapter_import: &str, selected: BTreeSet<String>) -> HarnessBundle {
    GoHarness
        .generate(&HarnessRequest {
            adapter_import: adapter_import.into(),
            subjects: selected,
        })
        .unwrap()
}

fn write_bundle(root: &Path, bundle: &HarnessBundle) {
    fs::write(root.join("go.mod"), "module dctx0testing\n\ngo 1.25.0\n").unwrap();
    for artifact in &bundle.artifacts {
        fs::create_dir_all(root.join(&artifact.path).parent().unwrap()).unwrap();
        fs::write(root.join(&artifact.path), &artifact.contents).unwrap();
    }
}

/// Fixture directory with the counter application and the observed-call adapter.
fn counter_root(label: &str) -> (PathBuf, HarnessBundle) {
    let root = temp_root(label);
    let bundle = generate("./adapter.go", subjects());
    write_bundle(&root, &bundle);
    fs::write(root.join("counter.go"), include_str!("fixtures/counter.go")).unwrap();
    fs::write(root.join("adapter.go"), include_str!("fixtures/adapter.go")).unwrap();
    (root, bundle)
}

struct Worker {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    id: u64,
    protocol_samples: Vec<serde_json::Value>,
    measured: Vec<serde_json::Value>,
}

impl Worker {
    fn spawn(root: &Path, bundle: &HarnessBundle) -> Self {
        if let Some(build) = &bundle.build {
            let output = Command::new(&build.program)
                .args(&build.arguments)
                .env("GOTOOLCHAIN", "local")
                .env("GOPROXY", "off")
                .env("CGO_ENABLED", "0")
                .env("GOWORK", "off")
                .current_dir(root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let mut child = Command::new(&bundle.recipe.program)
            .args(&bundle.recipe.arguments)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Go toolchain is required for executable target conformance");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin: Some(stdin),
            lines: receiver,
            id: 0,
            protocol_samples: Vec::new(),
            measured: Vec::new(),
        }
    }

    fn send(&mut self, request: WorkerRequest) -> u64 {
        self.id += 1;
        let bytes = encode_request(
            &RequestEnvelope {
                protocol_version: SDK_VERSION,
                id: self.id,
                request,
            },
            MAX_MESSAGE_BYTES,
        )
        .unwrap();
        self.stdin.as_mut().unwrap().write_all(&bytes).unwrap();
        self.id
    }

    fn raw(&mut self, text: &str) {
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(format!("{text}\n").as_bytes())
            .unwrap();
    }

    fn next_message(&mut self) -> WorkerMessage {
        let line = match self.lines.recv_timeout(Duration::from_secs(120)) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => panic!("Go worker reply timed out"),
            Err(RecvTimeoutError::Disconnected) => panic!("Go worker closed stdout unexpectedly"),
        };
        let sample: serde_json::Value = serde_json::from_str(&line).unwrap();
        self.protocol_samples.push(sample);
        decode_message(line.as_bytes(), MAX_MESSAGE_BYTES)
            .unwrap()
            .message
    }

    fn request(&mut self, request: WorkerRequest) -> WorkerReply {
        let expected = self.send(request);
        let reply = match self.next_message() {
            WorkerMessage::Reply { id, reply } => {
                assert_eq!(id, expected);
                reply
            }
            other => panic!("expected reply, got {other:?}"),
        };
        if let WorkerReply::Measured {
            peak_rss_bytes,
            runtime_version,
            platform,
        } = &reply
        {
            self.measured.push(serde_json::json!({
                "peak_rss_bytes": peak_rss_bytes,
                "runtime_version": runtime_version,
                "platform": platform,
            }));
        }
        reply
    }

    fn exchange(&mut self, request: WorkerRequest) -> WorkerMessage {
        self.send(request);
        self.next_message()
    }

    fn expect_technical(&mut self, request: WorkerRequest, category: &str) -> String {
        match self.exchange(request) {
            WorkerMessage::TechnicalError {
                category: actual,
                message,
                ..
            } => {
                assert_eq!(actual, category);
                message
            }
            other => panic!("expected technical error, got {other:?}"),
        }
    }

    fn wait_exit(&mut self) -> (bool, String) {
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "Go worker did not exit");
            std::thread::sleep(Duration::from_millis(25));
        };
        let mut stderr = String::new();
        if let Some(mut stream) = self.child.stderr.take() {
            stream.read_to_string(&mut stderr).unwrap();
        }
        (status.success(), stderr)
    }
}

#[test]
fn generated_worker_is_deterministic_scaffolds_are_unavailable_and_adapters_cannot_escape() {
    let request = HarnessRequest {
        adapter_import: "./adapter.go".into(),
        subjects: subjects(),
    };
    let bundle = GoHarness.generate(&request).unwrap();
    assert_eq!(bundle, GoHarness.generate(&request).unwrap());
    assert_eq!(bundle.target, "go");
    assert_eq!(bundle.generator_version, "dctx0-go-harness-2");
    assert_eq!(bundle.sdk_version, 1);
    assert_eq!(bundle.recipe.program, "./.bin/worker");
    assert!(bundle.recipe.arguments.is_empty());
    assert_eq!(bundle.build.as_ref().unwrap().program, "go");
    assert_eq!(bundle.artifacts.len(), 3);
    assert!(!bundle.artifacts[1].contents.contains("__SUBJECTS__"));
    assert!(bundle.artifacts[0].contents.contains("package main"));
    assert!(bundle.artifacts[2].contents.contains("func prepare()"));
    let scaffold = GoHarness.scaffold();
    assert_eq!(scaffold.path, "adapter.go");
    assert!(scaffold.contents.contains("not implemented"));
    for path in [
        "adapter.go",
        "adapter.ts",
        "./adapter.ts",
        "/absolute.go",
        "./../escape.go",
        "./folder/adapter.go",
        "./folder//adapter.go",
        "./a\\b.go",
        "./a\nb.go",
        "./.go",
        "./",
        "",
    ] {
        let mut invalid = request.clone();
        invalid.adapter_import = path.into();
        assert!(
            GoHarness
                .generate(&invalid)
                .unwrap_err()
                .to_string()
                .contains("Harness input"),
            "expected rejection for {path:?}"
        );
    }
    for collision in ["./contract.go", "./worker.go"] {
        let mut colliding = request.clone();
        colliding.adapter_import = collision.into();
        assert!(
            GoHarness
                .generate(&colliding)
                .unwrap_err()
                .to_string()
                .contains("replace generated harness")
        );
    }
    let mut slot = request.clone();
    slot.adapter_import = "./__SUBJECTS__.go".into();
    slot.subjects.insert("__SUBJECTS__".into());
    let generated = GoHarness.generate(&slot).unwrap();
    assert!(
        generated.artifacts[1]
            .contents
            .contains("\"__SUBJECTS__\": true")
    );
    let mut escaped = request.clone();
    escaped.subjects.insert("we\"ird\\subject".into());
    let generated = GoHarness.generate(&escaped).unwrap();
    assert!(
        generated.artifacts[1]
            .contents
            .contains("we\\\"ird\\\\subject")
    );
    let empty = generate("./adapter.go", BTreeSet::new());
    assert!(empty.artifacts[1].contents.contains("map[string]bool{}"));
}

#[test]
fn real_go_worker_conforms_to_sessions_observation_mock_transport_and_failures() {
    let (root, bundle) = counter_root("conformance");
    let mut worker = Worker::spawn(&root, &bundle);

    let boot = Instant::now();
    match worker.request(WorkerRequest::Metrics) {
        WorkerReply::Measured {
            peak_rss_bytes,
            runtime_version,
            platform,
        } => {
            assert!(
                peak_rss_bytes.unwrap_or(0) > 0,
                "peak RSS must be measured on Linux"
            );
            assert!(runtime_version.starts_with("go"));
            assert!(!platform.is_empty());
        }
        other => panic!("expected metrics, got {other:?}"),
    }
    match worker.request(WorkerRequest::Describe) {
        WorkerReply::Described { description } => {
            assert_eq!(description.protocol_version, 1);
            assert_eq!(description.subjects.len(), 3);
            assert_eq!(description.artifacts, ["counter.go"]);
            assert!(
                description
                    .capabilities
                    .contains(&"call-observation".to_string())
            );
            assert_eq!(
                description.representations.get("Int"),
                Some(&"text integers".to_string())
            );
        }
        other => panic!("expected description, got {other:?}"),
    }
    match worker.request(begin("first")) {
        WorkerReply::Begun { case_id } => assert_eq!(case_id, "first"),
        other => panic!("expected begun, got {other:?}"),
    }
    let _initialization = boot.elapsed();

    match worker.request(invoke(increment(2))) {
        WorkerReply::Invoked {
            observation,
            events,
            calls,
            resources,
        } => {
            assert_eq!(observation, Observation::Returned { value: integer(2) });
            assert_eq!(
                events,
                [VerificationEvent {
                    subject: "CounterChanged".into(),
                    payload: integer(2),
                }]
            );
            let calls = calls.expect("call-observation capability must publish actual calls");
            assert_eq!(
                calls,
                [InvocationObservation {
                    invocation: increment(2),
                    observation: Observation::Returned { value: integer(2) },
                }]
            );
            assert_eq!(resources.as_ref().map(Vec::len), Some(0));
        }
        other => panic!("expected invocation result, got {other:?}"),
    }

    match worker.request(WorkerRequest::Observe {
        entities: vec!["Entity".into()],
    }) {
        WorkerReply::Observed { states } => {
            let rows = states.get("Entity").expect("observed Entity state");
            assert_eq!(rows.len(), 1);
            match &rows[0] {
                VerificationValue::Record(fields) => {
                    assert_eq!(fields.get("count"), Some(&integer(2)));
                }
                other => panic!("expected entity record, got {other:?}"),
            }
        }
        other => panic!("expected observed states, got {other:?}"),
    }

    let invoke_id = worker.send(invoke(Invocation {
        subject: "operation".into(),
        arguments: vec![text("mock")],
    }));
    let mock_request_id = match worker.next_message() {
        WorkerMessage::MockCall {
            request_id,
            invocation,
        } => {
            assert_eq!(invocation.subject, "port");
            assert_eq!(invocation.arguments, [integer(2)]);
            request_id
        }
        other => panic!("expected mock call, got {other:?}"),
    };
    worker.send(WorkerRequest::MockReply {
        request_id: mock_request_id,
        observation: Observation::Returned { value: integer(7) },
    });
    match worker.next_message() {
        WorkerMessage::Reply { id, reply } => {
            assert_eq!(id, invoke_id);
            match reply {
                WorkerReply::Invoked {
                    observation,
                    events,
                    calls,
                    resources,
                } => {
                    assert_eq!(observation, Observation::Returned { value: integer(7) });
                    assert_eq!(
                        events,
                        [VerificationEvent {
                            subject: "CounterChanged".into(),
                            payload: integer(7),
                        }]
                    );
                    assert_eq!(calls.as_ref().map(Vec::len), Some(1));
                    assert_eq!(resources.as_ref().map(Vec::len), Some(0));
                }
                other => panic!("expected mock invocation result, got {other:?}"),
            }
        }
        other => panic!("expected reply, got {other:?}"),
    }

    match worker.request(WorkerRequest::Validate {
        subject: "value".into(),
        value: integer(1),
    }) {
        WorkerReply::ValueChecked { outcome } => {
            assert_eq!(outcome, ValueCheck::Accepted { value: integer(1) })
        }
        other => panic!("expected accepted validation, got {other:?}"),
    }
    match worker.request(WorkerRequest::Validate {
        subject: "value".into(),
        value: text("bad"),
    }) {
        WorkerReply::ValueChecked { outcome } => match outcome {
            ValueCheck::Rejected { reason } => assert!(reason.contains("integer")),
            other => panic!("expected rejected validation, got {other:?}"),
        },
        other => panic!("expected rejected validation, got {other:?}"),
    }

    assert!(matches!(
        worker.request(WorkerRequest::End),
        WorkerReply::Ended
    ));
    assert!(matches!(
        worker.request(begin("fresh")),
        WorkerReply::Begun { .. }
    ));
    match worker.request(invoke(Invocation {
        subject: "operation".into(),
        arguments: vec![text("application-panic")],
    })) {
        WorkerReply::Invoked { observation, .. } => assert_eq!(
            observation,
            Observation::Threw {
                message: "Application panic: Actual production panic".into()
            }
        ),
        other => panic!("expected actual application panic, got {other:?}"),
    }
    match worker.request(invoke(Invocation {
        subject: "operation".into(),
        arguments: vec![text("exception")],
    })) {
        WorkerReply::Invoked {
            observation,
            events,
            calls,
            resources,
        } => {
            assert_eq!(
                observation,
                Observation::Threw {
                    message: "Unexpected application exception".into()
                }
            );
            assert!(events.is_empty());
            assert_eq!(calls.as_ref().map(Vec::len), Some(1));
            assert_eq!(resources.as_ref().map(Vec::len), Some(0));
        }
        other => panic!("expected threw observation, got {other:?}"),
    }

    assert!(matches!(
        worker.request(WorkerRequest::Shutdown),
        WorkerReply::Bye
    ));
    let (success, stderr) = worker.wait_exit();
    if success {
        let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/native-harness-conformance");
        fs::create_dir_all(&evidence).unwrap();
        for sample in &worker.protocol_samples {
            decode_message(&serde_json::to_vec(sample).unwrap(), MAX_MESSAGE_BYTES).unwrap();
        }
        let report = serde_json::json!({
            "schema_version": 1,
            "scope": "Go transport conformance; not full Go verification",
            "target": "go",
            "samples": worker.measured,
            "protocolSamples": worker.protocol_samples,
            "conformance_status": "pass",
            "native_stderr": stderr,
        });
        fs::write(
            evidence.join(format!("go-{}.json", std::process::id())),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
    fs::remove_dir_all(&root).unwrap();
    assert!(
        success,
        "worker must exit successfully after shutdown: {stderr}"
    );
    assert!(stderr.contains("actual application output"));
}

#[test]
fn adapter_failures_and_missing_capabilities_are_technical_never_success() {
    let scenarios: [Vec<WorkerRequest>; 9] = [
        vec![begin("init-error")],
        vec![invoke(increment(1))],
        vec![
            begin("normal"),
            invoke(Invocation {
                subject: "unselected".into(),
                arguments: vec![],
            }),
        ],
        vec![
            begin("normal"),
            invoke(Invocation {
                subject: "operation".into(),
                arguments: vec![text("adapter-error")],
            }),
        ],
        vec![begin("cleanup-error"), WorkerRequest::End],
        vec![begin("normal"), begin("again")],
        vec![
            begin("normal"),
            WorkerRequest::Validate {
                subject: "unselected".into(),
                value: integer(1),
            },
        ],
        vec![
            begin("normal"),
            WorkerRequest::Observe {
                entities: vec!["unselected".into()],
            },
        ],
        vec![
            begin("normal"),
            invoke(Invocation {
                subject: "operation".into(),
                arguments: vec![text("invalid-resource")],
            }),
        ],
    ];
    for requests in scenarios {
        let (root, bundle) = counter_root("failure");
        let mut worker = Worker::spawn(&root, &bundle);
        let mut last = None;
        for request in requests {
            last = Some(worker.exchange(request));
        }
        match last {
            Some(WorkerMessage::TechnicalError { category, .. }) => {
                assert_eq!(category, "adapter-or-request")
            }
            other => panic!("expected technical failure, got {other:?}"),
        }
        worker.stdin = None;
        let (success, stderr) = worker.wait_exit();
        fs::remove_dir_all(&root).unwrap();
        assert!(success, "failed scenario must still exit cleanly: {stderr}");
    }

    let root = temp_root("plain");
    let bundle = generate("./adapter.go", subjects());
    write_bundle(&root, &bundle);
    fs::write(
        root.join("adapter.go"),
        include_str!("fixtures/adapter_plain.go"),
    )
    .unwrap();
    let mut worker = Worker::spawn(&root, &bundle);
    match worker.request(WorkerRequest::Describe) {
        WorkerReply::Described { description } => {
            assert_eq!(description.protocol_version, 1);
            assert!(description.subjects.is_empty());
            assert!(description.artifacts.is_empty());
            assert!(description.capabilities.is_empty());
            assert!(description.representations.is_empty());
        }
        other => panic!("expected normalized description, got {other:?}"),
    }
    assert!(matches!(
        worker.request(begin("plain")),
        WorkerReply::Begun { .. }
    ));
    match worker.request(WorkerRequest::Validate {
        subject: "value".into(),
        value: integer(1),
    }) {
        WorkerReply::ValueChecked { outcome } => match outcome {
            ValueCheck::Unavailable { reason } => {
                assert!(reason.contains("No production value factory/decoder"))
            }
            other => panic!("expected unavailable validation, got {other:?}"),
        },
        other => panic!("expected unavailable validation, got {other:?}"),
    }
    match worker.request(invoke(increment(1))) {
        WorkerReply::Invoked {
            observation,
            events,
            calls,
            resources,
        } => {
            assert_eq!(
                observation,
                Observation::Returned {
                    value: VerificationValue::Unit
                }
            );
            assert!(events.is_empty());
            assert!(
                calls.is_none(),
                "missing call-observation capability must omit calls"
            );
            assert!(
                resources.is_none(),
                "missing resource-observation capability must omit resources"
            );
        }
        other => panic!("expected unit invocation, got {other:?}"),
    }
    let message = worker.expect_technical(
        invoke(Invocation {
            subject: "operation".into(),
            arguments: vec![text("record")],
        }),
        "adapter-or-request",
    );
    assert!(message.contains("No active resource observer"));
    assert!(matches!(
        worker.request(begin("plain-record")),
        WorkerReply::Begun { .. }
    ));
    let message = worker.expect_technical(
        WorkerRequest::Observe {
            entities: vec!["Entity".into()],
        },
        "adapter-or-request",
    );
    assert!(message.contains("No actual state observer"));
    worker.stdin = None;
    let (success, stderr) = worker.wait_exit();
    fs::remove_dir_all(&root).unwrap();
    assert!(success, "plain adapter session must exit cleanly: {stderr}");

    let root = temp_root("scaffold");
    let bundle = generate("./adapter.go", BTreeSet::new());
    write_bundle(&root, &bundle);
    fs::write(root.join("adapter.go"), GoHarness.scaffold().contents).unwrap();
    let mut worker = Worker::spawn(&root, &bundle);
    match worker.request(WorkerRequest::Describe) {
        WorkerReply::Described { description } => {
            assert_eq!(description.protocol_version, 1);
            assert!(description.subjects.is_empty());
        }
        other => panic!("expected scaffold description, got {other:?}"),
    }
    let message = worker.expect_technical(begin("unimplemented"), "adapter-or-request");
    assert!(message.contains("not implemented"));
    worker.stdin = None;
    let (success, stderr) = worker.wait_exit();
    fs::remove_dir_all(&root).unwrap();
    assert!(success, "scaffold worker must exit cleanly: {stderr}");
}

#[test]
fn protocol_violations_release_pending_collaborators_and_never_report_success() {
    let (root, bundle) = counter_root("protocol");
    let mut worker = Worker::spawn(&root, &bundle);
    for (frame, category) in [
        ("{", "protocol"),
        (
            r#"{"protocol_version":2,"id":1,"request":{"kind":"describe"}}"#,
            "protocol",
        ),
        (
            r#"{"protocol_version":1,"id":1,"request":{"kind":"describe","PASS":true}}"#,
            "adapter-or-request",
        ),
        (
            r#"{"protocol_version":1,"id":1,"request":{"kind":"mock-reply","request_id":99,"observation":{"kind":"returned","value":{"kind":"unit"}}}}"#,
            "protocol",
        ),
    ] {
        worker.raw(frame);
        match worker.next_message() {
            WorkerMessage::TechnicalError {
                category: actual, ..
            } => assert_eq!(actual, category, "frame {frame}"),
            other => panic!("expected technical error for {frame}, got {other:?}"),
        }
    }
    assert!(matches!(
        worker.request(WorkerRequest::Shutdown),
        WorkerReply::Bye
    ));
    let (success, stderr) = worker.wait_exit();
    fs::remove_dir_all(&root).unwrap();
    assert!(success, "protocol recovery must stay runnable: {stderr}");

    let (root, bundle) = counter_root("eof");
    let mut worker = Worker::spawn(&root, &bundle);
    assert!(matches!(
        worker.request(begin("eof")),
        WorkerReply::Begun { .. }
    ));
    worker.send(invoke(Invocation {
        subject: "operation".into(),
        arguments: vec![text("mock")],
    }));
    match worker.next_message() {
        WorkerMessage::MockCall { request_id, .. } => {
            assert!(request_id > 0);
        }
        other => panic!("expected mock call, got {other:?}"),
    }
    worker.stdin = None;
    match worker.next_message() {
        WorkerMessage::Reply { reply, .. } => match reply {
            WorkerReply::Invoked { observation, .. } => assert_eq!(
                observation,
                Observation::Threw {
                    message: "Worker input closed".into()
                }
            ),
            other => panic!("expected released collaborator, got {other:?}"),
        },
        other => panic!("expected reply after EOF, got {other:?}"),
    }
    let (success, stderr) = worker.wait_exit();
    fs::remove_dir_all(&root).unwrap();
    assert!(success, "EOF must close the session cleanly: {stderr}");
}

#[test]
fn recorded_observations_retain_their_value_at_observation_time() {
    let (root, bundle) = counter_root("snapshot");
    let mut worker = Worker::spawn(&root, &bundle);
    assert!(matches!(
        worker.request(begin("snapshot")),
        WorkerReply::Begun { .. }
    ));
    match worker.request(invoke(Invocation {
        subject: "operation".into(),
        arguments: vec![text("mutable-observation")],
    })) {
        WorkerReply::Invoked {
            observation,
            events,
            resources,
            ..
        } => {
            assert_eq!(observation, Observation::Returned { value: integer(1) });
            assert_eq!(
                resources.expect("resource-observation capability must publish resources"),
                [ResourceObservation {
                    operation: None,
                    resource: "Counter".into(),
                    access: ResourceAccess::Write,
                }]
            );
            let snapshot = events
                .iter()
                .find(|event| event.subject == "Snapshot")
                .expect("Snapshot event");
            assert_eq!(
                snapshot.payload,
                integer(1),
                "recorded payloads must retain their value at record time"
            );
        }
        other => panic!("expected mutable observation result, got {other:?}"),
    }
    assert!(matches!(
        worker.request(WorkerRequest::End),
        WorkerReply::Ended
    ));
    assert!(matches!(
        worker.request(begin("fresh-resource-observation")),
        WorkerReply::Begun { .. }
    ));
    match worker.request(invoke(increment(1))) {
        WorkerReply::Invoked {
            observation,
            resources,
            ..
        } => {
            assert_eq!(observation, Observation::Returned { value: integer(1) });
            assert_eq!(resources.as_ref().map(Vec::len), Some(0));
        }
        other => panic!("expected fresh-session invocation, got {other:?}"),
    }
    assert!(matches!(
        worker.request(WorkerRequest::Shutdown),
        WorkerReply::Bye
    ));
    let (success, stderr) = worker.wait_exit();
    fs::remove_dir_all(&root).unwrap();
    assert!(success, "observation snapshot session must exit: {stderr}");
}

#[test]
fn observation_overflow_is_explicit_and_new_sessions_start_with_a_fresh_budget() {
    let (root, bundle) = counter_root("overflow");
    let mut worker = Worker::spawn(&root, &bundle);
    assert!(matches!(
        worker.request(begin("observation-overflow")),
        WorkerReply::Begun { .. }
    ));
    let message = worker.expect_technical(
        invoke(Invocation {
            subject: "operation".into(),
            arguments: vec![text("observation-limit")],
        }),
        "adapter-or-request",
    );
    assert!(message.contains("Observation budget exceeded"));
    assert!(matches!(
        worker.request(begin("caught-overflow")),
        WorkerReply::Begun { .. }
    ));
    let message = worker.expect_technical(
        invoke(Invocation {
            subject: "operation".into(),
            arguments: vec![text("caught-observation-limit")],
        }),
        "adapter-or-request",
    );
    assert!(
        message.contains("Observation budget exceeded"),
        "swallowed record failures must still be reported: {message}"
    );
    assert!(matches!(
        worker.request(WorkerRequest::End),
        WorkerReply::Ended
    ));
    assert!(matches!(
        worker.request(begin("fresh-budget")),
        WorkerReply::Begun { .. }
    ));
    match worker.request(invoke(increment(1))) {
        WorkerReply::Invoked { observation, .. } => {
            assert_eq!(observation, Observation::Returned { value: integer(1) });
        }
        other => panic!("expected fresh budget invocation, got {other:?}"),
    }
    assert!(matches!(
        worker.request(WorkerRequest::Shutdown),
        WorkerReply::Bye
    ));
    let (success, stderr) = worker.wait_exit();
    fs::remove_dir_all(&root).unwrap();
    assert!(success, "budget session must exit: {stderr}");
}

#[test]
fn go_dependency_resolution_uses_the_module_aware_toolchain() {
    let (root, bundle) = counter_root("resolve");
    let mut worker = Worker::spawn(&root, &bundle);
    let importer = root.join("adapter.go").to_string_lossy().into_owned();
    let request = WorkerRequest::ResolveDependency {
        mode: DependencyResolution::Import,
        specifier: "fmt".into(),
        importer: importer.clone(),
    };
    let resolved = match worker.request(request.clone()) {
        WorkerReply::DependencyResolved { path } => path,
        other => panic!("expected resolved dependency, got {other:?}"),
    };
    assert!(Path::new(&resolved).is_absolute());
    assert!(
        resolved.ends_with("/fmt"),
        "expected stdlib directory, got {resolved}"
    );
    match worker.request(request) {
        WorkerReply::DependencyResolved { path } => assert_eq!(path, resolved),
        other => panic!("expected cached resolution, got {other:?}"),
    }
    match worker.exchange(WorkerRequest::ResolveDependency {
        mode: DependencyResolution::Require,
        specifier: "fmt".into(),
        importer: importer.clone(),
    }) {
        WorkerMessage::TechnicalError { category, .. } => {
            assert_eq!(category, "adapter-or-request")
        }
        other => panic!("expected require-mode rejection, got {other:?}"),
    }
    let message = worker.expect_technical(
        WorkerRequest::ResolveDependency {
            mode: DependencyResolution::Import,
            specifier: "./missing-local".into(),
            importer: importer.clone(),
        },
        "adapter-or-request",
    );
    assert!(message.contains("Dependency resolution failed"));
    let message = worker.expect_technical(
        WorkerRequest::ResolveDependency {
            mode: DependencyResolution::Import,
            specifier: "".into(),
            importer,
        },
        "adapter-or-request",
    );
    assert!(message.contains("Invalid dependency resolution request"));
    assert!(matches!(
        worker.request(WorkerRequest::Shutdown),
        WorkerReply::Bye
    ));
    let (success, stderr) = worker.wait_exit();
    fs::remove_dir_all(&root).unwrap();
    assert!(success, "resolution session must exit: {stderr}");
}

#[test]
fn adapter_and_protocol_faults_wait_for_session_resource_release() {
    for kind in ["adapter", "protocol", "panic"] {
        let (root, bundle) = counter_root("cleanup-order");
        let mut worker = Worker::spawn(&root, &bundle);
        let case_id = format!("cleanup-order-{kind}");
        assert!(matches!(
            worker.request(begin(&case_id)),
            WorkerReply::Begun { .. }
        ));
        let response = if kind == "adapter" {
            worker.exchange(invoke(Invocation {
                subject: "operation".into(),
                arguments: vec![text("adapter-error")],
            }))
        } else if kind == "protocol" {
            worker.raw("{broken}");
            worker.next_message()
        } else {
            worker.exchange(WorkerRequest::End)
        };
        assert!(matches!(response, WorkerMessage::TechnicalError { .. }));
        assert_eq!(
            fs::read_to_string(root.join(format!("{case_id}.closed"))).unwrap(),
            "closed"
        );
        worker.stdin = None;
        let (success, stderr) = worker.wait_exit();
        assert!(success, "{stderr}");
        fs::remove_dir_all(root).unwrap();
    }
}
