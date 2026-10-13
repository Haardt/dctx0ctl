use dctx0_core::*;
use dctx0_engine::{
    NativeCancellation, NativeCommand, NativeFailure, NativeFailureKind, NativeLimits,
    NativeSessionEvidence, NativeWorker,
};
use dctx0_target_sdk::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "dctx0-managed-native-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn recipe(code: &str) -> ExecutionRecipe {
    ExecutionRecipe {
        program: "python3".into(),
        arguments: vec!["-u".into(), "-c".into(), code.into()],
    }
}
fn limits() -> NativeLimits {
    NativeLimits {
        initialization: Duration::from_millis(200),
        case: Duration::from_millis(200),
        run: Duration::from_secs(3),
        cleanup: Duration::from_secs(1),
        ..Default::default()
    }
}
fn no_mock(_: &Invocation) -> Result<Observation, NativeFailure> {
    panic!("Unexpected mock in transport fixture")
}

#[tokio::test]
async fn evidence_limits_keep_the_acknowledged_prefix_reap_workers_and_reset_for_new_cases() {
    let code = r#"import sys,json
for line in sys.stdin:
 p=json.loads(line);r=p['request'];k=r['kind']
 if k=='begin': reply={'kind':'begun','case_id':r['case_id']}
 elif k=='validate': reply={'kind':'value-checked','outcome':{'kind':'accepted','value':r['value']}}
 elif k=='end': reply={'kind':'ended'}
 else: reply={'kind':'bye'}
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':reply}}),flush=True)
 if k=='shutdown':break
"#;
    let t = Temp::new();
    let cancel = NativeCancellation::default();
    for constrained in [
        NativeLimits {
            evidence_entries: 1,
            ..limits()
        },
        NativeLimits {
            evidence_bytes: 100,
            ..limits()
        },
    ] {
        let mut worker = NativeWorker::spawn(&recipe(code), &t.0, constrained)
            .await
            .unwrap();
        let pid = worker.pid();
        let failure = worker
            .execute_value_case(
                "limited",
                "W.Value",
                &VerificationValue::String("payload".repeat(20)),
                &cancel,
            )
            .await
            .unwrap_err();
        assert_eq!(failure.kind, NativeFailureKind::MessageLimit);
        assert!(failure.detail.contains("retained prefix is incomplete"));
        assert!(matches!(
            worker.session_evidence(),
            [NativeSessionEvidence::Begun { .. }]
        ));
        assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
        worker.abort().await.unwrap();
    }
    let mut worker = NativeWorker::spawn(
        &recipe(code),
        &t.0,
        NativeLimits {
            evidence_entries: 3,
            evidence_bytes: 512,
            ..limits()
        },
    )
    .await
    .unwrap();
    for id in ["first", "second"] {
        let outcome = worker
            .execute_value_case(
                id,
                "W.Value",
                &VerificationValue::Integer("1".into()),
                &cancel,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, ValueCheck::Accepted { .. }));
        assert_eq!(worker.session_evidence().len(), 3);
    }
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
}

#[tokio::test]
async fn runtime_identity_binds_the_managed_executable_and_initial_environment() {
    let temporary = Temp::new();
    // Identity capture requires a live worker, not a process that races its
    // shutdown acknowledgement against reads of /proc/<pid>.
    let handshake = r#"
import sys,json
for line in sys.stdin:
 p=json.loads(line)
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':{'kind':'described','description':{'protocol_version':1,'subjects':[],'artifacts':[],'capabilities':[],'representations':{}}}}}),flush=True)
"#;
    let mut worker = NativeWorker::spawn(&recipe(handshake), &temporary.0, NativeLimits::default())
        .await
        .unwrap();
    assert_eq!(
        worker.runtime_identity().unwrap_err().kind,
        NativeFailureKind::Technical
    );
    assert!(matches!(
        worker
            .request(
                WorkerRequest::Describe,
                &NativeCancellation::default(),
                &mut no_mock
            )
            .await
            .unwrap(),
        WorkerReply::Described { .. }
    ));
    let identity = worker.runtime_identity().unwrap();
    assert_eq!(identity.schema_version, 1);
    assert_eq!(identity.executable_hash.len(), 64);
    assert_eq!(identity.initial_environment_hash.len(), 64);
    assert!(identity.executable_bytes > 0);
    assert_eq!(identity, worker.runtime_identity().unwrap());
    assert!(identity.still_current().unwrap());
    worker.abort().await.unwrap();
    assert_eq!(
        worker.runtime_identity().unwrap_err().kind,
        NativeFailureKind::WorkerExit
    );
}
const ECHO: &str = r#"
import sys,json
print('native application log',file=sys.stderr,flush=True)
for line in sys.stdin:
 p=json.loads(line);kind=p['request']['kind']
 if kind=='invoke':
  print(json.dumps({'protocol_version':1,'message':{'kind':'mock-call','request_id':1,'invocation':{'subject':'port','arguments':[]}}}),flush=True)
  answer=json.loads(sys.stdin.readline())['request']['observation']
  reply={'kind':'invoked','observation':answer,'events':[]}
 else: reply={'kind':'bye'}
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':reply}}),flush=True)
 if kind=='shutdown': break
