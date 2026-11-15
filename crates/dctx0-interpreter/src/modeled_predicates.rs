//! Deterministic predicate models for value refinements, never native call evidence.
use super::*;
use dctx0_core::StatementKind;

pub(crate) fn evaluate_refinement(
    ir: &WorkspaceIr,
    expression: &Expression,
    variables: &BTreeMap<String, V>,
    budgets: &VerificationBudgets,
) -> Evaluation {
    let context = ObservationContext::default();
    let mut evaluator = Evaluator {
        ir,
        observations: &[],
        budgets,
        cursor: 0,
        steps: 0,
        context: &context,
        modeled_predicates: true,
    };
    match evaluator.eval(expression, variables, 0) {
        Ok(value) => Evaluation::Complete(value),
        Err(outcome) => outcome,
    }
}

impl Evaluator<'_> {
    pub(super) fn modeled_predicate(
        &mut self,
        expression: &Expression,
        subject: &str,
        arguments: Vec<V>,
        depth: usize,
    ) -> EvalResult {
        let operation = &self.ir.operations[subject];
        let unavailable = || {
            self.issue(
            expression,
            "UNSUPPORTED_REFINEMENT",
            "Predicate requires a pure Boolean result uniquely defined by an equality postcondition",
        )
        };
        if !operation.failures.is_empty()
            || operation.result != dctx0_core::TypeRef::builtin("Boolean")
            || arguments.len() != operation.parameters.len()
        {
            return Err(unavailable());
        }
        // Refined/composite parameter checks could recursively re-enter this
        // model with a fresh checker budget. Keep those explicitly unavailable.
        for (field, value) in operation.parameters.iter().zip(&arguments) {
            if field.ty.symbol_id.is_some() || !field.ty.arguments.is_empty() {
                return Err(unavailable());
            }
            match check_value(self.ir, &field.ty, value, self.budgets) {
                ValueContract::Accepted => {}
                ValueContract::Rejected { .. } => {
                    return Err(self.issue(
                        expression,
                        "MODELED_ARGUMENT_TYPE",
                        "Predicate argument violates its declared type",
                    ));
                }
                ValueContract::Unknown { issue } => return Err(Evaluation::Unknown(issue)),
            }
        }
        let mut variables: BTreeMap<_, _> = operation
            .parameters
            .iter()
            .zip(arguments)
            .map(|(field, value)| (field.name.clone(), value))
            .collect();
        let clauses: Vec<_> = self
            .ir
            .nodes
            .iter()
            .filter(|node| node.owner == subject)
            .filter(|node| matches!(node.kind, StatementKind::Requires | StatementKind::Ensures))
            .collect();
        let definition = clauses
            .iter()
            .filter(|node| node.kind == StatementKind::Ensures)
            .flat_map(|node| &node.expressions)
            .find_map(result_definition)
            .ok_or_else(unavailable)?;
        for clause in clauses
            .iter()
            .filter(|node| node.kind == StatementKind::Requires)
        {
            for requirement in &clause.expressions {
                if self.eval(requirement, &variables, depth)? != V::Boolean(true) {
                    return Err(self.issue(
                        expression,
                        "PREDICATE_PRECONDITION",
                        "Predicate model precondition is not satisfied",
                    ));
                }
            }
        }
        let result = self.eval(definition, &variables, depth)?;
        if !matches!(result, V::Boolean(_)) {
            return Err(self.bad(expression, "Predicate model must return Boolean"));
        }
        variables.insert("result".into(), result.clone());
        for clause in clauses
            .iter()
            .filter(|node| node.kind == StatementKind::Ensures)
        {
            for postcondition in &clause.expressions {
                if self.eval(postcondition, &variables, depth)? != V::Boolean(true) {
                    return Err(self.issue(
                        expression,
                        "INCONSISTENT_PREDICATE_MODEL",
                        "Predicate model does not satisfy every postcondition",
                    ));
                }
            }
        }
        Ok(result)
    }
}

fn result_definition(expression: &Expression) -> Option<&Expression> {
    if let E::Binary {
        operator,
        left,
        right,
    } = &expression.value
        && operator == "=="
    {
        if matches!(&left.value, E::Variable { name } if name == "result") {
            return Some(right);
        }
        if matches!(&right.value, E::Variable { name } if name == "result") {
            return Some(left);
        }
    }
    None
}
