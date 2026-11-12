use super::*;
use crate::{AccessMode, Store};
use dctx0_core::*;
use dctx0_verification::{application_plan::*, application_session::*, *};
use rusqlite::Connection;

fn fixture() -> (WorkspaceIr, ApplicationTestPlan) {
    let compiled = dctx0_language::compile(&[Source {
        file: "application.dctx0".into(),
        text: "workspace W { value Year = Int { range 3..4 } }".into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let program = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: ["W.Year".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let cases = case_manifest(&program).required_cases;
    let config = ApplicationTestConfiguration {
        schema_version: 2,
        groups: vec![ExecutionGroup {
            id: "native".into(),
            backend: TestBackend::ApplicationTests,
            target: "go".into(),
            selection: ["TestYear".into()].into(),
            area: TestArea {
                cases: cases.clone(),
                ..Default::default()
            },
            resources: GroupResources {
                namespace: "native".into(),
                timeout_seconds: 60,
                max_message_bytes: 1_048_576,
            },
        }],
    };
    let plan = application_test_plan(
        &program,
        &config,
        &[TestRegistration {
            group: "native".into(),
            test: "TestYear".into(),
            form: TestForm::Decoder,
            cases,
        }],
    )
    .unwrap();
    (ir, plan)
}

fn register(session: &mut ApplicationSession<'_>) {
    assert!(matches!(
        session.handle(ApplicationRequest::Register {
            test: "TestYear".into()
        }),
        ApplicationReply::Registered
    ));
}
fn next(session: &mut ApplicationSession<'_>) -> (u64, Vec<u8>) {
    let ApplicationReply::Case {
        context,
        directive: ApplicationDirective::Decode { raw, .. },
        ..
    } = session.handle(ApplicationRequest::Next {
        test: "TestYear".into(),
    })
    else {
        panic!("Expected decoder case");
    };
    (context, raw)
}
fn actual_decoder(raw: &[u8]) -> DecoderOutcome {
    let value = serde_json::from_slice::<serde_json::Value>(raw).unwrap();
    match value.as_i64() {
        Some(value @ 3..=4) => DecoderOutcome::Accepted {
            value: VerificationValue::Integer(value.to_string()),
        },
        _ => DecoderOutcome::Rejected {
            reason: "production rejected".into(),
        },
    }
}
fn native_proof_fixture() -> (
    dctx0_verification::application_evidence::ApplicationExecutionProof,
    VerificationRun,
) {
    use dctx0_verification::application_evidence::{
        ApplicationExecutionProof, ApplicationPhaseProof,
    };
    let (ir, plan) = fixture();
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    register(&mut session);
    for _ in &plan.manifest.required_cases {
        let (context, raw) = next(&mut session);
        session.handle(ApplicationRequest::Observe {
            context,
            observation: ApplicationObservation::Decoded {
                outcome: actual_decoder(&raw),
                raw,
            },
        });
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
    }
    session.handle(ApplicationRequest::Next {
        test: "TestYear".into(),
    });
    session.handle(ApplicationRequest::CloseTest {
        test: "TestYear".into(),
    });
    session.handle(ApplicationRequest::Finish);
    let assessment = session.report();
    assert_eq!(assessment.result, VerificationResult::Pass);
    let inputs = std::collections::BTreeMap::from([("production.go".into(), "source-hash".into())]);
    let materialized =
        std::collections::BTreeMap::from([("tests.binary".into(), "binary-hash".into())]);
    let build = std::collections::BTreeMap::from([("go.mod".into(), "module-hash".into())]);
    let run = VerificationRun {
        schema_version: 1,
        origin: VerificationRunOrigin::Native,
        started_at_unix_ns: "1".into(),
        elapsed_us: 1,
        provenance: VerificationProvenance {
            snapshot: plan.program.snapshot.clone(),
            check_occurrence: plan.program.check_occurrence as i64,
            plan_hash: "plan-hash".into(),
            implementation: inputs.clone(),
            dependencies: materialized.clone(),
            adapter_hash: "bundle-hash".into(),
            configuration_hash: "config-hash".into(),
            harness_hash: "receipt-hash".into(),
            interpreter_version: "interpreter".into(),
            generator_version: plan.program.generator_version.clone(),
            protocol_version: 2,
            target: "go".into(),
            runtime_version: "go".into(),
            build_configuration: build.clone(),
            task_pin: None,
            incomplete: vec![],
        },
        budgets: plan.program.budgets.clone(),
        selected_subjects: plan.program.scope.subjects.clone(),
        required_subjects: plan.program.closure.clone(),
        not_selected: vec![],
        required_cases: plan.manifest.required_cases.clone(),
        cases: assessment.cases.clone(),
        open_obligations: vec![],
        bounded_obligations: vec![],
        artifacts: vec![],
        result: VerificationResult::Pass,
    };
    let proof = ApplicationExecutionProof {
        schema_version: 1,
        plan,
        assessment,
        source_inputs: inputs,
        build_inputs: build,
        materialized_inputs: materialized,
        toolchain_inputs: [("VERSION".into(), "go-version".into())].into(),
        phases: ["toolchain", "catalog", "native-tests"]
            .into_iter()
            .map(|phase| ApplicationPhaseProof {
                phase: phase.into(),
                exit_code: Some(0),
                runtime_hash: "a".repeat(64),
                supervised: true,
            })
            .collect(),
        receipt_hash: "receipt-hash".into(),
    };
    (proof, run)
}
fn assigned_native_proof_fixture() -> (
    dctx0_verification::application_evidence::ApplicationExecutionProof,
    VerificationRun,
    VerificationCaseRecord,
) {
    use dctx0_verification::application_plan::*;
    let (mut proof, mut run) = native_proof_fixture();
    let manifest = proof.plan.manifest.clone();
    let mut native = manifest.required_cases.clone();
    let legacy_case = native.pop_last().unwrap();
    let legacy_record = run
        .cases
        .iter()
        .find(|case| case.id == legacy_case)
        .unwrap()
        .clone();
    assert!(!native.is_empty());
    let mut configuration = proof.plan.configuration.clone();
    configuration.groups[0].area = TestArea {
        cases: native.clone(),
        ..Default::default()
    };
    let mut legacy = configuration.groups[0].clone();
    legacy.id = "legacy".into();
    legacy.backend = TestBackend::Adapter;
    legacy.selection = ["legacy.go".into()].into();
    legacy.resources.namespace = "legacy".into();
    legacy.area.cases = [legacy_case.clone()].into();
    configuration.groups.push(legacy);
    let mut registrations = proof.plan.registrations.clone();
    registrations[0].cases = native.clone();
    proof.plan =
        application_test_plan(&proof.plan.program, &configuration, &registrations).unwrap();
    proof.assessment.required_cases = native.clone();
    proof.assessment.cases.retain(|case| case.id != legacy_case);
    run.required_cases = native;
    run.cases = proof.assessment.cases.clone();
    assert_eq!(proof.plan.manifest, manifest);
    (proof, run, legacy_record)
}

fn profile_catalog() -> (dctx0_core::TargetManifest, Vec<dctx0_core::ProfileDocument>) {
    use dctx0_core::*;
    let manifest = TargetManifest {
        format_version: 1,
        target: "rust".into(),
        template_version: 1,
        standard_keys: standard_profile_keys(),
    };
    let mut docs = vec![ProfileDocument {
        target: "rust".into(),
        key: None,
        path: ".dctx0/profiles/rust/common.md".into(),
        text: "Original guidance".into(),
        content_hash: dctx0_changes::hash(&"Original guidance"),
        subjects: vec![],
    }];
    for key in &manifest.standard_keys {
        docs.push(ProfileDocument {
            target: "rust".into(),
            key: Some(key.clone()),
            path: format!(".dctx0/profiles/rust/{}", key.path()),
            text: "Original guidance".into(),
            content_hash: dctx0_changes::hash(&"Original guidance"),
            subjects: vec![],
        });
    }
    (manifest, docs)
}
fn stored_hybrid(
    failure: &str,
) -> (
    Store,
    VerificationRun,
    VerificationCaseManifest,
    String,
    VerificationTaskPin,
    i64,
    i64,
) {
    use dctx0_verification::hybrid_evidence::{HYBRID_EXECUTION_MEDIA_TYPE, HybridExecutionProof};
    let mut store = Store {
        connection: Connection::open_in_memory().unwrap(),
        mode: AccessMode::ReadWrite,
        maintenance_lock: None,
    };
    crate::initialize(&mut store.connection, 0, AccessMode::ReadWrite).unwrap();
    let inspection_mode = failure.starts_with("inspection_");
    let sources = [Source {
        file: "application.dctx0".into(),
        text: if inspection_mode {
            "workspace W { value Year = Int { range 3..4 } proof Approval }"
        } else {
            "workspace W { value Year = Int { range 3..4 } }"
        }
        .into(),
    }];
    let ir = dctx0_language::compile(&sources).ir.unwrap();
    let snapshot = dctx0_changes::snapshot(&sources, ir.clone(), dctx0_graph::build(&ir));
    use dctx0_changes::HistoryWriter;
    store.publish("p", &snapshot, None).unwrap();
    let check = store.task_check("p").unwrap().0;
    store
        .publish_tasks(
            "p",
            &dctx0_tasks::plan(&snapshot).unwrap(),
            check,
            None,
            0,
            &Default::default(),
        )
        .unwrap();
    let session = store.task_session("p", None, None).unwrap();
    let task = session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == dctx0_tasks::TaskKind::Contract)
        .unwrap()
        .task_id;
    let started = store
        .task_action(
            "p",
            dctx0_tasks::Action::Start {
                task,
                actor: "author".into(),
                reservation: None,
            },
        )
        .unwrap()
        .state
        .unwrap();
    let done = store
        .task_action(
            "p",
            dctx0_tasks::Action::Complete {
                task,
                actor: "author".into(),
                claim: started.claim.unwrap(),
                version: started.version,
                result: dctx0_tasks::TaskResult {
                    summary: "historical".into(),
                    artifacts: vec![],
                    decisions: vec![],
                    implementation: None,
                },
            },
        )
        .unwrap()
        .state
        .unwrap();
    if failure == "done_inspected_prior_fake" {
        let mut previous = done.clone();
        previous.evidence.push(dctx0_tasks::CheckEvidence {
            requirements_hash: "forged".into(),
            implementation: "forged".into(),
            verifier: "dctx0-inspected-verifier".into(),
            verifier_version: "forged".into(),
            command: "fake".into(),
            started_at: "1".into(),
            finished_at: "2".into(),
            report: "previous fake report".into(),
            status: dctx0_tasks::CheckStatus::Pass,
        });
        super::super::read::event(
            &store.connection,
            "p",
            Some(task),
            "fixture-import",
            "Prior fake",
            Some(&previous),
        )
        .unwrap();
    }
    if matches!(
        failure,
        "done_inspected_claim" | "done_inspected_reservation" | "done_inspected_actor"
    ) {
        let mut conflicting = done.clone();
        match failure {
            "done_inspected_claim" => conflicting.claim = Some(77),
            "done_inspected_reservation" => conflicting.reservation = Some(77),
            _ => conflicting.actor = Some("another-worker".into()),
        }
        super::super::read::event(
            &store.connection,
            "p",
            Some(task),
            "fixture-conflict",
            "Conflicting state",
            Some(&conflicting),
        )
        .unwrap();
    }
    let session = store.task_session("p", None, None).unwrap();
    let candidate = &session.member(task).unwrap().candidate;
    let (profile, docs) = profile_catalog();
    let bundle = dctx0_tasks::select_profiles(&snapshot, candidate, &profile, &docs)
        .bundle
        .hash;
    let pin = VerificationTaskPin {
        plan_id: session.plan.id,
        event_sequence: session.event_seq,
        requirements: [(task, candidate.requirements_hash.clone())].into(),
        profile_bundles: [(task, bundle)].into(),
    };
    let (mut app, mut app_run, legacy) = assigned_native_proof_fixture();
    if inspection_mode {
        let program = project_program(
            &ir,
            &snapshot.id,
            check as u64,
            VerificationScope {
                subjects: ["W".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        app.plan =
            application_test_plan(&program, &app.plan.configuration, &app.plan.registrations)
                .unwrap();
        app_run.selected_subjects = program.scope.subjects.clone();
        app_run.required_subjects = program.closure.clone();
        app_run.not_selected = program.not_selected.clone();
    }
    app.plan.program.snapshot = snapshot.id.clone();
    app.plan.program.check_occurrence = check as u64;
    app_run.provenance.snapshot = snapshot.id.clone();
    app_run.provenance.task_pin = Some(pin.clone());
    app_run.provenance.check_occurrence = check;
    let mut program = app.plan.program.clone();
    if failure == "plan" {
        program.generator_version.push_str("-forged");
    }
    let plan_artifact = crate::VerificationArtifact::new(
        "application/vnd.dctx0.plan+json",
        serde_json::to_vec(&program).unwrap(),
    );
    app_run.provenance.plan_hash = plan_artifact.reference.hash.clone();
    let phase_audit = serde_json::json!({"pid":22,"stderr":"","stdout":"","received_messages":0,"exit_code":0,"supervision":{"process_tree":true}});
    let mut receipt = serde_json::json!({"schema_version":1,"group":app.assessment.group,"backend":"application-tests","target":"go","adoption_authority":false,"adopted_tasks":[],"adoption_issues":[],"adoption_proposal":null,"evidence":null,"replay_of":null,"requested_case":null,"plan_hash":app_run.provenance.adapter_hash,"source_inputs":app.source_inputs,"build_inputs":app.build_inputs,"materialized_inputs":app.materialized_inputs,"toolchain_inputs":app.toolchain_inputs,"assessment":app.assessment,"issues":[],"result":{"status":"PASS"},"phases":app.phases.iter().map(|phase| serde_json::json!({"phase":phase.phase,"runtime":{"executable_hash":phase.runtime_hash},"audit":phase_audit})).collect::<Vec<_>>()});
    match failure {
        "receipt" => receipt["assessment"]["finalized"] = false.into(),
        "phase" => receipt["phases"][0]["runtime"]["executable_hash"] = "b".repeat(64).into(),
        "phase_exit" => receipt["phases"][0]["audit"]["exit_code"] = 1.into(),
        "phase_supervision" => {
            receipt["phases"][0]["audit"]["supervision"]["process_tree"] = false.into()
        }
        "source" => receipt["source_inputs"] = serde_json::json!({"forged.go":"x"}),
        "replay" => receipt["replay_of"] = "historical".into(),
        "evidence" => receipt["evidence"] = serde_json::json!({"id":"historical"}),
        "authority" => receipt["adoption_authority"] = true.into(),
        "target" => receipt["target"] = "typescript".into(),
        _ => (),
    }
    let receipt_artifact = crate::VerificationArtifact::new(
        "application/vnd.dctx0.application-native-receipt+json",
        serde_json::to_vec(&receipt).unwrap(),
    );
    app.receipt_hash = receipt_artifact.reference.hash.clone();
    app_run.provenance.harness_hash = app.receipt_hash.clone();
    app_run.started_at_unix_ns = "100".into();
    let mut run = app_run.clone();
    run.started_at_unix_ns = "1".into();
    run.elapsed_us = 4;
    run.provenance.protocol_version = 1;
    run.provenance.adapter_hash = "adapter".into();
    run.provenance.harness_hash = "legacy-harness".into();
    run.required_cases = app.plan.manifest.required_cases.clone();
    run.cases.push(legacy);
    let mut proof = HybridExecutionProof {
        schema_version: 1,
        application: app,
        application_run: app_run,
        adapter_group: "legacy".into(),
        adapter_started_at_unix_ns: "2000".into(),
        adapter_elapsed_us: 1,
    };
    if failure == "source_pin" {
        proof
            .application_run
            .provenance
            .implementation
            .insert("other.go".into(), "changed".into());
    }
    if failure == "duplicate_group" {
        proof.adapter_group = proof.application.assessment.group.clone();
    }
    let manifest = proof.application.plan.manifest.clone();
    let mut audit = serde_json::json!({"pid":42,"stderr":"","stdout":"","received_messages":1,"exit_code":0,"supervision":{"process_tree":true}});
    if failure == "audit_exit" {
        audit["exit_code"] = 1.into();
    }
    if failure == "audit_supervision" {
        audit["supervision"]["process_tree"] = false.into();
    }
    let audit = crate::VerificationArtifact::new(
        "application/vnd.dctx0.native-audit+json",
        serde_json::to_vec(&audit).unwrap(),
    );
    let runtime = crate::VerificationArtifact::new("application/vnd.dctx0.runtime-identity+json", serde_json::to_vec(&serde_json::json!({"schema_version":1,"executable_hash":"a".repeat(64),"executable_path":"/go","executable_bytes":1,"initial_environment_hash":"b".repeat(64)})).unwrap());
    let limits = crate::VerificationArtifact::new(
        "application/vnd.dctx0.native-limits+json",
        serde_json::to_vec(
            &serde_json::json!({"schema_version":1,"limits":{"supervision":"auto","initialization":{"secs":1,"nanos":0},"case":{"secs":1,"nanos":0},"run":{"secs":1,"nanos":0},"cleanup":{"secs":1,"nanos":0},"log_bytes":10,"message_bytes":10,"evidence_bytes":10,"evidence_entries":10}}),
        )
        .unwrap(),
    );
    let supervision = crate::VerificationArtifact::new(
        "application/vnd.dctx0.native-supervision+json",
        serde_json::to_vec(&serde_json::json!({"process_tree":true,"detached_descendants":true,"collective_memory_limit":false})).unwrap(),
    );
    let mapping = crate::VerificationArtifact::new(
        "application/vnd.dctx0.case-manifest+json",
        serde_json::to_vec(&manifest).unwrap(),
    );
    let hybrid = crate::VerificationArtifact::new(
        HYBRID_EXECUTION_MEDIA_TYPE,
        serde_json::to_vec(&proof).unwrap(),
    );
    let mut artifacts = vec![
        plan_artifact,
        mapping,
        hybrid,
        audit,
        runtime,
        limits,
        supervision,
    ];
    if failure != "missing_receipt" {
        artifacts.push(receipt_artifact);
    }
    if failure == "done_inspected_hybrid_missing" {
        artifacts.retain(|artifact| artifact.reference.media_type != HYBRID_EXECUTION_MEDIA_TYPE);
    }
    for (name, media) in [
        ("missing_audit", "application/vnd.dctx0.native-audit+json"),
        (
            "missing_runtime",
            "application/vnd.dctx0.runtime-identity+json",
        ),
        ("missing_limits", "application/vnd.dctx0.native-limits+json"),
        (
            "missing_supervision",
            "application/vnd.dctx0.native-supervision+json",
        ),
    ] {
        if failure == name {
            artifacts.retain(|artifact| artifact.reference.media_type != media);
        }
    }
    if inspection_mode && failure != "inspection_missing" {
        use dctx0_verification::program_inspection::{
            ProgramInspection, assess_with_inspections, inspection_requirements,
        };
        let subject = "W.Approval";
        let mut inspections = vec![ProgramInspection {
            inspection: VerificationAttestation {
                schema_version: 1,
                actor: "reviewer".into(),
                subject: subject.into(),
                snapshot: snapshot.id.clone(),
                plan_hash: run.provenance.plan_hash.clone(),
                requirements: inspection_requirements(&program, subject).unwrap(),
                implementation_artifacts: run.provenance.implementation.clone(),
                report: "Explicitly reviewed the approval requirement".into(),
                limitations: vec!["Agent judgement".into()],
                result: VerificationResult::Pass,
            },
            review: None,
        }];
        let mut assessment = assess_with_inspections(
            &snapshot.ir,
            &program,
            &run.cases,
            &run.provenance.plan_hash,
            &run.provenance.implementation,
            &inspections,
        );
        assert_eq!(assessment.result, VerificationResult::Pass);
        assert!(assessment.open_obligations.is_empty());
        match failure {
            "inspection_stale_source" => {
                inspections[0]
                    .inspection
                    .implementation_artifacts
                    .insert("production.go".into(), "stale".into());
            }
            "inspection_stale_plan" => inspections[0].inspection.plan_hash = "stale".into(),
            "inspection_absent" => inspections.clear(),
            "inspection_duplicate" => inspections.push(inspections[0].clone()),
            "inspection_forged_assessment" => assessment.subjects.clear(),
            _ => (),
        }
        if failure != "inspection_no_attestation" {
            artifacts.push(crate::VerificationArtifact::new(
                "application/vnd.dctx0.program-inspections+json",
                serde_json::to_vec(&inspections).unwrap(),
            ));
        }
        artifacts.push(crate::VerificationArtifact::new(
            "application/vnd.dctx0.effective-program-assessment+json",
            serde_json::to_vec(&assessment).unwrap(),
        ));
    }
    if failure.starts_with("done_inspected_") && failure != "done_inspected_inventory_missing" {
        let mut inventory = run.provenance.implementation.clone();
        if failure == "done_inspected_inventory_changed" {
            inventory.insert("other".into(), "changed".into());
        }
        artifacts.push(crate::VerificationArtifact::new(
            "application/vnd.dctx0.captured-input-inventory+json",
            serde_json::to_vec(&inventory).unwrap(),
        ));
    }
    if failure.starts_with("done_inspected_") && failure != "done_inspected_missing" {
        use dctx0_tasks::{DoneTaskInspection, InspectedDoneBatch, TASK_INSPECTIONS_MEDIA_TYPE};
        let (path, hash) = run.provenance.implementation.iter().next().unwrap();
        let mut review = DoneTaskInspection {
            state_version: done.version,
            requirements_hash: candidate.requirements_hash.clone(),
            profile_bundle: pin.profile_bundles[&task].clone(),
            subjects: candidate.subjects.iter().cloned().collect(),
            types: candidate.types.iter().cloned().collect(),
            facts: candidate.facts.iter().cloned().collect(),
            fact_reviews: candidate.facts.iter().cloned().collect(),
            implementation_finding: "Checked implementation against the task".into(),
            profile_finding: "Checked the selected target profile".into(),
            adapter_binding_finding: None,
            sources: [path.clone()].into(),
            limitations: vec!["Manual inspection".into()],
        };
        let mut batch = InspectedDoneBatch {
            schema_version: 1,
            actor: "reviewer".into(),
            snapshot: snapshot.id.clone(),
            check_occurrence: check,
            task_pin: pin.clone(),
            sources: [(path.clone(), hash.clone())].into(),
            facts: candidate
                .facts
                .iter()
                .map(|fact| {
                    (
                        fact.clone(),
                        dctx0_tasks::SharedFactInspection {
                            fact: fact.clone(),
                            task_ids: [task].into(),
                            finding: "Reviewed this fact".into(),
                            sources: [path.clone()].into(),
                            limitations: vec!["Manual review".into()],
                        },
                    )
                })
                .collect(),
            tasks: Default::default(),
        };
        match failure {
            "done_inspected_source" => {
                batch.sources.insert(path.clone(), "stale".into());
            }
            "done_inspected_pin" => batch.task_pin.event_sequence += 1,
            "done_inspected_check" => batch.check_occurrence += 1,
            "done_inspected_snapshot" => batch.snapshot = "different".into(),
            "done_inspected_version" => review.state_version += 1,
            "done_inspected_requirements" => review.requirements_hash = "changed".into(),
            "done_inspected_profile" => review.profile_bundle = "changed".into(),
            "done_inspected_finding" => review.profile_finding.clear(),
            "done_inspected_fact" => review.fact_reviews.clear(),
            "done_inspected_subjects" => {
                review.subjects.clear();
            }
            "done_inspected_extra_task" => {
                batch.tasks.insert(task + 1_000_000, review.clone());
            }
            "done_inspected_orphan_fact" => {
                batch.facts.insert(
                    "orphan".into(),
                    dctx0_tasks::SharedFactInspection {
                        fact: "orphan".into(),
                        task_ids: [task + 1_000_000].into(),
                        finding: "A finding bound to another task".into(),
                        sources: [path.clone()].into(),
                        limitations: vec![],
                    },
                );
            }
            "done_inspected_unbound_source" => {
                review.sources = ["not-captured.go".into()].into();
            }
            "done_inspected_blank_author" => batch.actor = " ".into(),
            _ => (),
        }
        batch.tasks.insert(task, review);
        artifacts.push(crate::VerificationArtifact::new(
            TASK_INSPECTIONS_MEDIA_TYPE,
            serde_json::to_vec(&batch).unwrap(),
        ));
    }
    if failure == "done_inspected_imported" {
        run.origin = dctx0_core::VerificationRunOrigin::Imported;
    }
    if failure == "done_inspected_partial" {
        run.cases.pop();
        run.result = VerificationResult::Unknown {
            issue: VerificationIssue {
                code: "INCOMPLETE".into(),
                message: "Missing case".into(),
                span: None,
            },
        };
    }
    run.artifacts = artifacts.iter().map(|a| a.reference.clone()).collect();
    let id = store
        .record_verification_run("p", &run, &artifacts)
        .unwrap()
        .id;
    (store, run, manifest, id, pin, task, done.version)
}

#[test]
fn unlinked_incremental_proof_cannot_inherit_hybrid_task_authority() {
    let (store, mut run, manifest, id, _, _, _) = stored_hybrid("");
    run.artifacts.push(
        crate::verification::VerificationArtifact::new(
            dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE,
            b"{}".to_vec(),
        )
        .reference,
    );
    assert!(validate_open_obligations(&store.connection, &id, &run, &manifest).is_err());
}

#[test]
fn incremental_ancestor_requires_complete_bound_managed_cohort() {
    let (store, run, _, id, _, _, _) = stored_hybrid("");
    validate_incremental_root(&store.connection, &id, &run).unwrap();
    let (store, run, _, id, _, _, _) = stored_hybrid("audit_exit");
    assert!(validate_incremental_root(&store.connection, &id, &run).is_err());
    let (store, run, _, id, _, _, _) = stored_hybrid("receipt");
    assert!(validate_incremental_root(&store.connection, &id, &run).is_err());
}

#[test]
fn incremental_lineage_revalidates_parent_receipts_and_rejects_corruption() {
    use dctx0_verification::incremental::{self, ExternalInputReceipt, IncrementalProof};
    let (mut store, mut root, _, root_id, _, _, _) = stored_hybrid("");
    let mut artifacts: Vec<_> = root
        .artifacts
        .iter()
        .map(|a| store.verification_artifact(&root_id, &a.hash).unwrap())
        .collect();
    let plan: dctx0_verification::ProgramPlan = serde_json::from_slice(
        &artifacts
            .iter()
            .find(|a| a.reference.hash == root.provenance.plan_hash)
            .unwrap()
            .contents,
    )
    .unwrap();
    let receipt = ExternalInputReceipt {
        schema_version: 1,
        deterministic_units: incremental::program_units(&plan)
            .iter()
            .filter(|u| u.reusable)
            .map(|u| u.id.clone())
            .collect(),
        files: Default::default(),
        environment: Default::default(),
    };
    let units = incremental::receipt_units(&plan, &receipt).unwrap();
    let input = crate::VerificationArtifact::new(
        incremental::INCREMENTAL_INPUTS_MEDIA_TYPE,
        serde_json::to_vec(&receipt).unwrap(),
    );
    artifacts.push(input.clone());
    root.artifacts = artifacts.iter().map(|a| a.reference.clone()).collect();
    let parent = store
        .record_verification_run("p", &root, &artifacts)
        .unwrap();
    let binding =
        incremental::binding(&plan, root.provenance.clone(), input.reference.hash.clone()).unwrap();
    let selection = incremental::plan_reuse(&binding, &binding, &root, &units).unwrap();
    assert!(!selection.reused.is_empty());
    let proof = IncrementalProof {
        parent_run: parent.id.clone(),
        binding,
        units,
        plan: selection,
    };
    let artifact = crate::VerificationArtifact::new(
        incremental::INCREMENTAL_PROOF_MEDIA_TYPE,
        serde_json::to_vec(&proof).unwrap(),
    );
    let mut child = root.clone();
    child.started_at_unix_ns = "2".into();
    artifacts.push(artifact.clone());
    child.artifacts.push(artifact.reference.clone());
    let saved = store
        .record_verification_run("p", &child, &artifacts)
        .unwrap();
    validate_incremental_lineage(&store.connection, &saved.id, &child).unwrap();
    // A later descendant cannot promote a copied intermediate's fresh-only
    // cohort claim into incremental task authority.
    let mut descendant_proof = proof.clone();
    descendant_proof.parent_run = saved.id.clone();
    let descendant_artifact = crate::VerificationArtifact::new(
        incremental::INCREMENTAL_PROOF_MEDIA_TYPE,
        serde_json::to_vec(&descendant_proof).unwrap(),
    );
    let mut descendant_artifacts = artifacts.clone();
    descendant_artifacts.pop();
    descendant_artifacts.push(descendant_artifact);
    let mut descendant = child.clone();
    descendant.started_at_unix_ns = "3".into();
    descendant.artifacts = descendant_artifacts
        .iter()
        .map(|a| a.reference.clone())
        .collect();
    let descendant_saved = store
        .record_verification_run("p", &descendant, &descendant_artifacts)
        .unwrap();
    assert!(
        validate_incremental_lineage(&store.connection, &descendant_saved.id, &descendant).is_err()
    );
    let original: String = store
        .connection
        .query_row(
            "SELECT data FROM verification_runs WHERE id=?1",
            [&parent.id],
            |r| r.get(0),
        )
        .unwrap();
    store
        .connection
        .execute(
            "UPDATE verification_runs SET data=json_set(data,'$.origin','imported') WHERE id=?1",
            [&parent.id],
        )
        .unwrap();
    assert!(validate_incremental_lineage(&store.connection, &saved.id, &child).is_err());
    store
        .connection
        .execute(
            "UPDATE verification_runs SET data=?2 WHERE id=?1",
            rusqlite::params![parent.id, original],
        )
        .unwrap();
    store.connection.execute("UPDATE verification_runs SET data=json_set(data,'$.provenance.incomplete',json('[{\"code\":\"PARTIAL\",\"message\":\"partial\",\"span\":null}]')) WHERE id=?1", [&parent.id]).unwrap();
    assert!(validate_incremental_lineage(&store.connection, &saved.id, &child).is_err());
    store
        .connection
        .execute(
            "UPDATE verification_runs SET data=?2 WHERE id=?1",
            rusqlite::params![parent.id, original],
        )
        .unwrap();
    store.connection.execute("UPDATE verification_artifacts SET bytes=x'7b7d',encoding='identity',raw_length=2 WHERE hash=?1", [&artifact.reference.hash]).unwrap();
    assert!(validate_incremental_lineage(&store.connection, &saved.id, &child).is_err());
}

#[test]
fn stored_hybrid_requires_bound_native_artifacts_and_full_manifest() {
    for failure in [
        "",
        "receipt",
        "missing_receipt",
        "phase",
        "phase_exit",
        "phase_supervision",
        "source",
        "replay",
        "evidence",
        "authority",
        "plan",
        "audit_exit",
        "audit_supervision",
        "duplicate_group",
        "source_pin",
        "target",
        "missing_audit",
        "missing_runtime",
        "missing_limits",
        "missing_supervision",
        "manifest",
        "inspection_valid",
        "inspection_missing",
        "inspection_no_attestation",
        "inspection_stale_source",
        "inspection_stale_plan",
        "inspection_absent",
        "inspection_duplicate",
        "inspection_forged_assessment",
    ] {
        let (mut store, run, mut manifest, id, pin, task, version) = stored_hybrid(failure);
        if failure == "manifest" {
            manifest.required_cases.pop_first();
        }
        let result = validate_open_obligations(&store.connection, &id, &run, &manifest);
        let valid = failure.is_empty() || failure == "inspection_valid";
        assert_eq!(result.is_ok(), valid, "{failure}: {result:?}");
        if failure != "manifest" {
            let before = store.task_session("p", None, None).unwrap();
            let (profiles, docs) = profile_catalog();
            let publication = store.publish_native_task_batch(
                "p",
                super::super::TaskDeliveryPublication {
                    actor: "verifier",
                    pin: &pin,
                    deliveries: &[],
                    profiles: (&profiles, &docs),
                    run_id: Some(&id),
                },
                &[(task, version)].into(),
            );
            assert_eq!(
                publication.is_ok(),
                valid,
                "publication {failure}: {publication:?}"
            );
            if valid {
                assert_eq!(
                    store.task_session("p", None, None).unwrap().states[&task].check,
                    dctx0_tasks::CheckStatus::Pass
                );
            } else {
                assert_eq!(
                    serde_json::to_vec(&before).unwrap(),
                    serde_json::to_vec(&store.task_session("p", None, None).unwrap()).unwrap()
                );
            }
        }
        let count: i64 = store
            .connection
            .query_row("SELECT count(*) FROM verification_task_links", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, i64::from(valid));
    }
}

#[test]
fn inspected_done_requires_current_task_sources_and_complete_hybrid() {
    for failure in [
        "done_inspected_valid",
        "done_inspected_prior_fake",
        "done_inspected_missing",
        "done_inspected_hybrid_missing",
        "done_inspected_source",
        "done_inspected_pin",
        "done_inspected_check",
        "done_inspected_snapshot",
        "done_inspected_version",
        "done_inspected_requirements",
        "done_inspected_profile",
        "done_inspected_finding",
        "done_inspected_fact",
        "done_inspected_subjects",
        "done_inspected_extra_task",
        "done_inspected_orphan_fact",
        "done_inspected_unbound_source",
        "done_inspected_blank_author",
        "done_inspected_imported",
        "done_inspected_partial",
        "done_inspected_inventory_missing",
        "done_inspected_inventory_changed",
        "done_inspected_claim",
        "done_inspected_reservation",
        "done_inspected_actor",
    ] {
        let (mut store, _, _, id, pin, task, version) = stored_hybrid(failure);
        let before = store.task_session("p", None, None).unwrap();
        let native_before = store.verification_run("p", &id).unwrap();
        let (profile, docs) = profile_catalog();
        let result = store.publish_inspected_native_task_batch(
            "p",
            super::super::TaskDeliveryPublication {
                actor: "reviewer",
                pin: &pin,
                deliveries: &[],
                profiles: (&profile, &docs),
                run_id: Some(&id),
            },
            &[(task, version)].into(),
        );
        assert_eq!(
            result.is_ok(),
            matches!(
                failure,
                "done_inspected_valid" | "done_inspected_prior_fake"
            ),
            "{failure}: {result:?}"
        );
        let after = store.task_session("p", None, None).unwrap();
        if result.is_ok() {
            assert_eq!(after.states[&task].check, dctx0_tasks::CheckStatus::Pass);
            assert_eq!(
                serde_json::to_vec(&before.states[&task].result).unwrap(),
                serde_json::to_vec(&after.states[&task].result).unwrap()
            );
            assert_eq!(before.states[&task].reviews, after.states[&task].reviews);
            if failure == "done_inspected_prior_fake" {
                let fake = &before.states[&task].evidence[0];
                let metadata = crate::storage::evidence_metadata(task, fake);
                assert!(
                    !store
                        .verification_inspected_task_linked(
                            "p",
                            &id,
                            task,
                            &pin.requirements[&task],
                            metadata["report_hash"].as_str().unwrap()
                        )
                        .unwrap()
                );
            }
            let evidence = after.states[&task].evidence.last().unwrap();
            let metadata = crate::storage::evidence_metadata(task, evidence);
            let hash = metadata["report_hash"].as_str().unwrap();
            assert!(
                store
                    .verification_inspected_task_linked(
                        "p",
                        &id,
                        task,
                        &pin.requirements[&task],
                        hash
                    )
                    .unwrap()
            );
            assert!(
                !store
                    .verification_inspected_task_linked(
                        "p",
                        &id,
                        task,
                        &pin.requirements[&task],
                        "forged-report-hash"
                    )
                    .unwrap()
            );
            super::super::read::event(
                &store.connection,
                "p",
                Some(task),
                "note",
                "Later note",
                Some(&after.states[&task]),
            )
            .unwrap();
            assert!(
                store
                    .verification_inspected_task_linked(
                        "p",
                        &id,
                        task,
                        &pin.requirements[&task],
                        hash
                    )
                    .unwrap()
            );
            assert!(
                after.states[&task]
                    .evidence
                    .last()
                    .unwrap()
                    .report
                    .contains("INSPECTED")
            );
        } else {
            assert_eq!(
                serde_json::to_vec(&before).unwrap(),
                serde_json::to_vec(&after).unwrap(),
                "{failure}"
            );
            assert_eq!(store.verification_run("p", &id).unwrap(), native_before);
            assert!(store.task_delivery_publication("p", &id).unwrap().is_none());
            let links: i64 = store
                .connection
                .query_row(
                    "SELECT count(*) FROM verification_task_links WHERE run_id=?1",
                    [&id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(links, 0, "{failure}");
        }
    }
}

#[test]
fn inspected_status_requires_native_check_event_not_only_generic_run_link() {
    let (store, _, _, id, pin, task, _) = stored_hybrid("done_inspected_valid");
    store.connection.execute("INSERT INTO verification_task_links(run_id,task_id,requirements_hash) VALUES(?1,?2,?3)", rusqlite::params![id, task, pin.requirements[&task]]).unwrap();
    assert!(
        !store
            .verification_inspected_task_linked(
                "p",
                &id,
                task,
                &pin.requirements[&task],
                "fake-report"
            )
            .unwrap()
    );
}

#[test]
fn inspected_batch_rolls_back_earlier_check_when_later_task_is_invalid() {
    let (mut store, _, _, id, pin, task, version) = stored_hybrid("done_inspected_valid");
    let before = store.task_session("p", None, None).unwrap();
    let (profile, docs) = profile_catalog();
    let result = store.publish_inspected_native_task_batch(
        "p",
        super::super::TaskDeliveryPublication {
            actor: "reviewer",
            pin: &pin,
            deliveries: &[],
            profiles: (&profile, &docs),
            run_id: Some(&id),
        },
        &[(task, version), (i64::MAX, 0)].into(),
    );
    assert!(result.is_err());
    assert_eq!(
        serde_json::to_vec(&before).unwrap(),
        serde_json::to_vec(&store.task_session("p", None, None).unwrap()).unwrap()
    );
}

#[test]
fn inspected_run_and_finding_artifact_survive_retention() {
    let (mut store, mut run, _, id, pin, task, version) = stored_hybrid("done_inspected_valid");
    let (profile, docs) = profile_catalog();
    store
        .publish_inspected_native_task_batch(
            "p",
            super::super::TaskDeliveryPublication {
                actor: "reviewer",
                pin: &pin,
                deliveries: &[],
                profiles: (&profile, &docs),
                run_id: Some(&id),
            },
            &[(task, version)].into(),
        )
        .unwrap();
    let artifact = run
        .artifacts
        .iter()
        .find(|a| a.media_type == dctx0_tasks::TASK_INSPECTIONS_MEDIA_TYPE)
        .unwrap()
        .clone();
    let contents = run
        .artifacts
        .iter()
        .map(|reference| store.verification_artifact(&id, &reference.hash).unwrap())
        .collect::<Vec<_>>();
    run.started_at_unix_ns = "5000".into();
    run.origin = VerificationRunOrigin::Imported;
    let later = store.record_verification_run("p", &run, &contents).unwrap();
    assert_ne!(later.id, id);
    store.verification_gc("p").unwrap();
    assert!(
        store
            .verification_run_summary("p", &id)
            .unwrap()
            .details_available
    );
    assert_eq!(
        store
            .verification_artifact(&id, &artifact.hash)
            .unwrap()
            .reference,
        artifact
    );
}

#[test]
fn inspected_native_rejects_corrupt_bound_audit_metadata_and_payload_atomically() {
    for failure in ["reference-size", "malformed-payload"] {
        let (mut store, mut run, _, id, pin, task, version) = stored_hybrid("done_inspected_valid");
        let reference = run
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.media_type == "application/vnd.dctx0.native-audit+json")
            .unwrap();
        if failure == "reference-size" {
            // Keep the actual content hash intact while corrupting the persisted run's reference length.
            reference.bytes += 1;
        } else {
            // Valid content address and run linkage cannot make malformed typed audit JSON valid.
            let changed =
                crate::VerificationArtifact::new(&reference.media_type, b"{broken".to_vec());
            store
                .connection
                .execute(
                    "INSERT INTO verification_artifacts(hash,media_type,bytes) VALUES(?1,?2,?3)",
                    rusqlite::params![
                        changed.reference.hash,
                        changed.reference.media_type,
                        changed.contents
                    ],
                )
                .unwrap();
            store
                .connection
                .execute(
                    "INSERT INTO verification_run_artifacts(run_id,artifact_hash) VALUES(?1,?2)",
                    rusqlite::params![id, changed.reference.hash],
                )
                .unwrap();
            *reference = changed.reference;
        }
        // Model a corrupt legacy row without manufacturing a successful native run.
        store
            .connection
            .execute(
                "UPDATE verification_runs SET data=?1,corpus_hash=NULL WHERE id=?2",
                rusqlite::params![serde_json::to_string(&run).unwrap(), id],
            )
            .unwrap();
        store
            .connection
            .execute(
                "DELETE FROM verification_case_projections WHERE run_id=?1",
                [&id],
            )
            .unwrap();
        let before = store.task_session("p", None, None).unwrap();
        let original = store.verification_run("p", &id).unwrap();
        let (profile, docs) = profile_catalog();
        let error = store
            .publish_inspected_native_task_batch(
                "p",
                super::super::TaskDeliveryPublication {
                    actor: "reviewer",
                    pin: &pin,
                    deliveries: &[],
                    profiles: (&profile, &docs),
                    run_id: Some(&id),
                },
                &[(task, version)].into(),
            )
            .unwrap_err();
        let expected = if failure == "reference-size" {
            "Native proof artifact integrity"
        } else {
            "Invalid native proof artifact"
        };
        assert!(error.to_string().contains(expected), "{failure}: {error}");
        assert_eq!(
            serde_json::to_vec(&before).unwrap(),
            serde_json::to_vec(&store.task_session("p", None, None).unwrap()).unwrap()
        );
        assert_eq!(store.verification_run("p", &id).unwrap(), original);
        assert!(store.task_delivery_publication("p", &id).unwrap().is_none());
        let links: i64 = store
            .connection
            .query_row(
                "SELECT count(*) FROM verification_task_links WHERE run_id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(links, 0);
    }
}
