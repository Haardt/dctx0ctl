use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value};
use dctx0_verification::{generate_rejections, generate_values};

#[test]
fn predicate_refinements_generate_positive_and_negative_corpora_for_nested_profiles() {
    let source = Source { file: "profiles.dctx0".into(), text: r#"workspace W {
      value Name = String { length 1..100
 where Api.name(value) }
      value Username = String { length 3..32
 where Api.username(value) }
      message Human { first: Name
        last: Name }
      surface Api {
        operation name(text: String) -> Boolean { pure ensures result == (text == trim(text) and length(text) >= 1 and length(text) <= 100) }
        operation username(text: String) -> Boolean { pure ensures result == matchesText(text, "^[a-z][a-z0-9_]{2,31}$") }
      }
    }"#.into() };
    let source = Source {
        text: source.text.replace(" ensures ", "\n ensures "),
        ..source
    };
    let compiled = dctx0_language::compile(&[source]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    for name in ["W.Name", "W.Username", "W.Human"] {
        let ty = TypeRef {
            name: name.into(),
            symbol_id: Some(name.into()),
            arguments: vec![],
        };
        let positive = generate_values(&ir, &ty, &Default::default());
        assert!(positive.issues.is_empty(), "{name}: {:?}", positive.issues);
        assert!(!positive.values.is_empty(), "{name}");
        for value in &positive.values {
            assert_eq!(
                check_value(&ir, &ty, value, &Default::default()),
                ValueContract::Accepted
            );
        }
        let negative = generate_rejections(&ir, &ty, &Default::default());
        assert!(negative.issues.is_empty(), "{name}: {:?}", negative.issues);
        assert!(!negative.values.is_empty());
        for value in &negative.values {
            assert!(matches!(
                check_value(&ir, &ty, value, &Default::default()),
                ValueContract::Rejected { .. }
            ));
        }
    }
}
