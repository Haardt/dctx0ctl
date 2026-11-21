use dctx0_core::*;
use dctx0_verification::{application_plan::*, application_session::*, *};
use std::collections::{BTreeMap, BTreeSet};

fn fixture(
    text: &str,
    subject: &str,
    arguments: Vec<VerificationValue>,
) -> (WorkspaceIr, ApplicationTestPlan) {
    let c = dctx0_language::compile(&[Source {
        file: "native.dctx0".into(),
        text: text.into(),
    }]);
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    let ir = c.ir.unwrap();
    let input = OperationCaseInput {
        id: "native-case".into(),
        invocation: Invocation {
            subject: subject.into(),
            arguments,
        },
        setup: vec![],
        argument_bindings: vec![],
        mocks: vec![],
    };
    let fixtures = ProjectionFixtures {
        operations: BTreeMap::from([(subject.into(), vec![input])]),
        ..Default::default()
    };
    let program = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: BTreeSet::from([subject.into()]),
            tasks: Default::default(),
        },
        &fixtures,
        Default::default(),
    );
    let manifest = case_manifest(&program);
    let resources = |namespace: &str| GroupResources {
        namespace: namespace.into(),
        timeout_seconds: 60,
        max_message_bytes: 1_048_576,
    };
    let legacy = ExecutionGroup {
        id: "legacy".into(),
        backend: TestBackend::Adapter,
        target: "go".into(),
        selection: BTreeSet::from(["adapter".into()]),
        area: TestArea {
            cases: manifest.required_cases.clone(),
            ..Default::default()
        },
        stateless_decoders: Default::default(),
        resources: resources("legacy"),
    };
    let classified = application_test_plan(
        &program,
        &ApplicationTestConfiguration {
            schema_version: 2,
            groups: vec![legacy.clone()],
        },
        &[],
    )
    .unwrap();
    let form = classified.case_forms["native-case"];
    let cases = BTreeSet::from(["native-case".into()]);
    let groups = vec![
        ExecutionGroup {
            id: "native".into(),
            backend: TestBackend::ApplicationTests,
            target: "go".into(),
            selection: BTreeSet::from(["TestOperation".into()]),
            area: TestArea {
                cases: cases.clone(),
                ..Default::default()
            },
            stateless_decoders: Default::default(),
            resources: resources("native"),
        },
        ExecutionGroup {
            area: TestArea {
                cases: manifest
                    .required_cases
                    .difference(&cases)
                    .cloned()
                    .collect(),
                ..Default::default()
            },
            ..legacy
        },
    ];
    let plan = application_test_plan(
        &program,
        &ApplicationTestConfiguration {
            schema_version: 2,
            groups,
        },
        &[TestRegistration {
            group: "native".into(),
            test: "TestOperation".into(),
            form,
            cases,
        }],
    )
    .unwrap();
    (ir, plan)
}
fn begin(session: &mut ApplicationSession<'_>) -> (u64, ApplicationDirective) {
    session.handle(ApplicationRequest::Register {
        test: "TestOperation".into(),
    });
    let ApplicationReply::Case {
        context, directive, ..
    } = session.handle(ApplicationRequest::Next {
        test: "TestOperation".into(),
    })
    else {
        panic!("case required")
    };
    (context, directive)
}
fn observe(
    session: &mut ApplicationSession<'_>,
    context: u64,
    observation: ApplicationObservation,
) -> ApplicationDirective {
    let ApplicationReply::Action { directive, .. } = session.handle(ApplicationRequest::Observe {
        context,
        observation,
    }) else {
        panic!("action required")
    };
    directive
}
fn invoked(invocation: Invocation, value: VerificationValue) -> ApplicationObservation {
    ApplicationObservation::Invoked {
        invocation,
        observation: Observation::Returned { value },
        calls: Some(vec![]),
        events: Some(vec![]),
        resources: Some(vec![]),
    }
}
fn int(n: &str) -> VerificationValue {
    VerificationValue::Integer(n.into())
}
fn row() -> VerificationValue {
    VerificationValue::Record(BTreeMap::from([
        ("id".into(), int("1")),
        ("amount".into(), int("2")),
    ]))
}

