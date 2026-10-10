use super::{expressions, issue, model::*, owned};
use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value};
use std::collections::BTreeSet;

fn reference_at(ir: &WorkspaceIr, node: &SemanticNode, kind: ReferenceKind) -> Option<String> {
    ir.references
        .iter()
        .find(|r| {
            r.from == node.owner
                && r.kind == kind
                && r.span.file == node.span.file
                && r.span.start.offset >= node.span.start.offset
                && r.span.end.offset <= node.span.end.offset
        })
        .map(|r| r.target.clone())
}
pub(super) fn example(
    ir: &WorkspaceIr,
    id: &str,
    fixtures: &ProjectionFixtures,
    budgets: &VerificationBudgets,
) -> ExamplePlan {
    let mut plan = ExamplePlan {
        subject: id.into(),
        order: Vec::new(),
        variables: Vec::new(),
        bindings: fixtures.examples.get(id).cloned().unwrap_or_default(),
        mocks: Vec::new(),
        invocations: Vec::new(),
        expectations: Vec::new(),
        issues: Vec::new(),
    };
    let mut bound = BTreeSet::new();
    for node in owned(ir, id) {
        match node.kind {
            StatementKind::Given => {
                let Some(invocation) = node.expressions.last() else {
                    plan.issues.push(issue(
                        Some(&node.span),
                        "MISSING_MOCK_INVOCATION",
                        "Given has no typed invocation",
                    ));
                    continue;
                };
                if !matches!(
                    invocation.value,
                    ExpressionKind::Call {
                        target: Some(_),
                        ..
                    }
                ) {
                    plan.issues.push(issue(
                        Some(&node.span),
                        "UNSUPPORTED_GIVEN",
                        "Given is not a declared operation mock",
                    ));
                }
                let outcome = node
                    .expressions
                    .first()
                    .filter(|_| node.expressions.len() == 2)
                    .map(|expression| {
                        if node
                            .terms
                            .iter()
                            .any(|t| t.kind == TokenKind::Identifier && t.text == "fails")
                        {
                            MockOutcome::Failed {
                                expression: expression.clone(),
                            }
                        } else {
                            MockOutcome::Returned {
                                expression: expression.clone(),
                            }
                        }
                    });
                if outcome.is_none() {
                    plan.issues.push(issue(
                        Some(&node.span),
                        "MISSING_MOCK_OUTCOME",
                        "Mock requires a declared returns/fails outcome",
                    ));
                }
                match expressions::variables(&node.expressions, &bound) {
                    Ok(vars) => plan.variables.extend(vars),
                    Err(error) => plan.issues.push(*error),
                }
                plan.order
                    .push(ExampleAction::PrepareMock(plan.mocks.len()));
                plan.mocks.push(MockStep {
                    source_id: node.id.clone(),
                    invocation: invocation.clone(),
                    outcome,
                });
            }
            StatementKind::When => {
                let Some(invocation) = node.expressions.last() else {
                    plan.issues.push(issue(
                        Some(&node.span),
                        "MISSING_INVOCATION",
                        "When has no typed invocation",
                    ));
                    continue;
                };
                match expressions::variables(std::slice::from_ref(invocation), &bound) {
                    Ok(vars) => plan.variables.extend(vars),
                    Err(error) => plan.issues.push(*error),
                }
                let binding = node
                    .terms
                    .first()
                    .map_or("result", |t| t.text.as_str())
                    .to_owned();
                bound.insert(binding.clone());
                plan.order
                    .push(ExampleAction::Invoke(plan.invocations.len()));
                plan.invocations.push(InvocationStep {
                    source_id: node.id.clone(),
                    binding,
                    invocation: invocation.clone(),
                });
                if node.expressions.len() != 1 {
                    plan.issues.push(issue(
                        Some(&node.span),
                        "WHEN_OUTCOME_UNSUPPORTED",
                        "Actual invocation must not be replaced by a stubbed outcome",
                    ));
                }
            }
            StatementKind::Expect | StatementKind::Fails | StatementKind::Emits => {
                // An explicit outcome/event is not a Boolean expression assertion.
                let first = node.terms.first().map(|t| t.text.as_str());
                let expectation = if first == Some("Success") {
                    Some(Expectation::Success {
                        source_id: node.id.clone(),
                    })
                } else if first == Some("Failure")
                    || node.kind == StatementKind::Fails
                    || node
                        .terms
                        .iter()
                        .any(|t| t.kind == TokenKind::Identifier && t.text == "fails")
                {
                    Some(Expectation::Failure {
                        source_id: node.id.clone(),
                        declaration: reference_at(ir, node, ReferenceKind::FailsWith),
                        payload: node.expressions.first().cloned(),
                    })
                } else if node.kind == StatementKind::Emits
                    || node
                        .terms
                        .iter()
                        .any(|t| t.kind == TokenKind::Identifier && t.text == "emits")
                {
                    if let Some(declaration) = reference_at(ir, node, ReferenceKind::Emits) {
                        Some(Expectation::Emitted {
                            source_id: node.id.clone(),
                            declaration,
                            payload: node.expressions.first().cloned(),
                        })
                    } else {
                        plan.issues.push(issue(
                            Some(&node.span),
                            "MISSING_EVENT",
                            "Expected event has no resolved declaration",
                        ));
                        None
                    }
                } else if let Some(expression) = node.expressions.first() {
                    Some(Expectation::Assertion {
                        source_id: node.id.clone(),
                        expression: expression.clone(),
                    })
                } else {
                    plan.issues.push(issue(
                        Some(&node.span),
                        "UNSUPPORTED_EXPECTATION",
                        "Expectation has no typed execution meaning",
                    ));
                    None
                };
                if let Some(expectation) = expectation {
                    plan.order
                        .push(ExampleAction::Check(plan.expectations.len()));
                    plan.expectations.push(expectation);
                }
                match expressions::variables(&node.expressions, &bound) {
                    Ok(vars) => plan.variables.extend(vars),
                    Err(error) => plan.issues.push(*error),
                }
            }
            _ => {}
        }
    }
    plan.variables.sort_by(|a, b| a.name.cmp(&b.name));
    let mut previous = None;
    for variable in &plan.variables {
        if let Some(previous) = previous {
            let previous: &FieldIr = previous;
            if previous.name == variable.name && previous.ty != variable.ty {
                plan.issues.push(issue(
                    None,
                    "AMBIGUOUS_BINDING_TYPE",
                    "Symbolic example input has inconsistent types",
                ));
            }
        }
        previous = Some(variable);
    }
    plan.variables.dedup_by(|a, b| a.name == b.name);
    for variable in &plan.variables {
        match plan.bindings.get(&variable.name) {
            None => plan.issues.push(issue(
                None,
                "EXAMPLE_BINDINGS_REQUIRED",
                &format!("Example {id} needs concrete input {}", variable.name),
            )),
            Some(value) => match check_value(ir, &variable.ty, value, budgets) {
                ValueContract::Accepted => {}
                ValueContract::Rejected { reason } => {
                    plan.issues
                        .push(issue(None, "INVALID_EXAMPLE_BINDING", &reason))
                }
                ValueContract::Unknown { issue } => plan.issues.push(issue),
            },
        }
    }
    for name in plan.bindings.keys() {
        if !plan.variables.iter().any(|v| &v.name == name) {
            plan.issues.push(issue(
                None,
                "SURPLUS_EXAMPLE_BINDING",
                &format!("Unused input {name} is not part of example {id}"),
            ));
        }
    }
    if plan.invocations.is_empty() {
        plan.issues.push(issue(
            None,
            "MISSING_INVOCATION",
            "Example does not invoke an actual implementation",
        ));
    }
    if plan.expectations.is_empty() {
        plan.issues
            .push(issue(None, "MISSING_EXPECTATION", "Example has no checks"));
    }
    plan
}
pub(super) fn lifecycle(
    ir: &WorkspaceIr,
    id: &str,
    budgets: &VerificationBudgets,
) -> LifecyclePlan {
    let nodes = owned(ir, id);
    let declaration = nodes.iter().find(|n| n.kind == StatementKind::Lifecycle);
    let entity = declaration
        .and_then(|n| reference_at(ir, n, ReferenceKind::State))
        .unwrap_or_default();
    let field = declaration
        .and_then(|n| after(&n.terms, "using"))
        .unwrap_or_default();
    let mut plan = LifecyclePlan {
        subject: id.into(),
        entity,
        field,
        states: Vec::new(),
        transitions: Vec::new(),
        paths: Vec::new(),
        issues: Vec::new(),
    };
    if plan.entity.is_empty() || plan.field.is_empty() {
        plan.issues.push(issue(
            declaration.map(|n| &n.span),
            "LIFECYCLE_OBSERVER_REQUIRED",
            "Lifecycle needs resolved entity and discriminator field",
        ));
    }
    for node in nodes {
        match node.kind {
            StatementKind::State => plan.states.push(LifecycleState {
                name: node.terms.first().map_or("", |t| t.text.as_str()).into(),
                initial: node.terms.iter().any(|t| t.text == "initial"),
                terminal: node.terms.iter().any(|t| t.text == "terminal"),
            }),
            StatementKind::Transition => {
                let operation = reference_at(ir, node, ReferenceKind::Calls).unwrap_or_default();
                if operation.is_empty() {
                    plan.issues.push(issue(
                        Some(&node.span),
                        "MISSING_TRANSITION_OPERATION",
                        "Transition operation is unresolved",
                    ));
                }
                plan.transitions.push(LifecycleTransition {
                    source_id: node.id.clone(),
                    operation,
                    from: after(&node.terms, "from").unwrap_or_default(),
                    to: after(&node.terms, "to").unwrap_or_default(),
                    guard: node.expressions.first().cloned(),
                });
            }
            _ => {}
        }
    }
    if plan.states.iter().filter(|s| s.initial).count() != 1 || plan.transitions.is_empty() {
        plan.issues.push(issue(
            None,
            "INCOMPLETE_LIFECYCLE",
            "Lifecycle needs one initial state and executable transitions",
        ));
    }
    super::paths::plan_paths(&mut plan, budgets.recursion_depth);
    plan
}
fn after(terms: &[Token], marker: &str) -> Option<String> {
    terms
        .windows(2)
        .find(|p| p[0].text == marker)
        .map(|p| p[1].text.clone())
}
