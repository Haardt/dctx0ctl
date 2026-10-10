use dctx0_core::*;
use dctx0_verification::*;
use std::collections::{BTreeMap, BTreeSet};
fn cinema() -> WorkspaceIr {
    static IR: std::sync::OnceLock<WorkspaceIr> = std::sync::OnceLock::new();
    IR.get_or_init(load_cinema).clone()
}
fn load_cinema() -> WorkspaceIr {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema/dctx0");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
        file: file.into(),
        text: std::fs::read_to_string(root.join(file)).unwrap(),
    });
    let compiled = dctx0_language::compile(&sources);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    compiled.ir.unwrap()
}
fn scope(subject: &str) -> VerificationScope {
    VerificationScope {
        subjects: BTreeSet::from([subject.into()]),
        tasks: BTreeSet::new(),
    }
}
fn plan(
    ir: &WorkspaceIr,
    selection: VerificationScope,
    fixtures: &ProjectionFixtures,
) -> ProgramPlan {
    project_program(
        ir,
        "published-snapshot",
        3,
        selection,
        fixtures,
        Default::default(),
    )
}
#[test]
fn entire_cinema_projection_preserves_every_contract_operation_example_property_and_transition() {
    let ir = cinema();
    let original = serde_json::to_string(&ir).unwrap();
    let plan = plan(&ir, scope("HomeCinema.MovieNight"), &Default::default());
    assert!(plan.issues.is_empty(), "{:?}", plan.issues);
    assert_eq!(plan.contracts.len(), 45);
    assert_eq!(plan.operations.len(), 26);
    assert_eq!(plan.examples.len(), 12);
    assert_eq!(plan.properties.len(), 1);
    assert_eq!(plan.lifecycles.len(), 1);
    assert_eq!(plan.suites.len(), 9);
    assert_eq!(plan.policies.len(), 1);
    assert_eq!(
        plan.manual_checks
            .iter()
            .filter(|c| c.kind == StatementKind::Review)
            .count(),
        3
    );
    assert_eq!(
        plan.manual_checks
            .iter()
            .filter(|c| c.kind == StatementKind::Decision)
            .count(),
        4
    );
    assert_eq!(plan.properties[0].quantifiers.len(), 2);
    assert_eq!(plan.properties[0].assertions.len(), 2);
    assert!(
        plan.properties[0]
            .assertions
            .iter()
            .all(|a| a.issues.is_empty()),
        "{:?}",
        plan.properties[0].assertions
    );
    assert_eq!(
        plan.properties[0].assertions[1].cases.len(),
        1,
        "Explicit closed assertion must not disappear behind generation limits"
    );
    let lifecycle = &plan.lifecycles[0];
    assert!(lifecycle.issues.is_empty(), "{:?}", lifecycle.issues);
    assert_eq!(lifecycle.entity, "HomeCinema.MovieNight.ScreeningPoll");
    assert_eq!(lifecycle.field, "status");
    assert_eq!(lifecycle.states.len(), 2);
    assert_eq!(lifecycle.transitions.len(), 2);
    assert_eq!(lifecycle.transitions[0].from, "Open");
    assert_eq!(lifecycle.transitions[0].to, "Open");
    assert_eq!(lifecycle.transitions[1].to, "Closed");
    assert!(lifecycle.transitions.iter().all(|t| t.guard.is_some()));
    assert_eq!(lifecycle.paths.len(), 2);
    assert_eq!(lifecycle.paths[0].transitions, vec![0]);
    assert_eq!(lifecycle.paths[1].transitions, vec![1]);
    assert_eq!(lifecycle.paths[1].final_state, "Closed");
    for capability in [
        Capability::Values,
        Capability::PureCalls,
        Capability::Operations,
        Capability::Mocks,
        Capability::Events,
        Capability::State,
        Capability::Lifecycles,
        Capability::Inspection,
        Capability::Review,
    ] {
        assert!(plan.capabilities.contains(&capability));
    }
    assert!(
        plan.examples
            .iter()
            .all(|e| e.invocations.len() == 1 && !e.mocks.is_empty() && !e.expectations.is_empty())
    );
    for example in &plan.examples {
        let invoke = example
            .order
            .iter()
            .position(|a| matches!(a, ExampleAction::Invoke(0)))
            .unwrap();
        assert!(
            example.order[..invoke]
                .iter()
                .all(|a| matches!(a, ExampleAction::PrepareMock(_)))
        );
        assert!(
            example.order[invoke + 1..]
                .iter()
                .all(|a| matches!(a, ExampleAction::Check(_)))
        );
    }
    assert!(
        plan.examples.iter().all(|e| e
            .issues
            .iter()
            .all(|i| i.code == "EXAMPLE_BINDINGS_REQUIRED")),
        "{:?}",
        plan.examples
            .iter()
            .flat_map(|e| &e.issues)
            .collect::<Vec<_>>()
    );
    let cast = plan
        .operations
        .iter()
        .find(|o| o.operation.id.ends_with("ScreeningPlanningApi.castVote"))
        .unwrap();
    assert!(
        cast.clauses
            .iter()
            .any(|n| n.kind == StatementKind::Ensures)
    );
    assert!(cast.clauses.iter().any(|n| n.kind == StatementKind::Call));
    assert!(cast.clauses.iter().any(|n| n.kind == StatementKind::Emit));
    assert_eq!(serde_json::to_string(&ir).unwrap(), original);
    assert_eq!(
        serde_json::from_str::<ProgramPlan>(&serde_json::to_string(&plan).unwrap()).unwrap(),
        plan
    );
    assert_eq!(
        plan,
        project_program(
            &ir,
            "published-snapshot",
            3,
            scope("HomeCinema.MovieNight"),
            &Default::default(),
            Default::default()
        )
    );
}
#[test]
fn reviewed_concrete_example_inputs_are_typed_and_never_inferred_from_observed_outputs() {
    let ir = cinema();
    let projected = plan(&ir, scope("HomeCinema.MovieNight"), &Default::default());
    let mut fixtures = ProjectionFixtures::default();
    for example in &projected.examples {
        let mut values = BTreeMap::new();
        for variable in &example.variables {
            let corpus = generate_values(&ir, &variable.ty, &Default::default());
            assert!(corpus.issues.is_empty(), "{:?}", corpus.issues);
            values.insert(variable.name.clone(), corpus.values[0].clone());
        }
        fixtures.examples.insert(example.subject.clone(), values);
    }
    let complete = plan(&ir, scope("HomeCinema.MovieNight"), &fixtures);
    assert!(
        complete.examples.iter().all(|e| e.issues.is_empty()),
        "{:?}",
        complete
            .examples
            .iter()
            .flat_map(|e| &e.issues)
            .collect::<Vec<_>>()
    );
    // Input bindings merely make an example runnable; they do not prove it passes.
    let failure = complete
        .examples
        .iter()
        .find(|e| e.subject.contains("unknown film details"))
        .unwrap();
    assert!(failure.expectations.iter().any(|e|matches!(e,Expectation::Failure {declaration:Some(id),..} if id.ends_with("FilmNotFound"))));
    let event = complete
        .examples
        .iter()
        .find(|e| e.subject.contains("save a film"))
        .unwrap();
    assert!(event.expectations.iter().any(|e|matches!(e,Expectation::Emitted {declaration,payload:Some(_),..} if declaration.ends_with("FilmSaved"))));
    let chosen = projected
        .examples
        .iter()
        .find(|e| !e.variables.is_empty())
        .unwrap();
    let variable = &chosen.variables[0];
    fixtures
        .examples
        .get_mut(&chosen.subject)
        .unwrap()
        .insert(variable.name.clone(), VerificationValue::Unit);
    fixtures
        .examples
        .get_mut(&chosen.subject)
        .unwrap()
        .insert("not_a_spec_input".into(), VerificationValue::Unit);
    let invalid = plan(&ir, scope(&chosen.subject), &fixtures);
    assert!(
        invalid.examples[0]
            .issues
            .iter()
            .any(|i| i.code == "INVALID_EXAMPLE_BINDING")
    );
    assert!(
        invalid.examples[0]
            .issues
            .iter()
            .any(|i| i.code == "SURPLUS_EXAMPLE_BINDING")
    );
}
#[test]
fn narrow_selection_follows_dependencies_and_governing_facts_without_claiming_whole_application() {
    let ir = cinema();
    let selected = "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner";
    let plan = plan(&ir, scope(selected), &Default::default());
    assert!(plan.closure.contains("HomeCinema.MovieNight.FilmId"));
    assert!(
        plan.closure
            .contains("HomeCinema.MovieNight.ScreeningPlanning.CandidateOrderTieBreak")
    );
    assert!(
        plan.not_selected
            .iter()
            .any(|id| id.ends_with("FilmCatalogApi.searchFilms"))
    );
    assert_eq!(plan.properties.len(), 1);
    assert!(!plan.manual_checks.is_empty());
    let empty = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: BTreeSet::new(),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    );
    assert!(empty.issues.iter().any(|i| i.code == "INVALID_SCOPE"));
    let unknown = project_program(
        &ir,
        "snapshot",
        1,
        scope("not-a-subject"),
        &Default::default(),
        Default::default(),
    );
    assert!(unknown.issues.iter().any(|i| i.code == "UNKNOWN_SUBJECT"));
}

