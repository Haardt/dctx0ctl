use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
use dctx0_verification::*;
use std::collections::{BTreeMap, BTreeSet};

fn fixture() -> (WorkspaceIr, OperationPlan, InvocationObservation) {
    let compiled = dctx0_language::compile(&[Source {
        file: "effects.dctx0".into(),
        text: "workspace W {\nevent Changed { id: Int }\nsurface Api {\noperation save(id:Int) -> Int {\nemit Changed(id: id)\n}\n}\n}".into(),
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
            subjects: BTreeSet::from(["W.Api.save".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let observed = InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.save".into(),
            arguments: vec![VerificationValue::Integer("7".into())],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("7".into()),
        },
    };
    (ir, plan, observed)
}
fn event(id: &str) -> VerificationEvent {
    VerificationEvent {
        subject: "W.Changed".into(),
        payload: VerificationValue::Record(BTreeMap::from([(
            "id".into(),
            VerificationValue::Integer(id.into()),
        )])),
    }
}

#[test]
fn modeled_event_payload_is_evaluated_centrally_and_observation_is_mandatory() {
    let (ir, plan, observed) = fixture();
    let budgets = VerificationBudgets::default();
    let assessed = |events| {
        assess_operation(
            &ir,
            &plan,
            &observed,
            &ObservationContext {
                events,
                ..Default::default()
            },
            &budgets,
        )
        .result
    };
    assert_eq!(assessed(Some(vec![event("7")])), VerificationResult::Pass);
    assert!(
        matches!(assessed(None), VerificationResult::Unknown { issue } if issue.code == "EVENT_OBSERVER_REQUIRED")
    );
    for events in [
        vec![],
        vec![event("8")],
        vec![VerificationEvent {
            subject: "Other.Changed".into(),
            payload: event("7").payload,
        }],
    ] {
        assert!(
            matches!(assessed(Some(events)), VerificationResult::Fail { issue } if issue.code == "EVENT_EFFECT_VIOLATED")
        );
    }
}

#[test]
fn emission_alias_uses_a_real_unique_collaborator_result_and_checks_its_contract() {
    let compiled = dctx0_language::compile(&[Source {
        file: "aliases.dctx0".into(),
        text: "workspace W {\nevent Changed { id: Int }\nport Store { operation save(id:Int) -> Int }\nsurface Api {\noperation save(id:Int) -> Int {\ncall Store.save as saved\nemit Changed(id: saved)\n}\n}\n}".into(),
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
            subjects: BTreeSet::from(["W.Api.save".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .into_iter()
    .find(|p| p.operation.id == "W.Api.save")
    .unwrap();
    let (_, _, observed) = fixture();
    let call = InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.save".into(),
            arguments: vec![VerificationValue::Integer("7".into())],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("9".into()),
        },
    };
    let assess = |calls| {
        assess_operation(
            &ir,
            &plan,
            &observed,
            &ObservationContext {
                calls,
                events: Some(vec![event("9")]),
                ..Default::default()
            },
            &Default::default(),
        )
        .result
    };
    assert_eq!(assess(Some(vec![call.clone()])), VerificationResult::Pass);
    assert!(
        matches!(assess(Some(vec![])), VerificationResult::Unknown { issue } if issue.code == "CALL_ALIAS_AMBIGUOUS")
    );
    assert!(
        matches!(assess(Some(vec![call.clone(), call.clone()])), VerificationResult::Unknown { issue } if issue.code == "CALL_ALIAS_AMBIGUOUS")
    );
    assert!(
        matches!(assess(None), VerificationResult::Unknown { issue } if issue.code == "CALL_OBSERVER_REQUIRED")
    );
    let mut invalid = call.clone();
    invalid.invocation.arguments.clear();
    assert!(
        matches!(assess(Some(vec![invalid])), VerificationResult::Fail { issue } if issue.code == "CALL_CONTRACT_VIOLATED")
    );
    let mut invalid = call.clone();
    invalid.observation = Observation::Returned {
        value: VerificationValue::String("9".into()),
    };
    assert!(
        matches!(assess(Some(vec![invalid])), VerificationResult::Fail { issue } if issue.code == "CALL_CONTRACT_VIOLATED")
    );
    let mut unavailable = call.clone();
    unavailable.observation = Observation::Unavailable {
        reason: "missing observation".into(),
    };
    assert!(
        matches!(assess(Some(vec![unavailable])), VerificationResult::Unknown { issue } if issue.code == "CALL_OBSERVATION_UNAVAILABLE")
    );
    let mut thrown = call;
    thrown.observation = Observation::Threw {
        message: "actual exception".into(),
    };
    assert!(
        matches!(assess(Some(vec![thrown])), VerificationResult::Fail { issue } if issue.code == "CALL_EXCEPTION")
    );
}
