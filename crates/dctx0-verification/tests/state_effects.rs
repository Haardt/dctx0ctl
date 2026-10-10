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
        matches!(assess_operation(&ir,&plan,&observed(row("1","2")),&context(vec![row("1","2")],vec![row("1","2")]),&budgets).result,VerificationResult::Unknown{issue} if issue.code=="INSERT_IDENTITY_EXISTS")
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
