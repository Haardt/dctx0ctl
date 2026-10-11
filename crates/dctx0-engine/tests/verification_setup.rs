use dctx0_engine::*;
use dctx0_target_sdk::*;
use dctx0_target_typescript::BunHarness;
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dctx0-verification-setup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("spec")).unwrap();
        fs::create_dir_all(path.join("app")).unwrap();
        fs::write(
            path.join("spec/main.dctx0"),
            "workspace W { type Number = Int }",
        )
        .unwrap();
        Self(path)
    }
    fn project(&self) -> Project {
        Project::new(&self.0.join("spec"), None).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn scope() -> ConfiguredVerificationScope {
    ConfiguredVerificationScope {
        subjects: BTreeSet::from(["W.Number".into()]),
        tasks: BTreeSet::new(),
    }
}
#[test]
fn setup_preserves_developer_files_configuration_and_compiler_history_without_execution() {
    let t = Temp::new();
    let p = t.project();
    let app = t.0.join("app");
    assert!(
        p.optional_verification_configuration(None)
            .unwrap()
            .is_none()
    );
    assert!(p.check().unwrap().ok);
    let history = p.history().unwrap();
    let report = p
        .initialize_verification(&app, &BunHarness, Default::default())
        .unwrap();
    assert!(!report.executed);
    assert_eq!(report.created.len(), 6);
    let observer = app.join(".dctx0/testing/typescript/module-observer.ts");
    let observer_text = fs::read_to_string(&observer).unwrap();
    assert!(observer_text.contains("installModuleObserver"));
    assert!(report.configuration.scope.subjects.is_empty());
    assert_eq!(
        p.verification_configuration(None).unwrap(),
        (app.clone(), report.configuration.clone())
    );
    assert!(
        p.optional_verification_configuration(Some(&app))
            .unwrap()
            .is_some()
    );
    let adapter = app.join(".dctx0/testing/typescript/adapter.ts");
    fs::write(&adapter, "reviewed existing developer adapter").unwrap();
    let repeated = p
        .initialize_verification(&app, &BunHarness, scope())
        .unwrap();
    assert!(repeated.created.is_empty());
    assert_eq!(repeated.preserved.len(), 6);
    assert_eq!(fs::read_to_string(&observer).unwrap(), observer_text);
    assert!(repeated.configuration.scope.subjects.is_empty());
    assert_eq!(
        fs::read_to_string(adapter).unwrap(),
        "reviewed existing developer adapter"
    );
    assert_eq!(p.history().unwrap().len(), history.len());
    assert_eq!(p.history().unwrap()[0].snapshot_id, history[0].snapshot_id);
}
struct Broken(u8);
impl HarnessGenerator for Broken {
    fn generate(
        &self,
        r: &HarnessRequest,
    ) -> std::result::Result<HarnessBundle, dctx0_target_sdk::Error> {
        if self.0 == 0 {
            return Err(dctx0_target_sdk::Error::Invalid(
                "missing capability".into(),
            ));
        }
        let mut b = BunHarness.generate(r)?;
        match self.0 {
            1 => b.sdk_version += 1,
            2 => b.target.clear(),
            3 => b.generator_version.clear(),
            4 => b.artifacts.push(Artifact {
                path: "adapter.ts".into(),
                contents: "duplicate".into(),
            }),
            5 => b.artifacts[0].path = "../escape.ts".into(),
            6 => b.target = "../outside".into(),
            7 => b.recipe.program.clear(),
            8 => b.artifacts[0].path = "n".repeat(300),
            10 => b.artifacts[0].path = format!("{}/contract.ts", "n".repeat(300)),
            _ => {}
        }
        Ok(b)
    }
    fn scaffold(&self) -> Artifact {
        let mut a = BunHarness.scaffold();
        if self.0 == 9 {
            a.path = "unexpected.ts".into();
        }
        a
    }
}
#[test]
fn invalid_providers_and_paths_are_rejected_before_testing_files_are_published() {
    for mode in 0..10 {
        let t = Temp::new();
        let p = t.project();
        assert!(
            p.initialize_verification(&t.0.join("app"), &Broken(mode), scope())
                .is_err()
        );
        assert!(!t.0.join("app/.dctx0/testing/verification.json").exists());
    }
    let t = Temp::new();
    fs::write(t.0.join("file"), "").unwrap();
    assert!(
        t.project()
            .initialize_verification(&t.0.join("file"), &BunHarness, scope())
            .is_err()
    );
    assert!(
        t.project()
            .initialize_verification(&t.0.join("missing"), &BunHarness, scope())
            .is_err()
    );
}
#[test]
fn configuration_is_strict_bounded_and_never_silently_reassigns_a_project() {
    let t = Temp::new();
    let p = t.project();
    let app = t.0.join("app");
    let original = p
        .initialize_verification(&app, &BunHarness, scope())
        .unwrap()
        .configuration;
    let path = app.join(".dctx0/testing/verification.json");
    for bad in [
        serde_json::json!({"schema_version":2}),
        serde_json::json!({"scope":{"subjects":[],"tasks":[-1]}}),
        serde_json::json!({"scope":{"subjects":[" "],"tasks":[]}}),
        serde_json::json!({"target":"UNKNOWN"}),
        serde_json::json!({"runtime":""}),
        serde_json::json!({"unknown":true}),
        serde_json::json!({"adapter":"../escape.ts"}),
        serde_json::json!({"fixtures":"./invalid.json"}),
    ] {
        let mut value = serde_json::to_value(&original).unwrap();
        for (key, value2) in bad.as_object().unwrap() {
            value[key] = value2.clone();
        }
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(p.verification_configuration(None).is_err());
    }
    fs::write(&path, vec![b' '; 524289]).unwrap();
    assert!(
        p.verification_configuration(None)
            .unwrap_err()
            .to_string()
            .contains("512 KiB")
    );
    fs::write(&path, "{").unwrap();
    assert!(
        p.initialize_verification(&app, &BunHarness, scope())
            .is_err()
    );
    let mut value = original.clone();
    value.runtime = "other-runtime".into();
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        p.initialize_verification(&app, &BunHarness, scope())
            .unwrap_err()
            .to_string()
            .contains("different target/runtime")
    );
    fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
    let other = t.0.join("other");
    fs::create_dir(&other).unwrap();
    assert!(
        p.initialize_verification(&other, &BunHarness, scope())
            .unwrap_err()
            .to_string()
            .contains("already associated")
    );
    assert!(!other.join(".dctx0/testing/verification.json").exists());
    let pointer = t.0.join("spec/.dctx0ctl/verification.json");
    fs::write(
        &pointer,
        serde_json::json!({"schema_version":2,"project":app}).to_string(),
    )
    .unwrap();
    assert!(
        p.verification_configuration(None)
            .unwrap_err()
            .to_string()
            .contains("Unsupported verification project association")
    );
}
#[cfg(unix)]
#[test]
fn symlinks_and_non_directory_parents_cannot_escape_the_testing_area() {
    use std::os::unix::fs::symlink;
    for leaf in [false, true] {
        let t = Temp::new();
        let app = t.0.join("app");
        let outside = t.0.join("outside");
        fs::create_dir(&outside).unwrap();
        if leaf {
            fs::create_dir_all(app.join(".dctx0/testing/typescript")).unwrap();
            fs::write(outside.join("owned.ts"), "untouched").unwrap();
            symlink(
                outside.join("owned.ts"),
                app.join(".dctx0/testing/typescript/contract.ts"),
            )
            .unwrap();
        } else {
            symlink(&outside, app.join(".dctx0")).unwrap();
        }
        assert!(
            t.project()
                .initialize_verification(&app, &BunHarness, scope())
                .is_err()
        );
        assert!(!outside.join("testing").exists());
    }
    let t = Temp::new();
    fs::write(t.0.join("app/.dctx0"), "not a directory").unwrap();
    assert!(
        t.project()
            .initialize_verification(&t.0.join("app"), &BunHarness, scope())
            .is_err()
    );
    let t = Temp::new();
    fs::create_dir_all(t.0.join("app/.dctx0/testing/typescript/contract.ts")).unwrap();
    assert!(
        t.project()
            .initialize_verification(&t.0.join("app"), &BunHarness, scope())
            .is_err()
    );
}

