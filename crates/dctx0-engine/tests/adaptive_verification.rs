use dctx0_core::VerificationResult;
use dctx0_engine::{
    ConfiguredVerificationScope, NativeCancellation, NativeCommand, NativeLimits,
    NativeVerificationReport, Project,
};
use dctx0_target_typescript::BunHarness;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    project: Project,
    app: PathBuf,
}
fn copy(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".bin" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
impl Fixture {
    fn new(variant: &str, mode: &str) -> Self {
        let original = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/conformance/adaptive-typescript");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(original.join("production-frozen.json")).unwrap())
                .unwrap();
        for (path, expected) in manifest["sha256"].as_object().unwrap() {
            assert_eq!(
                dctx0_store::VerificationArtifact::new(
                    "application/octet-stream",
                    fs::read(original.join(path)).unwrap()
                )
                .reference
                .hash,
                expected.as_str().unwrap(),
                "Frozen production changed: {path}"
            );
        }
        let root = std::env::temp_dir().join(format!(
            "dctx0-adaptive-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let app = root.join("app");
        copy(&original.join(variant), &app);
        let spec = root.join("spec");
        fs::create_dir(&spec).unwrap();
        fs::write(
            spec.join("w.dctx0"),
            fs::read(original.join("spec/workspace.dctx0")).unwrap(),
        )
        .unwrap();
        let project = Project::new(&spec, None).unwrap();
        assert!(project.check().unwrap().ok);
        project
            .initialize_verification(
                &app,
                &BunHarness,
                ConfiguredVerificationScope {
                    subjects: ["Adaptive.Counter.next".into()].into(),
                    tasks: Default::default(),
                },
            )
            .unwrap();
        if variant == "service" {
            fs::write(
                app.join(".dctx0/testing/typescript/config.json"),
                serde_json::to_vec(&serde_json::json!({"entrypoint":mode})).unwrap(),
            )
            .unwrap();
        }
        Self { root, project, app }
    }
    async fn run(&self) -> NativeVerificationReport {
        self.project
            .run_verification(
                Some(&self.app),
                Default::default(),
                &BunHarness,
                &NativeCancellation::default(),
                true,
            )
            .await
            .unwrap()
    }
    async fn typecheck(&self) -> serde_json::Value {
        let started = std::time::Instant::now();
        let recipe = dctx0_target_sdk::ExecutionRecipe {
            program: "bun".into(),
            arguments: vec!["node_modules/typescript/bin/tsc".into(), "--noEmit".into()],
        };
        let mut command = NativeCommand::spawn(&recipe, &self.app, NativeLimits::default())
            .await
            .unwrap();
        let audit = command.wait(&NativeCancellation::default()).await.unwrap();
        serde_json::json!({"command":recipe,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"peak_rss_bytes":null,"memory_limitation":"NativeCommand does not yet retain reliable per-compiler VmHWM; never reported as zero","audit":audit})
    }
    fn evidence(&self, report: &NativeVerificationReport) -> serde_json::Value {
        let store =
            dctx0_store::Store::open(&self.project.database, dctx0_store::AccessMode::ReadOnly)
                .unwrap();
        let metrics = report
            .evidence
            .run
            .artifacts
            .iter()
            .filter(|a| a.media_type == "application/vnd.dctx0.runtime-metrics+json")
            .map(|reference| {
                let artifact = store
                    .verification_artifact(&report.evidence.id, &reference.hash)
                    .unwrap();
                serde_json::from_slice::<serde_json::Value>(&artifact.contents).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(metrics.len(), 1);
        serde_json::json!({"report":report,"native_worker_metrics":metrics,"run_elapsed_ms":report.evidence.run.elapsed_us as f64 / 1000.0,"memory_scope":"Native Bun worker VmHWM; Rust test-process peak memory is not measured here"})
    }
    fn mutate(&self, file: &str, before: &str, after: &str) {
        let path = self.app.join(file);
        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains(before));
        fs::write(path, source.replace(before, after)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[tokio::test]
async fn one_plan_verifies_direct_service_and_real_http_paths_and_detects_business_and_wiring_mutations()
 {
    let mut plans = BTreeMap::new();
    let mut evidence = Vec::new();
    for (variant, mode, file, before, after) in [
        (
            "direct",
            "direct",
            "src/counter.ts",
            "return value + 1",
            "return 2",
        ),
        (
            "service",
            "service",
            "src/service.ts",
            "return amount + this.increment",
            "return amount + this.increment + 1",
        ),
        (
            "service",
            "http",
            "src/app.ts",
            "increment: asValue(1)",
            "increment: asValue(2)",
        ),
    ] {
        let f = Fixture::new(variant, mode);
        let compiler = f.typecheck().await;
        let good = f.run().await;
        assert_eq!(
            good.evidence.run.result,
            VerificationResult::Pass,
            "{variant}/{mode}: {:?}",
            good.evidence.run
        );
        assert!(good.adopted_tasks.is_empty());
        assert_eq!(
            good.evidence.run.required_cases.len(),
            good.evidence.run.cases.len()
        );
        assert_eq!(
            good.evidence
                .run
                .cases
                .iter()
                .filter(|c| c.subject == "Adaptive.Counter.next")
                .count(),
            4
        );
        assert!(
            good.evidence
                .run
                .cases
                .iter()
                .all(|c| c.result == VerificationResult::Pass)
        );
        plans.insert(
            format!("{variant}/{mode}"),
            good.evidence.run.provenance.plan_hash.clone(),
        );
        f.mutate(file, before, after);
        let bad = f.run().await;
        evidence.push(serde_json::json!({"variant":variant,"entrypoint":mode,"production_typecheck":compiler,"initial":f.evidence(&good),"mutation":f.evidence(&bad)}));
        assert!(
            matches!(bad.evidence.run.result, VerificationResult::Fail { ref issue } if issue.code == "OPERATION_CLAUSE_VIOLATED"),
            "{variant}/{mode}: {:?}",
            bad.evidence.run.result
        );
        assert!(
            bad.evidence
                .run
                .cases
                .iter()
                .any(|c| c.subject == "Adaptive.Counter.next"
                    && matches!(c.result, VerificationResult::Fail { .. }))
        );
        assert_eq!(
            good.evidence.run.provenance.plan_hash,
            bad.evidence.run.provenance.plan_hash
        );
        assert_ne!(
            good.evidence.run.provenance.implementation,
            bad.evidence.run.provenance.implementation
        );
        assert_eq!(
            f.project
                .verification_summary(&good.evidence.id)
                .unwrap()
                .result,
            good.evidence.run.result
        );
    }
    assert_eq!(
        plans
            .values()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1,
        "Different architectures must use the same exact pinned plan"
    );
    let fake = Fixture::new("direct", "direct");
    fake.mutate(
        ".dctx0/testing/typescript/adapter.ts",
        "() => nextValue(argument)",
        "() => 2",
    );
    let fake_report = fake.run().await;
    assert!(
        matches!(fake_report.evidence.run.result, VerificationResult::Fail { ref issue } if issue.code == "OPERATION_CLAUSE_VIOLATED")
    );
    evidence.push(serde_json::json!({"variant":"direct","entrypoint":"fake-constant-adapter","negative_acceptance":fake.evidence(&fake_report)}));
    let target =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/adaptive-conformance");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join(format!("{}-adaptive.json", std::process::id())), serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,"scope":"One frozen spec/plan; direct function, real Awilix service and real Hono route; three production mutations plus fake constant adapter","status":"pass","runs":evidence,"limitations":["Not complete Cinema acceptance or automatic adoption","Per-compiler and Rust-process peak memory remain unavailable; native worker VmHWM is retained"]})).unwrap()).unwrap();
}
