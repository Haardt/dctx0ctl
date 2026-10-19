//! Versioned deterministic counterexample candidates; native evaluation stays external.
use crate::{OperationPlan, operation_ready};
use dctx0_core::*;
use dctx0_interpreter::{ObservationContext, ValueContract, check_value};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const SHRINK_VERSION: &str = "dctx0-shrink-2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShrinkAttempt {
    pub invocation: Invocation,
    pub result: VerificationResult,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShrinkReport {
    pub version: String,
    pub original: Invocation,
    pub minimized: Invocation,
    pub attempts: Vec<ShrinkAttempt>,
    pub exhausted: bool,
    pub issues: Vec<VerificationIssue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleShrinkAttempt {
    pub input: crate::LifecycleCaseInput,
    pub assessment: crate::LifecycleAssessment,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleShrinkReport {
    pub version: String,
    pub original: crate::LifecycleCaseInput,
    pub original_assessment: crate::LifecycleAssessment,
    pub minimized: crate::LifecycleCaseInput,
    pub attempts: Vec<LifecycleShrinkAttempt>,
    pub exhausted: bool,
    pub issues: Vec<VerificationIssue>,
}
fn candidates(
    value: &VerificationValue,
    depth: usize,
    budgets: &VerificationBudgets,
) -> Vec<VerificationValue> {
    use VerificationValue as V;
    if depth >= budgets.recursion_depth || budgets.shrink_steps == 0 {
        return vec![];
    }
    let mut out = vec![];
    match value {
        V::Integer(text) => {
            if let Ok(number) = text.parse::<BigInt>() {
                for candidate in [
                    BigInt::zero(),
                    &number / 2,
                    if number.is_negative() {
                        BigInt::from(-1)
                    } else {
                        BigInt::from(1)
                    },
                ] {
                    if candidate.abs() < number.abs() {
                        out.push(V::Integer(candidate.to_string()));
                    }
                }
            }
        }
        V::Decimal(text) => {
            if text != "0" {
                out.push(V::Decimal("0".into()));
            }
            if let Some((whole, fraction)) = text.split_once('.') {
                if !fraction.is_empty() {
                    out.push(V::Decimal(whole.into()));
                }
            }
        }
        V::Float(text) => {
            if let Ok(number) = text.parse::<f64>() {
                if number.is_finite() && number != 0.0 {
                    out.push(V::Float("0".into()));
                    let half = number / 2.0;
                    if half != number && half != 0.0 {
                        out.push(V::Float(half.to_string()));
                    }
                }
            }
        }
        V::String(text) => {
            let count = text.chars().count();
            if count > 0 {
                out.push(V::String(String::new().into()));
                out.push(V::String(
                    text.chars().take(count / 2).collect::<String>().into(),
                ));
            }
        }
        V::List(items) => {
            if !items.is_empty() {
                out.push(V::List(vec![]));
                for index in 0..items.len() {
                    if out.len() >= budgets.shrink_steps {
                        break;
                    }
                    let mut shorter = items.clone();
                    shorter.remove(index);
                    out.push(V::List(shorter));
                }
                for (index, item) in items.iter().enumerate() {
                    if out.len() >= budgets.shrink_steps {
                        break;
                    }
                    for smaller in candidates(item, depth + 1, budgets) {
                        if out.len() >= budgets.shrink_steps {
                            break;
                        }
                        let mut changed = items.clone();
                        changed[index] = smaller;
                        out.push(V::List(changed));
                    }
                }
            }
        }
        V::Record(fields) => {
            for (name, value) in fields {
                if out.len() >= budgets.shrink_steps {
                    break;
                }
                for smaller in candidates(value, depth + 1, budgets) {
                    if out.len() >= budgets.shrink_steps {
                        break;
                    }
                    let mut changed = fields.clone();
                    changed.insert(name.clone(), smaller);
                    out.push(V::Record(changed));
                }
            }
        }
        V::Option(Some(payload)) => {
            out.push(V::Option(None));
            for smaller in candidates(payload, depth + 1, budgets) {
                if out.len() >= budgets.shrink_steps {
                    break;
                }
                out.push(V::Option(Some(Box::new(smaller))));
            }
        }
        V::Variant {
            declaration,
            name,
            payload: Some(payload),
        } => {
            for smaller in candidates(payload, depth + 1, budgets) {
                if out.len() >= budgets.shrink_steps {
                    break;
                }
                out.push(V::Variant {
                    declaration: declaration.clone(),
                    name: name.clone(),
                    payload: Some(Box::new(smaller)),
                });
            }
        }
        V::Failure {
            declaration,
            payload: Some(payload),
        } => {
            for smaller in candidates(payload, depth + 1, budgets) {
                if out.len() >= budgets.shrink_steps {
                    break;
                }
                out.push(V::Failure {
                    declaration: declaration.clone(),
                    payload: Some(Box::new(smaller)),
                });
            }
        }
        _ => {}
    }
    let mut unique = vec![];
    for candidate in out {
        if candidate != *value && !unique.contains(&candidate) {
            unique.push(candidate);
        }
        if unique.len() >= budgets.shrink_steps {
            break;
        }
    }
    unique
}
/// Candidates must retain type contracts and evaluable preconditions. Actual
/// state/setup observers are supplied by the caller; unavailable guards stay open.
pub fn shrink_invocations(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    original: &Invocation,
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> (Vec<Invocation>, Vec<VerificationIssue>) {
    let mut accepted = vec![];
    let mut issues = vec![];
    if original.subject != plan.operation.id
        || original.arguments.len() != plan.operation.parameters.len()
    {
        return (
            accepted,
            vec![VerificationIssue {
                code: "SHRINK_SIGNATURE".into(),
                message: "Counterexample does not match its pinned signature".into(),
                span: ir
                    .symbols
                    .get(&plan.operation.id)
                    .map(|symbol| symbol.span.clone()),
            }],
        );
    }
    if budgets.shrink_steps == 0 {
        return (accepted, issues);
    }
    if let VerificationResult::Fail { issue } | VerificationResult::Unknown { issue } =
        operation_ready(ir, plan, original, context, budgets)
    {
        return (accepted, vec![issue]);
    }
    let mut visited = BTreeSet::new();
    for (index, (parameter, value)) in plan
        .operation
        .parameters
        .iter()
        .zip(&original.arguments)
        .enumerate()
    {
        for smaller in candidates(value, 0, budgets) {
            if serde_json::to_vec(&smaller).is_ok_and(|bytes| bytes.len() > budgets.value_bytes) {
                continue;
            }
            match check_value(ir, &parameter.ty, &smaller, budgets) {
                ValueContract::Accepted => {}
                ValueContract::Rejected { .. } => continue,
                ValueContract::Unknown { issue } => {
                    issues.push(issue);
                    continue;
                }
            }
            let mut invocation = original.clone();
            invocation.arguments[index] = smaller;
            match operation_ready(ir, plan, &invocation, context, budgets) {
                VerificationResult::Pass => {}
                VerificationResult::Fail { issue } | VerificationResult::Unknown { issue } => {
                    if issue.code != "OPERATION_PRECONDITION_UNMET" {
                        issues.push(issue)
                    }
                    continue;
                }
            }
            if let Ok(key) = serde_json::to_string(&invocation) {
                if visited.insert(key) {
                    accepted.push(invocation)
                }
            }
            if accepted.len() >= budgets.shrink_steps {
                return (accepted, issues);
            }
        }
    }
    (accepted, issues)
}

/// Propose smaller session inputs without altering keys or the pinned path.
/// These are candidates only: actual execution must retain prerequisites and
/// the original failing transition before a candidate can be accepted.
pub fn shrink_lifecycle_inputs(
    ir: &WorkspaceIr,
    plan: &crate::LifecyclePlan,
    path: &crate::LifecyclePath,
    original: &crate::LifecycleCaseInput,
    budgets: &VerificationBudgets,
) -> (Vec<crate::LifecycleCaseInput>, Vec<VerificationIssue>) {
    let problem = |message: &str| VerificationIssue {
        code: "LIFECYCLE_SHRINK_SIGNATURE".into(),
        message: message.into(),
        span: None,
    };
    match crate::lifecycle_input_contract(ir, plan, path, original, budgets) {
        VerificationResult::Pass => {}
        VerificationResult::Fail { issue } | VerificationResult::Unknown { issue } => {
            return (vec![], vec![problem(&issue.message)]);
        }
    }
    let mut invocations = original.setup.clone();
    for (transition, arguments) in path.transitions.iter().zip(&original.arguments) {
        let transition = &plan.transitions[*transition];
        invocations.push(Invocation {
            subject: transition.operation.clone(),
            arguments: arguments.clone(),
        });
    }
    if budgets.shrink_steps == 0 {
        return (vec![], vec![]);
    }
    let mut out = vec![];
    for index in 0..original.setup.len() {
        let mut candidate = original.clone();
        candidate.setup.remove(index);
        out.push(candidate);
        if out.len() >= budgets.shrink_steps {
            return (out, vec![]);
        }
    }
    for (index, invocation) in invocations.iter().enumerate() {
        let operation = &ir.operations[&invocation.subject];
        for (argument, (parameter, value)) in operation
            .parameters
            .iter()
            .zip(&invocation.arguments)
            .enumerate()
        {
            for smaller in candidates(value, 0, budgets) {
                if !preserves_lifecycle_keys(value, &smaller, &original.keys)
                    || !matches!(
                        check_value(ir, &parameter.ty, &smaller, budgets),
                        ValueContract::Accepted
                    )
                {
                    continue;
                }
                let mut candidate = original.clone();
                if index < original.setup.len() {
                    candidate.setup[index].arguments[argument] = smaller;
                } else {
                    candidate.arguments[index - original.setup.len()][argument] = smaller;
                }
                if !out.contains(&candidate) {
                    out.push(candidate);
                }
                if out.len() >= budgets.shrink_steps {
                    return (out, vec![]);
                }
            }
        }
    }
    (out, vec![])
}

fn has_lifecycle_key(value: &VerificationValue, keys: &[VerificationValue]) -> bool {
    use VerificationValue as V;
    keys.contains(value)
        || match value {
            V::Record(fields) => fields.values().any(|value| has_lifecycle_key(value, keys)),
            V::List(values) => values.iter().any(|value| has_lifecycle_key(value, keys)),
            V::Option(Some(value))
            | V::Variant {
                payload: Some(value),
                ..
            }
            | V::Failure {
                payload: Some(value),
                ..
            } => has_lifecycle_key(value, keys),
            _ => false,
        }
}
fn preserves_lifecycle_keys(
    before: &VerificationValue,
    after: &VerificationValue,
    keys: &[VerificationValue],
) -> bool {
    use VerificationValue as V;
    if keys.contains(before) {
        return before == after;
    }
    match (before, after) {
        (V::Record(before), V::Record(after)) => before.iter().all(|(field, value)| {
            after
                .get(field)
                .is_some_and(|next| preserves_lifecycle_keys(value, next, keys))
        }),
        (V::Option(Some(before)), V::Option(Some(after))) => {
            preserves_lifecycle_keys(before, after, keys)
        }
        (
            V::Variant {
                declaration: a,
                name: x,
                payload: Some(before),
            },
            V::Variant {
                declaration: b,
                name: y,
                payload: Some(after),
            },
        ) if a == b && x == y => preserves_lifecycle_keys(before, after, keys),
        _ => !has_lifecycle_key(before, keys) || before == after,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_value_candidates_keep_their_declaration_payload_shape_and_unicode_boundaries() {
        use VerificationValue as V;
        use std::collections::BTreeMap;
        let compiled = dctx0_language::compile(&[Source {
            file: "error-values.dctx0".into(),
            text: "workspace W { failure Broken { reason:String\ncode:Int } }".into(),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let ir = compiled.ir.unwrap();
        // Failure<T> is the internal outcome type, not a DSL parameter syntax.
        let ty = TypeRef {
            name: "Failure".into(),
            symbol_id: None,
            arguments: vec![TypeRef {
                name: "Broken".into(),
                symbol_id: Some("W.Broken".into()),
                arguments: vec![],
            }],
        };
        let original = V::Failure {
            declaration: "W.Broken".into(),
            payload: Some(Box::new(V::Record(BTreeMap::from([
                ("reason".into(), V::String("ä🍿broken".into())),
                ("code".into(), V::Integer("16".into())),
            ])))),
        };
        let budgets = VerificationBudgets::default();
        assert_eq!(
            check_value(&ir, &ty, &original, &budgets),
            ValueContract::Accepted
        );
        let smaller = candidates(&original, 0, &budgets);
        assert!(!smaller.is_empty());
        for value in &smaller {
            assert_eq!(
                check_value(&ir, &ty, value, &budgets),
                ValueContract::Accepted
            );
            let V::Failure {
                declaration,
                payload: Some(payload),
            } = value
            else {
                panic!("Error identity and payload must remain intact");
            };
            assert_eq!(declaration, "W.Broken");
            let V::Record(fields) = payload.as_ref() else {
                panic!("Error payload remains a record");
            };
            assert_eq!(fields.len(), 2);
            let shortened_text = matches!(&fields["reason"], V::String(text) if text.chars().count() < "ä🍿broken".chars().count());
            let shortened_code = fields["code"] != V::Integer("16".into());
            assert_ne!(
                shortened_text, shortened_code,
                "Each candidate changes exactly one payload field"
            );
        }
        assert_eq!(smaller, candidates(&original, 0, &budgets));
        assert_eq!(
            candidates(
                &original,
                0,
                &VerificationBudgets {
                    shrink_steps: 1,
                    ..budgets.clone()
                }
            )
            .len(),
            1
        );
        assert!(
            candidates(
                &original,
                0,
                &VerificationBudgets {
                    recursion_depth: 2,
                    ..budgets
                }
            )
            .is_empty()
        );
        assert!(
            candidates(
                &V::Failure {
                    declaration: "W.Broken".into(),
                    payload: None
                },
                0,
                &Default::default()
            )
            .is_empty()
        );
    }
}
