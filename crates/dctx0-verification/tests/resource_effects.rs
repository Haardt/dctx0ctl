use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
use dctx0_verification::*;
use std::collections::BTreeSet;

#[test]
fn read_write_and_observe_permissions_use_their_declared_access_kind() {
    for (clause, access) in [
        ("read", ResourceAccess::Read),
        ("write", ResourceAccess::Write),
        ("observe", ResourceAccess::Observe),
    ] {
        let target = if access == ResourceAccess::Observe {
            "Store"
        } else {
            "Item"
        };
        let compiled = dctx0_language::compile(&[Source {
            file: "access-kind.dctx0".into(),
            text: format!(
                "workspace W {{ entity Item {{ id:Int }} port Store {{ operation ping() -> Unit }} surface Api {{ operation work() -> Unit {{\n{clause} {target}\n}} }} }}"
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
                subjects: ["W.Api.work".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .remove(0);
        let actual = InvocationObservation {
            invocation: Invocation {
                subject: "W.Api.work".into(),
                arguments: vec![],
            },
            observation: Observation::Returned {
                value: VerificationValue::Unit,
            },
        };
        let context = ObservationContext {
            resources: Some(vec![ResourceObservation {
                operation: None,
                resource: format!("W.{target}"),
                access,
            }]),
            ..Default::default()
        };
        assert_eq!(
            assess_operation(&ir, &plan, &actual, &context, &Default::default()).result,
            VerificationResult::Pass
        );
        let wrong = if access == ResourceAccess::Read {
            ResourceAccess::Write
        } else {
            ResourceAccess::Read
        };
        let mut context = context;
        context.resources.as_mut().unwrap()[0].access = wrong;
        assert!(
            matches!(assess_operation(&ir, &plan, &actual, &context, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "UNDECLARED_RESOURCE_ACCESS")
        );
    }
}

#[test]
fn resource_permissions_require_actual_observation_and_do_not_invent_counts() {
    let compiled = dctx0_language::compile(&[Source {
        file: "resources.dctx0".into(),
        text: "workspace W {\nentity Item { id: Int }\nsurface Api {\noperation load() -> Int {\nread Item\n}\n}\n}".into(),
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
            subjects: BTreeSet::from(["W.Api.load".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let observed = InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.load".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("1".into()),
        },
    };
    let assess = |resources| {
        assess_operation(
            &ir,
            &plan,
            &observed,
            &ObservationContext {
                resources,
                ..Default::default()
            },
            &Default::default(),
        )
        .result
    };
    assert!(
        matches!(assess(None), VerificationResult::Unknown { issue } if issue.code == "RESOURCE_OBSERVER_REQUIRED")
    );
    assert_eq!(assess(Some(vec![])), VerificationResult::Pass);
    let access = ResourceObservation {
        operation: None,
        resource: "W.Item".into(),
        access: ResourceAccess::Read,
    };
    assert_eq!(assess(Some(vec![access.clone()])), VerificationResult::Pass);
    // A zero-access observation cannot discharge a clause whose pinned
    // reference or resource contract is unavailable.
    for (remove_reference, expected) in [
        (true, "RESOURCE_REFERENCE_REQUIRED"),
        (false, "RESOURCE_CONTRACT_REQUIRED"),
    ] {
        let mut incomplete = ir.clone();
        if remove_reference {
            incomplete.references.retain(|reference| {
                reference.from != "W.Api.load" || reference.kind != ReferenceKind::Reads
            });
        } else {
            incomplete.symbols.remove("W.Item");
        }
        let assessment = assess_operation(
            &incomplete,
            &plan,
            &observed,
            &ObservationContext {
                resources: Some(vec![]),
                ..Default::default()
            },
            &Default::default(),
        );
        assert!(matches!(
            assessment.result,
            VerificationResult::Unknown { .. }
        ));
        assert!(assessment.open.iter().any(|issue| issue.code == expected));
    }
    assert!(
        matches!(assess(Some(vec![ResourceObservation { access: ResourceAccess::Write, ..access.clone() }])), VerificationResult::Fail { issue } if issue.code == "UNDECLARED_RESOURCE_ACCESS")
    );
    assert!(
        matches!(assess(Some(vec![ResourceObservation { resource: "opaque".into(), ..access.clone() }])), VerificationResult::Unknown { issue } if issue.code == "RESOURCE_LABEL_UNMODELED")
    );
    assert!(
        matches!(assess(Some(vec![ResourceObservation { operation: Some("W.Other.load".into()), ..access }])), VerificationResult::Unknown { issue } if issue.code == "RESOURCE_OPERATION_UNOBSERVED")
    );
}

#[test]
fn an_operation_without_resource_permissions_rejects_observed_access() {
    let compiled = dctx0_language::compile(&[Source {
        file: "no-resources.dctx0".into(),
        text:
            "workspace W {\nentity Item { id: Int }\nsurface Api {\noperation load() -> Int\n}\n}"
                .into(),
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
            subjects: BTreeSet::from(["W.Api.load".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .remove(0);
    let observed = InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.load".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("1".into()),
        },
    };
    let result = assess_operation(
        &ir,
        &plan,
        &observed,
        &ObservationContext {
            resources: Some(vec![ResourceObservation {
                operation: None,
                resource: "W.Item".into(),
                access: ResourceAccess::Read,
            }]),
            ..Default::default()
        },
        &Default::default(),
    )
    .result;
    assert!(
        matches!(result, VerificationResult::Fail { issue } if issue.code == "UNDECLARED_RESOURCE_ACCESS")
    );
}

#[test]
fn delegated_access_requires_a_real_declared_call_and_the_collaborators_permission() {
    let compiled = dctx0_language::compile(&[Source {
        file: "delegated-resources.dctx0".into(),
        text: "workspace W {\nentity Item { id: Int }\nport Store {\noperation load() -> Int {\nread Item\n}\n}\nsurface Api {\noperation load() -> Int {\ncall Store.load\n}\n}\n}".into(),
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
            subjects: BTreeSet::from(["W.Api.load".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .into_iter()
    .find(|p| p.operation.id == "W.Api.load")
    .unwrap();
    let observed = InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.load".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("1".into()),
        },
    };
    let collaborator = InvocationObservation {
        invocation: Invocation {
            subject: "W.Store.load".into(),
            arguments: vec![],
        },
        observation: observed.observation.clone(),
    };
    let access = ResourceObservation {
        operation: Some("W.Store.load".into()),
        resource: "W.Item".into(),
        access: ResourceAccess::Read,
    };
    let context = ObservationContext {
        calls: Some(vec![collaborator]),
        resources: Some(vec![access.clone()]),
        ..Default::default()
    };
    let mut opaque = context.clone();
    opaque.calls.as_mut().unwrap()[0].invocation.subject = "unmodeled:operation".into();
    opaque.resources.as_mut().unwrap()[0].operation = Some("unmodeled:operation".into());
    let unknown = assess_operation(&ir, &plan, &observed, &opaque, &Default::default());
    assert!(matches!(unknown.result, VerificationResult::Unknown { .. }));
    assert!(
        unknown
            .open
            .iter()
            .any(|issue| issue.code == "RESOURCE_OPERATION_UNMODELED")
    );
    assert_eq!(
        assess_operation(&ir, &plan, &observed, &context, &Default::default()).result,
        VerificationResult::Pass
    );
    let bad = ObservationContext {
        resources: Some(vec![ResourceObservation {
            access: ResourceAccess::Write,
            ..access
        }]),
        ..context.clone()
    };
    assert!(
        matches!(assess_operation(&ir, &plan, &observed, &bad, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "UNDECLARED_RESOURCE_ACCESS")
    );
    let mut undeclared = ir.clone();
    undeclared
        .references
        .retain(|r| !(r.from == "W.Api.load" && r.kind == ReferenceKind::Calls));
    assert!(
        matches!(assess_operation(&undeclared, &plan, &observed, &context, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "UNDECLARED_OPERATION_EFFECT")
    );
}

#[test]
fn an_opaque_trace_does_not_hide_a_known_undeclared_resource_delegate() {
    let compiled = dctx0_language::compile(&[Source {
        file: "opaque-delegate.dctx0".into(),
        text: "workspace W { entity Item { id:Int } port Store { operation load() -> Int { read Item } } port Other { operation load() -> Int { read Item } } surface Api { operation load() -> Int { call Store.load } } }".into(),
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
            subjects: ["W.Api.load".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    )
    .operations
    .into_iter()
    .find(|plan| plan.operation.id == "W.Api.load")
    .unwrap();
    let observed = InvocationObservation {
        invocation: Invocation {
            subject: "W.Api.load".into(),
            arguments: vec![],
        },
        observation: Observation::Returned {
            value: VerificationValue::Integer("1".into()),
        },
    };
    let context = ObservationContext {
        calls: Some(
            ["opaque:trace", "W.Other.load"]
                .into_iter()
                .map(|subject| InvocationObservation {
                    invocation: Invocation {
                        subject: subject.into(),
                        arguments: vec![],
                    },
                    observation: observed.observation.clone(),
                })
                .collect(),
        ),
        resources: Some(vec![ResourceObservation {
            operation: Some("W.Other.load".into()),
            resource: "W.Item".into(),
            access: ResourceAccess::Read,
        }]),
        ..Default::default()
    };
    assert!(
        matches!(assess_operation(&ir, &plan, &observed, &context, &Default::default()).result, VerificationResult::Fail { issue } if issue.code == "UNDECLARED_OPERATION_EFFECT")
    );
}
