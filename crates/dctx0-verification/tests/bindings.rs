use dctx0_core::*;
use dctx0_verification::*;

fn model() -> WorkspaceIr {
    let compiled = dctx0_language::compile(&[Source {
        file: "bindings.dctx0".into(),
        text: r#"workspace W {
value Id = Int { range 1..3 }
value Row { id: Id }
type Choice = Empty | Some(Row)
failure Broken { reason: String }
entity Item { id: Id
children: List<Item>
key id }
surface Helpers { operation increment(input: Id) -> Int { pure } }
surface Api {
 operation apply(input: Option<List<Choice>>) -> Row ! Broken {
  state lists Item
  requires Helpers.increment(1) > 0
 }
 operation later() -> String
}
}"#
        .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    compiled.ir.unwrap()
}
fn plan(ir: &WorkspaceIr) -> ProgramPlan {
    project_program(
        ir,
        "published",
        4,
        VerificationScope {
            subjects: ["W.Api.apply".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        VerificationBudgets {
            generated_cases: 2,
            ..Default::default()
        },
    )
}

#[test]
fn needs_include_helpers_state_failures_and_recursive_type_closure_without_later_siblings() {
    let ir = model();
    let plan = plan(&ir);
    let before = serde_json::to_string(&ir).unwrap();
    let needs = binding_requirements(&ir, &plan);
    assert_eq!(needs, binding_requirements(&ir, &plan));
    assert_eq!(needs.snapshot, "published");
    assert_eq!(needs.check_occurrence, 4);
    assert!(
        needs
            .requirements
            .iter()
            .any(|r| r.subject == "W.Helpers.increment"
                && r.operation.as_ref() == ir.operations.get(&r.subject))
    );
    assert!(
        needs
            .requirements
            .iter()
            .any(|r| r.subject == "W.Item" && r.role == BindingRole::EntityObserver)
    );
    assert!(
        !needs
            .requirements
            .iter()
            .any(|r| r.subject == "W.Api.later")
    );
    for name in ["W.Row", "W.Id", "W.Item", "W.Broken", "W.Choice"] {
        assert!(needs.declarations.contains_key(name), "{name}");
    }
    assert!(
        needs
            .requirements
            .iter()
            .all(|r| r.span.as_ref().is_some_and(|s| s.file == "bindings.dctx0"))
    );
    assert!(needs.capabilities.contains("state"));
    assert_eq!(before, serde_json::to_string(&ir).unwrap());
}

#[test]
fn missing_or_inconsistent_pinned_requirements_remain_open() {
    let mut ir = model();
    let mut plan = plan(&ir);
    ir.data.remove("W.Choice");
    plan.lifecycles.push(LifecyclePlan {
        subject: "W.Lifecycle".into(),
        entity: "W.Item".into(),
        field: "status".into(),
        states: vec![],
        paths: vec![],
        issues: vec![],
        transitions: vec![LifecycleTransition {
            source_id: "missing".into(),
            operation: "W.Missing".into(),
            from: "A".into(),
            to: "B".into(),
            guard: None,
        }],
    });
    let needs = binding_requirements(&ir, &plan);
    assert!(
        needs
            .issues
            .iter()
            .any(|i| i.code == "BINDING_TYPE_MISSING")
    );
    assert_eq!(
        needs
            .issues
            .iter()
            .filter(|i| i.code == "BINDING_OPERATION_MISSING")
            .count(),
        1
    );
    assert!(!needs.requirements.iter().any(|r| r.subject == "W.Missing"));
}
