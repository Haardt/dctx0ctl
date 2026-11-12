//! A returned key may select the modeled changed row, never supply its payload.
//!
//! See `docs/application-test-implementation.md`: only typed result-field keys
//! are supported here; general result-dependent state projections remain open.
use dctx0_core::*;
use std::collections::BTreeMap;

// Check the entire expression before cloning it: a small evaluation budget must
// also bound the work and recursion of the copy preceding the rewrite.
pub(crate) fn projection_within_budget(
    expression: &Expression,
    budgets: &VerificationBudgets,
) -> bool {
    fn visit(e: &Expression, depth: usize, remaining: &mut usize) -> bool {
        if depth == 0 || *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        let next = depth - 1;
        match &e.value {
            ExpressionKind::Field { target, .. } => visit(target, next, remaining),
            ExpressionKind::VariantRecord { payload, .. } => visit(payload, next, remaining),
            ExpressionKind::Record { fields, .. } => {
                fields.iter().all(|(_, e)| visit(e, next, remaining))
            }
            ExpressionKind::List { values } => values.iter().all(|e| visit(e, next, remaining)),
            ExpressionKind::Unary { operand, .. } => visit(operand, next, remaining),
            ExpressionKind::Binary { left, right, .. } => {
                visit(left, next, remaining) && visit(right, next, remaining)
            }
            ExpressionKind::Call { arguments, .. }
            | ExpressionKind::State {
                keys: arguments, ..
            } => arguments.iter().all(|e| visit(e, next, remaining)),
            ExpressionKind::Trace {
                arguments, count, ..
            } => {
                arguments.iter().all(|e| visit(e, next, remaining)) && visit(count, next, remaining)
            }
            ExpressionKind::Collection {
                collection,
                predicate,
                projection,
                ..
            } => {
                visit(collection, next, remaining)
                    && predicate.as_ref().is_none_or(|e| visit(e, next, remaining))
                    && projection
                        .as_ref()
                        .is_none_or(|e| visit(e, next, remaining))
            }
            ExpressionKind::Literal { .. }
            | ExpressionKind::Variable { .. }
            | ExpressionKind::Variant { .. } => true,
        }
    }
    let mut remaining = budgets.evaluation_steps;
    visit(expression, budgets.recursion_depth, &mut remaining)
}

pub(crate) fn bind_changed_row_keys(
    expression: &mut Expression,
    entity: &str,
    keys: &[VerificationValue],
    result: &VerificationValue,
    variables: &mut BTreeMap<String, VerificationValue>,
    budgets: &VerificationBudgets,
) -> bool {
    let mut rewrite = Rewrite {
        entity,
        keys,
        result,
        variables,
        remaining: budgets.evaluation_steps,
        serial: 0,
    };
    rewrite.visit(expression, budgets.recursion_depth, true)
}

struct Rewrite<'a> {
    entity: &'a str,
    keys: &'a [VerificationValue],
    result: &'a VerificationValue,
    variables: &'a mut BTreeMap<String, VerificationValue>,
    remaining: usize,
    serial: usize,
}