#[tokio::test]
async fn generated_setup_scaffold_fails_explicitly_in_real_bun_instead_of_returning_example_answers()
 {
    let t = Temp::new();
    let p = t.project();
    let app = t.0.join("app");
    p.initialize_verification(&app, &BunHarness, Default::default())
        .unwrap();
    let started = std::time::Instant::now();
    let mut worker = NativeWorker::spawn(
        &ExecutionRecipe {
            program: "bun".into(),
            arguments: vec!["run".into(), "worker.ts".into()],
        },
        &app.join(".dctx0/testing/typescript"),
        Default::default(),
    )
    .await
    .unwrap();
    let cancel = NativeCancellation::default();
    let mut no_mock = |_: &dctx0_core::Invocation| {
        Err(NativeFailure {
            kind: NativeFailureKind::Technical,
            detail: "unexpected mock".into(),
            cleanup_error: None,
        })
    };
    let metric = worker
        .request(WorkerRequest::Metrics, &cancel, &mut no_mock)
        .await
        .unwrap();
    let failure = worker
        .request(
            WorkerRequest::Begin {
                case_id: "required-case".into(),
            },
            &cancel,
            &mut no_mock,
        )
        .await
        .unwrap_err();
    assert_eq!(failure.kind, NativeFailureKind::Technical);
    assert!(failure.detail.contains("not implemented"));
    worker.abort().await.unwrap();
    assert!(!p.database.exists());
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-executor-conformance");
    fs::create_dir_all(&evidence).unwrap();
    fs::write(evidence.join(format!("{}-setup.json",std::process::id())),serde_json::to_vec_pretty(&serde_json::json!({"scope":"Bun scaffold initialization cannot certify a task","elapsed_ms":started.elapsed().as_secs_f64()*1000.,"metrics":metric,"failure":failure})).unwrap()).unwrap();
}

