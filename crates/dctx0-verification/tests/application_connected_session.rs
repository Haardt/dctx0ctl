use dctx0_core::*;
use dctx0_verification::{application_plan::*, application_session::*, *};
use std::collections::{BTreeMap, BTreeSet};

fn fixture(
    text: &str,
    form: TestForm,
    fixtures: ProjectionFixtures,
) -> (WorkspaceIr, ApplicationTestPlan) {
    let compiled = dctx0_language::compile(&[Source {
        file: "connected.dctx0".into(),
        text: text.into(),
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
            subjects: ["W".into()].into(),
            tasks: Default::default(),
        },
        &fixtures,
        Default::default(),
    );
    let manifest = case_manifest(&program);
    let group = |id: &str, backend, cases: BTreeSet<String>| ExecutionGroup {
        id: id.into(),
        backend,
        target: "go".into(),
        selection: [if id == "native" {
            "TestConnected".into()
        } else {
            "adapter".into()
        }]
        .into(),
        area: TestArea {
            cases,
            ..Default::default()
        },
        resources: GroupResources {
            namespace: id.into(),
            timeout_seconds: 30,
            max_message_bytes: 1_048_576,
        },
    };
    let config = ApplicationTestConfiguration {
        schema_version: 2,
        groups: vec![group(
            "legacy",
            TestBackend::Adapter,
            manifest.required_cases.clone(),
        )],
    };
    let classified = application_test_plan(&program, &config, &[]).unwrap();
    let cases = classified
        .case_forms
        .iter()
        .filter(|(_, f)| **f == form)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    assert!(!cases.is_empty());
    let plan = application_test_plan(
        &program,
        &ApplicationTestConfiguration {
            schema_version: 2,
            groups: vec![
                group("native", TestBackend::ApplicationTests, cases.clone()),
                group(
                    "legacy",
                    TestBackend::Adapter,
                    manifest
                        .required_cases
                        .difference(&cases)
                        .cloned()
                        .collect(),
                ),
            ],
        },
        &[TestRegistration {
            group: "native".into(),
            test: "TestConnected".into(),
            form,
            cases,
        }],
    )
    .unwrap();
    (ir, plan)
}
fn next(session: &mut ApplicationSession<'_>) -> (u64, ApplicationDirective, Vec<String>) {
    assert!(matches!(
        session.handle(ApplicationRequest::Register {
            test: "TestConnected".into()
        }),
        ApplicationReply::Registered
    ));
    let ApplicationReply::Case {
        context,
        directive,
        mocks,
        ..
    } = session.handle(ApplicationRequest::Next {
        test: "TestConnected".into(),
    })
    else {
        panic!("Expected case")
    };
    (context, directive, mocks)
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
        panic!("Expected action")
    };
    directive
}
fn returned(value: VerificationValue) -> Observation {
    Observation::Returned { value }
}

