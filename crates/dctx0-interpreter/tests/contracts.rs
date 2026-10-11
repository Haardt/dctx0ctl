use dctx0_core::*;
use dctx0_interpreter::{Evaluation, ValueContract as C, check_value, evaluate};
use std::collections::BTreeMap;

#[test]
fn cinema_comment_text_uses_reference_unicode_trim_without_rewriting_the_spec() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema/dctx0");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|name| Source {
        file: name.into(),
        text: std::fs::read_to_string(root.join(name)).unwrap(),
    });
    let compiled = dctx0_language::compile(&sources);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    for (value, accepted) in [
        ("\u{feff}", false),
        ("\u{00a0}\u{feff}", false),
        ("\u{0085}", true),
        ("\u{feff}text\u{feff}", true),
    ] {
        let checked = check_value(
            &ir,
            &ty("HomeCinema.MovieNight.CommentText"),
            &VerificationValue::String(value.into()),
            &Default::default(),
        );
        assert_eq!(
            matches!(checked, C::Accepted),
            accepted,
            "{value:?}: {checked:?}"
        );
        if !accepted {
            assert!(matches!(checked, C::Rejected { .. }));
        }
    }
}
fn ir() -> WorkspaceIr {
    let input = Source {
        file: "contracts.dctx0".into(),
        text: r#"workspace W {
 value Year = Int { range 1888..2100 }
 value Flag = Boolean
 type Entry { year: Year
 flag: Flag
 where flag == true }
 type Choice = Empty | Item(Entry)
 value Money = Decimal { range 0.1..1.5 }
 value Positive = Int { where value > 0 }
 value Measured = String { length 1..4 }
 failure Broken { reason: String }
 surface Api { operation number() -> Int { pure } }
}"#
        .into(),
    };
    let c = dctx0_language::compile(&[input]);
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    c.ir.unwrap()
}
fn ty(id: &str) -> TypeRef {
    TypeRef {
        name: id.rsplit('.').next().unwrap().into(),
        symbol_id: Some(id.into()),
        arguments: vec![],
    }
}
fn int(value: &str) -> VerificationValue {
    VerificationValue::Integer(value.into())
}
fn string(value: &str) -> VerificationValue {
    VerificationValue::String(value.into())
}
fn check(ir: &WorkspaceIr, ty: &TypeRef, value: &VerificationValue) -> C {
    check_value(ir, ty, value, &Default::default())
}
fn rejected(c: C) {
    assert!(matches!(c, C::Rejected { .. }), "{c:?}");
}
fn unknown(c: C, code: &str) {
    assert!(matches!(c, C::Unknown { issue } if issue.code == code));
}
fn entry() -> VerificationValue {
    VerificationValue::Record(BTreeMap::from([
        ("year".into(), int("2000")),
        ("flag".into(), VerificationValue::Boolean(true)),
    ]))
}
#[test]
fn aliases_ranges_records_variants_and_where_conditions_come_from_actual_ir() {
    let ir = ir();
    for v in ["1888", "2100", "2000"] {
        assert_eq!(check(&ir, &ty("W.Year"), &int(v)), C::Accepted);
    }
    for v in ["1887", "2101", "0", "not-a-number"] {
        rejected(check(&ir, &ty("W.Year"), &int(v)));
    }
    rejected(check(&ir, &ty("W.Year"), &string("2000")));
    assert_eq!(check(&ir, &ty("W.Entry"), &entry()), C::Accepted);
    rejected(check(
        &ir,
        &ty("W.Entry"),
        &VerificationValue::Record(BTreeMap::new()),
    ));
    rejected(check(
        &ir,
        &ty("W.Entry"),
        &VerificationValue::Record(BTreeMap::from([
            ("other".into(), int("1")),
            ("year".into(), int("2000")),
        ])),
    ));
    rejected(check(&ir, &ty("W.Entry"), &int("1")));
    let variant = |declaration: &str, name: &str, payload| VerificationValue::Variant {
        declaration: declaration.into(),
        name: name.into(),
        payload,
    };
    assert_eq!(
        check(&ir, &ty("W.Choice"), &variant("W.Choice", "Empty", None)),
        C::Accepted
    );
    assert_eq!(
        check(
            &ir,
            &ty("W.Choice"),
            &variant("W.Choice", "Item", Some(Box::new(entry())))
        ),
        C::Accepted
    );
    for value in [
        variant("W.Other", "Empty", None),
        variant("W.Choice", "Missing", None),
        variant("W.Choice", "Item", None),
        variant("W.Choice", "Empty", Some(Box::new(entry()))),
        int("1"),
    ] {
        rejected(check(&ir, &ty("W.Choice"), &value));
    }
    assert_eq!(check(&ir, &ty("W.Positive"), &int("1")), C::Accepted);
    rejected(check(&ir, &ty("W.Positive"), &int("0")));
    assert_eq!(check(&ir, &ty("W.Measured"), &string("a")), C::Accepted);
    unknown(check(&ir, &ty("W.Missing"), &int("0")), "MISSING_TYPE");
}
#[test]
fn builtin_contracts_reject_invalid_representations_without_rounding() {
    let ir = ir();
    for (name, v) in [
        ("Unit", VerificationValue::Unit),
        ("Boolean", VerificationValue::Boolean(true)),
        ("String", string("text")),
        ("Int", int("9007199254740993")),
        ("Decimal", VerificationValue::Decimal("0.1".into())),
        ("Float", VerificationValue::Float("4.5".into())),
        ("UUID", string("00000000-0000-4000-8000-000000000001")),
        ("Timestamp", string("2026-10-10T00:00:00Z")),
        ("Failure", VerificationValue::Unit),
    ] {
        assert_eq!(check(&ir, &TypeRef::builtin(name), &v), C::Accepted);
    }
    for (name, v) in [
        ("Boolean", int("1")),
        ("Int", int("1.5")),
        ("Decimal", VerificationValue::Decimal("1e2".into())),
        ("Float", VerificationValue::Float("NaN".into())),
        ("UUID", string("not-a-uuid")),
        ("UUID", string("00000000_0000-4000-8000-000000000001")),
        ("Timestamp", string("2026-02-30T00:00:00Z")),
    ] {
        rejected(check(&ir, &TypeRef::builtin(name), &v));
    }
    rejected(check(&ir, &TypeRef::builtin("Duration"), &string("1s")));
    unknown(
        check(&ir, &TypeRef::builtin("Other"), &int("0")),
        "UNSUPPORTED_TYPE",
    );
    let generic = |name: &str, args| TypeRef {
        name: name.into(),
        symbol_id: None,
        arguments: args,
    };
    let list = generic("List", vec![ty("W.Year")]);
    assert_eq!(
        check(
            &ir,
            &list,
            &VerificationValue::List(vec![int("1888"), int("2100")])
        ),
        C::Accepted
    );
    rejected(check(
        &ir,
        &list,
        &VerificationValue::List(vec![int("2101")]),
    ));
    unknown(
        check(
            &ir,
            &TypeRef::builtin("List"),
            &VerificationValue::List(vec![]),
        ),
        "TYPE_ARGUMENTS",
    );
    let option = generic("Option", vec![ty("W.Year")]);
    for value in [
        VerificationValue::Option(None),
        VerificationValue::Option(Some(Box::new(int("2000")))),
    ] {
        assert_eq!(check(&ir, &option, &value), C::Accepted);
    }
    unknown(
        check(
            &ir,
            &TypeRef::builtin("Option"),
            &VerificationValue::Option(None),
        ),
        "TYPE_ARGUMENTS",
    );
    let failure = generic("Failure", vec![ty("W.Broken")]);
    let value = VerificationValue::Failure {
        declaration: "W.Broken".into(),
        payload: Some(Box::new(VerificationValue::Record(BTreeMap::from([(
            "reason".into(),
            string("broken"),
        )])))),
    };
    assert_eq!(check(&ir, &failure, &value), C::Accepted);
    rejected(check(
        &ir,
        &failure,
        &VerificationValue::Failure {
            declaration: "W.Other".into(),
            payload: None,
        },
    ));
    rejected(check(
        &ir,
        &failure,
        &VerificationValue::Failure {
            declaration: "W.Broken".into(),
            payload: None,
        },
    ));
}
#[test]
fn resource_caps_and_malformed_refinements_remain_unknown() {
    let mut workspace = ir();
    for budgets in [
        VerificationBudgets {
            value_bytes: 1,
            ..Default::default()
        },
        VerificationBudgets {
            evaluation_steps: 0,
            ..Default::default()
        },
        VerificationBudgets {
            recursion_depth: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            check_value(&workspace, &ty("W.Entry"), &entry(), &budgets),
            C::Unknown { .. }
        ));
    }
    let range_index = workspace
        .nodes
        .iter()
        .position(|n| n.owner == "W.Year" && n.kind == StatementKind::Range)
        .unwrap();
    let original = workspace.nodes[range_index].terms.clone();
    for text in ["unknown", "bad..2100", "2100..1888"] {
        let mut tokens = original.clone();
        tokens.truncate(1);
        tokens[0].text = text.into();
        workspace.nodes[range_index].terms = tokens;
        unknown(
            check(&workspace, &ty("W.Year"), &int("2000")),
            "INVALID_REFINEMENT",
        );
    }
    workspace.nodes[range_index].terms = original;
    workspace.data.get_mut("W.Year").unwrap().alias = Some(ty("W.Year"));
    unknown(check(&workspace, &ty("W.Year"), &int("2000")), "TYPE_DEPTH");
    let mut broken = ir();
    let node = broken
        .nodes
        .iter_mut()
        .find(|n| n.owner == "W.Positive" && n.kind == StatementKind::Where)
        .unwrap();
    let expression = &mut node.expressions[0];
    expression.value = ExpressionKind::Variable {
        name: "missing".into(),
    };
    assert!(
        matches!(check(&broken, &ty("W.Positive"), &int("1")), C::Unknown { issue } if issue.span.is_some())
    );
    expression_no_boolean(&mut broken);
    unknown(
        check(&broken, &ty("W.Positive"), &int("1")),
        "UNSUPPORTED_REFINEMENT",
    );
}
fn expression_no_boolean(ir: &mut WorkspaceIr) {
    let node = ir
        .nodes
        .iter_mut()
        .find(|n| n.owner == "W.Positive" && n.kind == StatementKind::Where)
        .unwrap();
    node.expressions[0].value = ExpressionKind::Literal {
        text: "1".into(),
        literal_kind: "Int".into(),
    };
}
#[test]
fn operation_success_flags_cannot_replace_typed_return_values() {
    let ir = ir();
    let position = Position {
        offset: 0,
        line: 1,
        column: 1,
    };
    let span = Span {
        file: "contracts.dctx0".into(),
        start: position,
        end: position,
    };
    let expression = Expression {
        ty: TypeRef::builtin("Int"),
        span,
        value: ExpressionKind::Call {
            name: "number".into(),
            target: Some("W.Api.number".into()),
            arguments: vec![],
        },
    };
    let observations = [InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.number".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Boolean(true),
        },
    }];
    assert!(
        matches!(evaluate(&ir, &expression, &BTreeMap::new(), &observations, &Default::default()), Evaluation::Unknown(issue) if issue.code == "INVALID_RETURN_TYPE")
    );
}