#[cfg(unix)]
#[test]
fn kernel_path_errors_and_failed_association_publication_are_rejected_and_resumable() {
    use std::os::unix::fs::PermissionsExt;
    for mode in [8, 10] {
        let t = Temp::new();
        let app = t.0.join("app");
        fs::create_dir_all(app.join(".dctx0/testing/typescript")).unwrap();
        assert!(matches!(
            t.project()
                .initialize_verification(&app, &Broken(mode), scope()),
            Err(dctx0_engine::Error::Io(_))
        ));
        assert!(!app.join(".dctx0/testing/verification.json").exists());
    }
    let t = Temp::new();
    let p = t.project();
    let state = t.0.join("spec/.dctx0ctl");
    fs::create_dir(&state).unwrap();
    fs::write(state.join("project.lock"), "").unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o555)).unwrap();
    let result = p.initialize_verification(&t.0.join("app"), &BunHarness, scope());
    fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(result,Err(dctx0_engine::Error::Io(ref error)) if error.kind()==std::io::ErrorKind::PermissionDenied),
        "{result:?}"
    );
    assert!(!state.join("verification.json").exists());
    assert!(!p.database.exists());
    let resumed = p
        .initialize_verification(&t.0.join("app"), &BunHarness, Default::default())
        .unwrap();
    assert!(resumed.created.is_empty());
    assert_eq!(resumed.configuration.scope, scope());
    assert!(p.verification_configuration(None).is_ok());
}