#[test]
fn state_change_requires_ordered_complete_scoped_actual_snapshots_and_detects_missing_writes() {
    let (ir, plan) = fixture(
        "workspace W { entity Item { id:Int\namount:Int\nkey id } port Store { operation apply(item:Item) -> Unit { state upserts Item from item } } }",
        "W.Store.apply",
        vec![row()],
    );
    for mutation in 0..5 {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, directive) = begin(&mut session);
        let ApplicationDirective::ReadState {
            invocation,
            phase: StatePhase::Before,
            entities,
        } = directive
        else {
            panic!("before state required")
        };
        assert_eq!(entities, vec!["W.Item"]);
        let action = observe(
            &mut session,
            context,
            ApplicationObservation::State {
                entities: BTreeMap::from([("W.Item".into(), vec![])]),
                phase: if mutation == 1 {
                    StatePhase::After
                } else {
                    StatePhase::Before
                },
                invocation: invocation.clone(),
                complete: mutation != 2,
            },
        );
        if matches!(mutation, 1 | 2) {
            assert!(matches!(
                action,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Fail { .. } | VerificationResult::Unknown { .. }
                }
            ));
            continue;
        }
        assert!(matches!(
            action,
            ApplicationDirective::Invoke {
                phase: ApplicationInvocationPhase::Production,
                ..
            }
        ));
        let action = observe(
            &mut session,
            context,
            invoked(invocation.clone(), VerificationValue::Unit),
        );
        assert!(matches!(
            action,
            ApplicationDirective::ReadState {
                phase: StatePhase::After,
                ..
            }
        ));
        let action = observe(
            &mut session,
            context,
            ApplicationObservation::State {
                entities: if mutation == 3 {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(
                        "W.Item".into(),
                        if mutation == 4 { vec![] } else { vec![row()] },
                    )])
                },
                phase: StatePhase::After,
                invocation,
                complete: true,
            },
        );
        if mutation == 0 {
            assert!(matches!(
                action,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Pass
                }
            ));
            session.handle(ApplicationRequest::CloseCase {
                context,
                cleanup_error: None,
            });
            session.handle(ApplicationRequest::CloseTest {
                test: "TestOperation".into(),
            });
            session.handle(ApplicationRequest::Finish);
            assert_eq!(session.report().result, VerificationResult::Pass);
            assert_eq!(session.report().cases[0].states.len(), 2)
        } else {
            assert!(matches!(
                action,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Fail { .. }
                }
            ))
        }
    }
}

#[test]
fn pure_oracle_continuations_use_separate_tapes_and_latch_mutated_inputs_or_results() {
    let (ir, plan) = fixture(
        "workspace W { surface Rules { operation expected(n:Int) -> Int { pure } operation apply(n:Int) -> Int { pure\nensures result == expected(n) } } }",
        "W.Rules.apply",
        vec![int("4")],
    );
    for mutation in 0..3 {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, directive) = begin(&mut session);
        let ApplicationDirective::Invoke {
            mut invocation,
            phase: ApplicationInvocationPhase::Production,
        } = directive
        else {
            panic!("invoke required")
        };
        if mutation == 1 {
            invocation.arguments = vec![int("5")]
        }
        let action = observe(&mut session, context, invoked(invocation, int("4")));
        if mutation == 1 {
            assert!(matches!(
                action,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Fail { .. }
                }
            ));
            continue;
        }
        let ApplicationDirective::Invoke {
            invocation,
            phase: ApplicationInvocationPhase::Oracle,
        } = action
        else {
            panic!("local oracle required: {action:?}")
        };
        assert_eq!(invocation.subject, "W.Rules.expected");
        let result = observe(
            &mut session,
            context,
            invoked(invocation, int(if mutation == 2 { "5" } else { "4" })),
        );
        assert!(matches!(
            (mutation, result),
            (
                0,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Pass
                }
            ) | (
                2,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Fail { .. }
                }
            )
        ));
        assert_eq!(session.report().cases[0].observations.len(), 2);
    }
}

