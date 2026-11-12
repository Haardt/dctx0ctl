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
pub(crate) fn marker(node: &SemanticNode, name: &str) -> bool {
    let mut depth = 0usize;
    for term in node.terms.iter().skip(2) {
        if depth == 0 && term.text == name {
            return true;
        }
        match term.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
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
fn unalias<'a>(
    ir: &'a WorkspaceIr,
    mut ty: &'a TypeRef,
    budgets: &VerificationBudgets,
) -> Option<&'a TypeRef> {
    for _ in 0..=budgets.recursion_depth {
        match ty
            .symbol_id
            .as_ref()
            .and_then(|id| ir.data.get(id))
            .and_then(|data| data.alias.as_ref())
        {
            Some(alias) => ty = alias,
            None => return Some(ty),
        }
    }
    None
}
fn explicit_projection<'a>(
    plan: &'a OperationPlan,
    budgets: &VerificationBudgets,
) -> Option<&'a Expression> {
    let mut values = Vec::new();
    let mut remaining = budgets.evaluation_steps;
    for expression in plan
        .clauses
        .iter()
        .filter(|node| node.kind == StatementKind::Ensures)
        .flat_map(|node| &node.expressions)
    {
        let expression = match &expression.value {
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } if operator == "implies"
                && matches!(&left.value,ExpressionKind::Variable{name} if name=="success") =>
            {
                right.as_ref()
            }
            _ => expression,
        };
        if !collect_projection(
            expression,
            &mut values,
            &mut remaining,
            budgets.recursion_depth,
        ) {
            return None;
        }
    }
    if values.len() == 1 {
        values.pop()
    } else {
        None
    }
}

// A response containing the observed entity still needs its actual collection
// projection. An unrelated receipt field cannot certify those nested rows.
fn contains_entity(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    entity: &str,
    depth: usize,
    remaining: &mut usize,
) -> bool {
    if depth == 0 || *remaining == 0 {
        return true;
    }
    *remaining -= 1;
    if ty.symbol_id.as_deref() == Some(entity) {
        return true;
    }
    if ty
        .arguments
        .iter()
        .any(|argument| contains_entity(ir, argument, entity, depth - 1, remaining))
    {
        return true;
    }
    let Some(data) = ty.symbol_id.as_ref().and_then(|id| ir.data.get(id)) else {
        return false;
    };
    if ir.data.get(entity).is_some_and(|observed| {
        data.variants.is_empty()
            && data.fields.len() == observed.fields.len()
            && observed
                .fields
                .iter()
                .all(|field| data.fields.contains(field))
    }) {
        return true;
    }
    data.alias
        .as_ref()
        .is_some_and(|alias| contains_entity(ir, alias, entity, depth - 1, remaining))
        || data
            .fields
            .iter()
            .any(|field| contains_entity(ir, &field.ty, entity, depth - 1, remaining))
        || data
            .variants
            .iter()
            .filter_map(|variant| variant.payload.as_ref())
            .any(|payload| contains_entity(ir, payload, entity, depth - 1, remaining))
}

