//! Human instructions derived from the pinned contract, not invented targets.
use crate::*;
use dctx0_changes::Snapshot;
use dctx0_core::StatementKind as K;
use std::collections::BTreeSet;

pub fn assignment(snapshot: &Snapshot, candidate: &Candidate) -> (String, Vec<String>) {
    let action = match candidate.kind {
        TaskKind::Operation => "Implement the surface operation",
        TaskKind::Adapter => "Implement the port adapter operation",
        TaskKind::Integration => "Integrate the delivered implementation for",
        TaskKind::Review => "Review the implementation for",
        TaskKind::Retire => "Retire the obsolete implementation for",
        TaskKind::Contract => "Define the contract for",
    };
    if candidate.kind != TaskKind::Contract {
        return (
            format!(
                "{action} {} according to the specification below.",
                candidate.subjects.join(", ")
            ),
            candidate.acceptance.clone(),
        );
    }
    let mut goals = vec![];
    let mut acceptance = BTreeSet::new();
    for subject in &candidate.subjects {
        if let Some(data) = snapshot.ir.data.get(subject) {
            let kind = snapshot.ir.symbols[subject].kind.as_str();
            if let Some(alias) = &data.alias {
                goals.push(format!(
                    "Implement the {kind} `{subject}` using `{}` as its specified base type.",
                    type_name(alias)
                ));
                acceptance.insert(format!(
                    "Preserve the declared `{}` base type and the named contract.",
                    type_name(alias)
                ));
            } else {
                goals.push(format!(
                    "Implement the {kind} `{subject}` with the structure declared below."
                ));
            }
            if !data.fields.is_empty() {
                acceptance.insert(
                    "Preserve every declared field name, type and optional/list structure.".into(),
                );
            }
            if !data.variants.is_empty() {
                acceptance.insert("Support every declared variant and its payload type.".into());
            }
            if !data.keys.is_empty() {
                acceptance.insert(format!(
                    "Preserve the declared identity keys: {}.",
                    data.keys.join(", ")
                ));
            }
            if let Some(stable_id) = &data.stable_id {
                acceptance.insert(format!(
                    "Preserve the declaration's stableId metadata `{stable_id}`."
                ));
            }
            if snapshot.ir.nodes.iter().any(|n| {
                n.owner == *subject && matches!(n.kind, K::Range | K::Length | K::Format | K::Where)
            }) {
                acceptance.insert("Implement the specified refinements and test their boundaries and rejected values.".into());
            }
        } else {
            goals.push(format!(
                "{action} operation `{subject}` with its declared parameters, result and failures."
            ));
            acceptance.insert("Preserve parameter/result types, declared failures and modeled operation conditions.".into());
        }
    }
    if candidate.subjects.len() > 1 {
        goals.push("Implement this recursive contract component together; preserve references between its members.".into());
    }
    acceptance.insert("Document implementation artifacts and representation decisions; add focused tests without inventing extra requirements.".into());
    (goals.join("\n\n"), acceptance.into_iter().collect())
}