#[test]
fn scenario_uses_actual_collaborator_requests_and_latches_wrong_inputs() {
    let text = r#"workspace W {
port Store { operation load(input: Int) -> Int }
surface Api { operation read(input: Int) -> Int }
example "consumer" {
 given Store.load(input) returns 5
 when result = Api.read(input)
 expect Success
 expect result == 5
}
}"#;
    let fixtures = ProjectionFixtures {
        examples: BTreeMap::from([(
            "W.consumer".into(),
            BTreeMap::from([("input".into(), VerificationValue::Integer("1".into()))]),
        )]),
        ..Default::default()
    };
    let (ir, plan) = fixture(text, TestForm::Scenario, fixtures);
    for corrupt in [false, true] {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, directive, mocks) = next(&mut session);
        assert_eq!(mocks, vec!["W.Store.load"]);
        let ApplicationDirective::Invoke { mut invocation, .. } = directive else {
            panic!("Expected production step")
        };
        let collaborator = Invocation {
            subject: "W.Store.load".into(),
            arguments: invocation.arguments.clone(),
        };
        let ApplicationReply::CollaboratorResult { observation, .. } =
            session.handle(ApplicationRequest::Collaborator {
                context,
                invocation: collaborator.clone(),
            })
        else {
            panic!("Expected actual reply")
        };
        assert_eq!(
            observation,
            returned(VerificationValue::Integer("5".into()))
        );
        if corrupt {
            invocation.arguments = vec![VerificationValue::Integer("2".into())];
        }
        let directive = observe(
            &mut session,
            context,
            ApplicationObservation::Invoked {
                invocation,
                observation: observation.clone(),
                calls: Some(vec![InvocationObservation {
                    invocation: collaborator,
                    observation,
                }]),
                events: Some(vec![]),
                resources: Some(vec![]),
            },
        );
        assert!(
            matches!(
                directive,
                ApplicationDirective::Verdict {
                    result: VerificationResult::Pass
                }
            ) != corrupt
        );
        assert!(matches!(
            session.handle(ApplicationRequest::Collaborator {
                context,
                invocation: Invocation {
                    subject: "W.Store.load".into(),
                    arguments: vec![]
                }
            }),
            ApplicationReply::Rejected { .. }
        ));
    }
}
#[test]
fn lifecycle_requires_actual_before_and_after_states_and_complete_snapshots() {
    let text = "workspace W { type Status = Open | Closed\nentity Item { id:String\nstatus:Status\nkey id }\nsurface Api { operation finish(id:String) -> Int }\nlifecycle Flow for Item using status { state Open initial\nstate Closed terminal\ntransition Api.finish from Open to Closed when true } }";
    let (ir, plan) = fixture(text, TestForm::Lifecycle, Default::default());
    let id = plan.program.lifecycles[0].paths[0].id.clone();
    for (closed, complete, expected) in [
        (true, true, "PASS"),
        (false, true, "FAIL"),
        (true, false, "UNKNOWN"),
    ] {
        let input = LifecycleCaseInput {
            keys: vec![VerificationValue::String("key".into())],
            setup: vec![],
            arguments: vec![vec![VerificationValue::String("key".into())]],
        };
        let mut session = ApplicationSession::new(&ir, &plan, "native")
            .unwrap()
            .with_lifecycle_inputs([(id.clone(), input)].into());
        let (context, mut directive, _) = next(&mut session);
        for _ in 0..10 {
            directive = match directive {
                ApplicationDirective::ReadState {
                    entities,
                    phase,
                    invocation,
                } => {
                    let status = if phase == StatePhase::After && closed {
                        "Closed"
                    } else {
                        "Open"
                    };
                    let row = VerificationValue::Record(
                        [
                            ("id".into(), VerificationValue::String("key".into())),
                            (
                                "status".into(),
                                VerificationValue::Variant {
                                    declaration: "W.Status".into(),
                                    name: status.into(),
                                    payload: None,
                                },
                            ),
                        ]
                        .into(),
                    );
                    observe(
                        &mut session,
                        context,
                        ApplicationObservation::State {
                            entities: [(entities[0].clone(), vec![row])].into(),
                            phase,
                            invocation,
                            complete,
                        },
                    )
                }
                ApplicationDirective::Invoke { invocation, .. } => observe(
                    &mut session,
                    context,
                    ApplicationObservation::Invoked {
                        invocation,
                        observation: returned(VerificationValue::Integer("1".into())),
                        calls: Some(vec![]),
                        events: Some(vec![]),
                        resources: Some(vec![]),
                    },
                ),
                ApplicationDirective::Verdict { ref result } => {
                    assert_eq!(serde_json::to_value(result).unwrap()["status"], expected);
                    break;
                }
                _ => panic!("Unexpected lifecycle directive"),
            };
        }
        assert!(matches!(directive, ApplicationDirective::Verdict { .. }));
    }
}

fn rejected_code(directive: ApplicationDirective) -> String {
    match directive {
        ApplicationDirective::Verdict {
            result: VerificationResult::Fail { issue } | VerificationResult::Unknown { issue },
        } => issue.code,
        other => panic!("Expected incomplete/failed verdict, got {other:?}"),
    }
}

