//! Validate static setup fields without treating replaced dataflow placeholders as values.
use dctx0_core::{TypeRef, VerificationBudgets, VerificationValue, WorkspaceIr};
use dctx0_interpreter::{ValueContract, check_value};

pub(super) fn check(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    value: &VerificationValue,
    bound: &[&[String]],
    budgets: &VerificationBudgets,
) -> ValueContract {
    if bound.iter().any(|path| path.is_empty()) {
        // The native worker validates the fully resolved invocation before calling it.
        return ValueContract::Accepted;
    }
    if bound.is_empty() {
        return check_value(ir, ty, value, budgets);
    }
    let Some(data) = ty.symbol_id.as_ref().and_then(|id| ir.data.get(id)) else {
        return check_value(ir, ty, value, budgets);
    };
    if let Some(alias) = &data.alias {
        return check(ir, alias, value, bound, budgets);
    }
    let VerificationValue::Record(fields) = value else {
        return ValueContract::Rejected {
            reason: "Expected a record".into(),
        };
    };
    if fields.len() != data.fields.len()
        || data
            .fields
            .iter()
            .any(|field| !fields.contains_key(&field.name))
    {
        return ValueContract::Rejected {
            reason: "Setup template fields do not match the declared record".into(),
        };
    }
    for field in &data.fields {
        let paths = bound
            .iter()
            .filter(|path| path.first() == Some(&field.name))
            .map(|path| &path[1..])
            .collect::<Vec<_>>();
        let result = check(ir, &field.ty, &fields[&field.name], &paths, budgets);
        if result != ValueContract::Accepted {
            return result;
        }
    }
    ValueContract::Accepted
}