#[test]
fn unavailable_return_contracts_and_nonfinite_inputs_keep_their_source_reasons() {
    let mut workspace = ir();
    let position = Position {
        offset: 0,
        line: 1,
        column: 1,
    };
    let span = Span {
        file: "contracts.dctx0".into(),
        start: position,
        end: position,
    };
    let expression = Expression {
        ty: TypeRef::builtin("Int"),
        span,
        value: ExpressionKind::Call {
            name: "number".into(),
            target: Some("W.Api.number".into()),
            arguments: vec![],
        },
    };
    let observation = |value| InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.number".into(),
            arguments: vec![],
        },
        observation: Observation::Returned { value },
    };
    workspace.operations.get_mut("W.Api.number").unwrap().result =
        TypeRef::builtin("FutureBuiltin");
    assert!(
        matches!(evaluate(&workspace, &expression, &BTreeMap::new(), &[observation(string("1s"))], &Default::default()), Evaluation::Unknown(issue) if issue.code == "UNSUPPORTED_TYPE" && issue.span.is_some())
    );
    workspace.operations.get_mut("W.Api.number").unwrap().result = ty("W.Positive");
    let condition = workspace
        .nodes
        .iter_mut()
        .find(|n| n.owner == "W.Positive" && n.kind == StatementKind::Where)
        .unwrap();
    condition.expressions[0].value = ExpressionKind::Variable {
        name: "missing".into(),
    };
    assert!(
        matches!(evaluate(&workspace, &expression, &BTreeMap::new(), &[observation(int("1"))], &Default::default()), Evaluation::Unknown(issue) if issue.code == "UNBOUND_VALUE" && issue.span.is_some())
    );
    let variables = BTreeMap::from([("bad".into(), VerificationValue::Float("Infinity".into()))]);
    assert!(
        matches!(evaluate(&workspace, &expression, &variables, &[], &Default::default()), Evaluation::Unknown(issue) if issue.code == "INVALID_VALUE")
    );
    let value = VerificationValue::Float("4.5".into());
    assert_eq!(
        serde_json::from_str::<VerificationValue>(&serde_json::to_string(&value).unwrap()).unwrap(),
        value
    );
}