fn minimal(kind: StatementKind) -> WorkspaceIr {
    let mut ir = cinema();
    let mut symbol = ir.symbols.values().next().unwrap().clone();
    symbol.id = "W.Subject".into();
    symbol.name = "Subject".into();
    symbol.parent = None;
    symbol.kind = kind;
    let mut node = ir.nodes[0].clone();
    node.id = "W.Subject:0".into();
    node.owner = symbol.id.clone();
    node.kind = kind;
    node.terms.clear();
    node.expressions.clear();
    ir.symbols = BTreeMap::from([(symbol.id.clone(), symbol)]);
    ir.nodes = vec![node];
    ir.data.clear();
    ir.operations.clear();
    ir.references.clear();
    ir
}
fn variable(ir: &WorkspaceIr, name: &str, ty: &str) -> Expression {
    Expression {
        span: ir.nodes[0].span.clone(),
        ty: TypeRef::builtin(ty),
        value: ExpressionKind::Variable { name: name.into() },
    }
}
fn node(
    ir: &WorkspaceIr,
    kind: StatementKind,
    terms: &[&str],
    expressions: Vec<Expression>,
) -> SemanticNode {
    let mut node = ir.nodes[0].clone();
    node.kind = kind;
    node.expressions = expressions;
    node.terms = terms
        .iter()
        .map(|s| Token {
            kind: TokenKind::Identifier,
            text: (*s).into(),
            span: node.span.clone(),
        })
        .collect();
    node
}
fn codes(plan: &ProgramPlan) -> BTreeSet<&str> {
    plan.issues
        .iter()
        .chain(plan.examples.iter().flat_map(|e| &e.issues))
        .chain(
            plan.properties
                .iter()
                .flat_map(|p| p.assertions.iter().flat_map(|a| &a.issues)),
        )
        .chain(plan.lifecycles.iter().flat_map(|l| &l.issues))
        .map(|i| i.code.as_str())
        .collect()
}
#[test]
fn incomplete_examples_report_missing_typed_calls_mock_outcomes_expectations_and_observers() {
    let mut ir = minimal(StatementKind::Example);
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(codes(&p).is_superset(&BTreeSet::from([
        "MISSING_INVOCATION",
        "MISSING_EXPECTATION"
    ])));
    ir.nodes.extend([
        node(&ir, StatementKind::Given, &[], vec![]),
        node(&ir, StatementKind::When, &[], vec![]),
        node(&ir, StatementKind::Expect, &[], vec![]),
        node(&ir, StatementKind::Emits, &["event"], vec![]),
        node(
            &ir,
            StatementKind::Given,
            &[],
            vec![variable(&ir, "value", "Int")],
        ),
        node(
            &ir,
            StatementKind::When,
            &["actual"],
            vec![variable(&ir, "value", "Int"), variable(&ir, "value", "Int")],
        ),
        node(&ir, StatementKind::Expect, &["Success"], vec![]),
        node(&ir, StatementKind::Expect, &["Failure"], vec![]),
    ]);
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    for code in [
        "MISSING_MOCK_INVOCATION",
        "MISSING_INVOCATION",
        "UNSUPPORTED_EXPECTATION",
        "MISSING_EVENT",
        "UNSUPPORTED_GIVEN",
        "MISSING_MOCK_OUTCOME",
        "WHEN_OUTCOME_UNSUPPORTED",
        "EXAMPLE_BINDINGS_REQUIRED",
    ] {
        assert!(codes(&p).contains(code), "{code} {:?}", codes(&p));
    }
    assert_eq!(
        p.examples[0].order,
        vec![
            ExampleAction::PrepareMock(0),
            ExampleAction::Invoke(0),
            ExampleAction::Check(0),
            ExampleAction::Check(1)
        ]
    );
}
#[test]
fn property_quantifiers_unbound_values_empty_assertions_and_corrupt_types_stay_unknown() {
    let mut ir = minimal(StatementKind::Invariant);
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(codes(&p).contains("MISSING_ASSERTION"));
    ir.nodes.extend([
        node(&ir, StatementKind::Forall, &["unused"], vec![]),
        node(
            &ir,
            StatementKind::Assertion,
            &[],
            vec![variable(&ir, "free", "Int")],
        ),
    ]);
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(codes(&p).is_superset(&BTreeSet::from([
        "QUANTIFIER_TYPE_UNAVAILABLE",
        "UNBOUND_PROPERTY_VALUE"
    ])));
    let conflict = Expression {
        span: ir.nodes[0].span.clone(),
        ty: TypeRef::builtin("Boolean"),
        value: ExpressionKind::Binary {
            operator: "==".into(),
            left: Box::new(variable(&ir, "free", "Int")),
            right: Box::new(variable(&ir, "free", "String")),
        },
    };
    ir.nodes.last_mut().unwrap().expressions = vec![conflict];
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(codes(&p).contains("AMBIGUOUS_BINDING_TYPE"));
}
#[test]
fn unresolved_dependencies_and_opaque_trace_labels_never_become_execution_evidence() {
    let mut ir = minimal(StatementKind::Operation);
    ir.references.push(Reference {
        from: "W.Subject".into(),
        target: "missing".into(),
        kind: ReferenceKind::UsesType,
        spelling: "missing".into(),
        span: ir.nodes[0].span.clone(),
    });
    ir.nodes[0].expressions = vec![Expression {
        span: ir.nodes[0].span.clone(),
        ty: TypeRef::builtin("Boolean"),
        value: ExpressionKind::Trace {
            trace_kind: "calls".into(),
            target: "opaque".into(),
            modeled_target: None,
            arguments: vec![],
            count: Box::new(variable(&ir, "count", "Int")),
        },
    }];
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(codes(&p).is_superset(&BTreeSet::from([
        "MISSING_CLOSURE_SYMBOL",
        "UNMODELED_TRACE"
    ])));
    assert!(!p.closure.contains("opaque"));
    assert!(p.capabilities.contains(&Capability::Mocks));
    let mut event = ir.symbols["W.Subject"].clone();
    event.id = "W.Event".into();
    event.kind = StatementKind::Event;
    ir.symbols.insert(event.id.clone(), event);
    ir.references.push(Reference {
        from: "W.Subject".into(),
        target: "W.Event".into(),
        kind: ReferenceKind::Emits,
        spelling: "Event".into(),
        span: ir.nodes[0].span.clone(),
    });
    if let ExpressionKind::Trace {
        trace_kind,
        modeled_target,
        ..
    } = &mut ir.nodes[0].expressions[0].value
    {
        *trace_kind = "emits".into();
        *modeled_target = Some("W.Event".into());
    }
    let events = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(events.capabilities.contains(&Capability::Events));
    assert!(events.closure.contains("W.Event"));
    assert!(!codes(&events).contains("UNMODELED_TRACE"));
    let p = project_program(
        &ir,
        "",
        0,
        scope("W.Subject"),
        &Default::default(),
        Default::default(),
    );
    assert!(codes(&p).contains("INVALID_SCOPE"));
}
#[test]
fn malformed_lifecycle_requires_a_real_observer_one_initial_state_and_resolved_operations() {
    let mut ir = minimal(StatementKind::Lifecycle);
    ir.nodes.push(node(
        &ir,
        StatementKind::Transition,
        &["advance", "from", "Open", "to", "Closed"],
        vec![],
    ));
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(codes(&p).is_superset(&BTreeSet::from([
        "LIFECYCLE_OBSERVER_REQUIRED",
        "MISSING_TRANSITION_OPERATION",
        "INCOMPLETE_LIFECYCLE"
    ])));
}
#[test]
fn mock_setup_after_an_invocation_uses_the_delivered_binding_and_keeps_source_order() {
    let mut ir = minimal(StatementKind::Example);
    let input = variable(&ir, "input", "Int");
    let result = variable(&ir, "first", "Int");
    let invoke = |argument| Expression {
        span: ir.nodes[0].span.clone(),
        ty: TypeRef::builtin("Int"),
        value: ExpressionKind::Call {
            name: "real".into(),
            target: Some("real".into()),
            arguments: vec![argument],
        },
    };
    ir.nodes.extend([
        node(&ir, StatementKind::When, &["first"], vec![invoke(input)]),
        node(
            &ir,
            StatementKind::Given,
            &["fails"],
            vec![result.clone(), invoke(result)],
        ),
        node(&ir, StatementKind::Expect, &["Success"], vec![]),
    ]);
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert_eq!(
        p.examples[0]
            .variables
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        vec!["input"]
    );
    assert_eq!(
        p.examples[0].order,
        vec![
            ExampleAction::Invoke(0),
            ExampleAction::PrepareMock(0),
            ExampleAction::Check(0)
        ]
    );
    assert!(matches!(
        p.examples[0].mocks[0].outcome,
        Some(MockOutcome::Failed { .. })
    ));
}

