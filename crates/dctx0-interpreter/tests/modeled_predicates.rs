use dctx0_core::*;
use dctx0_interpreter::{Evaluation, ValueContract, check_value, evaluate};
use std::collections::BTreeMap;

fn compile(operation: &str) -> WorkspaceIr {
    let source = Source {
        file: "predicates.dctx0".into(),
        text: format!(
            "workspace W {{ value Name = String {{ where Api.valid(value) }} surface Api {{ {operation} }} }}"
        ).replace(" ensures ", "\n ensures ").replace(" requires ", "\n requires "),
    };
    let result = dctx0_language::compile(&[source]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.ir.unwrap()
}
fn check(ir: &WorkspaceIr, text: &str) -> ValueContract {
    check_value(
        ir,
        &TypeRef {
            name: "Name".into(),
            symbol_id: Some("W.Name".into()),
            arguments: vec![],
        },
        &VerificationValue::String(text.into()),
        &Default::default(),
    )
}

#[test]
fn nested_predicate_models_check_refinements_without_fabricating_native_observations() {
    let ir = compile(
        r#"
      operation valid(name: String) -> Boolean { pure
        ensures result == Api.canonical(name) }
      operation canonical(name: String) -> Boolean { pure
        ensures (name == trim(name) and length(name) >= 1 and length(name) <= 100) == result }
    "#,
    );
    for text in ["Anna", "Änne", "\u{0085}"] {
        assert_eq!(check(&ir, text), ValueContract::Accepted);
    }
    for text in ["", " Anna", "Anna ", "\u{feff}", &"a".repeat(101)] {
        assert!(matches!(check(&ir, text), ValueContract::Rejected { .. }));
    }
    let expression = ir
        .nodes
        .iter()
        .find(|node| node.owner == "W.Name" && node.kind == StatementKind::Where)
        .unwrap()
        .expressions[0]
        .clone();
    assert!(matches!(
        evaluate(
            &ir,
            &expression,
            &BTreeMap::from([("value".into(), VerificationValue::String("Anna".into()))]),
            &[],
            &Default::default()
        ),
        Evaluation::Invoke(_)
    ));
}

#[test]
fn incomplete_impure_and_inconsistent_models_remain_unknown() {
    for (operation, code) in [
        (
            "operation valid(name: String) -> Boolean { pure ensures not result or length(name) > 0 }",
            "UNSUPPORTED_REFINEMENT",
        ),
        (
            "operation valid(name: String) -> Boolean { ensures result == true }",
            "IMPURE_OPERATION",
        ),
        (
            "operation valid(name: String) -> Boolean { pure ensures result == true ensures result == false }",
            "INCONSISTENT_PREDICATE_MODEL",
        ),
        (
            "operation valid(name: String) -> Boolean { pure requires length(name) > 0 ensures result == true }",
            "PREDICATE_PRECONDITION",
        ),
    ] {
        let ir = compile(operation);
        assert!(
            matches!(check(&ir, ""), ValueContract::Unknown { issue } if issue.code == code),
            "{operation}"
        );
    }
    let ir = compile(
        "operation valid(name: String) -> Boolean { pure requires length(name) > 0 ensures result == true }",
    );
    assert_eq!(check(&ir, "Anna"), ValueContract::Accepted);
    let ir = compile(
        "operation valid(name: String) -> Boolean { pure ensures result == Api.valid(name) }",
    );
    assert!(
        matches!(check(&ir, "Anna"), ValueContract::Unknown { issue } if issue.code == "EVALUATION_BUDGET")
    );
}

#[test]
fn regex_models_enforce_full_username_alphabet_and_keep_bad_patterns_unknown() {
    let ir = compile(
        r#"operation valid(name: String) -> Boolean { pure ensures result == matchesText(name, "^[a-z][a-z0-9_]{2,31}$") }"#,
    );
    for text in ["anna", "a_1", &"a".repeat(32)] {
        assert_eq!(check(&ir, text), ValueContract::Accepted);
    }
    for text in [
        "aa",
        "@anna",
        "1anna",
        "Änne",
        "abc\n",
        "a-b",
        &"a".repeat(33),
    ] {
        assert!(
            matches!(check(&ir, text), ValueContract::Rejected { .. }),
            "{text:?}"
        );
    }
    let ir = compile(
        r#"operation valid(name: String) -> Boolean { pure ensures result == matchesText(name, "[") }"#,
    );
    assert!(
        matches!(check(&ir, "Anna"), ValueContract::Unknown { issue } if issue.code == "REGEX_MODEL")
    );
    let source = Source { file: "bad.dctx0".into(), text: "workspace W { surface Api { operation valid() -> Boolean { pure\n ensures result == matchesText(1, 2) } } }".into() };
    assert!(
        dctx0_language::compile(&[source])
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DCTX0_EXPRESSION_TYPE")
    );
}

#[test]
fn predicate_models_refuse_failures_bad_signatures_and_non_boolean_definitions() {
    let original =
        compile("operation valid(name: String) -> Boolean { pure ensures result == true }");
    let mut ir = original.clone();
    ir.operations
        .get_mut("W.Api.valid")
        .unwrap()
        .failures
        .push(TypeRef::builtin("Unit"));
    assert!(matches!(check(&ir, "Anna"), ValueContract::Unknown { .. }));
    let mut ir = original.clone();
    ir.operations.get_mut("W.Api.valid").unwrap().result = TypeRef::builtin("Int");
    assert!(matches!(check(&ir, "Anna"), ValueContract::Unknown { .. }));
    let mut ir = original.clone();
    ir.operations
        .get_mut("W.Api.valid")
        .unwrap()
        .parameters
        .clear();
    assert!(matches!(check(&ir, "Anna"), ValueContract::Unknown { .. }));
    let mut ir = original.clone();
    ir.operations.get_mut("W.Api.valid").unwrap().parameters[0].ty = TypeRef::builtin("Int");
    assert!(
        matches!(check(&ir, "Anna"), ValueContract::Unknown { issue } if issue.code == "MODELED_ARGUMENT_TYPE")
    );
    let mut ir = original.clone();
    ir.operations.get_mut("W.Api.valid").unwrap().parameters[0].ty = TypeRef {
        name: "Name".into(),
        symbol_id: Some("W.Name".into()),
        arguments: vec![],
    };
    assert!(matches!(check(&ir, "Anna"), ValueContract::Unknown { .. }));
    let mut ir = original.clone();
    ir.operations.get_mut("W.Api.valid").unwrap().parameters[0].ty = TypeRef {
        name: "List".into(),
        symbol_id: None,
        arguments: vec![TypeRef::builtin("String")],
    };
    assert!(matches!(check(&ir, "Anna"), ValueContract::Unknown { .. }));
    let mut ir = original;
    let clause = ir
        .nodes
        .iter_mut()
        .find(|node| node.owner == "W.Api.valid" && node.kind == StatementKind::Ensures)
        .unwrap();
    if let ExpressionKind::Binary { right, .. } = &mut clause.expressions[0].value {
        right.value = ExpressionKind::Literal {
            text: "wrong type".into(),
            literal_kind: "String".into(),
        };
    }
    assert!(matches!(check(&ir, "Anna"), ValueContract::Unknown { .. }));
}
