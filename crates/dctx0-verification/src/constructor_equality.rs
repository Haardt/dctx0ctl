//! Shared modeled constructor equality. This does not rewrite evidence values.
use dctx0_core::{
    Expression, ExpressionKind, TypeRef, VerificationBudgets, VerificationResult,
    VerificationValue, WorkspaceIr,
};
use dctx0_interpreter::{Evaluation, evaluate};
use std::collections::BTreeMap;

/// Compare the actual value to the selected input using the interpreter's
/// existing equality rules and budgets; preserve both original wire values.
pub fn constructor_value_equality(
    ir: &WorkspaceIr,
    subject: &str,
    ty: &TypeRef,
    expected: &VerificationValue,
    actual: &VerificationValue,
    budgets: &VerificationBudgets,
) -> VerificationResult {
    let Some(symbol) = ir.symbols.get(subject) else {
        return crate::unknown(
            "APPLICATION_IR_SCOPE",
            "Constructor subject has no declared source",
        );
    };
    let variable = |name: &str| Expression {
        ty: ty.clone(),
        span: symbol.span.clone(),
        value: ExpressionKind::Variable { name: name.into() },
    };
    let expression = Expression {
        ty: TypeRef {
            name: "Boolean".into(),
            symbol_id: None,
            arguments: vec![],
        },
        span: symbol.span.clone(),
        value: ExpressionKind::Binary {
            operator: "==".into(),
            left: Box::new(variable("expected")),
            right: Box::new(variable("actual")),
        },
    };
    let variables = BTreeMap::from([
        ("expected".into(), expected.clone()),
        ("actual".into(), actual.clone()),
    ]);
    match evaluate(ir, &expression, &variables, &[], budgets) {
        Evaluation::Complete(VerificationValue::Boolean(true)) => VerificationResult::Pass,
        Evaluation::Complete(VerificationValue::Boolean(false)) => VerificationResult::Fail {
            issue: dctx0_core::VerificationIssue {
                code: "NATIVE_VALUE_CHANGED".into(),
                message: "Production changed the modeled input value".into(),
                span: None,
            },
        },
        Evaluation::Unknown(issue) => VerificationResult::Unknown { issue },
        _ => crate::unknown(
            "APPLICATION_VALUE_EQUALITY",
            "Input equality could not be evaluated",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dctx0_core::{Source, VerificationValue as V};

    #[test]
    fn preserves_modeled_numeric_and_recursive_equality_without_changing_values() {
        let ir = dctx0_language::compile(&[Source {
            file: "equality.dctx0".into(),
            text: "workspace W { value X = Float }".into(),
        }])
        .ir
        .unwrap();
        let ty = TypeRef {
            name: "Float".into(),
            symbol_id: None,
            arguments: vec![],
        };
        let float = |s: &str| V::Float(s.into());
        let nested = |v: V| {
            V::Record(BTreeMap::from([(
                "value".into(),
                V::Option(Some(Box::new(v))),
            )]))
        };
        let variant = |name: &str, v: V| V::Variant {
            declaration: "W.Kind".into(),
            name: name.into(),
            payload: Some(Box::new(v)),
        };
        for (left, right, equal) in [
            (float("1e308"), float("1e+308"), true),
            (float("1000000000000000000000"), float("1e21"), true),
            (float("0"), float("-0.0"), true),
            (float("5e-324"), float("4.9406564584124654e-324"), true),
            (float("1.0"), float("1.0000000000000002"), false),
            (nested(float("1e308")), nested(float("1e+308")), true),
            (
                nested(float("1.0")),
                nested(float("1.0000000000000002")),
                false,
            ),
            (
                V::Option(None),
                V::Option(Some(Box::new(float("0")))),
                false,
            ),
            (
                variant("A", float("1e308")),
                variant("A", float("1e+308")),
                true,
            ),
            (variant("A", float("1")), variant("B", float("1")), false),
            (
                V::Integer("9007199254740992".into()),
                V::Integer("9007199254740993".into()),
                false,
            ),
            (
                V::Integer("9007199254740993".into()),
                float("9007199254740992"),
                false,
            ),
            (
                V::Decimal("0.123456789012345678901234567890".into()),
                V::Decimal("0.123456789012345678901234567891".into()),
                false,
            ),
            (V::Decimal("1.00".into()), V::Decimal("1.0".into()), true),
            (V::String("1e308".into()), V::String("1e+308".into()), false),
        ] {
            let before = (left.clone(), right.clone());
            let actual = constructor_value_equality(
                &ir,
                "W.X",
                &ty,
                &left,
                &right,
                &VerificationBudgets::default(),
            );
            assert_eq!(
                matches!(actual, VerificationResult::Pass),
                equal,
                "{left:?} vs {right:?}: {actual:?}"
            );
            assert_eq!((left, right), before, "wire text must not be rewritten");
        }
    }
}
