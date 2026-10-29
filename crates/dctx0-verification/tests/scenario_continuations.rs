use dctx0_core::*;
use dctx0_verification::*;
use std::collections::BTreeSet;

fn reference() -> (WorkspaceIr, ExamplePlan) {
    let compiled = dctx0_language::compile(&[Source {
        file: "nested.dctx0".into(),
        text: r#"workspace W {
 surface Api {
  operation next() -> Int { pure }
  operation sum(left: Int, right: Int) -> Int { pure }
 }
 event Changed { number: Int }
 example "nested actual calls" {
  when result = Api.sum(Api.next(), Api.next())
  expect result == Api.sum(Api.next(), Api.next())
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
    let plan = project_program(
        &ir,
        "nested",
        1,
        VerificationScope {
            subjects: BTreeSet::from(["W".into()]),
            tasks: Default::default(),
        },
        &ProjectionFixtures::default(),
        VerificationBudgets::default(),
    );
    assert!(
        plan.examples[0].issues.is_empty(),
        "{:?}",
        plan.examples[0].issues
    );
    (ir, plan.examples[0].clone())
}
fn returned(number: &str) -> Observation {
    Observation::Returned {
        value: VerificationValue::Integer(number.into()),
    }
}
fn completed(step: ScenarioStep) -> ScenarioEvidence {
    let ScenarioStep::Complete(evidence) = step else {
        panic!("Expected completion")
    };
    *evidence
}

#[test]
fn nested_argument_and_assertion_calls_execute_once_per_occurrence_in_order() {
    let (ir, plan) = reference();
    let budgets = VerificationBudgets::default();
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    for (subject, arguments, number) in [
        ("W.Api.next", vec![], "1"),
        ("W.Api.next", vec![], "2"),
        ("W.Api.sum", vec!["1", "2"], "3"),
        ("W.Api.next", vec![], "1"),
        ("W.Api.next", vec![], "2"),
        ("W.Api.sum", vec!["1", "2"], "3"),
    ] {
        let ScenarioStep::Invoke(invocation) = scenario.advance() else {
            panic!("Expected {subject}")
        };
        assert_eq!(invocation.subject, subject);
        assert_eq!(
            invocation.arguments,
            arguments
                .into_iter()
                .map(|value| VerificationValue::Integer(value.into()))
                .collect::<Vec<_>>()
        );
        scenario.deliver_observed(returned(number), vec![], Some(vec![]));
    }
    let result = completed(scenario.advance());
    assert_eq!(result.result, VerificationResult::Pass);
    assert_eq!(result.invocations.len(), 6);
    assert_eq!(result.checks.len(), 1);
}

#[test]
fn nested_calls_keep_bad_returns_exceptions_events_and_missing_delivery_visible() {
    let (ir, plan) = reference();
    let budgets = VerificationBudgets::default();
    for (observation, events, expected) in [
        (
            Observation::Returned {
                value: VerificationValue::Boolean(true),
            },
            vec![],
            "SCENARIO_CONTRACT",
        ),
        (
            Observation::Threw {
                message: "actual error".into(),
            },
            vec![],
            "APPLICATION_EXCEPTION",
        ),
        (
            Observation::Unavailable {
                reason: "not implemented".into(),
            },
            vec![],
            "OPERATION_UNAVAILABLE",
        ),
        (
            returned("1"),
            vec![VerificationEvent {
                subject: "W.Changed".into(),
                payload: VerificationValue::Record(Default::default()),
            }],
            "PURE_OPERATION_EMITTED",
        ),
    ] {
        let mut scenario = Scenario::new(&ir, &plan, &budgets);
        assert!(matches!(scenario.advance(), ScenarioStep::Invoke(_)));
        scenario.deliver(observation, events);
        let result = completed(scenario.advance()).result;
        assert!(
            matches!(result, VerificationResult::Fail { ref issue } | VerificationResult::Unknown { ref issue } if issue.code == expected),
            "{result:?}"
        );
    }
    let mut scenario = Scenario::new(&ir, &plan, &budgets);
    assert!(matches!(scenario.advance(), ScenarioStep::Invoke(_)));
    assert!(
        matches!(completed(scenario.advance()).result, VerificationResult::Unknown { issue } if issue.code == "SCENARIO_PROTOCOL")
    );
}
