//! Runtime value contracts derived from validated declaration IR, without target I/O.
use crate::{Evaluation, evaluate, number};
use dctx0_core::{
    StatementKind as K, TypeRef, VerificationBudgets, VerificationIssue, VerificationValue as V,
    WorkspaceIr,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ValueContract {
    Accepted,
    Rejected { reason: String },
    Unknown { issue: VerificationIssue },
}

pub fn check_value(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    value: &V,
    budgets: &VerificationBudgets,
) -> ValueContract {
    let mut remaining = budgets.evaluation_steps;
    let guard = wire_budget(value, budgets, 0, &mut remaining);
    if !matches!(guard, ValueContract::Accepted) {
        return guard;
    }
    Checker { ir, budgets }.check(ty, value, 0)
}
struct Checker<'a> {
    ir: &'a WorkspaceIr,
    budgets: &'a VerificationBudgets,
}
fn rejected(reason: &str) -> ValueContract {
    ValueContract::Rejected {
        reason: reason.into(),
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
impl Checker<'_> {
    fn check(&self, ty: &TypeRef, value: &V, depth: usize) -> ValueContract {
        if depth > self.budgets.recursion_depth {
            return unknown(
                "TYPE_DEPTH",
                "Runtime type closure exceeded its recursion budget",
            );
        }
        if serde_json::to_vec(value).map_or(true, |v| v.len() > self.budgets.value_bytes) {
            return unknown("VALUE_SIZE", "Canonical value exceeds the byte budget");
        }
        if let Some(id) = &ty.symbol_id {
            let Some(data) = self.ir.data.get(id) else {
                return unknown("MISSING_TYPE", "Referenced declaration is unavailable");
            };
            let shape = if let Some(alias) = &data.alias {
                self.check(alias, value, depth + 1)
            } else if !data.variants.is_empty() {
                self.variant(id, &data.variants, value, depth + 1)
            } else {
                match value {
                    V::Record(fields) => {
                        if fields.len() != data.fields.len() {
                            return rejected("Record fields do not match the declared contract");
                        }
                        let mut verdict = ValueContract::Accepted;
                        for field in &data.fields {
                            let Some(value) = fields.get(&field.name) else {
                                return rejected("Required record field is missing");
                            };
                            verdict = combine(verdict, self.check(&field.ty, value, depth + 1));
                        }
                        verdict
                    }
                    _ => rejected("Expected a record"),
                }
            };
            if !matches!(shape, ValueContract::Accepted) {
                return shape;
            }
            self.refinements(id, value)
        } else {
            self.builtin(ty, value, depth)
        }
    }
    fn variant(
        &self,
        id: &str,
        alternatives: &[dctx0_core::VariantIr],
        value: &V,
        depth: usize,
    ) -> ValueContract {
        let V::Variant {
            declaration,
            name,
            payload,
        } = value
        else {
            return rejected("Expected a tagged variant");
        };
        if declaration != id {
            return rejected("Variant belongs to another declaration");
        }
        let Some(alternative) = alternatives.iter().find(|a| &a.name == name) else {
            return rejected("Unknown variant alternative");
        };
        match (&alternative.payload, payload) {
            (None, None) => ValueContract::Accepted,
            (Some(ty), Some(value)) => self.check(ty, value, depth),
            _ => rejected("Variant payload does not match its alternative"),
        }
    }
    fn builtin(&self, ty: &TypeRef, value: &V, depth: usize) -> ValueContract {
        let valid = match (ty.name.as_str(), value) {
            ("Unit", V::Unit) | ("Boolean", V::Boolean(_)) | ("String", V::String(_)) => true,
            ("Int", V::Integer(text)) => number::parse(text, true).is_some(),
            ("Decimal", V::Decimal(text)) => number::parse(text, false).is_some(),
            ("Float", V::Float(text)) => text.parse::<f64>().is_ok_and(f64::is_finite),
            ("UUID", V::String(text)) => uuid(text),
            ("Timestamp", V::String(text)) => chrono::DateTime::parse_from_rfc3339(text).is_ok(),
            ("Duration", _) => {
                return unknown(
                    "UNSUPPORTED_TYPE",
                    "Duration representation is not yet supported",
                );
            }
            ("List", V::List(values)) => {
                let Some(element) = ty.arguments.first().filter(|_| ty.arguments.len() == 1) else {
                    return unknown("TYPE_ARGUMENTS", "List needs one element type");
                };
                let mut verdict = ValueContract::Accepted;
                for value in values {
                    verdict = combine(verdict, self.check(element, value, depth + 1));
                }
                return verdict;
            }
            ("Option", V::Option(value)) => {
                let Some(element) = ty.arguments.first().filter(|_| ty.arguments.len() == 1) else {
                    return unknown("TYPE_ARGUMENTS", "Option needs one element type");
                };
                return value.as_ref().map_or(ValueContract::Accepted, |value| {
                    self.check(element, value, depth + 1)
                });
            }
            (
                "Failure",
                V::Failure {
                    declaration,
                    payload,
                },
            ) => {
                let Some(failure) = ty
                    .arguments
                    .iter()
                    .find(|f| f.symbol_id.as_ref() == Some(declaration))
                else {
                    return rejected("Undeclared failure");
                };
                return payload.as_ref().map_or_else(
                    || self.check(failure, &V::Record(BTreeMap::new()), depth + 1),
                    |value| self.check(failure, value, depth + 1),
                );
            }
            ("Failure", V::Unit) => true,
            (
                "Unit" | "Boolean" | "String" | "Int" | "Decimal" | "Float" | "UUID" | "Timestamp"
                | "List" | "Option" | "Failure",
                _,
            ) => false,
            _ => {
                return unknown(
                    "UNSUPPORTED_TYPE",
                    &format!("Unsupported builtin {}", ty.name),
                );
            }
        };
        if valid {
            ValueContract::Accepted
        } else {
            rejected("Value representation violates the declared type")
        }
    }
    fn refinements(&self, id: &str, value: &V) -> ValueContract {
        let mut verdict = ValueContract::Accepted;
        for node in self.ir.nodes.iter().filter(|node| {
            node.owner == id && matches!(node.kind, K::Range | K::Length | K::Format | K::Where)
        }) {
            let current = match node.kind {
                K::Range => range(
                    &node
                        .terms
                        .iter()
                        .map(|t| t.text.as_str())
                        .collect::<Vec<_>>(),
                    value,
                ),
                K::Where if !node.expressions.is_empty() => {
                    let mut variables = BTreeMap::from([("value".into(), value.clone())]);
                    if let V::Record(fields) = value {
                        variables.extend(fields.clone());
                    }
                    let mut result = ValueContract::Accepted;
                    for expression in &node.expressions {
                        let next = match evaluate(
                            self.ir,
                            expression,
                            &variables,
                            &[],
                            self.budgets,
                        ) {
                            Evaluation::Complete(V::Boolean(true)) => ValueContract::Accepted,
                            Evaluation::Complete(V::Boolean(false)) => {
                                rejected("Modeled where condition is false")
                            }
                            Evaluation::Unknown(issue) | Evaluation::ApplicationFailure(issue) => {
                                ValueContract::Unknown { issue }
                            }
                            _ => unknown(
                                "UNSUPPORTED_REFINEMENT",
                                "Where condition needs invocation observations or has no Boolean result",
                            ),
                        };
                        result = combine(result, next);
                    }
                    result
                }
                _ => unknown(
                    "UNSUPPORTED_REFINEMENT",
                    "Refinement has no supported execution model yet",
                ),
            };
            let current = match current {
                ValueContract::Unknown { mut issue } => {
                    issue.span = Some(node.span.clone());
                    ValueContract::Unknown { issue }
                }
                result => result,
            };
            verdict = combine(verdict, current);
        }
        verdict
    }
}
fn combine(first: ValueContract, second: ValueContract) -> ValueContract {
    match first {
        ValueContract::Unknown { .. } => first,
        ValueContract::Rejected { .. } if !matches!(second, ValueContract::Unknown { .. }) => first,
        _ => second,
    }
}
fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
fn range(terms: &[&str], value: &V) -> ValueContract {
    let text = terms.join("");
    let Some((min, max)) = text.split_once("..") else {
        return unknown("INVALID_REFINEMENT", "Range bounds are unavailable");
    };
    let (Some(min), Some(max)) = (number::parse(min, false), number::parse(max, false)) else {
        return unknown(
            "INVALID_REFINEMENT",
            "Range bounds are not finite decimal numbers",
        );
    };
    if min > max {
        return unknown("INVALID_REFINEMENT", "Range bounds are descending");
    }
    let number = match value {
        V::Integer(text) => number::parse(text, true),
        V::Decimal(text) => number::parse(text, false),
        _ => {
            return unknown(
                "UNSUPPORTED_REFINEMENT",
                "Range needs an exact numeric value representation",
            );
        }
    };
    match number {
        Some(number) if number >= min && number <= max => ValueContract::Accepted,
        _ => rejected("Value is outside its modeled inclusive range"),
    }
}

fn wire_budget(
    value: &V,
    budgets: &VerificationBudgets,
    depth: usize,
    remaining: &mut usize,
) -> ValueContract {
    if depth > budgets.recursion_depth || *remaining == 0 {
        return unknown("VALUE_BUDGET", "Wire value exceeds depth or node budget");
    }
    *remaining -= 1;
    let children: Box<dyn Iterator<Item = &V> + '_> = match value {
        V::Record(fields) => Box::new(fields.values()),
        V::List(values) => Box::new(values.iter()),
        V::Option(Some(value))
        | V::Variant {
            payload: Some(value),
            ..
        }
        | V::Failure {
            payload: Some(value),
            ..
        } => Box::new(std::iter::once(value.as_ref())),
        _ => Box::new(std::iter::empty()),
    };
    for child in children {
        let result = wire_budget(child, budgets, depth + 1, remaining);
        if !matches!(result, ValueContract::Accepted) {
            return result;
        }
    }
    ValueContract::Accepted
}
