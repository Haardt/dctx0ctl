use dctx0_core::*;
use dctx0_verification::*;
fn fixture() -> (WorkspaceIr, OperationMock) {
    let compiled = dctx0_language::compile(&[Source {
        file: "mocks.dctx0".into(),
        text: "workspace W { port Store { operation load(id:Int) -> Int } }".into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    (
        compiled.ir.unwrap(),
        OperationMock {
            invocation: Invocation {
                subject: "W.Store.load".into(),
                arguments: vec![VerificationValue::Integer("1".into())],
            },
            outcome: Observation::Returned {
                value: VerificationValue::Integer("2".into()),
            },
        },
    )
}
#[test]
fn prepared_behavior_records_real_requests_without_inventing_exact_once_counts() {
    let (ir, mock) = fixture();
    let mut session =
        OperationMockSession::new(&ir, std::slice::from_ref(&mock), &Default::default());
    assert!(session.issues.is_empty());
    assert!(session.observed.is_empty());
    assert_eq!(session.respond(&mock.invocation), mock.outcome);
    assert_eq!(session.respond(&mock.invocation), mock.outcome);
    assert_eq!(session.observed.len(), 2);
    let mut unexpected = mock.invocation.clone();
    unexpected.arguments[0] = VerificationValue::Integer("3".into());
    assert!(matches!(
        session.respond(&unexpected),
        Observation::Unavailable { .. }
    ));
    assert_eq!(session.issues[0].code, "UNEXPECTED_MOCK_CALL");
}

#[test]
fn recording_a_mock_call_cannot_invent_drop_or_change_actual_collaborator_requests() {
    let (ir, mock) = fixture();
    let mut session =
        OperationMockSession::new(&ir, std::slice::from_ref(&mock), &Default::default());
    let claimed = InvocationObservation {
        invocation: mock.invocation.clone(),
        observation: mock.outcome.clone(),
    };
    assert!(
        matches!(session.check_recorded_calls(Some(std::slice::from_ref(&claimed))), VerificationResult::Fail { issue } if issue.code == "MOCK_OBSERVATION_MISMATCH")
    );
    assert!(
        matches!(session.check_recorded_calls(None), VerificationResult::Unknown { issue } if issue.code == "MOCK_CALL_OBSERVER_REQUIRED")
    );
    session.respond(&mock.invocation);
    assert_eq!(
        session.check_recorded_calls(Some(std::slice::from_ref(&claimed))),
        VerificationResult::Pass
    );
    assert!(matches!(
        session.check_recorded_calls(Some(&[])),
        VerificationResult::Fail { .. }
    ));
    assert!(matches!(
        session.check_recorded_calls(Some(&[claimed.clone(), claimed.clone()])),
        VerificationResult::Fail { .. }
    ));
    let mut altered = claimed.clone();
    altered.observation = Observation::Returned {
        value: VerificationValue::Integer("999".into()),
    };
    assert!(matches!(
        session.check_recorded_calls(Some(&[altered])),
        VerificationResult::Fail { .. }
    ));
    let mut other = claimed.clone();
    other.invocation.subject = "W.Other.load".into();
    assert_eq!(
        session.check_recorded_calls(Some(&[claimed, other])),
        VerificationResult::Pass
    );
}

#[test]
fn mock_trace_consistency_does_not_invent_completion_order_requirements() {
    let (ir, first) = fixture();
    let mut second = first.clone();
    second.invocation.arguments[0] = VerificationValue::Integer("3".into());
    let mut session =
        OperationMockSession::new(&ir, &[first.clone(), second.clone()], &Default::default());
    session.respond(&first.invocation);
    session.respond(&second.invocation);
    let completed = [
        InvocationObservation {
            invocation: second.invocation,
            observation: second.outcome,
        },
        InvocationObservation {
            invocation: first.invocation,
            observation: first.outcome,
        },
    ];
    assert_eq!(
        session.check_recorded_calls(Some(&completed)),
        VerificationResult::Pass
    );
}
#[test]
fn ambiguous_and_invalid_prepared_outcomes_remain_explicitly_unavailable() {
    let (ir, mock) = fixture();
    let mut ambiguous =
        OperationMockSession::new(&ir, &[mock.clone(), mock.clone()], &Default::default());
    assert!(matches!(
        ambiguous.respond(&mock.invocation),
        Observation::Unavailable { .. }
    ));
    assert_eq!(ambiguous.issues[0].code, "AMBIGUOUS_MOCK_REPLY");
    let mut invalid = mock.clone();
    invalid.outcome = Observation::Returned {
        value: VerificationValue::String("wrong".into()),
    };
    let mut session = OperationMockSession::new(&ir, &[invalid], &Default::default());
    assert_eq!(session.issues[0].code, "MOCK_OUTCOME_CONTRACT");
    assert!(matches!(
        session.respond(&mock.invocation),
        Observation::Unavailable { .. }
    ));
}

#[test]
fn historical_operation_cases_without_mocks_keep_the_original_wire_shape() {
    let case: OperationCaseInput = serde_json::from_value(serde_json::json!({"id":"legacy", "invocation":{"subject":"W.Api.next","arguments":[]}, "setup":[]})).unwrap();
    assert!(case.mocks.is_empty());
    assert!(serde_json::to_value(case).unwrap().get("mocks").is_none());
}

#[test]
fn invalid_mock_subjects_argument_counts_and_values_never_supply_prepared_answers() {
    let (ir, mock) = fixture();
    for (mutation, code) in [
        ("subject", "MOCK_OPERATION_UNKNOWN"),
        ("count", "MOCK_ARGUMENT_CONTRACT"),
        ("value", "MOCK_ARGUMENT_CONTRACT"),
    ] {
        let mut invalid = mock.clone();
        match mutation {
            "subject" => invalid.invocation.subject = "W.Missing.load".into(),
            "count" => invalid.invocation.arguments.clear(),
            _ => invalid.invocation.arguments[0] = VerificationValue::Boolean(true),
        }
        let mut session = OperationMockSession::new(&ir, &[invalid], &Default::default());
        assert!(session.issues.iter().any(|issue| issue.code == code));
        assert!(matches!(
            session.respond(&mock.invocation),
            Observation::Unavailable { .. }
        ));
        assert_eq!(session.observed.len(), 1);
    }
}

#[test]
fn declared_mock_failures_keep_typed_payloads_and_reject_undeclared_or_invalid_errors() {
    let compiled = dctx0_language::compile(&[Source { file: "mock-failures.dctx0".into(), text: "workspace W { failure Unavailable { reason:String } port Store { operation load() -> Int ! Unavailable } }".into() }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let mock = OperationMock {
        invocation: Invocation {
            subject: "W.Store.load".into(),
            arguments: vec![],
        },
        outcome: Observation::Returned {
            value: VerificationValue::Failure {
                declaration: "W.Unavailable".into(),
                payload: Some(Box::new(VerificationValue::Record(
                    [(
                        "reason".into(),
                        VerificationValue::String("storage unavailable".into()),
                    )]
                    .into(),
                ))),
            },
        },
    };
    let mut valid =
        OperationMockSession::new(&ir, std::slice::from_ref(&mock), &Default::default());
    assert!(valid.issues.is_empty());
    assert_eq!(valid.respond(&mock.invocation), mock.outcome);
    for wrong_declaration in [false, true] {
        let mut invalid = mock.clone();
        invalid.outcome = Observation::Returned {
            value: VerificationValue::Failure {
                declaration: if wrong_declaration {
                    "W.Missing".into()
                } else {
                    "W.Unavailable".into()
                },
                payload: Some(Box::new(VerificationValue::Record(
                    [("reason".into(), VerificationValue::Integer("1".into()))].into(),
                ))),
            },
        };
        let mut session = OperationMockSession::new(&ir, &[invalid], &Default::default());
        assert!(
            session
                .issues
                .iter()
                .any(|issue| issue.code == "MOCK_OUTCOME_CONTRACT")
        );
        assert!(matches!(
            session.respond(&mock.invocation),
            Observation::Unavailable { .. }
        ));
    }
}

#[test]
fn unsupported_mock_argument_and_outcome_types_remain_unknown() {
    let compiled = dctx0_language::compile(&[Source {
        file: "mock-unsupported.dctx0".into(),
        text: "workspace W { port Store { operation load(value:Int) -> Int } }".into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let mut ir = compiled.ir.unwrap();
    // Simulate a future pinned IR type, rather than the supported Duration builtin.
    let operation = ir.operations.get_mut("W.Store.load").unwrap();
    operation.parameters[0].ty = TypeRef::builtin("FutureBuiltin");
    operation.result = TypeRef::builtin("FutureBuiltin");
    let mock = OperationMock {
        invocation: Invocation {
            subject: "W.Store.load".into(),
            arguments: vec![VerificationValue::String("one second".into())],
        },
        outcome: Observation::Returned {
            value: VerificationValue::String("two seconds".into()),
        },
    };
    let mut session =
        OperationMockSession::new(&ir, std::slice::from_ref(&mock), &Default::default());
    assert_eq!(session.issues.len(), 2);
    assert!(
        session
            .issues
            .iter()
            .all(|issue| issue.code == "UNSUPPORTED_TYPE")
    );
    assert!(matches!(
        session.respond(&mock.invocation),
        Observation::Unavailable { .. }
    ));
}

#[test]
fn planning_rejects_invalid_replies_and_undeclared_mock_collaborators() {
    let compiled = dctx0_language::compile(&[Source { file:"planning-mocks.dctx0".into(), text:"workspace W { port Store { operation load(id:Int) -> Int } surface Api { operation next(id:Int) -> Int {\n call Store.load\n } } }".into() }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let (_, mock) = fixture();
    let input = OperationCaseInput {
        argument_bindings: vec![],
        id: "consumer-case".into(),
        invocation: Invocation {
            subject: "W.Api.next".into(),
            arguments: vec![VerificationValue::Integer("1".into())],
        },
        setup: vec![],
        mocks: vec![mock],
    };
    let project = |ir: &WorkspaceIr, input: OperationCaseInput| {
        project_program(
            ir,
            "snapshot",
            1,
            VerificationScope {
                subjects: ["W.Api.next".into()].into(),
                tasks: Default::default(),
            },
            &ProjectionFixtures {
                operations: [("W.Api.next".into(), vec![input])].into(),
                ..Default::default()
            },
            Default::default(),
        )
    };
    let valid = project(&ir, input.clone());
    assert!(
        !valid
            .issues
            .iter()
            .any(|issue| issue.code == "INVALID_OPERATION_CASE")
    );
    assert!(valid.capabilities.contains(&Capability::Mocks));
    let mut invalid = input.clone();
    invalid.mocks[0].outcome = Observation::Threw {
        message: "untyped reply".into(),
    };
    assert!(
        project(&ir, invalid)
            .issues
            .iter()
            .any(|issue| issue.code == "INVALID_OPERATION_CASE")
    );
    let mut undeclared = ir.clone();
    undeclared.references.retain(|reference| {
        !(reference.from == "W.Api.next" && reference.kind == ReferenceKind::Calls)
    });
    assert!(
        project(&undeclared, input)
            .issues
            .iter()
            .any(|issue| issue.code == "INVALID_OPERATION_CASE")
    );
}
