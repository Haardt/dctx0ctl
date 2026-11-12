use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
use dctx0_verification::*;
use std::collections::BTreeMap;

fn number(n: i32) -> VerificationValue {
    VerificationValue::Integer(n.to_string())
}
fn row(id: i32, amount: i32) -> VerificationValue {
    VerificationValue::Record(BTreeMap::from([
        ("id".into(), number(id)),
        ("amount".into(), number(amount)),
    ]))
}
fn fixture(source: &str) -> (WorkspaceIr, OperationPlan) {
    let c = dctx0_language::compile(&[Source {
        file: "passthrough.dctx0".into(),
        text: source.into(),
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
    (
        ir,
        p.operations
            .into_iter()
            .find(|p| p.operation.id == "W.Store.apply")
            .unwrap(),
    )
}
fn assess(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    returned: VerificationValue,
    after: Vec<VerificationValue>,
    calls: Option<Vec<InvocationObservation>>,
) -> VerificationResult {
    let obs = InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.apply".into(),
            arguments: vec![row(1, 2)],
        },
        observation: Observation::Returned { value: returned },
    };
    let context = ObservationContext {
        before: Some(BTreeMap::from([("W.Item".into(), vec![row(0, 9)])])),
        after: Some(BTreeMap::from([("W.Item".into(), after)])),
        calls,
        ..Default::default()
    };
    assess_operation(ir, plan, &obs, &context, &Default::default()).result
}
#[test]
fn variant_projection_reads_the_written_entity_and_rejects_mutations() {
    let (ir, p) = fixture(
        "workspace W { entity Item { id:Int\namount:Int\nkey id } type Reply = Saved(Item) | Empty\nport Store { operation apply(item:Item) -> Reply { state upserts Item from item\nensures success implies result == Saved { value = after.Item(item.id) } } } }",
    );
    let saved = |v| VerificationValue::Variant {
        declaration: "W.Reply".into(),
        name: "Saved".into(),
        payload: Some(Box::new(v)),
    };
    let rows = vec![row(0, 9), row(1, 2)];
    assert_eq!(
        assess(&ir, &p, saved(row(1, 2)), rows.clone(), None),
        VerificationResult::Pass
    );
    assert!(matches!(
        assess(&ir, &p, saved(row(1, 3)), rows, None),
        VerificationResult::Fail { .. }
    ));
    assert!(matches!(
        assess(&ir, &p, saved(row(1, 2)), vec![row(0, 9), row(1, 3)], None),
        VerificationResult::Fail { .. }
    ));
}
#[test]
fn projected_trace_requires_a_real_observer_and_distinguishes_empty_tape() {
    let (ir, p) = fixture(
        "workspace W { entity Item { id:Int\namount:Int\nkey id } message Receipt { id:Int\nmatched:Boolean } port Logger { operation record(id:Int) -> Unit } port Store { operation apply(item:Item) -> Receipt { state upserts Item from item\nensures success implies result == Receipt { id = after.Item(item.id).id matched = calls Logger.record(item.id) exactly 1 } } } }",
    );
    let receipt = |matched| {
        VerificationValue::Record(BTreeMap::from([
            ("id".into(), number(1)),
            ("matched".into(), VerificationValue::Boolean(matched)),
        ]))
    };
    let rows = vec![row(0, 9), row(1, 2)];
    let call = InvocationObservation {
        invocation: Invocation {
            subject: "W.Logger.record".into(),
            arguments: vec![number(1)],
        },
        observation: Observation::Returned {
            value: VerificationValue::Unit,
        },
    };
    assert_eq!(
        assess(&ir, &p, receipt(true), rows.clone(), Some(vec![call])),
        VerificationResult::Pass
    );
    assert!(matches!(
        assess(&ir, &p, receipt(true), rows.clone(), Some(vec![])),
        VerificationResult::Fail { .. }
    ));
    assert_eq!(
        assess(&ir, &p, receipt(false), rows.clone(), Some(vec![])),
        VerificationResult::Pass
    );
    assert!(matches!(
        assess(&ir, &p, receipt(false), rows, None),
        VerificationResult::Unknown { .. }
    ));
}
