//! Effect evidence uses runtime observations; declarations never imply counts.
use crate::OperationPlan;
use dctx0_core::*;
use dctx0_interpreter::{
    Evaluation, ObservationContext, ValueContract, check_value, evaluate_observed,
};
use std::collections::BTreeMap;

fn unknown(node: &SemanticNode, code: &str, message: &str) -> VerificationResult {
    VerificationResult::Unknown {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: Some(node.span.clone()),
        },
    }
}

fn effect_reference<'a>(
    ir: &'a WorkspaceIr,
    plan: &OperationPlan,
    node: &SemanticNode,
    kind: ReferenceKind,
) -> Option<&'a Reference> {
    ir.references.iter().find(|r| {
        r.from == plan.operation.id
            && r.kind == kind
            && r.span.file == node.span.file
            && r.span.start.offset >= node.span.start.offset
            && r.span.end.offset <= node.span.end.offset
    })
}

pub(crate) fn assess_call(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    node: &SemanticNode,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let Some(calls) = &context.calls else {
        return unknown(
            node,
            "CALL_OBSERVER_REQUIRED",
            "Call effects need actual invocation observations",
        );
    };
    let Some(reference) = effect_reference(ir, plan, node, ReferenceKind::Calls) else {
        return unknown(
            node,
            "CALL_REFERENCE_REQUIRED",
            "Call effect has no resolved operation",
        );
    };
    assess_call_contract(ir, node, &reference.target, calls, budgets)
}

fn assess_call_contract(
    ir: &WorkspaceIr,
    node: &SemanticNode,
    subject: &str,
    calls: &[InvocationObservation],
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let Some(operation) = ir.operations.get(subject) else {
        return unknown(
            node,
            "CALL_CONTRACT_REQUIRED",
            "Observed collaborator has no pinned signature",
        );
    };
    for call in calls.iter().filter(|c| c.invocation.subject == subject) {
        let mut valid = call.invocation.arguments.len() == operation.parameters.len();
        for (parameter, value) in operation.parameters.iter().zip(&call.invocation.arguments) {
            match check_value(ir, &parameter.ty, value, budgets) {
                ValueContract::Accepted => {}
                ValueContract::Rejected { .. } => valid = false,
                ValueContract::Unknown { issue } => return VerificationResult::Unknown { issue },
            }
        }
        let (value, ty) = match &call.observation {
            Observation::Returned { value }
                if matches!(value, VerificationValue::Failure { .. }) =>
            {
                (
                    value,
                    TypeRef {
                        name: "Failure".into(),
                        symbol_id: None,
                        arguments: operation.failures.clone(),
                    },
                )
            }
            Observation::Returned { value } => (value, operation.result.clone()),
            Observation::Unavailable { .. } => {
                return unknown(
                    node,
                    "CALL_OBSERVATION_UNAVAILABLE",
                    "Collaborator observation is unavailable",
                );
            }
            Observation::Threw { .. } => {
                return VerificationResult::Fail {
                    issue: VerificationIssue {
                        code: "CALL_EXCEPTION".into(),
                        message: "Observed collaborator threw an unexpected exception".into(),
                        span: Some(node.span.clone()),
                    },
                };
            }
        };
        match check_value(ir, &ty, value, budgets) {
            ValueContract::Accepted => {}
            ValueContract::Rejected { .. } => valid = false,
            ValueContract::Unknown { issue } => return VerificationResult::Unknown { issue },
        }
        if !valid {
            return VerificationResult::Fail {
                issue: VerificationIssue {
                    code: "CALL_CONTRACT_VIOLATED".into(),
                    message:
                        "Actual collaborator arguments or outcome violate its pinned signature"
                            .into(),
                    span: Some(node.span.clone()),
                },
            };
        }
    }
    // A call declaration grants permission; exact counts need an explicit trace.
    VerificationResult::Pass
}

pub(crate) fn bind_call_aliases(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    context: &ObservationContext,
    variables: &mut BTreeMap<String, VerificationValue>,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let Some(calls) = &context.calls else {
        return VerificationResult::Pass;
    };
    for node in plan
        .clauses
        .iter()
        .filter(|n| n.kind == StatementKind::Call)
    {
        let Some(Syntax::Effect {
            result_alias: Some(alias),
            ..
        }) = &node.syntax
        else {
            continue;
        };
        let Some(reference) = effect_reference(ir, plan, node, ReferenceKind::Calls) else {
            return unknown(
                node,
                "CALL_REFERENCE_REQUIRED",
                "Call alias has no resolved operation",
            );
        };
        let matching = calls
            .iter()
            .filter(|c| c.invocation.subject == reference.target)
            .collect::<Vec<_>>();
        // An early declared failure may bypass a success-only collaborator.
        // Leave its alias unbound; expressions that actually need it stay unknown.
        if matching.is_empty()
            && variables.get("success") == Some(&VerificationValue::Boolean(false))
        {
            continue;
        }
        if matching.len() != 1 {
            return unknown(
                node,
                "CALL_ALIAS_AMBIGUOUS",
                "Call alias requires one observed invocation; no result is guessed",
            );
        }
        let checked = assess_call(ir, plan, node, context, budgets);
        if checked != VerificationResult::Pass {
            return checked;
        }
        if let Observation::Returned { value } = &matching[0].observation {
            if !matches!(value, VerificationValue::Failure { .. }) {
                variables.insert(alias.clone(), value.clone());
            }
        }
    }
    VerificationResult::Pass
}

