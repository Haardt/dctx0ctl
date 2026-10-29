use dctx0_core::*;
use dctx0_verification::*;

fn number(value: &str) -> VerificationValue {
    VerificationValue::Integer(value.into())
}
fn record(fields: &[(&str, VerificationValue)]) -> VerificationValue {
    VerificationValue::Record(
        fields
            .iter()
            .map(|(k, v)| ((*k).into(), v.clone()))
            .collect(),
    )
}
fn fixture() -> (WorkspaceIr, OperationCaseInput, Vec<InvocationObservation>) {
    let compiled = dctx0_language::compile(&[Source {
        file: "argument-dataflow.dctx0".into(),
        text: "workspace W { type Item { id:Int } type Envelope { items:List<Item> }
        type Request { id:Int\nnested:Item }
        port Store { operation first() -> Envelope
        operation second(request:Request) -> Envelope
        operation apply(request:Request) -> Unit } }"
            .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let request = record(&[
        ("id", number("0")),
        ("nested", record(&[("id", number("0"))])),
    ]);
    let input = OperationCaseInput {
        id: "actual-setup-dataflow".into(),
        invocation: Invocation {
            subject: "W.Store.apply".into(),
            arguments: vec![request.clone()],
        },
        setup: vec![
            Invocation {
                subject: "W.Store.first".into(),
                arguments: vec![],
            },
            Invocation {
                subject: "W.Store.second".into(),
                arguments: vec![request],
            },
        ],
        argument_bindings: vec![
            OperationArgumentBinding {
                setup: 0,
                source_path: vec!["items".into(), "0".into(), "id".into()],
                target_setup: Some(1),
                parameter: "request".into(),
                target_path: vec!["nested".into(), "id".into()],
            },
            OperationArgumentBinding {
                setup: 1,
                source_path: vec!["items".into(), "0".into(), "id".into()],
                target_setup: None,
                parameter: "request".into(),
                target_path: vec!["id".into()],
            },
        ],
        mocks: vec![],
    };
    let observed = input
        .setup
        .iter()
        .zip(["7", "8"])
        .map(|(invocation, id)| InvocationObservation {
            invocation: invocation.clone(),
            observation: Observation::Returned {
                value: record(&[(
                    "items",
                    VerificationValue::List(vec![record(&[("id", number(id))])]),
                )]),
            },
        })
        .collect();
    (compiled.ir.unwrap(), input, observed)
}

#[test]
fn actual_setup_results_feed_later_setup_and_main_arguments_without_changing_templates() {
    let (ir, input, observed) = fixture();
    let original = input.clone();
    assert_eq!(
        resolve_operation_arguments(&ir, &input, &[], Some(0)).unwrap(),
        input.setup[0]
    );
    let setup = resolve_operation_arguments(&ir, &input, &observed[..1], Some(1)).unwrap();
    assert_eq!(
        setup.arguments,
        vec![record(&[
            ("id", number("0")),
            ("nested", record(&[("id", number("7"))]))
        ])]
    );
    let main = resolve_operation_arguments(&ir, &input, &observed, None).unwrap();
    assert_eq!(
        main.arguments,
        vec![record(&[
            ("id", number("8")),
            ("nested", record(&[("id", number("0"))]))
        ])]
    );
    assert_eq!(input, original);
    let mut whole = input.clone();
    whole.argument_bindings = vec![OperationArgumentBinding {
        setup: 0,
        source_path: vec!["items".into(), "0".into()],
        target_setup: None,
        parameter: "request".into(),
        target_path: vec!["nested".into()],
    }];
    let bound = resolve_operation_arguments(&ir, &whole, &observed, None).unwrap();
    assert_eq!(
        bound.arguments[0],
        record(&[
            ("id", number("0")),
            ("nested", record(&[("id", number("7"))]))
        ])
    );
}

#[test]
fn binding_validation_rejects_forward_unbounded_missing_and_overlapping_destinations() {
    let (ir, input, _) = fixture();
    let mut cases = vec![];
    let mut bad = input.clone();
    bad.argument_bindings = vec![input.argument_bindings[0].clone(); 33];
    cases.push((bad, "finite limit"));
    let mut bad = input.clone();
    bad.argument_bindings[0].setup = 1;
    cases.push((bad, "preceding"));
    for source in [true, false] {
        let mut bad = input.clone();
        if source {
            bad.argument_bindings[0].source_path = vec!["x".into(); 17];
        } else {
            bad.argument_bindings[0].target_path = vec!["x".into(); 17];
        }
        cases.push((bad, "bounded"));
    }
    let mut bad = input.clone();
    bad.argument_bindings[0].target_setup = Some(99);
    cases.push((bad, "target setup is absent"));
    let mut bad = input.clone();
    bad.invocation.subject = "W.Store.missing".into();
    cases.push((bad, "undeclared"));
    let mut bad = input.clone();
    bad.argument_bindings[0].parameter = "missing".into();
    cases.push((bad, "parameter is absent"));
    for path in [vec![], vec!["id".into()]] {
        let mut bad = input.clone();
        let mut duplicate = input.argument_bindings[1].clone();
        duplicate.target_path = path;
        bad.argument_bindings.push(duplicate);
        cases.push((bad, "overlapping"));
    }
    for (bad, expected) in cases {
        assert!(
            validate_argument_bindings(&ir, &bad)
                .unwrap_err()
                .contains(expected),
            "{expected}"
        );
    }
}

#[test]
fn argument_resolution_rejects_missing_failed_or_misaddressed_actual_results() {
    let (ir, input, observed) = fixture();
    assert!(
        resolve_operation_arguments(&ir, &input, &[], None)
            .unwrap_err()
            .contains("result is absent")
    );
    let mut wrong = observed.clone();
    wrong[1].invocation.subject = "W.Store.first".into();
    assert!(
        resolve_operation_arguments(&ir, &input, &wrong, None)
            .unwrap_err()
            .contains("another subject")
    );
    for observation in [
        Observation::Threw {
            message: "failure".into(),
        },
        Observation::Unavailable {
            reason: "missing".into(),
        },
        Observation::Returned {
            value: VerificationValue::Failure {
                declaration: "W.Failed".into(),
                payload: None,
            },
        },
    ] {
        let mut wrong = observed.clone();
        wrong[1].observation = observation;
        assert!(resolve_operation_arguments(&ir, &input, &wrong, None).is_err());
    }
    for (path, expected) in [
        (vec!["missing"], "field is absent"),
        (vec!["items", "bad"], "index is invalid"),
        (vec!["items", "00"], "canonical"),
        (vec!["items", "1"], "item is absent"),
        (vec!["items", "0", "id", "extra"], "not a record or list"),
    ] {
        let mut bad = input.clone();
        bad.argument_bindings[1].source_path = path.into_iter().map(String::from).collect();
        assert!(
            resolve_operation_arguments(&ir, &bad, &observed, None)
                .unwrap_err()
                .contains(expected),
            "{expected}"
        );
    }
    for (path, expected) in [
        (vec!["missing"], "field is absent"),
        (vec!["id", "extra"], "not a record"),
    ] {
        let mut bad = input.clone();
        bad.argument_bindings[1].target_path = path.into_iter().map(String::from).collect();
        assert!(
            resolve_operation_arguments(&ir, &bad, &observed, None)
                .unwrap_err()
                .contains(expected)
        );
    }
    let mut bad = input.clone();
    bad.invocation.arguments.clear();
    assert!(
        resolve_operation_arguments(&ir, &bad, &observed, None)
            .unwrap_err()
            .contains("target is absent")
    );
    let mut no_bindings = input.clone();
    no_bindings.argument_bindings.clear();
    assert!(
        resolve_operation_arguments(&ir, &no_bindings, &observed, Some(99))
            .unwrap_err()
            .contains("invocation is absent")
    );
    no_bindings.invocation.subject = "W.Missing".into();
    assert!(
        resolve_operation_arguments(&ir, &no_bindings, &observed, None)
            .unwrap_err()
            .contains("undeclared")
    );
    let mut root = input.clone();
    root.argument_bindings = vec![OperationArgumentBinding {
        setup: 0,
        source_path: vec![],
        target_setup: None,
        parameter: "request".into(),
        target_path: vec![],
    }];
    let supplied = record(&[
        ("id", number("9")),
        ("nested", record(&[("id", number("7"))])),
    ]);
    let mut ir = ir;
    ir.operations.get_mut("W.Store.first").unwrap().result = TypeRef {
        name: "Request".into(),
        symbol_id: Some("W.Request".into()),
        arguments: vec![],
    };
    let source = vec![InvocationObservation {
        invocation: input.setup[0].clone(),
        observation: Observation::Returned {
            value: supplied.clone(),
        },
    }];
    let result = resolve_operation_arguments(&ir, &root, &source, None).unwrap();
    assert_eq!(result.arguments, vec![supplied]);
}