#[test]
fn combined_uncertainty_and_record_refinements_are_not_overwritten_by_later_fields() {
    let mut workspace = ir();
    let list = |element| TypeRef {
        name: "List".into(),
        symbol_id: None,
        arguments: vec![element],
    };
    rejected(check(
        &workspace,
        &list(ty("W.Year")),
        &VerificationValue::List(vec![int("1887"), int("1888")]),
    ));
    unknown(
        check(
            &workspace,
            &list(TypeRef::builtin("FutureBuiltin")),
            &VerificationValue::List(vec![string("1s"), string("2s")]),
        ),
        "UNSUPPORTED_TYPE",
    );
    let mut false_entry = match entry() {
        VerificationValue::Record(fields) => fields,
        _ => unreachable!(),
    };
    false_entry.insert("flag".into(), VerificationValue::Boolean(false));
    rejected(check(
        &workspace,
        &ty("W.Entry"),
        &VerificationValue::Record(false_entry),
    ));
    assert_eq!(
        check(
            &workspace,
            &ty("W.Money"),
            &VerificationValue::Decimal("0.1".into())
        ),
        C::Accepted
    );
    rejected(check(
        &workspace,
        &ty("W.Money"),
        &VerificationValue::Decimal("1.6".into()),
    ));
    workspace.data.get_mut("W.Year").unwrap().alias = Some(TypeRef::builtin("String"));
    unknown(
        check(&workspace, &ty("W.Year"), &string("2000")),
        "UNSUPPORTED_REFINEMENT",
    );
    let position = Position {
        offset: 0,
        line: 1,
        column: 1,
    };
    let expression = Expression {
        ty: TypeRef::builtin("String"),
        span: Span {
            file: "size.dctx0".into(),
            start: position,
            end: position,
        },
        value: ExpressionKind::Literal {
            text: "long output".into(),
            literal_kind: "String".into(),
        },
    };
    assert!(
        matches!(evaluate(&workspace, &expression, &BTreeMap::new(), &[], &VerificationBudgets { value_bytes: 1, ..Default::default() }), Evaluation::Unknown(issue) if issue.code == "VALUE_SIZE")
    );
}