"#;
#[tokio::test]
async fn real_process_callbacks_logs_graceful_exit_and_repeated_abort_are_managed() {
    let t = Temp::new();
    assert!(NativeWorker::supported());
    let started = Instant::now();
    let mut w = NativeWorker::spawn(&recipe(ECHO), &t.0, limits())
        .await
        .unwrap();
    let cancel = NativeCancellation::default();
    let mut mock = |call: &Invocation| {
        assert_eq!(call.subject, "port");
        Ok(Observation::Returned {
            value: VerificationValue::Integer("7".into()),
        })
    };
    let result = w
        .request(
            WorkerRequest::Invoke {
                invocation: Invocation {
                    subject: "op".into(),
                    arguments: vec![],
                },
            },
            &cancel,
            &mut mock,
        )
        .await
        .unwrap();
    assert!(matches!(
        result,
        WorkerReply::Invoked {
            resources: None,
            observation: Observation::Returned { .. },
            ..
        }
    ));
    let status = fs::read_to_string(format!("/proc/{}/status", w.pid())).unwrap();
    let peak = status
        .lines()
        .find(|s| s.starts_with("VmHWM:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<u64>()
        .unwrap()
        * 1024;
    let report = serde_json::json!({"scope":"managed Python transport fixture; not application verification","elapsed_ms":started.elapsed().as_secs_f64()*1000.,"peak_rss_bytes":peak});
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-executor-conformance");
    fs::create_dir_all(&evidence).unwrap();
    fs::write(
        evidence.join(format!("{}.json", std::process::id())),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(
        w.request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
            .await
            .unwrap(),
        WorkerReply::Bye
    );
    let audit = w.finish().await.unwrap();
    assert_eq!(audit.exit_code, Some(0));
    assert!(audit.stderr.contains("native application log"));
    assert_eq!(audit.received_messages, 3);
    assert!(!PathBuf::from(format!("/proc/{}", w.pid())).exists());
    w.abort().await.unwrap();
    assert_eq!(
        w.request(WorkerRequest::Describe, &cancel, &mut no_mock)
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::WorkerExit
    );
    assert_eq!(
        w.finish().await.unwrap_err().kind,
        NativeFailureKind::WorkerExit
    );
}
#[tokio::test]
async fn missing_runtime_invalid_limits_timeouts_and_cancellation_never_return_a_reply() {
    let t = Temp::new();
    let mut missing = recipe("");
    missing.program = "/missing/dctx0-runtime".into();
    assert_eq!(
        NativeWorker::spawn(&missing, &t.0, limits())
            .await
            .err()
            .unwrap()
            .kind,
        NativeFailureKind::Unavailable
    );
    for bad in [
        NativeLimits {
            case: Duration::ZERO,
            ..limits()
        },
        NativeLimits {
            run: Duration::MAX,
            ..limits()
        },
        NativeLimits {
            log_bytes: 0,
            ..limits()
        },
        NativeLimits {
            message_bytes: MAX_MESSAGE_BYTES + 1,
            ..limits()
        },
        NativeLimits {
            evidence_bytes: 0,
            ..limits()
        },
        NativeLimits {
            evidence_entries: 0,
            ..limits()
        },
    ] {
        assert_eq!(
            NativeWorker::spawn(&recipe(""), &t.0, bad)
                .await
                .err()
                .unwrap()
                .kind,
            NativeFailureKind::InvalidLimits
        );
    }
    for cancelled in [false, true] {
        let mut w = NativeWorker::spawn(&recipe("import time;time.sleep(300)"), &t.0, limits())
            .await
            .unwrap();
        let pid = w.pid();
        let cancel = NativeCancellation::default();
        if cancelled {
            cancel.cancel();
        }
        let failure = w
            .request(WorkerRequest::Describe, &cancel, &mut no_mock)
            .await
            .unwrap_err();
        assert_eq!(
            failure.kind,
            if cancelled {
                NativeFailureKind::Cancelled
            } else {
                NativeFailureKind::Timeout
            }
        );
        assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
    }
    let mut w = NativeWorker::spawn(&recipe("import time;time.sleep(300)"), &t.0, limits())
        .await
        .unwrap();
    let cancel = NativeCancellation::default();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(15)).await;
        trigger.cancel();
    });
    assert_eq!(
        w.request(WorkerRequest::Describe, &cancel, &mut no_mock)
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Cancelled
    );
}
#[tokio::test]
async fn malformed_oversized_incomplete_and_unsolicited_messages_are_terminal() {
    let t = Temp::new();
    for (code, kind) in [
        ("print('{',flush=True)", NativeFailureKind::Protocol),
        (
            "import sys;sys.stdout.write('{}');sys.stdout.flush()",
            NativeFailureKind::Protocol,
        ),
        ("print('x'*300,flush=True)", NativeFailureKind::MessageLimit),
        (
            "import sys;print('x'*300,file=sys.stderr,flush=True)",
            NativeFailureKind::LogLimit,
        ),
        (
            "print('{\"protocol_version\":1,\"message\":{\"kind\":\"reply\",\"id\":999,\"reply\":{\"kind\":\"bye\"}}}',flush=True)",
            NativeFailureKind::Protocol,
        ),
        (
            "print('{\"protocol_version\":1,\"message\":{\"kind\":\"technical-error\",\"id\":1,\"category\":\"cleanup\",\"message\":\"failed\"}}',flush=True)",
            NativeFailureKind::Technical,
        ),
        ("pass", NativeFailureKind::WorkerExit),
    ] {
        let mut w = NativeWorker::spawn(
            &recipe(code),
            &t.0,
            NativeLimits {
                message_bytes: 128,
                log_bytes: 32,
                ..limits()
            },
        )
        .await
        .unwrap();
        let failure = w
            .request(
                WorkerRequest::Describe,
                &NativeCancellation::default(),
                &mut no_mock,
            )
            .await
            .unwrap_err();
        assert_eq!(failure.kind, kind, "{code}: {failure:?}");
    }
}
#[tokio::test]
async fn cleanup_kills_and_reaps_descendants_and_drop_is_a_last_resort() {
    let t = Temp::new();
    let pid_file = t.0.join("child.pid");
    for (drop_only, detached) in [(false, false), (true, false), (false, true), (true, true)] {
        let code = format!(
            "import subprocess,time,pathlib;child=subprocess.Popen(['python3','-c','import time;time.sleep(300)'],start_new_session={});p=pathlib.Path({:?});pending=p.with_suffix('.pending');pending.write_text(str(child.pid));pending.replace(p);time.sleep(300)",
            if detached { "True" } else { "False" },
            pid_file
        );
        let mut w = NativeWorker::spawn(&recipe(&code), &t.0, limits())
            .await
            .unwrap();
        let start = Instant::now();
        while !pid_file.exists() {
            assert!(start.elapsed() < Duration::from_secs(2));
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        let child = fs::read_to_string(&pid_file)
            .unwrap()
            .parse::<u32>()
            .unwrap();
        let parent = w.pid();
        if drop_only {
            drop(w);
        } else {
            w.abort().await.unwrap();
            drop(w);
        }
        let start = Instant::now();
        while PathBuf::from(format!("/proc/{child}")).exists()
            || PathBuf::from(format!("/proc/{parent}")).exists()
        {
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "descendants remain: parent={parent}, child={child}, drop={drop_only}, detached={detached}"
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        fs::remove_file(&pid_file).unwrap();
    }
}
#[tokio::test]
async fn acknowledged_shutdown_still_requires_a_real_clean_exit() {
    let t = Temp::new();
    let code = "import sys,json,time;p=json.loads(sys.stdin.readline());print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':{'kind':'bye'}}}),flush=True);time.sleep(300)";
    let mut w = NativeWorker::spawn(
        &recipe(code),
        &t.0,
        NativeLimits {
            cleanup: Duration::from_millis(25),
            ..limits()
        },
    )
    .await
    .unwrap();
    w.request(
        WorkerRequest::Shutdown,
        &NativeCancellation::default(),
        &mut no_mock,
    )
    .await
    .unwrap();
    assert_eq!(
        w.finish().await.unwrap_err().kind,
        NativeFailureKind::Timeout
    );
    let mut w = NativeWorker::spawn(&recipe("import sys;sys.exit(3)"), &t.0, limits())
        .await
        .unwrap();
    assert_eq!(
        w.finish().await.unwrap_err().kind,
        NativeFailureKind::WorkerExit
    );
    let extra = "import sys,json;p=json.loads(sys.stdin.readline());print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':{'kind':'bye'}}}),flush=True);print('{}',flush=True)";
    let mut w = NativeWorker::spawn(&recipe(extra), &t.0, limits())
        .await
        .unwrap();
    w.request(
        WorkerRequest::Shutdown,
        &NativeCancellation::default(),
        &mut no_mock,
    )
    .await
    .unwrap();
    assert_eq!(
        w.finish().await.unwrap_err().kind,
        NativeFailureKind::Protocol
    );
}

#[tokio::test]
async fn spec_derived_cinema_properties_use_real_bun_and_central_rust_assessment() {
    use dctx0_target_typescript::BunHarness;
    use dctx0_verification::*;
    let temp = Temp::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
        file: file.into(),
        text: fs::read_to_string(root.join("dctx0").join(file)).unwrap(),
    });
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let subject = "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner";
    let projected = project_program(
        &ir,
        "test-pinned-snapshot",
        1,
        VerificationScope {
            subjects: std::collections::BTreeSet::from([subject.into()]),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let property = &projected.properties[0];
    let bundle = BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./adapter.ts".into(),
            subjects: std::collections::BTreeSet::from([subject.into()]),
        })
        .unwrap();
    for artifact in &bundle.artifacts {
        fs::write(temp.0.join(&artifact.path), &artifact.contents).unwrap();
    }
    fs::create_dir_all(temp.0.join("src/domain")).unwrap();
    for file in ["film-id.ts", "candidate-order-winner.ts", "voting-rules.ts"] {
        fs::copy(
            root.join("src/domain").join(file),
            temp.0.join("src/domain").join(file),
        )
        .unwrap();
    }
    let adapter = r#"import {candidateOrderWinner} from './src/domain/candidate-order-winner.ts';
 export const adapter={async describe(){return {protocol_version:1,subjects:[],artifacts:[],capabilities:[],representations:{}};},async begin(){return {async invoke(call){const [votes,candidates]=call.arguments.map(v=>v.value.map(x=>x.value));const result=candidateOrderWinner(votes,candidates);return {kind:'returned',value:{kind:'option',value:result?{kind:'string',value:result}:null}};},async close(){}};}};"#;
    fs::write(temp.0.join("adapter.ts"), adapter).unwrap();
    let plan = VerificationPlan {
        schema_version: 1,
        snapshot: projected.snapshot,
        check_occurrence: 1,
        scope: projected.scope,
        budgets: projected.budgets,
        obligations: vec![],
    };
    let cancel = NativeCancellation::default();
    let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
        .await
        .unwrap();
    let started = Instant::now();
    let mut count = 0;
    let mut invocations = 0;
    for assertion in &property.assertions {
        assert!(assertion.issues.is_empty());
        for case in &assertion.cases {
            let result = worker
                .execute_pure_case(&ir, &plan, case, &cancel)
                .await
                .unwrap();
            assert_eq!(
                result.assessment.result,
                VerificationResult::Pass,
                "{}",
                case.id
            );
            count += 1;
            invocations += result.evidence.observations.len();
        }
    }
    assert!(count > 1);
    let status = fs::read_to_string(format!("/proc/{}/status", worker.pid())).unwrap();
    let peak = status
        .lines()
        .find(|s| s.starts_with("VmHWM:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<u64>()
        .unwrap()
        * 1024;
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-executor-conformance");
    fs::create_dir_all(&evidence).unwrap();
    fs::write(evidence.join(format!("{}-cinema.json",std::process::id())),serde_json::to_vec(&serde_json::json!({"scope":"complete modeled Cinema winner invariant; not complete application verification","cases":count,"invocations":invocations,"elapsed_ms":started.elapsed().as_secs_f64()*1000.,"peak_rss_bytes":peak})).unwrap()).unwrap();
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
    // Change only the isolated application copy: the spec and its plan stay fixed.
    fs::write(
        temp.0.join("src/domain/candidate-order-winner.ts"),
        "export const candidateOrderWinner=()=>undefined;",
    )
    .unwrap();
    let mut mutant = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
        .await
        .unwrap();
    let result = mutant
        .execute_pure_case(&ir, &plan, &property.assertions[1].cases[0], &cancel)
        .await
        .unwrap();
    assert!(matches!(
        result.assessment.result,
        VerificationResult::Fail { .. }
    ));
    mutant
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    mutant.finish().await.unwrap();
}

#[tokio::test]
async fn all_cinema_examples_invoke_real_services_with_central_mocks_and_observed_events() {
    use dctx0_target_typescript::BunHarness;
    use dctx0_verification::*;
    let temp = Temp::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
        file: file.into(),
        text: fs::read_to_string(root.join("dctx0").join(file)).unwrap(),
    });
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let fixtures = serde_json::from_slice(
        &fs::read(root.join(".dctx0/testing/typescript/examples.json")).unwrap(),
    )
    .unwrap();
    let plan = project_program(
        &ir,
        "test-pinned-snapshot",
        1,
        VerificationScope {
            subjects: std::collections::BTreeSet::from(["HomeCinema.MovieNight".into()]),
            tasks: Default::default(),
        },
        &fixtures,
        Default::default(),
    );
    assert_eq!(plan.examples.len(), 12);
    assert!(
        plan.examples
            .iter()
            .all(|example| example.issues.is_empty())
    );
    let bundle = BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./adapter.ts".into(),
            subjects: plan.closure.clone(),
        })
        .unwrap();
    for artifact in &bundle.artifacts {
        fs::write(temp.0.join(&artifact.path), &artifact.contents).unwrap();
    }
    for directory in ["application", "domain"] {
        fs::create_dir_all(temp.0.join("src").join(directory)).unwrap();
    }
    for file in [
        "application/movie-night.ts",
        "domain/film-id.ts",
        "domain/release-year.ts",
        "domain/contracts.ts",
        "domain/timestamp.ts",
        "domain/voting-rules.ts",
        "domain/candidate-order-winner.ts",
    ] {
        fs::copy(root.join("src").join(file), temp.0.join("src").join(file)).unwrap();
    }
    let adapter = fs::read_to_string(root.join(".dctx0/testing/typescript/adapter.ts"))
        .unwrap()
        .replace("../../../src/", "./src/");
    fs::write(temp.0.join("adapter.ts"), adapter).unwrap();
    fs::copy(
        root.join(".dctx0/testing/typescript/resource-observer.ts"),
        temp.0.join("resource-observer.ts"),
    )
    .unwrap();
    fs::write(
        temp.0.join("http-invocation.ts"),
        fs::read_to_string(root.join(".dctx0/testing/typescript/http-invocation.ts"))
            .unwrap()
            .replace("../../../src/", "./src/"),
    )
    .unwrap();
    fs::write(
        temp.0.join("model.json"),
        serde_json::to_vec(&serde_json::json!({"data":ir.data,"operations":ir.operations}))
            .unwrap(),
    )
    .unwrap();
    let cancel = NativeCancellation::default();
    let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
        .await
        .unwrap();
    let started = Instant::now();
    let mut results = Vec::new();
    for example in &plan.examples {
        let result = worker
            .execute_example(&ir, example, &plan.budgets, &cancel)
            .await
            .unwrap();
        assert_eq!(
            result.result,
            VerificationResult::Pass,
            "{}: {:?}",
            example.subject,
            result
        );
        assert_eq!(result.invocations.len(), example.invocations.len());
        assert!(result.unused_mocks.is_empty());
        let requested = worker
            .session_evidence()
            .iter()
            .filter(|entry| matches!(entry, NativeSessionEvidence::MockRequested { .. }))
            .count();
        let replied = worker
            .session_evidence()
            .iter()
            .filter(|entry| matches!(entry, NativeSessionEvidence::MockReplied { .. }))
            .count();
        assert_eq!(requested, result.collaborators.len());
        assert_eq!(replied, requested);
        results.push(result);
    }
    let elapsed = started.elapsed().as_secs_f64() * 1000.;
    let metrics = worker
        .request(WorkerRequest::Metrics, &cancel, &mut no_mock)
        .await
        .unwrap();
    let WorkerReply::Measured { peak_rss_bytes, .. } = metrics else {
        panic!("Missing metrics")
    };
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-executor-conformance");
    fs::create_dir_all(&evidence).unwrap();
    fs::write(evidence.join(format!("{}-cinema-examples.json",std::process::id())),serde_json::to_vec(&serde_json::json!({
        "scope":"all 12 Cinema examples against real surface services with centrally evaluated port mocks; not production persistence or lifecycle proof",
        "elapsed_ms":elapsed,"peak_rss_bytes":peak_rss_bytes,"examples":results,
    })).unwrap()).unwrap();
    let production = temp.0.join("src/application/movie-night.ts");
    let original = fs::read_to_string(&production).unwrap();
    let example = plan
        .examples
        .iter()
        .find(|example| example.subject.ends_with("save a film in the collection"))
        .unwrap();
    for mutated in [
        original.replace("filmId:request.id", "filmId:'wrong-id'"),
        original.replace(
            "this.publish({name:'FilmSaved',payload:{filmId:request.id}});",
            "/* lost event */",
        ),
        original.replace(
            "this.ports.catalog.save(request)",
            "this.ports.catalog.find(request.id)",
        ),
    ] {
        assert_ne!(original, mutated);
        fs::write(&production, mutated).unwrap();
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let result = worker
            .execute_example(&ir, example, &plan.budgets, &cancel)
            .await;
        assert!(
            matches!(
                result,
                Ok(ScenarioEvidence {
                    result: VerificationResult::Fail { .. },
                    ..
                }) | Err(NativeFailure {
                    kind: NativeFailureKind::Technical,
                    ..
                })
            ),
            "{result:?}"
        );
        worker.abort().await.unwrap();
    }
    // The actual reference app and spec remain untouched; only isolated copies mutate.
    assert_eq!(
        fs::read_to_string(root.join("src/application/movie-night.ts")).unwrap(),
        original
    );
}

