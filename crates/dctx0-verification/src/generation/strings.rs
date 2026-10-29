//! Candidate construction for literal string prefixes, never a contract solver.
use dctx0_core::{
    ExpressionKind as E, StatementKind as K, TypeRef, VerificationBudgets, VerificationValue as V,
    WorkspaceIr,
};
use std::collections::BTreeSet;

pub(super) fn prefix_witnesses(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    budgets: &VerificationBudgets,
) -> Vec<V> {
    let mut prefixes = BTreeSet::new();
    let mut lengths = BTreeSet::new();
    let mut current = ty;
    for _ in 0..=budgets.recursion_depth.min(ir.data.len()) {
        let Some(id) = &current.symbol_id else { break };
        for node in ir.nodes.iter().filter(|node| node.owner == *id) {
            if node.kind == K::Length {
                let text = node
                    .terms
                    .iter()
                    .map(|token| token.text.as_str())
                    .collect::<String>();
                if let Some((minimum, maximum)) = text.split_once("..") {
                    lengths.extend(
                        [minimum, maximum]
                            .into_iter()
                            .filter_map(|bound| bound.parse::<usize>().ok()),
                    );
                }
            } else if node.kind == K::Where {
                for expression in &node.expressions {
                    if let E::Call {
                        name,
                        target: None,
                        arguments,
                    } = &expression.value
                        && name == "startsWith"
                        && let [value, prefix] = arguments.as_slice()
                        && matches!(&value.value, E::Variable { name } if name == "value")
                        && let E::Literal { text, literal_kind } = &prefix.value
                        && literal_kind == "String"
                    {
                        prefixes.insert(text.as_str());
                    }
                }
            }
        }
        let Some(alias) = ir.data.get(id).and_then(|data| data.alias.as_ref()) else {
            break;
        };
        current = alias;
    }
    let mut values = Vec::new();
    for prefix in prefixes {
        let prefix_length = prefix.encode_utf16().count();
        for length in std::iter::once(prefix_length).chain(lengths.iter().copied()) {
            let Some(padding) = length.checked_sub(prefix_length) else {
                continue;
            };
            if prefix.len().saturating_add(padding) > budgets.value_bytes.saturating_sub(32) {
                continue;
            }
            values.push(V::String(format!("{prefix}{}", "x".repeat(padding)).into()));
        }
    }
    values
}