#[test]
fn assertion_calls_are_ordered_repeated_native_invocations_with_actual_contract_checks() {
    let compiled=dctx0_language::compile(&[Source {file:"assertion.dctx0".into(),text:"workspace W { surface Rules { operation next(n:Int) -> Int { pure } } invariant Repeat { assert Rules.next(4) == Rules.next(4) } }".into()}]);
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
            subjects: BTreeSet::from(["W.Repeat".into()]),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let all_cases = case_manifest(&program).required_cases;
    let cases = program
        .properties
        .iter()
        .flat_map(|p| &p.assertions)
        .flat_map(|a| &a.cases)
        .map(|c| c.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(cases.len(), 1);
    let mut config = ApplicationTestConfiguration {
        schema_version: 2,
        groups: vec![ExecutionGroup {
            id: "native".into(),
            backend: TestBackend::ApplicationTests,
            target: "go".into(),
            selection: BTreeSet::from(["TestOperation".into()]),
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
    config.groups.push(ExecutionGroup {
        id: "legacy".into(),
        backend: TestBackend::Adapter,
        target: "go".into(),
        selection: BTreeSet::from(["adapter".into()]),
        area: TestArea {
            cases: all_cases.difference(&cases).cloned().collect(),
            ..Default::default()
        },
        stateless_decoders: Default::default(),
        resources: GroupResources {
            namespace: "legacy".into(),
            timeout_seconds: 60,
            max_message_bytes: 1_048_576,
        },
    });
    let plan = application_test_plan(
        &program,
        &config,
        &[TestRegistration {
            group: "native".into(),
            test: "TestOperation".into(),
            form: TestForm::Assertion,
            cases,
        }],
    )
    .unwrap();
    for mutation in 0..4 {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, action) = begin(&mut session);
        let ApplicationDirective::Invoke {
            invocation,
            phase: ApplicationInvocationPhase::Oracle,
        } = action
        else {
            panic!("oracle required")
        };
        let action = observe(&mut session, context, invoked(invocation.clone(), int("4")));
        let ApplicationDirective::Invoke {
            invocation: second,
            phase: ApplicationInvocationPhase::Oracle,
        } = action
        else {
            panic!("second actual observation required")
        };
        assert_eq!(invocation, second);
        let value = match mutation {
            1 => int("5"),
            2 => VerificationValue::Boolean(true),
            _ => int("4"),
        };
        let mut observation = invoked(second, value);
        if mutation == 3
            && let ApplicationObservation::Invoked { events, .. } = &mut observation
        {
            *events = Some(vec![VerificationEvent {
                subject: "unexpected".into(),
                payload: VerificationValue::Unit,
            }]);
        }
        let directive = observe(&mut session, context, observation);
        assert!(matches!(
            (mutation, directive),
            (
                0,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Pass
                }
            ) | (
                1..=3,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Fail { .. }
                }
            )
        ));
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
        session.handle(ApplicationRequest::CloseTest {
            test: "TestOperation".into(),
        });
        session.handle(ApplicationRequest::Finish);
        assert_eq!(
            matches!(session.report().result, VerificationResult::Pass),
            mutation == 0
        );
    }
}

fn verdict_code(directive: ApplicationDirective) -> String {
    match directive {
        ApplicationDirective::Verdict {
            result: VerificationResult::Fail { issue } | VerificationResult::Unknown { issue },
        } => issue.code,
        other => panic!("Expected rejected verdict, got {other:?}"),
    }
}

#[test]
fn pure_operation_oracle_rejects_effects_wrong_contract_binding_and_out_of_order_observations() {
    let (ir, plan) = fixture(
        "workspace W { surface Rules { operation expected(n:Int) -> Int { pure } operation apply(n:Int) -> Int { pure\nensures result == expected(n) } } }",
        "W.Rules.apply",
        vec![int("4")],
    );
    for mutation in 0..6 {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, directive) = begin(&mut session);
        let ApplicationDirective::Invoke { invocation, .. } = directive else {
            panic!("production required")
        };
        let oracle = observe(&mut session, context, invoked(invocation, int("4")));
        let ApplicationDirective::Invoke {
            mut invocation,
            phase: ApplicationInvocationPhase::Oracle,
        } = oracle
        else {
            panic!("oracle required")
        };
        let mut observation = invoked(invocation.clone(), int("4"));
        let expected = match mutation {
            0 => {
                observation = invoked(invocation.clone(), VerificationValue::Boolean(true));
                "NATIVE_RETURN_CONTRACT"
            }
            1 => {
                if let ApplicationObservation::Invoked { resources, .. } = &mut observation {
                    *resources = Some(vec![ResourceObservation {
                        operation: Some(invocation.subject.clone()),
                        resource: "W.Database".into(),
                        access: ResourceAccess::Write,
                    }]);
                }
                "PURE_CLAUSE_EFFECT"
            }
            2 => {
                if let ApplicationObservation::Invoked { events, .. } = &mut observation {
                    *events = Some(vec![VerificationEvent {
                        subject: "W.Changed".into(),
                        payload: VerificationValue::Unit,
                    }]);
                }
                "PURE_CLAUSE_EFFECT"
            }
            3 => {
                invocation.arguments = vec![int("5")];
                observation = invoked(invocation, int("4"));
                "APPLICATION_INPUT_CHANGED"
            }
            4 => {
                observation = ApplicationObservation::BindingError {
                    message: "native projector failed".into(),
                };
                "APPLICATION_BINDING_ERROR"
            }
            _ => {
                observation = ApplicationObservation::State {
                    entities: BTreeMap::new(),
                    phase: StatePhase::After,
                    invocation,
                    complete: true,
                };
                "APPLICATION_OBSERVATION_ORDER"
            }
        };
        assert_eq!(
            verdict_code(observe(&mut session, context, observation)),
            expected
        );
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
        session.handle(ApplicationRequest::CloseTest {
            test: "TestOperation".into(),
        });
        session.handle(ApplicationRequest::Finish);
        assert!(!matches!(session.report().result, VerificationResult::Pass));
    }
}

