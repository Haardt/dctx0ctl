//! Projection from a published validated IR. No compiler publication, files or execution.
mod expressions;
mod model;
mod paths;
mod scenarios;
use crate::{VerificationCase, VerificationScope, generate_inputs, generate_values};
use dctx0_core::*;
pub use model::*;
use std::collections::BTreeSet;

pub fn project_program(
    ir: &WorkspaceIr,
    snapshot: &str,
    occurrence: u64,
    scope: VerificationScope,
    fixtures: &ProjectionFixtures,
    budgets: VerificationBudgets,
) -> ProgramPlan {
    let mut plan = ProgramPlan {
        schema_version: 1,
        snapshot: snapshot.into(),
        check_occurrence: occurrence,
        scope,
        closure: BTreeSet::new(),
        not_selected: Vec::new(),
        budgets,
        generator_version: crate::GENERATOR_VERSION.into(),
        contracts: Vec::new(),
        operations: Vec::new(),
        properties: Vec::new(),
        examples: Vec::new(),
        lifecycles: Vec::new(),
        manual_checks: Vec::new(),
        suites: Vec::new(),
        policies: Vec::new(),
        capabilities: Vec::new(),
        issues: Vec::new(),
    };
    if snapshot.is_empty() || occurrence == 0 || plan.scope.subjects.is_empty() {
        plan.issues.push(issue(
            None,
            "INVALID_SCOPE",
            "A pinned snapshot/check occurrence and explicit nonempty selection are required",
        ));
        return plan;
    }
    for selected in &plan.scope.subjects {
        if !ir.symbols.contains_key(selected) {
            plan.issues.push(issue(
                None,
                "UNKNOWN_SUBJECT",
                &format!("Unknown subject {selected}"),
            ));
        }
        for symbol in ir
            .symbols
            .values()
            .filter(|s| s.id == *selected || s.id.starts_with(&format!("{selected}.")))
        {
            plan.closure.insert(symbol.id.clone());
        }
    }
    // Follow actual resolved requirements; opaque trace spellings never add edges.
    loop {
        let mut additions = BTreeSet::new();
        for reference in &ir.references {
            if plan.closure.contains(&reference.from)
                && matches!(
                    reference.kind,
                    ReferenceKind::UsesType
                        | ReferenceKind::Accepts
                        | ReferenceKind::Returns
                        | ReferenceKind::FailsWith
                        | ReferenceKind::Calls
                        | ReferenceKind::Reads
                        | ReferenceKind::Writes
                        | ReferenceKind::Emits
                        | ReferenceKind::State
                        | ReferenceKind::Observes
                )
            {
                additions.insert(reference.target.clone());
            }
            if reference.kind == ReferenceKind::Calls
                && plan.closure.contains(&reference.target)
                && ir.symbols.get(&reference.from).is_some_and(|s| {
                    matches!(
                        s.kind,
                        StatementKind::Invariant
                            | StatementKind::Example
                            | StatementKind::Lifecycle
                    )
                })
            {
                additions.insert(reference.from.clone());
            }
        }
        for subject in &plan.closure {
            let reviewed = fixtures
                .operations
                .get(subject)
                .into_iter()
                .flatten()
                .flat_map(|case| &case.setup);
            let generated = fixtures
                .generated_preparations
                .get(subject)
                .into_iter()
                .flatten()
                .flat_map(|case| &case.setup);
            for setup in reviewed.chain(generated) {
                if ir.operations.contains_key(&setup.subject) {
                    additions.insert(setup.subject.clone());
                }
            }
        }
        let old = plan.closure.len();
        plan.closure.extend(additions);
        if old == plan.closure.len() {
            break;
        }
    }
    // Ancestor facts govern selected work without scheduling unrelated siblings.
    let mut ancestors = Vec::new();
    for id in &plan.closure {
        let mut parent = ir.symbols.get(id).and_then(|s| s.parent.as_ref());
        while let Some(id) = parent {
            ancestors.push(id.clone());
            parent = ir.symbols.get(id).and_then(|s| s.parent.as_ref());
        }
    }
    plan.closure.extend(ancestors.clone());
    for symbol in ir.symbols.values().filter(|s| {
        s.kind == StatementKind::Decision
            && s.parent.as_ref().is_some_and(|p| ancestors.contains(p))
    }) {
        plan.closure.insert(symbol.id.clone());
    }
    plan.not_selected = ir
        .symbols
        .keys()
        .filter(|id| !plan.closure.contains(*id))
        .cloned()
        .collect();
    for id in plan.closure.clone() {
        if let Some(data) = ir.data.get(&id) {
            let ty = TypeRef {
                name: ir
                    .symbols
                    .get(&id)
                    .map_or_else(|| id.clone(), |s| s.name.clone()),
                symbol_id: Some(id.clone()),
                arguments: Vec::new(),
            };
            plan.capabilities.push(Capability::Values);
            if !data.keys.is_empty() {
                plan.capabilities.push(Capability::State);
            }
            plan.contracts.push(ContractPlan {
                subject: data.id.clone(),
                identity_requirements: ir
                    .nodes
                    .iter()
                    .filter(|node| node.owner == data.id && node.kind == StatementKind::Key)
                    .map(|node| node.id.clone())
                    .collect(),
                structural_requirements: ir
                    .nodes
                    .iter()
                    .filter(|node| node.owner == data.id && node.kind == StatementKind::Field)
                    .map(|node| node.id.clone())
                    .collect(),
                refinement_requirements: ir
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.owner == data.id
                            && matches!(
                                node.kind,
                                StatementKind::Range
                                    | StatementKind::Length
                                    | StatementKind::Format
                                    | StatementKind::Where
                            )
                    })
                    .map(|node| node.id.clone())
                    .collect(),
                corpus: generate_values(ir, &ty, &plan.budgets),
                rejection_corpus: crate::generate_rejections(ir, &ty, &plan.budgets),
                rejection_generator_version: crate::REJECTION_GENERATOR_VERSION.into(),
                ty,
            });
        }
        if let Some(operation) = ir.operations.get(&id) {
            let clauses: Vec<_> = owned(ir, &id)
                .into_iter()
                .filter(|n| {
                    matches!(
                        n.kind,
                        StatementKind::Requires
                            | StatementKind::Ensures
                            | StatementKind::State
                            | StatementKind::Read
                            | StatementKind::Write
                            | StatementKind::Call
                            | StatementKind::Emit
                            | StatementKind::Observe
                    )
                })
                .cloned()
                .collect();
            plan.capabilities.push(if operation.pure {
                Capability::PureCalls
            } else {
                Capability::Operations
            });
            let (inputs, precondition_selection) = crate::select_precondition_inputs(
                ir,
                &clauses,
                generate_inputs(ir, &operation.parameters, &plan.budgets),
                &plan.budgets,
            );
            plan.operations.push(OperationPlan {
                operation: operation.clone(),
                inputs,
                precondition_selection,
                clauses,
                cases: fixtures.operations.get(&id).cloned().unwrap_or_default(),
                generated_preparations: fixtures
                    .generated_preparations
                    .get(&id)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        let Some(symbol) = ir.symbols.get(&id) else {
            plan.issues.push(issue(
                None,
                "MISSING_CLOSURE_SYMBOL",
                "Resolved dependency has no declaration",
            ));
            continue;
        };
        match symbol.kind {
            StatementKind::Invariant => plan.properties.push(property(ir, &id, &plan.budgets)),
            StatementKind::Example => {
                plan.examples
                    .push(scenarios::example(ir, &id, fixtures, &plan.budgets))
            }
            StatementKind::Lifecycle => {
                plan.capabilities.push(Capability::Lifecycles);
                plan.capabilities.push(Capability::State);
                plan.lifecycles
                    .push(scenarios::lifecycle(ir, &id, &plan.budgets));
            }
            StatementKind::Decision => plan.manual_checks.push(ManualCheck {
                subject: id.clone(),
                kind: symbol.kind,
                nodes: owned(ir, &id).into_iter().cloned().collect(),
            }),
            StatementKind::Review => {
                plan.capabilities.push(Capability::Review);
                plan.manual_checks.push(ManualCheck {
                    subject: id.clone(),
                    kind: symbol.kind,
                    nodes: owned(ir, &id).into_iter().cloned().collect(),
                });
            }
            StatementKind::Policy => plan.policies.push(ManualCheck {
                subject: id.clone(),
                kind: symbol.kind,
                nodes: owned(ir, &id).into_iter().cloned().collect(),
            }),
            StatementKind::Proof | StatementKind::Test => {
                plan.capabilities.push(Capability::Inspection);
                plan.suites.push(SuitePlan {
                    declaration: symbol.clone(),
                    nodes: owned(ir, &id).into_iter().cloned().collect(),
                });
            }
            _ => {}
        }
        for node in owned(ir, &id) {
            match node.kind {
                StatementKind::Read | StatementKind::Write | StatementKind::Observe => {
                    plan.capabilities.push(Capability::ResourceObservation);
                }
                StatementKind::Call => plan.capabilities.push(Capability::CallObservation),
                StatementKind::State => plan.capabilities.push(Capability::State),
                _ => {}
            }
            if matches!(node.kind, StatementKind::Constraint | StatementKind::Intent) {
                plan.manual_checks.push(ManualCheck {
                    subject: node.id.clone(),
                    kind: node.kind,
                    nodes: vec![node.clone()],
                });
            }
            for e in &node.expressions {
                expressions::walk(e, &BTreeSet::new(), &mut |e, _| match &e.value {
                    ExpressionKind::State { .. } => plan.capabilities.push(Capability::State),
                    ExpressionKind::Trace {
                        modeled_target,
                        trace_kind,
                        ..
                    } => {
                        plan.capabilities.push(if trace_kind == "emits" {
                            Capability::Events
                        } else {
                            Capability::Mocks
                        });
                        if modeled_target.is_none() {
                            plan.issues.push(issue(
                                Some(&e.span),
                                "UNMODELED_TRACE",
                                "Opaque trace labels cannot become resolved execution evidence",
                            ));
                        }
                    }
                    ExpressionKind::Call {
                        name,
                        target: Some(target),
                        ..
                    } if name.contains(':') && ir.data.contains_key(target) => {
                        plan.capabilities.push(Capability::State)
                    }
                    _ => {}
                });
            }
            if node.kind == StatementKind::Given {
                plan.capabilities.push(Capability::Mocks);
            }
            if node.kind == StatementKind::Emit
                || node.kind == StatementKind::Emits
                || node
                    .terms
                    .iter()
                    .any(|t| t.kind == TokenKind::Identifier && t.text == "emits")
            {
                plan.capabilities.push(Capability::Events);
            }
        }
    }
    let mut case_ids = BTreeSet::new();
    for operation in &plan.operations {
        let span = ir
            .symbols
            .get(&operation.operation.id)
            .map(|symbol| &symbol.span);
        let mut seen = Vec::new();
        for preparation in &operation.generated_preparations {
            if operation.operation.pure
                || !operation.inputs.bindings.contains(&preparation.bindings)
                || seen.contains(&&preparation.bindings)
            {
                plan.issues.push(issue(
                    span,
                    "INVALID_GENERATED_PREPARATION",
                    "Preparations require unique exact generated bindings for a nonpure operation",
                ));
            }
            seen.push(&preparation.bindings);
        }
        let prepared_cases = operation
            .generated_preparations
            .iter()
            .enumerate()
            .map(|(index, preparation)| crate::OperationCaseInput {
                id: format!("{}:preparation:{index}", operation.operation.id),
                invocation: Invocation {
                    subject: operation.operation.id.clone(),
                    arguments: operation
                        .operation
                        .parameters
                        .iter()
                        .filter_map(|parameter| preparation.bindings.get(&parameter.name).cloned())
                        .collect(),
                },
                setup: preparation.setup.clone(),
                mocks: vec![],
            })
            .collect::<Vec<_>>();
        for case in operation.cases.iter().chain(&prepared_cases) {
            for setup in &case.setup {
                let span = ir
                    .symbols
                    .get(&operation.operation.id)
                    .map(|symbol| &symbol.span);
                let Some(preparation) = ir.operations.get(&setup.subject) else {
                    plan.issues.push(issue(
                        span,
                        "INVALID_OPERATION_SETUP",
                        "Reviewed setup operation is undeclared",
                    ));
                    continue;
                };
                if preparation.parameters.len() != setup.arguments.len() {
                    plan.issues.push(issue(
                        span,
                        "INVALID_OPERATION_SETUP",
                        "Reviewed setup arguments do not match the declared signature",
                    ));
                    continue;
                }
                for (parameter, value) in preparation.parameters.iter().zip(&setup.arguments) {
                    match dctx0_interpreter::check_value(ir, &parameter.ty, value, &plan.budgets) {
                        dctx0_interpreter::ValueContract::Accepted => {}
                        dctx0_interpreter::ValueContract::Rejected { reason } => plan
                            .issues
                            .push(issue(span, "INVALID_OPERATION_SETUP", &reason)),
                        dctx0_interpreter::ValueContract::Unknown { issue } => {
                            plan.issues.push(issue)
                        }
                    }
                }
            }
            let mocks = crate::OperationMockSession::new(ir, &case.mocks, &plan.budgets);
            if !mocks.issues.is_empty() {
                plan.issues.push(issue(
                    ir.symbols
                        .get(&operation.operation.id)
                        .map(|symbol| &symbol.span),
                    "INVALID_OPERATION_CASE",
                    "Reviewed operation mock plans must have valid unambiguous typed replies",
                ));
                plan.issues.extend(mocks.issues);
            }
            for prepared in &case.mocks {
                if !ir.references.iter().any(|reference| {
                    reference.from == operation.operation.id
                        && reference.target == prepared.invocation.subject
                        && reference.kind == ReferenceKind::Calls
                }) {
                    plan.issues.push(issue(
                        ir.symbols
                            .get(&operation.operation.id)
                            .map(|symbol| &symbol.span),
                        "INVALID_OPERATION_CASE",
                        "Prepared mocks must replace declared operation collaborators",
                    ));
                }
            }
            if !case.mocks.is_empty() {
                plan.capabilities.push(Capability::Mocks);
            }
            if case.id.trim().is_empty()
                || !case_ids.insert(case.id.clone())
                || case.invocation.subject != operation.operation.id
                || case.invocation.arguments.len() != operation.operation.parameters.len()
            {
                plan.issues.push(issue(ir.symbols.get(&operation.operation.id).map(|s|&s.span),"INVALID_OPERATION_CASE","Reviewed operation cases need unique IDs and the exact declared invocation signature"));
            }
        }
    }
    // Preserve source ordering within each declaration; sort only set-like metadata.
    plan.capabilities.sort_by_key(|c| format!("{c:?}"));
    plan.capabilities.dedup();
    plan
}
pub(super) fn issue(span: Option<&Span>, code: &str, message: &str) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: span.cloned(),
    }
}
pub(super) fn owned<'a>(ir: &'a WorkspaceIr, id: &str) -> Vec<&'a SemanticNode> {
    ir.nodes.iter().filter(|n| n.owner == id).collect()
}
fn property(ir: &WorkspaceIr, id: &str, budgets: &VerificationBudgets) -> PropertyPlan {
    let nodes = owned(ir, id);
    let declared = nodes
        .iter()
        .filter(|n| n.kind == StatementKind::Forall)
        .filter_map(|n| n.terms.first().map(|t| t.text.clone()))
        .collect::<BTreeSet<_>>();
    let all = nodes
        .iter()
        .filter(|n| n.kind == StatementKind::Assertion)
        .flat_map(|n| n.expressions.clone())
        .collect::<Vec<_>>();
    let (variables, mut general_issues) = match expressions::variables(&all, &BTreeSet::new()) {
        Ok(values) => (values, Vec::new()),
        Err(issue) => (Vec::new(), vec![*issue]),
    };
    let quantifiers = variables
        .into_iter()
        .filter(|v| declared.contains(&v.name))
        .collect::<Vec<_>>();
    if quantifiers.len() != declared.len() {
        general_issues.push(issue(
            nodes.first().map(|n| &n.span),
            "QUANTIFIER_TYPE_UNAVAILABLE",
            "A quantified binding has no retained typed occurrence",
        ));
    }
    let mut assertions = Vec::new();
    for node in nodes
        .into_iter()
        .filter(|n| n.kind == StatementKind::Assertion)
    {
        for (index, expression) in node.expressions.iter().enumerate() {
            let mut issues = general_issues.clone();
            let used =
                match expressions::variables(std::slice::from_ref(expression), &BTreeSet::new()) {
                    Ok(values) => values,
                    Err(issue) => {
                        issues.push(*issue);
                        Vec::new()
                    }
                };
            if used.iter().any(|v| !declared.contains(&v.name)) {
                issues.push(issue(
                    Some(&node.span),
                    "UNBOUND_PROPERTY_VALUE",
                    "Property uses a value not bound by forall",
                ));
            }
            let corpus = generate_inputs(ir, &used, budgets);
            issues.extend(corpus.issues);
            assertions.push(AssertionPlan {
                source_id: node.id.clone(),
                bounded_corpus: !corpus.exhaustive,
                issues,
                cases: corpus
                    .bindings
                    .into_iter()
                    .enumerate()
                    .map(|(case, variables)| VerificationCase {
                        id: format!("{}:{index}:{case}", node.id),
                        variables,
                        assertion: expression.clone(),
                    })
                    .collect(),
            });
        }
    }
    if assertions.is_empty() {
        assertions.push(AssertionPlan {
            source_id: id.into(),
            cases: Vec::new(),
            bounded_corpus: false,
            issues: vec![issue(
                None,
                "MISSING_ASSERTION",
                "Invariant has no executable assertions",
            )],
        });
    }
    PropertyPlan {
        subject: id.into(),
        quantifiers,
        assertions,
    }
}
