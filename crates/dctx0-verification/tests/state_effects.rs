use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
use dctx0_verification::*;
use std::collections::{BTreeMap, BTreeSet};
fn model(mode: &str, result: &str) -> (WorkspaceIr, OperationPlan) {
    let text = format!(
        "workspace W {{\nentity Item {{\nid:Int\namount:Int\nkey id\n}}\nport Store {{\noperation apply(item:Item) -> {result} {{\nstate {mode} Item{}\n}}\n}}\n}}",
        if matches!(mode, "inserts" | "upserts") {
            " from item"
        } else if matches!(mode, "observes" | "removes" | "replaces") {
            " by item.id"
        } else {
            ""
        }
    );
    let c = dctx0_language::compile(&[Source {
        file: "states.dctx0".into(),
        text,
    }]);
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    let ir = c.ir.unwrap();
    let p = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: BTreeSet::from(["W.Store.apply".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    );
    (ir, p.operations.into_iter().next().unwrap())
}
fn row(id: &str, amount: &str) -> VerificationValue {
    VerificationValue::Record(BTreeMap::from([
        ("id".into(), VerificationValue::Integer(id.into())),
        ("amount".into(), VerificationValue::Integer(amount.into())),
    ]))
}
fn context(before: Vec<VerificationValue>, after: Vec<VerificationValue>) -> ObservationContext {
    ObservationContext {
        resources: None,
        before: Some(BTreeMap::from([("W.Item".into(), before)])),
        after: Some(BTreeMap::from([("W.Item".into(), after)])),
        ..Default::default()
    }
}
fn observed(result: VerificationValue) -> InvocationObservation {
    InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.apply".into(),
            arguments: vec![row("1", "2")],
        },
        observation: Observation::Returned { value: result },
    }
}
#[test]
fn incomplete_pinned_state_contracts_never_certify_unchanged_observations() {
    let compiled = dctx0_language::compile(&[Source {
        file: "incomplete-state.dctx0".into(),
        text: "workspace W {\nentity Item {\nid:Int\namount:Int\nkey id\n}\nsurface Api { operation inspect() -> Unit {\nstate lists Item\n} }\n}".into(),
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
            subjects: ["W.Api.inspect".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let invocation = InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.inspect".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Unit,
        },
    };
    let state = context(vec![row("1", "2")], vec![row("1", "2")]);
    assert_eq!(
        assess_operation(&ir, &plan, &invocation, &state, &Default::default()).result,
        VerificationResult::Pass
    );
    for missing in [
        "STATE_REFERENCE_REQUIRED",
        "STATE_ENTITY_REQUIRED",
        "STATE_KEY_REQUIRED",
    ] {
        let mut incomplete = ir.clone();
        match missing {
            "STATE_REFERENCE_REQUIRED" => incomplete
                .references
                .retain(|r| r.kind != ReferenceKind::State),
            "STATE_ENTITY_REQUIRED" => {
                incomplete.data.remove("W.Item");
            }
            _ => incomplete.data.get_mut("W.Item").unwrap().keys.clear(),
        }
        let assessed =
            assess_operation(&incomplete, &plan, &invocation, &state, &Default::default());
        assert!(
            matches!(assessed.result, VerificationResult::Unknown { issue } if issue.code == missing),
            "{missing}"
        );
    }
}
#[test]
fn state_result_aliases_keep_the_actual_entity_projection_and_cardinality() {
    for (result_type, result) in [
        ("Alias", row("1", "2")),
        (
            "Maybe",
            VerificationValue::Option(Some(Box::new(row("1", "2")))),
        ),
        ("Many", VerificationValue::List(vec![row("1", "2")])),
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "state-result-alias.dctx0".into(),
            text: format!(
                "workspace W {{\nentity Item {{\nid:Int\namount:Int\nkey id\n}}\nvalue Alias = Item\nvalue Maybe = Option<Alias>\nvalue Many = List<Alias>\nport Store {{ operation apply(item:Item) -> {result_type} {{\nstate observes Item by item.id\n}} }}\n}}"
            ),
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
                subjects: ["W.Store.apply".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .remove(0);
        let snapshot = context(vec![row("1", "2")], vec![row("1", "2")]);
        assert_eq!(
            assess_operation(
                &ir,
                &plan,
                &observed(result),
                &snapshot,
                &Default::default()
            )
            .result,
            VerificationResult::Pass
        );
        let unrelated = match result_type {
            "Maybe" => VerificationValue::Option(Some(Box::new(row("9", "8")))),
            "Many" => VerificationValue::List(vec![row("9", "8")]),
            _ => row("9", "8"),
        };
        assert!(matches!(
            assess_operation(&ir, &plan, &observed(unrelated), &snapshot, &Default::default()).result,
            VerificationResult::Fail { issue } if issue.code == "STATE_RESULT_VIOLATED"
        ));
        let absent = match result_type {
            "Maybe" => VerificationValue::Option(None),
            "Many" => VerificationValue::List(vec![]),
            _ => row("1", "2"),
        };
        let empty = assess_operation(
            &ir,
            &plan,
            &observed(absent),
            &context(vec![], vec![]),
            &Default::default(),
        );
        if result_type == "Alias" {
            assert!(
                matches!(empty.result, VerificationResult::Unknown { issue } if issue.code == "STATE_RESULT_CARDINALITY")
            );
        } else {
            assert_eq!(empty.result, VerificationResult::Pass);
        }
    }
}
#[test]
fn insertion_and_upsert_check_full_state_preserved_rows_and_real_return_values() {
    let budgets = VerificationBudgets::default();
    for mode in ["inserts", "upserts"] {
        let (ir, plan) = model(mode, "Item");
        let good = observed(row("1", "2"));
        let other = row("9", "8");
        assert_eq!(
            assess_operation(
                &ir,
                &plan,
                &good,
                &context(vec![other.clone()], vec![row("1", "2"), other.clone()]),
                &budgets
            )
            .result,
            VerificationResult::Pass
        );
        assert!(
            matches!(assess_operation(&ir,&plan,&good,&context(vec![],vec![]),&budgets).result,VerificationResult::Fail{issue} if issue.code=="STATE_EFFECT_VIOLATED")
        );
        assert!(
            matches!(assess_operation(&ir,&plan,&observed(row("1","3")),&context(vec![],vec![row("1","2")]),&budgets).result,VerificationResult::Fail{issue} if issue.code=="STATE_RESULT_VIOLATED")
        );
        assert!(matches!(
            assess_operation(
                &ir,
                &plan,
                &good,
                &context(vec![other], vec![row("1", "2")]),
                &budgets
            )
            .result,
            VerificationResult::Fail { .. }
        ));
        assert!(
            matches!(assess_operation(&ir,&plan,&good,&ObservationContext::default(),&budgets).result,VerificationResult::Unknown{issue} if issue.code=="STATE_OBSERVER_REQUIRED")
        );
    }
    let (ir, plan) = model("upserts", "Item");
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(row("1", "2")),
            &context(vec![row("1", "1")], vec![row("1", "2")]),
            &budgets
        )
        .result,
        VerificationResult::Pass
    );
    let (ir, plan) = model("inserts", "Item");
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(row("1","2")),&context(vec![row("1","2")],vec![row("1","2")]),&budgets).result,VerificationResult::Fail{issue} if issue.code=="INSERT_IDENTITY_EXISTS")
    );
}
#[test]
fn reads_cannot_mutate_state_or_return_unrelated_entities_and_ambiguity_stays_unknown() {
    let budgets = VerificationBudgets::default();
    let rows = vec![row("1", "2"), row("9", "8")];
    let (ir, plan) = model("observes", "Option<Item>");
    let good = observed(VerificationValue::Option(Some(Box::new(row("1", "2")))));
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &good,
            &context(rows.clone(), rows.clone()),
            &budgets
        )
        .result,
        VerificationResult::Pass
    );
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(VerificationValue::Option(Some(Box::new(row("9","8"))))),&context(rows.clone(),rows.clone()),&budgets).result,VerificationResult::Fail{issue} if issue.code=="STATE_RESULT_VIOLATED")
    );
    assert!(matches!(
        assess_operation(&ir, &plan, &good, &context(rows.clone(), vec![]), &budgets).result,
        VerificationResult::Fail { .. }
    ));
    assert!(
        matches!(assess_operation(&ir,&plan,&good,&context(vec![row("1","2"),row("1","3")],vec![]),&budgets).result,VerificationResult::Unknown{issue} if issue.code=="AMBIGUOUS_STATE_IDENTITY")
    );
    let (ir, plan) = model("lists", "List<Item>");
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(VerificationValue::List(rows.clone())),
            &context(rows.clone(), rows),
            &budgets
        )
        .result,
        VerificationResult::Pass
    );
    let (ir, plan) = model("removes", "Unit");
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(VerificationValue::Unit),
            &context(vec![row("1", "2")], vec![]),
            &budgets
        )
        .result,
        VerificationResult::Pass
    );
    let (ir, plan) = model("replaces", "Item");
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(row("1","2")),&context(vec![row("1","1")],vec![row("1","2")]),&budgets).result,VerificationResult::Unknown{issue} if issue.code=="STATE_REPLACEMENT_SOURCE_REQUIRED")
    );
}

