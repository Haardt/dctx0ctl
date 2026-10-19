use dctx0_core::{Source, TypeRef, VerificationBudgets, VerificationValue as V};
use dctx0_interpreter::{ValueContract, check_value};
use dctx0_verification::generate_values;

#[test]
fn productive_recursive_json_has_a_deterministic_finite_corpus_without_budget_exhaustion() {
    let result = dctx0_language::compile(&[Source {
        file: "recursive-json.dctx0".into(),
        text: r#"workspace W {
          type Json = Null | Bool(Boolean) | Number(Float) | Text(String) | Array(List<Json>) | Object(Object)
          message Member { name: String; value: Json }
          value Object = List<Member>
          type Impossible { next: Impossible }
        }"#.into(),
    }]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let ir = result.ir.unwrap();
    let budgets = VerificationBudgets {
        recursion_depth: 64,
        ..Default::default()
    };
    for name in ["W.Json", "W.Member", "W.Object"] {
        let ty = TypeRef {
            name: name.into(),
            symbol_id: Some(name.into()),
            arguments: vec![],
        };
        let corpus = generate_values(&ir, &ty, &budgets);
        assert!(corpus.issues.is_empty(), "{name}: {:?}", corpus.issues);
        assert!(!corpus.values.is_empty());
        assert!(!corpus.exhaustive);
        assert_eq!(corpus, generate_values(&ir, &ty, &budgets));
        for value in &corpus.values {
            assert_eq!(
                check_value(&ir, &ty, value, &budgets),
                ValueContract::Accepted
            );
        }
        if name == "W.Json" {
            for alternative in ["Null", "Bool", "Number", "Text", "Array", "Object"] {
                assert!(
                    corpus.values.iter().any(
                        |value| matches!(value, V::Variant { name, .. } if name == alternative)
                    ),
                    "missing {alternative}"
                );
            }
            assert!(corpus.values.iter().any(|value| matches!(value, V::Variant { name, payload: Some(payload), .. } if name == "Array" && matches!(payload.as_ref(), V::List(items) if !items.is_empty()))));
        }
    }
    let ty = TypeRef {
        name: "W.Impossible".into(),
        symbol_id: Some("W.Impossible".into()),
        arguments: vec![],
    };
    let corpus = generate_values(&ir, &ty, &budgets);
    assert!(corpus.values.is_empty());
    assert!(!corpus.exhaustive);
    assert!(
        corpus
            .issues
            .iter()
            .any(|issue| issue.code == "EMPTY_CORPUS")
    );
}