#[test]
fn assertion_requires_pending_invocation_and_distinguishes_native_effects_from_binding_errors() {
    let (ir, plan) = fixture(
        "workspace W { surface Rules { operation next(n:Int) -> Int { pure } } invariant Repeat { assert Rules.next(4) == 4 } }",
        TestForm::Assertion,
        Default::default(),
    );
    for mutation in 0..5 {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, directive, _) = next(&mut session);
        let ApplicationDirective::Invoke { mut invocation, .. } = directive else {
            panic!("oracle required")
        };
        let mut observation = ApplicationObservation::Invoked {
            invocation: invocation.clone(),
            observation: returned(VerificationValue::Integer("4".into())),
            calls: Some(vec![]),
            events: Some(vec![]),
            resources: Some(vec![]),
        };
        let expected = match mutation {
            0 => {
                if let ApplicationObservation::Invoked { resources, .. } = &mut observation {
                    *resources = Some(vec![ResourceObservation {
                        operation: None,
                        resource: "W.Database".into(),
                        access: ResourceAccess::Write,
                    }]);
                }
                "PURE_CLAUSE_EFFECT"
            }
            1 => {
                invocation.arguments = vec![VerificationValue::Integer("5".into())];
                if let ApplicationObservation::Invoked {
                    invocation: actual, ..
                } = &mut observation
                {
                    *actual = invocation;
                }
                "APPLICATION_INPUT_CHANGED"
            }
            2 => {
                observation = ApplicationObservation::BindingError {
                    message: "actual return cannot be projected".into(),
                };
                "APPLICATION_BINDING_ERROR"
            }
            3 => {
                observation = ApplicationObservation::State {
                    entities: BTreeMap::new(),
                    phase: StatePhase::Before,
                    invocation,
                    complete: true,
                };
                "APPLICATION_INPUT_CHANGED"
            }
            _ => {
                if let ApplicationObservation::Invoked { observation, .. } = &mut observation {
                    *observation = returned(VerificationValue::Boolean(true));
                }
                "NATIVE_RETURN_CONTRACT"
            }
        };
        assert_eq!(
            rejected_code(observe(&mut session, context, observation)),
            expected
        );
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
        session.handle(ApplicationRequest::CloseTest {
            test: "TestConnected".into(),
        });
        session.handle(ApplicationRequest::Finish);
        assert!(!matches!(session.report().result, VerificationResult::Pass));
    }
}

#[test]
fn scenario_requires_actual_mock_requests_observed_calls_and_event_observer() {
    let text = r#"workspace W {
port Store { operation load(input:Int) -> Int }
surface Api { operation read(input:Int) -> Int }
example "consumer" { given Store.load(input) returns 5
when result = Api.read(input)
expect Success
expect result == 5 }
}"#;
    let fixtures = ProjectionFixtures {
        examples: BTreeMap::from([(
            "W.consumer".into(),
            BTreeMap::from([("input".into(), VerificationValue::Integer("1".into()))]),
        )]),
        ..Default::default()
    };
    let (ir, plan) = fixture(text, TestForm::Scenario, fixtures);
    for mutation in 0..6 {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, directive, _) = next(&mut session);
        let ApplicationDirective::Invoke { invocation, .. } = directive else {
            panic!("production required")
        };
        let collaborator = Invocation {
            subject: "W.Store.load".into(),
            arguments: invocation.arguments.clone(),
        };
        let reply = returned(VerificationValue::Integer("5".into()));
        if mutation != 1 {
            assert!(matches!(
                session.handle(ApplicationRequest::Collaborator {
                    context,
                    invocation: collaborator.clone()
                }),
                ApplicationReply::CollaboratorResult { .. }
            ));
        }
        let mut observation = ApplicationObservation::Invoked {
            invocation: invocation.clone(),
            observation: reply.clone(),
            calls: Some(vec![InvocationObservation {
                invocation: collaborator,
                observation: reply,
            }]),
            events: Some(vec![]),
            resources: Some(vec![]),
        };
        let expected = match mutation {
            0 => {
                if let ApplicationObservation::Invoked { calls, .. } = &mut observation {
                    *calls = None;
                }
                "MOCK_CALL_OBSERVER_REQUIRED"
            }
            1 => "MOCK_OBSERVATION_MISMATCH",
            2 => {
                if let ApplicationObservation::Invoked { calls, .. } = &mut observation {
                    *calls = Some(vec![]);
                }
                "MOCK_OBSERVATION_MISMATCH"
            }
            3 => {
                if let ApplicationObservation::Invoked { events, .. } = &mut observation {
                    *events = None;
                }
                "APPLICATION_EVENT_OBSERVER"
            }
            4 => {
                observation = ApplicationObservation::BindingError {
                    message: "fixture lost context".into(),
                };
                "APPLICATION_BINDING_ERROR"
            }
            _ => {
                observation = ApplicationObservation::State {
                    entities: BTreeMap::new(),
                    phase: StatePhase::Before,
                    invocation,
                    complete: true,
                };
                "APPLICATION_OBSERVATION_ORDER"
            }
        };
        assert_eq!(
            rejected_code(observe(&mut session, context, observation)),
            expected
        );
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
        session.handle(ApplicationRequest::CloseTest {
            test: "TestConnected".into(),
        });
        session.handle(ApplicationRequest::Finish);
        assert!(!matches!(session.report().result, VerificationResult::Pass));
    }
}

