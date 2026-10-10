use super::*;

pub(crate) fn expand(ty: &TypeRef, ir: &WorkspaceIr) -> TypeRef {
    let mut result = ty.clone();
    // Alias cycles are rejected before expression compilation.
    while let Some(id) = &result.symbol_id {
        if let Some(alias) = ir.data.get(id).and_then(|d| d.alias.as_ref()) {
            result = alias.clone();
        } else {
            break;
        }
    }
    result
}
pub(super) fn compatible(expected: &TypeRef, actual: &TypeRef, ir: &WorkspaceIr) -> bool {
    let a = expand(expected, ir);
    let b = expand(actual, ir);
    let name = if a.symbol_id.is_some() || b.symbol_id.is_some() {
        a.symbol_id == b.symbol_id
    } else {
        a.name == b.name
            || matches!(
                (a.name.as_str(), b.name.as_str()),
                ("Float", "Int" | "Decimal")
                    | ("Decimal", "Int")
                    | ("String", "UUID" | "Timestamp" | "Duration")
                    | ("UUID" | "Timestamp" | "Duration", "String")
            )
    };
    name && (a.arguments.len() == b.arguments.len()
        && a.arguments
            .iter()
            .zip(&b.arguments)
            .all(|(a, b)| compatible(a, b, ir))
        || a.name == "Option" && b.arguments.is_empty()
        || a.name == "List" && b.arguments.first().is_some_and(|t| t.name == "Unit"))
}
// Inspect tree depth iteratively: left-associative chains otherwise bypass the
// Pratt recursion limit and can overflow later serialization/drop recursion.
pub(super) fn expression_depth(expression: &Expression) -> usize {
    let mut stack = vec![(expression, 1usize)];
    let mut maximum = 0;
    while let Some((expression, depth)) = stack.pop() {
        maximum = maximum.max(depth);
        if maximum > 128 {
            return maximum;
        }
        let next = depth + 1;
        match &expression.value {
            ExpressionKind::Literal { .. }
            | ExpressionKind::Variable { .. }
            | ExpressionKind::Variant { .. } => {}
            ExpressionKind::Field { target, .. } => stack.push((target, next)),
            ExpressionKind::Unary { operand, .. } => stack.push((operand, next)),
            ExpressionKind::VariantRecord { payload, .. } => stack.push((payload, next)),
            ExpressionKind::Binary { left, right, .. } => {
                stack.push((left, next));
                stack.push((right, next));
            }
            ExpressionKind::Record { fields, .. } => {
                for (_, value) in fields {
                    stack.push((value, next));
                }
            }
            ExpressionKind::List { values } => {
                for value in values {
                    stack.push((value, next));
                }
            }
            ExpressionKind::Call { arguments, .. }
            | ExpressionKind::State {
                keys: arguments, ..
            } => {
                for value in arguments {
                    stack.push((value, next));
                }
            }
            ExpressionKind::Trace {
                arguments, count, ..
            } => {
                stack.push((count, next));
                for value in arguments {
                    stack.push((value, next));
                }
            }
            ExpressionKind::Collection {
                collection,
                predicate,
                projection,
                ..
            } => {
                stack.push((collection, next));
                if let Some(value) = predicate {
                    stack.push((value, next));
                }
                if let Some(value) = projection {
                    stack.push((value, next));
                }
            }
        }
    }
    maximum
}
