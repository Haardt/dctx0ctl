use dctx0_core::*;
use dctx0_verification::{application_plan::*, application_session::*, *};

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
            stateless_decoders: Default::default(),
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

#[test]
fn reused_cases_skip_central_iteration_but_still_require_native_registration() {
    let (ir, plan) = fixture();
    let id = plan.manifest.required_cases.iter().next().unwrap().clone();
    let record = VerificationCaseRecord {
        id: id.clone(),
        subject: "W.Year".into(),
        kind: VerificationCaseKind::Contract,
        bindings: Default::default(),
        setup: vec![],
        arguments: vec![],
        observations: vec![],
        events: None,
        resources: None,
        states: vec![],
        result: VerificationResult::Pass,
        elapsed_us: 0,
    };
    let mut session = ApplicationSession::new(&ir, &plan, "native")
        .unwrap()
        .with_reused_cases(vec![record.clone()])
        .unwrap();
    assert!(matches!(
        session.handle(ApplicationRequest::CloseTest {
            test: "TestYear".into()
        }),
        ApplicationReply::Rejected { .. }
    ));
    assert!(
        ApplicationSession::new(&ir, &plan, "native")
            .unwrap()
            .with_reused_cases(vec![record.clone(), record])
            .is_err()
    );
    let mut session = ApplicationSession::new(&ir, &plan, "native")
        .unwrap()
        .with_reused_cases(vec![VerificationCaseRecord {
            id,
            subject: "W.Year".into(),
            kind: VerificationCaseKind::Contract,
            bindings: Default::default(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        }])
        .unwrap();
    register(&mut session);
    for _ in 1..plan.manifest.required_cases.len() {
        let (context, raw) = next(&mut session);
        let outcome = actual_decoder(&raw);
        session.handle(ApplicationRequest::Observe {
            context,
            observation: ApplicationObservation::Decoded { raw, outcome },
        });
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
    }
    assert!(matches!(
        session.handle(ApplicationRequest::Next {
            test: "TestYear".into()
        }),
        ApplicationReply::NoMoreCases
    ));
    assert!(matches!(
        session.handle(ApplicationRequest::CloseTest {
            test: "TestYear".into()
        }),
        ApplicationReply::TestClosed
    ));
    session.handle(ApplicationRequest::Finish);
    assert_eq!(session.report().result, VerificationResult::Pass);
}

#[test]
fn central_iteration_assesses_every_case_and_requires_test_and_group_finalization() {
    let (ir, plan) = fixture();
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    register(&mut session);
    for _ in &plan.manifest.required_cases {
        let (context, raw) = next(&mut session);
        let outcome = actual_decoder(&raw);
        let assessed = session.handle(ApplicationRequest::Observe {
            context,
            observation: ApplicationObservation::Decoded { raw, outcome },
        });
        assert!(matches!(
            assessed,
            ApplicationReply::Action {
                directive: ApplicationDirective::Verdict {
                    result: VerificationResult::Pass
                },
                ..
            }
        ));
        assert!(matches!(
            session.handle(ApplicationRequest::CloseCase {
                context,
                cleanup_error: None
            }),
            ApplicationReply::CaseClosed {
                result: VerificationResult::Pass,
                ..
            }
        ));
    }
    assert!(matches!(
        session.handle(ApplicationRequest::Next {
            test: "TestYear".into()
        }),
        ApplicationReply::NoMoreCases
    ));
    assert!(!matches!(session.report().result, VerificationResult::Pass));
    assert!(matches!(
        session.handle(ApplicationRequest::CloseTest {
            test: "TestYear".into()
        }),
        ApplicationReply::TestClosed
    ));
    let ApplicationReply::Finished { summary } = session.handle(ApplicationRequest::Finish) else {
        panic!("Expected summary");
    };
    assert_eq!(summary.status, ApplicationAssessmentStatus::Pass);
    assert_eq!(summary.completed_cases, plan.manifest.required_cases.len());
    assert_eq!(summary.missing_cases, 0);
    assert_eq!(session.report().result, VerificationResult::Pass);
}

#[test]
fn caught_contract_failure_stays_failed_without_close_or_after_repeated_observations() {
    let (ir, plan) = fixture();
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    register(&mut session);
    let (context, raw) = next(&mut session);
    let failure = session.handle(ApplicationRequest::Observe {
        context,
        observation: ApplicationObservation::Decoded {
            raw: raw.clone(),
            outcome: DecoderOutcome::Accepted {
                value: VerificationValue::Integer("0".into()),
            },
        },
    });
    assert!(matches!(
        failure,
        ApplicationReply::Action {
            directive: ApplicationDirective::Verdict {
                result: VerificationResult::Fail { .. }
            },
            ..
        }
    ));
    assert!(matches!(
        session.handle(ApplicationRequest::Observe {
            context,
            observation: ApplicationObservation::Decoded {
                raw,
                outcome: DecoderOutcome::Rejected {
                    reason: "caught".into()
                },
            }
        }),
        ApplicationReply::Rejected { .. }
    ));
    assert!(matches!(
        session.report().result,
        VerificationResult::Fail { .. }
    ));
    let ApplicationReply::Finished { summary } = session.handle(ApplicationRequest::Finish) else {
        panic!("Expected summary");
    };
    assert_eq!(summary.status, ApplicationAssessmentStatus::Fail);
    assert!(summary.missing_cases > 0);
    assert_eq!(session.report().cases.len(), 1);
    assert!(matches!(
        session.report().cases[0].result,
        VerificationResult::Fail { .. }
    ));
}

#[test]
fn input_substitution_codec_failures_and_cleanup_failures_cannot_pass_rejection_cases() {
    let (ir, plan) = fixture();
    for mutation in 0..4 {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        register(&mut session);
        let (context, raw) = next(&mut session);
        let observation = match mutation {
            0 => ApplicationObservation::Decoded {
                raw: b"different".to_vec(),
                outcome: DecoderOutcome::Rejected {
                    reason: "codec".into(),
                },
            },
            1 => ApplicationObservation::Decoded {
                raw,
                outcome: DecoderOutcome::BindingError {
                    message: "test projector failed".into(),
                },
            },
            2 => ApplicationObservation::Decoded {
                raw,
                outcome: DecoderOutcome::Exception {
                    message: "production panicked".into(),
                },
            },
            _ => ApplicationObservation::Decoded {
                outcome: actual_decoder(&raw),
                raw,
            },
        };
        session.handle(ApplicationRequest::Observe {
            context,
            observation,
        });
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: (mutation == 3).then(|| "close failed".into()),
        });
        assert_ne!(session.report().cases[0].result, VerificationResult::Pass);
        assert_ne!(session.report().result, VerificationResult::Pass);
    }
}