#[test]
fn incompatible_symbolic_types_are_reported_in_each_example_phase_and_across_phases() {
    let base = minimal(StatementKind::Example);
    let conflict = Expression {
        span: base.nodes[0].span.clone(),
        ty: TypeRef::builtin("Boolean"),
        value: ExpressionKind::Binary {
            operator: "==".into(),
            left: Box::new(variable(&base, "input", "Int")),
            right: Box::new(variable(&base, "input", "String")),
        },
    };
    for kind in [
        StatementKind::Given,
        StatementKind::When,
        StatementKind::Expect,
    ] {
        let mut ir = base.clone();
        ir.nodes.push(node(&ir, kind, &[], vec![conflict.clone()]));
        let p = plan(&ir, scope("W.Subject"), &Default::default());
        assert!(codes(&p).contains("AMBIGUOUS_BINDING_TYPE"));
    }
    let mut ir = base.clone();
    ir.nodes.extend([
        node(
            &ir,
            StatementKind::When,
            &["actual"],
            vec![variable(&ir, "input", "Int")],
        ),
        node(
            &ir,
            StatementKind::Expect,
            &[],
            vec![variable(&ir, "input", "String")],
        ),
    ]);
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(codes(&p).contains("AMBIGUOUS_BINDING_TYPE"));
    ir.nodes.last_mut().unwrap().expressions.clear();
    ir.nodes[1].expressions[0].ty = TypeRef::builtin("Duration");
    let fixtures = ProjectionFixtures {
        operations: Default::default(),
        examples: BTreeMap::from([(
            "W.Subject".into(),
            BTreeMap::from([("input".into(), VerificationValue::String("1h".into()))]),
        )]),
    };
    let p = plan(&ir, scope("W.Subject"), &fixtures);
    assert!(!p.examples[0].issues.is_empty());
    assert!(!codes(&p).contains("EXAMPLE_BINDINGS_REQUIRED"));
}

