use dctx0_core::*;
use dctx0_interpreter::{Evaluation as O, evaluate};
use std::collections::BTreeMap;

fn ir() -> WorkspaceIr {
    let compiled = dctx0_language::compile(&[Source { file: "reference.dctx0".into(), text: "workspace W { surface M { operation identity(value: Int) -> Int { pure } operation write(value: Int) -> Int {} } }".into() }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    compiled.ir.unwrap()
}
fn expr(value: ExpressionKind) -> Expression {
    Expression {
        ty: TypeRef::builtin("Int"),
        span: Span {
            file: "reference.dctx0".into(),
            start: Position {
                offset: 0,
                line: 1,
                column: 1,
            },
            end: Position {
                offset: 1,
                line: 1,
                column: 2,
            },
        },
        value,
    }
}
fn lit(kind: &str, text: &str) -> Expression {
    expr(ExpressionKind::Literal {
        text: text.into(),
        literal_kind: kind.into(),
    })
}
fn int(text: &str) -> Expression {
    lit("Int", text)
}
fn bool_expr(value: bool) -> Expression {
    lit("Boolean", if value { "true" } else { "false" })
}
fn binary(op: &str, a: Expression, b: Expression) -> Expression {
    expr(ExpressionKind::Binary {
        operator: op.into(),
        left: Box::new(a),
        right: Box::new(b),
    })
}
fn list(values: Vec<Expression>) -> Expression {
    expr(ExpressionKind::List { values })
}
fn call(name: &str, arguments: Vec<Expression>) -> Expression {
    expr(ExpressionKind::Call {
        name: name.into(),
        target: None,
        arguments,
    })
}
fn var(name: &str) -> Expression {
    expr(ExpressionKind::Variable { name: name.into() })
}
fn run(e: &Expression) -> O {
    evaluate(
        &ir(),
        e,
        &BTreeMap::new(),
        &[],
        &VerificationBudgets::default(),
    )
}
fn integer(text: &str) -> VerificationValue {
    VerificationValue::Integer(text.into())
}
fn expect(e: Expression, value: VerificationValue) {
    assert_eq!(run(&e), O::Complete(value));
}
fn unknown(e: Expression, code: &str) {
    let O::Unknown(issue) = run(&e) else {
        panic!("Expected UNKNOWN: {:?}", run(&e));
    };
    assert_eq!(issue.code, code);
    assert_eq!(issue.span.unwrap().file, "reference.dctx0");
}

#[test]
fn mean_preserves_exact_cancellation_and_reports_unrepresentable_float_results() {
    let magnitude = "9".repeat(400);
    let expression = call("mean", vec![var("values")]);
    let run_values = |values| {
        evaluate(
            &ir(),
            &expression,
            &BTreeMap::from([("values".into(), VerificationValue::List(values))]),
            &[],
            &Default::default(),
        )
    };
    assert_eq!(
        run_values(vec![integer(&magnitude), integer(&format!("-{magnitude}"))]),
        O::Complete(VerificationValue::Option(Some(Box::new(
            VerificationValue::Float("0".into())
        ))))
    );
    let O::Unknown(issue) = run_values(vec![integer(&magnitude)]) else {
        panic!("An unrepresentable mean must not silently become infinity");
    };
    assert_eq!(issue.code, "FLOAT_REPRESENTATION_REQUIRED");
    assert_eq!(issue.span.unwrap().file, "reference.dctx0");
}

#[test]
fn list_search_and_mean_budget_exhaustion_do_not_return_partial_results() {
    let variables = BTreeMap::from([
        (
            "values".into(),
            VerificationValue::List(vec![integer("1"); 32]),
        ),
        ("needle".into(), integer("9")),
    ]);
    for (name, arguments, expected, message) in [
        (
            "mean",
            vec![var("values")],
            VerificationValue::Option(Some(Box::new(VerificationValue::Float("1".into())))),
            "Mean",
        ),
        (
            "indexOf",
            vec![var("values"), var("needle")],
            integer("-1"),
            "List search",
        ),
    ] {
        let expression = call(name, arguments);
        assert_eq!(
            evaluate(&ir(), &expression, &variables, &[], &Default::default()),
            O::Complete(expected)
        );
        let O::Unknown(issue) = evaluate(
            &ir(),
            &expression,
            &variables,
            &[],
            &VerificationBudgets {
                evaluation_steps: 16,
                ..Default::default()
            },
        ) else {
            panic!("{name} must report an exhausted mandatory evaluation");
        };
        assert_eq!(issue.code, "EVALUATION_BUDGET");
        assert!(issue.message.contains(message));
        assert_eq!(issue.span.unwrap().file, "reference.dctx0");
    }
}
fn collection(
    op: &str,
    values: Vec<Expression>,
    binder: Option<&str>,
    predicate: Option<Expression>,
    projection: Option<Expression>,
) -> Expression {
    expr(ExpressionKind::Collection {
        operator: op.into(),
        binder: binder.map(str::to_owned),
        collection: Box::new(list(values)),
        predicate: predicate.map(Box::new),
        projection: projection.map(Box::new),
    })
}

#[test]
fn unsupported_sort_keys_and_incomplete_comparisons_never_return_a_partial_order() {
    unknown(
        collection(
            "sortBy",
            vec![bool_expr(true), bool_expr(false)],
            Some("item"),
            None,
            Some(var("item")),
        ),
        "UNSUPPORTED_SORT_KEY",
    );
    unknown(
        collection(
            "sortBy",
            vec![int("1"), lit("String", "a")],
            Some("item"),
            None,
            Some(var("item")),
        ),
        "INCOMPATIBLE_SORT_KEYS",
    );
    let expression = expr(ExpressionKind::Collection {
        operator: "sortBy".into(),
        binder: Some("item".into()),
        collection: Box::new(var("values")),
        predicate: None,
        projection: Some(Box::new(var("item"))),
    });
    let variables = BTreeMap::from([(
        "values".into(),
        VerificationValue::List((0..16).rev().map(|n| integer(&n.to_string())).collect()),
    )]);
    assert_eq!(
        evaluate(&ir(), &expression, &variables, &[], &Default::default()),
        O::Complete(VerificationValue::List(
            (0..16).map(|n| integer(&n.to_string())).collect()
        ))
    );
    let O::Unknown(issue) = evaluate(
        &ir(),
        &expression,
        &variables,
        &[],
        &VerificationBudgets {
            evaluation_steps: 40,
            ..Default::default()
        },
    ) else {
        panic!("An incomplete sort cannot establish an ordering");
    };
    assert_eq!(issue.code, "EVALUATION_BUDGET");
    assert!(issue.message.contains("Sorting"));
    assert_eq!(issue.span.unwrap().file, "reference.dctx0");
}

#[test]
fn exact_arithmetic_matches_independent_reference_vectors() {
    for (op, a, b, answer) in [
        ("+", "9007199254740993", "2", "9007199254740995"),
        ("-", "3", "8", "-5"),
        ("*", "-5", "2", "-10"),
        ("/", "-7", "3", "-2"),
        ("%", "-7", "3", "-1"),
    ] {
        expect(binary(op, int(a), int(b)), integer(answer));
    }
    for (op, a, b, answer) in [
        ("+", "0.1", "0.2", "0.3"),
        ("-", "1.20", "0.3", "0.9"),
        ("*", "0.01", "0.02", "0.0002"),
        ("/", "1.0", "8.0", "0.125"),
        ("+", "-0.1", "-0.20", "-0.3"),
        ("+", "1.0", "1.00", "2"),
    ] {
        expect(
            binary(op, lit("Decimal", a), lit("Decimal", b)),
            VerificationValue::Decimal(answer.into()),
        );
    }
    expect(
        binary("/", lit("Decimal", "1"), lit("Decimal", "5")),
        VerificationValue::Decimal("0.2".into()),
    );
    expect(
        binary("/", lit("Decimal", "1"), lit("Decimal", "40")),
        VerificationValue::Decimal("0.025".into()),
    );
    expect(
        binary("+", lit("Decimal", "0"), lit("Decimal", "-0.0")),
        VerificationValue::Decimal("0".into()),
    );
    unknown(
        binary("/", lit("Decimal", "1"), lit("Decimal", "3")),
        "NONTERMINATING_DECIMAL",
    );
    unknown(
        binary("%", lit("Decimal", "1.2"), lit("Decimal", "0.5")),
        "UNSUPPORTED_OPERATOR",
    );
    for op in ["/", "%"] {
        unknown(binary(op, int("1"), int("0")), "DIVISION_BY_ZERO");
    }
    unknown(
        binary("mystery", int("1"), int("2")),
        "UNSUPPORTED_OPERATOR",
    );
    for text in ["", "-", "a", "1..2", ".1", "1.", "+1", "1e2", "1.2", " 1"] {
        unknown(int(text), "INVALID_VALUE");
    }
    unknown(int(&"9".repeat(65537)), "INVALID_VALUE");
    unknown(lit("Decimal", "1.2a"), "INVALID_VALUE");
    expect(lit("Float", "1e2"), VerificationValue::Float("1e2".into()));
    unknown(lit("Float", "NaN"), "INVALID_VALUE");
    unknown(lit("Timestamp", "now"), "UNSUPPORTED_LITERAL");
    unknown(lit("Boolean", "yes"), "INVALID_VALUE");
}

#[test]
fn logical_comparison_membership_and_ranges_preserve_short_circuits() {
    for (op, left, right, expected) in [
        ("and", false, true, false),
        ("and", true, false, false),
        ("or", true, false, true),
        ("or", false, true, true),
        ("implies", false, false, true),
        ("implies", true, false, false),
    ] {
        expect(
            binary(op, bool_expr(left), bool_expr(right)),
            VerificationValue::Boolean(expected),
        );
    }
    for (op, value) in [("and", false), ("or", true), ("implies", false)] {
        expect(
            binary(op, bool_expr(value), var("absent")),
            VerificationValue::Boolean(op != "and"),
        );
    }
    unknown(binary("and", int("1"), bool_expr(true)), "INVALID_VALUE");
    unknown(binary("and", bool_expr(true), int("1")), "INVALID_VALUE");
    for (op, a, b, answer) in [
        ("==", "1", "1.00", true),
        ("!=", "1", "1.01", true),
        ("<", "1", "2", true),
        ("<=", "2", "2", true),
        (">", "3", "2", true),
        (">=", "2", "3", false),
    ] {
        expect(
            binary(op, int(a), lit("Decimal", b)),
            VerificationValue::Boolean(answer),
        );
    }
    expect(
        binary("..", int("-1"), int("1")),
        VerificationValue::List(vec![integer("-1"), integer("0"), integer("1")]),
    );
    unknown(binary("..", int("2"), int("1")), "INVALID_VALUE");
    unknown(
        binary("..", lit("Decimal", "0.5"), int("1")),
        "INVALID_VALUE",
    );
    unknown(binary("..", int("0"), int("999999")), "EVALUATION_BUDGET");
    unknown(
        binary("..", int("0"), int("99999999999999999999999999")),
        "INVALID_VALUE",
    );
    for (needle, answer) in [("1", true), ("2", false)] {
        expect(
            binary("in", int(needle), list(vec![int("1")])),
            VerificationValue::Boolean(answer),
        );
    }
    unknown(binary("in", int("1"), int("2")), "INVALID_VALUE");
    unknown(binary("+", lit("String", "a"), int("1")), "INVALID_VALUE");
    expect(
        binary("+", lit("String", "hello"), lit("String", " world")),
        VerificationValue::String("hello world".into()),
    );
    unknown(
        binary("-", lit("String", "a"), lit("String", "b")),
        "INVALID_VALUE",
    );
    expect(
        expr(ExpressionKind::Unary {
            operator: "-".into(),
            operand: Box::new(int("1")),
        }),
        integer("-1"),
    );
    expect(
        expr(ExpressionKind::Unary {
            operator: "not".into(),
            operand: Box::new(bool_expr(false)),
        }),
        VerificationValue::Boolean(true),
    );
    unknown(
        expr(ExpressionKind::Unary {
            operator: "?".into(),
            operand: Box::new(int("1")),
        }),
        "UNSUPPORTED_OPERATOR",
    );
}

#[test]
fn builtin_vectors_cover_strings_options_and_list_translation() {
    for (name, source, answer) in [
        ("lowercase", "HeLLo", "hello"),
        ("uppercase", "Hi", "HI"),
        ("trim", "  x \n", "x"),
    ] {
        expect(
            call(name, vec![lit("String", source)]),
            VerificationValue::String(answer.into()),
        );
    }
    expect(call("length", vec![lit("String", "a😀")]), integer("3"));
    expect(
        call("length", vec![list(vec![int("1"), int("2")])]),
        integer("2"),
    );
    expect(
        call(
            "startsWith",
            vec![lit("String", "hello"), lit("String", "he")],
        ),
        VerificationValue::Boolean(true),
    );
    expect(
        call("endsWith", vec![lit("String", "hello"), lit("String", "x")]),
        VerificationValue::Boolean(false),
    );
    expect(
        call("reverse", vec![list(vec![int("1"), int("2")])]),
        VerificationValue::List(vec![integer("2"), integer("1")]),
    );
    expect(call("None", vec![]), VerificationValue::Option(None));
    expect(
        call(
            "ifElse",
            vec![bool_expr(true), int("7"), binary("/", int("1"), int("0"))],
        ),
        integer("7"),
    );
    expect(
        call(
            "ifElse",
            vec![bool_expr(false), binary("/", int("1"), int("0")), int("8")],
        ),
        integer("8"),
    );
    expect(
        binary(">=", lit("Float", "4.5"), int("4")),
        VerificationValue::Boolean(true),
    );
    expect(
        binary("<", lit("Float", "4.5"), int("4")),
        VerificationValue::Boolean(false),
    );
    expect(
        binary("==", lit("Float", "1e0"), int("1")),
        VerificationValue::Boolean(true),
    );
    expect(
        call(
            "includesText",
            vec![lit("String", "Cinéma"), lit("String", "né")],
        ),
        VerificationValue::Boolean(true),
    );
    expect(
        call(
            "includesText",
            vec![lit("String", "Cinema"), lit("String", "cinema")],
        ),
        VerificationValue::Boolean(false),
    );
    for (offset, limit, expected) in [
        ("1", "1", vec![integer("2")]),
        ("0", "0", vec![]),
        ("99", "1", vec![]),
        (
            "0",
            "9007199254740993",
            vec![integer("1"), integer("2"), integer("3")],
        ),
        ("9007199254740993", "1", vec![]),
    ] {
        expect(
            call(
                "slice",
                vec![
                    list(vec![int("1"), int("2"), int("3")]),
                    int(offset),
                    int(limit),
                ],
            ),
            VerificationValue::List(expected),
        );
    }
    unknown(
        call("slice", vec![list(vec![int("1")]), int("-1"), int("1")]),
        "SLICE_INDEX_REQUIRED",
    );
    expect(call("single", vec![list(vec![int("4")])]), integer("4"));
    unknown(
        call("single", vec![list(vec![])]),
        "RESULT_CARDINALITY_REQUIRED",
    );
    unknown(
        call("single", vec![list(vec![int("1"), int("2")])]),
        "RESULT_CARDINALITY_REQUIRED",
    );
    expect(
        call("mean", vec![list(vec![int("4"), int("5")])]),
        VerificationValue::Option(Some(Box::new(VerificationValue::Float("4.5".into())))),
    );
    expect(
        call("mean", vec![list(vec![])]),
        VerificationValue::Option(None),
    );
    expect(
        call(
            "getOrElse",
            vec![call("mean", vec![list(vec![])]), lit("Float", "0e0")],
        ),
        VerificationValue::Float("0e0".into()),
    );
    expect(
        call("indexOf", vec![list(vec![int("4"), int("5")]), int("5")]),
        integer("1"),
    );
    expect(
        call("indexOf", vec![list(vec![int("4"), int("5")]), int("9")]),
        integer("-1"),
    );
    expect(
        binary("==", lit("Float", "0e0"), lit("Float", "0")),
        VerificationValue::Boolean(true),
    );
    expect(
        call("first", vec![list(vec![])]),
        VerificationValue::Option(None),
    );
    expect(
        call("first", vec![list(vec![int("2"), int("1")])]),
        VerificationValue::Option(Some(Box::new(integer("2")))),
    );
    unknown(call("first", vec![int("1")]), "UNSUPPORTED_BUILTIN");
    expect(
        call("Some", vec![int("1")]),
        VerificationValue::Option(Some(Box::new(integer("1")))),
    );
    unknown(call("round", vec![]), "UNSUPPORTED_BUILTIN");
    unknown(call("trim", vec![int("1")]), "UNSUPPORTED_BUILTIN");
}

#[test]
fn unicode_trim_matches_the_independent_typescript_reference() {
    for (input, expected) in [
        ("\u{00a0}\u{feff}x\u{feff}\u{00a0}", "x"),
        ("\u{0085}x\u{0085}", "\u{0085}x\u{0085}"),
        ("\u{feff}", ""),
        ("\u{180e}x\u{180e}", "\u{180e}x\u{180e}"),
    ] {
        expect(
            call("trim", vec![lit("String", input)]),
            VerificationValue::String(expected.into()),
        );
    }
}

#[test]
fn records_variants_and_nested_equality_are_lossless() {
    let record = |values| {
        expr(ExpressionKind::Record {
            declaration: "R".into(),
            fields: values,
        })
    };
    let variant = |name: &str, payload: Expression| {
        expr(ExpressionKind::VariantRecord {
            declaration: "Choice".into(),
            variant: name.into(),
            payload: Box::new(payload),
        })
    };
    let bare = |declaration: &str, name: &str| {
        expr(ExpressionKind::Variant {
            declaration: declaration.into(),
            name: name.into(),
        })
    };
    expect(
        expr(ExpressionKind::Field {
            target: Box::new(record(vec![("x".into(), int("2"))])),
            name: "x".into(),
        }),
        integer("2"),
    );
    expect(
        expr(ExpressionKind::Field {
            target: Box::new(variant("A", int("2"))),
            name: "value".into(),
        }),
        integer("2"),
    );
    unknown(
        expr(ExpressionKind::Field {
            target: Box::new(record(vec![])),
            name: "x".into(),
        }),
        "INVALID_VALUE",
    );
    unknown(
        expr(ExpressionKind::Field {
            target: Box::new(int("2")),
            name: "x".into(),
        }),
        "INVALID_VALUE",
    );
    unknown(
        record(vec![("x".into(), int("1")), ("x".into(), int("2"))]),
        "INVALID_VALUE",
    );
    for (left, right, answer) in [
        (
            list(vec![int("1")]),
            list(vec![lit("Decimal", "1.0")]),
            true,
        ),
        (list(vec![int("1")]), list(vec![]), false),
        (list(vec![int("1")]), list(vec![int("2")]), false),
        (
            record(vec![("x".into(), int("1"))]),
            record(vec![("x".into(), lit("Decimal", "1.0"))]),
            true,
        ),
        (record(vec![("x".into(), int("1"))]), record(vec![]), false),
        (
            record(vec![("x".into(), int("1"))]),
            record(vec![("x".into(), int("2"))]),
            false,
        ),
        (
            call("Some", vec![int("1")]),
            call("Some", vec![lit("Decimal", "1.0")]),
            true,
        ),
        (
            variant("A", int("1")),
            variant("A", lit("Decimal", "1.0")),
            true,
        ),
        (variant("A", int("1")), variant("B", int("1")), false),
        (variant("A", int("1")), bare("Choice", "A"), false),
        (bare("Choice", "A"), bare("Choice", "A"), true),
        (bare("Choice", "A"), bare("Other", "A"), false),
        (bool_expr(true), bool_expr(false), false),
    ] {
        expect(
            binary("==", left, right),
            VerificationValue::Boolean(answer),
        );
    }
    expect(
        binary("is", variant("A", int("2")), bare("Choice", "A")),
        VerificationValue::Boolean(true),
    );
    expect(
        binary("is", int("1"), int("1")),
        VerificationValue::Boolean(true),
    );
    unknown(var("missing"), "UNBOUND_VALUE");
}

#[test]
fn collections_bind_locally_and_cover_empty_and_unknown_aggregates() {
    expect(
        collection(
            "sortDescBy",
            vec![int("1"), int("3"), int("2")],
            Some("x"),
            None,
            Some(var("x")),
        ),
        VerificationValue::List(vec![integer("3"), integer("2"), integer("1")]),
    );
    expect(
        collection(
            "sortBy",
            vec![int("3"), int("1"), int("2")],
            Some("x"),
            None,
            Some(var("x")),
        ),
        VerificationValue::List(vec![integer("1"), integer("2"), integer("3")]),
    );
    expect(
        collection(
            "sortBy",
            vec![lit("Float", "3e0"), lit("Float", "1e0")],
            Some("x"),
            None,
            Some(var("x")),
        ),
        VerificationValue::List(vec![
            VerificationValue::Float("1e0".into()),
            VerificationValue::Float("3e0".into()),
        ]),
    );
    let predicate = || binary(">", var("x"), int("1"));
    for (op, answer) in [
        ("count", integer("1")),
        ("all", VerificationValue::Boolean(false)),
        ("any", VerificationValue::Boolean(true)),
    ] {
        expect(
            collection(
                op,
                vec![int("1"), int("2")],
                Some("x"),
                Some(predicate()),
                None,
            ),
            answer,
        );
    }
    expect(
        collection("count", vec![int("1")], None, None, None),
        integer("1"),
    );
    for (op, values, answer) in [
        ("all", vec![bool_expr(true)], true),
        ("any", vec![bool_expr(false)], false),
        ("all", vec![], true),
        ("any", vec![], false),
    ] {
        expect(
            collection(op, values, None, None, None),
            VerificationValue::Boolean(answer),
        );
    }
    expect(
        collection(
            "sum",
            vec![int("1"), int("2")],
            Some("x"),
            None,
            Some(binary("*", var("x"), int("2"))),
        ),
        integer("6"),
    );
    expect(
        collection(
            "sum",
            vec![lit("Decimal", "0.1"), lit("Decimal", "0.2")],
            None,
            None,
            None,
        ),
        VerificationValue::Decimal("0.3".into()),
    );
    expect(collection("sum", vec![], None, None, None), integer("0"));
    expect(
        collection(
            "distinct",
            vec![int("1"), int("2"), lit("Decimal", "1.0")],
            None,
            None,
            None,
        ),
        VerificationValue::List(vec![integer("1"), integer("2")]),
    );
    for (op, answer) in [("min", "1"), ("max", "3")] {
        expect(
            collection(op, vec![int("2"), int("1"), int("3")], None, None, None),
            integer(answer),
        );
    }
    unknown(
        collection("min", vec![], None, None, None),
        "EMPTY_AGGREGATE",
    );
    unknown(
        collection("mystery", vec![], None, None, None),
        "UNSUPPORTED_COLLECTION",
    );
    for (needle, answer) in [("1", true), ("3", false)] {
        expect(
            collection(
                "contains",
                vec![int("1"), int("2")],
                None,
                Some(int(needle)),
                None,
            ),
            VerificationValue::Boolean(answer),
        );
    }
    unknown(
        collection("contains", vec![], None, None, None),
        "INVALID_VALUE",
    );
    unknown(
        expr(ExpressionKind::Collection {
            operator: "sum".into(),
            binder: None,
            collection: Box::new(int("1")),
            predicate: None,
            projection: None,
        }),
        "INVALID_VALUE",
    );
    unknown(
        collection("all", vec![int("1")], None, None, None),
        "INVALID_VALUE",
    );
}

#[test]
fn suspended_calls_require_ordered_matching_observations_and_real_pure_operations() {
    let ir = ir();
    let call = |subject: &str| {
        expr(ExpressionKind::Call {
            name: subject.into(),
            target: Some(subject.into()),
            arguments: vec![int("3")],
        })
    };
    let expression = binary("+", call("W.M.identity"), call("W.M.identity"));
    let invocation = Invocation {
        subject: "W.M.identity".into(),
        arguments: vec![integer("3")],
    };
    let returned = InvocationObservation {
        invocation: invocation.clone(),
        observation: Observation::Returned {
            value: integer("4"),
        },
    };
    let eval = |observations: &[InvocationObservation]| {
        evaluate(
            &ir,
            &expression,
            &BTreeMap::new(),
            observations,
            &VerificationBudgets::default(),
        )
    };
    assert_eq!(eval(&[]), O::Invoke(invocation.clone()));
    assert_eq!(
        eval(std::slice::from_ref(&returned)),
        O::Invoke(invocation.clone())
    );
    assert_eq!(
        eval(&[returned.clone(), returned.clone()]),
        O::Complete(integer("8"))
    );
    assert!(
        matches!(eval(&[returned.clone(), returned.clone(), returned.clone()]), O::Unknown(i) if i.code == "OBSERVATION_MISMATCH")
    );
    let wrong = InvocationObservation {
        invocation: Invocation {
            subject: "other".into(),
            arguments: vec![],
        },
        observation: returned.observation.clone(),
    };
    assert!(matches!(eval(&[wrong]), O::Unknown(i) if i.code == "OBSERVATION_MISMATCH"));
    for observation in [
        Observation::Threw {
            message: "boom".into(),
        },
        Observation::Unavailable {
            reason: "no adapter".into(),
        },
    ] {
        let outcome = eval(&[InvocationObservation {
            invocation: invocation.clone(),
            observation,
        }]);
        assert!(
            matches!(&outcome, O::Unknown(i) if i.code == "APPLICATION_EXCEPTION")
                || matches!(&outcome, O::Unknown(i) if i.code == "OPERATION_UNAVAILABLE")
        );
    }
    unknown(call("W.M.write"), "IMPURE_OPERATION");
    unknown(call("missing"), "IMPURE_OPERATION");
}

#[test]
fn budgets_invalid_observations_and_unobservable_state_never_become_true() {
    let ir = ir();
    let e = var("input");
    let outcome = |value: VerificationValue, budgets: VerificationBudgets| {
        evaluate(
            &ir,
            &e,
            &BTreeMap::from([("input".into(), value)]),
            &[],
            &budgets,
        )
    };
    for value in [
        VerificationValue::Record(BTreeMap::from([("x".into(), integer("bad"))])),
        VerificationValue::List(vec![integer("bad")]),
        VerificationValue::Option(Some(Box::new(integer("bad")))),
        VerificationValue::Failure {
            declaration: "E".into(),
            payload: Some(Box::new(integer("bad"))),
        },
    ] {
        assert!(
            matches!(outcome(value, VerificationBudgets::default()), O::Unknown(i) if i.code == "INVALID_VALUE")
        );
    }
    for value in [
        VerificationValue::Unit,
        VerificationValue::Option(None),
        VerificationValue::Failure {
            declaration: "E".into(),
            payload: None,
        },
    ] {
        assert!(matches!(
            outcome(value, VerificationBudgets::default()),
            O::Complete(_)
        ));
    }
    assert!(
        matches!(outcome(VerificationValue::List(vec![integer("1")]), VerificationBudgets { recursion_depth: 0, ..Default::default() }), O::Unknown(i) if i.code == "VALUE_DEPTH")
    );
    assert!(
        matches!(outcome(VerificationValue::String("long".into()), VerificationBudgets { value_bytes: 1, ..Default::default() }), O::Unknown(i) if i.code == "VALUE_SIZE")
    );
    assert!(
        matches!(outcome(integer("1"), VerificationBudgets { evaluation_steps: 0, ..Default::default() }), O::Unknown(i) if i.code == "EVALUATION_BUDGET")
    );
    assert!(
        matches!(evaluate(&ir, &list(vec![int("1")]), &BTreeMap::new(), &[], &VerificationBudgets { recursion_depth: 0, ..Default::default() }), O::Unknown(i) if i.code == "EVALUATION_BUDGET")
    );
    assert!(
        matches!(evaluate(&ir, &collection("count", vec![int("1")], None, None, None), &BTreeMap::new(), &[], &VerificationBudgets { evaluation_steps: 3, ..Default::default() }), O::Unknown(i) if i.code == "EVALUATION_BUDGET")
    );
    unknown(
        expr(ExpressionKind::State {
            phase: StatePhase::Before,
            entity: "E".into(),
            keys: vec![],
        }),
        "STATE_OBSERVER_REQUIRED",
    );
    for modeled_target in [None, Some("E".into())] {
        unknown(
            expr(ExpressionKind::Trace {
                trace_kind: "calls".into(),
                target: "opaque".into(),
                modeled_target,
                arguments: vec![],
                count: Box::new(int("1")),
            }),
            "TRACE_OBSERVER_REQUIRED",
        );
    }
    assert_eq!(VerificationBudgets::default().seed, 42);
    for value in [
        VerificationValue::Unit,
        VerificationValue::Boolean(true),
        integer("9007199254740993"),
        VerificationValue::Decimal("0.1".into()),
        VerificationValue::Record(BTreeMap::new()),
        VerificationValue::List(vec![]),
        VerificationValue::Option(None),
        VerificationValue::Variant {
            declaration: "T".into(),
            name: "A".into(),
            payload: None,
        },
        VerificationValue::Failure {
            declaration: "E".into(),
            payload: None,
        },
    ] {
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<VerificationValue>(&json).unwrap(),
            value
        );
    }
}

#[test]
fn compiled_postcondition_uses_actual_result_and_keeps_semantic_ir_unchanged() {
    let compiled = dctx0_language::compile(&[Source {
        file: "postcondition.dctx0".into(),
        text: "workspace W { surface M { operation twice(x: Int) -> Int {\n pure\n ensures result == x * 2\n } } }".into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let original = serde_json::to_string(&ir).unwrap();
    let condition = &ir
        .nodes
        .iter()
        .find(|node| node.kind == StatementKind::Ensures)
        .unwrap()
        .expressions[0];
    for (result, expected) in [("8", true), ("9", false)] {
        let variables = BTreeMap::from([
            ("x".into(), integer("4")),
            ("result".into(), integer(result)),
        ]);
        assert_eq!(
            evaluate(
                &ir,
                condition,
                &variables,
                &[],
                &VerificationBudgets::default()
            ),
            O::Complete(VerificationValue::Boolean(expected))
        );
    }
    assert_eq!(serde_json::to_string(&ir).unwrap(), original);
}

fn rounding_mode(name: &str) -> Expression {
    expr(ExpressionKind::Variant {
        name: name.into(),
        declaration: "RoundingMode".into(),
    })
}
#[test]
fn decimal_rounding_uses_exact_signed_ties_and_all_four_modes() {
    for (input, mode, output) in [
        ("2.345", "Down", "2.34"),
        ("2.341", "Up", "2.35"),
        ("2.345", "HalfUp", "2.35"),
        ("2.345", "HalfEven", "2.34"),
        ("2.355", "HalfEven", "2.36"),
        ("-2.345", "HalfUp", "-2.35"),
        ("-2.345", "HalfEven", "-2.34"),
        ("-2.341", "Up", "-2.35"),
        ("-2.349", "Down", "-2.34"),
        ("1", "Up", "1"),
        ("9007199254740993.125", "HalfEven", "9007199254740993.12"),
    ] {
        expect(
            call(
                "round",
                vec![lit("Decimal", input), int("2"), rounding_mode(mode)],
            ),
            VerificationValue::Decimal(output.into()),
        );
    }
    for (left, right, mode, output) in [
        ("1", "3", "Down", "0.33"),
        ("-1", "3", "Up", "-0.34"),
        ("1", "8", "Down", "0.125"),
    ] {
        expect(
            call(
                "divide",
                vec![
                    lit("Decimal", left),
                    lit("Decimal", right),
                    int("2"),
                    rounding_mode(mode),
                ],
            ),
            VerificationValue::Decimal(output.into()),
        );
    }
    unknown(
        call(
            "divide",
            vec![int("1"), int("0"), int("2"), rounding_mode("Down")],
        ),
        "DIVISION_BY_ZERO",
    );
    unknown(
        call("round", vec![int("1"), int("-1"), rounding_mode("Down")]),
        "INVALID_VALUE",
    );
    unknown(
        call(
            "round",
            vec![int("1"), int("1000000"), rounding_mode("Down")],
        ),
        "EVALUATION_BUDGET",
    );
}

#[test]
fn exact_division_does_not_bypass_rounding_mode_validation() {
    for expression in [
        call(
            "round",
            vec![
                lit("Decimal", "2.345"),
                int("2"),
                rounding_mode("FutureMode"),
            ],
        ),
        call(
            "divide",
            vec![int("1"), int("8"), int("2"), rounding_mode("FutureMode")],
        ),
        call(
            "round",
            vec![lit("Decimal", "2.345"), int("2"), lit("String", "HalfEven")],
        ),
    ] {
        unknown(expression, "INVALID_VALUE");
    }
}
