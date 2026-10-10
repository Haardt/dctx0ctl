//! Pure operation-clause assessment over actual observations.
use crate::OperationPlan;
use dctx0_core::*;
use dctx0_interpreter::{
    Evaluation, ObservationContext, ValueContract, check_value, evaluate_observed,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationCaseInput {
    pub id: String,
    pub invocation: Invocation,
    pub setup: Vec<Invocation>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct OperationObservers {
    pub states: bool,
    pub calls: bool,
    pub events: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationAssessment {
    pub subject: String,
    pub checked: Vec<String>,
    pub open: Vec<VerificationIssue>,
    pub result: VerificationResult,
}
fn issue(
    plan: &OperationPlan,
    code: &str,
    message: impl Into<String>,
    span: Option<Span>,
) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: format!("{}: {}", plan.operation.id, message.into()),
        span,
    }
}
fn expression(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    node: &SemanticNode,
    variables: &BTreeMap<String, VerificationValue>,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    if node.expressions.is_empty() {
        return VerificationResult::Unknown {
            issue: issue(
                plan,
                "CLAUSE_NOT_EXECUTABLE",
                "Clause has no typed Boolean expression",
                Some(node.span.clone()),
            ),
        };
    }
    for expression in &node.expressions {
        match evaluate_observed(ir, expression, variables, &[], context, budgets) {
            Evaluation::Complete(VerificationValue::Boolean(true)) => {}
            Evaluation::Complete(VerificationValue::Boolean(false)) => {
                return VerificationResult::Fail {
                    issue: issue(
                        plan,
                        "OPERATION_CLAUSE_VIOLATED",
                        format!("{:?}", node.kind),
                        Some(expression.span.clone()),
                    ),
                };
            }
            Evaluation::Unknown(issue) => return VerificationResult::Unknown { issue },
            Evaluation::ApplicationFailure(issue) => return VerificationResult::Fail { issue },
            _ => {
                return VerificationResult::Unknown {
                    issue: issue(
                        plan,
                        "CLAUSE_OBSERVATION_REQUIRED",
                        "Clause needs a missing call observation or Boolean result",
                        Some(expression.span.clone()),
                    ),
                };
            }
        }
    }
    VerificationResult::Pass
}
fn bindings(
    plan: &OperationPlan,
    invocation: &Invocation,
) -> Option<BTreeMap<String, VerificationValue>> {
    if invocation.subject != plan.operation.id
        || invocation.arguments.len() != plan.operation.parameters.len()
    {
        return None;
    }
    Some(
        plan.operation
            .parameters
            .iter()
            .zip(&invocation.arguments)
            .map(|(p, v)| (p.name.clone(), v.clone()))
            .collect(),
    )
}
pub fn operation_ready(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    invocation: &Invocation,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let Some(variables) = bindings(plan, invocation) else {
        return VerificationResult::Unknown {
            issue: issue(
                plan,
                "OPERATION_INVOCATION",
                "Invocation does not match the pinned signature",
                None,
            ),
        };
    };
    for (parameter, value) in plan.operation.parameters.iter().zip(&invocation.arguments) {
        match check_value(ir, &parameter.ty, value, budgets) {
            ValueContract::Accepted => {}
            ValueContract::Rejected { reason } => {
                return VerificationResult::Unknown {
                    issue: issue(plan, "INVALID_OPERATION_INPUT", reason, None),
                };
            }
            ValueContract::Unknown { issue } => return VerificationResult::Unknown { issue },
        }
    }
    let mut before = context.clone();
    before.phase = StatePhase::Before;
    for node in plan
        .clauses
        .iter()
        .filter(|n| n.kind == StatementKind::Requires)
    {
        match expression(ir, plan, node, &variables, &before, budgets) {
            VerificationResult::Pass => {}
            VerificationResult::Fail { issue: mut problem } => {
                problem.code = "OPERATION_PRECONDITION_UNMET".into();
                return VerificationResult::Unknown { issue: problem };
            }
            unknown => return unknown,
        }
    }
    VerificationResult::Pass
}
pub fn assess_operation(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    observed: &InvocationObservation,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> OperationAssessment {
    let mut report = OperationAssessment {
        subject: plan.operation.id.clone(),
        checked: vec![],
        open: vec![],
        result: operation_ready(ir, plan, &observed.invocation, context, budgets),
    };
    if report.result != VerificationResult::Pass {
        return report;
    }
    let mut variables = bindings(plan, &observed.invocation).unwrap_or_default();
    let value = match &observed.observation {
        Observation::Returned { value } => value,
        Observation::Threw { message } => {
            report.result = VerificationResult::Fail {
                issue: issue(plan, "APPLICATION_EXCEPTION", message, None),
            };
            return report;
        }
        Observation::Unavailable { reason } => {
            report.result = VerificationResult::Unknown {
                issue: issue(plan, "OPERATION_UNAVAILABLE", reason, None),
            };
            return report;
        }
    };
    let failure = matches!(value, VerificationValue::Failure { .. });
    let ty = if failure {
        TypeRef {
            name: "Failure".into(),
            symbol_id: None,
            arguments: plan.operation.failures.clone(),
        }
    } else {
        plan.operation.result.clone()
    };
    match check_value(ir, &ty, value, budgets) {
        ValueContract::Accepted => {}
        ValueContract::Rejected { reason } => {
            report.result = VerificationResult::Fail {
                issue: issue(plan, "OPERATION_RESULT_CONTRACT", reason, None),
            };
            return report;
        }
        ValueContract::Unknown { issue } => {
            report.result = VerificationResult::Unknown { issue };
            return report;
        }
    }
    variables.insert("result".into(), value.clone());
    variables.insert("success".into(), VerificationValue::Boolean(!failure));
    variables.insert(
        "failure".into(),
        if failure {
            value.clone()
        } else {
            VerificationValue::Unit
        },
    );
    let mut after = context.clone();
    after.phase = StatePhase::After;
    match crate::operation_effects::bind_call_aliases(ir, plan, &after, &mut variables, budgets) {
        VerificationResult::Pass => {}
        VerificationResult::Fail { issue } => {
            report.result = VerificationResult::Fail { issue };
            return report;
        }
        VerificationResult::Unknown { issue } => report.open.push(issue),
    }
    for node in &plan.clauses {
        let result = match node.kind {
            StatementKind::Requires => VerificationResult::Pass,
            StatementKind::Ensures => expression(ir, plan, node, &variables, &after, budgets),
            StatementKind::State => {
                crate::operation_state::assess_state(ir, plan, node, &variables, &after, budgets)
            }
            StatementKind::Emit => crate::operation_effects::assess_emission(
                ir, plan, node, &variables, &after, budgets,
            ),
            StatementKind::Call => {
                crate::operation_effects::assess_call(ir, plan, node, &after, budgets)
            }
            _ => VerificationResult::Unknown {
                issue: issue(
                    plan,
                    "OPERATION_EFFECT_PENDING",
                    format!("{:?} needs its typed runtime obligation", node.kind),
                    Some(node.span.clone()),
                ),
            },
        };
        match result {
            VerificationResult::Pass => report.checked.push(node.id.clone()),
            VerificationResult::Fail { issue } => {
                report.result = VerificationResult::Fail { issue };
                return report;
            }
            VerificationResult::Unknown { issue } => report.open.push(issue),
        }
    }
    if let Some(issue) = report.open.first() {
        report.result = VerificationResult::Unknown {
            issue: issue.clone(),
        };
    }
    report
}
