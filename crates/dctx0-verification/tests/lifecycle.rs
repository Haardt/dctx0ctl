use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
use dctx0_verification::*;
use std::collections::{BTreeMap, BTreeSet};
fn fixture() -> (WorkspaceIr, LifecyclePlan) {
    let compiled = dctx0_language::compile(&[Source {
        file: "lifecycle.dctx0".into(),
        text: r#"workspace W {
type Status = Open | Closed
entity Item { id: String
 status: Status
 key id }
surface Api { operation finish(id: String) -> Int }
lifecycle Flow for Item using status {
 state Open initial
 state Closed terminal
 transition Api.finish from Open to Closed when true
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
        "pinned",
        1,
        VerificationScope {
            subjects: BTreeSet::from(["W.Flow".into()]),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    assert!(plan.lifecycles[0].issues.is_empty());
    (ir, plan.lifecycles[0].clone())
}
fn rows(name: &str) -> Option<BTreeMap<String, Vec<VerificationValue>>> {
    Some(BTreeMap::from([(
        "W.Item".into(),
        vec![VerificationValue::Record(BTreeMap::from([
            ("id".into(), VerificationValue::String("chosen".into())),
            (
                "status".into(),
                VerificationValue::Variant {
                    declaration: "W.Status".into(),
                    name: name.into(),
                    payload: None,
                },
            ),
        ]))],
    )]))
}
fn observation() -> LifecycleObservation {
    LifecycleObservation {
        invocation: InvocationObservation {
            invocation: Invocation {
                subject: "W.Api.finish".into(),
                arguments: vec![VerificationValue::String("chosen".into())],
            },
            observation: Observation::Returned {
                value: VerificationValue::Integer("1".into()),
            },
        },
        context: ObservationContext {
            before: rows("Open"),
            after: rows("Closed"),
            ..Default::default()
        },
    }
}
fn keys() -> Vec<VerificationValue> {
    vec![VerificationValue::String("chosen".into())]
}
fn code(result: VerificationResult, expected: &str) {
    assert!(
        matches!(&result,VerificationResult::Fail {issue}|VerificationResult::Unknown {issue} if issue.code==expected),
        "{expected}: {result:?}"
    );
}
#[test]
fn actual_observed_state_and_every_mandatory_transition_are_required() {
    let (ir, plan) = fixture();
    let budgets = VerificationBudgets::default();
    let path = &plan.paths[0];
    let obs = observation();
    let result = assess_lifecycle(
        &ir,
        &plan,
        path,
        &keys(),
        std::slice::from_ref(&obs),
        &budgets,
    );
    assert_eq!(result.result, VerificationResult::Pass);
    assert_eq!(result.reached_states, vec!["Open", "Closed"]);
    assert!(result.open_transitions.is_empty());
    code(
        assess_lifecycle(&ir, &plan, path, &keys(), &[], &budgets).result,
        "MISSING_LIFECYCLE_TRANSITION",
    );
    for (change, expected) in [
        (0, "LIFECYCLE_STATE_MISMATCH"),
        (1, "STATE_OBSERVER_REQUIRED"),
        (2, "LIFECYCLE_INVOCATION"),
        (3, "LIFECYCLE_ARGUMENTS"),
        (4, "LIFECYCLE_OPERATION_FAILED"),
        (5, "OPERATION_UNAVAILABLE"),
        (6, "LIFECYCLE_RETURN_CONTRACT"),
    ] {
        let mut obs = obs.clone();
        match change {
            0 => obs.context.after = rows("Open"),
            1 => obs.context.after = None,
            2 => obs.invocation.invocation.subject = "wrong".into(),
            3 => obs.invocation.invocation.arguments.clear(),
            4 => {
                obs.invocation.observation = Observation::Threw {
                    message: "crash".into(),
                }
            }
            5 => {
                obs.invocation.observation = Observation::Unavailable {
                    reason: "missing".into(),
                }
            }
            _ => {
                obs.invocation.observation = Observation::Returned {
                    value: VerificationValue::Boolean(true),
                }
            }
        }
        code(
            assess_lifecycle(&ir, &plan, path, &keys(), &[obs], &budgets).result,
            expected,
        );
    }
    code(
        assess_lifecycle(&ir, &plan, path, &[], std::slice::from_ref(&obs), &budgets).result,
        "INVALID_LIFECYCLE_CASE",
    );
    code(
        assess_lifecycle(
            &ir,
            &plan,
            path,
            &keys(),
            &[obs.clone(), obs.clone()],
            &budgets,
        )
        .result,
        "SURPLUS_LIFECYCLE_OBSERVATIONS",
    );
    let mut plan = plan.clone();
    plan.transitions[0].guard.as_mut().unwrap().value = ExpressionKind::Literal {
        text: "false".into(),
        literal_kind: "Boolean".into(),
    };
    code(
        lifecycle_ready(
            &ir,
            &plan,
            0,
            &keys(),
            &obs.invocation.invocation,
            &obs.context,
            &budgets,
        ),
        "LIFECYCLE_GUARD",
    );
    code(
        assess_lifecycle(&ir, &plan, &plan.paths[0], &keys(), &[obs], &budgets).result,
        "LIFECYCLE_GUARD",
    );
}

#[test]
fn lifecycle_readiness_rejects_missing_contracts_states_and_unexecutable_guards() {
    let (ir, plan) = fixture();
    let b = VerificationBudgets::default();
    let obs = observation();
    assert_eq!(
        lifecycle_ready(
            &ir,
            &plan,
            0,
            &keys(),
            &obs.invocation.invocation,
            &obs.context,
            &b
        ),
        VerificationResult::Pass
    );
    code(
        lifecycle_ready(
            &ir,
            &plan,
            1,
            &keys(),
            &obs.invocation.invocation,
            &obs.context,
            &b,
        ),
        "LIFECYCLE_TRANSITION",
    );
    let mut context = obs.context.clone();
    context.before = None;
    code(
        lifecycle_ready(
            &ir,
            &plan,
            0,
            &keys(),
            &obs.invocation.invocation,
            &context,
            &b,
        ),
        "STATE_OBSERVER_REQUIRED",
    );
    context.before = rows("Closed");
    code(
        lifecycle_ready(
            &ir,
            &plan,
            0,
            &keys(),
            &obs.invocation.invocation,
            &context,
            &b,
        ),
        "LIFECYCLE_STATE_MISMATCH",
    );
    let mut unconditional = plan.clone();
    unconditional.transitions[0].guard = None;
    assert_eq!(
        lifecycle_ready(
            &ir,
            &unconditional,
            0,
            &keys(),
            &obs.invocation.invocation,
            &obs.context,
            &b
        ),
        VerificationResult::Pass
    );
    assert_eq!(
        assess_lifecycle(
            &ir,
            &unconditional,
            &unconditional.paths[0],
            &keys(),
            std::slice::from_ref(&obs),
            &b
        )
        .result,
        VerificationResult::Pass
    );
    let mut missing = ir.clone();
    missing.symbols.remove("W.Item");
    code(
        *lifecycle_status(
            &missing,
            &plan,
            StatePhase::Before,
            &keys(),
            &obs.context,
            &b,
        )
        .unwrap_err(),
        "LIFECYCLE_ENTITY",
    );
    missing = ir.clone();
    missing.data.get_mut("W.Item").unwrap().fields.clear();
    code(
        *lifecycle_status(
            &missing,
            &plan,
            StatePhase::Before,
            &keys(),
            &obs.context,
            &b,
        )
        .unwrap_err(),
        "LIFECYCLE_FIELD",
    );
    let mut guarded = plan.clone();
    guarded.transitions[0].guard.as_mut().unwrap().value = ExpressionKind::Variable {
        name: "missing-observation".into(),
    };
    assert!(matches!(
        lifecycle_ready(
            &ir,
            &guarded,
            0,
            &keys(),
            &obs.invocation.invocation,
            &obs.context,
            &b
        ),
        VerificationResult::Unknown { .. }
    ));
    assert!(matches!(
        assess_lifecycle(
            &ir,
            &guarded,
            &guarded.paths[0],
            &keys(),
            std::slice::from_ref(&obs),
            &b
        )
        .result,
        VerificationResult::Unknown { .. }
    ));
    let mut unsupported = ir.clone();
    unsupported
        .data
        .get_mut("W.Item")
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.name == "status")
        .unwrap()
        .ty = TypeRef::builtin("Boolean");
    let mut boolean = obs.context.clone();
    if let VerificationValue::Record(fields) =
        &mut boolean.before.as_mut().unwrap().get_mut("W.Item").unwrap()[0]
    {
        fields.insert("status".into(), VerificationValue::Boolean(true));
    }
    code(
        *lifecycle_status(
            &unsupported,
            &plan,
            StatePhase::Before,
            &keys(),
            &boolean,
            &b,
        )
        .unwrap_err(),
        "LIFECYCLE_STATE_REPRESENTATION",
    );
}

#[test]
fn disconnected_paths_invalid_inputs_and_unsupported_results_cannot_certify_a_lifecycle() {
    let (ir, plan) = fixture();
    let b = VerificationBudgets::default();
    let obs = observation();
    let assess = |ir: &WorkspaceIr, plan: &LifecyclePlan, observations: &[LifecycleObservation]| {
        assess_lifecycle(ir, plan, &plan.paths[0], &keys(), observations, &b).result
    };
    let mut invalid = plan.clone();
    invalid.issues.push(VerificationIssue {
        code: "UNSUPPORTED_PATH".into(),
        message: "review required".into(),
        span: None,
    });
    code(
        assess(&ir, &invalid, std::slice::from_ref(&obs)),
        "UNSUPPORTED_PATH",
    );
    invalid = plan.clone();
    invalid.paths[0].transitions.push(9);
    code(
        assess(&ir, &invalid, std::slice::from_ref(&obs)),
        "LIFECYCLE_TRANSITION",
    );
    let mut missing = ir.clone();
    missing.operations.clear();
    code(
        assess(&missing, &plan, std::slice::from_ref(&obs)),
        "LIFECYCLE_OPERATION",
    );
    let mut bad = obs.clone();
    bad.invocation.invocation.arguments[0] = VerificationValue::Unit;
    code(assess(&ir, &plan, &[bad]), "LIFECYCLE_ARGUMENTS");
    bad = obs.clone();
    bad.context.before = None;
    code(assess(&ir, &plan, &[bad]), "STATE_OBSERVER_REQUIRED");
    let mut chained = plan.clone();
    chained.paths[0].transitions.push(0);
    let prefix = assess_lifecycle(
        &ir,
        &chained,
        &chained.paths[0],
        &keys(),
        std::slice::from_ref(&obs),
        &b,
    );
    assert_eq!(prefix.completed_transitions.len(), 1);
    assert_eq!(
        prefix.open_transitions,
        vec![plan.transitions[0].source_id.clone()]
    );
    let mut second = obs.clone();
    second.context.before = rows("Open");
    code(
        assess(&ir, &chained, &[obs.clone(), second]),
        "DISCONNECTED_LIFECYCLE_EVIDENCE",
    );
    let mut unsupported = ir.clone();
    unsupported
        .operations
        .get_mut("W.Api.finish")
        .unwrap()
        .result = TypeRef::builtin("Duration");
    code(assess(&unsupported, &plan, &[obs]), "UNSUPPORTED_TYPE");
}

#[test]
fn transition_guards_use_before_state_even_when_a_permit_is_consumed() {
    let compiled = dctx0_language::compile(&[Source {
        file: "permit.dctx0".into(),
        text: r#"workspace W {
 type Status = Open | Closed
 entity Item { id: String
 status: Status
 key id }
 entity Permit { id: String
 key id }
 surface Api { operation finish(id: String) -> Int { requires exists Permit(id) } }
 lifecycle Flow for Item using status {
 state Open initial
 state Closed terminal
 transition Api.finish from Open to Closed when true
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
    let program = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: BTreeSet::from(["W.Flow".into()]),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let mut guarded = program.lifecycles[0].clone();
    guarded.transitions[0].guard = Some(
        ir.nodes
            .iter()
            .find(|node| node.owner == "W.Api.finish" && node.kind == StatementKind::Requires)
            .unwrap()
            .expressions[0]
            .clone(),
    );
    let plan = &guarded;
    let mut obs = observation();
    obs.context.phase = StatePhase::After;
    obs.context.before.as_mut().unwrap().insert(
        "W.Permit".into(),
        vec![VerificationValue::Record(BTreeMap::from([(
            "id".into(),
            VerificationValue::String("chosen".into()),
        )]))],
    );
    obs.context
        .after
        .as_mut()
        .unwrap()
        .insert("W.Permit".into(), vec![]);
    assert_eq!(
        assess_lifecycle(&ir, plan, &plan.paths[0], &keys(), &[obs], &program.budgets).result,
        VerificationResult::Pass
    );
}
