use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
use dctx0_verification::*;
use std::collections::{BTreeMap, BTreeSet};
fn model() -> (WorkspaceIr, OperationPlan) {
    let c = dctx0_language::compile(&[Source {
        file: "operations.dctx0".into(),
        text: r#"workspace W {
failure Broken { reason: String }
port Store { operation load(input:Int) -> Int ! Broken }
surface Api {
 operation read(input:Int) -> Int ! Broken {
  requires input >= 0
  ensures success implies result == input + 1
  ensures calls Store.load(input) exactly 1
 }
}
}"#
        .into(),
    }]);
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    let ir = c.ir.unwrap();
    let p = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: BTreeSet::from(["W.Api.read".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    );
    let op = p
        .operations
        .into_iter()
        .find(|o| o.operation.id == "W.Api.read")
        .unwrap();
    (ir, op)
}
fn observed(value: VerificationValue) -> InvocationObservation {
    InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.read".into(),
            arguments: vec![VerificationValue::Integer("1".into())],
        },
        observation: Observation::Returned { value },
    }
}
fn context() -> ObservationContext {
    ObservationContext {
        resources: None,
        calls: Some(vec![InvocationObservation {
            invocation: Invocation {
                subject: "W.Store.load".into(),
                arguments: vec![VerificationValue::Integer("1".into())],
            },
            observation: Observation::Returned {
                value: VerificationValue::Integer("2".into()),
            },
        }]),
        ..Default::default()
    }
}

#[test]
fn a_matching_call_count_does_not_prove_an_invalid_collaborator_return() {
    let (ir, plan) = model();
    let good = observed(VerificationValue::Integer("2".into()));
    let mut invalid = context();
    invalid.calls.as_mut().unwrap()[0].observation = Observation::Returned {
        value: VerificationValue::String("not an Int".into()),
    };
    assert!(
        matches!(assess_operation(&ir, &plan, &good, &invalid, &Default::default()).result,
        VerificationResult::Fail { issue } if issue.code == "CALL_CONTRACT_VIOLATED")
    );
    invalid.calls.as_mut().unwrap()[0].observation = Observation::Unavailable {
        reason: "Missing actual result".into(),
    };
    assert!(
        matches!(assess_operation(&ir, &plan, &good, &invalid, &Default::default()).result,
        VerificationResult::Unknown { issue } if issue.code == "CALL_OBSERVATION_UNAVAILABLE")
    );
}
#[test]
fn central_clauses_use_observed_call_counts_and_declared_outcomes() {
    let (ir, plan) = model();
    let good = observed(VerificationValue::Integer("2".into()));
    let budgets = VerificationBudgets::default();
    let report = assess_operation(&ir, &plan, &good, &context(), &budgets);
    assert_eq!(report.result, VerificationResult::Pass);
    assert_eq!(report.checked.len(), 3);
    let wrong = assess_operation(
        &ir,
        &plan,
        &observed(VerificationValue::Integer("3".into())),
        &context(),
        &budgets,
    );
    assert!(
        matches!(wrong.result,VerificationResult::Fail{issue} if issue.code=="OPERATION_CLAUSE_VIOLATED")
    );
    let mut missing = context();
    missing.calls = None;
    assert!(
        matches!(assess_operation(&ir,&plan,&good,&missing,&budgets).result,VerificationResult::Unknown{issue} if issue.code=="TRACE_OBSERVER_REQUIRED")
    );
    missing.calls = Some(vec![]);
    assert!(matches!(
        assess_operation(&ir, &plan, &good, &missing, &budgets).result,
        VerificationResult::Fail { .. }
    ));
    let failure = observed(VerificationValue::Failure {
        declaration: "W.Broken".into(),
        payload: Some(Box::new(VerificationValue::Record(BTreeMap::from([(
            "reason".into(),
            VerificationValue::String("actual failure".into()),
        )])))),
    });
    assert_eq!(
        assess_operation(&ir, &plan, &failure, &context(), &budgets).result,
        VerificationResult::Pass
    );
    let undeclared = observed(VerificationValue::Failure {
        declaration: "W.Other".into(),
        payload: None,
    });
    assert!(
        matches!(assess_operation(&ir,&plan,&undeclared,&context(),&budgets).result,VerificationResult::Fail{issue} if issue.code=="OPERATION_RESULT_CONTRACT")
    );
}
#[test]
fn invalid_inputs_unmet_requires_missing_observers_and_native_exceptions_are_distinct() {
    let (ir, mut plan) = model();
    let budgets = VerificationBudgets::default();
    let mut good = observed(VerificationValue::Integer("2".into()));
    good.invocation.arguments[0] = VerificationValue::Integer("-1".into());
    assert!(
        matches!(operation_ready(&ir,&plan,&good.invocation,&context(),&budgets),VerificationResult::Unknown{issue} if issue.code=="OPERATION_PRECONDITION_UNMET")
    );
    good.invocation.arguments[0] = VerificationValue::Unit;
    assert!(
        matches!(operation_ready(&ir,&plan,&good.invocation,&context(),&budgets),VerificationResult::Unknown{issue} if issue.code=="INVALID_OPERATION_INPUT")
    );
    good.invocation.arguments.clear();
    assert!(
        matches!(operation_ready(&ir,&plan,&good.invocation,&context(),&budgets),VerificationResult::Unknown{issue} if issue.code=="OPERATION_INVOCATION")
    );
    good = observed(VerificationValue::Integer("2".into()));
    good.observation = Observation::Threw {
        message: "actual exception".into(),
    };
    assert!(
        matches!(assess_operation(&ir,&plan,&good,&context(),&budgets).result,VerificationResult::Fail{issue} if issue.code=="APPLICATION_EXCEPTION")
    );
    good.observation = Observation::Unavailable {
        reason: "not wired".into(),
    };
    assert!(matches!(
        assess_operation(&ir, &plan, &good, &context(), &budgets).result,
        VerificationResult::Unknown { .. }
    ));
    plan.clauses[0].expressions.clear();
    assert!(
        matches!(operation_ready(&ir,&plan,&good.invocation,&context(),&budgets),VerificationResult::Unknown{issue} if issue.code=="CLAUSE_NOT_EXECUTABLE")
    );
}

