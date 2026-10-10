use dctx0_core::{ExpressionKind, Source, StatementKind as K, WorkspaceIr};
use dctx0_language::compile;
use std::{fs, path::Path};
fn sources(source: &str) -> Vec<Source> {
    vec![Source {
        file: "workspace.dctx0".into(),
        text: source.into(),
    }]
}
fn valid(source: &str) -> WorkspaceIr {
    let result = compile(&sources(source));
    assert!(
        result.diagnostics.is_empty(),
        "{source}: {:?}",
        result.diagnostics
    );
    result.ir.unwrap()
}
fn invalid(source: &str, code: &str) {
    let result = compile(&sources(source));
    assert!(result.ir.is_none(), "{source}");
    assert!(
        result.diagnostics.iter().any(|d| d.code == code),
        "{source}: missing {code}: {:?}",
        result.diagnostics
    );
}
#[test]
fn both_complete_specs_match_independent_typescript_semantic_models() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, directory) in [
        ("cinema", "examples/cinema/dctx0"),
        ("runatron-next", "examples/runatron-next/dctx0"),
    ] {
        let mut sources = fs::read_dir(root.join(directory))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "dctx0"))
            .map(|p| Source {
                file: p.file_name().unwrap().to_str().unwrap().into(),
                text: fs::read_to_string(&p).unwrap(),
            })
            .collect::<Vec<_>>();
        sources.sort_by(|a, b| a.file.cmp(&b.file));
        let result = compile(&sources);
        assert!(
            result.diagnostics.is_empty(),
            "{name}: {:?}",
            result.diagnostics
        );
        let ir = result.ir.unwrap();
        let actual = serde_json::json!({"workspace":ir.workspace,"symbol_count":ir.symbols.len(),"data":ir.data,"operations":ir.operations});
        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.join(format!("fixtures/semantic/{name}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected, "{name}");
        for reference in &ir.references {
            let source = &sources
                .iter()
                .find(|s| s.file == reference.span.file)
                .unwrap()
                .text;
            assert_eq!(
                &source[reference.span.start.offset..reference.span.end.offset],
                reference.spelling
            );
        }
        let mut reordered = sources;
        reordered.reverse();
        let same = compile(&reordered).ir.unwrap();
        assert_eq!(ir, same);
    }
}
#[test]
fn composition_resolves_scopes_fragments_and_cross_file_targets() {
    let result = compile(&[
        Source {
            file: "a.dctx0".into(),
            text: "fragment D.M.Api { operation get() -> Item }".into(),
        },
        Source {
            file: "b.dctx0".into(),
            text: "fragment D.M { surface Api {} }".into(),
        },
        Source {
            file: "z.dctx0".into(),
            text: "workspace W { domain D { type Item { id: String } module M {} } }".into(),
        },
    ]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let ir = result.ir.unwrap();
    assert!(ir.operations.contains_key("W.D.M.Api.get"));
    assert_eq!(
        ir.operations["W.D.M.Api.get"].result.symbol_id.as_deref(),
        Some("W.D.Item")
    );
    let ir = valid(
        "workspace W { domain A { type Item = Int; surface Api { operation get() -> Item } } domain B { type Item = String; surface Api { operation get() -> W.A.Item } } }",
    );
    assert_eq!(
        ir.operations["W.B.Api.get"].result.symbol_id.as_deref(),
        Some("W.A.Item")
    );
}
#[test]
fn composition_rejects_unknown_ambiguous_and_duplicate_symbols() {
    assert!(compile(&[]).ir.is_none());
    for source in ["domain D {}", "workspace", "workspace W {} workspace X {}"] {
        invalid(source, "DCTX0_WORKSPACE_COUNT");
    }
    invalid(
        "workspace W { type T = Int; type T = String }",
        "DCTX0_DUPLICATE",
    );
    invalid("workspace W { domain {} }", "DCTX0_NAME_REQUIRED");
    invalid(
        "workspace W {} fragment Missing {}",
        "DCTX0_FRAGMENT_TARGET_UNKNOWN",
    );
    invalid(
        "workspace W { type T = Int } fragment T {}",
        "DCTX0_FRAGMENT_TARGET_KIND",
    );
    invalid(
        "workspace W { domain A { type T = Int } domain B { type T = Int } surface Api { operation get() -> T } }",
        "DCTX0_AMBIGUOUS_REFERENCE",
    );
    invalid(
        "workspace W { type T = Missing }",
        "DCTX0_UNKNOWN_REFERENCE",
    );
}
#[test]
fn nominal_types_keys_aliases_and_stable_ids_are_validated() {
    valid(
        "workspace W { value Id = UUID { stableId \"product.id\" } entity Item { id: Id; key id } type X = A | B(List<Item>) }",
    );
    for (source, code) in [
        (
            "workspace W { value A = B; value B = A }",
            "DCTX0_ALIAS_CYCLE",
        ),
        (
            "workspace W { type T { id: Int; id: String } }",
            "DCTX0_FIELD_DUPLICATE",
        ),
        (
            "workspace W { entity T { id: Int; key missing } }",
            "DCTX0_KEY_UNKNOWN",
        ),
        ("workspace W { value T = List }", "DCTX0_TYPE_ARITY"),
        (
            "workspace W { type T = Int; type X = T<Int> }",
            "DCTX0_TYPE_ARITY",
        ),
        (
            "workspace W { value T = Int { stableId id } }",
            "DCTX0_STABLE_ID_INVALID",
        ),
        (
            "workspace W { value T = Int { stableId \"bad id\" } }",
            "DCTX0_STABLE_ID_INVALID",
        ),
        (
            "workspace W { value T = Int { stableId \"id\"; stableId \"another\" } }",
            "DCTX0_STABLE_ID_DUPLICATE",
        ),
        (
            "workspace W { value A = Int { stableId \"id\" } value B = Int { stableId \"id\" } }",
            "DCTX0_STABLE_ID_DUPLICATE",
        ),
        (
            "workspace W { surface Api { operation get(x: Int,x: Int) -> Int } }",
            "DCTX0_PARAMETER_DUPLICATE",
        ),
        (
            "workspace W { surface Api { operation get() -> Unit ! String } }",
            "DCTX0_UNKNOWN_FAILURE",
        ),
        (
            "workspace W { surface Api exposes invalid {} }",
            "DCTX0_SURFACE_EXPOSURE",
        ),
    ] {
        invalid(source, code);
    }
}
#[test]
fn expressions_preserve_precedence_associativity_types_and_exact_numbers() {
    let ir = valid(
        "workspace W { invariant Arithmetic { assert 1 + 2 * 3 == 7; assert true implies false implies true; assert not false; assert -2 < 0; assert 1.00000000000000000001 > 0 } }",
    );
    let assertions = ir
        .nodes
        .iter()
        .filter(|n| n.kind == K::Assertion)
        .collect::<Vec<_>>();
    let ExpressionKind::Binary { left, .. } = &assertions[0].expressions[0].value else {
        panic!()
    };
    let ExpressionKind::Binary { right, .. } = &left.value else {
        panic!()
    };
    assert!(matches!(&right.value,ExpressionKind::Binary{operator,..}if operator=="*"));
    let ExpressionKind::Binary { right, .. } = &assertions[1].expressions[0].value else {
        panic!()
    };
    assert!(matches!(&right.value,ExpressionKind::Binary{operator,..}if operator=="implies"));
    assert!(
        serde_json::to_string(&assertions[4].expressions)
            .unwrap()
            .contains("1.00000000000000000001")
    );
    for (source, code) in [
        (
            "workspace W { invariant I { assert true + 1 == 2 } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { invariant I { assert unknown == 2 } }",
            "DCTX0_UNBOUND_VARIABLE",
        ),
        (
            "workspace W { invariant I { assert 1 } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { invariant I { assert not 2 } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { invariant I { assert -true } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { invariant I { assert mystery(1) == 1 } }",
            "DCTX0_UNKNOWN_FUNCTION",
        ),
        (
            "workspace W { invariant I { assert length(1) == 1 } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { invariant I { assert startsWith(\"x\") } }",
            "DCTX0_EXPRESSION_ARITY",
        ),
        (
            "workspace W { invariant I { assert true unexpected } }",
            "DCTX0_INVALID_EXPRESSION",
        ),
    ] {
        invalid(source, code);
    }
}
#[test]
fn conditions_collections_records_and_state_references_are_typed() {
    valid(
        "workspace W { entity Item { id: Int; score: Int; key id } surface Api { operation get(id: Int) -> Item { state observes Item by id; requires exists Item(id); ensures success implies result == after.Item(id); ensures preserves Item(id); ensures calls Api.get(id) exactly 1 } } invariant I { forall xs: List<Item>; assert count(x in xs where x.score > 0) >= 0; assert sum(x.score for x in xs) >= 0; assert contains([1,2],1); assert length(reverse([1,2])) == 2; assert all(x in xs where x.score >= 0) } }",
    );
    for (source, code) in [
        (
            "workspace W { type T { id: Int } invariant I { forall x: T; assert x.missing == 1 } }",
            "DCTX0_UNKNOWN_FIELD",
        ),
        (
            "workspace W { surface Api { operation run() -> Unit { requires after.Missing == after.Missing } } }",
            "DCTX0_AFTER_STATE_CONTEXT",
        ),
        (
            "workspace W { invariant I { assert count(1) == 1 } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { type T { id: Int } invariant I { assert T { id = \"bad\" } == T { id = 1 } } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { type T { id: Int } invariant I { assert T {} == T {} } }",
            "DCTX0_RECORD_FIELD_MISSING",
        ),
        (
            "workspace W { type T { id: Int } invariant I { assert T { id = 1 id = 2 } == T { id = 1 } } }",
            "DCTX0_FIELD_DUPLICATE",
        ),
        (
            "workspace W { type T { id: Int } invariant I { assert T { other = 1 } == T { id = 1 } } }",
            "DCTX0_UNKNOWN_FIELD",
        ),
    ] {
        invalid(source, code);
    }
}
#[test]
fn architecture_lifecycles_and_failures_are_checked() {
    for (source, code) in [
        (
            "workspace W { entity E {} module A { owns { E } } module B { owns { E } } }",
            "DCTX0_OWNERSHIP_DUPLICATE",
        ),
        (
            "workspace W { entity E {} surface Api { operation run() -> Unit { pure; read E } } }",
            "DCTX0_PURE_EFFECT",
        ),
        (
            "workspace W { surface Api { operation run() -> Unit { state invalid Unknown } } }",
            "DCTX0_STATE_SYNTAX",
        ),
        (
            "workspace W { entity E {} lifecycle L for E { state A; state B } }",
            "DCTX0_LIFECYCLE_INITIAL",
        ),
        (
            "workspace W { entity E {} lifecycle L for E { state A initial; state A terminal } }",
            "DCTX0_LIFECYCLE_STATE_DUPLICATE",
        ),
        (
            "workspace W { entity E {} surface Api { operation run() -> Unit } lifecycle L for E { state A initial; transition Api.run from A to Missing } }",
            "DCTX0_LIFECYCLE_STATE_UNKNOWN",
        ),
        ("workspace W { test Unknown }", "DCTX0_UNKNOWN_TEST_SUITE"),
        (
            "workspace W { value V = Int { range 1 } }",
            "DCTX0_REFINEMENT_SYNTAX",
        ),
        (
            "workspace W { value V = String { format Unknown } }",
            "DCTX0_FORMAT_UNKNOWN",
        ),
    ] {
        invalid(source, code);
    }
}

#[test]
fn builtin_numeric_collection_and_option_expressions_validate_operand_types() {
    for expression in [
        "1 in 0 .. 2",
        "1.5 + 2 == 3.5",
        "1e2 + 2 == 102",
        "None == None()",
        "Some(1) == Some(1)",
        "[1] == []",
        "round(1.5, 0, HalfUp) == 2",
        "min([1,2]) == 1",
        "max([1,2]) == 2",
        "sum([1,2]) == 3",
        "any([true,false])",
        "count(x in [1,2]) == 2",
        "(1 + 2) == 3",
        "trim(\" x \") == \"x\"",
    ] {
        valid(&format!(
            "workspace W {{ invariant I {{ assert {expression} }} }}"
        ));
    }
    for (expression, code) in [
        ("lowercase(1 2)", "DCTX0_INVALID_EXPRESSION"),
        ("reverse(1) == 1", "DCTX0_EXPRESSION_TYPE"),
        ("reverse() == []", "DCTX0_EXPRESSION_ARITY"),
        ("count([1],2) == 1", "DCTX0_EXPRESSION_ARITY"),
        ("count(x in [1] where x) == 1", "DCTX0_EXPRESSION_TYPE"),
        ("1 +", "DCTX0_INVALID_EXPRESSION"),
    ] {
        invalid(
            &format!("workspace W {{ invariant I {{ assert {expression} }} }}"),
            code,
        );
    }
    let nested = format!("{}true{}", "(".repeat(129), ")".repeat(129));
    invalid(
        &format!("workspace W {{ invariant I {{ assert {nested} }} }}"),
        "DCTX0_EXPRESSION_DEPTH",
    );
}

#[test]
fn data_predicates_variants_trace_events_and_state_keys_are_checked() {
    valid(
        "workspace W { value V = Int { where value >= 0 } type R { n: Int; where n >= 0 } invariant I { forall r: R; assert r.n >= 0 } }",
    );
    valid("workspace W { event E { id: Int } invariant I { assert emits E() exactly 1 } }");
    invalid(
        "workspace W { entity E { id: Int; key id } invariant I { assert before.E.id == before.E.id } }",
        "DCTX0_UNKNOWN_FIELD",
    );
    for (source, code) in [
        (
            "workspace W { invariant I { forall x; assert true } }",
            "DCTX0_QUANTIFIER_SYNTAX",
        ),
        (
            "workspace W { type A = On | Off; type B = On | Off; invariant I { assert On == Off } }",
            "DCTX0_AMBIGUOUS_REFERENCE",
        ),
        (
            "workspace W { invariant I { assert emits Missing() exactly 1 } }",
            "DCTX0_UNKNOWN_REFERENCE",
        ),
        (
            "workspace W { entity E { id: Int; key id } invariant I { assert exists E() } }",
            "DCTX0_EXPRESSION_ARITY",
        ),
        (
            "workspace W { entity E { id: Int; key id } invariant I { assert before.E() == before.E() } }",
            "DCTX0_EXPRESSION_ARITY",
        ),
        (
            "workspace W { surface Api { operation get(x: Int) -> Int } invariant I { assert Api.get() == 1 } }",
            "DCTX0_EXPRESSION_ARITY",
        ),
        (
            "workspace W { surface Api { operation get() -> Int } example X { given Api.get() returns \"bad\" } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { example X { expect Success unexpected } }",
            "DCTX0_INVALID_EXPECTATION",
        ),
        (
            "workspace W { event E { id: Int } surface Api { operation run() -> Unit { emit E(other: 1) } } }",
            "DCTX0_UNKNOWN_FIELD",
        ),
        (
            "workspace W { event E { id: Int } surface Api { operation run() -> Unit { emit E(id: \"bad\") } } }",
            "DCTX0_EXPRESSION_TYPE",
        ),
        (
            "workspace W { event E { id: Int } surface Api { operation run() -> Unit { emit E(other: 1) } } }",
            "DCTX0_RECORD_FIELD_MISSING",
        ),
    ] {
        invalid(source, code);
    }
    valid("workspace W { event E = Int; surface Api { operation run() -> Unit { emit E(1) } } }");
    valid(
        "workspace W { surface Api { operation get() -> Int } module M { surface Inner { operation run() -> Unit { observe Api.get } } } }",
    );
    invalid(
        "workspace W { domain D { fragment D {} } }",
        "DCTX0_FRAGMENT_NESTED",
    );
    valid("workspace W { domain D {} } fragment W.D { type T = Int }");
}

#[test]
fn union_payload_constructors_use_scope_expected_types_and_qualified_names() {
    invalid(
        "workspace W { type A = Box(Int) | EmptyA; type B = Box(Int) | EmptyB; invariant I { assert Box { value = 1 } == Box { value = 1 } } }",
        "DCTX0_AMBIGUOUS_REFERENCE",
    );
    valid(
        "workspace W { type A = Box(Int) | EmptyA; type B = Box(String) | EmptyB; invariant I { assert A.Box { value = 1 } == A.Box { value = 1 } } surface Api { operation consume(a: A) -> Unit } example X { when Api.consume(Box { value = 1 }); expect Success } }",
    );
    invalid(
        "workspace W { type A = Yes | No; invariant I { assert A.Unknown == true } }",
        "DCTX0_UNBOUND_VARIABLE",
    );
    invalid(
        "workspace W { invariant I { assert Missing {} == Missing {} } }",
        "DCTX0_UNKNOWN_REFERENCE",
    );
    valid(
        "workspace W { surface Api { operation get() -> Int } review R for Api {} test Examples for Api }",
    );
}

#[test]
fn malformed_obligations_and_duplicate_same_owner_relations_are_handled() {
    valid("workspace W { entity E {} module M { owns { E; E } } }");
    invalid(
        "workspace W { policy P { require } }",
        "DCTX0_REFERENCE_REQUIRED",
    );
    invalid(
        "workspace W { entity E {} lifecycle L for E { state A initial; transition } }",
        "DCTX0_LIFECYCLE_STATE_UNKNOWN",
    );
}

#[test]
fn module_dependencies_unit_results_and_example_errors_are_explicit() {
    valid(
        "workspace W { module A {} module B { requires { A } surface Api { operation run() -> Unit } } }",
    );
    valid(
        "workspace W { failure F; surface Api { operation get() -> Int ! F } example X { given Api.get() fails F; expect result fails F } }",
    );
    valid("workspace W { example X { expect calls Boundary.run(sample) exactly 1 } }");
    valid("workspace W { event E; surface Api { operation run() -> Unit { emit E } } }");
    for source in [
        "workspace W { invariant I { assert Missing.Yes == true } }",
        "workspace W { surface Api { operation get() -> Int } example X { given Api.get() returns 1 + } }",
        "workspace W { example X { given 1 +; expect 1 + } }",
        "workspace W { event E { id: Int } surface Api { operation run() -> Unit { emit E(id: 1 +) } } }",
        "workspace W { event E { id: Int } surface Api { operation run() -> Unit { emit E() } } }",
    ] {
        assert!(compile(&sources(source)).ir.is_none(), "{source}");
    }
    // Incomplete expectation payloads are retained but cannot fabricate a
    // native execution result; modeled failure/event names resolve independently.
    invalid(
        "workspace W { event E; example X { expect result emits E; expect result fails; expect } }",
        "DCTX0_REFERENCE_REQUIRED",
    );
}

#[test]
fn unknown_ownership_and_module_dependency_targets_are_rejected() {
    invalid(
        "workspace W { module M { owns { Unknown } } }",
        "DCTX0_UNKNOWN_REFERENCE",
    );
    invalid(
        "workspace W { module M { requires { Unknown } } }",
        "DCTX0_UNKNOWN_REFERENCE",
    );
}

#[test]
fn flat_operator_and_field_chains_cannot_overflow_expression_trees() {
    let chain = std::iter::repeat_n("1", 200)
        .collect::<Vec<_>>()
        .join(" + ");
    invalid(
        &format!("workspace W {{ invariant I {{ assert {chain} == 200 }} }}"),
        "DCTX0_EXPRESSION_DEPTH",
    );
    let path = format!(
        "r.{}",
        std::iter::repeat_n("n", 200).collect::<Vec<_>>().join(".")
    );
    invalid(
        &format!(
            "workspace W {{ type R {{ n: Int }} invariant I {{ forall r: R; assert {path} == 1 }} }}"
        ),
        "DCTX0_EXPRESSION_DEPTH",
    );
    let path = format!("(r){}", " . n".repeat(200));
    invalid(
        &format!(
            "workspace W {{ type R {{ n: Int }} invariant I {{ forall r: R; assert {path} == 1 }} }}"
        ),
        "DCTX0_EXPRESSION_DEPTH",
    );
}

#[test]
fn empty_collection_results_and_nested_flat_trees_obey_type_and_depth_limits() {
    valid("workspace W { invariant I { assert [1] == reverse([]) } }");
    let chain = std::iter::repeat_n("1", 128)
        .collect::<Vec<_>>()
        .join(" + ");
    invalid(
        &format!("workspace W {{ invariant I {{ assert length([{chain}]) }} }}"),
        "DCTX0_EXPRESSION_DEPTH",
    );
}

#[test]
fn scalar_event_payload_types_are_checked() {
    invalid(
        "workspace W { event E = Int; surface Api { operation run() -> Unit { emit E(\"wrong\") } } }",
        "DCTX0_EXPRESSION_TYPE",
    );
    invalid(
        "workspace W { event E { id: Int } surface Api { operation run() -> Unit { emit E(1) } } }",
        "DCTX0_EXPRESSION_TYPE",
    );
    valid("workspace W { event E; surface Api { operation run() -> Unit { emit E() } } }");
}

#[test]
fn effects_and_policy_requirements_reject_incompatible_declarations() {
    invalid(
        "workspace W { type T = Int; surface Api { operation run() -> Unit { observe T } } }",
        "DCTX0_UNKNOWN_REFERENCE",
    );
    invalid(
        "workspace W { type T = Int; policy P { require T } }",
        "DCTX0_UNKNOWN_REFERENCE",
    );
    let ir = valid(
        "workspace W { surface Api { operation get() -> Int; operation run() -> Unit { observe Api.get } } }",
    );
    assert!(
        ir.references
            .iter()
            .any(|reference| reference.target == "W.Api.get"
                && reference.kind == dctx0_core::ReferenceKind::Observes)
    );
}
