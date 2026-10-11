use dctx0_core::*;
use dctx0_verification::*;
use std::collections::BTreeSet;

#[test]
fn shrinking_keeps_unevaluable_candidate_preconditions_open() {
    let compiled = dctx0_language::compile(&[Source {
        file: "shrink-unknown-guard.dctx0".into(),
        text: "workspace W { surface Api { operation calculate(input:Int) -> Int {\npure\nrequires input > 5 or single(filter(value in [input] where value > 5)) == input\n} } }".into(),
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
            subjects: ["W.Api.calculate".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let original = Invocation {
        subject: plan.operation.id.clone(),
        arguments: vec![VerificationValue::Integer("10".into())],
    };
    assert_eq!(
        operation_ready(
            &ir,
            &plan,
            &original,
            &Default::default(),
            &Default::default()
        ),
        VerificationResult::Pass
    );
    let result = shrink_invocations(
        &ir,
        &plan,
        &original,
        &Default::default(),
        &Default::default(),
    );
    assert!(
        result.0.is_empty(),
        "An unknown guard cannot authorize a smaller input"
    );
    assert!(
        !result.1.is_empty(),
        "The unresolved guard must remain visible"
    );
    assert_eq!(
        result,
        shrink_invocations(
            &ir,
            &plan,
            &original,
            &Default::default(),
            &Default::default()
        )
    );
}

#[test]
fn large_explicit_lists_keep_a_deterministic_bounded_shrink_prefix() {
    let compiled = dctx0_language::compile(&[Source {
        file: "large-shrink.dctx0".into(),
        text:
            "workspace W { surface Api { operation calculate(input:List<Int>) -> Unit { pure } } }"
                .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let budgets = VerificationBudgets {
        shrink_steps: 2,
        ..Default::default()
    };
    let plan = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: ["W.Api.calculate".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        budgets.clone(),
    )
    .operations
    .remove(0);
    let items = (0..1000)
        .map(|number| VerificationValue::Integer(number.to_string()))
        .collect::<Vec<_>>();
    let original = Invocation {
        subject: plan.operation.id.clone(),
        arguments: vec![VerificationValue::List(items.clone())],
    };
    let (candidates, issues) =
        shrink_invocations(&ir, &plan, &original, &Default::default(), &budgets);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.arguments.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![VerificationValue::List(vec![])],
            vec![VerificationValue::List(items[1..].to_vec())]
        ]
    );
    let disabled = VerificationBudgets {
        shrink_steps: 0,
        ..budgets
    };
    assert!(
        shrink_invocations(&ir, &plan, &original, &Default::default(), &disabled)
            .0
            .is_empty()
    );
}

#[test]
fn shrinking_is_deterministic_and_preserves_type_and_precondition_boundaries() {
    let compiled = dctx0_language::compile(&[Source {
        file: "shrink.dctx0".into(),
        text: "workspace W {\nsurface Api {\noperation calculate(input: Int) -> Int {\npure\nrequires input > 2\nensures result == input\n}\n}\n}".into(),
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
            subjects: BTreeSet::from(["W.Api.calculate".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let original = Invocation {
        subject: plan.operation.id.clone(),
        arguments: vec![VerificationValue::Integer("10".into())],
    };
    let first = shrink_invocations(
        &ir,
        &plan,
        &original,
        &Default::default(),
        &Default::default(),
    );
    assert_eq!(
        first,
        shrink_invocations(
            &ir,
            &plan,
            &original,
            &Default::default(),
            &Default::default()
        )
    );
    assert!(first.1.is_empty());
    assert_eq!(
        first.0,
        vec![Invocation {
            subject: original.subject.clone(),
            arguments: vec![VerificationValue::Integer("5".into())]
        }]
    );
    let wrong = Invocation {
        subject: "other".into(),
        arguments: original.arguments.clone(),
    };
    let rejected = shrink_invocations(&ir, &plan, &wrong, &Default::default(), &Default::default());
    assert!(rejected.0.is_empty());
    assert_eq!(rejected.1[0].code, "SHRINK_SIGNATURE");
    let outside = Invocation {
        subject: original.subject.clone(),
        arguments: vec![VerificationValue::Integer("1".into())],
    };
    let rejected = shrink_invocations(
        &ir,
        &plan,
        &outside,
        &Default::default(),
        &Default::default(),
    );
    assert!(rejected.0.is_empty());
    assert_eq!(rejected.1[0].code, "OPERATION_PRECONDITION_UNMET");
    let malformed = Invocation {
        subject: original.subject.clone(),
        arguments: vec![VerificationValue::String("invalid integer".into())],
    };
    let rejected = shrink_invocations(
        &ir,
        &plan,
        &malformed,
        &Default::default(),
        &Default::default(),
    );
    assert!(rejected.0.is_empty());
    assert_eq!(rejected.1[0].code, "INVALID_OPERATION_INPUT");
    let budget = VerificationBudgets {
        shrink_steps: 0,
        ..Default::default()
    };
    assert!(
        shrink_invocations(&ir, &plan, &original, &Default::default(), &budget)
            .0
            .is_empty()
    );
}

#[test]
fn structured_shrinking_preserves_unmodified_fields_and_unicode_scalar_boundaries() {
    use VerificationValue as V;
    use std::collections::BTreeMap;
    let compiled = dctx0_language::compile(&[Source {
        file: "structured-shrink.dctx0".into(),
        text: "workspace W {\n type Input {\n integer:Int\n decimal:Decimal\n floating:Float\n text:String\n items:List<Int>\n optional:Option<Int>\n }\n surface Api {\n operation calculate(input:Input) -> Int {\n pure\n }\n }\n}".into(),
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
            subjects: ["W.Api.calculate".into()].into(),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let fields = BTreeMap::from([
        ("integer".into(), V::Integer("-16".into())),
        ("decimal".into(), V::Decimal("12.75".into())),
        ("floating".into(), V::Float("8".into())),
        ("text".into(), V::String("ä🍿abcd".into())),
        (
            "items".into(),
            V::List(vec![V::Integer("-8".into()), V::Integer("4".into())]),
        ),
        (
            "optional".into(),
            V::Option(Some(Box::new(V::Integer("10".into())))),
        ),
    ]);
    let original = Invocation {
        subject: plan.operation.id.clone(),
        arguments: vec![V::Record(fields.clone())],
    };
    let (candidates, issues) = shrink_invocations(
        &ir,
        &plan,
        &original,
        &Default::default(),
        &Default::default(),
    );
    assert!(issues.is_empty(), "{issues:?}");
    for (field, replacement) in [
        ("integer", V::Integer("-8".into())),
        ("decimal", V::Decimal("12".into())),
        ("floating", V::Float("4".into())),
        ("text", V::String("ä🍿a".into())),
        ("items", V::List(vec![V::Integer("4".into())])),
        (
            "items",
            V::List(vec![V::Integer("-4".into()), V::Integer("4".into())]),
        ),
        ("optional", V::Option(None)),
        (
            "optional",
            V::Option(Some(Box::new(V::Integer("5".into())))),
        ),
    ] {
        let mut expected = fields.clone();
        expected.insert(field.into(), replacement);
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.arguments == [V::Record(expected.clone())]),
            "Missing shrink for {field}: {candidates:?}"
        );
    }
    for candidate in &candidates {
        let V::Record(changed) = &candidate.arguments[0] else {
            panic!("Record shape must remain intact");
        };
        assert_eq!(changed.len(), fields.len());
        assert_eq!(
            changed
                .iter()
                .filter(|(name, value)| fields.get(*name) != Some(*value))
                .count(),
            1
        );
    }
    let shallow = VerificationBudgets {
        recursion_depth: 1,
        ..Default::default()
    };
    assert!(
        shrink_invocations(&ir, &plan, &original, &Default::default(), &shallow)
            .0
            .is_empty()
    );
    let tiny = VerificationBudgets {
        value_bytes: 1,
        ..Default::default()
    };
    assert!(
        shrink_invocations(&ir, &plan, &original, &Default::default(), &tiny)
            .0
            .is_empty()
    );
    let limited = VerificationBudgets {
        shrink_steps: 1,
        ..Default::default()
    };
    assert_eq!(
        shrink_invocations(&ir, &plan, &original, &Default::default(), &limited)
            .0
            .len(),
        1
    );
}

#[test]
fn variant_payload_shrinking_keeps_the_declared_alternative() {
    use VerificationValue as V;
    let compiled = dctx0_language::compile(&[Source {
        file: "variant-shrink.dctx0".into(),
        text: "workspace W {\n type Choice = Empty | Full(Int)\n surface Api {\n operation calculate(input:Choice) -> Int {\n pure\n }\n }\n}".into(),
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
            subjects: ["W.Api.calculate".into()].into(),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let original = Invocation {
        subject: plan.operation.id.clone(),
        arguments: vec![V::Variant {
            declaration: "W.Choice".into(),
            name: "Full".into(),
            payload: Some(Box::new(V::Integer("16".into()))),
        }],
    };
    let (candidates, issues) = shrink_invocations(
        &ir,
        &plan,
        &original,
        &Default::default(),
        &Default::default(),
    );
    assert!(issues.is_empty());
    assert_eq!(candidates.len(), 3);
    assert!(candidates.iter().any(|candidate| candidate.arguments
        == [V::Variant {
            declaration: "W.Choice".into(),
            name: "Full".into(),
            payload: Some(Box::new(V::Integer("8".into()))),
        }]));
    let empty = Invocation {
        subject: original.subject.clone(),
        arguments: vec![V::Variant {
            declaration: "W.Choice".into(),
            name: "Empty".into(),
            payload: None,
        }],
    };
    assert!(
        shrink_invocations(&ir, &plan, &empty, &Default::default(), &Default::default())
            .0
            .is_empty()
    );
}
