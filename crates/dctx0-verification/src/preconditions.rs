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