// Only conjunctions make their nested equality unconditional. Do not extract
// projections from alternatives or other conditional expressions.
fn collect_projection<'a>(
    expression: &'a Expression,
    values: &mut Vec<&'a Expression>,
    remaining: &mut usize,
    depth: usize,
) -> bool {
    if *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    let ExpressionKind::Binary {
        operator,
        left,
        right,
    } = &expression.value
    else {
        return true;
    };
    match operator.as_str() {
        "and" | "&&" => {
            depth > 0
                && collect_projection(left, values, remaining, depth - 1)
                && collect_projection(right, values, remaining, depth - 1)
        }
        "==" => {
            if matches!(&left.value,ExpressionKind::Variable{name} if name=="result") {
                values.push(right.as_ref());
            } else if matches!(&right.value,ExpressionKind::Variable{name} if name=="result") {
                values.push(left.as_ref());
            }
            true
        }
        _ => true,
    }
}
// An unrelated observed entity is an authority/input to an operation, not its
// return value. Only accept that distinction when an actual result field is
// independently constrained by an explicit, unconditionally evaluated clause.
fn has_independent_result_field_constraint(
    plan: &OperationPlan,
    budgets: &VerificationBudgets,
    selected_entity: Option<&str>,
) -> bool {
    fn result_field(e: &Expression) -> bool {
        matches!(&e.value, ExpressionKind::Field { target, .. }
            if matches!(&target.value, ExpressionKind::Variable { name } if name == "result"))
    }
    fn independent(e: &Expression, plan: &OperationPlan, binder: Option<&str>) -> bool {
        matches!(&e.value, ExpressionKind::Literal { .. })
            || matches!(&e.value, ExpressionKind::Field { target, .. }
                if matches!(&target.value, ExpressionKind::Variable { name }
                    if name != "result" && (binder == Some(name.as_str()) || plan.operation.parameters.iter().any(|parameter| &parameter.name == name))))
    }
    let mut pending: Vec<_> = plan
        .clauses
        .iter()
        .filter(|node| node.kind == StatementKind::Ensures)
        .flat_map(|node| &node.expressions)
        .map(|expression| (expression, 0usize, None))
        .collect();
    let mut remaining = budgets.evaluation_steps;
    while let Some((e, depth, binder)) = pending.pop() {
        if remaining == 0 || depth > budgets.recursion_depth {
            return false;
        }
        remaining -= 1;
        match &e.value {
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } if operator == "implies"
                && matches!(&left.value, ExpressionKind::Variable { name } if name == "success") =>
            {
                pending.push((right, depth + 1, binder))
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } if operator == "and" || operator == "&&" => {
                pending.push((left, depth + 1, binder));
                pending.push((right, depth + 1, binder));
            }
            // The caller supplies an entity only for an actual, uniquely
            // selected row. This excludes vacuous or unrelated guard scopes;
            // the normal postcondition evaluator still checks returned values.
            ExpressionKind::Collection {
                operator,
                binder: Some(name),
                collection,
                predicate: Some(predicate),
                projection: None,
            } if binder.is_none()
                && operator == "all"
                && name != "result"
                && name != "success"
                && !plan
                    .operation
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == *name)
                && matches!(&collection.value, ExpressionKind::State { phase: StatePhase::Before, entity, keys }
                        if Some(entity.as_str()) == selected_entity && keys.is_empty()) =>
            {
                pending.push((predicate, depth + 1, Some(name.as_str())));
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } if operator == "=="
                && ((result_field(left) && independent(right, plan, binder))
                    || (result_field(right) && independent(left, plan, binder))) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
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
                        "Successful insertion reused an observed existing identity",
                        true,
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
            // Only a complete explicit source supplies the replacement values.
            // A selector alone must never be interpreted as an inferred update.
            if !marker(node, "from") || marker(node, "where") || values.len() != data.keys.len() + 1
            {
                return problem(
                    node,
                    "STATE_REPLACEMENT_SOURCE_REQUIRED",
                    "Replacement needs a complete source and identity selector",
                    false,
                );
            }
            let identity = values[..data.keys.len()].to_vec();
            let source = &values[data.keys.len()];
            match check_value(ir, &ty, source, budgets) {
                ValueContract::Accepted => {}
                ValueContract::Unknown { issue } => return VerificationResult::Unknown { issue },
                _ => {
                    return problem(
                        node,
                        "STATE_SOURCE_INCOMPLETE",
                        "Replacement source is not a complete valid entity",
                        false,
                    );
                }
            }
            if key(data, source) != Some(identity.clone()) {
                return problem(
                    node,
                    "STATE_REPLACEMENT_IDENTITY",
                    "Replacement source must preserve the selected identity",
                    false,
                );
            }
            let Some(index) = expected
                .iter()
                .position(|row| key(data, row) == Some(identity.clone()))
            else {
                return problem(
                    node,
                    "STATE_REPLACEMENT_SELECTION_REQUIRED",
                    "Successful replacement selected an observed absent identity",
                    true,
                );
            };
            expected[index] = source.clone();
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
    // A read-only observation still has to preserve its complete actual rows.
    // In a mixed operation the returned payload belongs to the mutating state
    // clause, which checks its independent projection below. Checking that same
    // projection against an unrelated observed entity leaves valid result-key
    // selectors unbound (e.g. authority read + socket upsert).
    if matches!(mode, "observes" | "lists")
        && plan.clauses.iter().any(|clause| {
            clause.kind == StatementKind::State
                && clause.terms.first().is_some_and(|term| {
                    matches!(
                        term.text.as_str(),
                        "inserts" | "upserts" | "replaces" | "removes"
                    )
                })
        })
    {
        return VerificationResult::Pass;
    }
    if let Some(projection) = explicit_projection(plan, budgets) {
        if !crate::state_projection::projection_within_budget(projection, budgets) {
            return problem(
                node,
                "STATE_RESULT_PROJECTION_REQUIRED",
                "Result projection exceeds the evaluation budget",
                false,
            );
        }
        let mut independent = variables.clone();
        independent.remove("result");
        let mut selected_projection = projection.clone();
        let source = match mode {
            "inserts" | "upserts" => values.first(),
            "replaces" => values.last(),
            _ => None,
        };
        if let Some(keys) = source.and_then(|source| key(data, source)) {
            let actual = variables.get("result").unwrap_or(&VerificationValue::Unit);
            if !crate::state_projection::bind_changed_row_keys(
                &mut selected_projection,
                &data.id,
                &keys,
                actual,
                &mut independent,
                budgets,
            ) {
                return problem(
                    node,
                    "STATE_RESULT_PROJECTION_REQUIRED",
                    "Result-key projection requires the exact modeled changed identity within budgets",
                    false,
                );
            }
        }
        return match evaluate_observed(
            ir,
            &selected_projection,
            &independent,
            &[],
            context,
            budgets,
        ) {
            Evaluation::Complete(expected) => {
                let mut left = projection.clone();
                left.value = ExpressionKind::Variable {
                    name: "actual".into(),
                };
                let mut right = projection.clone();
                right.value = ExpressionKind::Variable {
                    name: "expected".into(),
                };
                let comparison = Expression {
                    ty: TypeRef::builtin("Boolean"),
                    span: projection.span.clone(),
                    value: ExpressionKind::Binary {
                        operator: "==".into(),
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                };
                let actual = variables
                    .get("result")
                    .cloned()
                    .unwrap_or(VerificationValue::Unit);
                let bindings =
                    BTreeMap::from([("actual".into(), actual), ("expected".into(), expected)]);
                match evaluate_observed(ir, &comparison, &bindings, &[], context, budgets) {
                    Evaluation::Complete(VerificationValue::Boolean(true)) => {
                        VerificationResult::Pass
                    }
                    Evaluation::Unknown(issue) => VerificationResult::Unknown { issue },
                    _ => problem(
                        node,
                        "STATE_RESULT_VIOLATED",
                        "Actual result differs from the explicit modeled result projection",
                        true,
                    ),
                }
            }
            Evaluation::Unknown(issue) => VerificationResult::Unknown { issue },
            _ => problem(
                node,
                "STATE_RESULT_PROJECTION_REQUIRED",
                "Explicit result projection needs unavailable observations",
                false,
            ),
        };
    }
    let result_shape = unalias(ir, &plan.operation.result, budgets);
    if result_shape.is_some_and(|ty| ty.name == "Unit") {
        return VerificationResult::Pass;
    }
    let result_ty = result_shape
        .and_then(|ty| {
            if matches!(ty.name.as_str(), "Option" | "List") {
                ty.arguments.first()
            } else {
                Some(ty)
            }
        })
        .and_then(|ty| unalias(ir, ty, budgets));
    let equivalent_record = result_ty
        .and_then(|ty| ty.symbol_id.as_ref())
        .and_then(|id| ir.data.get(id))
        .is_some_and(|result| {
            result.variants.is_empty()
                && result.fields.len() == data.fields.len()
                && data.fields.iter().all(|field| {
                    result
                        .fields
                        .iter()
                        .any(|other| other.name == field.name && other.ty == field.ty)
                })
        });
    if result_ty.and_then(|t| t.symbol_id.as_ref()) != Some(&data.id) && !equivalent_record {
        if mode == "observes" && values.len() != data.keys.len() {
            return problem(
                node,
                "STATE_SELECTOR_REQUIRED",
                "Observation needs every key component",
                false,
            );
        }
        // An independently constrained response does not excuse a missing
        // observed key. See docs/application-test-implementation.md for the
        // selected-authority regression and evidence boundaries.
        let selected = before
            .iter()
            .any(|row| key(data, row) == Some(values.clone()));
        let exactly_selected = before.len() == 1 && selected;
        // A collection may be an observed input to an acknowledgement rather
        // than its return value. A direct, nonvacuous response constraint still
        // has to be checked by the ordinary postcondition evaluator. Unlike a
        // keyed observation, a genuinely observed empty collection is valid.
        let mut remaining = budgets.evaluation_steps;
        if (mode == "lists" || mode == "observes" && selected)
            && !contains_entity(
                ir,
                &plan.operation.result,
                &data.id,
                budgets.recursion_depth,
                &mut remaining,
            )
            && has_independent_result_field_constraint(
                plan,
                budgets,
                (mode == "observes" && exactly_selected).then_some(data.id.as_str()),
            )
        {
            return VerificationResult::Pass;
        }
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
        let Some(source) = (if mode == "replaces" {
            values.last()
        } else {
            values.first()
        }) else {
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
    let expected_result = match result_shape.map(|ty| ty.name.as_str()) {
        Some("List") => VerificationValue::List(selected),
        Some("Option") if selected.len() <= 1 => {
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
