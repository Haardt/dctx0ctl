//! Structural counterexamples with valid elements expose collection refinements.
use super::*;

/// Mutate one individually valid field in an otherwise valid record. A final
/// outer-contract rejection isolates cross-field invariants, not wrong shapes.
pub(super) fn refined_records(
    ir: &WorkspaceIr,
    data: &dctx0_core::DataIr,
    budgets: &VerificationBudgets,
) -> Vec<V> {
    let ty = TypeRef {
        name: data.id.clone(),
        symbol_id: Some(data.id.clone()),
        arguments: vec![],
    };
    let Some(V::Record(first)) = generate_values(ir, &ty, budgets).values.into_iter().next() else {
        return vec![];
    };
    let mut shape = data;
    for _ in 0..=budgets.recursion_depth {
        let Some(alias) = &shape.alias else {
            break;
        };
        let Some(next) = alias.symbol_id.as_ref().and_then(|id| ir.data.get(id)) else {
            return vec![];
        };
        shape = next;
    }
    let mut candidates = Vec::new();
    for field in shape.fields.iter().take(16) {
        for value in generate_values(ir, &field.ty, budgets)
            .values
            .into_iter()
            .filter(|value| first.get(&field.name) != Some(value))
            .take(3)
        {
            let mut fields = first.clone();
            fields.insert(field.name.clone(), value);
            candidates.push(V::Record(fields));
        }
    }
    candidates
}

pub(super) fn refined_lists(
    ir: &WorkspaceIr,
    data: &dctx0_core::DataIr,
    budgets: &VerificationBudgets,
) -> Vec<V> {
    let Some(list) = data
        .alias
        .as_ref()
        .filter(|ty| ty.name == "List" && ty.arguments.len() == 1)
    else {
        return vec![];
    };
    if budgets.collection_size < 2 {
        return vec![];
    }
    let element = &list.arguments[0];
    let Some(first) = generate_values(ir, element, budgets)
        .values
        .into_iter()
        .next()
    else {
        return vec![];
    };
    let mut result = vec![V::List(vec![first.clone(), first.clone()])];
    // Also vary one non-key field: deduplicating complete records is weaker
    // than enforcing uniqueness of a selected field such as attemptId.
    let mut shape = element;
    for _ in 0..=budgets.recursion_depth {
        let Some(record) = shape.symbol_id.as_ref().and_then(|id| ir.data.get(id)) else {
            break;
        };
        if let Some(alias) = &record.alias {
            shape = alias;
            continue;
        }
        if let V::Record(fields) = &first {
            for field in record.fields.iter().take(16) {
                if let Some(other) = generate_values(ir, &field.ty, budgets)
                    .values
                    .into_iter()
                    .find(|value| fields.get(&field.name) != Some(value))
                {
                    let mut changed = fields.clone();
                    changed.insert(field.name.clone(), other);
                    let changed = V::Record(changed);
                    if check_value(ir, element, &changed, budgets) == ValueContract::Accepted {
                        result.push(V::List(vec![first.clone(), changed]));
                    }
                }
            }
        }
        break;
    }
    // The caller retains only values rejected by the complete outer contract.
    result
}