#[tokio::test]
async fn cinema_existing_validators_and_boundaries_are_real_and_mutations_are_detected() {
    use dctx0_verification::*;
    let temp = Temp::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
        file: file.into(),
        text: fs::read_to_string(root.join("dctx0").join(file)).unwrap(),
    });
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let names = [
        "MemberId",
        "CommentId",
        "PollId",
        "StarRating",
        "FilmAvailability",
        "RuntimeMinutes",
        "Genre",
        "PageOffset",
        "PageLimit",
        "FilmSort",
        "Film",
        "SaveFilm",
        "FilmSearch",
        "ChangeAvailability",
        "RateFilm",
        "Rating",
        "OpenVoting",
        "CastVote",
        "AddComment",
        "CommentText",
        "Comment",
        "CommentAdded",
        "CommentNotFound",
        "CommentOverview",
        "CommentView",
        "FilmAvailabilityChanged",
        "FilmDetails",
        "FilmNotFound",
        "FilmOverview",
        "FilmRated",
        "FilmSaved",
        "FilmSummary",
        "FilmVote",
        "FilmVoteResult",
        "PersistenceUnavailable",
        "PollNotFound",
        "PollStatus",
        "RatingSummary",
        "ScreeningPoll",
        "VoteCast",
        "VotingClosed",
        "VotingCompleted",
        "VotingOpened",
        "VotingOverview",
    ];
    let plan = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: names
                .iter()
                .map(|name| format!("HomeCinema.MovieNight.{name}"))
                .collect(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let bundle = dctx0_target_typescript::BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./adapter.ts".into(),
            subjects: plan.closure.clone(),
        })
        .unwrap();
    for artifact in &bundle.artifacts {
        fs::write(temp.0.join(&artifact.path), &artifact.contents).unwrap();
    }
    for file in [
        "application/movie-night.ts",
        "persistence/sqlite.ts",
        "domain/film-id.ts",
        "domain/release-year.ts",
        "domain/contracts.ts",
        "domain/timestamp.ts",
        "domain/candidate-order-winner.ts",
        "domain/voting-rules.ts",
    ] {
        let destination = temp.0.join("src").join(file);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(root.join("src").join(file), destination).unwrap();
    }
    let adapter = fs::read_to_string(root.join(".dctx0/testing/typescript/adapter.ts"))
        .unwrap()
        .replace("../../../src/", "./src/");
    fs::write(temp.0.join("adapter.ts"), adapter).unwrap();
    fs::copy(
        root.join(".dctx0/testing/typescript/resource-observer.ts"),
        temp.0.join("resource-observer.ts"),
    )
    .unwrap();
    fs::write(
        temp.0.join("http-invocation.ts"),
        fs::read_to_string(root.join(".dctx0/testing/typescript/http-invocation.ts"))
            .unwrap()
            .replace("../../../src/", "./src/"),
    )
    .unwrap();
    fs::write(
        temp.0.join("model.json"),
        serde_json::to_vec(&serde_json::json!({"data":ir.data,"operations":ir.operations}))
            .unwrap(),
    )
    .unwrap();
    let cancel = NativeCancellation::default();
    for mutated in [false, true] {
        if mutated {
            fs::write(temp.0.join("src/domain/film-id.ts"), "export type FilmId=string; export function filmId(input:unknown):FilmId {if(typeof input!=='string')throw new TypeError('Expected text');return input;}").unwrap();
            let path = temp.0.join("src/persistence/sqlite.ts");
            let source = fs::read_to_string(&path).unwrap();
            let changed = source.replace("private star(value:Stars){if(!stars.includes(value))throw new TypeError('Unknown star rating');}", "private star(value:Stars){void value;}").replace("private availability(value:string){if(!['Available','Watched','Wishlist','Unavailable'].includes(value))throw new TypeError('Unknown availability');}", "private availability(value:string){void value;}");
            let changed = changed.replace("request.runtimeMinutes<1", "request.runtimeMinutes<0").replace("request.runtimeMinutes>600", "request.runtimeMinutes>601")
                .replace("filter.offset<0", "filter.offset<-1").replace("filter.offset>100000", "filter.offset>100001")
                .replace("filter.limit<1", "filter.limit<0").replace("filter.limit>100", "filter.limit>101")
                .replace("request.genres.some(genre=>!['Action','Adventure','Animation','Comedy','Crime','Documentary','Drama','Fantasy','Horror','Mystery','Romance','ScienceFiction','Thriller','Western'].includes(genre))", "request.genres.some(genre=>typeof genre!=='string')")
                .replace("if(!['TitleAscending','NewestFirst','HighestRated','MostVoted'].includes(filter.sort))throw new TypeError('Unknown film sort');", "if(typeof filter.sort!=='string')throw new TypeError('Expected text');");
            assert_ne!(source, changed);
            fs::write(path, changed).unwrap();
            // Disable the newly added independent output guard as well:
            // otherwise a second production validator catches this mutation.
            let path = temp.0.join("src/domain/contracts.ts");
            let source = fs::read_to_string(&path).unwrap();
            let changed = source.replace("validate(value);return value;", "return value;");
            assert_ne!(source, changed);
            fs::write(path, changed).unwrap();
        }
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let mut violated = std::collections::BTreeSet::new();
        let mut known_record_failures = 0;
        for contract in &plan.contracts {
            for (negative, corpus) in [
                (false, &contract.corpus),
                (true, &contract.rejection_corpus),
            ] {
                for (index, value) in corpus.values.iter().enumerate() {
                    let reply = worker
                        .execute_value_case(
                            &format!("{}:{negative}:{index}", contract.subject),
                            &contract.subject,
                            value,
                            &cancel,
                        )
                        .await;
                    if !mutated {
                        if negative
                            && matches!(reply, Ok(ValueCheck::Accepted { .. }))
                            && [
                                "HomeCinema.MovieNight.Film",
                                "HomeCinema.MovieNight.SaveFilm",
                                "HomeCinema.MovieNight.FilmSearch",
                                "HomeCinema.MovieNight.ChangeAvailability",
                                "HomeCinema.MovieNight.RateFilm",
                                "HomeCinema.MovieNight.Rating",
                                "HomeCinema.MovieNight.OpenVoting",
                                "HomeCinema.MovieNight.CastVote",
                            ]
                            .contains(&contract.subject.as_str())
                        {
                            let VerificationValue::Record(fields) = value else {
                                panic!("Known record violation must preserve its record input");
                            };
                            assert!(
                                fields.contains_key("__unexpected_field__")
                                    || (contract.subject == "HomeCinema.MovieNight.FilmSearch"
                                        && !fields.contains_key("availability")),
                                "Unexpected production record violation: {value:?}"
                            );
                            known_record_failures += 1;
                        } else {
                            assert!(
                                matches!(
                                    (&reply, negative),
                                    (Ok(ValueCheck::Accepted { .. }), false)
                                        | (Ok(ValueCheck::Rejected { .. }), true)
                                ),
                                "{}: {reply:?}",
                                contract.subject
                            );
                        }
                    } else if negative && matches!(reply, Ok(ValueCheck::Accepted { .. })) {
                        violated.insert(contract.subject.clone());
                    }
                }
            }
        }
        worker.finish().await.unwrap();
        if !mutated {
            assert_eq!(
                known_record_failures, 0,
                "Actual boundaries now reject missing/extra-field violations"
            );
        }
        if mutated {
            assert_eq!(
                violated,
                names
                    .iter()
                    .map(|name| format!("HomeCinema.MovieNight.{name}"))
                    .chain(std::iter::once("HomeCinema.MovieNight.FilmId".into()))
                    .collect()
            );
        }
    }
}

