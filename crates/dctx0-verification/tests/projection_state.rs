use dctx0_core::*;
use dctx0_verification::*;

fn compile(text: &str) -> WorkspaceIr {
    let compiled = dctx0_language::compile(&[Source {
        file: "state-contract.dctx0".into(),
        text: text.into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    compiled.ir.unwrap()
}

fn project(ir: &WorkspaceIr, subject: &str) -> ProgramPlan {
    project_program(
        ir,
        "pinned-state-contract",
        1,
        VerificationScope {
            subjects: [subject.into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
}

#[test]
fn historical_replacements_report_missing_constructive_sources_before_execution() {
    for clause in [
        "state replaces Item by item.id",
        "state replaces Item from item",
        "state replaces Item by item.id where true from item",
        "state replaces Item by (item.id, item.amount) from item",
    ] {
        let ir = compile(&format!(
            "workspace W {{ entity Item {{ id:Int\namount:Int\nkey id }} port Store {{ operation save(item:Item) -> Unit {{ {clause} }} }} }}"
        ));
        let before = serde_json::to_string(&ir).unwrap();
        let plan = project(&ir, "W.Store.save");
        let issues: Vec<_> = plan
            .issues
            .iter()
            .filter(|issue| issue.code == "STATE_REPLACEMENT_SOURCE_REQUIRED")
            .collect();
        assert_eq!(issues.len(), 1, "{clause}: {:?}", plan.issues);
        assert!(issues[0].message.contains("from Entity"));
        assert_eq!(
            issues[0].span.as_ref().unwrap().file,
            "state-contract.dctx0"
        );
        assert_eq!(serde_json::to_string(&ir).unwrap(), before);
        assert_eq!(plan.operations.len(), 1);
    }
}

#[test]
fn observes_with_write_to_the_same_resolved_entity_reports_actionable_plan_conflict() {
    let ir = compile(
        "workspace W { entity Item { id:Int\namount:Int\nkey id } port Store { operation prepare(item:Item) -> Unit {
            state observes W.Item by item.id
            ensures success implies exists Item(item.id)
            ensures success implies after.Item(item.id).amount == 2
        } effects { read Item write Item } } }",
    );
    let plan = project(&ir, "W.Store.prepare");
    let issue = plan
        .issues
        .iter()
        .find(|issue| issue.code == "STATE_OBSERVES_WRITE_CONFLICT")
        .unwrap();
    assert!(issue.message.contains("W.Store.prepare"));
    assert!(issue.message.contains("W.Item"));
    assert!(issue.message.contains("unchanged"));
    assert!(issue.message.contains("upserts"));
    assert!(issue.span.is_some());
}

#[test]
fn constructive_replacements_and_readers_do_not_inherit_unrelated_write_conflicts() {
    let ir = compile(
        "workspace W { entity Item { id:Int\namount:Int\nkey id }
        entity Other { id:Int\nkey id }
        port Store {
          operation save(item:Item) -> Unit { state replaces Item by item.id from Item { id = item.id amount = item.amount } }
          effects { write Item }
          operation inspect(item:Item) -> Unit { state observes Item by item.id }
          effects { read Item write Other }
          operation conflicting(item:Item) -> Unit { state observes Item by item.id }
          effects { write Item }
        } }",
    );
    for subject in ["W.Store.save", "W.Store.inspect", "W.Other"] {
        let plan = project(&ir, subject);
        assert!(plan.issues.is_empty(), "{subject}: {:?}", plan.issues);
    }
    let plan = project(&ir, "W.Store.conflicting");
    assert!(
        plan.issues
            .iter()
            .any(|issue| issue.code == "STATE_OBSERVES_WRITE_CONFLICT")
    );
}

#[test]
fn complete_workshop_tutorial_has_constructive_replacements() {
    let document = include_str!("../../../docs/site/src/docs/tutorial/complete-workshophub.md");
    let source = document
        .split_once("```dctx0\n")
        .unwrap()
        .1
        .split_once("```")
        .unwrap()
        .0;
    let ir = compile(source);
    for subject in [
        "WorkshopHub.Workshops.WorkshopBooking.WorkshopStore.publish",
        "WorkshopHub.Workshops.WorkshopBooking.WorkshopStore.cancel",
    ] {
        let plan = project(&ir, subject);
        assert!(
            !plan.issues.iter().any(|issue| matches!(
                issue.code.as_str(),
                "STATE_REPLACEMENT_SOURCE_REQUIRED" | "STATE_OBSERVES_WRITE_CONFLICT"
            )),
            "{:?}",
            plan.issues
        );
        assert!(
            plan.operations
                .iter()
                .any(|operation| operation.operation.id == subject)
        );
    }
}
