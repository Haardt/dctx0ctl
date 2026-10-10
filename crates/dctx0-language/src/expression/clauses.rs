//! Clause dispatch preserves workspace order and shared example bindings.
use super::*;
mod conditions;
mod context;
mod events;
mod examples;
mod state;

pub(crate) fn compile_clauses(ir: &mut WorkspaceIr, diagnostics: &mut Vec<Diagnostic>) {
    let mut refs = Vec::new();
    let mut results = BTreeMap::<String, Vec<Expression>>::new();
    let mut sample_variables = BTreeMap::<String, BTreeMap<String, TypeRef>>::new();
    let resolver = SymbolResolver::new(&ir.symbols);
    let mut owned = BTreeMap::<&str, Vec<&SemanticNode>>::new();
    for node in &ir.nodes {
        owned.entry(&node.owner).or_default().push(node);
    }
    for node in &ir.nodes {
        if !matches!(
            node.kind,
            StatementKind::Requires
                | StatementKind::Ensures
                | StatementKind::Where
                | StatementKind::Assertion
                | StatementKind::Given
                | StatementKind::When
                | StatementKind::Expect
                | StatementKind::Fails
                | StatementKind::Emits
                | StatementKind::Emit
                | StatementKind::State
                | StatementKind::Transition
        ) {
            continue;
        }
        let mut variables =
            context::variables_for_clause(node, ir, &resolver, &owned, &mut refs, diagnostics);
        let samples = ir
            .symbols
            .get(&node.owner)
            .is_some_and(|s| s.kind == StatementKind::Example);
        if samples {
            variables.extend(
                sample_variables
                    .get(&node.owner)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        let mut context = ExpressionContext {
            ir,
            resolver: &resolver,
            owner: &node.owner,
            variables,
            samples,
            allow_after: node.kind != StatementKind::Requires,
            refs: &mut refs,
            diagnostics,
        };
        let mut expressions = Vec::new();
        match node.kind {
            StatementKind::Requires
            | StatementKind::Ensures
            | StatementKind::Where
            | StatementKind::Assertion
                if !node.terms.is_empty() =>
            {
                conditions::compile_condition(node, &mut context, &mut expressions)
            }
            StatementKind::Given | StatementKind::When => {
                examples::compile_example_step(node, &mut context, &mut expressions)
            }
            StatementKind::Expect | StatementKind::Fails | StatementKind::Emits => {
                examples::compile_expectation(node, &mut context, &mut expressions)
            }
            StatementKind::Emit => events::compile_emission(node, &mut context, &mut expressions),
            StatementKind::State if ir.operations.contains_key(&node.owner) => {
                state::compile_state_selector(node, &mut context, &mut expressions)
            }
            StatementKind::Transition => {
                conditions::compile_transition_guard(node, &mut context, &mut expressions)
            }
            _ => {}
        }

        if samples {
            sample_variables.insert(node.owner.clone(), context.variables);
        }
        results.insert(node.id.clone(), expressions);
    }
    for node in &mut ir.nodes {
        node.expressions = results.remove(&node.id).unwrap_or_default();
    }
    ir.references.extend(refs);
}
