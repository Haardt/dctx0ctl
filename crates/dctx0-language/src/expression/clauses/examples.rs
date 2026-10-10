//! Compile example invocations, stub outcomes and expectations.
use super::*;

/// Named components prevent accidental dependence on expression-vector offsets.
struct ExampleStep {
    invocation: Expression,
    outcome: Option<Expression>,
}
impl ExampleStep {
    fn append_to(self, expressions: &mut Vec<Expression>) {
        // Preserve the established IR order: stub outcome precedes invocation.
        expressions.extend(self.outcome);
        expressions.push(self.invocation);
    }
}

pub(super) fn compile_example_step<'a>(
    node: &'a SemanticNode,
    context: &mut ExpressionContext<'a>,
    expressions: &mut Vec<Expression>,
) {
    let ir = context.ir;

    let terms =
        if node.kind == StatementKind::When && node.terms.get(1).is_some_and(|t| t.text == "=") {
            &node.terms[2..]
        } else {
            &node.terms
        };
    let outcome = top_level(terms, 0, "returns").or_else(|| top_level(terms, 0, "fails"));
    let invocation = &terms[..outcome.unwrap_or(terms.len())];
    if let Some(value) = parse_expression(invocation, context, None) {
        if node.kind == StatementKind::When {
            let name = node.terms.first().map_or("result", |t| t.text.as_str());
            context.variables.insert(name.into(), value.ty.clone());
        }
        let mut parsed_outcome = None;
        if let Some(i) = outcome {
            let expected = if terms[i].text == "returns" {
                Some(value.ty.clone())
            } else {
                None
            };
            if let Some(result) = parse_expression(&terms[i + 1..], context, expected.as_ref()) {
                if let Some(ty) = expected {
                    if !compatible(&ty, &result.ty, ir) {
                        problem(
                            context.diagnostics,
                            "DCTX0_EXPRESSION_TYPE",
                            "Stub result has wrong type",
                            &node.span,
                        );
                    }
                }
                parsed_outcome = Some(result);
            }
        }
        ExampleStep {
            invocation: value,
            outcome: parsed_outcome,
        }
        .append_to(expressions);
    }
}
pub(super) fn compile_expectation<'a>(
    node: &'a SemanticNode,
    context: &mut ExpressionContext<'a>,
    expressions: &mut Vec<Expression>,
) {
    let terms = &node.terms;
    if terms
        .first()
        .is_some_and(|t| matches!(t.text.as_str(), "Success" | "Failure"))
    {
        if terms.len() != 1 {
            problem(
                context.diagnostics,
                "DCTX0_INVALID_EXPECTATION",
                "Unexpected outcome tokens",
                &node.span,
            );
        }
    } else if let Some(index) = terms
        .iter()
        .position(|t| matches!(t.text.as_str(), "fails" | "emits"))
    {
        let rest = &terms[index + 1..];
        if rest.is_empty() {
            problem(
                context.diagnostics,
                "DCTX0_REFERENCE_REQUIRED",
                "Expected a failure or event",
                &node.span,
            );
        }
        if let Some(token) = rest.first() {
            let kind = if terms[index].text == "emits" {
                StatementKind::Event
            } else {
                StatementKind::Failure
            };
            reference(
                &token.text,
                &token.span,
                &node.owner,
                if kind == StatementKind::Event {
                    ReferenceKind::Emits
                } else {
                    ReferenceKind::FailsWith
                },
                context.resolver,
                |k| k == kind,
                (context.refs, context.diagnostics),
            );
            if rest.len() > 1 {
                if let Some(value) = parse_expression(rest, context, None) {
                    expressions.push(value);
                }
            }
        }
    } else {
        if let Some(value) = parse_expression(terms, context, None) {
            expressions.push(value);
        }
    }
}
