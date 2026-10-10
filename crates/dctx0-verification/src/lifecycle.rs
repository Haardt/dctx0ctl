//! Lifecycle verdicts use actual entity observations, never operation reply labels.
use crate::{LifecyclePath, LifecyclePlan};
use dctx0_core::*;
use dctx0_interpreter::{
    Evaluation, ObservationContext, ValueContract, check_value, evaluate_observed,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleObservation {
    pub invocation: InvocationObservation,
    pub context: ObservationContext,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleAssessment {
    pub case_id: String,
    pub result: VerificationResult,
    pub reached_states: Vec<String>,
    pub completed_transitions: Vec<String>,
    pub open_transitions: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleCaseInput {
    pub keys: Vec<VerificationValue>,
    /// Reviewed real application calls establish the initial state.
    pub setup: Vec<Invocation>,
    /// Ordered arguments for every transition occurrence, including repeats.
    pub arguments: Vec<Vec<VerificationValue>>,
}
fn unknown(code: &str, message: &str) -> VerificationResult {
    VerificationResult::Unknown {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: None,
        },
    }
}
fn failed(code: &str, message: &str) -> VerificationResult {
    VerificationResult::Fail {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: None,
        },
    }
}
pub fn lifecycle_status(
    ir: &WorkspaceIr,
    plan: &LifecyclePlan,
    phase: StatePhase,
    keys: &[VerificationValue],
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> Result<String, Box<VerificationResult>> {
    let entity = ir.symbols.get(&plan.entity).ok_or_else(|| {
        Box::new(unknown(
            "LIFECYCLE_ENTITY",
            "Lifecycle entity declaration is missing",
        ))
    })?;
    let field = ir
        .data
        .get(&plan.entity)
        .and_then(|data| data.fields.iter().find(|field| field.name == plan.field))
        .ok_or_else(|| Box::new(unknown("LIFECYCLE_FIELD", "Lifecycle field is missing")))?;
    let expression = Expression {
        ty: field.ty.clone(),
        span: entity.span.clone(),
        value: ExpressionKind::Field {
            name: plan.field.clone(),
            target: Box::new(Expression {
                ty: TypeRef {
                    name: entity.name.clone(),
                    symbol_id: Some(entity.id.clone()),
                    arguments: vec![],
                },
                span: entity.span.clone(),
                value: ExpressionKind::State {
                    phase,
                    entity: plan.entity.clone(),
                    keys: keys
                        .iter()
                        .enumerate()
                        .map(|(index, _)| Expression {
                            ty: TypeRef::builtin("Sample"),
                            span: entity.span.clone(),
                            value: ExpressionKind::Variable {
                                name: format!("key:{index}"),
                            },
                        })
                        .collect(),
                },
            }),
        },
    };
    let variables = keys
        .iter()
        .enumerate()
        .map(|(index, value)| (format!("key:{index}"), value.clone()))
        .collect();
    match evaluate_observed(ir, &expression, &variables, &[], context, budgets) {
        Evaluation::Complete(VerificationValue::Variant { name, .. })
        | Evaluation::Complete(VerificationValue::String(name)) => Ok(name),
        Evaluation::Unknown(issue) => Err(Box::new(VerificationResult::Unknown { issue })),
        _ => Err(Box::new(unknown(
            "LIFECYCLE_STATE_REPRESENTATION",
            "Lifecycle needs a named actual String/variant state",
        ))),
    }
}
pub fn lifecycle_ready(
    ir: &WorkspaceIr,
    plan: &LifecyclePlan,
    index: usize,
    keys: &[VerificationValue],
    invocation: &Invocation,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let Some(transition) = plan.transitions.get(index) else {
        return unknown("LIFECYCLE_TRANSITION", "Transition is missing");
    };
    let before = match lifecycle_status(ir, plan, StatePhase::Before, keys, context, budgets) {
        Ok(state) => state,
        Err(result) => return *result,
    };
    if before != transition.from {
        return failed(
            "LIFECYCLE_STATE_MISMATCH",
            "Actual initial state does not match the transition source",
        );
    }
    if let Some(guard) = &transition.guard {
        let variables = ir
            .operations
            .get(&transition.operation)
            .map(|op| {
                op.parameters
                    .iter()
                    .zip(&invocation.arguments)
                    .map(|(p, v)| (p.name.clone(), v.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let mut before_context = context.clone();
        before_context.phase = StatePhase::Before;
        match evaluate_observed(ir, guard, &variables, &[], &before_context, budgets) {
            Evaluation::Complete(VerificationValue::Boolean(true)) => {}
            Evaluation::Unknown(issue) => return VerificationResult::Unknown { issue },
            _ => {
                return unknown(
                    "LIFECYCLE_GUARD",
                    "Mandatory path guard was not satisfied or executable",
                );
            }
        }
    }
    VerificationResult::Pass
}
pub fn assess_lifecycle(
    ir: &WorkspaceIr,
    plan: &LifecyclePlan,
    path: &LifecyclePath,
    keys: &[VerificationValue],
    observations: &[LifecycleObservation],
    budgets: &VerificationBudgets,
) -> LifecycleAssessment {
    let mut report = LifecycleAssessment {
        case_id: path.id.clone(),
        result: VerificationResult::Pass,
        reached_states: vec![],
        completed_transitions: vec![],
        open_transitions: path
            .transitions
            .iter()
            .filter_map(|index| plan.transitions.get(*index))
            .map(|transition| transition.source_id.clone())
            .collect(),
    };
    if !plan.paths.contains(path)
        || path.transitions.is_empty()
        || keys.is_empty()
        || plan
            .states
            .iter()
            .find(|state| state.initial)
            .is_none_or(|state| state.name != path.initial)
    {
        report.result = unknown(
            "INVALID_LIFECYCLE_CASE",
            "A pinned nonempty lifecycle path and full entity key are required",
        );
        return report;
    }
    if let Some(issue) = plan.issues.first() {
        report.result = VerificationResult::Unknown {
            issue: issue.clone(),
        };
        return report;
    }
    if observations.len() > path.transitions.len() {
        report.result = unknown(
            "SURPLUS_LIFECYCLE_OBSERVATIONS",
            "Evidence contains transitions outside the selected path",
        );
        return report;
    }
    for (position, index) in path.transitions.iter().enumerate() {
        let Some(transition) = plan.transitions.get(*index) else {
            report.result = unknown(
                "LIFECYCLE_TRANSITION",
                "Path refers to a missing transition",
            );
            break;
        };
        let Some(observation) = observations.get(position) else {
            report.result = unknown(
                "MISSING_LIFECYCLE_TRANSITION",
                "Mandatory transition was not executed",
            );
            break;
        };
        if observation.invocation.invocation.subject != transition.operation {
            report.result = unknown(
                "LIFECYCLE_INVOCATION",
                "Actual invocation does not match the pinned transition",
            );
            break;
        }
        let Some(operation) = ir.operations.get(&transition.operation) else {
            report.result = unknown("LIFECYCLE_OPERATION", "Transition operation is unavailable");
            break;
        };
        if operation.parameters.len() != observation.invocation.invocation.arguments.len() {
            report.result = unknown(
                "LIFECYCLE_ARGUMENTS",
                "Actual arguments do not match the transition contract",
            );
            break;
        }
        if operation
            .parameters
            .iter()
            .zip(&observation.invocation.invocation.arguments)
            .any(|(parameter, value)| {
                check_value(ir, &parameter.ty, value, budgets) != ValueContract::Accepted
            })
        {
            report.result = unknown(
                "LIFECYCLE_ARGUMENTS",
                "Transition argument contracts are invalid or unsupported",
            );
            break;
        }
        if position > 0 && observation.context.before != observations[position - 1].context.after {
            report.result = unknown(
                "DISCONNECTED_LIFECYCLE_EVIDENCE",
                "Consecutive observations do not share the delivered boundary state",
            );
            break;
        }
        let before = match lifecycle_status(
            ir,
            plan,
            StatePhase::Before,
            keys,
            &observation.context,
            budgets,
        ) {
            Ok(state) => state,
            Err(result) => {
                report.result = *result;
                break;
            }
        };
        let after = match lifecycle_status(
            ir,
            plan,
            StatePhase::After,
            keys,
            &observation.context,
            budgets,
        ) {
            Ok(state) => state,
            Err(result) => {
                report.result = *result;
                break;
            }
        };
        if before != transition.from || after != transition.to {
            report.result = failed(
                "LIFECYCLE_STATE_MISMATCH",
                "Actual before/after state violates the declared transition",
            );
            break;
        }
        if position == 0 {
            report.reached_states.push(before);
        }
        if let Some(guard) = &transition.guard {
            let variables = operation
                .parameters
                .iter()
                .zip(&observation.invocation.invocation.arguments)
                .map(|(p, v)| (p.name.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>();
            let mut before_context = observation.context.clone();
            before_context.phase = StatePhase::Before;
            match evaluate_observed(ir, guard, &variables, &[], &before_context, budgets) {
                Evaluation::Complete(VerificationValue::Boolean(true)) => {}
                Evaluation::Unknown(issue) => {
                    report.result = VerificationResult::Unknown { issue };
                    break;
                }
                _ => {
                    report.result = unknown(
                        "LIFECYCLE_GUARD",
                        "Mandatory path guard was not satisfied or executable",
                    );
                    break;
                }
            }
        }
        match &observation.invocation.observation {
            Observation::Returned { value }
                if !matches!(value, VerificationValue::Failure { .. }) =>
            {
                match check_value(ir, &operation.result, value, budgets) {
                    ValueContract::Accepted => {}
                    ValueContract::Rejected { reason } => {
                        report.result = failed("LIFECYCLE_RETURN_CONTRACT", &reason);
                        break;
                    }
                    ValueContract::Unknown { issue } => {
                        report.result = VerificationResult::Unknown { issue };
                        break;
                    }
                }
            }
            Observation::Unavailable { reason } => {
                report.result = unknown("OPERATION_UNAVAILABLE", reason);
                break;
            }
            _ => {
                report.result = failed(
                    "LIFECYCLE_OPERATION_FAILED",
                    "Transition operation did not return success",
                );
                break;
            }
        }
        report.reached_states.push(after);
        report
            .completed_transitions
            .push(transition.source_id.clone());
    }
    report.open_transitions = path
        .transitions
        .iter()
        .filter_map(|index| plan.transitions.get(*index))
        .skip(report.completed_transitions.len())
        .map(|transition| transition.source_id.clone())
        .collect();
    report
}