#[test]
fn text_refinements_use_utf16_lengths_and_all_reviewed_formats() {
    let result = dctx0_language::compile(&[Source {
        file: "formats.dctx0".into(),
        text: r#"workspace W {
 value Text = String { length 2..4 }
 value Entries = List<Int> { length 1..2 }
 value Email = String { format Email }
 value Identifier = String { format UUID }
 value Link = String { format Uri }
 value Day = String { format Date }
 value Instant = String { format DateTime }
}"#
        .into(),
    }]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let ir = result.ir.unwrap();
    for (name, value, accepted) in [
        ("Text", "", false),
        ("Text", "a", false),
        ("Text", "ab", true),
        ("Text", "😀", true),
        ("Text", "😀😀", true),
        ("Text", "abcde", false),
        ("Email", "a@b.co", true),
        ("Email", "a@@b.co", false),
        ("Email", "a@b", false),
        ("Email", "a@b.co\u{feff}", false),
        ("Email", "a@b.co\u{0085}", true),
        ("Identifier", "00000000-0000-4000-8000-000000000001", true),
        ("Identifier", "00000000-0000-0000-0000-000000000001", false),
        ("Link", "https://example.test/tasks", true),
        ("Link", "mailto:a@b.co", true),
        ("Link", "/relative", false),
        ("Link", "https://bad host.test", false),
        ("Day", "2000-02-29", true),
        ("Day", "1900-02-29", false),
        ("Day", "2026-02-30", false),
        ("Day", "2026-1-01", false),
        ("Instant", "2026-10-11T12:00:00+02:00", true),
        ("Instant", "2026-02-30T00:00:00Z", false),
        ("Instant", "October 11, 2026", false),
    ] {
        let actual = check(&ir, &ty(&format!("W.{name}")), &string(value));
        assert_eq!(
            actual == C::Accepted,
            accepted,
            "{name}: {value:?}: {actual:?}"
        );
    }
    assert_eq!(
        check(
            &ir,
            &ty("W.Entries"),
            &VerificationValue::List(vec![int("1")])
        ),
        C::Accepted
    );
    rejected(check(
        &ir,
        &ty("W.Entries"),
        &VerificationValue::List(vec![]),
    ));
}