#[test]
fn missing_data_symbol_and_state_expression_capabilities_are_explicit() {
    let mut ir = minimal(StatementKind::Operation);
    ir.data.insert(
        "missing".into(),
        DataIr {
            id: "missing".into(),
            stable_id: None,
            alias: Some(TypeRef::builtin("Int")),
            fields: vec![],
            variants: vec![],
            keys: vec![],
        },
    );
    ir.references.push(Reference {
        from: "W.Subject".into(),
        target: "missing".into(),
        kind: ReferenceKind::UsesType,
        spelling: "missing".into(),
        span: ir.nodes[0].span.clone(),
    });
    ir.nodes[0].expressions = vec![Expression {
        span: ir.nodes[0].span.clone(),
        ty: TypeRef::builtin("Int"),
        value: ExpressionKind::State {
            phase: StatePhase::Before,
            entity: "missing".into(),
            keys: vec![],
        },
    }];
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert_eq!(p.contracts[0].ty.name, "missing");
    assert!(codes(&p).contains("MISSING_CLOSURE_SYMBOL"));
    assert!(p.capabilities.contains(&Capability::State));
}

#[test]
fn quoted_effect_words_are_values_and_cannot_invent_event_or_failure_requirements() {
    let mut ir = minimal(StatementKind::Example);
    let mut assertion = node(
        &ir,
        StatementKind::Expect,
        &["emits", "fails"],
        vec![Expression {
            span: ir.nodes[0].span.clone(),
            ty: TypeRef::builtin("Boolean"),
            value: ExpressionKind::Literal {
                text: "true".into(),
                literal_kind: "Boolean".into(),
            },
        }],
    );
    for term in &mut assertion.terms {
        term.kind = TokenKind::String;
    }
    ir.nodes.push(assertion);
    let p = plan(&ir, scope("W.Subject"), &Default::default());
    assert!(matches!(
        p.examples[0].expectations[0],
        Expectation::Assertion { .. }
    ));
    assert!(!p.capabilities.contains(&Capability::Events));
    assert!(!codes(&p).contains("MISSING_EVENT"));
}