#[test]
fn operation_collaborators_cannot_run_before_state_observation_or_without_prepared_behavior() {
    let (ir, plan) = fixture(
        "workspace W { entity Item { id:Int\namount:Int\nkey id } port Store { operation apply(item:Item) -> Unit { state upserts Item from item } } }",
        "W.Store.apply",
        vec![row()],
    );
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    let (context, directive) = begin(&mut session);
    let ApplicationDirective::ReadState { invocation, .. } = directive else {
        panic!("before required")
    };
    assert!(
        matches!(session.handle(ApplicationRequest::Collaborator{context,invocation:invocation.clone()}),ApplicationReply::Rejected{issue} if issue.code=="APPLICATION_COLLABORATOR_CONTEXT")
    );
    observe(
        &mut session,
        context,
        ApplicationObservation::State {
            entities: BTreeMap::from([("W.Item".into(), vec![])]),
            phase: StatePhase::Before,
            invocation: invocation.clone(),
            complete: true,
        },
    );
    assert!(
        matches!(session.handle(ApplicationRequest::Collaborator{context,invocation:Invocation{subject:"W.Store.apply".into(),arguments:vec![row()]}}),ApplicationReply::CollaboratorResult{observation:Observation::Unavailable{reason},..} if reason=="MOCK_PLAN_REQUIRED")
    );
    assert_eq!(
        verdict_code(observe(
            &mut session,
            context,
            invoked(invocation, VerificationValue::Unit)
        )),
        "MOCK_PLAN_REQUIRED"
    );
}