#[tokio::test]
async fn cinema_lifecycle_paths_use_actual_sqlite_state_and_real_scoped_services() {
    use dctx0_target_typescript::BunHarness;
    use dctx0_verification::*;
    let temp = Temp::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
        file: file.into(),
        text: fs::read_to_string(root.join("dctx0").join(file)).unwrap(),
    });
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let fixtures: ProjectionFixtures = serde_json::from_slice(
        &fs::read(root.join(".dctx0/testing/typescript/examples.json")).unwrap(),
    )
    .unwrap();
    let plan = project_program(
        &ir,
        "test-pinned",
        1,
        VerificationScope {
            subjects: std::collections::BTreeSet::from(["HomeCinema.MovieNight".into()]),
            tasks: Default::default(),
        },
        &fixtures,
        serde_json::from_value::<VerificationBudgets>(
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(root.join(".dctx0/testing/verification.json")).unwrap(),
            )
            .unwrap()["budgets"]
                .clone(),
        )
        .unwrap(),
    );
    let bundle = BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./adapter.ts".into(),
            subjects: plan.closure.clone(),
        })
        .unwrap();
    for artifact in &bundle.artifacts {
        fs::write(temp.0.join(&artifact.path), &artifact.contents).unwrap();
    }
    for file in [
        "application/movie-night.ts",
        "persistence/sqlite.ts",
        "app/session.ts",
        "http/app.ts",
        "domain/film-id.ts",
        "domain/release-year.ts",
        "domain/contracts.ts",
        "domain/timestamp.ts",
        "domain/candidate-order-winner.ts",
        "domain/voting-rules.ts",
    ] {
        let target = temp.0.join("src").join(file);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(root.join("src").join(file), target).unwrap();
    }
    std::os::unix::fs::symlink(root.join("node_modules"), temp.0.join("node_modules")).unwrap();
    fs::write(
        temp.0.join("adapter.ts"),
        fs::read_to_string(root.join(".dctx0/testing/typescript/adapter.ts"))
            .unwrap()
            .replace("../../../src/", "./src/"),
    )
    .unwrap();
    fs::copy(
        root.join(".dctx0/testing/typescript/resource-observer.ts"),
        temp.0.join("resource-observer.ts"),
    )
    .unwrap();
    fs::write(
        temp.0.join("http-invocation.ts"),
        fs::read_to_string(root.join(".dctx0/testing/typescript/http-invocation.ts"))
            .unwrap()
            .replace("../../../src/", "./src/"),
    )
    .unwrap();
    fs::write(
        temp.0.join("model.json"),
        serde_json::to_vec(&serde_json::json!({"data":ir.data,"operations":ir.operations}))
            .unwrap(),
    )
    .unwrap();
    fs::write(temp.0.join("config.json"),r#"{"mode":"real","database":":memory:","time":"2026-10-10T18:00:00Z","ids":["00000000-0000-4000-8000-000000000030"]}"#).unwrap();
    let save=fixtures.examples["HomeCinema.MovieNight.Catalog.save a film in the collection"]["saveFilm"].clone();
    let mut second = save.clone();
    if let VerificationValue::Record(fields) = &mut second {
        fields.insert(
            "id".into(),
            VerificationValue::String("00000000-0000-4000-8000-000000000002".into()),
        );
    }
    let open=fixtures.examples["HomeCinema.MovieNight.ScreeningPlanning.open voting for the next appointment"]["openVoting"].clone();
    let cast=fixtures.examples["HomeCinema.MovieNight.ScreeningPlanning.cast or replace a film vote"]["castVote"].clone();
    let poll = if let VerificationValue::Record(fields) = &open {
        fields["pollId"].clone()
    } else {
        panic!("Expected request")
    };
    let setup = vec![
        Invocation {
            subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
            arguments: vec![save],
        },
        Invocation {
            subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
            arguments: vec![second],
        },
        Invocation {
            subject: "HomeCinema.MovieNight.ScreeningPlanning.ScreeningPlanningApi.openVoting"
                .into(),
            arguments: vec![open],
        },
    ];
    let lifecycle = &plan.lifecycles[0];
    assert_eq!(lifecycle.paths.len(), 2);
    let cancel = NativeCancellation::default();
    let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
        .await
        .unwrap();
    let started = Instant::now();
    let mut results = Vec::new();
    for path in &lifecycle.paths {
        let input = LifecycleCaseInput {
            keys: vec![poll.clone()],
            setup: setup.clone(),
            arguments: path
                .transitions
                .iter()
                .map(|index| {
                    if lifecycle.transitions[*index]
                        .operation
                        .ends_with("castVote")
                    {
                        vec![cast.clone()]
                    } else {
                        vec![poll.clone()]
                    }
                })
                .collect(),
        };
        let result = worker
            .execute_lifecycle(&ir, lifecycle, path, &input, &plan.budgets, &cancel)
            .await
            .unwrap();
        assert_eq!(
            result.assessment.result,
            VerificationResult::Pass,
            "{result:?}"
        );
        assert!(result.assessment.open_transitions.is_empty());
        assert_eq!(result.setup.len(), 3);
        assert!(result.observations.iter().all(|obs| {
            obs.context
                .calls
                .as_ref()
                .is_some_and(|calls| calls.len() == 1)
        }));
        results.push(result);
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
    let WorkerReply::Measured { peak_rss_bytes, .. } = worker
        .request(WorkerRequest::Metrics, &cancel, &mut no_mock)
        .await
        .unwrap()
    else {
        panic!("Expected metrics")
    };
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
    let output =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-executor-conformance");
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join(format!("{}-cinema-lifecycles.json",std::process::id())),serde_json::to_vec(&serde_json::json!({"scope":"both modeled Cinema lifecycle paths with actual SQLite state and real ports; no full task-adoption proof","elapsed_ms":elapsed_ms,"peak_rss_bytes":peak_rss_bytes,"results":results})).unwrap()).unwrap();
    let production = temp.0.join("src/persistence/sqlite.ts");
    let original = fs::read_to_string(&production).unwrap();
    fs::write(
        &production,
        original.replace(
            "const closed:Poll={...poll,status:'Closed'}",
            "const closed:Poll={...poll,status:'Open'}",
        ),
    )
    .unwrap();
    let path = &lifecycle.paths[1];
    let input = LifecycleCaseInput {
        keys: vec![poll.clone()],
        setup,
        arguments: vec![vec![poll]],
    };
    let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
        .await
        .unwrap();
    let result = worker
        .execute_lifecycle(&ir, lifecycle, path, &input, &plan.budgets, &cancel)
        .await
        .unwrap();
    assert!(
        matches!(result.assessment.result,VerificationResult::Fail {issue} if issue.code=="LIFECYCLE_STATE_MISMATCH")
    );
    worker.abort().await.unwrap();
    // The overview expectation comes from two actual persisted comments,
    // independently filtered and projected by the Rust evaluator.
    let save = fixtures.examples["HomeCinema.MovieNight.Catalog.save a film in the collection"]["saveFilm"].clone();
    let comment =
        fixtures.examples["HomeCinema.MovieNight.Community.add a comment"]["addComment"].clone();
    let VerificationValue::Record(comment_fields) = &comment else {
        panic!("Expected comment request");
    };
    let film = comment_fields["filmId"].clone();
    let other_id = VerificationValue::String("00000000-0000-4000-8000-000000000002".into());
    let mut other_film = save.clone();
    let mut other_comment = comment.clone();
    if let VerificationValue::Record(fields) = &mut other_film {
        fields.insert("id".into(), other_id.clone());
    }
    if let VerificationValue::Record(fields) = &mut other_comment {
        fields.insert("filmId".into(), other_id);
    }
    fs::write(temp.0.join("config.json"),r#"{"mode":"real","database":":memory:","time":"2026-10-10T18:00:00Z","ids":["00000000-0000-4000-8000-000000000030","00000000-0000-4000-8000-000000000031"]}"#).unwrap();
    let input = OperationCaseInput {
        id: "cinema-comment-projection".into(),
        invocation: Invocation {
            subject: "HomeCinema.MovieNight.Community.CommentStore.listByFilm".into(),
            arguments: vec![film],
        },
        setup: vec![
            Invocation {
                subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                arguments: vec![save],
            },
            Invocation {
                subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                arguments: vec![other_film],
            },
            Invocation {
                subject: "HomeCinema.MovieNight.Community.FilmCommunityApi.addComment".into(),
                arguments: vec![comment],
            },
            Invocation {
                subject: "HomeCinema.MovieNight.Community.FilmCommunityApi.addComment".into(),
                arguments: vec![other_comment],
            },
        ],
        mocks: vec![],
    };
    let operation = plan
        .operations
        .iter()
        .find(|operation| operation.operation.id == input.invocation.subject)
        .unwrap();
    let mut projection_results = Vec::new();
    for replacement in [
        None,
        Some("total:items.length+1"),
        Some("total:items.length"),
    ] {
        let changed = match replacement {
            None => original.clone(),
            Some("total:items.length+1") => original.replace(
                "return {items,total:items.length};",
                "return {items,total:items.length+1};",
            ),
            Some(_) => {
                original.replace(".filter(comment=>comment.filmId===id)", ".filter(()=>true)")
            }
        };
        if replacement.is_some() {
            assert_ne!(changed, original);
        }
        fs::write(&production, changed).unwrap();
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                operation,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if replacement.is_none() {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{assessed:?}"
            );
        } else {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results.push(serde_json::json!({"mutation":replacement,"evidence":assessed}));
    }
    let presence = plan
        .operations
        .iter()
        .find(|operation| {
            operation.operation.id == "HomeCinema.MovieNight.Community.FilmPresence.exists"
        })
        .unwrap();
    let film = fixtures.examples["HomeCinema.MovieNight.Community.add a comment"]["filmId"].clone();
    for (exists, mutated) in [(true, false), (false, false), (true, true), (false, true)] {
        let source = if mutated {
            if exists {
                original.replace("this.getFilm(id)?.id", "undefined")
            } else {
                original.replace("this.getFilm(id)?.id", "id")
            }
        } else {
            original.clone()
        };
        if mutated {
            assert_ne!(source, original);
        }
        fs::write(&production, source).unwrap();
        let input = OperationCaseInput {
            id: format!("cinema-presence-{exists}-{mutated}"),
            invocation: Invocation {
                subject: presence.operation.id.clone(),
                arguments: vec![film.clone()],
            },
            setup: if exists {
                vec![Invocation {subject:"HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),arguments:vec![fixtures.examples["HomeCinema.MovieNight.Catalog.save a film in the collection"]["saveFilm"].clone()]}]
            } else {
                vec![]
            },
            mocks: vec![],
        };
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                presence,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if mutated {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{assessed:?}"
            );
        } else {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results.push(serde_json::json!({"subject":"FilmPresence.exists","exists":exists,"mutated":mutated,"evidence":assessed}));
    }
    let rating_plan = plan
        .operations
        .iter()
        .find(|operation| {
            operation.operation.id == "HomeCinema.MovieNight.Community.RatingStore.summary"
        })
        .unwrap();
    let film_id =
        fixtures.examples["HomeCinema.MovieNight.Community.add a comment"]["filmId"].clone();
    let member_id =
        fixtures.examples["HomeCinema.MovieNight.Community.add a comment"]["memberId"].clone();
    let save=fixtures.examples["HomeCinema.MovieNight.Catalog.save a film in the collection"]["saveFilm"].clone();
    let mut other_film = save.clone();
    let other_id = VerificationValue::String("00000000-0000-4000-8000-000000000002".into());
    if let VerificationValue::Record(fields) = &mut other_film {
        fields.insert("id".into(), other_id.clone());
    }
    let rated = |film: VerificationValue, member: VerificationValue, stars: &str| Invocation {
        subject: "HomeCinema.MovieNight.Community.RatingStore.upsert".into(),
        arguments: vec![VerificationValue::Record(std::collections::BTreeMap::from(
            [
                ("filmId".into(), film),
                ("memberId".into(), member),
                (
                    "stars".into(),
                    VerificationValue::Variant {
                        declaration: "HomeCinema.MovieNight.StarRating".into(),
                        name: stars.into(),
                        payload: None,
                    },
                ),
            ],
        ))],
    };
    for mutation in ["empty", "none", "average", "member", "filter"] {
        let source=match mutation {
            "average"=>original.replace("ratings.reduce((sum,rating)=>sum+stars.indexOf(rating.stars)+1,0)/ratings.length","ratings.reduce((sum,rating)=>sum+stars.indexOf(rating.stars)+1,0)/ratings.length+1"),
            "member"=>original.replace("ratings.find(rating=>rating.memberId===memberId)?.stars","undefined"),
            "filter"=>original.replace(".filter(rating=>rating.filmId===filmIdValue)",".filter(()=>true)"),
            _=>original.clone(),
        };
        let mutated = !matches!(mutation, "empty" | "none");
        if mutated {
            assert_ne!(source, original);
        }
        fs::write(&production, source).unwrap();
        let setup = if mutation == "empty" {
            vec![]
        } else {
            vec![
                Invocation {
                    subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                    arguments: vec![save.clone()],
                },
                Invocation {
                    subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                    arguments: vec![other_film.clone()],
                },
                rated(film_id.clone(), member_id.clone(), "FourStars"),
                rated(
                    film_id.clone(),
                    VerificationValue::String("00000000-0000-4000-8000-000000000011".into()),
                    "FiveStars",
                ),
                rated(other_id.clone(), member_id.clone(), "OneStar"),
            ]
        };
        let input = OperationCaseInput {
            id: format!("cinema-rating-{mutation}"),
            invocation: Invocation {
                subject: rating_plan.operation.id.clone(),
                arguments: vec![film_id.clone(), member_id.clone()],
            },
            setup,
            mocks: vec![],
        };
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                rating_plan,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if mutated {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{assessed:?}"
            );
        } else {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results.push(serde_json::json!({"subject":"RatingStore.summary","mutation":mutation,"evidence":assessed}));
    }
    let upsert_plan = plan
        .operations
        .iter()
        .find(|operation| {
            operation.operation.id == "HomeCinema.MovieNight.Community.RatingStore.upsert"
        })
        .unwrap();
    for mutation in ["none", "count", "write"] {
        let source = match mutation {
            "count" => original.replace(
                "ratingCount:ratings.length,memberRating:",
                "ratingCount:ratings.length+1,memberRating:",
            ),
            "write" => original.replace(
                "ON CONFLICT(film_id,member_id) DO UPDATE SET document=excluded.document",
                "ON CONFLICT(film_id,member_id) DO NOTHING",
            ),
            _ => original.clone(),
        };
        if mutation != "none" {
            assert_ne!(source, original);
        }
        fs::write(&production, source).unwrap();
        let input = OperationCaseInput {
            id: format!("cinema-upsert-{mutation}"),
            invocation: rated(film_id.clone(), member_id.clone(), "FiveStars"),
            setup: vec![
                Invocation {
                    subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                    arguments: vec![save.clone()],
                },
                Invocation {
                    subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                    arguments: vec![other_film.clone()],
                },
                rated(film_id.clone(), member_id.clone(), "FourStars"),
                rated(
                    film_id.clone(),
                    VerificationValue::String("00000000-0000-4000-8000-000000000011".into()),
                    "FiveStars",
                ),
                rated(other_id.clone(), member_id.clone(), "OneStar"),
            ],
            mocks: vec![],
        };
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                upsert_plan,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if mutation == "none" {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{assessed:?}"
            );
        } else {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results.push(serde_json::json!({"subject":"RatingStore.upsert","mutation":mutation,"evidence":assessed}));
    }
    let append_plan = plan
        .operations
        .iter()
        .find(|operation| {
            operation.operation.id == "HomeCinema.MovieNight.Community.CommentStore.append"
        })
        .unwrap();
    let comment =
        fixtures.examples["HomeCinema.MovieNight.Community.add a comment"]["addComment"].clone();
    let mut other_comment = comment.clone();
    if let VerificationValue::Record(fields) = &mut other_comment {
        fields.insert("filmId".into(), other_id.clone());
    }
    for mutation in ["none", "content", "returned"] {
        let source = match mutation {
            "content" => original.replace(
                "...request,createdAt:",
                "...request,text:'Incorrect',createdAt:",
            ),
            "returned" => original.replace(
                "JSON.stringify(comment));return comment;",
                "JSON.stringify(comment));return {...comment,text:'Incorrect'};",
            ),
            _ => original.clone(),
        };
        if mutation != "none" {
            assert_ne!(source, original);
        }
        fs::write(&production, source).unwrap();
        let input = OperationCaseInput {
            id: format!("cinema-append-{mutation}"),
            invocation: Invocation {
                subject: append_plan.operation.id.clone(),
                arguments: vec![comment.clone()],
            },
            setup: vec![
                Invocation {
                    subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                    arguments: vec![save.clone()],
                },
                Invocation {
                    subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
                    arguments: vec![other_film.clone()],
                },
                Invocation {
                    subject: "HomeCinema.MovieNight.Community.CommentStore.append".into(),
                    arguments: vec![other_comment.clone()],
                },
            ],
            mocks: vec![],
        };
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                append_plan,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if mutation == "none" {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{assessed:?}"
            );
        } else {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results.push(serde_json::json!({"subject":"CommentStore.append","mutation":mutation,"evidence":assessed}));
    }
    let open=fixtures.examples["HomeCinema.MovieNight.ScreeningPlanning.open voting for the next appointment"]["openVoting"].clone();
    let cast=fixtures.examples["HomeCinema.MovieNight.ScreeningPlanning.cast or replace a film vote"]["castVote"].clone();
    let catalog_setup = vec![
        Invocation {
            subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
            arguments: vec![save.clone()],
        },
        Invocation {
            subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
            arguments: vec![other_film.clone()],
        },
        rated(film_id.clone(), member_id.clone(), "FourStars"),
        rated(
            film_id.clone(),
            VerificationValue::String("00000000-0000-4000-8000-000000000011".into()),
            "FiveStars",
        ),
        rated(other_id.clone(), member_id.clone(), "OneStar"),
        Invocation {
            subject: "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.open".into(),
            arguments: vec![open],
        },
        Invocation {
            subject: "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.cast".into(),
            arguments: vec![cast],
        },
    ];
    let mut edited = save.clone();
    if let VerificationValue::Record(fields) = &mut edited {
        fields.insert(
            "title".into(),
            VerificationValue::String("Edited film".into()),
        );
    }
    let availability = VerificationValue::Record(std::collections::BTreeMap::from([
        ("filmId".into(), film_id.clone()),
        (
            "availability".into(),
            VerificationValue::Variant {
                declaration: "HomeCinema.MovieNight.FilmAvailability".into(),
                name: "Watched".into(),
                payload: None,
            },
        ),
    ]));
    for (method, mutation, argument) in [
        ("find", "none", film_id.clone()),
        (
            "find",
            "missing",
            VerificationValue::String("00000000-0000-4000-8000-000000000099".into()),
        ),
        ("save", "none", edited.clone()),
        ("changeAvailability", "none", availability.clone()),
        ("find", "average", film_id.clone()),
        ("find", "votes", other_id.clone()),
        ("save", "content", edited),
        ("changeAvailability", "availability", availability),
    ] {
        let source=match mutation {
            "average"=>original.replace("ratings.reduce((sum,rating)=>sum+stars.indexOf(rating.stars)+1,0)/ratings.length","ratings.reduce((sum,rating)=>sum+stars.indexOf(rating.stars)+1,0)/ratings.length+1"),
            "votes"=>original.replace("vote.pollId===current.id&&vote.filmId===film.id","vote.pollId===current.id"),
            "content"=>original.replace("const film={...request,genres:[...request.genres]}","const film={...request,title:'Incorrect',genres:[...request.genres]}"),
            "availability"=>original.replace("const changed={...film,availability:request.availability}","const changed={...film,availability:'Available' as const}"),
            _=>original.clone(),
        };
        let mutated = !matches!(mutation, "none" | "missing");
        if mutated {
            assert_ne!(source, original);
        }
        fs::write(&production, source).unwrap();
        let subject = format!("HomeCinema.MovieNight.Catalog.FilmCatalogStore.{method}");
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.operation.id == subject)
            .unwrap();
        let input = OperationCaseInput {
            id: format!("cinema-catalog-{method}-{mutation}"),
            invocation: Invocation {
                subject,
                arguments: vec![argument],
            },
            setup: catalog_setup.clone(),
            mocks: vec![],
        };
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                operation,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if mutated {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{assessed:?}"
            );
        } else {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results
            .push(serde_json::json!({"subject":method,"mutation":mutation,"evidence":assessed}));
    }
    // Poll DTOs and persisted updates are checked against independent state projections.
    let voting_request = fixtures.examples["HomeCinema.MovieNight.ScreeningPlanning.cast or replace a film vote"]["castVote"].clone();
    let poll_id = match &voting_request {
        VerificationValue::Record(fields) => fields["pollId"].clone(),
        _ => panic!("Expected cast request"),
    };
    let mut replacement_vote = voting_request.clone();
    if let VerificationValue::Record(fields) = &mut replacement_vote {
        fields.insert("filmId".into(), other_id.clone());
    }
    for (method, mutation) in [
        ("cast", "none"),
        ("close", "none"),
        ("open", "none"),
        ("current", "none"),
        ("current", "closed"),
        ("current", "empty"),
        ("open", "retained_votes"),
        ("open", "schedule"),
        ("current", "vote_count"),
        ("cast", "vote_count"),
        ("cast", "ignored_update"),
        ("close", "status"),
        ("close", "winner"),
    ] {
        let source = match mutation {
            "retained_votes" => original.replace(
                "this.db.query('DELETE FROM votes WHERE poll_id=?').run(poll.id);",
                "",
            ),
            "schedule" => original.replace(
                "scheduledFor:request.scheduledFor,candidateFilmIds",
                "scheduledFor:'2030-01-01T00:00:00Z',candidateFilmIds",
            ),
            "vote_count" => original.replace(
                "votes:votes.filter(vote=>vote===filmId).length",
                "votes:votes.filter(vote=>vote===filmId).length+1",
            ),
            "ignored_update" => original.replace(
                "DO UPDATE SET film_id=excluded.film_id,document=excluded.document",
                "DO UPDATE SET film_id=film_id,document=document",
            ),
            "status" => original.replace(
                "const closed:Poll={...poll,status:'Closed'}",
                "const closed:Poll={...poll,status:'Open'}",
            ),
            "winner" => original.replace(
                "winningFilmId:candidateOrderWinner(votes,poll.candidateFilmIds)",
                "winningFilmId:undefined",
            ),
            _ => original.clone(),
        };
        let mutated = !matches!(mutation, "none" | "closed" | "empty");
        if mutated {
            assert_ne!(source, original);
        }
        fs::write(&production, source).unwrap();
        let subject = format!("HomeCinema.MovieNight.ScreeningPlanning.VotingStore.{method}");
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.operation.id == subject)
            .unwrap();
        let input = OperationCaseInput {
            id: format!("cinema-voting-{method}-{mutation}"),
            invocation: Invocation {
                subject,
                arguments: match method {
                    "current" => vec![],
                    "open" => catalog_setup
                        .iter()
                        .find(|call| call.subject.ends_with("VotingStore.open"))
                        .unwrap()
                        .arguments
                        .clone(),
                    "cast" => vec![replacement_vote.clone()],
                    _ => vec![poll_id.clone()],
                },
            },
            setup: {
                let mut setup = if mutation == "empty" {
                    vec![]
                } else {
                    catalog_setup.clone()
                };
                if mutation == "closed" {
                    setup.push(Invocation {
                        subject: "HomeCinema.MovieNight.ScreeningPlanning.VotingStore.close".into(),
                        arguments: vec![poll_id.clone()],
                    });
                }
                setup
            },
            mocks: vec![],
        };
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                operation,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if !mutated {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{assessed:?}"
            );
        } else {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results.push(serde_json::json!({"subject":format!("VotingStore.{method}"),"mutation":mutation,"evidence":assessed}));
    }
    let search_plan = plan
        .operations
        .iter()
        .find(|operation| {
            operation.operation.id == "HomeCinema.MovieNight.Catalog.FilmCatalogStore.search"
        })
        .unwrap();
    let mut search_setup = catalog_setup.clone();
    if let VerificationValue::Record(fields) = &mut search_setup[1].arguments[0] {
        fields.insert(
            "title".into(),
            VerificationValue::String("AAA Other".into()),
        );
        fields.insert(
            "releaseYear".into(),
            VerificationValue::Integer("2010".into()),
        );
        fields.insert(
            "runtimeMinutes".into(),
            VerificationValue::Integer("180".into()),
        );
        fields.insert(
            "genres".into(),
            VerificationValue::List(vec![VerificationValue::Variant {
                declaration: "HomeCinema.MovieNight.Genre".into(),
                name: "Action".into(),
                payload: None,
            }]),
        );
        fields.insert(
            "availability".into(),
            VerificationValue::Variant {
                declaration: "HomeCinema.MovieNight.FilmAvailability".into(),
                name: "Wishlist".into(),
                payload: None,
            },
        );
    }
    let mut third = save.clone();
    if let VerificationValue::Record(fields) = &mut third {
        fields.insert(
            "id".into(),
            VerificationValue::String("00000000-0000-4000-8000-000000000003".into()),
        );
    }
    search_setup.push(Invocation {
        subject: "HomeCinema.MovieNight.Catalog.FilmCatalogApi.saveFilm".into(),
        arguments: vec![third],
    });
    let variant = |declaration: &str, name: &str| VerificationValue::Variant {
        declaration: format!("HomeCinema.MovieNight.{declaration}"),
        name: name.into(),
        payload: None,
    };
    for (scenario, sort, mutation) in [
        ("all", "TitleAscending", "none"),
        ("all", "NewestFirst", "none"),
        ("all", "HighestRated", "none"),
        ("all", "MostVoted", "none"),
        ("combined", "TitleAscending", "none"),
        ("page", "TitleAscending", "none"),
        ("missing", "TitleAscending", "none"),
        ("missing", "TitleAscending", "query"),
        ("genre", "TitleAscending", "genre"),
        ("all", "NewestFirst", "sort"),
        ("all", "TitleAscending", "tie"),
        ("page", "TitleAscending", "page"),
        ("page", "TitleAscending", "total"),
    ] {
        let source=match mutation {
            "query"=>original.replace("(filter.query===undefined||film.title.toLowerCase().includes(filter.query.toLowerCase()))","true"),
            "genre"=>original.replace("(filter.genre===undefined||film.genres.includes(filter.genre))","true"),
            "sort"=>original.replace("b.releaseYear-a.releaseYear","a.releaseYear-b.releaseYear"),
            "tie"=>original.replace("return primary||(a.id<b.id?-1:a.id>b.id?1:0);","return primary||(a.id<b.id?1:a.id>b.id?-1:0);"),
            "page"=>original.replace("items.slice(filter.offset,filter.offset+filter.limit)","items.slice(0,filter.limit)"),
            "total"=>original.replace("total:items.length,offset:filter.offset","total:items.slice(filter.offset,filter.offset+filter.limit).length,offset:filter.offset"),
            _=>original.clone(),
        };
        if mutation != "none" {
            assert_ne!(source, original);
        }
        fs::write(&production, source).unwrap();
        let mut fields = std::collections::BTreeMap::from([
            ("query".into(), VerificationValue::Option(None)),
            ("genre".into(), VerificationValue::Option(None)),
            ("minimumStars".into(), VerificationValue::Option(None)),
            ("availability".into(), VerificationValue::Option(None)),
            (
                "maximumRuntimeMinutes".into(),
                VerificationValue::Option(None),
            ),
            ("releasedFrom".into(), VerificationValue::Option(None)),
            ("sort".into(), variant("FilmSort", sort)),
            ("offset".into(), VerificationValue::Integer("0".into())),
            ("limit".into(), VerificationValue::Integer("10".into())),
        ]);
        if scenario == "combined" {
            fields.insert(
                "query".into(),
                VerificationValue::Option(Some(Box::new(VerificationValue::String(
                    "mOvIe".into(),
                )))),
            );
            for (field, value) in [
                ("genre", variant("Genre", "Comedy")),
                ("minimumStars", variant("StarRating", "FourStars")),
                ("availability", variant("FilmAvailability", "Available")),
                (
                    "maximumRuntimeMinutes",
                    VerificationValue::Integer("120".into()),
                ),
                ("releasedFrom", VerificationValue::Integer("2000".into())),
            ] {
                fields.insert(
                    field.into(),
                    VerificationValue::Option(Some(Box::new(value))),
                );
            }
        }
        if scenario == "missing" {
            fields.insert(
                "query".into(),
                VerificationValue::Option(Some(Box::new(VerificationValue::String(
                    "Not present".into(),
                )))),
            );
        }
        if scenario == "genre" {
            fields.insert(
                "genre".into(),
                VerificationValue::Option(Some(Box::new(variant("Genre", "Comedy")))),
            );
        }
        if scenario == "page" {
            fields.insert("offset".into(), VerificationValue::Integer("1".into()));
            fields.insert("limit".into(), VerificationValue::Integer("1".into()));
        }
        let input = OperationCaseInput {
            id: format!("cinema-search-{scenario}-{sort}-{mutation}"),
            invocation: Invocation {
                subject: search_plan.operation.id.clone(),
                arguments: vec![VerificationValue::Record(fields)],
            },
            setup: search_setup.clone(),
            mocks: vec![],
        };
        let mut worker = NativeWorker::spawn(&bundle.recipe, &temp.0, NativeLimits::default())
            .await
            .unwrap();
        let assessed = worker
            .execute_operation_contract(
                &ir,
                search_plan,
                &input,
                (
                    &plan.budgets,
                    OperationObservers {
                        states: true,
                        calls: true,
                        events: true,
                    },
                    &cancel,
                ),
            )
            .await
            .unwrap();
        if mutation == "none" {
            assert_eq!(
                assessed.assessment.result,
                VerificationResult::Pass,
                "{scenario}/{sort}: {assessed:?}"
            );
        } else {
            assert!(
                matches!(assessed.assessment.result, VerificationResult::Fail { .. }),
                "{scenario}/{mutation}: {assessed:?}"
            );
        }
        worker.finish().await.unwrap();
        projection_results.push(serde_json::json!({"subject":"search","scenario":scenario,"sort":sort,"mutation":mutation,"evidence":assessed}));
    }
    fs::write(output.join("cinema-comment-projection.json"),serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,"test_status":"pass","scope":"Actual Cinema comment overview with two persisted films/comments; independent Rust state projection; incorrect count and cross-film selection mutations","results":projection_results,"performance_status":"deferred"})).unwrap()).unwrap();
}