#[test]
fn removal_result_comes_from_the_removed_before_state_and_preserves_other_rows() {
    let (ir, plan) = model("removes", "Item");
    let budgets = VerificationBudgets::default();
    let other = row("9", "8");
    let state = context(vec![row("1", "2"), other.clone()], vec![other]);
    assert_eq!(
        assess_operation(&ir, &plan, &observed(row("1", "2")), &state, &budgets).result,
        VerificationResult::Pass
    );
    assert!(matches!(
        assess_operation(&ir, &plan, &observed(row("1", "3")), &state, &budgets).result,
        VerificationResult::Fail { issue } if issue.code == "STATE_RESULT_VIOLATED"
    ));
}

#[test]
fn explicit_replacement_preserves_identity_and_every_unselected_row() {
    let source = Source {
        file: "replacement.dctx0".into(),
        text: "workspace W {\nentity Item {\nid:Int\namount:Int\nkey id\n}\nport Store {\noperation apply(item:Item) -> Item {\nstate replaces Item by item.id from item\n}\n}\n}".into(),
    };
    let compiled = dctx0_language::compile(&[source]);
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
            subjects: BTreeSet::from(["W.Store.apply".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let before = vec![row("1", "1"), row("9", "8")];
    let after = vec![row("1", "2"), row("9", "8")];
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(row("1", "2")),
            &context(before.clone(), after),
            &Default::default()
        )
        .result,
        VerificationResult::Pass
    );
    assert!(
        matches!(assess_operation(&ir, &plan, &observed(row("1", "2")), &context(before.clone(), vec![row("1", "2"), row("9", "7")]), &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "STATE_EFFECT_VIOLATED")
    );
    assert!(
        matches!(assess_operation(&ir, &plan, &observed(row("1", "2")), &context(vec![row("9", "8")], vec![row("1", "2"), row("9", "8")]), &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "STATE_REPLACEMENT_SELECTION_REQUIRED")
    );
}

#[test]
fn explicit_replacement_cannot_change_the_selected_identity() {
    let compiled = dctx0_language::compile(&[Source {
        file: "replacement-key.dctx0".into(),
        text: "workspace W {\nentity Item {\nid:Int\namount:Int\nkey id\n}\nport Store {\noperation apply(item:Item) -> Item {\nstate replaces Item by item.amount from item\n}\n}\n}".into(),
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
            subjects: BTreeSet::from(["W.Store.apply".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    assert!(
        matches!(assess_operation(&ir, &plan, &observed(row("1", "2")), &context(vec![row("2", "1")], vec![row("1", "2")]), &Default::default()).result, VerificationResult::Unknown { issue } if issue.code == "STATE_REPLACEMENT_IDENTITY")
    );
}

#[test]
fn missing_or_invalid_actual_state_never_satisfies_a_read_contract() {
    let (ir, plan) = model("lists", "Unit");
    let good = observed(VerificationValue::Unit);
    for mut state in [
        context(vec![row("1", "2")], vec![row("1", "2")]),
        context(vec![], vec![]),
    ] {
        state.before = None;
        assert!(
            matches!(assess_operation(&ir, &plan, &good, &state, &Default::default()).result,
            VerificationResult::Unknown {issue} if issue.code == "STATE_OBSERVER_REQUIRED")
        );
        state.before = Some(BTreeMap::from([("W.Item".into(), vec![])]));
        state.after = None;
        assert!(
            matches!(assess_operation(&ir, &plan, &good, &state, &Default::default()).result,
            VerificationResult::Unknown {issue} if issue.code == "STATE_OBSERVER_REQUIRED")
        );
    }
    for invalid in [
        row("1", "not-an-integer"),
        VerificationValue::Record(BTreeMap::from([(
            "id".into(),
            VerificationValue::Integer("1".into()),
        )])),
    ] {
        for state in [
            context(vec![invalid.clone()], vec![]),
            context(vec![], vec![invalid]),
        ] {
            assert!(
                matches!(assess_operation(&ir, &plan, &good, &state, &Default::default()).result,
                VerificationResult::Fail {issue} if issue.code == "INVALID_OBSERVED_STATE")
            );
        }
    }
}

#[test]
fn duplicate_entity_identity_cannot_prove_a_read_even_with_matching_snapshots() {
    let (ir, plan) = model("lists", "Unit");
    let invocation = observed(VerificationValue::Unit);
    for duplicate in [row("1", "2"), row("1", "99")] {
        let ambiguous = vec![row("1", "2"), duplicate];
        for state in [
            context(ambiguous.clone(), vec![row("1", "2")]),
            context(vec![row("1", "2")], ambiguous.clone()),
            context(ambiguous.clone(), ambiguous),
        ] {
            assert!(matches!(
                assess_operation(&ir, &plan, &invocation, &state, &Default::default()).result,
                VerificationResult::Unknown { issue }
                    if issue.code == "AMBIGUOUS_STATE_IDENTITY"
            ));
        }
    }
    // Observer row order is not an entity-state change.
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &invocation,
            &context(
                vec![row("1", "2"), row("2", "3")],
                vec![row("2", "3"), row("1", "2")]
            ),
            &Default::default(),
        )
        .result,
        VerificationResult::Pass
    );
}

#[test]
fn filtered_dto_projection_is_derived_from_actual_state_and_rejects_wrong_items_or_total() {
    let compiled = dctx0_language::compile(&[Source {
        file: "filtered-overview.dctx0".into(),
        text: "workspace W {\nentity Item { id:Int\namount:Int\nkey id }\nmessage View { id:Int\namount:Int }\nmessage Overview { items:List<View>\ntotal:Int }\nport Store { operation apply(item:Item) -> Overview {\nstate lists Item\nensures result == Overview { items = map(View { id = selected.id, amount = selected.amount } for selected in filter(row in before.Item where row.id == item.id)), total = count(row in before.Item where row.id == item.id) }\n} } }".into(),
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
            subjects: ["W.Store.apply".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let result = |rows: Vec<VerificationValue>, total: &str| {
        VerificationValue::Record(BTreeMap::from([
            ("items".into(), VerificationValue::List(rows)),
            ("total".into(), VerificationValue::Integer(total.into())),
        ]))
    };
    let rows = vec![row("9", "8"), row("1", "2")];
    let snapshot = context(rows.clone(), rows);
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(result(vec![row("1", "2")], "1")),
            &snapshot,
            &Default::default()
        )
        .result,
        VerificationResult::Pass
    );
    for actual in [
        result(vec![row("9", "8")], "1"),
        result(vec![row("1", "2")], "2"),
        result(vec![], "0"),
    ] {
        assert!(matches!(
            assess_operation(
                &ir,
                &plan,
                &observed(actual),
                &snapshot,
                &Default::default()
            )
            .result,
            VerificationResult::Fail { .. }
        ));
    }
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(result(vec![], "0")),
            &context(vec![], vec![]),
            &Default::default()
        )
        .result,
        VerificationResult::Pass
    );
}

#[test]
fn incomplete_state_selectors_and_sources_cannot_validate_unchanged_snapshots() {
    for (mode, result, expected) in [
        ("observes", "Item", "STATE_SELECTOR_REQUIRED"),
        ("removes", "Unit", "STATE_SELECTOR_REQUIRED"),
        ("inserts", "Item", "STATE_SOURCE_REQUIRED"),
        ("upserts", "Item", "STATE_SOURCE_REQUIRED"),
    ] {
        let (ir, mut plan) = model(mode, result);
        let clause = plan
            .clauses
            .iter_mut()
            .find(|node| node.kind == StatementKind::State)
            .unwrap();
        clause.expressions.clear();
        let returned = if result == "Unit" {
            VerificationValue::Unit
        } else {
            row("1", "2")
        };
        let state = context(vec![row("1", "2")], vec![row("1", "2")]);
        assert!(
            matches!(assess_operation(&ir, &plan, &observed(returned), &state, &Default::default()).result,
            VerificationResult::Unknown { issue } if issue.code == expected),
            "{mode}"
        );
    }
    let (mut ir, plan) = model("lists", "Unit");
    ir.data.get_mut("W.Item").unwrap().keys.clear();
    assert!(
        matches!(assess_operation(&ir, &plan, &observed(VerificationValue::Unit),
        &context(vec![row("1", "2")], vec![row("1", "2")]), &Default::default()).result,
        VerificationResult::Unknown { issue } if issue.code == "STATE_KEY_REQUIRED")
    );
}

#[test]
fn recursive_observed_state_exceeding_the_budget_is_unknown_even_when_unchanged() {
    let compiled = dctx0_language::compile(&[Source {
        file: "recursive-state.dctx0".into(),
        text: "workspace W { entity Item { id:Int\nchildren:List<Item>\nkey id } port Store { operation read() -> Unit { state lists Item } } }".into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let program = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: ["W.Store.read".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let mut nested = VerificationValue::List(vec![]);
    for _ in 0..10 {
        nested = VerificationValue::List(vec![VerificationValue::Record(BTreeMap::from([
            ("id".into(), VerificationValue::Integer("1".into())),
            ("children".into(), nested),
        ]))]);
    }
    let VerificationValue::List(rows) = nested else {
        unreachable!()
    };
    let actual = InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.read".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Unit,
        },
    };
    let assessment = assess_operation(
        &ir,
        &program.operations[0],
        &actual,
        &context(rows.clone(), rows),
        &Default::default(),
    );
    assert!(
        matches!(&assessment.result,
        VerificationResult::Unknown { issue } if issue.code == "VALUE_BUDGET"),
        "{:?}",
        assessment.result
    );
}

#[test]
fn absent_selected_entities_return_none_but_cannot_satisfy_direct_entity_results() {
    let (ir, plan) = model("observes", "Option<Item>");
    let absent = context(vec![row("9", "8")], vec![row("9", "8")]);
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(VerificationValue::Option(None)),
            &absent,
            &Default::default()
        )
        .result,
        VerificationResult::Pass
    );
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(VerificationValue::Option(Some(Box::new(row("1","2"))))),&absent,&Default::default()).result,
        VerificationResult::Fail {issue} if issue.code == "STATE_RESULT_VIOLATED")
    );
    let (ir, plan) = model("observes", "Item");
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(row("1","2")),&absent,&Default::default()).result,
        VerificationResult::Unknown {issue} if issue.code == "STATE_RESULT_CARDINALITY")
    );
    let (ir, plan) = model("lists", "Item");
    let rows = vec![row("1", "2"), row("9", "8")];
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(row("1","2")),&context(rows.clone(),rows),&Default::default()).result,
        VerificationResult::Unknown {issue} if issue.code == "STATE_RESULT_CARDINALITY")
    );
}