#[test]
fn incomplete_pinned_postconditions_and_result_types_remain_open() {
    let (ir, plan) = model();
    let actual = observed(VerificationValue::Integer("2".into()));
    assert_eq!(
        assess_operation(&ir, &plan, &actual, &context(), &Default::default()).result,
        VerificationResult::Pass
    );
    let mut non_boolean = plan.clone();
    let clause = non_boolean
        .clauses
        .iter_mut()
        .find(|node| node.kind == StatementKind::Ensures)
        .unwrap();
    clause.expressions[0].value = ExpressionKind::Variable {
        name: "input".into(),
    };
    let assessed = assess_operation(&ir, &non_boolean, &actual, &context(), &Default::default());
    assert!(
        matches!(assessed.result, VerificationResult::Unknown { issue }
        if issue.code == "CLAUSE_OBSERVATION_REQUIRED")
    );

    let mut unknown_type = plan.clone();
    unknown_type.operation.result = TypeRef::builtin("UnmodeledResult");
    let assessed = assess_operation(&ir, &unknown_type, &actual, &context(), &Default::default());
    assert!(matches!(
        assessed.result,
        VerificationResult::Unknown { .. }
    ));

    let mut unsupported = plan.clone();
    let clause = unsupported
        .clauses
        .iter_mut()
        .find(|node| node.kind == StatementKind::Ensures)
        .unwrap();
    let clause_id = clause.id.clone();
    clause.kind = StatementKind::Decision;
    let assessed = assess_operation(&ir, &unsupported, &actual, &context(), &Default::default());
    assert!(
        matches!(assessed.result, VerificationResult::Unknown { issue }
        if issue.code == "OPERATION_EFFECT_PENDING")
    );
    assert!(!assessed.checked.contains(&clause_id));
}
