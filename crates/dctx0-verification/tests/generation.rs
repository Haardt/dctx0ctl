use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value};
use dctx0_verification::{ValueCorpus, generate_values};
use std::collections::BTreeSet;
fn ir() -> WorkspaceIr {
    let result = dctx0_language::compile(&[Source {
        file: "generation.dctx0".into(),
        text: r#"workspace W {
 value Year = Int { range 1888..2100 }
 value Small = Int { range -1..1 }
 value Money = Decimal { range 2.1..2.2 }
 value Nested = Small { range 0..1 }
 value Sparse = Int { where value == 123456789 }
 value BoundedText = String { length 1..4 }
 type Pair { first: Boolean
 second: Boolean }
 type Empty {}
 type Choice = None | Pair(Pair)
 type Cycle { next: Cycle }
 type Wide { a: Int
 b: Int
 c: Int
 d: Int
 e: Int
 f: Int
 g: Int
 h: Int
 i: Int
 j: Int }
}"#
        .into(),
    }]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.ir.unwrap()
}
fn ty(id: &str) -> TypeRef {
    TypeRef {
        name: id.into(),
        symbol_id: Some(id.into()),
        arguments: vec![],
    }
}
fn generic(name: &str, element: TypeRef) -> TypeRef {
    TypeRef {
        name: name.into(),
        symbol_id: None,
        arguments: vec![element],
    }
}

