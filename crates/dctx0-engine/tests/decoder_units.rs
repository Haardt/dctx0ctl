//! Real compiler-selected Go cohorts and their persisted native evidence.
use dctx0_core::{UnitCoverage, VerificationAttestation, VerificationResult};
use dctx0_engine::*;
use dctx0_target_go::GoHarness;
use std::fs;

#[tokio::test]
async fn decoder_units_reuse_unchanged_cohorts_and_reject_a_superseded_composition() {
    let root = std::env::temp_dir().join(format!("dctx0-go-units-{}", std::process::id()));
    let app = root.join("app");
    let spec = root.join("spec");
    fs::create_dir_all(&spec).unwrap();
    fs::write(
        spec.join("w.dctx0"),
        r#"workspace W {
            constraint "Operations staff review decoder documentation."
            value A = String { length 1..2 }
            value B = String { length 1..2 }
        }"#,
    )
    .unwrap();
    let decoder = r#"package decoder
import ("encoding/json"; "errors"; "unicode/utf8")
func Decode(raw []byte) (string, error) {
    var value *string
    if err := json.Unmarshal(raw, &value); err != nil { return "", err }
    if value == nil || utf8.RuneCountInString(*value) < 1 || utf8.RuneCountInString(*value) > 2 {
        return "", errors.New("outside contract")
    }
    return *value, nil
}
"#;
    for group in ["alpha", "beta"] {
        fs::create_dir_all(app.join(group)).unwrap();
        fs::write(app.join(group).join("decoder.go"), decoder).unwrap();
    }
    fs::write(
        app.join("go.mod"),
        "module example.com/decoderfixture\n\ngo 1.25.0\n",
    )
    .unwrap();
    fs::write(app.join(".gitignore"), "**/.dctx0ctl/\n").unwrap();
    let project = Project::new(&spec, None).unwrap();
    assert!(project.check().unwrap().ok);
    project
        .initialize_verification(
            &app,
            &GoHarness,
            ConfiguredVerificationScope {
                subjects: ["W".into()].into(),
                tasks: Default::default(),
            },
        )
        .unwrap();
    let config_path = app.join(".dctx0/testing/verification.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["environment"] = serde_json::json!({
        "mode":"controlled", "inherit":["PATH","HOME","TMPDIR","GOCACHE"],
        "values":{"GOENV":"off","GOWORK":"off","GOTOOLCHAIN":"local","CGO_ENABLED":"0"}
    });
    fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let units = ".dctx0/testing/units.json";
    fs::write(app.join(units), r#"{"schema_version":1,"groups":[
        {"id":"alpha","subjects":["W.A"],"decoders":[{"subject":"W.A","import_path":"example.com/decoderfixture/alpha","function":"Decode"}]},
        {"id":"beta","subjects":["W.B"],"decoders":[{"subject":"W.B","import_path":"example.com/decoderfixture/beta","function":"Decode"}]}
    ]}"#).unwrap();
    let cancel = NativeCancellation::default();
    let run = |mode| {
        let project = &project;
        let app = &app;
        let cancel = &cancel;
        async move {
            project
                .run_decoder_units(
                    app,
                    units,
                    &GoHarness,
                    &NativeReuseOptions {
                        mode,
                        ..Default::default()
                    },
                    cancel,
                )
                .await
        }
    };
    let blocked = run(NativeReuseMode::Auto).await.unwrap_err();
    assert!(
        blocked
            .to_string()
            .contains("FULL_EXECUTION_REQUIRES_EXPLICIT_OFF")
    );
    assert!(!app.join(".dctx0ctl/unit-workspaces").exists());
    let inactive = app.join("alpha/conditional.go");
    let excluded = "//go:build never_active_in_decoder_fixture\n\npackage decoder\nfunc Decode([]byte) (string, error) { return \"\", nil }\n";
    fs::write(&inactive, excluded).unwrap();
    let first = run(NativeReuseMode::Off).await.unwrap();
    assert_eq!(first.worker_starts, Some(2));
    assert_eq!(first.executed_cases, first.plan.required_cases.len());
    assert!(
        first
            .composition
            .units
            .iter()
            .all(|unit| unit.coverage == UnitCoverage::FreshPass)
    );
    let warm = run(NativeReuseMode::Require).await.unwrap();
    assert_eq!(warm.worker_starts, Some(0));
    assert!(warm.native_runs.is_empty());
    assert_eq!(warm.reused_cases, first.executed_cases);
    assert_eq!(warm.open_obligations, first.open_obligations);
    assert_eq!(warm.open_obligations[0].code, "MANUAL_REQUIREMENT");
    assert!(
        warm.composition
            .units
            .iter()
            .all(|unit| unit.coverage == UnitCoverage::ReusedPass)
    );

    fs::write(&inactive, excluded.split_once("\n\n").unwrap().1).unwrap();
    let tag_edit = run(NativeReuseMode::Auto).await.unwrap();
    assert_eq!(
        tag_edit.composition.units[0].coverage,
        UnitCoverage::Unknown
    );
    assert_eq!(
        tag_edit.composition.units[1].coverage,
        UnitCoverage::ReusedPass
    );
    fs::write(inactive, excluded).unwrap();

    let store = dctx0_store::Store::open(&project.database, AccessMode::ReadOnly).unwrap();
    assert_eq!(
        store
            .evidence_composition(&project.key(), &warm.composition_id)
            .unwrap(),
        warm.composition,
    );
    let alpha = &warm.composition.units[0];
    let origin = alpha.proof.as_ref().unwrap();
    let mut forged = origin.clone();
    *forged.case_bodies.values_mut().next().unwrap() = "0".repeat(64);
    assert!(
        store
            .validate_unit_origin(&project.key(), alpha.binding.clone(), forged)
            .err()
            .unwrap()
            .to_string()
            .contains("NATIVE_UNIT_CASE_INVALID")
    );
    let original = project.saved_verification_run(&origin.native_run).unwrap();
    assert!(
        matches!(&original.run.result, VerificationResult::Unknown { issue } if issue.code == "MANUAL_REQUIREMENT")
    );
    let mut unlinked = origin.clone();
    unlinked.artifacts.retain(|hash| {
        original.run.artifacts.iter().any(|artifact| {
            artifact.hash == *hash && artifact.media_type != dctx0_core::UNIT_BINDING_MEDIA_TYPE
        })
    });
    assert!(
        store
            .validate_unit_origin(&project.key(), alpha.binding.clone(), unlinked)
            .err()
            .unwrap()
            .to_string()
            .contains("NATIVE_UNIT_BINDING_ARTIFACT_MISSING")
    );
    drop(store);

    create_profiles(&app, &dctx0_target_go::Provider, true, false, false).unwrap();
    let plan = project.task_plan("latest").unwrap();
    let member = plan
        .members
        .iter()
        .find(|member| member.candidate.subjects == ["W.B"])
        .unwrap();
    let snapshot = project.load("latest").unwrap();
    let profiles = project.profiles(Some(&app), Some("go")).unwrap().unwrap();
    let profile = dctx0_tasks::select_profiles(
        &snapshot,
        &member.candidate,
        &profiles.manifest,
        &profiles.documents,
    );
    let (_, config) = project.verification_configuration(Some(&app)).unwrap();
    let program = project
        .verification_plan(
            VerificationSelection {
                subjects: config.scope.subjects,
                ..Default::default()
            },
            &Default::default(),
            config.budgets,
        )
        .unwrap();
    let mut inspection = CompositionTaskAttestation {
        task: member.task_id,
        profile_bundle_hash: "stale profile".into(),
        review: None,
        inspection: VerificationAttestation {
            schema_version: 1,
            actor: "inspector".into(),
            subject: format!("T{}", member.task_id),
            snapshot: snapshot.id.clone(),
            plan_hash: dctx0_verification::unit_planner::evidence_digest(&program).unwrap(),
            requirements: member
                .candidate
                .subjects
                .iter()
                .chain(&member.candidate.types)
                .chain(&member.candidate.facts)
                .cloned()
                .collect(),
            implementation_artifacts: dctx0_store::composition_implementation(
                &warm
                    .composition
                    .units
                    .iter()
                    .map(|unit| unit.binding.clone())
                    .collect::<Vec<_>>(),
            ),
            report: "Reviewed the required scalar bounds and the real Go decoder".into(),
            limitations: vec!["Finite contract evidence".into()],
            result: VerificationResult::Pass,
        },
    };
    let attest = |inspection: &CompositionTaskAttestation| {
        project.publish_composition_attestations(
            &app,
            &warm.composition_id,
            units,
            &GoHarness,
            std::slice::from_ref(inspection),
        )
    };
    assert!(
        attest(&inspection)
            .unwrap_err()
            .to_string()
            .contains("profile pin changed")
    );
    inspection.profile_bundle_hash = profile.bundle.hash;
    let receipt = attest(&inspection).unwrap();
    assert!(
        receipt.run_id.is_none(),
        "Technical composition should retain its original native runs"
    );
    assert_eq!(
        project
            .status(Some(&app), None)
            .unwrap()
            .tasks
            .agent_current,
        1
    );

    fs::write(
        app.join("beta/decoder.go"),
        decoder.replace("outside contract", "bad length"),
    )
    .unwrap();
    assert!(attest(&inspection).is_err());
    assert_eq!(
        project.status(Some(&app), None).unwrap().tasks.agent_stale,
        1
    );
    let changed = run(NativeReuseMode::Auto).await.unwrap();
    assert_eq!(changed.worker_starts, Some(1));
    assert!(changed.reused_cases > 0 && changed.executed_cases > 0);
    assert_eq!(
        changed.reused_cases + changed.executed_cases,
        first.executed_cases
    );
    assert_eq!(
        changed.composition.units[0].coverage,
        UnitCoverage::ReusedPass
    );
    assert_eq!(
        changed.composition.units[1].coverage,
        UnitCoverage::FreshPass
    );
    fs::write(
        app.join("beta/decoder.go"),
        decoder.replace(
            "return *value, nil",
            "return \"\", errors.New(\"mutation\")",
        ),
    )
    .unwrap();
    let failed = run(NativeReuseMode::Auto).await.unwrap();
    assert_eq!(
        failed.composition.units[0].coverage,
        UnitCoverage::ReusedPass
    );
    assert_eq!(failed.composition.units[1].coverage, UnitCoverage::Fail);
    assert!(failed.composition.units[1].proof.is_none());
    fs::remove_dir_all(root).unwrap();
}
