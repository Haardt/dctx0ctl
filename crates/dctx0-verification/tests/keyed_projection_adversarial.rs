use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
use dctx0_verification::*;
use std::collections::BTreeMap;

fn value(a: i32, b: i32, v: i32) -> VerificationValue {
    VerificationValue::Record(BTreeMap::from([
        ("a".into(), VerificationValue::Integer(a.to_string())),
        ("b".into(), VerificationValue::Integer(b.to_string())),
        ("value".into(), VerificationValue::Integer(v.to_string())),
    ]))
}
fn other(a: i32, v: i32) -> VerificationValue {
    VerificationValue::Record(BTreeMap::from([
        ("a".into(), VerificationValue::Integer(a.to_string())),
        ("value".into(), VerificationValue::Integer(v.to_string())),
    ]))
}
fn assessment(
    projection: &str,
    returned: VerificationValue,
    after: Vec<VerificationValue>,
) -> VerificationResult {
    assessment_budget(projection, returned, after, &Default::default())
}
fn assessment_budget(
    projection: &str,
    returned: VerificationValue,
    after: Vec<VerificationValue>,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    assessment_source(
        projection,
        returned,
        after,
        budgets,
        "Item { a = result.a b = result.b value = result.value }",
    )
}
fn assessment_source(
    projection: &str,
    returned: VerificationValue,
    after: Vec<VerificationValue>,
    budgets: &VerificationBudgets,
    source: &str,
) -> VerificationResult {
    let c = dctx0_language::compile(&[Source {
        file: "adversarial.dctx0".into(),
        text: format!(
            "workspace W {{ entity Item {{ a:Int\nb:Int\nvalue:Int\nkey a b }} entity Other {{ a:Int\nvalue:Int\nkey a }} message View {{ a:Int\nb:Int\nvalue:Int }} port Store {{ operation apply(item:Item) -> View {{ state upserts Item from {source}\nensures success implies result.value == item.value\nensures success implies result == {projection} }} }} }}"
        ),
    }]);
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    let ir = c.ir.unwrap();
    let p = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: ["W.Store.apply".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let obs = InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.apply".into(),
            arguments: vec![value(1, 2, 0)],
        },
        observation: Observation::Returned { value: returned },
    };
    let state = ObservationContext {
        before: Some(BTreeMap::from([
            ("W.Item".into(), vec![value(0, 0, 9)]),
            ("W.Other".into(), vec![other(1, 0)]),
        ])),
        after: Some(BTreeMap::from([
            ("W.Item".into(), after),
            ("W.Other".into(), vec![other(1, 0)]),
        ])),
        ..Default::default()
    };
    assess_operation(&ir, &p.operations[0], &obs, &state, budgets).result
}
#[test]
fn returned_key_must_match_independently_modeled_write_identity() {
    let p = "View { a = after.Item(result.a,result.b).a b = after.Item(result.a,result.b).b value = after.Item(result.a,result.b).value }";
    let rows = vec![value(0, 0, 9), value(1, 2, 0)];
    assert_eq!(
        assessment_source(p, value(1, 2, 0), rows.clone(), &Default::default(), "item"),
        VerificationResult::Pass
    );
    let verdict = assessment_source(p, value(0, 0, 0), rows, &Default::default(), "item");
    assert!(matches!(verdict, VerificationResult::Fail { .. }));
}

#[test]
fn ordinary_collection_and_unary_projection_still_read_actual_rows() {
    let p = "View { a = after.Item(result.a,result.b).a b = after.Item(result.a,result.b).b value = sum(-(-n) for n in [after.Item(result.a,result.b).value]) }";
    assert_eq!(
        assessment(p, value(1, 2, 0), vec![value(0, 0, 9), value(1, 2, 0)]),
        VerificationResult::Pass
    );
    assert!(matches!(
        assessment(p, value(1, 2, 0), vec![value(0, 0, 9), value(1, 2, 8)]),
        VerificationResult::Fail { .. }
    ));
}
#[test]
fn composite_keys_and_other_entity() {
    let p = "View { a = after.Item(result.a,result.b).a b = after.Item(result.a,result.b).b value = after.Item(result.a,result.b).value }";
    let rows = vec![value(0, 0, 9), value(1, 2, 0)];
    assert_eq!(
        assessment(p, value(1, 2, 0), rows.clone()),
        VerificationResult::Pass
    );
    assert!(matches!(
        assessment(p, value(1, 0, 0), rows.clone()),
        VerificationResult::Fail { .. }
    ));
    assert!(matches!(
        assessment(p, value(1, 2, 0), vec![value(1, 2, 0)]),
        VerificationResult::Fail { .. }
    ));
    let unrelated = "View { a = after.Item(result.a,result.b).a b = after.Item(result.a,result.b).b value = after.Other(result.a).value }";
    assert!(matches!(
        assessment(unrelated, value(1, 2, 0), rows),
        VerificationResult::Unknown { .. }
    ));
}
#[test]
fn shadowed_result_cannot_select_outer_key() {
    let p = "View { a = after.Item(result.a,result.b).a b = after.Item(result.a,result.b).b value = sum(after.Item(result.a,result.b).value for result in after.Item) }";
    let verdict = assessment(p, value(1, 2, 0), vec![value(0, 0, 9), value(1, 2, 0)]);
    assert!(
        matches!(verdict, VerificationResult::Fail { issue } if issue.code == "STATE_RESULT_VIOLATED")
    );
}

#[test]
fn oversized_projection_is_unknown_with_small_budget() {
    let expression = format!(
        "{}{}",
        "after.Item(result.a,result.b).value",
        " + 0".repeat(70)
    );
    let p = format!(
        "View {{ a = after.Item(result.a,result.b).a b = after.Item(result.a,result.b).b value = {expression} }}"
    );
    let budgets = VerificationBudgets {
        evaluation_steps: 32,
        ..Default::default()
    };
    let verdict = assessment_budget(
        &p,
        value(1, 2, 0),
        vec![value(0, 0, 9), value(1, 2, 0)],
        &budgets,
    );
    assert!(
        matches!(verdict, VerificationResult::Unknown { issue } if issue.code == "STATE_RESULT_PROJECTION_REQUIRED")
    );
}

#[test]
fn constrained_key_rewrite_budget_never_certifies_a_result() {
    let p = "View { a = after.Item(result.a,result.b).a b = after.Item(result.a,result.b).b value = after.Item(result.a,result.b).value }";
    let rows = vec![value(0, 0, 9), value(1, 2, 0)];
    let mut rewrite_exhausted = false;
    for steps in 1..100 {
        let budgets = VerificationBudgets {
            evaluation_steps: steps,
            ..Default::default()
        };
        if let VerificationResult::Unknown { issue } =
            assessment_budget(p, value(1, 2, 0), rows.clone(), &budgets)
        {
            if issue
                .message
                .contains("exact modeled changed identity within budgets")
            {
                rewrite_exhausted = true;
                break;
            }
        }
    }
    assert!(rewrite_exhausted, "a constrained rewrite must fail closed");
}
