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
fn authority_observation_and_keyed_write_check_the_return_against_the_written_entity() {
    let compiled = dctx0_language::compile(&[Source {
        file: "mixed-state.dctx0".into(),
        text: "workspace W { entity Authority { id:Int\nkey id } entity Item { id:Int\namount:Int\nkey id } message View { key:Int\namount:Int } port Store { operation apply(item:Item) -> View { state observes Authority by 9\nstate upserts Item from Item { id = result.key amount = result.amount }\nensures success implies result.amount == item.amount\nensures success implies result == View { key = after.Item(result.key).id amount = after.Item(result.key).amount } } } }".into(),
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
            subjects: ["W.Store.apply".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let authority =
        VerificationValue::Record([("id".into(), VerificationValue::Integer("9".into()))].into());
    let receipt = |key: &str, amount: &str| {
        VerificationValue::Record(
            [
                ("key".into(), VerificationValue::Integer(key.into())),
                ("amount".into(), VerificationValue::Integer(amount.into())),
            ]
            .into(),
        )
    };
    for (actual, rows, authority_after, pass) in [
        (
            receipt("1", "2"),
            vec![row("1", "2")],
            vec![authority.clone()],
            true,
        ),
        (
            receipt("1", "3"),
            vec![row("1", "2")],
            vec![authority.clone()],
            false,
        ),
        (
            receipt("2", "2"),
            vec![row("1", "2")],
            vec![authority.clone()],
            false,
        ),
        (receipt("1", "2"), vec![], vec![authority.clone()], false),
        (receipt("1", "2"), vec![row("1", "2")], vec![], false),
    ] {
        let context = ObservationContext {
            before: Some(
                [
                    ("W.Authority".into(), vec![authority.clone()]),
                    ("W.Item".into(), vec![]),
                ]
                .into(),
            ),
            after: Some(
                [
                    ("W.Authority".into(), authority_after),
                    ("W.Item".into(), rows),
                ]
                .into(),
            ),
            ..Default::default()
        };
        let assessment = assess_operation(
            &ir,
            &program.operations[0],
            &observed(actual),
            &context,
            &Default::default(),
        );
        assert_eq!(
            assessment.result == VerificationResult::Pass,
            pass,
            "{assessment:?}"
        );
    }
}

#[test]
fn collection_input_can_return_an_independently_constrained_acknowledgement() {
    for (postcondition, extra_fields, supported) in [
        ("success implies result.accepted == request.id", "", true),
        (
            "success implies result.accepted == request.id",
            "items:List<Item>",
            false,
        ),
        (
            "success implies result.accepted == request.id",
            "items:Option<List<Item>>",
            false,
        ),
        (
            "success implies result.accepted == request.id",
            "items:List<ItemView>",
            false,
        ),
        (
            "success implies result.accepted == request.id or true",
            "",
            false,
        ),
        (
            "success implies all(row in before.Item where result.accepted == row.id)",
            "",
            false,
        ),
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "collection-ack.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} message ItemView {{ id:Int\namount:Int }} message Receipt {{ accepted:Int\n{extra_fields} }} port Store {{ operation apply(request:Item) -> Receipt {{ state lists Item\nensures {postcondition} }} }} }}"
            ),
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
                subjects: ["W.Store.apply".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let invocation = |id: &str| InvocationObservation {
            invocation: Invocation {
                subject: "W.Store.apply".into(),
                arguments: vec![row("1", "2")],
            },
            observation: Observation::Returned {
                value: VerificationValue::Record({
                    let mut fields = BTreeMap::from([(
                        "accepted".into(),
                        VerificationValue::Integer(id.into()),
                    )]);
                    if !extra_fields.is_empty() {
                        fields.insert(
                            "items".into(),
                            if extra_fields.contains("Option") {
                                VerificationValue::Option(None)
                            } else {
                                VerificationValue::List(vec![])
                            },
                        );
                    }
                    fields
                }),
            },
        };
        for rows in [vec![], vec![row("2", "3")]] {
            let result = assess_operation(
                &ir,
                &program.operations[0],
                &invocation("1"),
                &context(rows.clone(), rows.clone()),
                &Default::default(),
            );
            assert_eq!(
                result.result == VerificationResult::Pass,
                supported,
                "{postcondition}: {result:?}"
            );
            assert_ne!(
                assess_operation(
                    &ir,
                    &program.operations[0],
                    &invocation("2"),
                    &context(rows.clone(), rows),
                    &Default::default()
                )
                .result,
                VerificationResult::Pass
            );
        }
        for context in [
            ObservationContext::default(),
            context(vec![], vec![row("1", "2")]),
        ] {
            assert_ne!(
                assess_operation(
                    &ir,
                    &program.operations[0],
                    &invocation("1"),
                    &context,
                    &Default::default()
                )
                .result,
                VerificationResult::Pass
            );
        }
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
        let assert_pending = |report: OperationAssessment| {
            assert!(
                matches!(report.result, VerificationResult::Unknown { ref issue }
                if issue.code == "CLAUSE_OBSERVATION_REQUIRED" && issue.span.is_some())
            );
            assert!(
                report
                    .open
                    .iter()
                    .any(|issue| issue.code == "STATE_RESULT_PROJECTION_REQUIRED")
            );
            let pending = report
                .pending_call
                .expect("An independent oracle call is required");
            assert_eq!(pending.invocation.subject, "W.Factory.make");
            assert_eq!(pending.invocation.arguments, actual.invocation.arguments);
            pending
        };
        let pending = assert_pending(assess_operation(
            &ir,
            &plan,
            &actual,
            &state,
            &Default::default(),
        ));
        state.calls = Some(vec![InvocationObservation {
            invocation: pending.invocation.clone(),
            observation: Observation::Returned {
                value: row("1", "2"),
            },
        }]);
        // Consumer traces never become an independent expression-oracle tape.
        assert_pending(assess_operation(
            &ir,
            &plan,
            &actual,
            &state,
            &Default::default(),
        ));
        state.calls.as_mut().unwrap()[0].observation = Observation::Returned {
            value: row("1", "3"),
        };
        assert_pending(assess_operation(
            &ir,
            &plan,
            &actual,
            &state,
            &Default::default(),
        ));
        let tapes = BTreeMap::from([(
            pending.key,
            vec![InvocationObservation {
                invocation: pending.invocation,
                observation: Observation::Returned {
                    value: row("1", "2"),
                },
            }],
        )]);
        let resumed =
            assess_operation_continued(&ir, &plan, &actual, &state, &Default::default(), &tapes);
        assert!(resumed.pending_call.is_none());
        // A proved postcondition does not silently discharge the separate state
        // projection, which still requires its own independent support.
        assert!(
            matches!(resumed.result, VerificationResult::Unknown { issue }
            if issue.code == "STATE_RESULT_PROJECTION_REQUIRED")
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

#[test]
fn constructive_result_projections_follow_conjunctions_but_never_conditional_alternatives_or_self_reference()
 {
    for (postcondition, executable) in [
        (
            "success implies result == View { items = after.Item } and true",
            true,
        ),
        ("true and (result == View { items = after.Item })", true),
        ("result == View { items = after.Item } or true", false),
        ("false implies result == View { items = after.Item }", false),
        ("result == result", false),
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "bounded-projection.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} type View {{ items:List<Item> }} port Store {{ operation apply(item:Item) -> View {{ state lists Item\n ensures {postcondition} }} }} }}"
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
        let actual = observed(VerificationValue::Record(BTreeMap::from([(
            "items".into(),
            VerificationValue::List(vec![row("1", "2")]),
        )])));
        let assessment = assess_operation(
            &ir,
            &plan,
            &actual,
            &context(vec![row("1", "2")], vec![row("1", "2")]),
            &Default::default(),
        );
        if executable {
            assert_eq!(
                assessment.result,
                VerificationResult::Pass,
                "{postcondition}"
            );
        } else {
            assert!(
                matches!(assessment.result, VerificationResult::Unknown { .. }),
                "{postcondition}: {assessment:?}"
            );
        }
        if executable {
            for budgets in [
                VerificationBudgets {
                    recursion_depth: 0,
                    ..Default::default()
                },
                VerificationBudgets {
                    evaluation_steps: 0,
                    ..Default::default()
                },
            ] {
                assert!(matches!(
                    assess_operation(&ir, &plan, &actual, &context(vec![], vec![]), &budgets)
                        .result,
                    VerificationResult::Unknown { .. }
                ));
            }
        }
    }
}

#[test]
fn independent_result_constraints_do_not_turn_observed_authority_into_the_result() {
    let compiled = dctx0_language::compile(&[Source {
        file: "observed-authority.dctx0".into(),
        text: "workspace W { entity Item { id:Int\namount:Int\nkey id } message Receipt { id:Int } port Store { operation apply(item:Item) -> Receipt { state observes Item by item.id\nensures success implies result.id == item.id } } }".into(),
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
    let receipt = |id: &str| {
        VerificationValue::Record(BTreeMap::from([(
            "id".into(),
            VerificationValue::Integer(id.into()),
        )]))
    };
    let before = vec![row("1", "2")];
    let valid = context(before.clone(), before.clone());
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(receipt("1")),
            &valid,
            &Default::default()
        )
        .result,
        VerificationResult::Pass
    );
    assert!(matches!(
        assess_operation(
            &ir,
            &plan,
            &observed(receipt("2")),
            &valid,
            &Default::default()
        )
        .result,
        VerificationResult::Fail { .. }
    ));
    for rows in [vec![], vec![row("3", "2")]] {
        let missing = context(rows.clone(), rows);
        assert!(matches!(
            assess_operation(
                &ir,
                &plan,
                &observed(receipt("1")),
                &missing,
                &Default::default()
            )
            .result,
            VerificationResult::Unknown { .. } | VerificationResult::Fail { .. }
        ));
    }
    let multiple = vec![row("1", "2"), row("3", "2")];
    assert_eq!(
        assess_operation(
            &ir,
            &plan,
            &observed(receipt("1")),
            &context(multiple.clone(), multiple),
            &Default::default()
        )
        .result,
        VerificationResult::Pass
    );
    assert!(
        matches!(assess_operation(&ir, &plan, &observed(receipt("1")), &context(before, vec![row("1", "3")]), &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "STATE_EFFECT_VIOLATED")
    );
    let mut absent = valid;
    absent.after = None;
    assert!(
        matches!(assess_operation(&ir, &plan, &observed(receipt("1")), &absent, &Default::default()).result, VerificationResult::Unknown { issue } if issue.code == "STATE_OBSERVER_REQUIRED")
    );
}

#[test]
fn observed_selected_authority_can_bind_result_inside_nonempty_all() {
    let compiled = dctx0_language::compile(&[Source {
        file: "selected-authority.dctx0".into(),
        text: "workspace W { entity Item { id:Int\namount:Int\nkey id } message Receipt { id:Int\namount:Int\nmarker:Int } port Store { operation apply(item:Item) -> Receipt { state observes Item by item.id\nensures success implies all(gate in before.Item where result.id == gate.id and result.amount == gate.amount) } } }".into(),
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
    let receipt = |id: &str, amount: &str| {
        VerificationValue::Record(BTreeMap::from([
            ("id".into(), VerificationValue::Integer(id.into())),
            ("amount".into(), VerificationValue::Integer(amount.into())),
            ("marker".into(), VerificationValue::Integer("0".into())),
        ]))
    };
    let selected = vec![row("1", "2")];
    let valid = context(selected.clone(), selected.clone());
    let assess = |result, state: &ObservationContext| {
        assess_operation(&ir, &plan, &observed(result), state, &Default::default()).result
    };
    assert_eq!(assess(receipt("1", "2"), &valid), VerificationResult::Pass);
    assert!(matches!(
        assess(receipt("9", "2"), &valid),
        VerificationResult::Fail { .. }
    ));
    assert!(matches!(
        assess(receipt("1", "9"), &valid),
        VerificationResult::Fail { .. }
    ));
    assert!(matches!(
        assess(receipt("1", "2"), &context(vec![], vec![])),
        VerificationResult::Unknown { .. }
    ));
    let mut missing = valid.clone();
    missing.before = None;
    assert!(matches!(
        assess(receipt("1", "2"), &missing),
        VerificationResult::Unknown { .. }
    ));
    assert!(matches!(
        assess(
            receipt("1", "2"),
            &context(selected.clone(), vec![row("1", "9")])
        ),
        VerificationResult::Fail { .. }
    ));
    let small = VerificationBudgets {
        evaluation_steps: 1,
        ..Default::default()
    };
    assert!(matches!(
        assess_operation(&ir, &plan, &observed(receipt("1", "2")), &valid, &small).result,
        VerificationResult::Unknown { .. }
    ));
    assert!(matches!(
        assess(
            receipt("1", "2"),
            &context(vec![row("3", "2")], vec![row("3", "2")])
        ),
        VerificationResult::Unknown { .. } | VerificationResult::Fail { .. }
    ));
    assert!(matches!(
        assess(
            receipt("1", "2"),
            &context(
                vec![row("1", "2"), row("3", "2")],
                vec![row("1", "2"), row("3", "2")]
            )
        ),
        VerificationResult::Unknown { .. } | VerificationResult::Fail { .. }
    ));
}

#[test]
fn quantified_result_binding_does_not_accept_disjunction_or_nested_scope() {
    for condition in [
        "all(gate in before.Item where result.id == gate.id or true)",
        "all(gate in before.Item where all(other in before.Item where result.id == other.id))",
        "all(item in before.Item where result.id == item.id)",
        "all(result in before.Item where result.id == item.id)",
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "quantifier.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} message Receipt {{ id:Int }} port Store {{ operation apply(item:Item) -> Receipt {{ state observes Item by item.id\nensures success implies {condition} }} }} }}"
            ),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{condition}: {:?}",
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
        let receipt = VerificationValue::Record(BTreeMap::from([(
            "id".into(),
            VerificationValue::Integer("1".into()),
        )]));
        let state = context(vec![row("1", "2")], vec![row("1", "2")]);
        assert!(
            matches!(
                assess_operation(&ir, &plan, &observed(receipt), &state, &Default::default())
                    .result,
                VerificationResult::Unknown { .. }
            ),
            "{condition}"
        );
    }
}

#[test]
fn unrelated_state_result_stays_unknown_for_vacuous_or_conditional_constraints() {
    for condition in [
        "result.id == result.id",
        "(result.id == item.id) or true",
        "false implies result.id == item.id",
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "vacuous-observation.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} message Receipt {{ id:Int }} port Store {{ operation apply(item:Item) -> Receipt {{ state observes Item by item.id\nensures {condition} }} }} }}"
            ),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{condition}: {:?}",
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
        let receipt = VerificationValue::Record(BTreeMap::from([(
            "id".into(),
            VerificationValue::Integer("1".into()),
        )]));
        let state = context(vec![row("1", "2")], vec![row("1", "2")]);
        let result =
            assess_operation(&ir, &plan, &observed(receipt), &state, &Default::default()).result;
        assert!(
            matches!(result, VerificationResult::Unknown { .. }),
            "{condition}: {result:?}"
        );
    }
}

#[test]
fn collaborator_alias_equal_to_result_is_not_an_independent_result_binding() {
    let compiled = dctx0_language::compile(&[Source {
        file: "observed-result-alias.dctx0".into(),
        text: "workspace W { entity Item { id:Int\namount:Int\nkey id } message Receipt { id:Int } port Factory { operation copy(item:Item) -> Receipt } port Store { operation apply(item:Item) -> Receipt { call Factory.copy as alias\nstate observes Item by item.id\nensures success implies result.id == alias.id } } }".into(),
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
    let receipt = VerificationValue::Record(BTreeMap::from([(
        "id".into(),
        VerificationValue::Integer("1".into()),
    )]));
    let mut state = context(vec![row("1", "2")], vec![row("1", "2")]);
    state.calls = Some(vec![InvocationObservation {
        invocation: Invocation {
            subject: "W.Factory.copy".into(),
            arguments: vec![row("1", "2")],
        },
        observation: Observation::Returned {
            value: receipt.clone(),
        },
    }]);
    let report = assess_operation(&ir, &plan, &observed(receipt), &state, &Default::default());
    assert!(
        report
            .open
            .iter()
            .any(|issue| issue.code == "STATE_RESULT_PROJECTION_REQUIRED"),
        "{report:?}"
    );
}

#[test]
fn changed_row_projection_selects_the_real_returned_key_without_losing_other_rows() {
    fn receipt(key: &str, amount: &str) -> VerificationValue {
        VerificationValue::Record(BTreeMap::from([
            ("key".into(), VerificationValue::Integer(key.into())),
            ("amount".into(), VerificationValue::Integer(amount.into())),
        ]))
    }
    for (projection, supported) in [
        (
            "View { key = after.Item(result.key).id amount = after.Item(result.key).amount }",
            true,
        ),
        ("result", false),
        (
            "View { key = result.key amount = after.Item(result.key).amount }",
            false,
        ),
        (
            "View { key = before.Item(result.key).id amount = before.Item(result.key).amount }",
            false,
        ),
        (
            "View { key = after.Item(result.key + 0).id amount = after.Item(result.key + 0).amount }",
            false,
        ),
        (
            "View { key = single(after.Item).id amount = single(after.Item).amount }",
            false,
        ),
    ] {
        let c = dctx0_language::compile(&[Source {
            file: "keyed-projection.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} message View {{ key:Int\namount:Int }} port Store {{ operation apply(item:Item) -> View {{ state upserts Item from Item {{ id = result.key amount = result.amount }}\nensures success implies result.amount == item.amount\nensures success implies result == {projection} }} }} }}"
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
        let plan = &p.operations[0];
        let before = vec![row("0", "9")];
        let after = vec![row("0", "9"), row("1", "2")];
        let result = assess_operation(
            &ir,
            plan,
            &observed(receipt("1", "2")),
            &context(before.clone(), after.clone()),
            &Default::default(),
        );
        if supported {
            assert_eq!(result.result, VerificationResult::Pass, "{result:?}");
            for (actual, rows) in [
                (receipt("0", "9"), after.clone()),
                (receipt("1", "3"), after.clone()),
                (receipt("1", "2"), before.clone()),
                (receipt("1", "2"), vec![row("0", "9"), row("1", "3")]),
                (receipt("1", "2"), vec![row("1", "2")]),
                (
                    receipt("1", "2"),
                    vec![row("0", "9"), row("1", "2"), row("2", "4")],
                ),
            ] {
                let result = assess_operation(
                    &ir,
                    plan,
                    &observed(actual),
                    &context(before.clone(), rows),
                    &Default::default(),
                );
                assert!(
                    matches!(result.result, VerificationResult::Fail { .. }),
                    "{result:?}"
                );
            }
            for budgets in [
                VerificationBudgets {
                    recursion_depth: 1,
                    ..Default::default()
                },
                VerificationBudgets {
                    evaluation_steps: 1,
                    ..Default::default()
                },
            ] {
                assert!(matches!(
                    assess_operation(
                        &ir,
                        plan,
                        &observed(receipt("1", "2")),
                        &context(before.clone(), after.clone()),
                        &budgets
                    )
                    .result,
                    VerificationResult::Unknown { .. }
                ));
            }
        } else {
            assert!(
                matches!(result.result, VerificationResult::Unknown { .. }),
                "{projection}: {result:?}"
            );
        }
    }
}
