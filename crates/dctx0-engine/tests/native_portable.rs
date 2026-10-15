//! These tests run unchanged on each native release runner and exercise Child
//! explicitly on Linux. The coupled runner supplies its absolute interpreter.
use dctx0_engine::{
    NativeCancellation, NativeCommand, NativeFailureKind, NativeLimits, NativeSupervision,
    NativeWorker,
};
use dctx0_target_sdk::{ExecutionRecipe, WorkerRequest};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn recipe(code: &str) -> ExecutionRecipe {
    let python = std::env::var("DCTX0_TEST_PYTHON").expect("Run through scripts/native_portability.py or set DCTX0_TEST_PYTHON to an absolute Python interpreter");
    assert!(Path::new(&python).is_absolute());
    ExecutionRecipe {
        program: python,
        arguments: vec!["-u".into(), "-c".into(), code.into()],
    }
}
fn limits(supervision: NativeSupervision) -> NativeLimits {
    NativeLimits {
        supervision,
        initialization: Duration::from_secs(2),
        case: Duration::from_millis(200),
        run: Duration::from_secs(3),
        cleanup: Duration::from_millis(200),
        ..Default::default()
    }
}
#[tokio::test]
async fn portable_status_logs_input_limits_and_idempotent_cleanup() {
    for mode in [NativeSupervision::Auto, NativeSupervision::Child] {
        let mut command = NativeCommand::spawn(&recipe("import sys; print(sys.stdin.read(),end=''); print('actual-error',file=sys.stderr,end=''); sys.exit(7)"), &std::env::temp_dir(), limits(mode)).await.unwrap();
        assert_eq!(
            command
                .write_input(&vec![0; 1_048_577])
                .await
                .unwrap_err()
                .kind,
            NativeFailureKind::MessageLimit
        );
        command.write_input(b"actual-input").await.unwrap();
        let audit = command
            .wait_status(&NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(audit.exit_code, Some(7));
        assert_eq!(audit.stdout, "actual-input");
        assert_eq!(audit.stderr, "actual-error");
        let capabilities = audit.supervision.unwrap();
        assert_eq!(
            capabilities.process_tree,
            cfg!(target_os = "linux") && mode == NativeSupervision::Auto
        );
        assert!(!capabilities.collective_memory_limit);
        assert!(
            command
                .wait_status(&NativeCancellation::default())
                .await
                .is_err()
        );
    }
}
#[tokio::test]
async fn portable_protocol_identity_shutdown_and_legacy_identity_readability() {
    let code = r#"import json,sys
for line in sys.stdin:
 p=json.loads(line)
 reply={'kind':'bye'} if p['request']['kind']=='shutdown' else {'kind':'ended'}
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':reply}}),flush=True)
 if p['request']['kind']=='shutdown':break
"#;
    let mut worker = NativeWorker::spawn(
        &recipe(code),
        &std::env::temp_dir(),
        limits(NativeSupervision::Child),
    )
    .await
    .unwrap();
    assert!(worker.runtime_identity().is_err());
    worker
        .request(
            WorkerRequest::End,
            &NativeCancellation::default(),
            &mut |_| unreachable!(),
        )
        .await
        .unwrap();
    let identity = worker.runtime_identity().unwrap();
    assert_eq!(identity.schema_version, 2);
    assert_eq!(identity.method, "captured-launch");
    assert!(identity.still_current().unwrap());
    assert_eq!(
        identity.launch.as_ref().unwrap().arguments,
        recipe(code).arguments
    );
    let mut changed = identity.clone();
    changed.executable_hash = "changed".into();
    assert!(!changed.still_current().unwrap());
    let legacy: dctx0_engine::NativeRuntimeIdentity = serde_json::from_value(serde_json::json!({"schema_version":1,"executable_hash":"old","executable_path":"old","executable_bytes":3,"initial_environment_hash":"old"})).unwrap();
    assert_eq!(legacy.method, "procfs-live-image");
    worker
        .request(
            WorkerRequest::Shutdown,
            &NativeCancellation::default(),
            &mut |_| unreachable!(),
        )
        .await
        .unwrap();
    assert_eq!(worker.finish().await.unwrap().exit_code, Some(0));
    worker.abort().await.unwrap();
    worker.abort().await.unwrap();
}
#[tokio::test]
async fn portable_protocol_errors_missing_late_replies_timeout_and_cancellation_never_succeed() {
    for mode in [NativeSupervision::Auto, NativeSupervision::Child] {
        for (code, expected) in [
            (
                "import sys;sys.stdin.readline();print('invalid-json',flush=True)",
                NativeFailureKind::Protocol,
            ),
            (
                "import sys;sys.stdin.readline();sys.exit(9)",
                NativeFailureKind::WorkerExit,
            ),
            (
                "import sys,time;sys.stdin.readline();time.sleep(10)",
                NativeFailureKind::Timeout,
            ),
        ] {
            let mut settings = limits(mode);
            settings.initialization = Duration::from_millis(300);
            let mut worker = NativeWorker::spawn(&recipe(code), &std::env::temp_dir(), settings)
                .await
                .unwrap();
            let error = worker
                .request(
                    WorkerRequest::Describe,
                    &NativeCancellation::default(),
                    &mut |_| unreachable!(),
                )
                .await
                .unwrap_err();
            assert_eq!(error.kind, expected, "{error:?}");
            assert!(error.cleanup_error.is_none(), "{error:?}");
            worker.abort().await.unwrap();
        }
        let mut worker = NativeWorker::spawn(
            &recipe("import time;time.sleep(10)"),
            &std::env::temp_dir(),
            limits(mode),
        )
        .await
        .unwrap();
        let cancel = NativeCancellation::default();
        cancel.cancel();
        assert_eq!(
            worker
                .request(WorkerRequest::Describe, &cancel, &mut |_| unreachable!())
                .await
                .unwrap_err()
                .kind,
            NativeFailureKind::Cancelled
        );
        assert!(worker.runtime_identity().is_err());
        worker.abort().await.unwrap();
        let mut command = NativeCommand::spawn(
            &recipe("import time;time.sleep(10)"),
            &std::env::temp_dir(),
            NativeLimits {
                run: Duration::from_millis(100),
                ..limits(mode)
            },
        )
        .await
        .unwrap();
        assert_eq!(
            command
                .wait_status(&NativeCancellation::default())
                .await
                .unwrap_err()
                .kind,
            NativeFailureKind::Timeout
        );
    }
}
#[tokio::test]
async fn portable_stream_drain_is_bounded_when_a_descendant_keeps_pipes_open() {
    let code = "import subprocess,sys;subprocess.Popen([sys.executable,'-c','import time;time.sleep(1)']);sys.exit(0)";
    let mut command = NativeCommand::spawn(
        &recipe(code),
        &std::env::temp_dir(),
        limits(NativeSupervision::Child),
    )
    .await
    .unwrap();
    let started = Instant::now();
    assert_eq!(
        command
            .wait_status(&NativeCancellation::default())
            .await
            .unwrap_err()
            .kind,
        NativeFailureKind::Cleanup
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    // The deliberately untracked fixture terminates on its own; no long-lived
    // child is left behind by this test of the documented capability boundary.
    tokio::time::sleep(Duration::from_secs(1)).await;
}