#[test]
fn duration_bytes_option_aliases_and_result_have_central_lossless_contracts() {
    let variant = |declaration: &str, name: &str, payload| VerificationValue::Variant {
        declaration: declaration.into(),
        name: name.into(),
        payload,
    };
    let ir = ir();
    assert_eq!(
        check(
            &ir,
            &TypeRef::builtin("Duration"),
            &VerificationValue::Float("-1.5".into())
        ),
        C::Accepted
    );
    rejected(check(
        &ir,
        &TypeRef::builtin("Duration"),
        &VerificationValue::Float("NaN".into()),
    ));
    for value in [
        VerificationValue::List(vec![]),
        VerificationValue::List(vec![int("0"), int("255")]),
    ] {
        assert_eq!(check(&ir, &TypeRef::builtin("Bytes"), &value), C::Accepted);
    }
    for value in [
        VerificationValue::List(vec![int("256")]),
        VerificationValue::List(vec![int("-1")]),
        VerificationValue::List(vec![int("1.5")]),
        VerificationValue::List(vec![string("0")]),
    ] {
        rejected(check(&ir, &TypeRef::builtin("Bytes"), &value));
    }
    let generic = |name: &str, arguments| TypeRef {
        name: name.into(),
        symbol_id: None,
        arguments,
    };
    for name in ["Some", "Dctx0Option"] {
        let ty = generic(name, vec![TypeRef::builtin("Int")]);
        assert_eq!(
            check(
                &ir,
                &ty,
                &VerificationValue::Option(Some(Box::new(int("1"))))
            ),
            C::Accepted
        );
        rejected(check(
            &ir,
            &ty,
            &VerificationValue::Option(Some(Box::new(string("1")))),
        ));
        let none = check(&ir, &ty, &VerificationValue::Option(None));
        assert_eq!(none == C::Accepted, name != "Some");
    }
    assert_eq!(
        check(
            &ir,
            &TypeRef::builtin("None"),
            &VerificationValue::Option(None)
        ),
        C::Accepted
    );
    let result = generic(
        "Dctx0Result",
        vec![TypeRef::builtin("Int"), TypeRef::builtin("String")],
    );
    for (name, payload, accepted) in [
        ("Success", int("1"), true),
        ("Failure", string("reason"), true),
        ("Other", int("1"), false),
        ("Success", string("1"), false),
    ] {
        let value = variant("builtin:Dctx0Result", name, Some(Box::new(payload)));
        assert_eq!(check(&ir, &result, &value) == C::Accepted, accepted);
    }
    rejected(check(
        &ir,
        &result,
        &variant("other", "Success", Some(Box::new(int("1")))),
    ));
}
