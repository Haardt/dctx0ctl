//! Type-directed representation of modeled values, never of native observations.
use crate::{ValueContract, check_value, number};
use dctx0_core::{
    TypeRef, VerificationBudgets, VerificationIssue, VerificationValue as V, WorkspaceIr,
};
use num_rational::BigRational;
use num_traits::ToPrimitive;

/// Exact modeled numeric literals may inhabit a Float contract when their IEEE
/// representation preserves the value. Native results are checked without this
/// conversion, so a wrong adapter representation cannot be repaired here.
pub fn modeled_value(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    value: V,
    budgets: &VerificationBudgets,
) -> Result<V, ValueContract> {
    let value = convert(ir, ty, value, budgets, 0)?;
    match check_value(ir, ty, &value, budgets) {
        ValueContract::Accepted => Ok(value),
        other => Err(other),
    }
}
fn unknown(code: &str, message: &str) -> ValueContract {
    ValueContract::Unknown {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: None,
        },
    }
}
fn convert(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    value: V,
    budgets: &VerificationBudgets,
    depth: usize,
) -> Result<V, ValueContract> {
    if depth > budgets.recursion_depth {
        return Err(unknown(
            "TYPE_DEPTH",
            "Modeled value representation exceeded its type budget",
        ));
    }
    if let Some(data) = ty.symbol_id.as_ref().and_then(|id| ir.data.get(id)) {
        if let Some(alias) = &data.alias {
            return convert(ir, alias, value, budgets, depth + 1);
        }
        return match value {
            V::Record(mut fields) => {
                for field in &data.fields {
                    if let Some(value) = fields.remove(&field.name) {
                        fields.insert(
                            field.name.clone(),
                            convert(ir, &field.ty, value, budgets, depth + 1)?,
                        );
                    }
                }
                Ok(V::Record(fields))
            }
            V::Variant {
                declaration,
                name,
                payload: Some(payload),
            } => {
                let payload = match data
                    .variants
                    .iter()
                    .find(|variant| variant.name == name)
                    .and_then(|variant| variant.payload.as_ref())
                {
                    Some(ty) => convert(ir, ty, *payload, budgets, depth + 1)?,
                    None => *payload,
                };
                Ok(V::Variant {
                    declaration,
                    name,
                    payload: Some(Box::new(payload)),
                })
            }
            other => Ok(other),
        };
    }
    match (ty.name.as_str(), value) {
        ("Float", V::Integer(text) | V::Decimal(text)) => {
            let exact = number::parse(&text, false).ok_or_else(|| ValueContract::Rejected {
                reason: "Invalid modeled number".into(),
            })?;
            let native = exact
                .to_f64()
                .filter(|f| f.is_finite())
                .filter(|f| BigRational::from_float(*f).as_ref() == Some(&exact))
                .ok_or_else(|| {
                    unknown(
                        "FLOAT_REPRESENTATION",
                        "Modeled number cannot be represented losslessly as Float",
                    )
                })?;
            Ok(V::Float(native.to_string()))
        }
        ("List", V::List(values)) if ty.arguments.len() == 1 => Ok(V::List(
            values
                .into_iter()
                .map(|value| convert(ir, &ty.arguments[0], value, budgets, depth + 1))
                .collect::<Result<_, _>>()?,
        )),
        ("Option", V::Option(Some(value))) if ty.arguments.len() == 1 => Ok(V::Option(Some(
            Box::new(convert(ir, &ty.arguments[0], *value, budgets, depth + 1)?),
        ))),
        (_, value) => Ok(value),
    }
}