#[test]
fn missing_duplicate_early_closed_and_post_finalization_requests_remain_incomplete() {
    let (ir, plan) = fixture();
    assert!(ApplicationSession::new(&ir, &plan, "missing").is_err());
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    assert!(matches!(
        session.handle(ApplicationRequest::Next {
            test: "TestYear".into()
        }),
        ApplicationReply::Rejected { .. }
    ));
    register(&mut session);
    assert!(matches!(
        session.handle(ApplicationRequest::Register {
            test: "TestYear".into()
        }),
        ApplicationReply::Rejected { .. }
    ));
    let (context, _) = next(&mut session);
    assert!(matches!(
        session.handle(ApplicationRequest::Next {
            test: "TestYear".into()
        }),
        ApplicationReply::Rejected { .. }
    ));
    assert!(matches!(
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None
        }),
        ApplicationReply::CaseClosed {
            result: VerificationResult::Unknown { .. },
            ..
        }
    ));
    assert!(matches!(
        session.handle(ApplicationRequest::CloseTest {
            test: "TestYear".into()
        }),
        ApplicationReply::Rejected { .. }
    ));
    session.handle(ApplicationRequest::Finish);
    assert!(matches!(
        session.handle(ApplicationRequest::Register {
            test: "TestYear".into()
        }),
        ApplicationReply::Rejected { .. }
    ));
    assert_ne!(session.report().result, VerificationResult::Pass);
}