#[test]
fn standalone_operation_mocks_require_actual_requests_and_matching_observed_trace() {
    let (ir, mut plan) = fixture(
        "workspace W { surface Rules { operation expected(n:Int) -> Int { pure } operation apply(n:Int) -> Int { pure\nensures calls expected(n) exactly 1\nensures result == n } } }",
        "W.Rules.apply",
        vec![int("4")],
    );
    let call = Invocation {
        subject: "W.Rules.expected".into(),
        arguments: vec![int("4")],
    };
    let input = plan
        .program
        .operations
        .iter_mut()
        .find(|operation| operation.operation.id == "W.Rules.apply")
        .unwrap()
        .cases
        .iter_mut()
        .find(|case| case.id == "native-case")
        .unwrap();
    input.mocks.push(OperationMock {
        invocation: call.clone(),
        outcome: Observation::Returned { value: int("4") },
    });
    for missing_trace in [false, true] {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        assert!(matches!(
            session.handle(ApplicationRequest::Register {
                test: "TestOperation".into()
            }),
            ApplicationReply::Registered
        ));
        let ApplicationReply::Case {
            context,
            directive: ApplicationDirective::Invoke { invocation, .. },
            mocks,
            ..
        } = session.handle(ApplicationRequest::Next {
            test: "TestOperation".into(),
        })
        else {
            panic!("Production required")
        };
        assert_eq!(mocks, vec!["W.Rules.expected"]);
        let ApplicationReply::CollaboratorResult { observation, .. } =
            session.handle(ApplicationRequest::Collaborator {
                context,
                invocation: call.clone(),
            })
        else {
            panic!("Prepared collaborator required")
        };
        let mut response = invoked(invocation, int("4"));
        if let ApplicationObservation::Invoked { calls, .. } = &mut response {
            *calls = if missing_trace {
                None
            } else {
                Some(vec![InvocationObservation {
                    invocation: call.clone(),
                    observation,
                }])
            };
        }
        let directive = observe(&mut session, context, response);
        if missing_trace {
            assert_eq!(verdict_code(directive), "MOCK_CALL_OBSERVER_REQUIRED");
        } else {
            assert!(
                matches!(
                    directive,
                    ApplicationDirective::Verdict {
                        result: VerificationResult::Pass
                    }
                ),
                "{directive:?}"
            );
        }
    }
}

#[test]
fn pure_production_operation_cannot_hide_resource_writes_or_emitted_events() {
    let (ir, plan) = fixture(
        "workspace W { surface Rules { operation apply(n:Int) -> Int { pure\nensures result == n } } }",
        "W.Rules.apply",
        vec![int("4")],
    );
    for write_resource in [false, true] {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, directive) = begin(&mut session);
        let ApplicationDirective::Invoke { invocation, .. } = directive else {
            panic!("Production required")
        };
        let mut observation = invoked(invocation.clone(), int("4"));
        if let ApplicationObservation::Invoked {
            events, resources, ..
        } = &mut observation
        {
            if write_resource {
                *resources = Some(vec![ResourceObservation {
                    operation: Some(invocation.subject),
                    resource: "W.Database".into(),
                    access: ResourceAccess::Write,
                }]);
            } else {
                *events = Some(vec![VerificationEvent {
                    subject: "W.Changed".into(),
                    payload: VerificationValue::Unit,
                }]);
            }
        }
        assert_eq!(
            verdict_code(observe(&mut session, context, observation)),
            "PURE_OPERATION_EFFECT"
        );
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
        session.handle(ApplicationRequest::CloseTest {
            test: "TestOperation".into(),
        });
        session.handle(ApplicationRequest::Finish);
        assert!(!matches!(session.report().result, VerificationResult::Pass));
    }
}
