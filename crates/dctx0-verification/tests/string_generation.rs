use dctx0_core::{Source, TypeRef, VerificationBudgets, VerificationValue as V};
use dctx0_interpreter::{ValueContract, check_value};
use dctx0_verification::generate_values;

#[test]
fn literal_prefixes_combine_inherited_length_bounds_without_waiving_other_clauses() {
    let compiled = dctx0_language::compile(&[Source {
        file: "prefix.dctx0".into(),
        text: r#"workspace W {
 value Digest = String {
  length 71..71
  where startsWith(value, "sha256:")
 }
 value Alias = Digest
 value Sized = String { length 4..6 }
 value Prefix = Sized { where startsWith(value, "id:") }
 value Unicode = String {
  length 4..4
  where startsWith(value, "😀:")
 }
 value Conflict = Digest { where startsWith(value, "other:") }
 value TooShort = Digest { length 1..2 }
 value Exact = Digest { where value == "wrong" }
 type Image { digest: Alias }
}"#
        .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let budgets = VerificationBudgets::default();
    for name in ["Digest", "Alias", "Prefix", "Unicode", "Image"] {
        let ty = TypeRef {
            name: name.into(),
            symbol_id: Some(format!("W.{name}")),
            arguments: vec![],
        };
        let corpus = generate_values(&ir, &ty, &budgets);
        assert!(!corpus.values.is_empty(), "{name}: {:?}", corpus.issues);
        assert!(corpus.issues.is_empty(), "{name}: {:?}", corpus.issues);
        assert!(!corpus.exhaustive);
        assert_eq!(corpus, generate_values(&ir, &ty, &budgets));
        for value in &corpus.values {
            assert_eq!(
                check_value(&ir, &ty, value, &budgets),
                ValueContract::Accepted
            );
        }
        if name == "Prefix" {
            for length in [4, 6] {
                assert!(corpus.values.iter().any(|value| matches!(value, V::String(text) if text.encode_utf16().count() == length)));
            }
        }
        let single = VerificationBudgets {
            generated_cases: 1,
            ..budgets.clone()
        };
        let limited = generate_values(&ir, &ty, &single);
        assert_eq!(limited.values.len(), 1, "{name}: {:?}", limited.issues);
        assert!(limited.issues.is_empty());
    }
    for name in ["Conflict", "TooShort", "Exact"] {
        let ty = TypeRef {
            name: name.into(),
            symbol_id: Some(format!("W.{name}")),
            arguments: vec![],
        };
        let corpus = generate_values(&ir, &ty, &budgets);
        assert!(corpus.values.is_empty(), "{name}: {:?}", corpus.values);
        assert!(
            corpus
                .issues
                .iter()
                .any(|issue| issue.code == "EMPTY_CORPUS")
        );
    }
}