#[test]
fn central_case_manifest_preserves_all_cinema_obligations_and_requirement_links() {
    let ir = cinema();
    let projected = plan(&ir, scope("HomeCinema.MovieNight"), &Default::default());
    let manifest = case_manifest(&projected);
    assert_eq!(manifest, case_manifest(&projected));
    assert_eq!(
        manifest.required_cases,
        manifest.case_requirements.keys().cloned().collect()
    );
    assert!(
        manifest
            .open_obligations
            .iter()
            .any(|i| i.code == "OPERATION_CLOSURE_INCOMPLETE")
    );
    assert!(
        manifest
            .open_obligations
            .iter()
            .any(|i| i.code == "MANUAL_REQUIREMENT")
    );
    assert!(
        manifest
            .open_obligations
            .iter()
            .any(|i| i.code == "SUITE_INSPECTION_REQUIRED")
    );
    for example in &projected.examples {
        assert!(manifest.case_requirements[&example.subject].contains(&example.subject));
    }
    for lifecycle in &projected.lifecycles {
        for path in &lifecycle.paths {
            assert!(manifest.case_requirements[&path.id].contains(&lifecycle.subject));
        }
    }
    for property in &projected.properties {
        for assertion in &property.assertions {
            for case in &assertion.cases {
                assert!(manifest.case_requirements[&case.id].contains(&assertion.source_id));
                assert!(manifest.case_requirements[&case.id].contains(&property.subject));
            }
        }
    }
}

