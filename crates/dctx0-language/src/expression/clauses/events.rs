//! Resolve emitted events and validate argument types.
use super::*;

pub(super) fn compile_emission<'a>(
    node: &'a SemanticNode,
    context: &mut ExpressionContext<'a>,
    expressions: &mut Vec<Expression>,
) {
    let ir = context.ir;

    if let Some(Syntax::Effect {
        target,
        payload: Some(payload),
        ..
    }) = &node.syntax
    {
        let id = context
            .resolver
            .resolve_symbol(target, &node.owner, |k| k == StatementKind::Event)
            .expect("emit target was resolved before expressions");
        match payload {
            EffectPayload::Scalar { expression } => {
                let declaration = &ir.data[&id];
                let ty = declaration.alias.as_ref();
                if !(expression.is_empty() && ty.is_none() && declaration.fields.is_empty()) {
                    if let Some(value) = parse_expression(expression, context, ty) {
                        if let Some(ty) = ty {
                            if !compatible(ty, &value.ty, ir) {
                                problem(
                                    context.diagnostics,
                                    "DCTX0_EXPRESSION_TYPE",
                                    "Event payload has wrong type",
                                    &node.span,
                                );
                            }
                        } else {
                            problem(
                                context.diagnostics,
                                "DCTX0_EXPRESSION_TYPE",
                                "Record event requires named fields",
                                &node.span,
                            );
                        }
                        expressions.push(value);
                    }
                }
            }
            EffectPayload::Record { fields } => {
                let declared = &ir.data[&id].fields;
                for (name, terms) in fields {
                    let expected = declared.iter().find(|f| f.name == *name).map(|f| &f.ty);
                    if expected.is_none() {
                        problem(
                            context.diagnostics,
                            "DCTX0_UNKNOWN_FIELD",
                            format!("Unknown event field {name}"),
                            &node.span,
                        );
                    }
                    if let Some(value) = parse_expression(terms, context, expected) {
                        if let Some(ty) = expected {
                            if !compatible(ty, &value.ty, ir) {
                                problem(
                                    context.diagnostics,
                                    "DCTX0_EXPRESSION_TYPE",
                                    "Event field has wrong type",
                                    &node.span,
                                );
                            }
                        }
                        expressions.push(value);
                    }
                }
                for field in declared {
                    if !fields.iter().any(|(n, _)| *n == field.name) {
                        problem(
                            context.diagnostics,
                            "DCTX0_RECORD_FIELD_MISSING",
                            format!("Missing event field {}", field.name),
                            &node.span,
                        );
                    }
                }
            }
        }
    }
}
