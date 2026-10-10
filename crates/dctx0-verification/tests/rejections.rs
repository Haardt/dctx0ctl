use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value, numeric_offset};
use dctx0_verification::*;
fn ir() -> WorkspaceIr {
    let compiled = dctx0_language::compile(&[Source {
        file: "rejections.dctx0".into(),
        text: r#"workspace W {
value Small = Int { range 1..2 }
type Alias = Small
value Money = Decimal { range 0.1..0.5 }
value Id = UUID
entity Item {
 id: Id
 amount: Alias
}
type Choice = First | Second
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
fn rejected_corpora_include_exact_boundaries_uuid_and_record_failures_and_are_deterministic() {
    let ir = ir();
    let budgets = VerificationBudgets::default();
    for id in [
        "W.Small", "W.Alias", "W.Money", "W.Id", "W.Item", "W.Choice",
    ] {
        let corpus = generate_rejections(&ir, &ty(id), &budgets);
        assert!(!corpus.values.is_empty());
        assert!(!corpus.exhaustive);
        assert_eq!(corpus, generate_rejections(&ir, &ty(id), &budgets));
        assert!(corpus.values.iter().all(|v| matches!(
            check_value(&ir, &ty(id), v, &budgets),
            ValueContract::Rejected { .. }
        )));
        assert!(
            corpus
                .values
                .iter()
                .enumerate()
                .all(|(i, v)| !corpus.values[..i].contains(v))
        );
    }
    let small = generate_rejections(&ir, &ty("W.Alias"), &budgets);
    assert!(
        small
            .values
            .contains(&VerificationValue::Integer("0".into()))
    );
    assert!(
        small
            .values
            .contains(&VerificationValue::Integer("3".into()))
    );
    let money = generate_rejections(&ir, &ty("W.Money"), &budgets);
    assert!(
        money
            .values
            .contains(&VerificationValue::Decimal("-0.9".into()))
    );
    assert!(
        money
            .values
            .contains(&VerificationValue::Decimal("1.5".into()))
    );
    assert!(
        generate_rejections(&ir, &ty("W.Id"), &budgets)
            .values
            .contains(&VerificationValue::String("not-a-uuid".into()))
    );
    assert!(generate_rejections(&ir,&ty("W.Item"),&budgets).values.iter().any(|v|matches!(v,VerificationValue::Record(fields) if fields.contains_key("__unexpected_field__"))));
}
#[test]
fn exact_offsets_legacy_plans_and_unavailable_rejection_domains_do_not_invent_evidence() {
    assert_eq!(
        numeric_offset(&VerificationValue::Integer("9007199254740993".into()), 1),
        Some(VerificationValue::Integer("9007199254740994".into()))
    );
    assert_eq!(
        numeric_offset(
            &VerificationValue::Decimal("-0.000000000000000001".into()),
            -1
        ),
        Some(VerificationValue::Decimal("-1.000000000000000001".into()))
    );
    assert!(numeric_offset(&VerificationValue::Integer("not-a-number".into()), 1).is_none());
    assert!(numeric_offset(&VerificationValue::Float("1.5".into()), 1).is_none());
    let ir = ir();
    let budgets = VerificationBudgets {
        value_bytes: 0,
        ..Default::default()
    };
    assert_eq!(
        generate_rejections(&ir, &ty("W.Small"), &budgets).issues[0].code,
        "REJECTION_CORPUS_UNAVAILABLE"
    );
    let legacy:ContractPlan=serde_json::from_value(serde_json::json!({"subject":"W.Small","ty":ty("W.Small"),"corpus":generate_values(&ir,&ty("W.Small"),&VerificationBudgets::default())})).unwrap();
    assert_eq!(
        legacy.rejection_corpus.issues[0].code,
        "LEGACY_REJECTION_CORPUS"
    );
    assert!(legacy.rejection_generator_version.is_empty());
}