#[test]
fn central_case_manifest_never_drops_empty_or_duplicate_mandatory_cases() {
    let ir = dctx0_language::compile(&[Source { file: "w.dctx0".into(), text: "workspace W { value A = Int { range 0..2 } surface Api { operation next(a:A) -> Int { pure\n ensures result == a + 1\n } } }".into() }]).ir.unwrap();
    let mut projected = plan(&ir, scope("W.Api.next"), &Default::default());
    let operation = &projected.operations[0];
    let manifest = case_manifest(&projected);
    for i in 0..operation.inputs.bindings.len() {
        let linked = &manifest.case_requirements[&format!("W.Api.next:operation:{i}")];
        assert!(linked.contains("W.Api.next"));
        assert!(linked.contains(&operation.clauses[0].id));
    }
    projected.operations[0].inputs.bindings.clear();
    projected.contracts[0].corpus.values.clear();
    let empty = case_manifest(&projected);
    assert!(
        empty
            .open_obligations
            .iter()
            .any(|i| i.code == "EMPTY_OPERATION_CORPUS")
    );
    assert!(
        empty
            .open_obligations
            .iter()
            .any(|i| i.code == "EMPTY_CONTRACT_CORPUS")
    );
    projected.contracts[0].rejection_generator_version = "different-version".into();
    let duplicate = projected.contracts[0].clone();
    projected.contracts.push(duplicate);
    let invalid = case_manifest(&projected);
    assert!(
        invalid
            .open_obligations
            .iter()
            .any(|i| i.code == "REJECTION_GENERATOR_VERSION")
    );
    assert!(
        invalid
            .open_obligations
            .iter()
            .any(|i| i.code == "DUPLICATE_CASE_ID")
    );
}