impl Rewrite<'_> {
    fn tick(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }

    fn result_key(
        &mut self,
        expression: &Expression,
        mut depth: usize,
    ) -> Option<Option<VerificationValue>> {
        let mut expression = expression;
        let mut path = Vec::new();
        loop {
            if depth == 0 || !self.tick() {
                return None;
            }
            depth -= 1;
            match &expression.value {
                ExpressionKind::Variable { name } if name == "result" => break,
                ExpressionKind::Field { target, name } => {
                    path.push(name);
                    expression = target;
                }
                _ => return Some(None),
            }
        }
        let mut value = self.result;
        for name in path.into_iter().rev() {
            let VerificationValue::Record(fields) = value else {
                return Some(None);
            };
            let Some(next) = fields.get(name) else {
                return Some(None);
            };
            value = next;
        }
        Some(Some(value.clone()))
    }

    fn visit(&mut self, expression: &mut Expression, depth: usize, result_visible: bool) -> bool {
        if depth == 0 || !self.tick() {
            return false;
        }
        if let ExpressionKind::State {
            phase: StatePhase::After,
            entity,
            keys,
        } = &mut expression.value
        {
            if result_visible && entity == self.entity && keys.len() == self.keys.len() {
                for (expression, expected) in keys.iter_mut().zip(self.keys) {
                    let Some(value) = self.result_key(expression, depth - 1) else {
                        return false;
                    };
                    if let Some(value) = value {
                        if &value != expected {
                            return false;
                        }
                        // NUL cannot be a source identifier. All other result uses
                        // remain unbound when the independent projection evaluates.
                        let name = format!("\0state-key-{}", self.serial);
                        self.serial += 1;
                        self.variables.insert(name.clone(), value);
                        expression.value = ExpressionKind::Variable { name };
                    }
                }
            }
        }
        let next = depth - 1;
        match &mut expression.value {
            ExpressionKind::Field { target, .. } => self.visit(target, next, result_visible),
            ExpressionKind::VariantRecord { payload, .. } => {
                self.visit(payload, next, result_visible)
            }
            ExpressionKind::Record { fields, .. } => fields
                .iter_mut()
                .all(|(_, e)| self.visit(e, next, result_visible)),
            ExpressionKind::List { values } => values
                .iter_mut()
                .all(|e| self.visit(e, next, result_visible)),
            ExpressionKind::Unary { operand, .. } => self.visit(operand, next, result_visible),
            ExpressionKind::Binary { left, right, .. } => {
                self.visit(left, next, result_visible) && self.visit(right, next, result_visible)
            }
            ExpressionKind::Call { arguments, .. } => arguments
                .iter_mut()
                .all(|e| self.visit(e, next, result_visible)),
            ExpressionKind::Trace {
                arguments, count, ..
            } => {
                arguments
                    .iter_mut()
                    .all(|e| self.visit(e, next, result_visible))
                    && self.visit(count, next, result_visible)
            }
            ExpressionKind::Collection {
                binder,
                collection,
                predicate,
                projection,
                ..
            } => {
                let visible = result_visible && binder.as_deref() != Some("result");
                self.visit(collection, next, result_visible)
                    && predicate
                        .as_mut()
                        .is_none_or(|e| self.visit(e, next, visible))
                    && projection
                        .as_mut()
                        .is_none_or(|e| self.visit(e, next, visible))
            }
            ExpressionKind::State { keys, .. } => {
                keys.iter_mut().all(|e| self.visit(e, next, result_visible))
            }
            ExpressionKind::Literal { .. }
            | ExpressionKind::Variable { .. }
            | ExpressionKind::Variant { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expr(value: ExpressionKind) -> Expression {
        let pos = Position {
            offset: 0,
            line: 1,
            column: 1,
        };
        Expression {
            ty: TypeRef::builtin("Int"),
            span: Span {
                file: "budget".into(),
                start: pos,
                end: pos,
            },
            value,
        }
    }

    #[test]
    fn preflight_bounds_deep_and_wide_rhs_before_copy() {
        let leaf = || expr(ExpressionKind::Variable { name: "x".into() });
        let mut deep = leaf();
        for _ in 0..2048 {
            deep = expr(ExpressionKind::Field {
                target: Box::new(deep),
                name: "x".into(),
            });
        }
        let wide = expr(ExpressionKind::List {
            values: (0..10000).map(|_| leaf()).collect(),
        });
        let depth_budget = VerificationBudgets {
            recursion_depth: 16,
            ..Default::default()
        };
        let step_budget = VerificationBudgets {
            evaluation_steps: 16,
            ..Default::default()
        };
        assert!(!projection_within_budget(&deep, &depth_budget));
        assert!(!projection_within_budget(&wide, &step_budget));
        assert!(projection_within_budget(&leaf(), &step_budget));
    }
}