const LIFECYCLE_WITH_SETUP: &str = "workspace W { type Status = Open | Closed\nentity Item { id:String\nstatus:Status\nkey id }\nsurface Api { operation prepare(id:String) -> Unit\noperation finish(id:String) -> Int }\nlifecycle Flow for Item using status { state Open initial\nstate Closed terminal\ntransition Api.finish from Open to Closed when true } }";
fn lifecycle_native_input(setup: bool) -> LifecycleCaseInput {
    LifecycleCaseInput {
        keys: vec![VerificationValue::String("key".into())],
        setup: if setup {
            vec![Invocation {
                subject: "W.Api.prepare".into(),
                arguments: vec![VerificationValue::String("key".into())],
            }]
        } else {
            vec![]
        },
        arguments: vec![vec![VerificationValue::String("key".into())]],
    }
}
fn lifecycle_entity(status: &str) -> VerificationValue {
    VerificationValue::Record(
        [
            ("id".into(), VerificationValue::String("key".into())),
            (
                "status".into(),
                VerificationValue::Variant {
                    declaration: "W.Status".into(),
                    name: status.into(),
                    payload: None,
                },
            ),
        ]
        .into(),
    )
}
#[test]
fn lifecycle_missing_inputs_and_invalid_keys_fail_before_production_and_snapshots() {
    let (ir, plan) = fixture(
        LIFECYCLE_WITH_SETUP,
        TestForm::Lifecycle,
        Default::default(),
    );
    let id = plan.program.lifecycles[0].paths[0].id.clone();
    let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
    let (_, directive, _) = next(&mut session);
    assert_eq!(rejected_code(directive), "LIFECYCLE_INPUT_REQUIRED");
    let mut input = lifecycle_native_input(false);
    input.keys = vec![VerificationValue::Boolean(true)];
    let mut session = ApplicationSession::new(&ir, &plan, "native")
        .unwrap()
        .with_lifecycle_inputs([(id, input)].into());
    let (_, directive, _) = next(&mut session);
    assert!(matches!(
        directive,
        ApplicationDirective::Verdict {
            result: VerificationResult::Fail { .. } | VerificationResult::Unknown { .. }
        }
    ));
}

#[test]
fn lifecycle_setup_is_actual_ordered_work_and_faults_do_not_establish_prerequisites() {
    let (ir, plan) = fixture(
        LIFECYCLE_WITH_SETUP,
        TestForm::Lifecycle,
        Default::default(),
    );
    let id = plan.program.lifecycles[0].paths[0].id.clone();
    for mutation in 0..3 {
        let mut session = ApplicationSession::new(&ir, &plan, "native")
            .unwrap()
            .with_lifecycle_inputs([(id.clone(), lifecycle_native_input(true))].into());
        let (context, directive, _) = next(&mut session);
        let ApplicationDirective::Invoke {
            invocation,
            phase: ApplicationInvocationPhase::Setup,
        } = directive
        else {
            panic!("setup invocation required")
        };
        assert_eq!(invocation.subject, "W.Api.prepare");
        let directive = observe(
            &mut session,
            context,
            if mutation == 2 {
                ApplicationObservation::BindingError {
                    message: "fixture preparation codec failed".into(),
                }
            } else {
                ApplicationObservation::Invoked {
                    invocation: invocation.clone(),
                    observation: if mutation == 1 {
                        Observation::Threw {
                            message: "database unavailable".into(),
                        }
                    } else {
                        returned(VerificationValue::Unit)
                    },
                    calls: Some(vec![]),
                    events: Some(vec![]),
                    resources: Some(vec![]),
                }
            },
        );
        if mutation > 0 {
            if mutation == 2 {
                assert_eq!(rejected_code(directive), "APPLICATION_BINDING_ERROR");
            } else {
                assert!(matches!(
                    directive,
                    ApplicationDirective::Verdict {
                        result: VerificationResult::Unknown { .. }
                            | VerificationResult::Fail { .. }
                    }
                ));
            }
            assert!(session.report().cases[0].setup.is_empty());
        } else {
            assert_eq!(session.report().cases[0].setup, vec![invocation]);
            assert!(matches!(
                directive,
                ApplicationDirective::ReadState {
                    phase: StatePhase::Before,
                    ..
                }
            ));
        }
    }
}