#[test]
fn value_case_requirements_bind_modeled_refinements_without_asserting_free_text() {
    let sources = [Source { file: "w.dctx0".into(), text: "workspace W {\n intent \"Explain the range\"\n value A = Int {\n range 0..2\n where value >= 0\n } }".into() }];
    let compiled = dctx0_language::compile(&sources);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let ir = compiled.ir.unwrap();
    let projected = plan(&ir, scope("W.A"), &Default::default());
    let contract = &projected.contracts[0];
    let expected = ir
        .nodes
        .iter()
        .filter(|n| {
            n.owner == "W.A" && matches!(n.kind, StatementKind::Range | StatementKind::Where)
        })
        .map(|n| n.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        contract
            .refinement_requirements
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(expected.len(), 2);
    let intent = ir
        .nodes
        .iter()
        .find(|n| n.kind == StatementKind::Intent)
        .unwrap();
    let manifest = case_manifest(&projected);
    for requirements in manifest.case_requirements.values() {
        assert!(expected.is_subset(requirements));
        assert!(requirements.contains("W.A"));
        assert!(!requirements.contains(&intent.id));
    }
    let mut historical = serde_json::to_value(&projected).unwrap();
    historical["contracts"][0]
        .as_object_mut()
        .unwrap()
        .remove("refinement_requirements");
    let historical: ProgramPlan = serde_json::from_value(historical).unwrap();
    assert!(historical.contracts[0].refinement_requirements.is_empty());
    for requirements in case_manifest(&historical).case_requirements.values() {
        assert_eq!(requirements, &["W.A".into()].into());
    }
}
