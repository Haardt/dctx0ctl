use dctx0_core::*;
use dctx0_verification::*;

#[test]
fn malformed_preconditions_preserve_candidates_and_report_source_bound_unknowns() {
    let compiled = dctx0_language::compile(&[Source {
        file: "eligibility.dctx0".into(),
        text: "workspace W { surface Api { operation read(input:Int) -> Int { requires input > 0 } } }".into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let plan = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: ["W.Api.read".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let operation = &plan.operations[0];
    let requires = operation
        .clauses
        .iter()
        .find(|node| node.kind == StatementKind::Requires)
        .unwrap();
    let bindings: std::collections::BTreeMap<String, VerificationValue> =
        [("input".into(), VerificationValue::Integer("1".into()))].into();
    for missing in [false, true] {
        let mut clause = requires.clone();
        if missing {
            clause.expressions.clear();
        } else {
            clause.expressions[0].value = ExpressionKind::Variable {
                name: "input".into(),
            };
        }
        let corpus = InputCorpus {
            bindings: vec![bindings.clone()],
            exhaustive: true,
            issues: vec![],
        };
        let (selected, selection) =
            select_precondition_inputs(&ir, &[clause], corpus, &Default::default());
        assert_eq!(selected.bindings, vec![bindings.clone()]);
        assert!(!selected.exhaustive);
        let selection = selection.unwrap();
        assert_eq!(selection.unresolved_candidates, 1);
        assert_eq!(selection.accepted_candidates, 0);
        assert_eq!(selection.rejected_candidates, 0);
        assert_eq!(selected.issues.len(), 1);
        assert_eq!(selected.issues[0].span, Some(requires.span.clone()));
        assert_eq!(
            selected.issues[0].code,
            if missing {
                "PRECONDITION_GENERATION_UNSUPPORTED"
            } else {
                "PRECONDITION_GENERATION_NON_BOOLEAN"
            }
        );
    }
}

#[test]
fn state_preconditions_require_complete_exact_independently_checked_evidence() {
    let compiled = dctx0_language::compile(&[Source { file: "state-ready.dctx0".into(), text: "workspace W { entity Item { id: String\nkey id } surface Api { operation read(id:String) -> String { requires exists Item(id) } } }".into() }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let mut plan = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: ["W.Api.read".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let operation = &plan.operations[0];
    let expected = operation
        .inputs
        .issues
        .iter()
        .filter(|issue| issue.code == "STATE_OBSERVER_REQUIRED")
        .cloned()
        .collect::<Vec<_>>();
    assert!(!expected.is_empty());
    let cases = operation
        .inputs
        .bindings
        .iter()
        .enumerate()
        .map(|(index, bindings)| {
            let invocation = Invocation {
                subject: operation.operation.id.clone(),
                arguments: vec![bindings["id"].clone()],
            };
            VerificationCaseRecord {
                id: format!("W.Api.read:operation:{index}"),
                subject: "W.Api.read".into(),
                kind: VerificationCaseKind::Operation,
                bindings: bindings.clone(),
                setup: vec![],
                arguments: vec![invocation.arguments.clone()],
                observations: vec![InvocationObservation {
                    invocation,
                    observation: Observation::Returned {
                        value: bindings["id"].clone(),
                    },
                }],
                events: None,
                resources: None,
                states: vec![VerificationStateEvidence {
                    phase: StatePhase::Before,
                    phase_recorded: Some(true),
                    entities: [(
                        "W.Item".into(),
                        vec![VerificationValue::Record(
                            [("id".into(), bindings["id"].clone())].into(),
                        )],
                    )]
                    .into(),
                }],
                result: VerificationResult::Pass,
                elapsed_us: 0,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(resolved_state_preconditions(&ir, &plan, &cases), expected);
    for mutation in 0..10 {
        let mut bad = cases.clone();
        match mutation {
            0 => {
                bad.pop();
            }
            1 => bad.push(bad[0].clone()),
            2 => bad[0].states[0].phase_recorded = None,
            3 => bad[0].states[0].entities.clear(),
            4 => bad[0].states[0].entities.get_mut("W.Item").unwrap().clear(),
            5 => bad[0].bindings.clear(),
            6 => bad[0].arguments.clear(),
            7 => bad[0].observations.clear(),
            8 => bad[0].setup.push(Invocation {
                subject: "unexpected".into(),
                arguments: vec![],
            }),
            _ => {
                bad[0].result = VerificationResult::Unknown {
                    issue: expected[0].clone(),
                }
            }
        }
        assert!(
            resolved_state_preconditions(&ir, &plan, &bad).is_empty(),
            "Mutation {mutation}"
        );
    }
    plan.operations[0].inputs.issues.push(VerificationIssue {
        code: "MANUAL_REQUIREMENT".into(),
        message: "not runtime state".into(),
        span: None,
    });
    assert_eq!(resolved_state_preconditions(&ir, &plan, &cases), expected);
    plan.operations[0].inputs.bindings.clear();
    assert!(resolved_state_preconditions(&ir, &plan, &cases).is_empty());
}