#[test]
fn lifecycle_snapshot_phase_scope_invocation_and_actual_initial_state_are_enforced() {
    let (ir, plan) = fixture(
        LIFECYCLE_WITH_SETUP,
        TestForm::Lifecycle,
        Default::default(),
    );
    let id = plan.program.lifecycles[0].paths[0].id.clone();
    for mutation in 0..6 {
        let mut session = ApplicationSession::new(&ir, &plan, "native")
            .unwrap()
            .with_lifecycle_inputs([(id.clone(), lifecycle_native_input(false))].into());
        let (context, directive, _) = next(&mut session);
        let ApplicationDirective::ReadState { mut invocation, .. } = directive else {
            panic!("before required")
        };
        if mutation == 2 {
            invocation.arguments = vec![VerificationValue::String("foreign".into())];
        }
        let observation = if mutation == 4 {
            ApplicationObservation::BindingError {
                message: "state projector failed".into(),
            }
        } else {
            ApplicationObservation::State {
                entities: if mutation == 1 {
                    BTreeMap::new()
                } else {
                    [(
                        "W.Item".into(),
                        vec![lifecycle_entity(if mutation == 3 {
                            "Closed"
                        } else {
                            "Open"
                        })],
                    )]
                    .into()
                },
                phase: if mutation == 0 {
                    StatePhase::After
                } else {
                    StatePhase::Before
                },
                invocation: invocation.clone(),
                complete: true,
            }
        };
        let mut directive = observe(&mut session, context, observation);
        if mutation == 5 {
            let ApplicationDirective::Invoke { invocation, .. } = directive else {
                panic!("production required")
            };
            directive = observe(
                &mut session,
                context,
                ApplicationObservation::Invoked {
                    invocation,
                    observation: returned(VerificationValue::Integer("1".into())),
                    calls: Some(vec![]),
                    events: Some(vec![]),
                    resources: Some(vec![]),
                },
            );
            let ApplicationDirective::ReadState { invocation, .. } = directive else {
                panic!("after required")
            };
            directive = observe(
                &mut session,
                context,
                ApplicationObservation::State {
                    entities: [("W.Item".into(), vec![lifecycle_entity("Closed")])].into(),
                    phase: StatePhase::Before,
                    invocation,
                    complete: true,
                },
            );
        }
        let code = rejected_code(directive);
        if matches!(mutation, 0..=2 | 5) {
            assert_eq!(code, "APPLICATION_STATE_SCOPE");
        }
        if mutation == 4 {
            assert_eq!(code, "APPLICATION_BINDING_ERROR");
        }
        session.handle(ApplicationRequest::CloseCase {
            context,
            cleanup_error: None,
        });
        session.handle(ApplicationRequest::CloseTest {
            test: "TestConnected".into(),
        });
        session.handle(ApplicationRequest::Finish);
        assert!(!matches!(session.report().result, VerificationResult::Pass));
    }
}

#[test]
fn connected_scenario_accumulates_actual_resources_but_preserves_missing_observer_prefix() {
    let text = "workspace W { surface Api { operation read(input:Int) -> Int } example \"two steps\" { when first = Api.read(1)\nexpect Success\nwhen second = Api.read(2)\nexpect Success\nexpect first == 1\nexpect second == 2 } }";
    let (ir, plan) = fixture(text, TestForm::Scenario, Default::default());
    for missing_first in [false, true] {
        let mut session = ApplicationSession::new(&ir, &plan, "native").unwrap();
        let (context, mut directive, _) = next(&mut session);
        for index in 0..2 {
            let ApplicationDirective::Invoke { invocation, .. } = directive else {
                panic!("ordered step required")
            };
            let resources = if missing_first && index == 0 {
                None
            } else {
                Some(vec![ResourceObservation {
                    operation: Some(invocation.subject.clone()),
                    resource: format!("actual-{index}"),
                    access: ResourceAccess::Read,
                }])
            };
            directive = observe(
                &mut session,
                context,
                ApplicationObservation::Invoked {
                    observation: returned(invocation.arguments[0].clone()),
                    invocation,
                    calls: Some(vec![]),
                    events: Some(vec![]),
                    resources,
                },
            );
        }
        assert!(matches!(
            directive,
            ApplicationDirective::Verdict {
                result: VerificationResult::Pass
            }
        ));
        let record = &session.report().cases[0];
        if missing_first {
            assert_eq!(record.resources, None);
        } else {
            assert_eq!(record.resources.as_ref().unwrap().len(), 2);
        }
    }
}
