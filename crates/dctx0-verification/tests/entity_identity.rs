use dctx0_core::*;
use dctx0_verification::*;

fn ir() -> WorkspaceIr {
    let result = dctx0_language::compile(&[Source {
        file: "identity.dctx0".into(),
        text: "workspace W { entity Row { first:Int\nsecond:Int\nkey first second } }".into(),
    }]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.ir.unwrap()
}
fn row(first: &str, second: &str) -> VerificationValue {
    VerificationValue::Record(
        [
            ("first".into(), VerificationValue::Integer(first.into())),
            ("second".into(), VerificationValue::Integer(second.into())),
        ]
        .into(),
    )
}
fn source(rows: Vec<VerificationValue>) -> VerificationCaseRecord {
    VerificationCaseRecord {
        id: "W.Store.write:operation:0".into(),
        subject: "W.Store.write".into(),
        kind: VerificationCaseKind::Operation,
        bindings: Default::default(),
        setup: vec![],
        arguments: vec![],
        events: None,
        resources: None,
        observations: vec![InvocationObservation {
            invocation: Invocation {
                subject: "W.Store.write".into(),
                arguments: vec![],
            },
            observation: Observation::Returned {
                value: VerificationValue::Unit,
            },
        }],
        states: [StatePhase::Before, StatePhase::After]
            .into_iter()
            .map(|phase| VerificationStateEvidence {
                phase,
                phase_recorded: Some(true),
                entities: [("W.Row".into(), rows.clone())].into(),
            })
            .collect(),
        result: VerificationResult::Pass,
        elapsed_us: 0,
    }
}
fn assess(cases: &[VerificationCaseRecord]) -> VerificationCaseRecord {
    entity_identity_case(&ir(), "W.Row", cases, &VerificationBudgets::default())
}

#[test]
fn complete_composite_keys_allow_shared_components_but_reject_duplicate_identity() {
    let original = source(vec![row("1", "2"), row("1", "3"), row("2", "2")]);
    let valid = assess(std::slice::from_ref(&original));
    assert_eq!(valid.result, VerificationResult::Pass);
    assert_eq!(valid.states.len(), 2);
    let duplicate = source(vec![row("1", "2"), row("1", "2")]);
    assert!(
        matches!(assess(&[duplicate]).result, VerificationResult::Fail { issue } if issue.code == "DUPLICATE_ENTITY_IDENTITY")
    );
    let mut invalid = source(vec![row("1", "2")]);
    invalid.states[1].entities.get_mut("W.Row").unwrap()[0] =
        VerificationValue::Record(Default::default());
    assert!(
        matches!(assess(&[invalid]).result, VerificationResult::Fail { issue } if issue.code == "INVALID_ENTITY_IDENTITY_ROW")
    );
}

#[test]
fn empty_or_unrecorded_or_incomplete_sources_cannot_prove_entity_keys() {
    for cases in [vec![], vec![source(vec![])]] {
        assert!(
            matches!(assess(&cases).result, VerificationResult::Unknown { issue } if issue.code == "ENTITY_IDENTITY_OBSERVATION_REQUIRED")
        );
    }
    let complete = source(vec![row("1", "2")]);
    for mutation in 0..5 {
        let mut incomplete = complete.clone();
        match mutation {
            0 => incomplete.states[0].phase_recorded = None,
            1 => incomplete.states.pop().map(|_| ()).unwrap(),
            2 => incomplete.observations.clear(),
            3 => {
                incomplete.result = VerificationResult::Unknown {
                    issue: VerificationIssue {
                        code: "ABORTED".into(),
                        message: "Aborted".into(),
                        span: None,
                    },
                }
            }
            _ => incomplete.kind = VerificationCaseKind::Contract,
        }
        assert!(matches!(
            assess(&[incomplete]).result,
            VerificationResult::Unknown { .. }
        ));
    }
    assert!(
        matches!(entity_identity_case(&ir(), "W.Missing", &[], &VerificationBudgets::default()).result, VerificationResult::Unknown { issue } if issue.code == "ENTITY_KEY_DECLARATION_REQUIRED")
    );
    let budgets = VerificationBudgets {
        evaluation_steps: 1,
        ..Default::default()
    };
    assert!(matches!(
        entity_identity_case(&ir(), "W.Row", &[complete], &budgets).result,
        VerificationResult::Unknown { .. }
    ));
}

#[test]
fn manifest_maps_keys_separately_and_identity_evidence_is_reconstructed_from_sources() {
    let ir = ir();
    let plan = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: ["W.Row".into()].into(),
            tasks: Default::default(),
        },
        &ProjectionFixtures::default(),
        VerificationBudgets::default(),
    );
    let manifest = case_manifest(&plan);
    assert!(plan.capabilities.contains(&Capability::State));
    assert_eq!(
        manifest.case_requirements["W.Row:identity"],
        ["W.Row#key:1".into()].into()
    );
    for (id, requirements) in &manifest.case_requirements {
        if id != "W.Row:identity" {
            assert!(!requirements.contains("W.Row#key:1"));
        }
    }
    let original = source(vec![row("1", "2")]);
    let identity = assess(std::slice::from_ref(&original));
    assert!(validate_entity_identity_cases(
        &ir,
        &plan,
        &[original.clone(), identity.clone()]
    ));
    for mutation in 0..4 {
        let mut wrong = identity.clone();
        match mutation {
            0 => wrong.states.clear(),
            1 => wrong.bindings.clear(),
            2 => {
                wrong.result = VerificationResult::Unknown {
                    issue: VerificationIssue {
                        code: "FORGED".into(),
                        message: "Forged".into(),
                        span: None,
                    },
                }
            }
            _ => wrong.elapsed_us = 1,
        }
        assert!(!validate_entity_identity_cases(
            &ir,
            &plan,
            &[original.clone(), wrong]
        ));
    }
    assert!(!validate_entity_identity_cases(
        &ir,
        &plan,
        std::slice::from_ref(&identity)
    ));
    assert!(!validate_entity_identity_cases(
        &ir,
        &plan,
        &[original, identity.clone(), identity]
    ));
    let mut historical = plan;
    historical.contracts[0].identity_requirements.clear();
    assert!(
        !case_manifest(&historical)
            .required_cases
            .contains("W.Row:identity")
    );
    assert!(validate_entity_identity_cases(&ir, &historical, &[]));
}
