//! State effects over actual snapshots, matching the reference's complete-source rules.
use crate::OperationPlan;
use dctx0_core::*;
use dctx0_interpreter::{
    Evaluation, ObservationContext, ValueContract, check_value, evaluate_observed,
};
use std::collections::BTreeMap;
fn problem(node: &SemanticNode, code: &str, message: &str, failed: bool) -> VerificationResult {
    let issue = VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: Some(node.span.clone()),
    };
    if failed {
        VerificationResult::Fail { issue }
    } else {
        VerificationResult::Unknown { issue }
    }
}
fn key(data: &DataIr, row: &VerificationValue) -> Option<Vec<VerificationValue>> {
    let VerificationValue::Record(fields) = row else {
        return None;
    };
    if data.keys.is_empty() {
        return None;
    }
    data.keys.iter().map(|k| fields.get(k).cloned()).collect()
}
fn same_rows(left: &[VerificationValue], right: &[VerificationValue]) -> bool {
    left.len() == right.len() && left.iter().all(|row| right.contains(row))
}
pub(crate) fn assess_state(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    node: &SemanticNode,
    variables: &BTreeMap<String, VerificationValue>,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    // State effects describe successful outcomes. Failure-specific preservation
    // must be expressed by an explicit postcondition, not invented here.
    if variables.get("success") == Some(&VerificationValue::Boolean(false)) {
        return VerificationResult::Pass;
    }
    let Some(reference) = ir.references.iter().find(|r| {
        r.from == plan.operation.id
            && r.kind == ReferenceKind::State
            && r.span.file == node.span.file
            && r.span.start.offset >= node.span.start.offset
            && r.span.end.offset <= node.span.end.offset
    }) else {
        return problem(
            node,
            "STATE_REFERENCE_REQUIRED",
            "State clause has no resolved entity reference",
            false,
        );
    };
    let Some(data) = ir.data.get(&reference.target) else {
        return problem(
            node,
            "STATE_ENTITY_REQUIRED",
            "State entity declaration is absent",
            false,
        );
    };
    let (Some(before), Some(after)) = (
        context.before.as_ref().and_then(|s| s.get(&data.id)),
        context.after.as_ref().and_then(|s| s.get(&data.id)),
    ) else {
        return problem(
            node,
            "STATE_OBSERVER_REQUIRED",
            "State clause needs actual before/after observations",
            false,
        );
    };
    let ty = TypeRef {
        name: data.id.clone(),
        symbol_id: Some(data.id.clone()),
        arguments: vec![],
    };
    for rows in [before, after] {
        let mut keys = Vec::new();
        for row in rows {
            match check_value(ir, &ty, row, budgets) {
                ValueContract::Accepted => {}
                ValueContract::Rejected { .. } => {
                    return problem(
                        node,
                        "INVALID_OBSERVED_STATE",
                        "Observed row violates its entity contract",
                        true,
                    );
                }
                ValueContract::Unknown { issue } => return VerificationResult::Unknown { issue },
            }
            let Some(identity) = key(data, row) else {
                return problem(
                    node,
                    "STATE_KEY_REQUIRED",
                    "Complete entity identity is unavailable",
                    false,
                );
            };
            if keys.contains(&identity) {
                return problem(
                    node,
                    "AMBIGUOUS_STATE_IDENTITY",
                    "Observed entities share a declared identity",
                    false,
                );
            }
            keys.push(identity);
        }
    }
    let mode = node.terms.first().map(|t| t.text.as_str()).unwrap_or("");
    let mut values = Vec::new();
    for expression in &node.expressions {
        match evaluate_observed(ir, expression, variables, &[], context, budgets) {
            Evaluation::Complete(value) => values.push(value),
            Evaluation::Unknown(issue) => return VerificationResult::Unknown { issue },
            _ => {
                return problem(
                    node,
                    "STATE_SOURCE_UNAVAILABLE",
                    "State selector/source could not be evaluated",
                    false,
                );
            }
        }
    }
    let mut expected = before.clone();
    match mode {
        "lists" | "observes" => {}
        "inserts" | "upserts" => {
            let Some(source) = values.first() else {
                return problem(
                    node,
                    "STATE_SOURCE_REQUIRED",
                    "Insertion/upsert needs a complete modeled entity source",
                    false,
                );
            };
            match check_value(ir, &ty, source, budgets) {
                ValueContract::Accepted => {}
                ValueContract::Unknown { issue } => return VerificationResult::Unknown { issue },
                _ => {
                    return problem(
                        node,
                        "STATE_SOURCE_INCOMPLETE",
                        "Modeled source is not a complete valid entity",
                        false,
                    );
                }
            }
            let Some(identity) = key(data, source) else {
                return problem(
                    node,
                    "STATE_KEY_REQUIRED",
                    "State source has no complete identity",
                    false,
                );
            };
            match expected
                .iter()
                .position(|row| key(data, row) == Some(identity.clone()))
            {
                Some(_) if mode == "inserts" => {
                    return problem(
                        node,
                        "INSERT_IDENTITY_EXISTS",
                        "Selected insertion identity already exists",
                        false,
                    );
                }
                Some(index) => expected[index] = source.clone(),
                None => expected.push(source.clone()),
            }
        }
        "removes" => {
            if values.len() != data.keys.len() {
                return problem(
                    node,
                    "STATE_SELECTOR_REQUIRED",
                    "Removal needs the complete modeled key",
                    false,
                );
            }
            expected.retain(|row| key(data, row) != Some(values.clone()));
        }
        "replaces" => {
            return problem(
                node,
                "STATE_REPLACEMENT_SOURCE_REQUIRED",
                "Replacement needs a complete source or grounded transition/equality",
                false,
            );
        }
        _ => {
            return problem(
                node,
                "STATE_MODE_UNSUPPORTED",
                "Unsupported state clause mode",
                false,
            );
        }
    }
    if !same_rows(&expected, after) {
        return problem(
            node,
            "STATE_EFFECT_VIOLATED",
            "Actual state differs from the complete modeled state effect",
            true,
        );
    }
    if plan.operation.result.name == "Unit" {
        return VerificationResult::Pass;
    }
    let result_ty = if matches!(plan.operation.result.name.as_str(), "Option" | "List") {
        plan.operation.result.arguments.first()
    } else {
        Some(&plan.operation.result)
    };
    if result_ty.and_then(|t| t.symbol_id.as_ref()) != Some(&data.id) {
        return problem(
            node,
            "STATE_RESULT_PROJECTION_REQUIRED",
            "State result needs an explicit modeled projection or direct entity result",
            false,
        );
    }
    let selected = if matches!(mode, "lists" | "observes") {
        if mode == "observes" {
            if values.len() != data.keys.len() {
                return problem(
                    node,
                    "STATE_SELECTOR_REQUIRED",
                    "Observation needs every key component",
                    false,
                );
            }
            before
                .iter()
                .filter(|row| key(data, row) == Some(values.clone()))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            before.clone()
        }
    } else if mode == "removes" {
        before
            .iter()
            .filter(|row| key(data, row) == Some(values.clone()))
            .cloned()
            .collect()
    } else {
        let Some(source) = values.first() else {
            return problem(
                node,
                "STATE_RESULT_PROJECTION_REQUIRED",
                "Changed entity result needs a modeled source",
                false,
            );
        };
        after
            .iter()
            .filter(|row| key(data, row) == key(data, source))
            .cloned()
            .collect()
    };
    let expected_result = match plan.operation.result.name.as_str() {
        "List" => VerificationValue::List(selected),
        "Option" if selected.len() <= 1 => {
            VerificationValue::Option(selected.into_iter().next().map(Box::new))
        }
        _ if selected.len() == 1 => selected[0].clone(),
        _ => {
            return problem(
                node,
                "STATE_RESULT_CARDINALITY",
                "Direct entity result is missing or ambiguous",
                false,
            );
        }
    };
    if variables.get("result") == Some(&expected_result) {
        VerificationResult::Pass
    } else {
        problem(
            node,
            "STATE_RESULT_VIOLATED",
            "Actual result differs from modeled entity state",
            true,
        )
    }
}
