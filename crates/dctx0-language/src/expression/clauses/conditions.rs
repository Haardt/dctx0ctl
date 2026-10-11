//! Compile Boolean conditions and lifecycle guards.
use super::*;

pub(super) fn compile_condition<'a>(
    node: &'a SemanticNode,
    context: &mut ExpressionContext<'a>,
    expressions: &mut Vec<Expression>,
) {
    let ir = context.ir;

    if let Some(value) = parse_expression(&node.terms, context, None) {
        if expand(&value.ty, ir).name != "Boolean" {
            problem(
                context.diagnostics,
                "DCTX0_EXPRESSION_TYPE",
                "Condition must be Boolean",
                &node.span,
            );
        }
        expressions.push(value);
    }
}
pub(super) fn compile_transition_guard<'a>(
    node: &'a SemanticNode,
    context: &mut ExpressionContext<'a>,
    expressions: &mut Vec<Expression>,
) {
    if let Some(i) = top_level(&node.terms, 0, "when") {
        if let Some(value) = parse_expression(
            &node.terms[i + 1..],
            context,
            Some(&TypeRef::builtin("Boolean")),
        ) {
            if expand(&value.ty, context.ir).name != "Boolean" {
                problem(
                    context.diagnostics,
                    "DCTX0_EXPRESSION_TYPE",
                    "Transition guard must be Boolean",
                    &node.span,
                );
            }
            expressions.push(value);
        }
    }
}
