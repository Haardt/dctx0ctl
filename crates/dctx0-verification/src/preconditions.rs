//! Classify generated candidates centrally before native operation execution.
use crate::InputCorpus;
use dctx0_core::{
    SemanticNode, StatementKind, VerificationBudgets, VerificationIssue, VerificationValue,
    WorkspaceIr,
};
use dctx0_interpreter::{Evaluation, evaluate};
use serde::{Deserialize, Serialize};

pub const PRECONDITION_SELECTION_VERSION: &str = "dctx0-preconditions-1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreconditionSelection {
    pub version: String,
    pub examined_candidates: usize,
    pub accepted_candidates: usize,
    pub rejected_candidates: usize,
    pub unresolved_candidates: usize,
}
fn issue(node: &SemanticNode, code: &str, message: &str) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: Some(node.span.clone()),
    }
}
/// False generated preconditions classify candidates outside the valid domain.
/// Unresolved candidates stay in the plan with an explicit issue. Reviewed
/// examples/cases are independent mandatory input and are never filtered here.
pub fn select_precondition_inputs(
    ir: &WorkspaceIr,
    clauses: &[SemanticNode],
    mut corpus: InputCorpus,
    budgets: &VerificationBudgets,
) -> (InputCorpus, Option<PreconditionSelection>) {
    let requires = clauses
        .iter()
        .filter(|node| node.kind == StatementKind::Requires)
        .collect::<Vec<_>>();
    if requires.is_empty() {
        return (corpus, None);
    }
    let mut selection = PreconditionSelection {
        version: PRECONDITION_SELECTION_VERSION.into(),
        examined_candidates: corpus.bindings.len(),
        accepted_candidates: 0,
        rejected_candidates: 0,
        unresolved_candidates: 0,
    };
    let mut retained = vec![];
    for bindings in corpus.bindings {
        let mut rejected = false;
        let mut unresolved = vec![];
        for node in &requires {
            if node.expressions.is_empty() {
                unresolved.push(issue(
                    node,
                    "PRECONDITION_GENERATION_UNSUPPORTED",
                    "Requires clause has no executable expression",
                ));
            }
            for expression in &node.expressions {
                match evaluate(ir, expression, &bindings, &[], budgets) {
                    Evaluation::Complete(VerificationValue::Boolean(true)) => {}
                    Evaluation::Complete(VerificationValue::Boolean(false)) => {
                        rejected = true;
                        break;
                    }
                    Evaluation::Unknown(problem) | Evaluation::ApplicationFailure(problem) => {
                        unresolved.push(problem)
                    }
                    Evaluation::Invoke(_) => unresolved.push(issue(
                        node,
                        "PRECONDITION_GENERATION_OBSERVATION_REQUIRED",
                        "Generated input eligibility needs a real invocation observation",
                    )),
                    Evaluation::Complete(_) => unresolved.push(issue(
                        node,
                        "PRECONDITION_GENERATION_NON_BOOLEAN",
                        "Precondition did not evaluate to a Boolean",
                    )),
                }
            }
            if rejected {
                break;
            }
        }
        if rejected {
            selection.rejected_candidates += 1;
        } else {
            if unresolved.is_empty() {
                selection.accepted_candidates += 1;
            } else {
                selection.unresolved_candidates += 1;
                corpus.issues.extend(unresolved);
            }
            retained.push(bindings);
        }
    }
    corpus.bindings = retained;
    if selection.unresolved_candidates != 0 {
        corpus.exhaustive = false;
    }
    if corpus.bindings.is_empty() {
        corpus.issues.push(issue(
            requires[0],
            "EMPTY_PRECONDITION_CORPUS",
            "No accepted candidates exist in the selected generated corpus",
        ));
    }
    corpus.issues.sort_by(|a, b| {
        let location = |span: &Option<dctx0_core::Span>| {
            span.as_ref()
                .map(|s| (s.file.clone(), s.start.offset, s.end.offset))
        };
        (&a.code, &a.message, location(&a.span)).cmp(&(&b.code, &b.message, location(&b.span)))
    });
    corpus.issues.dedup();
    (corpus, Some(selection))
}

/// Discharge only planning-time missing-state hints after independently checking
/// every exact generated input against recorded actual before-state evidence.
/// Manual requirements and other unsupported-expression issues are untouched.
pub fn resolved_state_preconditions(
    ir: &WorkspaceIr,
    plan: &crate::ProgramPlan,
    cases: &[dctx0_core::VerificationCaseRecord],
) -> Vec<VerificationIssue> {
    use dctx0_core::{Invocation, StatePhase, VerificationCaseKind, VerificationResult};
    let mut resolved = Vec::new();
    for operation in &plan.operations {
        if operation.inputs.bindings.is_empty() {
            continue;
        }
        let complete = operation
            .inputs
            .bindings
            .iter()
            .enumerate()
            .all(|(index, bindings)| {
                let id = format!("{}:operation:{index}", operation.operation.id);
                let matching = cases
                    .iter()
                    .filter(|case| case.id == id)
                    .collect::<Vec<_>>();
                let [case] = matching.as_slice() else {
                    return false;
                };
                let arguments = operation
                    .operation
                    .parameters
                    .iter()
                    .filter_map(|parameter| bindings.get(&parameter.name).cloned())
                    .collect::<Vec<_>>();
                let invocation = Invocation {
                    subject: operation.operation.id.clone(),
                    arguments: arguments.clone(),
                };
                if case.kind != VerificationCaseKind::Operation
                    || case.subject != operation.operation.id
                    || case.bindings != *bindings
                    || case.arguments != vec![arguments]
                    || case.setup != operation.generated_setup(bindings)
                    || case.result != VerificationResult::Pass
                    || !case
                        .observations
                        .iter()
                        .any(|observed| observed.invocation == invocation)
                {
                    return false;
                }
                let Some(before) = case.states.iter().rev().find(|state| {
                    state.phase == StatePhase::Before && state.phase_recorded == Some(true)
                }) else {
                    return false;
                };
                let context = dctx0_interpreter::ObservationContext {
                    before: Some(before.entities.clone()),
                    calls: Some(case.observations.clone()),
                    ..Default::default()
                };
                crate::operation_ready(ir, operation, &invocation, &context, &plan.budgets)
                    == VerificationResult::Pass
            });
        if complete {
            resolved.extend(
                operation
                    .inputs
                    .issues
                    .iter()
                    .filter(|issue| issue.code == "STATE_OBSERVER_REQUIRED")
                    .cloned(),
            );
        }
    }
    resolved.sort_by(|left, right| (&left.code, &left.message).cmp(&(&right.code, &right.message)));
    resolved.dedup();
    resolved
}