#[test]
fn sampled_products_fill_the_requested_distinct_case_budget_despite_collisions() {
    let ir = ir();
    let parameters = ["first", "second", "third"].map(|name| FieldIr {
        name: name.into(),
        ty: TypeRef::builtin("Boolean"),
    });
    let budgets = VerificationBudgets {
        generated_cases: 7,
        ..Default::default()
    };
    let corpus = dctx0_verification::generate_inputs(&ir, &parameters, &budgets);
    assert_eq!(corpus.bindings.len(), 7);
    assert!(corpus.issues.is_empty(), "{:?}", corpus.issues);
    assert!(!corpus.exhaustive);
    assert_eq!(
        corpus,
        dctx0_verification::generate_inputs(&ir, &parameters, &budgets)
    );
    let distinct = corpus
        .bindings
        .iter()
        .map(|value| serde_json::to_string(value).unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(distinct.len(), 7);
}
fn valid(ir: &WorkspaceIr, ty: &TypeRef, corpus: &ValueCorpus, budgets: &VerificationBudgets) {
    for value in &corpus.values {
        assert_eq!(
            check_value(ir, ty, value, budgets),
            ValueContract::Accepted,
            "{ty:?} {value:?}"
        );
    }
    let keys = corpus
        .values
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(keys.len(), corpus.values.len());
    assert!(corpus.values.len() <= budgets.generated_cases);
}
#[test]
fn independent_reference_domains_preserve_finite_enumeration_and_boundaries() {
    let ir = ir();
    let budgets = VerificationBudgets::default();
    let small = generate_values(&ir, &ty("W.Small"), &budgets);
    assert!(small.exhaustive);
    assert!(small.issues.is_empty());
    assert_eq!(
        small.values,
        ["-1", "0", "1"].map(|s| VerificationValue::Integer(s.into()))
    );
    let nested = generate_values(&ir, &ty("W.Nested"), &budgets);
    assert!(nested.exhaustive);
    assert_eq!(nested.values.len(), 2);
    let pair = generate_values(&ir, &ty("W.Pair"), &budgets);
    assert!(pair.exhaustive);
    assert_eq!(pair.values.len(), 4);
    let choice = generate_values(&ir, &ty("W.Choice"), &budgets);
    assert!(choice.exhaustive);
    assert_eq!(choice.values.len(), 5);
    let empty = generate_values(&ir, &ty("W.Empty"), &budgets);
    assert!(empty.exhaustive);
    assert_eq!(empty.values.len(), 1);
    let year = generate_values(&ir, &ty("W.Year"), &budgets);
    assert!(!year.exhaustive);
    assert!(year.issues.is_empty());
    for value in ["1888", "1889", "2099", "2100"] {
        assert!(
            year.values
                .contains(&VerificationValue::Integer(value.into()))
        );
    }
    for (ty, corpus) in [
        (ty("W.Small"), small),
        (ty("W.Nested"), nested),
        (ty("W.Pair"), pair),
        (ty("W.Choice"), choice),
        (ty("W.Empty"), empty),
        (ty("W.Year"), year),
    ] {
        valid(&ir, &ty, &corpus, &budgets);
    }
    let money = generate_values(&ir, &ty("W.Money"), &budgets);
    assert_eq!(
        money.values,
        ["2.1", "2.2"].map(|v| VerificationValue::Decimal(v.into()))
    );
    assert!(!money.exhaustive);
    valid(&ir, &ty("W.Money"), &money, &budgets);
}
#[test]
fn bounded_lists_include_every_size_boundary_and_small_boolean_products() {
    let ir = ir();
    let budgets = VerificationBudgets::default();
    let bools = generic("List", TypeRef::builtin("Boolean"));
    let corpus = generate_values(&ir, &bools, &budgets);
    assert_eq!(corpus.values.len(), 31);
    assert!(!corpus.exhaustive);
    assert!(corpus.issues.is_empty());
    valid(&ir, &bools, &corpus, &budgets);
    let ids = generic("List", TypeRef::builtin("UUID"));
    let corpus = generate_values(&ir, &ids, &budgets);
    for length in 0..=4 {
        assert!(
            corpus
                .values
                .iter()
                .any(|v| matches!(v,VerificationValue::List(values) if values.len()==length))
        );
    }
    valid(&ir, &ids, &corpus, &budgets);
    let option = generic("Option", TypeRef::builtin("Boolean"));
    let corpus = generate_values(&ir, &option, &budgets);
    assert!(corpus.exhaustive);
    assert_eq!(corpus.values.len(), 3);
    valid(&ir, &option, &corpus, &budgets);
    let zero = VerificationBudgets {
        collection_size: 0,
        ..Default::default()
    };
    assert_eq!(
        generate_values(&ir, &bools, &zero).values,
        vec![VerificationValue::List(vec![])]
    );
}
#[test]
fn builtin_values_and_seeded_sampling_are_lossless_deterministic_and_versioned() {
    let ir = ir();
    let budgets = VerificationBudgets::default();
    for name in [
        "Unit",
        "Boolean",
        "String",
        "Int",
        "Decimal",
        "Float",
        "UUID",
        "Timestamp",
    ] {
        let ty = TypeRef::builtin(name);
        let first = generate_values(&ir, &ty, &budgets);
        assert!(!first.values.is_empty());
        assert!(first.issues.is_empty());
        valid(&ir, &ty, &first, &budgets);
        assert_eq!(first, generate_values(&ir, &ty, &budgets));
        assert_eq!(
            serde_json::from_str::<ValueCorpus>(&serde_json::to_string(&first).unwrap()).unwrap(),
            first
        );
    }
    let int = generate_values(&ir, &TypeRef::builtin("Int"), &budgets);
    assert!(
        int.values
            .contains(&VerificationValue::Integer("9007199254740993".into()))
    );
    let different = VerificationBudgets {
        seed: 43,
        ..budgets.clone()
    };
    assert_ne!(
        int.values,
        generate_values(&ir, &TypeRef::builtin("Int"), &different).values
    );
    let limited = VerificationBudgets {
        generated_cases: 1,
        ..Default::default()
    };
    let corpus = generate_values(&ir, &TypeRef::builtin("Boolean"), &limited);
    assert!(!corpus.exhaustive);
    assert_eq!(corpus.values.len(), 1);
    let wide = generate_values(&ir, &ty("W.Wide"), &budgets);
    assert!(!wide.exhaustive);
    valid(&ir, &ty("W.Wide"), &wide, &budgets);
}
#[test]
fn empty_recursive_unsupported_and_resource_limited_domains_cannot_claim_complete_evidence() {
    let ir = ir();
    let budgets = VerificationBudgets::default();
    for ty in [
        ty("W.Cycle"),
        ty("W.Sparse"),
        ty("W.Missing"),
        TypeRef::builtin("FutureBuiltin"),
        TypeRef::builtin("List"),
        TypeRef::builtin("Option"),
    ] {
        let corpus = generate_values(&ir, &ty, &budgets);
        assert!(!corpus.exhaustive);
        assert!(!corpus.issues.is_empty());
    }
    for budgets in [
        VerificationBudgets {
            generated_cases: 0,
            ..Default::default()
        },
        VerificationBudgets {
            evaluation_steps: 0,
            ..Default::default()
        },
        VerificationBudgets {
            collection_size: 100001,
            ..Default::default()
        },
        VerificationBudgets {
            recursion_depth: 0,
            ..Default::default()
        },
        VerificationBudgets {
            value_bytes: 1,
            ..Default::default()
        },
        VerificationBudgets {
            generated_cases: 10,
            evaluation_steps: 10,
            ..Default::default()
        },
    ] {
        let corpus = generate_values(&ir, &ty("W.Wide"), &budgets);
        assert!(!corpus.issues.is_empty());
        assert!(!corpus.exhaustive);
    }
    let huge = VerificationBudgets {
        generated_cases: 100,
        collection_size: 100,
        evaluation_steps: 100,
        ..Default::default()
    };
    let corpus = generate_values(&ir, &generic("List", TypeRef::builtin("Boolean")), &huge);
    assert!(corpus.issues.iter().any(|i| i.code == "GENERATION_BUDGET"));
}
#[test]
fn all_current_cinema_declarations_generate_valid_inputs_including_visible_comment_text() {
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
    assert_eq!(ir.data.len(), 46);
    for id in ir.data.keys() {
        let ty = ty(id);
        let budgets = VerificationBudgets::default();
        let corpus = generate_values(&ir, &ty, &budgets);
        assert!(!corpus.values.is_empty(), "{id}: {:?}", corpus.issues);
        assert!(corpus.issues.is_empty(), "{id}: {:?}", corpus.issues);
        valid(&ir, &ty, &corpus, &budgets);
        if id == "HomeCinema.MovieNight.CommentText" {
            // Independent ECMAScript reference: NEL is retained, BOM is trimmed.
            assert!(
                corpus
                    .values
                    .contains(&VerificationValue::String("\u{0085}".into()))
            );
            assert!(
                !corpus
                    .values
                    .contains(&VerificationValue::String("\u{feff}".into()))
            );
            let rejected = dctx0_verification::generate_rejections(&ir, &ty, &budgets);
            assert!(
                rejected
                    .values
                    .contains(&VerificationValue::String("\u{feff}".into()))
            );
            assert!(
                !rejected
                    .values
                    .contains(&VerificationValue::String("\u{0085}".into()))
            );
        }
    }
}

#[test]
fn malformed_ranges_alias_loops_and_composite_work_exhaustion_are_explicit() {
    let mut workspace = ir();
    let range = workspace
        .nodes
        .iter()
        .position(|n| n.owner == "W.Small" && n.kind == StatementKind::Range)
        .unwrap();
    let original = workspace.nodes[range].terms.clone();
    for (text, code) in [("1.5..2.5", "UNSUPPORTED_RANGE"), ("2..1", "INVALID_RANGE")] {
        let mut terms = original.clone();
        terms.truncate(1);
        terms[0].text = text.into();
        workspace.nodes[range].terms = terms;
        let corpus = generate_values(&workspace, &ty("W.Small"), &Default::default());
        assert!(corpus.issues.iter().any(|issue| issue.code == code));
        assert!(!corpus.exhaustive);
        if code == "UNSUPPORTED_RANGE" {
            let rejected = dctx0_verification::generate_rejections(
                &workspace,
                &ty("W.Small"),
                &Default::default(),
            );
            assert!(
                rejected.issues.iter().any(
                    |issue| issue.code == "REJECTION_RANGE_UNSUPPORTED" && issue.span.is_some()
                )
            );
            assert!(!rejected.exhaustive);
        }
    }
    workspace.nodes[range].terms = original;
    workspace.data.get_mut("W.Small").unwrap().alias = Some(TypeRef::builtin("String"));
    let corpus = generate_values(&workspace, &ty("W.Small"), &Default::default());
    assert!(
        corpus
            .issues
            .iter()
            .any(|i| i.code == "UNSUPPORTED_REFINEMENT")
    );
    let rejected =
        dctx0_verification::generate_rejections(&workspace, &ty("W.Small"), &Default::default());
    assert!(
        rejected
            .issues
            .iter()
            .any(|issue| issue.code == "REJECTION_RANGE_UNSUPPORTED")
    );
    assert!(!rejected.exhaustive);
    let mut absent = workspace.clone();
    absent.data.remove("W.Small");
    let rejected =
        dctx0_verification::generate_rejections(&absent, &ty("W.Small"), &Default::default());
    assert!(rejected.values.is_empty());
    assert!(!rejected.issues.is_empty());
    assert!(!rejected.exhaustive);
    workspace.data.get_mut("W.Small").unwrap().alias = Some(ty("W.Small"));
    let corpus = generate_values(&workspace, &ty("W.Small"), &Default::default());
    assert!(corpus.issues.iter().any(|i| i.code == "GENERATION_BUDGET"));
    let budgets = VerificationBudgets {
        generated_cases: 10,
        evaluation_steps: 20,
        ..Default::default()
    };
    for input in [ty("W.Wide"), generic("List", TypeRef::builtin("Boolean"))] {
        let corpus = generate_values(&workspace, &input, &budgets);
        assert!(corpus.issues.iter().any(|i| i.code == "GENERATION_BUDGET"));
        assert!(!corpus.exhaustive);
    }
}

#[test]
fn parameter_tuples_enumerate_small_spaces_and_keep_empty_or_partial_inputs_explicit() {
    use dctx0_verification::generate_inputs;
    let ir = ir();
    let budgets = VerificationBudgets::default();
    let params = [
        FieldIr {
            name: "a".into(),
            ty: TypeRef::builtin("Boolean"),
        },
        FieldIr {
            name: "b".into(),
            ty: TypeRef::builtin("Boolean"),
        },
    ];
    let inputs = generate_inputs(&ir, &params, &budgets);
    assert!(inputs.exhaustive);
    assert!(inputs.issues.is_empty());
    assert_eq!(inputs.bindings.len(), 4);
    assert_eq!(inputs, generate_inputs(&ir, &params, &budgets));
    assert_eq!(
        serde_json::from_str::<dctx0_verification::InputCorpus>(
            &serde_json::to_string(&inputs).unwrap()
        )
        .unwrap(),
        inputs
    );
    let zero = generate_inputs(&ir, &[], &budgets);
    assert!(zero.exhaustive);
    assert_eq!(zero.bindings, vec![std::collections::BTreeMap::new()]);
    let mut bad = params.clone();
    bad[1].name = bad[0].name.clone();
    assert!(generate_inputs(&ir, &bad, &budgets).bindings.is_empty());
    bad[1].name.clear();
    assert!(
        generate_inputs(&ir, &bad, &budgets)
            .issues
            .iter()
            .any(|i| i.code == "INVALID_PARAMETERS")
    );
    assert!(
        generate_inputs(
            &ir,
            &params,
            &VerificationBudgets {
                generated_cases: 0,
                ..Default::default()
            }
        )
        .bindings
        .is_empty()
    );
    let unsupported = [FieldIr {
        name: "unavailable".into(),
        ty: TypeRef::builtin("FutureBuiltin"),
    }];
    let corpus = generate_inputs(&ir, &unsupported, &budgets);
    assert!(corpus.bindings.is_empty());
    assert!(!corpus.issues.is_empty());
    let wide = (0..10)
        .map(|n| FieldIr {
            name: format!("p{n}"),
            ty: TypeRef::builtin("Int"),
        })
        .collect::<Vec<_>>();
    let corpus = generate_inputs(
        &ir,
        &wide,
        &VerificationBudgets {
            generated_cases: 10,
            evaluation_steps: 20,
            ..Default::default()
        },
    );
    assert!(corpus.issues.iter().any(|i| i.code == "GENERATION_BUDGET"));
    let limited = generate_inputs(
        &ir,
        &params,
        &VerificationBudgets {
            generated_cases: 1,
            ..Default::default()
        },
    );
    assert!(!limited.exhaustive);
    assert_eq!(limited.bindings.len(), 1);
}

#[test]
fn text_refinement_corpora_include_valid_format_samples_and_length_boundaries() {
    let compiled = dctx0_language::compile(&[Source {
        file: "formats.dctx0".into(),
        text: r#"workspace W {
 value Name = String { length 2..4 }
 value Email = String { format Email }
 value Identifier = String { format UUID }
 value Link = String { format Uri }
 value Day = String { format Date }
 value Instant = String { format DateTime }
}"#
        .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    for name in ["Name", "Email", "Identifier", "Link", "Day", "Instant"] {
        let ty = ty(&format!("W.{name}"));
        let corpus = generate_values(&ir, &ty, &Default::default());
        assert!(!corpus.values.is_empty(), "{name}: {corpus:?}");
        assert!(corpus.issues.is_empty(), "{name}: {corpus:?}");
        assert!(!corpus.exhaustive);
        valid(&ir, &ty, &corpus, &Default::default());
        assert_eq!(corpus, generate_values(&ir, &ty, &Default::default()));
        if name == "Name" {
            for length in [2, 4] {
                assert!(corpus.values.iter().any(|value| matches!(value, VerificationValue::String(s) if s.encode_utf16().count() == length)));
            }
        }
    }
}

#[test]
fn refinement_rejections_cover_actual_length_and_format_violations_not_only_wrong_types() {
    let compiled = dctx0_language::compile(&[Source {
        file: "negative-formats.dctx0".into(),
        text: r#"workspace W {
 value Text = String { length 1..4 }
 value Entries = List<Int> { length 1..2 }
 value Day = String { format Date }
 value Identifier = String { format UUID }
}"#
        .into(),
    }]);
    assert!(compiled.diagnostics.is_empty());
    let ir = compiled.ir.unwrap();
    let budgets = VerificationBudgets::default();
    for (name, expected) in [
        ("Text", VerificationValue::String("xxxxx".into())),
        ("Day", VerificationValue::String("2026-02-30".into())),
        (
            "Identifier",
            VerificationValue::String("00000000-0000-0000-0000-000000000001".into()),
        ),
    ] {
        let result =
            dctx0_verification::generate_rejections(&ir, &ty(&format!("W.{name}")), &budgets);
        assert!(result.issues.is_empty(), "{result:?}");
        assert!(result.values.contains(&expected), "{name}: {result:?}");
    }
    let result = dctx0_verification::generate_rejections(&ir, &ty("W.Entries"), &budgets);
    assert!(result.values.iter().any(|value| matches!(value, VerificationValue::List(items) if items.len() == 3 && items.iter().all(|item| matches!(item,VerificationValue::Integer(_))))));
}

#[test]
fn all_compatibility_builtins_generate_actual_valid_values_without_target_semantics() {
    let ir = ir();
    let budgets = VerificationBudgets::default();
    for ty in [
        TypeRef::builtin("Duration"),
        TypeRef::builtin("Bytes"),
        TypeRef::builtin("None"),
        generic("Some", TypeRef::builtin("Boolean")),
        generic("Dctx0Option", TypeRef::builtin("Boolean")),
        TypeRef {
            name: "Dctx0Result".into(),
            symbol_id: None,
            arguments: vec![TypeRef::builtin("Boolean"), TypeRef::builtin("String")],
        },
    ] {
        let corpus = generate_values(&ir, &ty, &budgets);
        assert!(corpus.issues.is_empty(), "{ty:?}: {corpus:?}");
        assert!(!corpus.values.is_empty());
        valid(&ir, &ty, &corpus, &budgets);
        assert_eq!(corpus, generate_values(&ir, &ty, &budgets));
    }
}

#[test]
fn finite_float_ranges_generate_endpoints_and_adjacent_outside_values() {
    let result = dctx0_language::compile(&[Source { file: "float-range.dctx0".into(), text: "workspace W { value Weight = Float { range 2.1..2.2 } value Estimate = Duration { range 0..3600 } }".into() }]);
    assert!(result.diagnostics.is_empty());
    let ir = result.ir.unwrap();
    for name in ["Weight", "Estimate"] {
        let ty = ty(&format!("W.{name}"));
        let values = generate_values(&ir, &ty, &Default::default());
        assert!(values.issues.is_empty(), "{values:?}");
        assert!(values.values.len() >= 2);
        valid(&ir, &ty, &values, &Default::default());
        let rejected = dctx0_verification::generate_rejections(&ir, &ty, &Default::default());
        assert!(rejected.issues.is_empty(), "{rejected:?}");
        assert!(
            rejected
                .values
                .iter()
                .filter(|value| matches!(value, VerificationValue::Float(_)))
                .count()
                >= 2
        );
        assert!(rejected.values.iter().all(|value| matches!(
            check_value(&ir, &ty, value, &Default::default()),
            ValueContract::Rejected { .. }
        )));
    }
}

#[test]
fn direct_string_equalities_seed_aliases_and_composites_without_bypassing_contracts() {
    let compiled = dctx0_language::compile(&[Source {
        file: "equality.dctx0".into(),
        text: r#"workspace W {
 value Profile = String { where value == "PlatformJsonV1" }
 value Reverse = String { where "PlatformJsonV1" == value }
 value Alias = Profile
 value Narrow = Alias { length 1..2 }
 value Conflict = Profile { where value == "Other" }
 type Payload { profile: Alias }
 surface Api { operation accept(profile: Alias) -> Alias { pure } }
}"#
        .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let budgets = VerificationBudgets {
        generated_cases: 1,
        ..Default::default()
    };
    for name in ["W.Profile", "W.Reverse", "W.Alias"] {
        let corpus = generate_values(&ir, &ty(name), &budgets);
        assert_eq!(
            corpus.values,
            vec![VerificationValue::String("PlatformJsonV1".into())]
        );
        assert!(corpus.issues.is_empty(), "{:?}", corpus.issues);
        assert!(!corpus.exhaustive);
        valid(&ir, &ty(name), &corpus, &budgets);
    }
    let payload = generate_values(&ir, &ty("W.Payload"), &budgets);
    assert_eq!(payload.values.len(), 1);
    assert!(payload.issues.is_empty());
    valid(&ir, &ty("W.Payload"), &payload, &budgets);
    for name in ["W.Narrow", "W.Conflict"] {
        let corpus = generate_values(&ir, &ty(name), &budgets);
        assert!(corpus.values.is_empty());
        assert!(
            corpus
                .issues
                .iter()
                .any(|issue| issue.code == "EMPTY_CORPUS")
        );
    }
    let inputs = dctx0_verification::generate_inputs(
        &ir,
        &ir.operations["W.Api.accept"].parameters,
        &budgets,
    );
    assert_eq!(inputs.bindings.len(), 1);
    assert!(inputs.issues.is_empty());
    let limited = VerificationBudgets {
        evaluation_steps: 1,
        collection_size: 0,
        ..budgets.clone()
    };
    assert!(
        generate_values(&ir, &ty("W.Profile"), &limited)
            .issues
            .iter()
            .any(|issue| issue.code == "GENERATION_BUDGET")
    );
    let small = VerificationBudgets {
        value_bytes: 4,
        ..budgets.clone()
    };
    assert!(
        generate_values(&ir, &ty("W.Profile"), &small)
            .values
            .is_empty()
    );
}