#[tokio::test]
async fn real_cinema_native_suite_typechecks_measures_storage_http_and_di_and_detects_wiring_mutation()
 {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema");
    let temp = Temp::new();
    for file in [
        "application/movie-night.ts",
        "persistence/sqlite.ts",
        "persistence/sqlite.test.ts",
        "app/session.ts",
        "http/app.ts",
        "domain/film-id.ts",
        "domain/release-year.ts",
        "domain/contracts.ts",
        "domain/timestamp.ts",
        "domain/release-year.test.ts",
        "domain/candidate-order-winner.ts",
        "domain/voting-rules.ts",
        "domain/voting-rules.test.ts",
    ] {
        let target = temp.0.join("src").join(file);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(root.join("src").join(file), target).unwrap();
    }
    fs::copy(root.join("tsconfig.json"), temp.0.join("tsconfig.json")).unwrap();
    std::os::unix::fs::symlink(root.join("node_modules"), temp.0.join("node_modules")).unwrap();
    let output =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-executor-conformance");
    fs::create_dir_all(&output).unwrap();
    let report = output.join(format!("{}-cinema-native.json", std::process::id()));
    let coverage = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/cinema-native-coverage/{}",
        std::process::id()
    ));
    let started = Instant::now();
    let mut compiler = NativeCommand::spawn(
        &ExecutionRecipe {
            program: "bun".into(),
            arguments: vec!["node_modules/typescript/bin/tsc".into(), "--noEmit".into()],
        },
        &temp.0,
        NativeLimits {
            cleanup: Duration::from_secs(30),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let compiler_pid = compiler.pid();
    let peak = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let sample = peak.clone();
    let sampling = tokio::spawn(async move {
        loop {
            if let Ok(status) = fs::read_to_string(format!("/proc/{compiler_pid}/status")) {
                if let Some(bytes) = status
                    .lines()
                    .find(|line| line.starts_with("VmHWM:"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|value| value.parse::<u64>().ok())
                {
                    sample.fetch_max(bytes * 1024, Ordering::Relaxed);
                }
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    let compilation = compiler.wait(&NativeCancellation::default()).await;
    sampling.abort();
    compilation.unwrap();
    let compile_ms = started.elapsed().as_secs_f64() * 1000.;
    assert!(peak.load(Ordering::Relaxed) > 0);
    let native = ExecutionRecipe {
        program: "env".into(),
        arguments: vec![
            format!("DCTX0_CINEMA_NATIVE_REPORT={}", report.display()),
            "bun".into(),
            "test".into(),
            "src".into(),
            "--coverage".into(),
            "--coverage-reporter=lcov".into(),
            format!("--coverage-dir={}", coverage.display()),
        ],
    };
    let started = Instant::now();
    let mut tests = NativeCommand::spawn(
        &native,
        &temp.0,
        NativeLimits {
            cleanup: Duration::from_secs(30),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let audit = tests.wait(&NativeCancellation::default()).await.unwrap();
    let mut evidence: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    evidence["process_wall_ms"] = serde_json::json!(started.elapsed().as_secs_f64() * 1000.);
    evidence["compiler"] = serde_json::json!({"elapsed_ms":compile_ms,"peak_rss_bytes":peak.load(Ordering::Relaxed),"memory_scope":"maximum observed Linux compiler VmHWM sampled every 1ms; lower bound if the final live sample is missed"});
    evidence["native_output"] = serde_json::json!(audit.stderr);
    evidence["coverage_file"] = serde_json::json!(coverage.join("lcov.info"));
    assert!(coverage.join("lcov.info").exists());
    fs::write(&report, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
    let route = temp.0.join("src/http/app.ts");
    let original = fs::read_to_string(&route).unwrap();
    let wrong = original.replace(
        "service.getFilm(c.req.param('id'))",
        "service.getFilm('00000000-0000-4000-8000-000000000099')",
    );
    assert_ne!(original, wrong);
    fs::write(route, wrong).unwrap();
    let mut mutated = NativeCommand::spawn(
        &ExecutionRecipe {
            program: "bun".into(),
            arguments: vec![
                "test".into(),
                "src/persistence/sqlite.test.ts".into(),
                "--test-name-pattern".into(),
                "Hono routes".into(),
            ],
        },
        &temp.0,
        NativeLimits {
            cleanup: Duration::from_secs(30),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        mutated
            .wait(&NativeCancellation::default())
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Build
    );
}

#[tokio::test]
async fn native_commands_bound_logs_time_cancellation_and_failures_without_protocol_claims() {
    let temp = Temp::new();
    let cancel = NativeCancellation::default();
    let mut command = NativeCommand::spawn(
        &recipe("import sys;print('{\"PASS\":true}');print('diagnostic',file=sys.stderr)"),
        &temp.0,
        limits(),
    )
    .await
    .unwrap();
    let audit = command.wait(&cancel).await.unwrap();
    assert!(audit.stdout.contains("PASS"));
    assert!(audit.stderr.contains("diagnostic"));
    assert_eq!(audit.received_messages, 0);
    assert_eq!(audit.exit_code, Some(0));
    assert_eq!(
        command.wait(&cancel).await.unwrap_err().kind,
        NativeFailureKind::WorkerExit
    );
    let mut failed = NativeCommand::spawn(
        &recipe("import sys;print('compiler error');sys.exit(7)"),
        &temp.0,
        limits(),
    )
    .await
    .unwrap();
    let failure = failed.wait(&cancel).await.unwrap_err();
    assert_eq!(failure.kind, NativeFailureKind::Build);
    assert!(failure.detail.contains("compiler error"));
    assert_eq!(
        NativeCommand::spawn(
            &ExecutionRecipe {
                program: "missing-native-tool-7594".into(),
                arguments: vec![]
            },
            &temp.0,
            limits()
        )
        .await
        .err()
        .unwrap()
        .kind,
        NativeFailureKind::Unavailable
    );
    for cancelled in [false, true] {
        let mut command = NativeCommand::spawn(
            &recipe("import time;time.sleep(300)"),
            &temp.0,
            NativeLimits {
                run: Duration::from_millis(30),
                ..limits()
            },
        )
        .await
        .unwrap();
        let token = NativeCancellation::default();
        if cancelled {
            token.cancel();
        }
        assert_eq!(
            command.wait(&token).await.unwrap_err().kind,
            if cancelled {
                NativeFailureKind::Cancelled
            } else {
                NativeFailureKind::Timeout
            }
        );
        assert!(!PathBuf::from(format!("/proc/{}", command.pid())).exists());
    }
    let mut loud = NativeCommand::spawn(
        &recipe("print('x'*1000)"),
        &temp.0,
        NativeLimits {
            log_bytes: 10,
            ..limits()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        loud.wait(&cancel).await.unwrap_err().kind,
        NativeFailureKind::LogLimit
    );
}

fn pure_input(
    result: &str,
) -> (
    WorkspaceIr,
    dctx0_verification::VerificationPlan,
    dctx0_verification::VerificationCase,
) {
    use dctx0_verification::*;
    let expr = if result == "Duration" {
        "Api.f() == Api.f()"
    } else {
        "Api.f() == 1"
    };
    let compiled = dctx0_language::compile(&[Source {
        file: "pure.dctx0".into(),
        text: format!(
            "workspace W {{ surface Api {{ operation f() -> {result} {{ pure }} }} invariant P {{ assert {expr} }} }}"
        ),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let mut ir = compiled.ir.unwrap();
    if result == "Duration" {
        // Unsupported-result coverage must not depend on a now-supported builtin.
        ir.operations.get_mut("W.Api.f").unwrap().result = TypeRef::builtin("FutureBuiltin");
    }
    let case = VerificationCase {
        id: "case".into(),
        variables: Default::default(),
        assertion: ir
            .nodes
            .iter()
            .find(|n| n.kind == StatementKind::Assertion)
            .unwrap()
            .expressions[0]
            .clone(),
    };
    let plan = VerificationPlan {
        schema_version: 1,
        snapshot: "test-pinned".into(),
        check_occurrence: 1,
        scope: VerificationScope {
            subjects: Default::default(),
            tasks: Default::default(),
        },
        budgets: Default::default(),
        obligations: vec![],
    };
    (ir, plan, case)
}
fn case_program(mode: &str) -> String {
    format!(
        r#"
import sys,json
mode={mode:?}
for line in sys.stdin:
 p=json.loads(line);r=p['request'];kind=r['kind']
 if kind=='begin':reply={{'kind':'begun','case_id':'wrong' if mode=='bad-begin' else r['case_id']}}
 elif kind=='end':reply={{'kind':'bye' if mode=='bad-end' else 'ended'}}
 elif kind=='invoke':
  if mode=='mock':
   print(json.dumps({{'protocol_version':1,'message':{{'kind':'mock-call','request_id':1,'invocation':{{'subject':'port','arguments':[]}}}}}}),flush=True)
   continue
  value={{'kind':'string','value':'duration'}} if mode=='duration' else {{'kind':'boolean','value':True}} if mode=='bad-return' else {{'kind':'integer','value':'1'}}
  observation={{'kind':'threw','message':'actual exception'}} if mode=='exception' else {{'kind':'unavailable','reason':'missing capability'}} if mode=='unavailable' else {{'kind':'returned','value':value}}
  events=[{{'subject':'event','payload':{{'kind':'unit'}}}}] if mode=='events' else []
  reply={{'kind':'ended'}} if mode=='bad-invoke' else {{'kind':'invoked','observation':observation,'events':events}}
 else:reply={{'kind':'bye'}}
 print(json.dumps({{'protocol_version':1,'message':{{'kind':'reply','id':p['id'],'reply':reply}}}}),flush=True)
 if kind=='shutdown':break
"#
    )
}

#[tokio::test]
async fn pure_operation_cases_validate_real_returns_effects_arguments_and_whole_case_deadlines() {
    let temp = Temp::new();
    let cancel = NativeCancellation::default();
    for mode in [
        "ok",
        "bad-return",
        "duration",
        "events",
        "exception",
        "unavailable",
        "bad-begin",
        "bad-end",
        "bad-invoke",
        "mock",
        "budget",
    ] {
        let (ir, mut plan, _) = pure_input(if mode == "duration" {
            "Duration"
        } else {
            "Int"
        });
        let operation = ir.operations.values().next().unwrap();
        let invocation = Invocation {
            subject: operation.id.clone(),
            arguments: vec![],
        };
        if mode == "budget" {
            plan.budgets.evaluation_steps = 0;
        }
        let mut worker = NativeWorker::spawn(&recipe(&case_program(mode)), &temp.0, limits())
            .await
            .unwrap();
        let result = worker
            .execute_pure_operation(&ir, operation, invocation, &plan.budgets, &cancel)
            .await;
        if mode.starts_with("bad-") && mode != "bad-return" || mode == "mock" {
            assert_eq!(result.unwrap_err().kind, NativeFailureKind::Protocol);
            continue;
        }
        match mode {
            "ok" => assert_eq!(result.unwrap(), VerificationResult::Pass),
            "bad-return" | "events" | "exception" => {
                assert!(matches!(result.unwrap(), VerificationResult::Fail { .. }))
            }
            _ => assert!(matches!(
                result.unwrap(),
                VerificationResult::Unknown { .. }
            )),
        }
        worker.abort().await.unwrap();
    }
    let (mut ir, plan, _) = pure_input("Int");
    let mut operation = ir.operations.values().next().unwrap().clone();
    let mut worker = NativeWorker::spawn(&recipe(&case_program("ok")), &temp.0, limits())
        .await
        .unwrap();
    assert_eq!(
        worker
            .execute_pure_operation(
                &ir,
                &operation,
                Invocation {
                    subject: "wrong".into(),
                    arguments: vec![]
                },
                &plan.budgets,
                &cancel
            )
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Technical
    );
    assert!(worker.session_evidence().is_empty());
    operation.parameters.push(FieldIr {
        name: "x".into(),
        ty: TypeRef::builtin("Int"),
    });
    ir.operations
        .insert(operation.id.clone(), operation.clone());
    assert_eq!(
        worker
            .execute_pure_operation(
                &ir,
                &operation,
                Invocation {
                    subject: operation.id.clone(),
                    arguments: vec![VerificationValue::Unit]
                },
                &plan.budgets,
                &cancel
            )
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Technical
    );
    worker.abort().await.unwrap();
    let (ir, plan, _) = pure_input("Int");
    let operation = ir.operations.values().next().unwrap();
    let mut worker = NativeWorker::spawn(
        &recipe("import time;time.sleep(1)"),
        &temp.0,
        NativeLimits {
            case: Duration::from_millis(5),
            ..limits()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        worker
            .execute_pure_operation(
                &ir,
                operation,
                Invocation {
                    subject: operation.id.clone(),
                    arguments: vec![]
                },
                &plan.budgets,
                &cancel
            )
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Timeout
    );
}

#[tokio::test]
async fn value_factory_transport_rejects_wrong_reply_kinds_and_whole_case_timeouts() {
    let temp = Temp::new();
    let cancel = NativeCancellation::default();
    let mut worker = NativeWorker::spawn(&recipe(&case_program("ok")), &temp.0, limits())
        .await
        .unwrap();
    assert_eq!(
        worker
            .execute_value_case(
                "case",
                "W.Value",
                &VerificationValue::Integer("1".into()),
                &cancel
            )
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Protocol
    );
    assert_eq!(worker.session_evidence().len(), 1);
    let mut worker = NativeWorker::spawn(
        &recipe("import time;time.sleep(1)"),
        &temp.0,
        NativeLimits {
            case: Duration::from_millis(5),
            ..limits()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        worker
            .execute_value_case(
                "case",
                "W.Value",
                &VerificationValue::Integer("1".into()),
                &cancel
            )
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Timeout
    );
    assert!(worker.session_evidence().is_empty());
}
#[tokio::test]
async fn example_protocol_guards_and_whole_session_deadlines_reap_workers() {
    use dctx0_verification::*;
    let temp = Temp::new();
    let cancel = NativeCancellation::default();
    let (ir, plan, case) = pure_input("Int");
    let ExpressionKind::Binary { left, .. } = &case.assertion.value else {
        panic!("Expected reference call")
    };
    let example = ExamplePlan {
        subject: "example".into(),
        order: vec![ExampleAction::Invoke(0), ExampleAction::Check(0)],
        variables: vec![],
        bindings: Default::default(),
        mocks: vec![],
        invocations: vec![InvocationStep {
            source_id: "call".into(),
            binding: "result".into(),
            invocation: *left.clone(),
        }],
        expectations: vec![Expectation::Success {
            source_id: "success".into(),
        }],
        issues: vec![],
    };
    for mode in ["bad-begin", "bad-invoke", "bad-end"] {
        let mut worker = NativeWorker::spawn(
            &recipe(&case_program(mode)),
            &temp.0,
            NativeLimits {
                initialization: Duration::from_secs(2),
                case: Duration::from_secs(2),
                run: Duration::from_secs(10),
                ..limits()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            worker
                .execute_example(&ir, &example, &plan.budgets, &cancel)
                .await
                .unwrap_err()
                .kind,
            NativeFailureKind::Protocol
        );
        assert!(!PathBuf::from(format!("/proc/{}", worker.pid())).exists());
        let prefix = worker.session_evidence();
        assert_eq!(
            prefix.len(),
            match mode {
                "bad-begin" => 0,
                "bad-invoke" => 1,
                _ => 2,
            }
        );
        if mode == "bad-end" {
            assert!(
                matches!(&prefix[1], NativeSessionEvidence::Invoked { observation: Observation::Returned { value: VerificationValue::Integer(value) }, .. } if value == "1")
            );
        }
    }
    let mut reusable = NativeWorker::spawn(&recipe(&case_program("ok")), &temp.0, limits())
        .await
        .unwrap();
    for _ in 0..2 {
        reusable
            .execute_example(&ir, &example, &plan.budgets, &cancel)
            .await
            .unwrap();
        assert_eq!(reusable.session_evidence().len(), 3);
        assert!(matches!(
            reusable.session_evidence().last(),
            Some(NativeSessionEvidence::Ended)
        ));
    }
    reusable.abort().await.unwrap();
    let delayed = case_program("ok")
        .replace("import sys,json", "import sys,json,time")
        .replace(
            "\n print(json.dumps",
            "\n time.sleep(0.08)\n print(json.dumps",
        );
    for pure in [true, false] {
        let mut worker = NativeWorker::spawn(
            &recipe(&delayed),
            &temp.0,
            NativeLimits {
                case: Duration::from_millis(200),
                ..limits()
            },
        )
        .await
        .unwrap();
        let error = if pure {
            worker
                .execute_pure_case(&ir, &plan, &case, &cancel)
                .await
                .unwrap_err()
        } else {
            worker
                .execute_example(&ir, &example, &plan.budgets, &cancel)
                .await
                .unwrap_err()
        };
        assert_eq!(error.kind, NativeFailureKind::Timeout);
        assert!(
            error.detail.contains("Whole native case deadline"),
            "{error}"
        );
        assert!(!PathBuf::from(format!("/proc/{}", worker.pid())).exists());
    }
}
#[tokio::test]
async fn central_cases_distinguish_contract_failures_unknowns_exceptions_and_protocol_errors() {
    let temp = Temp::new();
    let cancel = NativeCancellation::default();
    for mode in [
        "ok",
        "bad-return",
        "duration",
        "events",
        "exception",
        "unavailable",
        "bad-begin",
        "bad-end",
        "bad-invoke",
        "mock",
        "budget",
    ] {
        let (ir, mut plan, case) = pure_input(if mode == "duration" {
            "Duration"
        } else {
            "Int"
        });
        if mode == "budget" {
            plan.budgets.evaluation_steps = 0;
        }
        let mut w = NativeWorker::spawn(&recipe(&case_program(mode)), &temp.0, limits())
            .await
            .unwrap();
        let result = w.execute_pure_case(&ir, &plan, &case, &cancel).await;
        if mode.starts_with("bad-") && mode != "bad-return" || mode == "mock" {
            assert_eq!(result.unwrap_err().kind, NativeFailureKind::Protocol);
            if mode == "mock" {
                assert!(
                    matches!(w.session_evidence().last(), Some(NativeSessionEvidence::MockRequested { invocation, .. }) if invocation.subject == "port")
                );
                assert!(
                    !w.session_evidence()
                        .iter()
                        .any(|entry| matches!(entry, NativeSessionEvidence::MockReplied { .. }))
                );
            }
            continue;
        }
        let result = result.unwrap();
        match mode {
            "ok" => assert_eq!(result.assessment.result, VerificationResult::Pass),
            "bad-return" | "events" | "exception" => assert!(matches!(
                result.assessment.result,
                VerificationResult::Fail { .. }
            )),
            _ => assert!(matches!(
                result.assessment.result,
                VerificationResult::Unknown { .. }
            )),
        }
        w.request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
            .await
            .unwrap();
        w.finish().await.unwrap();
    }
}
#[tokio::test]
async fn duplicate_mock_ids_and_callback_rejection_abort_the_real_process() {
    let temp = Temp::new();
    let code = "import sys,json;json.loads(sys.stdin.readline());m={'protocol_version':1,'message':{'kind':'mock-call','request_id':1,'invocation':{'subject':'port','arguments':[]}}};print(json.dumps(m),flush=True);print(json.dumps(m),flush=True);sys.stdin.read()";
    for reject in [false, true] {
        let mut w = NativeWorker::spawn(&recipe(code), &temp.0, limits())
            .await
            .unwrap();
        let mut handler = |_: &Invocation| {
            if reject {
                Err(NativeFailure {
                    kind: NativeFailureKind::Technical,
                    detail: "Rejected callback".into(),
                    cleanup_error: None,
                })
            } else {
                Ok(Observation::Returned {
                    value: VerificationValue::Unit,
                })
            }
        };
        let result = w
            .request(
                WorkerRequest::Describe,
                &NativeCancellation::default(),
                &mut handler,
            )
            .await
            .unwrap_err();
        assert_eq!(
            result.kind,
            if reject {
                NativeFailureKind::Technical
            } else {
                NativeFailureKind::Protocol
            }
        );
    }
}

#[tokio::test]
async fn closed_worker_input_is_an_io_failure_and_reaps_the_leader() {
    let temp = Temp::new();
    let ready = temp.0.join("ready");
    let code = format!(
        "import os,time,pathlib;os.close(0);pathlib.Path({ready:?}).write_text('ready');time.sleep(300)"
    );
    let mut worker = NativeWorker::spawn(&recipe(&code), &temp.0, limits())
        .await
        .unwrap();
    let started = Instant::now();
    while !ready.exists() {
        assert!(started.elapsed() < Duration::from_secs(2));
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(
        worker
            .request(
                WorkerRequest::Describe,
                &NativeCancellation::default(),
                &mut no_mock
            )
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Io
    );
}

#[tokio::test]
async fn late_logs_are_rejected_and_owned_detached_processes_are_cleaned() {
    let temp = Temp::new();
    let cancel = NativeCancellation::default();
    let late = "import sys,json,time;p=json.loads(sys.stdin.readline());print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':{'kind':'bye'}}}),flush=True);time.sleep(0.04);print('x'*300,file=sys.stderr,flush=True)";
    let mut w = NativeWorker::spawn(
        &recipe(late),
        &temp.0,
        NativeLimits {
            log_bytes: 32,
            ..limits()
        },
    )
    .await
    .unwrap();
    w.request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    assert_eq!(
        w.finish().await.unwrap_err().kind,
        NativeFailureKind::LogLimit
    );
    let file = temp.0.join("detached.pid");
    let detached = format!(
        "import sys,json,subprocess,pathlib;p=json.loads(sys.stdin.readline());child=subprocess.Popen(['python3','-c','import time;time.sleep(300)'],start_new_session=True);pathlib.Path({file:?}).write_text(str(child.pid));print(json.dumps({{'protocol_version':1,'message':{{'kind':'reply','id':p['id'],'reply':{{'kind':'bye'}}}}}}),flush=True)"
    );
    let mut w = NativeWorker::spawn(
        &recipe(&detached),
        &temp.0,
        NativeLimits {
            cleanup: Duration::from_millis(40),
            ..limits()
        },
    )
    .await
    .unwrap();
    w.request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    let result = w.finish().await;
    // Managed ownership now includes detached sessions; successful cleanup must
    // leave neither a running descendant nor an unreaped zombie.
    let id = fs::read_to_string(file).unwrap().parse::<i32>().unwrap();
    assert!(result.is_ok(), "{result:?}");
    assert!(!std::path::Path::new(&format!("/proc/{id}")).exists());
}

#[tokio::test]
async fn lifecycle_inputs_setup_failures_observer_protocol_and_deadlines_remain_explicit() {
    use dctx0_verification::{LifecycleCaseInput, VerificationScope, project_program};
    let compiled = dctx0_language::compile(&[Source {
        file: "lifecycle-native.dctx0".into(),
        text: r#"workspace W {
 type Status = Open | Closed
 entity Item { id: String
 status: Status
 key id }
 port Store { operation setup() -> Int { requires true } }
 surface Api { operation finish(id: String) -> Int }
 lifecycle Flow for Item using status {
 state Open initial
 state Closed terminal
 transition Api.finish from Open to Closed when true
 }
 }"#
        .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let plan = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: std::collections::BTreeSet::from(["W.Flow".into()]),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let lifecycle = &plan.lifecycles[0];
    let path = &lifecycle.paths[0];
    let input = LifecycleCaseInput {
        keys: vec![VerificationValue::String("x".into())],
        setup: vec![Invocation {
            subject: "W.Store.setup".into(),
            arguments: vec![],
        }],
        arguments: vec![vec![VerificationValue::String("x".into())]],
    };
    let code = r#"
import sys,json,time
for line in sys.stdin:
 p=json.loads(line); r=p['request']; k=r['kind']
 if k=='begin': reply={'kind':'begun','case_id':r['case_id']}
 elif k=='invoke': reply={'kind':'invoked','observation':OBSERVATION,'events':[]}
 elif k=='observe': reply={'kind':'ended'}
 elif k=='end': reply={'kind':'ended'}
 else: reply={'kind':'bye'}
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':reply}}),flush=True)
 if k=='shutdown': break
"#;
    let t = Temp::new();
    let cancel = NativeCancellation::default();
    let spawn = |observation: &str| recipe(&code.replace("OBSERVATION", observation));
    let mut bad_keys = input.clone();
    bad_keys.keys[0] = VerificationValue::Integer("1".into());
    let mut worker = NativeWorker::spawn(
        &spawn("{'kind':'returned','value':{'kind':'integer','value':'1'}}"),
        &t.0,
        limits(),
    )
    .await
    .unwrap();
    let invalid = worker
        .execute_lifecycle(&ir, lifecycle, path, &bad_keys, &plan.budgets, &cancel)
        .await
        .unwrap();
    assert!(
        matches!(invalid.assessment.result, VerificationResult::Unknown { issue } if issue.code == "LIFECYCLE_INPUT_CONTRACT")
    );
    assert!(invalid.setup.is_empty());
    assert!(invalid.observations.is_empty());
    assert!(worker.session_evidence().is_empty());
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
    let mut denied = ir.clone();
    let guard = denied
        .nodes
        .iter_mut()
        .find(|node| node.owner == "W.Store.setup" && node.kind == StatementKind::Requires)
        .unwrap();
    let ExpressionKind::Literal { text, .. } = &mut guard.expressions[0].value else {
        panic!("Compiled literal guard expected")
    };
    *text = "false".into();
    let mut worker = NativeWorker::spawn(
        &spawn("{'kind':'returned','value':{'kind':'integer','value':'1'}}"),
        &t.0,
        limits(),
    )
    .await
    .unwrap();
    let blocked = worker
        .execute_lifecycle(&denied, lifecycle, path, &input, &plan.budgets, &cancel)
        .await
        .unwrap();
    assert!(
        matches!(&blocked.assessment.result, VerificationResult::Unknown { issue } if issue.code == "OPERATION_PRECONDITION_UNMET"),
        "{:?}",
        blocked.assessment.result
    );
    assert!(blocked.setup.is_empty());
    assert!(blocked.observations.is_empty());
    assert!(blocked.blocked_context.is_some());
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
    for outcome in [
        "{'kind':'returned','value':{'kind':'boolean','value':True}}",
        "{'kind':'returned','value':{'kind':'failure','value':{'declaration':'W.SetupRejected','payload':None}}}",
        "{'kind':'unavailable','reason':'missing setup'}",
        "{'kind':'threw','message':'setup crash'}",
    ] {
        let mut worker = NativeWorker::spawn(&spawn(outcome), &t.0, limits())
            .await
            .unwrap();
        let result = worker
            .execute_lifecycle(&ir, lifecycle, path, &input, &plan.budgets, &cancel)
            .await
            .unwrap();
        assert_ne!(result.assessment.result, VerificationResult::Pass);
        assert_eq!(result.setup.len(), 1);
        assert!(result.observations.is_empty());
        worker
            .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
            .await
            .unwrap();
        worker.finish().await.unwrap();
    }
    let accepted = "{'kind':'returned','value':{'kind':'integer','value':'1'}}";
    let mut unsupported = ir.clone();
    unsupported
        .operations
        .get_mut("W.Store.setup")
        .unwrap()
        .result = TypeRef::builtin("FutureBuiltin");
    let mut worker = NativeWorker::spawn(&spawn(accepted), &t.0, limits())
        .await
        .unwrap();
    let result = worker
        .execute_lifecycle(
            &unsupported,
            lifecycle,
            path,
            &input,
            &plan.budgets,
            &cancel,
        )
        .await
        .unwrap();
    assert!(
        matches!(result.assessment.result,VerificationResult::Unknown {issue} if issue.code=="UNSUPPORTED_TYPE")
    );
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
    let mut worker = NativeWorker::spawn(&spawn(accepted), &t.0, limits())
        .await
        .unwrap();
    let error = worker
        .execute_lifecycle(&ir, lifecycle, path, &input, &plan.budgets, &cancel)
        .await
        .unwrap_err();
    assert_eq!(error.kind, NativeFailureKind::Protocol);
    worker.abort().await.unwrap();
    for mode in 0..4 {
        let mut bad = input.clone();
        match mode {
            0 => bad.arguments.clear(),
            1 => bad.setup[0].subject = "undeclared".into(),
            2 => bad.setup[0].arguments.push(VerificationValue::Unit),
            _ => bad.arguments[0][0] = VerificationValue::Unit,
        }
        let mut worker = NativeWorker::spawn(&spawn(accepted), &t.0, limits())
            .await
            .unwrap();
        assert_eq!(
            worker
                .execute_lifecycle(&ir, lifecycle, path, &bad, &plan.budgets, &cancel)
                .await
                .unwrap_err()
                .kind,
            NativeFailureKind::Technical
        );
        worker.abort().await.unwrap();
    }
    let observed = code.replace("OBSERVATION", accepted).replace("elif k=='observe': reply={'kind':'ended'}", "elif k=='observe': reply={'kind':'observed','states':{'W.Item':[{'kind':'record','value':{'id':{'kind':'string','value':'x'},'status':{'kind':'variant','value':{'declaration':'W.Status','name':'Closed','payload':None}}}}]}}" );
    let mut worker = NativeWorker::spawn(&recipe(&observed), &t.0, limits())
        .await
        .unwrap();
    let blocked = worker
        .execute_lifecycle(&ir, lifecycle, path, &input, &plan.budgets, &cancel)
        .await
        .unwrap();
    assert!(
        matches!(blocked.assessment.result, VerificationResult::Fail {issue} if issue.code == "LIFECYCLE_STATE_MISMATCH")
    );
    assert!(blocked.blocked_context.is_some());
    assert!(blocked.observations.is_empty());
    assert!(worker.session_evidence().iter().any(|entry| matches!(entry, NativeSessionEvidence::Observed { states, .. } if states.contains_key("W.Item"))));
    worker
        .request(WorkerRequest::Shutdown, &cancel, &mut no_mock)
        .await
        .unwrap();
    worker.finish().await.unwrap();
    let mut short = limits();
    short.case = Duration::from_millis(5);
    let mut worker = NativeWorker::spawn(&recipe("import time; time.sleep(1)"), &t.0, short)
        .await
        .unwrap();
    assert_eq!(
        worker
            .execute_lifecycle(&ir, lifecycle, path, &input, &plan.budgets, &cancel)
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Timeout
    );
    worker.abort().await.unwrap();
}