#[test]
fn unchanged_observations_cannot_prove_a_different_result_projection() {
    let (ir, plan) = model("lists", "Int");
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(VerificationValue::Integer("1".into())),&context(vec![row("1","2")],vec![row("1","2")]),&Default::default()).result,
        VerificationResult::Unknown {issue} if issue.code == "STATE_RESULT_PROJECTION_REQUIRED")
    );
    let (ir, plan) = model("lists", "List<Item>");
    assert!(
        matches!(assess_operation(&ir,&plan,&observed(VerificationValue::List(vec![])),&context(vec![row("1","2")],vec![row("1","2")]),&Default::default()).result,
        VerificationResult::Fail {issue} if issue.code == "STATE_RESULT_VIOLATED")
    );
}

#[test]
fn explicit_result_projection_uses_actual_state_and_never_its_own_result() {
    for condition in [
        "result == before.Item(item.id).amount",
        "success implies before.Item(item.id).amount == result",
        "result == result",
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "projection.dctx0".into(),
            text: format!(
                "workspace W {{\nentity Item {{ id:Int\namount:Int\nkey id }}\nport Store {{ operation apply(item:Item) -> Int {{\nstate lists Item\nensures {condition}\n}} }} }}"
            ),
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
                subjects: ["W.Store.apply".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .remove(0);
        let state = context(
            vec![row("1", "2"), row("9", "8")],
            vec![row("1", "2"), row("9", "8")],
        );
        let result = assess_operation(
            &ir,
            &plan,
            &observed(VerificationValue::Integer("2".into())),
            &state,
            &Default::default(),
        )
        .result;
        if condition == "result == result" {
            assert!(
                matches!(result, VerificationResult::Unknown { .. }),
                "{result:?}"
            );
        } else {
            assert_eq!(result, VerificationResult::Pass);
            assert!(matches!(
                assess_operation(
                    &ir,
                    &plan,
                    &observed(VerificationValue::Integer("3".into())),
                    &state,
                    &Default::default()
                )
                .result,
                VerificationResult::Fail { .. }
            ));
        }
    }
}

#[test]
fn projection_ambiguity_and_missing_state_never_hide_contradictory_equalities() {
    for (conditions, rows, expected) in [
        (
            "ensures result == before.Item(item.id).amount",
            vec![],
            "UNKNOWN",
        ),
        (
            "ensures result == before.Item(item.id).amount\nensures result == item.amount",
            vec![row("1", "2")],
            "UNKNOWN",
        ),
        (
            "ensures result == before.Item(item.id).amount\nensures result == 3",
            vec![row("1", "2")],
            "FAIL",
        ),
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "projection-boundaries.dctx0".into(),
            text: format!(
                "workspace W {{\nentity Item {{ id:Int\namount:Int\nkey id }}\nport Store {{ operation apply(item:Item) -> Int {{\nstate lists Item\n{conditions}\n}} }} }}"
            ),
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
                subjects: ["W.Store.apply".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .remove(0);
        let result = assess_operation(
            &ir,
            &plan,
            &observed(VerificationValue::Integer("2".into())),
            &context(rows.clone(), rows),
            &Default::default(),
        )
        .result;
        assert!(
            matches!(
                (&result, expected),
                (VerificationResult::Unknown { .. }, "UNKNOWN")
                    | (VerificationResult::Fail { .. }, "FAIL")
            ),
            "{conditions}: {result:?}"
        );
    }
}
#[test]
fn composite_rating_identity_preserves_other_films_for_the_same_member() {
    let compiled=dctx0_language::compile(&[Source {file:"ratings.dctx0".into(),text:"workspace W { entity Rating { filmId:Int\nmemberId:Int\nstars:Int\nkey filmId memberId } port Store { operation apply(request:Rating) -> Unit { state upserts Rating from request } } }".into()}]);
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
            subjects: ["W.Store.apply".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let rating = |film: &str, stars: &str| {
        VerificationValue::Record(BTreeMap::from([
            ("filmId".into(), VerificationValue::Integer(film.into())),
            ("memberId".into(), VerificationValue::Integer("10".into())),
            ("stars".into(), VerificationValue::Integer(stars.into())),
        ]))
    };
    let invocation = InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.apply".into(),
            arguments: vec![rating("1", "5")],
        },
        observation: Observation::Returned {
            value: VerificationValue::Unit,
        },
    };
    let before = vec![rating("1", "3"), rating("2", "1")];
    for (after, passes) in [
        (vec![rating("1", "5"), rating("2", "1")], true),
        (vec![rating("1", "5")], false),
        (vec![rating("1", "5"), rating("2", "5")], false),
    ] {
        let state = ObservationContext {
            before: Some(BTreeMap::from([("W.Rating".into(), before.clone())])),
            after: Some(BTreeMap::from([("W.Rating".into(), after)])),
            ..Default::default()
        };
        let assessed = assess_operation(&ir, &plan, &invocation, &state, &Default::default());
        if passes {
            assert_eq!(assessed.result, VerificationResult::Pass);
        } else {
            assert!(
                matches!(assessed.result,VerificationResult::Fail {issue} if issue.code=="STATE_EFFECT_VIOLATED")
            );
        }
    }
}
#[test]
fn equivalent_named_record_results_use_real_entity_values_without_inventing_field_mappings() {
    for (fields, matches_shape) in [
        ("id:Int\namount:Int", true),
        ("amount:Int\nid:Int", true),
        ("id:Int\namount:Metric", false),
        ("id:Int\namount:Int\nextra:Int", false),
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "record-result.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} value Metric = Int\nmessage View {{ {fields} }}\nport Store {{ operation apply(item:Item) -> View {{ state observes Item by item.id }} }} }}"
            ),
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
                subjects: ["W.Store.apply".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .remove(0);
        let mut value = row("1", "2");
        if fields.contains("extra") {
            if let VerificationValue::Record(fields) = &mut value {
                fields.insert("extra".into(), VerificationValue::Integer("3".into()));
            }
        }
        let snapshot = context(vec![row("1", "2")], vec![row("1", "2")]);
        let assessed =
            assess_operation(&ir, &plan, &observed(value), &snapshot, &Default::default());
        if matches_shape {
            assert_eq!(assessed.result, VerificationResult::Pass);
        } else {
            assert!(
                matches!(assessed.result,VerificationResult::Unknown {issue} if issue.code=="STATE_RESULT_PROJECTION_REQUIRED")
            );
        }
        if matches_shape {
            assert!(
                matches!(assess_operation(&ir,&plan,&observed(row("1","99")),&snapshot,&Default::default()).result,VerificationResult::Fail {issue} if issue.code=="STATE_RESULT_VIOLATED")
            );
        }
    }
}
#[test]
fn paginated_projection_keeps_total_before_slicing_and_rejects_wrong_pages() {
    let compiled=dctx0_language::compile(&[Source {file:"paging.dctx0".into(),text:"workspace W { entity Item { id:Int\namount:Int\nkey id } message Overview { items:List<Item>\ntotal:Int } port Store { operation list(offset:Int,limit:Int) -> Overview { state lists Item\nensures result == Overview { items = slice(filter(row in before.Item where row.amount > 0),offset,limit), total = count(row in before.Item where row.amount > 0) } } } }".into()}]);
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
            subjects: ["W.Store.list".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let rows = vec![row("1", "2"), row("2", "3"), row("3", "4")];
    let snapshot = context(rows.clone(), rows);
    for (offset, items, total, passes) in [
        ("1", vec![row("2", "3")], "3", true),
        ("99", vec![], "3", true),
        ("1", vec![row("1", "2")], "3", false),
        ("1", vec![row("2", "3")], "1", false),
    ] {
        let invocation = InvocationObservation {
            invocation: Invocation {
                subject: plan.operation.id.clone(),
                arguments: vec![
                    VerificationValue::Integer(offset.into()),
                    VerificationValue::Integer("1".into()),
                ],
            },
            observation: Observation::Returned {
                value: VerificationValue::Record(BTreeMap::from([
                    ("items".into(), VerificationValue::List(items)),
                    ("total".into(), VerificationValue::Integer(total.into())),
                ])),
            },
        };
        let assessed = assess_operation(&ir, &plan, &invocation, &snapshot, &Default::default());
        if passes {
            assert_eq!(assessed.result, VerificationResult::Pass);
        } else {
            assert!(matches!(assessed.result, VerificationResult::Fail { .. }));
        }
    }
}
#[test]
fn stable_sort_projection_preserves_a_secondary_identity_order() {
    let compiled=dctx0_language::compile(&[Source {file:"sort.dctx0".into(),text:"workspace W { entity Item { id:Int\namount:Int\nkey id } port Store { operation list() -> List<Item> { state lists Item\nensures result == sortBy(row.amount for row in sortBy(row.id for row in before.Item)) } } }".into()}]);
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
            subjects: ["W.Store.list".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let rows = vec![row("2", "1"), row("1", "1"), row("3", "0")];
    let state = context(rows.clone(), rows);
    for (result, passes) in [
        (vec![row("3", "0"), row("1", "1"), row("2", "1")], true),
        (vec![row("3", "0"), row("2", "1"), row("1", "1")], false),
    ] {
        let invocation = InvocationObservation {
            invocation: Invocation {
                subject: plan.operation.id.clone(),
                arguments: vec![],
            },
            observation: Observation::Returned {
                value: VerificationValue::List(result),
            },
        };
        let assessed = assess_operation(&ir, &plan, &invocation, &state, &Default::default());
        if passes {
            assert_eq!(assessed.result, VerificationResult::Pass);
        } else {
            assert!(matches!(assessed.result, VerificationResult::Fail { .. }));
        }
    }
}

#[test]
fn unavailable_or_incomplete_state_sources_never_certify_an_empty_write() {
    for mode in ["inserts", "upserts"] {
        for missing_variable in [false, true] {
            let (ir, mut plan) = model(mode, "Unit");
            let clause = plan
                .clauses
                .iter_mut()
                .find(|node| node.kind == StatementKind::State)
                .unwrap();
            let source = clause.expressions.first_mut().unwrap();
            source.value = if missing_variable {
                ExpressionKind::Variable {
                    name: "unobservedSource".into(),
                }
            } else {
                // A scalar is not a complete entity, even with empty snapshots.
                ExpressionKind::Variable {
                    name: "success".into(),
                }
            };
            let assessed = assess_operation(
                &ir,
                &plan,
                &observed(VerificationValue::Unit),
                &context(vec![], vec![]),
                &Default::default(),
            );
            assert!(matches!(
                assessed.result,
                VerificationResult::Unknown { .. }
            ));
            if !missing_variable {
                assert!(
                    matches!(assessed.result, VerificationResult::Unknown { issue }
                    if issue.code == "STATE_SOURCE_INCOMPLETE")
                );
            }
        }
    }
}

#[test]
fn state_sources_requiring_an_unobserved_real_call_cannot_certify_empty_writes() {
    for mode in ["inserts", "upserts"] {
        let compiled = dctx0_language::compile(&[Source {
            file: "state-call-source.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} surface Factory {{ operation make(item:Item) -> Item {{ pure }} }} port Store {{ operation apply(item:Item) -> Unit {{ state {mode} Item from Factory.make(item) }} }} }}"
            ),
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
                subjects: ["W.Store.apply".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .into_iter()
        .find(|plan| plan.operation.id == "W.Store.apply")
        .unwrap();
        let assessed = assess_operation(
            &ir,
            &plan,
            &observed(VerificationValue::Unit),
            &context(vec![], vec![]),
            &Default::default(),
        );
        assert!(
            matches!(assessed.result, VerificationResult::Unknown { issue } if issue.code == "STATE_SOURCE_UNAVAILABLE" && issue.span.is_some()),
            "{mode}"
        );
    }
}

#[test]
fn result_projections_do_not_substitute_call_traces_for_expression_observations() {
    for projection in [
        "result == Factory.make(item)",
        "Factory.make(item) == result",
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "state-call-projection.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} surface Factory {{ operation make(item:Item) -> Item {{ pure }} }} port Store {{ operation apply(item:Item) -> Item {{ state lists Item\nensures {projection} }} }} }}"
            ),
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
                subjects: ["W.Store.apply".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .into_iter()
        .find(|plan| plan.operation.id == "W.Store.apply")
        .unwrap();
        let actual = observed(row("1", "2"));
        let mut state = context(vec![], vec![]);
        let unknown = assess_operation(&ir, &plan, &actual, &state, &Default::default());
        assert!(
            matches!(unknown.result, VerificationResult::Unknown { issue } if issue.code == "STATE_RESULT_PROJECTION_REQUIRED" && issue.span.is_some())
        );
        state.calls = Some(vec![InvocationObservation {
            invocation: Invocation {
                subject: "W.Factory.make".into(),
                arguments: vec![row("1", "2")],
            },
            observation: Observation::Returned {
                value: row("1", "2"),
            },
        }]);
        // Trace evidence records what the consumer called. It does not supply
        // the independent invocation requested while evaluating this expression.
        assert!(
            matches!(assess_operation(&ir, &plan, &actual, &state, &Default::default()).result,
            VerificationResult::Unknown { issue } if issue.code == "STATE_RESULT_PROJECTION_REQUIRED")
        );
        state.calls.as_mut().unwrap()[0].observation = Observation::Returned {
            value: row("1", "3"),
        };
        assert!(
            matches!(assess_operation(&ir, &plan, &actual, &state, &Default::default()).result, VerificationResult::Unknown { issue } if issue.code == "STATE_RESULT_PROJECTION_REQUIRED")
        );
    }
}

#[test]
fn unknown_state_effect_modes_do_not_certify_matching_snapshots() {
    let (ir, mut plan) = model("lists", "Unit");
    let clause = plan
        .clauses
        .iter_mut()
        .find(|node| node.kind == StatementKind::State)
        .unwrap();
    clause.terms[0].text = "future-state-mode".into();
    let assessed = assess_operation(
        &ir,
        &plan,
        &observed(VerificationValue::Unit),
        &context(vec![row("1", "2")], vec![row("1", "2")]),
        &Default::default(),
    );
    assert!(
        matches!(assessed.result, VerificationResult::Unknown { issue }
        if issue.code == "STATE_MODE_UNSUPPORTED")
    );
}

#[test]
fn missing_declared_key_fields_cannot_establish_entity_identity() {
    let (mut ir, plan) = model("lists", "Unit");
    ir.data
        .get_mut("W.Item")
        .unwrap()
        .keys
        .push("missingKey".into());
    let assessed = assess_operation(
        &ir,
        &plan,
        &observed(VerificationValue::Unit),
        &context(vec![row("1", "2")], vec![row("1", "2")]),
        &Default::default(),
    );
    assert!(
        matches!(assessed.result, VerificationResult::Unknown { issue }
        if issue.code == "STATE_KEY_REQUIRED")
    );
}

#[test]
fn incomplete_replacement_source_cannot_certify_an_unchanged_entity() {
    let compiled = dctx0_language::compile(&[Source {
        file: "replacement-source.dctx0".into(),
        text: "workspace W { entity Item { id:Int\namount:Int\nkey id } port Store { operation apply(item:Item) -> Unit { state replaces Item by item.id from item } } }".into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let mut plan = project_program(
        &ir,
        "pinned",
        1,
        VerificationScope {
            subjects: ["W.Store.apply".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let clause = plan
        .clauses
        .iter_mut()
        .find(|node| node.kind == StatementKind::State)
        .unwrap();
    assert_eq!(clause.expressions.len(), 2);
    clause.expressions[1].value = ExpressionKind::Variable {
        name: "success".into(),
    };
    let assessed = assess_operation(
        &ir,
        &plan,
        &observed(VerificationValue::Unit),
        &context(vec![row("1", "2")], vec![row("1", "2")]),
        &Default::default(),
    );
    assert!(
        matches!(assessed.result, VerificationResult::Unknown { issue }
        if issue.code == "STATE_SOURCE_INCOMPLETE")
    );
}
