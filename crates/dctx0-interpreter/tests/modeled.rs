use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value, modeled_value};
use std::collections::BTreeMap;
fn reference() -> WorkspaceIr {
    let compiled = dctx0_language::compile(&[Source {
        file: "modeled.dctx0".into(),
        text: r#"workspace W {
value Score = Float
type Reading { score: Score
 samples: List<Option<Score>> }
type Choice = Empty | Reading(Reading)
}"#
        .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    compiled.ir.unwrap()
}
fn ty(id: &str) -> TypeRef {
    TypeRef {
        name: id.into(),
        symbol_id: Some(id.into()),
        arguments: vec![],
    }
}
#[test]
fn modeled_float_literals_convert_only_when_lossless_and_native_results_are_never_repaired() {
    let ir = reference();
    let budgets = VerificationBudgets::default();
    let float = TypeRef::builtin("Float");
    for (input, expected) in [
        (VerificationValue::Integer("5".into()), "5"),
        (VerificationValue::Decimal("4.5".into()), "4.5"),
    ] {
        assert!(matches!(
            check_value(&ir, &float, &input, &budgets),
            ValueContract::Rejected { .. }
        ));
        assert_eq!(
            modeled_value(&ir, &float, input, &budgets).unwrap(),
            VerificationValue::Float(expected.into())
        );
    }
    for input in ["0.1", "9007199254740993", &format!("1{}", "0".repeat(400))] {
        assert!(
            matches!(modeled_value(&ir,&float,VerificationValue::Decimal(input.into()),&budgets),Err(ValueContract::Unknown { issue }) if issue.code=="FLOAT_REPRESENTATION")
        );
    }
    assert!(matches!(
        modeled_value(
            &ir,
            &float,
            VerificationValue::Decimal("invalid".into()),
            &budgets
        ),
        Err(ValueContract::Rejected { .. })
    ));
    assert!(matches!(
        modeled_value(
            &ir,
            &TypeRef::builtin("Int"),
            VerificationValue::Float("5".into()),
            &budgets
        ),
        Err(ValueContract::Rejected { .. })
    ));
    let record = VerificationValue::Record(BTreeMap::from([
        ("score".into(), VerificationValue::Decimal("4.5".into())),
        (
            "samples".into(),
            VerificationValue::List(vec![
                VerificationValue::Option(None),
                VerificationValue::Option(Some(Box::new(VerificationValue::Integer("5".into())))),
            ]),
        ),
    ]));
    let value = modeled_value(&ir, &ty("W.Reading"), record.clone(), &budgets).unwrap();
    assert_eq!(
        check_value(&ir, &ty("W.Reading"), &value, &budgets),
        ValueContract::Accepted
    );
    let variant = VerificationValue::Variant {
        declaration: "W.Choice".into(),
        name: "Reading".into(),
        payload: Some(Box::new(record.clone())),
    };
    assert!(modeled_value(&ir, &ty("W.Choice"), variant, &budgets).is_ok());
    let invalid = VerificationValue::Variant {
        declaration: "W.Choice".into(),
        name: "Missing".into(),
        payload: Some(Box::new(record)),
    };
    assert!(matches!(
        modeled_value(&ir, &ty("W.Choice"), invalid, &budgets),
        Err(ValueContract::Rejected { .. })
    ));
    assert!(matches!(
        modeled_value(
            &ir,
            &ty("W.Reading"),
            VerificationValue::Record(BTreeMap::new()),
            &budgets
        ),
        Err(ValueContract::Rejected { .. })
    ));
    assert!(matches!(
        modeled_value(&ir, &ty("W.Reading"), VerificationValue::Unit, &budgets),
        Err(ValueContract::Rejected { .. })
    ));
    let shallow = VerificationBudgets {
        recursion_depth: 0,
        ..budgets
    };
    assert!(
        matches!(modeled_value(&ir,&ty("W.Score"),VerificationValue::Integer("5".into()),&shallow),Err(ValueContract::Unknown { issue }) if issue.code=="TYPE_DEPTH")
    );
}
