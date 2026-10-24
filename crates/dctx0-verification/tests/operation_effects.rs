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
fn unknown_labels_and_unavailable_calls_do_not_hide_known_contract_violations() {
    let compiled = dctx0_language::compile(&[Source {
        file: "mixed-effect-evidence.dctx0".into(),
        text: "workspace W { event Changed { id:Int } port Store { operation load() -> Int } surface Api { operation next() -> Int { call Store.load\nemit Changed(id: 1) } } }".into(),
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
            subjects: ["W.Api.next".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .into_iter()
    .find(|plan| plan.operation.id == "W.Api.next")
    .unwrap();
    let actual = InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.next".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("1".into()),
        },
    };
    let invalid = InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.load".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::String("not an Int".into()),
        },
    };
    for unknown_label in [false, true] {
        let unknown = InvocationObservation {
            invocation: Invocation {
                subject: if unknown_label {
                    "opaque:trace".into()
                } else {
                    "W.Store.load".into()
                },
                arguments: vec![],
            },
            observation: Observation::Unavailable {
                reason: "Missing observation".into(),
            },
        };
        let context = ObservationContext {
            calls: Some(vec![unknown, invalid.clone()]),
            events: Some(vec![event("1")]),
            ..Default::default()
        };
        assert!(
            matches!(assess_operation(&ir, &plan, &actual, &context, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "CALL_CONTRACT_VIOLATED")
        );
    }
    let opaque = VerificationEvent {
        subject: "opaque:event".into(),
        payload: VerificationValue::Unit,
    };
    let malformed = VerificationEvent {
        subject: "W.Changed".into(),
        payload: VerificationValue::Record(
            [("id".into(), VerificationValue::String("not an Int".into()))].into(),
        ),
    };
    for events in [
        vec![opaque.clone(), malformed.clone(), event("1")],
        vec![malformed, opaque, event("1")],
    ] {
        let context = ObservationContext {
            calls: Some(vec![InvocationObservation {
                invocation: invalid.invocation.clone(),
                observation: Observation::Returned {
                    value: VerificationValue::Integer("1".into()),
                },
            }]),
            events: Some(events),
            ..Default::default()
        };
        assert!(
            matches!(assess_operation(&ir, &plan, &actual, &context, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "EVENT_CONTRACT_VIOLATED")
        );
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
                resources: None,
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
                resources: None,
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
        matches!(assess(Some(vec![thrown])), VerificationResult::Unknown { issue } if issue.code == "CALL_EXCEPTION")
    );
}

#[test]
fn delivered_effects_are_checked_even_without_effect_clauses() {
    let (mut ir, mut plan, observed) = fixture();
    plan.clauses.clear();
    ir.references.retain(|reference| {
        !(reference.from == plan.operation.id && reference.kind == ReferenceKind::Emits)
    });
    let assessed = |events| {
        assess_operation(
            &ir,
            &plan,
            &observed,
            &ObservationContext {
                events,
                ..Default::default()
            },
            &Default::default(),
        )
        .result
    };
    assert_eq!(assessed(Some(vec![])), VerificationResult::Pass);
    assert!(
        matches!(assessed(Some(vec![event("7")])), VerificationResult::Fail { issue } if issue.code == "UNDECLARED_OPERATION_EFFECT")
    );
    assert!(
        matches!(assessed(Some(vec![VerificationEvent { subject: "opaque-label".into(), payload: event("7").payload }])), VerificationResult::Unknown { issue } if issue.code == "EFFECT_LABEL_UNMODELED")
    );
    let context = ObservationContext {
        calls: Some(vec![observed.clone()]),
        ..Default::default()
    };
    assert!(
        matches!(assess_operation(&ir, &plan, &observed, &context, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "UNDECLARED_OPERATION_EFFECT")
    );
}

#[test]
fn declared_early_failure_does_not_require_a_success_only_call_alias() {
    for (condition, expected) in [
        ("success implies saved == id", "PASS"),
        ("saved == id", "UNKNOWN"),
        ("calls Store.save(id) exactly 1", "FAIL"),
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "early-failure.dctx0".into(),
            text: format!(
                "workspace W {{\nfailure Broken {{ reason:String }}\nport Store {{ operation save(id:Int) -> Int }}\nsurface Api {{ operation save(id:Int) -> Int ! Broken {{\ncall Store.save as saved\nensures {condition}\n}} }} }}"
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
                subjects: ["W.Api.save".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .into_iter()
        .find(|p| p.operation.id == "W.Api.save")
        .unwrap();
        let observed = InvocationObservation {
            invocation: Invocation {
                subject: "W.Api.save".into(),
                arguments: vec![VerificationValue::Integer("7".into())],
            },
            observation: Observation::Returned {
                value: VerificationValue::Failure {
                    declaration: "W.Broken".into(),
                    payload: Some(Box::new(VerificationValue::Record(
                        [(
                            "reason".into(),
                            VerificationValue::String("not available".into()),
                        )]
                        .into(),
                    ))),
                },
            },
        };
        let context = ObservationContext {
            calls: Some(vec![]),
            events: Some(vec![]),
            ..Default::default()
        };
        let result = assess_operation(&ir, &plan, &observed, &context, &Default::default()).result;
        assert!(
            matches!(
                (&result, expected),
                (VerificationResult::Pass, "PASS")
                    | (VerificationResult::Unknown { .. }, "UNKNOWN")
                    | (VerificationResult::Fail { .. }, "FAIL")
            ),
            "{condition}: {result:?}"
        );
        if expected == "PASS" {
            let success = InvocationObservation {
                observation: Observation::Returned {
                    value: VerificationValue::Integer("7".into()),
                },
                ..observed
            };
            assert!(
                matches!(assess_operation(&ir,&plan,&success,&context,&Default::default()).result,VerificationResult::Unknown{issue} if issue.code=="CALL_ALIAS_AMBIGUOUS")
            );
        }
    }
}

fn failure_effect_fixture() -> (
    WorkspaceIr,
    OperationPlan,
    InvocationObservation,
    ObservationContext,
) {
    let compiled = dctx0_language::compile(&[Source {
        file: "failure-effects.dctx0".into(),
        text: "workspace W { failure Broken { reason:String } event Changed { id:Int } port Store { operation save(id:Int) -> Int ! Broken } surface Api { operation save(id:Int) -> Int ! Broken { call Store.save as saved\nemit Changed(id: saved) } } }".into(),
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
            subjects: ["W.Api.save".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .into_iter()
    .find(|operation| operation.operation.id == "W.Api.save")
    .unwrap();
    let (_, _, observed) = fixture();
    let context = ObservationContext {
        calls: Some(vec![InvocationObservation {
            invocation: Invocation {
                subject: "W.Store.save".into(),
                arguments: observed.invocation.arguments.clone(),
            },
            observation: observed.observation.clone(),
        }]),
        events: Some(vec![event("7")]),
        ..Default::default()
    };
    (ir, plan, observed, context)
}

#[test]
fn collaborator_failure_payloads_are_typed_and_never_fabricate_success_aliases_or_emissions() {
    let (ir, plan, mut observed, mut context) = failure_effect_fixture();
    let failure = VerificationValue::Failure {
        declaration: "W.Broken".into(),
        payload: Some(Box::new(VerificationValue::Record(
            [(
                "reason".into(),
                VerificationValue::String("actual failure".into()),
            )]
            .into(),
        ))),
    };
    observed.observation = Observation::Returned {
        value: failure.clone(),
    };
    context.calls.as_mut().unwrap()[0].observation = Observation::Returned { value: failure };
    context.events = Some(vec![]);
    assert_eq!(
        assess_operation(&ir, &plan, &observed, &context, &Default::default()).result,
        VerificationResult::Pass
    );
    context.events = Some(vec![VerificationEvent {
        subject: "W.Changed".into(),
        payload: VerificationValue::Record(
            [(
                "id".into(),
                VerificationValue::String("invalid event field".into()),
            )]
            .into(),
        ),
    }]);
    assert!(
        matches!(assess_operation(&ir, &plan, &observed, &context, &Default::default()).result,
        VerificationResult::Fail { issue } if issue.code == "EVENT_CONTRACT_VIOLATED")
    );
    context.events = Some(vec![]);
    context.calls.as_mut().unwrap()[0].observation = Observation::Returned {
        value: VerificationValue::Failure {
            declaration: "W.Broken".into(),
            payload: Some(Box::new(VerificationValue::Record(
                [("reason".into(), VerificationValue::Integer("7".into()))].into(),
            ))),
        },
    };
    assert!(
        matches!(assess_operation(&ir, &plan, &observed, &context, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "CALL_CONTRACT_VIOLATED")
    );
}

#[test]
fn incomplete_collaborator_signatures_and_effect_references_stay_unknown() {
    let (ir, plan, observed, context) = failure_effect_fixture();
    for mutation in 0..5 {
        let mut ir = ir.clone();
        let mut context = context.clone();
        let expected = match mutation {
            0 => {
                ir.references.retain(|reference| {
                    !(reference.from == "W.Api.save" && reference.kind == ReferenceKind::Calls)
                });
                context.calls = Some(vec![]);
                "CALL_REFERENCE_REQUIRED"
            }
            1 => {
                ir.operations.remove("W.Store.save");
                "CALL_CONTRACT_REQUIRED"
            }
            2 => {
                ir.operations.get_mut("W.Store.save").unwrap().parameters[0]
                    .ty
                    .symbol_id = Some("missing-type".into());
                "MISSING_TYPE"
            }
            3 => {
                ir.operations
                    .get_mut("W.Store.save")
                    .unwrap()
                    .result
                    .symbol_id = Some("missing-type".into());
                "MISSING_TYPE"
            }
            _ => {
                ir.references.retain(|reference| {
                    !(reference.from == "W.Api.save" && reference.kind == ReferenceKind::Emits)
                });
                context.events = Some(vec![]);
                "EVENT_REFERENCE_REQUIRED"
            }
        };
        let report = assess_operation(&ir, &plan, &observed, &context, &Default::default());
        assert!(
            matches!(report.result, VerificationResult::Unknown { .. }),
            "Mutation {mutation}: {report:?}"
        );
        assert!(
            report.open.iter().any(|issue| issue.code == expected),
            "Mutation {mutation}: {report:?}"
        );
    }
    let mut bad_argument = context;
    bad_argument.calls.as_mut().unwrap()[0].invocation.arguments[0] =
        VerificationValue::String("7".into());
    assert!(
        matches!(assess_operation(&ir, &plan, &observed, &bad_argument, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "CALL_CONTRACT_VIOLATED")
    );
}

#[test]
fn missing_or_partial_event_payload_models_cannot_be_satisfied_by_observed_events() {
    let (ir, plan, observed) = fixture();
    let context = ObservationContext {
        events: Some(vec![event("7")]),
        ..Default::default()
    };
    for mutation in 0..3 {
        let mut plan = plan.clone();
        let emission = plan
            .clauses
            .iter_mut()
            .find(|node| node.kind == StatementKind::Emit)
            .unwrap();
        match mutation {
            0 => emission.syntax = None,
            1 => emission.expressions.clear(),
            _ => {
                let Some(Syntax::Effect { payload, .. }) = &mut emission.syntax else {
                    unreachable!()
                };
                *payload = None;
            }
        }
        assert!(
            matches!(assess_operation(&ir, &plan, &observed, &context, &Default::default()).result, VerificationResult::Unknown { issue } if issue.code == "EVENT_PAYLOAD_REQUIRED"),
            "Mutation {mutation}"
        );
    }
}

#[test]
fn event_payload_calls_need_actual_observations_and_empty_events_keep_their_record_shape() {
    for (clauses, events, expected) in [
        (
            "call Rules.next\nemit Changed(id: Rules.next(id))",
            vec![event("8")],
            "EVENT_PAYLOAD_UNAVAILABLE",
        ),
        (
            "emit Empty()",
            vec![VerificationEvent {
                subject: "W.Empty".into(),
                payload: VerificationValue::Record(BTreeMap::new()),
            }],
            "PASS",
        ),
    ] {
        let compiled = dctx0_language::compile(&[Source {
            file: "payload-calls.dctx0".into(),
            text: format!(
                "workspace W {{ event Changed {{ id:Int }} event Empty {{}} surface Rules {{ operation next(id:Int) -> Int {{ pure }} }} surface Api {{ operation save(id:Int) -> Int {{ {clauses} }} }} }}"
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
                subjects: ["W.Api.save".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .into_iter()
        .find(|operation| operation.operation.id == "W.Api.save")
        .unwrap();
        let (_, _, observed) = fixture();
        let context = ObservationContext {
            events: Some(events),
            calls: Some(vec![]),
            ..Default::default()
        };
        let assessed = assess_operation(&ir, &plan, &observed, &context, &Default::default());
        if expected == "PASS" {
            assert_eq!(assessed.result, VerificationResult::Pass);
        } else {
            assert!(
                matches!(assessed.result, VerificationResult::Unknown { issue } if issue.code == expected)
            );
        }
    }
}

#[test]
fn observed_effects_cannot_fill_missing_caller_or_event_metadata() {
    let (ir, plan, observed, context) = failure_effect_fixture();
    let mut missing_caller = ir.clone();
    missing_caller.nodes.retain(|node| node.id != "W.Api.save");
    assert!(
        matches!(assess_operation(&missing_caller, &plan, &observed, &context, &Default::default()).result,
        VerificationResult::Unknown { issue } if issue.code == "CALL_CONTEXT_REQUIRED")
    );
    let mut missing_event = ir;
    missing_event.data.remove("W.Changed");
    assert!(
        matches!(assess_operation(&missing_event, &plan, &observed, &context, &Default::default()).result,
        VerificationResult::Unknown { issue } if issue.code == "MISSING_TYPE")
    );
}
