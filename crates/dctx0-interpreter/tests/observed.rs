use dctx0_core::*;
use dctx0_interpreter::{Evaluation, ObservationContext, evaluate_observed};
use std::collections::BTreeMap;
fn fixture() -> (WorkspaceIr, Vec<Expression>) {
    let compiled = dctx0_language::compile(&[Source {
        file: "observed.dctx0".into(),
        text: r#"workspace W {
entity Item { a: String
 b: Int
 revision: Int
 key a, b }
event Changed { value: Int }
port Store { operation load(a: String, b: Int) -> Int }
surface Api { operation bump(a: String, b: Int) -> Int {
 requires exists Item(a, b)
 ensures after.Item(a, b).revision == before.Item(a, b).revision + 1
 ensures calls Store.load(a, b) exactly 1
 ensures emits Changed() exactly 1
} }
}"#
        .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let expressions = ir
        .nodes
        .iter()
        .filter(|node| {
            node.owner == "W.Api.bump"
                && matches!(node.kind, StatementKind::Requires | StatementKind::Ensures)
        })
        .flat_map(|node| node.expressions.clone())
        .collect();
    (ir, expressions)
}
fn int(n: &str) -> VerificationValue {
    VerificationValue::Integer(n.into())
}
fn row(revision: &str) -> VerificationValue {
    VerificationValue::Record(BTreeMap::from([
        ("a".into(), VerificationValue::String("r".into())),
        ("b".into(), int("2")),
        ("revision".into(), int(revision)),
    ]))
}
fn state(rows: Vec<VerificationValue>) -> Option<BTreeMap<String, Vec<VerificationValue>>> {
    Some(BTreeMap::from([("W.Item".into(), rows)]))
}
fn context() -> ObservationContext {
    ObservationContext {
        resources: None,
        phase: StatePhase::After,
        before: state(vec![row("3")]),
        after: state(vec![row("4")]),
        calls: Some(vec![InvocationObservation {
            invocation: Invocation {
                subject: "W.Store.load".into(),
                arguments: vec![VerificationValue::String("r".into()), int("2")],
            },
            observation: Observation::Returned { value: int("4") },
        }]),
        events: Some(vec![VerificationEvent {
            subject: "W.Changed".into(),
            payload: VerificationValue::Record(BTreeMap::from([("value".into(), int("4"))])),
        }]),
    }
}
fn run(
    ir: &WorkspaceIr,
    e: &Expression,
    c: &ObservationContext,
    b: &VerificationBudgets,
) -> Evaluation {
    evaluate_observed(
        ir,
        e,
        &BTreeMap::from([
            ("a".into(), VerificationValue::String("r".into())),
            ("b".into(), int("2")),
        ]),
        &[],
        c,
        b,
    )
}
fn unknown(value: Evaluation, code: &str) {
    assert!(
        matches!(&value,Evaluation::Unknown(issue) if issue.code==code),
        "{code}: {value:?}"
    );
}
#[test]
fn composite_identity_state_fields_and_actual_trace_counts_match_independent_reference_cases() {
    let (ir, expressions) = fixture();
    assert_eq!(expressions.len(), 4);
    let budgets = VerificationBudgets::default();
    let c = context();
    for expression in &expressions {
        assert_eq!(
            run(&ir, expression, &c, &budgets),
            Evaluation::Complete(VerificationValue::Boolean(true))
        );
    }
    let mut changed = c.clone();
    changed.after = state(vec![row("5")]);
    assert_eq!(
        run(&ir, &expressions[1], &changed, &budgets),
        Evaluation::Complete(VerificationValue::Boolean(false))
    );
    let mut absent = c.clone();
    absent.before = state(vec![]);
    absent.after = state(vec![]);
    for name in ["preserves", "creates", "updates", "removes", "exists"] {
        let mut expression = expressions[0].clone();
        if let ExpressionKind::Call { name: callee, .. } = &mut expression.value {
            *callee = format!("{name}:W.Item");
        }
        assert_eq!(
            run(&ir, &expression, &absent, &budgets),
            Evaluation::Complete(VerificationValue::Boolean(name == "preserves"))
        );
        if name == "updates" {
            assert_eq!(
                run(&ir, &expression, &c, &budgets),
                Evaluation::Complete(VerificationValue::Boolean(true))
            );
        }
    }
    for (before, after, name) in [
        (vec![], vec![row("4")], "creates"),
        (vec![row("3")], vec![], "removes"),
        (vec![row("3")], vec![row("3")], "preserves"),
    ] {
        let mut expression = expressions[0].clone();
        if let ExpressionKind::Call { name: callee, .. } = &mut expression.value {
            *callee = format!("{name}:W.Item");
        }
        let mut c = c.clone();
        c.before = state(before);
        c.after = state(after);
        assert_eq!(
            run(&ir, &expression, &c, &budgets),
            Evaluation::Complete(VerificationValue::Boolean(true))
        );
    }
    let mut zero = c.clone();
    zero.calls = Some(vec![]);
    zero.events = Some(vec![]);
    assert_eq!(
        run(&ir, &expressions[2], &zero, &budgets),
        Evaluation::Complete(VerificationValue::Boolean(false))
    );
    assert_eq!(
        run(&ir, &expressions[3], &zero, &budgets),
        Evaluation::Complete(VerificationValue::Boolean(false))
    );
    let mut twice = c.clone();
    twice
        .calls
        .as_mut()
        .unwrap()
        .push(c.calls.as_ref().unwrap()[0].clone());
    assert_eq!(
        run(&ir, &expressions[2], &twice, &budgets),
        Evaluation::Complete(VerificationValue::Boolean(false))
    );
}
#[test]
fn missing_ambiguous_invalid_or_budget_limited_observers_never_become_true() {
    let (mut ir, expressions) = fixture();
    let budgets = VerificationBudgets::default();
    let c = context();
    unknown(
        run(
            &ir,
            &expressions[0],
            &ObservationContext::default(),
            &budgets,
        ),
        "STATE_OBSERVER_REQUIRED",
    );
    for index in [2, 3] {
        unknown(
            run(
                &ir,
                &expressions[index],
                &ObservationContext::default(),
                &budgets,
            ),
            "TRACE_OBSERVER_REQUIRED",
        );
    }
    let mut duplicate = c.clone();
    duplicate.after = state(vec![row("4"), row("4")]);
    unknown(
        run(&ir, &expressions[1], &duplicate, &budgets),
        "AMBIGUOUS_STATE_IDENTITY",
    );
    let mut invalid = c.clone();
    invalid.after = state(vec![VerificationValue::Unit]);
    unknown(
        run(&ir, &expressions[1], &invalid, &budgets),
        "INVALID_VALUE",
    );
    invalid.after = state(vec![VerificationValue::Record(BTreeMap::new())]);
    unknown(
        run(&ir, &expressions[1], &invalid, &budgets),
        "INVALID_VALUE",
    );
    let limited = VerificationBudgets {
        evaluation_steps: 3,
        ..budgets.clone()
    };
    unknown(run(&ir, &expressions[0], &c, &limited), "EVALUATION_BUDGET");
    let mut expression = expressions[0].clone();
    if let ExpressionKind::Call { arguments, .. } = &mut expression.value {
        arguments.clear();
    }
    unknown(run(&ir, &expression, &c, &budgets), "STATE_KEY_ARITY");
    ir.data.get_mut("W.Item").unwrap().alias = Some(TypeRef::builtin("FutureBuiltin"));
    unknown(run(&ir, &expressions[0], &c, &budgets), "UNSUPPORTED_TYPE");
}

#[test]
fn state_collection_and_missing_keys_are_distinct_from_unavailable_observers() {
    let (ir, expressions) = fixture();
    let b = VerificationBudgets::default();
    let c = context();
    let mut state = expressions[0].clone();
    state.value = ExpressionKind::State {
        phase: StatePhase::After,
        entity: "W.Item".into(),
        keys: vec![],
    };
    assert_eq!(
        run(&ir, &state, &c, &b),
        Evaluation::Complete(VerificationValue::List(vec![row("4")]))
    );
    if let ExpressionKind::State { keys, .. } = &mut state.value {
        keys.push(Expression {
            ty: TypeRef::builtin("String"),
            span: state.span.clone(),
            value: ExpressionKind::Variable { name: "a".into() },
        });
    }
    unknown(run(&ir, &state, &c, &b), "STATE_KEY_ARITY");
    if let ExpressionKind::State { keys, .. } = &mut state.value {
        keys.push(Expression {
            ty: TypeRef::builtin("Int"),
            span: state.span.clone(),
            value: ExpressionKind::Variable { name: "b".into() },
        });
    }
    let mut absent = c.clone();
    absent.after = state_map_empty();
    unknown(run(&ir, &state, &absent, &b), "STATE_ENTITY_MISSING");
    let mut missing = ir.clone();
    missing.data.remove("W.Item");
    unknown(run(&missing, &state, &c, &b), "STATE_DECLARATION");
    let mut unsupported = expressions[0].clone();
    if let ExpressionKind::Call { name, .. } = &mut unsupported.value {
        *name = "unavailable-predicate:W.Item".into();
    }
    unknown(run(&ir, &unsupported, &c, &b), "STATE_PREDICATE");
    let mut label = expressions[0].clone();
    if let ExpressionKind::Call { name, .. } = &mut label.value {
        *name = "exists:unrelated-entity".into();
    }
    unknown(run(&ir, &label, &c, &b), "IMPURE_OPERATION");
}
fn state_map_empty() -> Option<BTreeMap<String, Vec<VerificationValue>>> {
    state(vec![])
}

#[test]
fn trace_targets_counts_arguments_and_traversal_budgets_require_actual_valid_evidence() {
    let (ir, expressions) = fixture();
    let b = VerificationBudgets::default();
    let c = context();
    let mut unmodeled = expressions[2].clone();
    if let ExpressionKind::Trace { modeled_target, .. } = &mut unmodeled.value {
        *modeled_target = None;
    }
    unknown(run(&ir, &unmodeled, &c, &b), "TRACE_OBSERVER_REQUIRED");
    for (kind, target, code) in [
        ("calls", "missing", "TRACE_TARGET"),
        ("emits", "W.Item", "TRACE_TARGET"),
        ("reads", "W.Item", "TRACE_KIND"),
    ] {
        let mut trace = expressions[2].clone();
        if let ExpressionKind::Trace {
            trace_kind,
            modeled_target,
            ..
        } = &mut trace.value
        {
            *trace_kind = kind.into();
            *modeled_target = Some(target.into());
        }
        unknown(run(&ir, &trace, &c, &b), code);
    }
    for (text, kind) in [("-1", "Int"), ("1.5", "Decimal")] {
        let mut trace = expressions[2].clone();
        if let ExpressionKind::Trace { count, .. } = &mut trace.value {
            count.value = ExpressionKind::Literal {
                text: text.into(),
                literal_kind: kind.into(),
            };
        }
        unknown(run(&ir, &trace, &c, &b), "INVALID_VALUE");
    }
    let mut event = expressions[3].clone();
    let payload = Expression {
        ty: TypeRef::builtin("Int"),
        span: event.span.clone(),
        value: ExpressionKind::Literal {
            text: "4".into(),
            literal_kind: "Int".into(),
        },
    };
    if let ExpressionKind::Trace { arguments, .. } = &mut event.value {
        arguments.extend([payload.clone(), payload]);
    }
    unknown(run(&ir, &event, &c, &b), "INVALID_VALUE");
    if let ExpressionKind::Trace { arguments, .. } = &mut event.value {
        arguments.truncate(1);
    }
    assert_eq!(
        run(&ir, &event, &c, &b),
        Evaluation::Complete(VerificationValue::Boolean(false))
    );
    let mut invalid_call = c.clone();
    invalid_call.calls.as_mut().unwrap()[0].invocation.arguments[1] = int("not-an-integer");
    unknown(
        run(&ir, &expressions[2], &invalid_call, &b),
        "INVALID_VALUE",
    );
    let mut many = c.clone();
    many.calls = Some(vec![c.calls.as_ref().unwrap()[0].clone(); 10]);
    many.events = Some(vec![c.events.as_ref().unwrap()[0].clone(); 10]);
    let limited = VerificationBudgets {
        evaluation_steps: 8,
        ..b
    };
    unknown(
        run(&ir, &expressions[2], &many, &limited),
        "EVALUATION_BUDGET",
    );
    unknown(
        run(&ir, &expressions[3], &many, &limited),
        "EVALUATION_BUDGET",
    );
}

#[test]
fn malformed_entity_keys_and_failure_declarations_cannot_forge_state_or_failure_identity() {
    let (mut ir, expressions) = fixture();
    let b = VerificationBudgets::default();
    let c = context();
    ir.data.get_mut("W.Item").unwrap().keys[0] = "undeclared-key".into();
    unknown(run(&ir, &expressions[0], &c, &b), "INVALID_VALUE");
    let expression = Expression {
        ty: TypeRef::builtin("Boolean"),
        span: expressions[0].span.clone(),
        value: ExpressionKind::Binary {
            operator: "is".into(),
            left: Box::new(Expression {
                ty: TypeRef::builtin("Sample"),
                span: expressions[0].span.clone(),
                value: ExpressionKind::Variable {
                    name: "failure".into(),
                },
            }),
            right: Box::new(Expression {
                ty: TypeRef::builtin("Sample"),
                span: expressions[0].span.clone(),
                value: ExpressionKind::Variable {
                    name: "expected".into(),
                },
            }),
        },
    };
    for (expected, matches) in [("W.Broken", true), ("W.Other", false)] {
        let vars = BTreeMap::from([
            (
                "failure".into(),
                VerificationValue::Failure {
                    declaration: "W.Broken".into(),
                    payload: None,
                },
            ),
            (
                "expected".into(),
                VerificationValue::Variant {
                    declaration: expected.into(),
                    name: "Broken".into(),
                    payload: None,
                },
            ),
        ]);
        assert_eq!(
            evaluate_observed(&ir, &expression, &vars, &[], &c, &b),
            Evaluation::Complete(VerificationValue::Boolean(matches))
        );
    }
}