pub(crate) fn assess_emission(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    node: &SemanticNode,
    variables: &BTreeMap<String, VerificationValue>,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let Some(events) = &context.events else {
        return unknown(
            node,
            "EVENT_OBSERVER_REQUIRED",
            "Emission needs actual event observations",
        );
    };
    let Some(reference) = ir.references.iter().find(|r| {
        r.from == plan.operation.id
            && r.kind == ReferenceKind::Emits
            && r.span.file == node.span.file
            && r.span.start.offset >= node.span.start.offset
            && r.span.end.offset <= node.span.end.offset
    }) else {
        return unknown(
            node,
            "EVENT_REFERENCE_REQUIRED",
            "Emission has no resolved event declaration",
        );
    };
    // Declared failures have no implicit success emission requirement.
    if variables.get("success") == Some(&VerificationValue::Boolean(false)) {
        return VerificationResult::Pass;
    }
    let Some(Syntax::Effect { payload, .. }) = &node.syntax else {
        return unknown(
            node,
            "EVENT_PAYLOAD_REQUIRED",
            "Emission has no typed effect payload",
        );
    };
    let mut values = Vec::new();
    for expression in &node.expressions {
        match evaluate_observed(ir, expression, variables, &[], context, budgets) {
            Evaluation::Complete(value) => values.push(value),
            Evaluation::Unknown(issue) => return VerificationResult::Unknown { issue },
            Evaluation::ApplicationFailure(issue) => return VerificationResult::Fail { issue },
            _ => {
                return unknown(
                    node,
                    "EVENT_PAYLOAD_UNAVAILABLE",
                    "Emission payload needs an unavailable call observation",
                );
            }
        }
    }
    let expected = match payload {
        Some(EffectPayload::Record { fields }) if fields.len() == values.len() => {
            VerificationValue::Record(
                fields
                    .iter()
                    .map(|(name, _)| name.clone())
                    .zip(values)
                    .collect(),
            )
        }
        Some(EffectPayload::Scalar { .. }) if values.len() == 1 => values.remove(0),
        Some(EffectPayload::Scalar { expression })
            if expression.is_empty() && values.is_empty() =>
        {
            VerificationValue::Record(BTreeMap::new())
        }
        _ => {
            return unknown(
                node,
                "EVENT_PAYLOAD_REQUIRED",
                "Complete modeled event payload is unavailable",
            );
        }
    };
    if events
        .iter()
        .any(|event| event.subject == reference.target && event.payload == expected)
    {
        VerificationResult::Pass
    } else {
        VerificationResult::Fail {
            issue: VerificationIssue {
                code: "EVENT_EFFECT_VIOLATED".into(),
                message: "Actual events omit the modeled emission or have a different payload"
                    .into(),
                span: Some(node.span.clone()),
            },
        }
    }
}

/// Delivered observations must stay within the declared operation boundary.
/// Missing observers are handled by mandatory clause assessment, not invented.
pub(crate) fn assess_boundary(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let mut unresolved = None;
    let observations = context
        .calls
        .iter()
        .flatten()
        .map(|call| (&call.invocation.subject, ReferenceKind::Calls))
        .chain(
            context
                .events
                .iter()
                .flatten()
                .map(|event| (&event.subject, ReferenceKind::Emits)),
        );
    for (subject, kind) in observations {
        let issue = |code: &str, message: String| VerificationIssue {
            code: code.into(),
            message,
            span: ir
                .symbols
                .get(&plan.operation.id)
                .map(|symbol| symbol.span.clone()),
        };
        if !ir.symbols.contains_key(subject) {
            unresolved.get_or_insert_with(|| {
                issue(
                    "EFFECT_LABEL_UNMODELED",
                    format!("Actual effect label {subject} is not modeled"),
                )
            });
            continue;
        }
        if !ir.references.iter().any(|reference| {
            reference.from == plan.operation.id
                && reference.target == *subject
                && reference.kind == kind
        }) {
            return VerificationResult::Fail {
                issue: issue(
                    "UNDECLARED_OPERATION_EFFECT",
                    format!(
                        "Actual {kind:?} effect on {subject} is outside the declared operation boundary"
                    ),
                ),
            };
        }
    }
    // A count assertion or early consumer failure does not waive the actual
    // collaborator/event contract. Validate every delivered permitted effect.
    for event in context.events.iter().flatten() {
        let ty = TypeRef {
            name: event.subject.clone(),
            symbol_id: Some(event.subject.clone()),
            arguments: vec![],
        };
        match check_value(ir, &ty, &event.payload, budgets) {
            ValueContract::Accepted => {}
            ValueContract::Unknown { issue } => {
                unresolved.get_or_insert(issue);
            }
            ValueContract::Rejected { reason } => {
                return VerificationResult::Fail {
                    issue: VerificationIssue {
                        code: "EVENT_CONTRACT_VIOLATED".into(),
                        message: reason,
                        span: ir
                            .symbols
                            .get(&plan.operation.id)
                            .map(|symbol| symbol.span.clone()),
                    },
                };
            }
        }
    }
    for call in context.calls.iter().flatten() {
        let Some(node) = ir.nodes.iter().find(|node| node.id == plan.operation.id) else {
            return VerificationResult::Unknown {
                issue: VerificationIssue {
                    code: "CALL_CONTEXT_REQUIRED".into(),
                    message: "Caller declaration is unavailable".into(),
                    span: None,
                },
            };
        };
        let assessed = assess_call_contract(
            ir,
            node,
            &call.invocation.subject,
            std::slice::from_ref(call),
            budgets,
        );
        match assessed {
            VerificationResult::Pass => {}
            VerificationResult::Unknown { issue } => {
                unresolved.get_or_insert(issue);
            }
            failed @ VerificationResult::Fail { .. } => return failed,
        }
    }
    unresolved.map_or(VerificationResult::Pass, |issue| {
        VerificationResult::Unknown { issue }
    })
}