#[test]
fn constructor_boundary_pins_exact_values_and_rejects_raw_decoder_observations() {
    let (ir, plan) = fixture();
    for swapped in [false, true] {
        let mut session = ApplicationSession::new_with_boundary(
            &ir,
            &plan,
            "native",
            ApplicationValueBoundary::ValueConstructor,
        )
        .unwrap();
        register(&mut session);
        let ApplicationReply::Case {
            context,
            directive: ApplicationDirective::Construct { value, .. },
            ..
        } = session.handle(ApplicationRequest::Next {
            test: "TestYear".into(),
        })
        else {
            panic!("expected native value constructor")
        };
        let observation = if swapped {
            ApplicationObservation::Constructed {
                input: if value == VerificationValue::Unit {
                    VerificationValue::Boolean(false)
                } else {
                    VerificationValue::Unit
                },
                outcome: DecoderOutcome::Rejected {
                    reason: "wrong input".into(),
                },
            }
        } else {
            ApplicationObservation::Decoded {
                raw: vec![],
                outcome: DecoderOutcome::Rejected {
                    reason: "wrong boundary".into(),
                },
            }
        };
        assert!(matches!(
            session.handle(ApplicationRequest::Observe {
                context,
                observation
            }),
            ApplicationReply::Action {
                directive: ApplicationDirective::Verdict {
                    result: VerificationResult::Fail { .. }
                },
                ..
            }
        ));
    }
    let mut session = ApplicationSession::new_with_boundary(
        &ir,
        &plan,
        "native",
        ApplicationValueBoundary::ValueConstructor,
    )
    .unwrap();
    register(&mut session);
    loop {
        let (context, input) = match session.handle(ApplicationRequest::Next {
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
        let accepted = matches!(&input, VerificationValue::Integer(n) if n == "3" || n == "4");
        let outcome = if accepted {
            DecoderOutcome::Accepted {
                value: input.clone(),
            }
        } else {
            DecoderOutcome::Rejected {
                reason: "out of range".into(),
            }
        };
        session.handle(ApplicationRequest::Observe {
            context,
            observation: ApplicationObservation::Constructed { input, outcome },
        });
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
    }
    session.handle(ApplicationRequest::CloseTest {
        test: "TestYear".into(),
    });
    session.handle(ApplicationRequest::Finish);
    assert_eq!(session.report().result, VerificationResult::Pass);
    session.record_transport_fault("late malformed traffic");
    assert!(matches!(
        session.report().result,
        VerificationResult::Unknown { .. }
    ));
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

#[test]
fn stored_native_application_proof_rejects_partial_collections_and_substituted_evidence() {
    let (proof, run) = native_proof_fixture();
    assert!(proof.validates(&run));
    let mut corruptions = Vec::new();
    let mut corrupt = proof.clone();
    corrupt.assessment.cases[0]
        .bindings
        .insert("swapped-input".into(), VerificationValue::Unit);
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.assessment.assessment_version = "unsupported-assessor".into();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.assessment.finalized = false;
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.assessment.missing_tests.insert("TestYear".into());
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.assessment.missing_cases.insert("missing".into());
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.assessment.cases.pop();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt
        .assessment
        .cases
        .push(corrupt.assessment.cases[0].clone());
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.source_inputs.clear();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.materialized_inputs.clear();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.build_inputs.clear();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.toolchain_inputs.clear();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.receipt_hash = "other-receipt".into();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.phases[0].exit_code = Some(1);
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.phases[0].supervised = false;
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.phases.pop();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.phases.push(corrupt.phases[0].clone());
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt.plan.configuration.groups[0].target = "typescript".into();
    corruptions.push(corrupt);
    let mut corrupt = proof.clone();
    corrupt
        .plan
        .case_groups
        .values_mut()
        .for_each(|group| *group = "other".into());
    corruptions.push(corrupt);
    for (index, corrupt) in corruptions.iter().enumerate() {
        assert!(
            !corrupt.validates(&run),
            "Corruption {index} must prevent task delivery"
        );
    }
    let mut other_run = run.clone();
    other_run.provenance.snapshot = "other-snapshot".into();
    assert!(!proof.validates(&other_run));
    let mut other_run = run.clone();
    other_run.origin = VerificationRunOrigin::Imported;
    assert!(!proof.validates(&other_run));
    let mut other_run = run.clone();
    other_run.provenance.protocol_version = 1;
    assert!(!proof.validates(&other_run));
    let mut other_run = run.clone();
    other_run.provenance.check_occurrence += 1;
    assert!(!proof.validates(&other_run));
    let mut other_run = run.clone();
    other_run
        .provenance
        .implementation
        .insert("helper.go".into(), "changed".into());
    assert!(!proof.validates(&other_run));
}

#[test]
fn foreign_ir_closures_and_type_bindings_are_rejected_before_case_execution() {
    let (ir, plan) = fixture();
    for mutation in 0..2 {
        let mut altered = plan.clone();
        if mutation == 0 {
            altered.program.closure.insert("Foreign.Requirement".into());
        } else {
            altered.program.contracts[0].ty.symbol_id = None;
        }
        let issues = ApplicationSession::new(&ir, &altered, "native")
            .err()
            .expect("Mismatched IR must be rejected");
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "APPLICATION_IR_SCOPE"),
            "{issues:?}"
        );
    }
}

#[test]
fn test_finalization_cannot_hide_an_active_production_decoder_case() {
    let (ir, plan) = fixture();
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    register(&mut session);
    let (_context, _raw) = next(&mut session);
    let reply = session.handle(ApplicationRequest::CloseTest {
        test: "TestYear".into(),
    });
    assert!(matches!(reply, ApplicationReply::Rejected { .. }));
    session.handle(ApplicationRequest::Finish);
    assert_ne!(session.report().result, VerificationResult::Pass);
    assert!(
        session
            .report()
            .issues
            .iter()
            .any(|issue| issue.code == "APPLICATION_TEST_INCOMPLETE")
    );
}

#[test]
fn binding_failure_is_not_production_rejection_and_unknown_contexts_remain_faults() {
    let (ir, plan) = fixture();
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    register(&mut session);
    let (context, _) = next(&mut session);
    assert!(matches!(
        session.handle(ApplicationRequest::Observe {
            context,
            observation: ApplicationObservation::BindingError {
                message: "Fixture projection could not invoke the decoder".into(),
            },
        }),
        ApplicationReply::Action {
            directive: ApplicationDirective::Verdict {
                result: VerificationResult::Unknown { issue }
            }, ..
        } if issue.code == "APPLICATION_BINDING_ERROR"
    ));
    session.handle(ApplicationRequest::CloseCase {
        context,
        cleanup_error: None,
    });
    assert!(matches!(
        session.handle(ApplicationRequest::CloseCase { context, cleanup_error: None }),
        ApplicationReply::Rejected { issue } if issue.code == "APPLICATION_CASE_CONTEXT"
    ));
    session.handle(ApplicationRequest::Finish);
    assert_ne!(session.report().result, VerificationResult::Pass);
}

#[test]
fn unsupported_decoder_wire_input_remains_unknown_without_production_invocation() {
    let (ir, mut plan) = fixture();
    // An imported/custom plan can contain a value requiring an explicit codec.
    // The default JSON boundary must expose that limitation before invoking production.
    plan.program.contracts[0].corpus.values[0] = VerificationValue::Failure {
        declaration: "W.ProductionFailure".into(),
        payload: None,
    };
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    register(&mut session);
    let mut saw_unsupported = false;
    for _ in &plan.manifest.required_cases {
        let ApplicationReply::Case {
            context, directive, ..
        } = session.handle(ApplicationRequest::Next {
            test: "TestYear".into(),
        })
        else {
            panic!("Expected central case")
        };
        match directive {
            ApplicationDirective::Decode { raw, .. } => {
                session.handle(ApplicationRequest::Observe {
                    context,
                    observation: ApplicationObservation::Decoded {
                        outcome: actual_decoder(&raw),
                        raw,
                    },
                });
            }
            ApplicationDirective::Verdict {
                result: VerificationResult::Unknown { issue },
            } => {
                assert_eq!(issue.code, "APPLICATION_JSON_CODEC");
                saw_unsupported = true;
            }
            other => panic!("Unexpected directive: {other:?}"),
        }
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
    }
    assert!(saw_unsupported);
    session.handle(ApplicationRequest::CloseTest {
        test: "TestYear".into(),
    });
    session.handle(ApplicationRequest::Finish);
    assert_ne!(session.report().result, VerificationResult::Pass);
    assert!(session.report().cases.iter().any(|case| matches!(
        case.result,
        VerificationResult::Unknown { .. }
    ) && case.observations.is_empty()));
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

#[test]
fn assigned_native_group_keeps_full_program_but_cannot_satisfy_standalone_task_proof() {
    let (proof, run, _) = assigned_native_proof_fixture();
    let manifest = proof.plan.manifest.clone();
    assert!(proof.validates_assigned_group(&run));
    assert!(!proof.validates(&run));
    let mut changed = run.clone();
    changed.origin = VerificationRunOrigin::Imported;
    assert!(!proof.validates_assigned_group(&changed));
    let mut changed = proof.clone();
    changed.assessment.finalized = false;
    assert!(!changed.validates_assigned_group(&run));
    let mut changed = proof.clone();
    changed
        .assessment
        .cases
        .push(changed.assessment.cases[0].clone());
    assert!(!changed.validates_assigned_group(&run));
    let mut changed = run.clone();
    changed.required_cases = manifest.required_cases;
    assert!(!proof.validates_assigned_group(&changed));
    let mut changed = proof.clone();
    changed.assessment.group = "legacy".into();
    assert!(!changed.validates_assigned_group(&run));
}

#[test]
fn hybrid_proof_binds_both_fresh_groups_and_keeps_native_observations_exact() {
    use dctx0_verification::hybrid_evidence::HybridExecutionProof;
    let (application, mut application_run, legacy) = assigned_native_proof_fixture();
    application_run.started_at_unix_ns = "100".into();
    let mut run = application_run.clone();
    run.started_at_unix_ns = "1".into();
    run.elapsed_us = 4;
    run.provenance.protocol_version = 1;
    run.provenance.adapter_hash = "legacy-adapter".into();
    run.provenance.harness_hash = "legacy-harness".into();
    run.required_cases = application.plan.manifest.required_cases.clone();
    run.cases.push(legacy);
    let proof = HybridExecutionProof {
        schema_version: 1,
        application,
        application_run,
        adapter_group: "legacy".into(),
        adapter_started_at_unix_ns: "2000".into(),
        adapter_elapsed_us: 1,
    };
    assert!(proof.validates(&run));
    assert!(!proof.application.validates(&proof.application_run));
    let mut bad = run.clone();
    bad.origin = VerificationRunOrigin::Imported;
    assert!(!proof.validates(&bad));
    let mut bad = run.clone();
    bad.cases.pop();
    assert!(!proof.validates(&bad));
    let mut bad = run.clone();
    bad.cases[0]
        .bindings
        .insert("changed-actual-input".into(), VerificationValue::Unit);
    assert!(!proof.validates(&bad));
    let mut bad = proof.clone();
    bad.application_run.started_at_unix_ns = "0".into();
    assert!(!bad.validates(&run));
    let mut bad = proof.clone();
    bad.adapter_started_at_unix_ns = "5000".into();
    assert!(!bad.validates(&run));
    let mut bad = proof.clone();
    bad.application_run.provenance.configuration_hash = "other-configuration".into();
    assert!(!bad.validates(&run));
    let mut bad = proof.clone();
    bad.application_run.provenance.implementation.clear();
    assert!(!bad.validates(&run));
    let mut bad = proof.clone();
    bad.adapter_group = bad.application.assessment.group.clone();
    assert!(!bad.validates(&run));
    let mut bad = run.clone();
    bad.started_at_unix_ns = "invalid-time".into();
    assert!(!proof.validates(&bad));
    let mut bad = run.clone();
    bad.provenance.incomplete.push(VerificationIssue {
        code: "HYBRID_COHORT_REQUIRED".into(),
        message: "diagnostic saved group".into(),
        span: None,
    });
    assert!(!proof.validates(&bad));
}

#[test]
fn generated_preparation_passes_actual_ids_through_nested_native_calls() {
    let compiled = dctx0_language::compile(&[Source { file: "flow.dctx0".into(), text: "workspace W { surface Api { operation create() -> String\noperation link(id:String) -> String\noperation read(id:String) -> String } }".into() }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let mut program = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: ["W.Api.read".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let operation = &mut program.operations[0];
    operation.inputs.bindings.truncate(1);
    let bindings = operation.inputs.bindings[0].clone();
    let template = bindings["id"].clone();
    operation.generated_preparations = vec![GeneratedPreparation {
        bindings,
        setup: vec![
            Invocation {
                subject: "W.Api.create".into(),
                arguments: vec![],
            },
            Invocation {
                subject: "W.Api.link".into(),
                arguments: vec![template],
            },
        ],
        argument_bindings: vec![
            OperationArgumentBinding {
                setup: 0,
                source_path: vec![],
                target_setup: Some(1),
                parameter: "id".into(),
                target_path: vec![],
            },
            OperationArgumentBinding {
                setup: 1,
                source_path: vec![],
                target_setup: None,
                parameter: "id".into(),
                target_path: vec![],
            },
        ],
    }];
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
            stateless_decoders: Default::default(),
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
            form: TestForm::Operation,
            cases,
        }],
    )
    .unwrap();
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    register(&mut session);
    let ApplicationReply::Case {
        context, directive, ..
    } = session.handle(ApplicationRequest::Next {
        test: "TestYear".into(),
    })
    else {
        panic!("expected operation")
    };
    let mut directive = directive;
    for (subject, expected, returned) in [
        ("W.Api.create", None, "connection-7"),
        ("W.Api.link", Some("connection-7"), "client-9"),
        ("W.Api.read", Some("client-9"), "done"),
    ] {
        let ApplicationDirective::Invoke { invocation, .. } = directive else {
            panic!("expected invocation")
        };
        assert_eq!(invocation.subject, subject);
        assert_eq!(
            invocation.arguments,
            expected
                .map(|s| vec![VerificationValue::String(s.into())])
                .unwrap_or_default()
        );
        let ApplicationReply::Action {
            directive: next, ..
        } = session.handle(ApplicationRequest::Observe {
            context,
            observation: ApplicationObservation::Invoked {
                invocation,
                observation: Observation::Returned {
                    value: VerificationValue::String(returned.into()),
                },
                calls: None,
                events: None,
                resources: None,
            },
        })
        else {
            panic!("expected next directive")
        };
        directive = next;
    }
    assert!(matches!(
        directive,
        ApplicationDirective::Verdict {
            result: VerificationResult::Pass
        }
    ));
    session.handle(ApplicationRequest::CloseCase {
        context,
        cleanup_error: None,
    });
    session.handle(ApplicationRequest::Next {
        test: "TestYear".into(),
    });
    session.handle(ApplicationRequest::CloseTest {
        test: "TestYear".into(),
    });
    session.handle(ApplicationRequest::Finish);
    let report = session.report();
    assert_eq!(report.result, VerificationResult::Pass);
    assert_eq!(
        report.cases[0].arguments,
        vec![vec![VerificationValue::String("client-9".into())]]
    );
}

#[test]
fn decoder_packets_assess_each_case_and_reject_out_of_order_submissions() {
    let (ir, mut plan) = fixture();
    let test = "TestYear";
    let mut unbound = ApplicationSession::new(&ir, &plan, "native").unwrap();
    assert!(matches!(
        unbound.handle(ApplicationRequest::RegisterDecoder { test: test.into() }),
        ApplicationReply::DecoderRegistered { batch: false, .. }
    ));
    assert!(matches!(
        unbound.handle(ApplicationRequest::NextDecoderBatch {
            test: test.into(),
            max_message_bytes: 1_048_576,
        }),
        ApplicationReply::Rejected { .. }
    ));

    plan.configuration.groups[0]
        .stateless_decoders
        .insert(test.into());
    for (reordered, tampered) in [(false, false), (true, false), (false, true)] {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        assert!(matches!(
            session.handle(ApplicationRequest::RegisterDecoder { test: test.into() }),
            ApplicationReply::DecoderRegistered { batch: true, .. }
        ));
        let ApplicationReply::DecoderBatch { cases } =
            session.handle(ApplicationRequest::NextDecoderBatch {
                test: test.into(),
                max_message_bytes: 1_048_576,
            })
        else {
            panic!("expected decoded inputs in a packet");
        };
        assert!(cases.len() > 1);
        let mut cases: Vec<_> = cases
            .into_iter()
            .map(|case| {
                let ApplicationDirective::Decode { raw, .. } = case.directive else {
                    panic!("expected a raw decoder case")
                };
                DecoderBatchObservation {
                    context: case.context,
                    observation: Some(ApplicationObservation::Decoded {
                        outcome: actual_decoder(&raw),
                        raw,
                    }),
                }
            })
            .collect();
        if reordered {
            cases.swap(0, 1);
        }
        if tampered
            && let Some(ApplicationObservation::Decoded { raw, .. }) = &mut cases[0].observation
        {
            raw.push(b' ');
        }
        let reply = session.handle(ApplicationRequest::CompleteDecoderBatch {
            test: test.into(),
            cases,
        });
        if reordered {
            assert!(matches!(reply, ApplicationReply::Rejected { .. }));
            let report = session.report();
            assert_eq!(report.missing_cases, report.required_cases);
            assert!(
                report
                    .cases
                    .iter()
                    .all(|case| matches!(case.result, VerificationResult::Unknown { .. }))
            );
        } else {
            let ApplicationReply::DecoderBatchClosed { cases } = reply else {
                panic!("expected central packet verdicts");
            };
            assert_eq!(
                cases[0].status,
                if tampered {
                    ApplicationAssessmentStatus::Fail
                } else {
                    ApplicationAssessmentStatus::Pass
                }
            );
            assert!(
                cases[1..]
                    .iter()
                    .all(|case| case.status == ApplicationAssessmentStatus::Pass)
            );
            assert_eq!(session.report().cases.len(), cases.len());
        }
    }
}
