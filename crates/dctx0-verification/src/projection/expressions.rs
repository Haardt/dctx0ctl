use dctx0_core::{Expression, ExpressionKind as E, FieldIr, TypeRef, VerificationIssue};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn variables(
    expressions: &[Expression],
    excluded: &BTreeSet<String>,
) -> Result<Vec<FieldIr>, Box<VerificationIssue>> {
    let mut values = BTreeMap::<String, TypeRef>::new();
    let mut conflict = None;
    for expression in expressions {
        walk(expression, excluded, &mut |e, bound| {
            if let E::Variable { name } = &e.value {
                if !bound.contains(name) {
                    if values.get(name).is_some_and(|previous| previous != &e.ty) {
                        conflict = Some(VerificationIssue {
                            code: "AMBIGUOUS_BINDING_TYPE".into(),
                            message: format!("Symbolic value {name} has conflicting types"),
                            span: Some(e.span.clone()),
                        });
                    } else {
                        values.insert(name.clone(), e.ty.clone());
                    }
                }
            }
        });
    }
    if let Some(issue) = conflict {
        return Err(Box::new(issue));
    }
    Ok(values
        .into_iter()
        .map(|(name, ty)| FieldIr { name, ty })
        .collect())
}

pub(super) fn walk(
    e: &Expression,
    bound: &BTreeSet<String>,
    visit: &mut impl FnMut(&Expression, &BTreeSet<String>),
) {
    visit(e, bound);
    match &e.value {
        E::Field { target, .. } => walk(target, bound, visit),
        E::VariantRecord { payload, .. } => walk(payload, bound, visit),
        E::Record { fields, .. } => {
            for (_, e) in fields {
                walk(e, bound, visit);
            }
        }
        E::List { values } => {
            for e in values {
                walk(e, bound, visit);
            }
        }
        E::Unary { operand, .. } => walk(operand, bound, visit),
        E::Binary { left, right, .. } => {
            walk(left, bound, visit);
            walk(right, bound, visit);
        }
        E::Call { arguments, .. }
        | E::State {
            keys: arguments, ..
        } => {
            for e in arguments {
                walk(e, bound, visit);
            }
        }
        E::Trace {
            arguments, count, ..
        } => {
            for e in arguments {
                walk(e, bound, visit);
            }
            walk(count, bound, visit);
        }
        E::Collection {
            binder,
            collection,
            predicate,
            projection,
            ..
        } => {
            walk(collection, bound, visit);
            let mut context = bound.clone();
            if let Some(name) = binder {
                context.insert(name.clone());
            }
            for e in [predicate, projection].into_iter().flatten() {
                walk(e, &context, visit);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dctx0_core::{Position, Span, StatePhase};
    fn expr(value: E) -> Expression {
        let pos = Position {
            offset: 0,
            line: 1,
            column: 1,
        };
        Expression {
            ty: TypeRef::builtin("Int"),
            span: Span {
                file: "test".into(),
                start: pos,
                end: pos,
            },
            value,
        }
    }
    fn variable(name: &str) -> Expression {
        expr(E::Variable { name: name.into() })
    }
    #[test]
    fn free_variables_include_payloads_state_keys_and_counts_but_exclude_collection_binders() {
        let leaf = variable("input");
        let expressions = vec![
            expr(E::Field {
                target: Box::new(leaf.clone()),
                name: "x".into(),
            }),
            expr(E::VariantRecord {
                declaration: "Result".into(),
                variant: "Found".into(),
                payload: Box::new(leaf.clone()),
            }),
            expr(E::Record {
                declaration: "R".into(),
                fields: vec![("x".into(), leaf.clone())],
            }),
            expr(E::List {
                values: vec![leaf.clone()],
            }),
            expr(E::Unary {
                operator: "-".into(),
                operand: Box::new(leaf.clone()),
            }),
            expr(E::Binary {
                operator: "+".into(),
                left: Box::new(leaf.clone()),
                right: Box::new(leaf.clone()),
            }),
            expr(E::Call {
                name: "f".into(),
                target: Some("f".into()),
                arguments: vec![leaf.clone()],
            }),
            expr(E::State {
                phase: StatePhase::Before,
                entity: "R".into(),
                keys: vec![leaf.clone()],
            }),
            expr(E::Trace {
                trace_kind: "calls".into(),
                target: "f".into(),
                modeled_target: Some("f".into()),
                arguments: vec![leaf.clone()],
                count: Box::new(variable("count")),
            }),
            expr(E::Collection {
                operator: "all".into(),
                binder: Some("item".into()),
                collection: Box::new(variable("items")),
                predicate: Some(Box::new(variable("item"))),
                projection: Some(Box::new(variable("outside"))),
            }),
            expr(E::Literal {
                text: "1".into(),
                literal_kind: "Int".into(),
            }),
        ];
        let result = variables(&expressions, &BTreeSet::new()).unwrap();
        assert_eq!(
            result.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            vec!["count", "input", "items", "outside"]
        );
        assert!(
            variables(&[leaf], &BTreeSet::from(["input".into()]))
                .unwrap()
                .is_empty()
        );
        let mut conflicting = variable("input");
        conflicting.ty = TypeRef::builtin("String");
        assert_eq!(
            variables(&[variable("input"), conflicting], &BTreeSet::new())
                .unwrap_err()
                .code,
            "AMBIGUOUS_BINDING_TYPE"
        );
    }
}
