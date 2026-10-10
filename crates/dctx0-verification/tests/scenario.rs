use dctx0_core::*;
use dctx0_verification::*;
use std::collections::{BTreeMap, BTreeSet};
fn reference() -> (WorkspaceIr, ExamplePlan) {
    let compiled = dctx0_language::compile(&[Source {
        file: "scenarios.dctx0".into(),
        text: r#"workspace W {
port Store { operation load(input: Int) -> Int ! Broken }
failure Broken { reason: String }
event Changed { number: Int }
surface Api { operation read(input: Int) -> Int ! Broken }
example "actual consumer" {
 given Store.load(input) returns 5
 when result = Api.read(input)
 expect Success
 expect result == 5
 expect emits Changed { number = 5 }
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
    let fixtures = ProjectionFixtures {
        examples: BTreeMap::from([(
            "W.actual consumer".into(),
            BTreeMap::from([("input".into(), VerificationValue::Integer("1".into()))]),
        )]),
        ..Default::default()
    };
    let plan = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: BTreeSet::from(["W".into()]),
            tasks: Default::default(),
        },
        &fixtures,
        Default::default(),
    );
    assert!(
        plan.examples[0].issues.is_empty(),
        "{:?}",
        plan.examples[0].issues
    );
    (ir, plan.examples[0].clone())
}
fn call() -> Invocation {
    Invocation {
        subject: "W.Store.load".into(),
        arguments: vec![VerificationValue::Integer("1".into())],
    }
}
fn value(number: &str) -> Observation {
    Observation::Returned {
        value: VerificationValue::Integer(number.into()),
    }
}
fn event(number: &str) -> VerificationEvent {
    VerificationEvent {
        subject: "W.Changed".into(),
        payload: VerificationValue::Record(BTreeMap::from([(
            "number".into(),
            VerificationValue::Integer(number.into()),
        )])),
    }
}
fn completed(step: ScenarioStep) -> ScenarioEvidence {
    match step {
        ScenarioStep::Complete(result) => *result,
        ScenarioStep::Invoke(_) => panic!("Expected completed scenario"),
    }
}
#[test]
fn actual_observations_and_mock_counts_are_retained_without_inventing_given_exactly_once() {
    let (ir, plan) = reference();
    let budgets = VerificationBudgets::default();
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    assert!(
        matches!(scenario.advance(),ScenarioStep::Invoke(invocation) if invocation.subject=="W.Api.read")
    );
    assert_eq!(scenario.collaborator(&call()), value("5"));
    assert_eq!(scenario.collaborator(&call()), value("5"));
    scenario.deliver(value("5"), vec![event("5")]);
    let result = completed(scenario.advance());
    assert_eq!(result.result, VerificationResult::Pass);
    assert_eq!(result.collaborators.len(), 2);
    assert_eq!(result.checks.len(), 3);
    assert!(result.unused_mocks.is_empty());
}
#[test]
fn missing_or_wrong_runtime_evidence_cannot_complete_a_consumer_example() {
    let (ir, plan) = reference();
    let budgets = VerificationBudgets::default();
    for (observation, events, code) in [
        (value("4"), vec![event("5")], "ASSERTION_FAILED"),
        (value("5"), vec![event("4")], "EXPECTED_EVENT"),
        (value("5"), vec![], "EXPECTED_EVENT"),
        (
            Observation::Threw {
                message: "real crash".into(),
            },
            vec![],
            "APPLICATION_EXCEPTION",
        ),
        (
            Observation::Unavailable {
                reason: "no implementation".into(),
            },
            vec![],
            "OPERATION_UNAVAILABLE",
        ),
        (
            Observation::Returned {
                value: VerificationValue::Boolean(true),
            },
            vec![],
            "SCENARIO_CONTRACT",
        ),
    ] {
        let mut scenario = Scenario::new(&ir, &plan, &budgets);
        assert!(matches!(scenario.advance(), ScenarioStep::Invoke(_)));
        scenario.collaborator(&call());
        scenario.deliver(observation, events);
        let result = completed(scenario.advance()).result;
        assert!(
            matches!(result,VerificationResult::Fail { issue }|VerificationResult::Unknown { issue } if issue.code==code),
            "{code}"
        );
    }
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    scenario.advance();
    scenario.deliver(value("5"), vec![event("5")]);
    assert!(
        matches!(completed(scenario.advance()).result,VerificationResult::Unknown { issue } if issue.code=="UNEXERCISED_COLLABORATOR")
    );
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    scenario.advance();
    let mut wrong = call();
    wrong.arguments = vec![VerificationValue::Integer("2".into())];
    assert!(matches!(
        scenario.collaborator(&wrong),
        Observation::Unavailable { .. }
    ));
    scenario.deliver(value("5"), vec![event("5")]);
    assert!(
        matches!(completed(scenario.advance()).result,VerificationResult::Fail { issue } if issue.code=="UNEXPECTED_COLLABORATOR")
    );
}

fn expect_code(result: VerificationResult, code: &str) {
    assert!(
        matches!(&result,VerificationResult::Fail { issue }|VerificationResult::Unknown { issue } if issue.code==code),
        "{code}: {result:?}"
    );
}
#[test]
fn unsupported_or_malformed_scenario_plans_and_delivery_states_are_explicit() {
    let (ir, original) = reference();
    let budgets = VerificationBudgets::default();
    let argument = match &original.mocks[0].invocation.value {
        ExpressionKind::Call { arguments, .. } => arguments[0].clone(),
        _ => panic!("Expected call"),
    };
    for (mutation, code) in [
        (0, "SCENARIO_ORDER"),
        (1, "SCENARIO_ORDER"),
        (2, "SCENARIO_ORDER"),
        (3, "SCENARIO_INVOCATION"),
        (4, "SCENARIO_OPERATION"),
        (5, "SCENARIO_ARITY"),
        (6, "UNBOUND_VALUE"),
        (7, "SCENARIO_MOCK_OUTCOME"),
        (8, "SCENARIO_FAILURE"),
        (9, "MODELED_VALUE_CONTRACT"),
        (10, "UNSUPPORTED_TYPE"),
        (11, "SCENARIO_INVOCATION"),
        (12, "EMPTY_SCENARIO"),
    ] {
        let mut plan = original.clone();
        match mutation {
            0 => plan.order = vec![ExampleAction::PrepareMock(99)],
            1 => plan.order = vec![ExampleAction::Invoke(99)],
            2 => plan.order = vec![ExampleAction::Check(99)],
            3 => plan.mocks[0].invocation = argument.clone(),
            4 => {
                if let ExpressionKind::Call { target, .. } = &mut plan.mocks[0].invocation.value {
                    *target = Some("missing".into());
                }
            }
            5 => {
                if let ExpressionKind::Call { arguments, .. } = &mut plan.mocks[0].invocation.value
                {
                    arguments.clear();
                }
            }
            6 => plan.bindings.clear(),
            7 => plan.mocks[0].outcome = None,
            8 => {
                plan.mocks[0].outcome = Some(MockOutcome::Failed {
                    expression: argument.clone(),
                })
            }
            9 => {
                let mut expression = argument.clone();
                expression.ty = TypeRef::builtin("Boolean");
                plan.mocks[0].outcome = Some(MockOutcome::Returned { expression });
            }
            10 => {
                let mut expression = argument.clone();
                expression.ty = TypeRef::builtin("Duration");
                plan.mocks[0].outcome = Some(MockOutcome::Returned { expression });
            }
            11 => {
                plan.order = vec![ExampleAction::Invoke(0)];
                plan.invocations[0].invocation = argument.clone();
            }
            _ => plan.order.clear(),
        }
        expect_code(
            completed(Scenario::new(&ir, &plan, &budgets).advance()).result,
            code,
        );
    }
    let limited = VerificationBudgets {
        evaluation_steps: 0,
        ..budgets.clone()
    };
    expect_code(
        completed(Scenario::new(&ir, &original, &limited).advance()).result,
        "EVALUATION_BUDGET",
    );
    let mut unsupported = original.clone();
    unsupported.issues.push(VerificationIssue {
        code: "UNSUPPORTED".into(),
        message: "Missing capability".into(),
        span: None,
    });
    expect_code(
        completed(Scenario::new(&ir, &unsupported, &budgets).advance()).result,
        "UNSUPPORTED",
    );
    let mut scenario = Scenario::new(&ir, &original, &budgets);
    scenario.deliver(value("5"), vec![]);
    expect_code(completed(scenario.advance()).result, "SCENARIO_PROTOCOL");
    let mut scenario = Scenario::new(&ir, &original, &budgets);
    scenario.advance();
    expect_code(completed(scenario.advance()).result, "SCENARIO_PROTOCOL");
    let mut pure = ir.clone();
    pure.operations.get_mut("W.Store.load").unwrap().pure = true;
    let mut nested = original.clone();
    nested.mocks[0].outcome = Some(MockOutcome::Returned {
        expression: nested.mocks[0].invocation.clone(),
    });
    expect_code(
        completed(Scenario::new(&pure, &nested, &budgets).advance()).result,
        "SCENARIO_EXPRESSION_CALL",
    );
}

#[test]
fn explicit_failure_payloads_and_missing_declarations_remain_separate_from_success() {
    let (ir, original) = reference();
    let budgets = VerificationBudgets::default();
    let span = original.mocks[0].invocation.span.clone();
    let string = Expression {
        ty: TypeRef::builtin("String"),
        span: span.clone(),
        value: ExpressionKind::Literal {
            text: "unavailable".into(),
            literal_kind: "String".into(),
        },
    };
    let payload = Expression {
        ty: TypeRef {
            name: "Broken".into(),
            symbol_id: Some("W.Broken".into()),
            arguments: vec![],
        },
        span: span.clone(),
        value: ExpressionKind::Record {
            declaration: "W.Broken".into(),
            fields: vec![("reason".into(), string)],
        },
    };
    let mut plan = original.clone();
    plan.order = vec![
        ExampleAction::PrepareMock(0),
        ExampleAction::Invoke(0),
        ExampleAction::Check(0),
    ];
    plan.mocks[0].outcome = Some(MockOutcome::Failed {
        expression: payload.clone(),
    });
    plan.expectations = vec![Expectation::Failure {
        source_id: "expected-failure".into(),
        declaration: Some("W.Broken".into()),
        payload: Some(payload),
    }];
    for mode in ["ok", "success", "wrong", "missing"] {
        let mut plan = plan.clone();
        if mode == "success" {
            plan.expectations[0] = Expectation::Success {
                source_id: "success".into(),
            };
        }
        if mode == "wrong" {
            if let Expectation::Failure { declaration, .. } = &mut plan.expectations[0] {
                *declaration = Some("different".into());
            }
        }
        if mode == "missing" {
            if let Expectation::Failure { declaration, .. } = &mut plan.expectations[0] {
                *declaration = None;
            }
        }
        let mut scenario = Scenario::new(&ir, &plan, &budgets);
        assert!(matches!(scenario.advance(), ScenarioStep::Invoke(_)));
        let failure = scenario.collaborator(&call());
        scenario.deliver(failure, vec![]);
        let result = completed(scenario.advance()).result;
        match mode {
            "ok" => assert_eq!(result, VerificationResult::Pass),
            "success" => expect_code(result, "EXPECTED_SUCCESS"),
            "wrong" => expect_code(result, "EXPECTED_FAILURE"),
            _ => expect_code(result, "EXPECTED_FAILURE_TYPE"),
        }
    }
    let mut nonboolean = original.clone();
    nonboolean.expectations[1] = Expectation::Assertion {
        source_id: "nonboolean".into(),
        expression: Expression {
            ty: TypeRef::builtin("Int"),
            span,
            value: ExpressionKind::Literal {
                text: "1".into(),
                literal_kind: "Int".into(),
            },
        },
    };
    let mut scenario = Scenario::new(&ir, &nonboolean, &budgets);
    scenario.advance();
    scenario.collaborator(&call());
    scenario.deliver(value("5"), vec![event("5")]);
    expect_code(
        completed(scenario.advance()).result,
        "NON_BOOLEAN_ASSERTION",
    );
    for observed in [
        VerificationEvent {
            subject: "missing".into(),
            payload: VerificationValue::Unit,
        },
        VerificationEvent {
            subject: "W.Changed".into(),
            payload: VerificationValue::Unit,
        },
    ] {
        let mut scenario = Scenario::new(&ir, &original, &budgets);
        scenario.advance();
        scenario.collaborator(&call());
        scenario.deliver(value("5"), vec![observed.clone()]);
        expect_code(
            completed(scenario.advance()).result,
            if observed.subject == "missing" {
                "UNDECLARED_EVENT"
            } else {
                "SCENARIO_CONTRACT"
            },
        );
    }
}

#[test]
fn skipped_or_duplicated_mandatory_actions_cannot_claim_pass() {
    let (ir, original) = reference();
    let budgets = VerificationBudgets::default();
    let mut skipped = original.clone();
    skipped
        .order
        .retain(|action| !matches!(action, ExampleAction::Check(_)));
    let mut scenario = Scenario::new(&ir, &skipped, &budgets);
    scenario.advance();
    scenario.collaborator(&call());
    scenario.deliver(value("5"), vec![event("5")]);
    expect_code(
        completed(scenario.advance()).result,
        "MISSING_SCENARIO_ACTION",
    );
    let mut duplicate = original.clone();
    duplicate.order.insert(0, ExampleAction::PrepareMock(0));
    expect_code(
        completed(Scenario::new(&ir, &duplicate, &budgets).advance()).result,
        "DUPLICATE_SCENARIO_ACTION",
    );
}

#[test]
fn unsupported_native_result_contracts_remain_unknown_even_with_an_observation() {
    let (mut ir, plan) = reference();
    let budgets = VerificationBudgets::default();
    ir.operations.get_mut("W.Api.read").unwrap().result = TypeRef::builtin("Duration");
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    scenario.advance();
    scenario.collaborator(&call());
    scenario.deliver(value("5"), vec![event("5")]);
    expect_code(completed(scenario.advance()).result, "UNSUPPORTED_TYPE");
}

#[test]
fn executed_failures_dominate_unknowns_and_keep_the_first_contract_violation() {
    let (mut ir, plan) = reference();
    let budgets = VerificationBudgets::default();
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    scenario.advance();
    let mut wrong = call();
    wrong.subject = "wrong port".into();
    scenario.collaborator(&wrong);
    scenario.deliver(
        Observation::Threw {
            message: "later exception".into(),
        },
        vec![VerificationEvent {
            subject: "wrong event".into(),
            payload: VerificationValue::Unit,
        }],
    );
    expect_code(
        completed(scenario.advance()).result,
        "UNEXPECTED_COLLABORATOR",
    );
    ir.operations.get_mut("W.Api.read").unwrap().result = TypeRef::builtin("Duration");
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    scenario.advance();
    scenario.collaborator(&call());
    scenario.deliver(
        value("5"),
        vec![VerificationEvent {
            subject: "wrong event".into(),
            payload: VerificationValue::Unit,
        }],
    );
    expect_code(completed(scenario.advance()).result, "UNDECLARED_EVENT");
}
