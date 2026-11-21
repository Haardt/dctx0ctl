use crate::{Change, ChangeFacet, ChangeKind, Changes, RenameMapping, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

pub fn diff(before: Option<&Snapshot>, after: &Snapshot, mappings: &[RenameMapping]) -> Changes {
    let Some(before) = before else {
        return Changes {
            before: None,
            after: after.id.clone(),
            source_changed: true,
            changes: Vec::new(),
        };
    };
    let old = &before.fingerprints;
    let renames = mappings
        .iter()
        .map(|m| (m.old.clone(), m.new.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut changes = compare_fingerprints(old, &after.fingerprints, |id| {
        Some(renames.get(id).unwrap_or(id))
    });
    annotate_impacts(&mut changes, before, after);
    Changes {
        before: Some(before.id.clone()),
        after: after.id.clone(),
        source_changed: before.source_hash != after.source_hash,
        changes,
    }
}

/// Compare the two sets of fingerprints using the caller's occurrence-specific
/// lineage. An absent counterpart is a removal, even if a name was later reused.
pub fn compare_fingerprints<'a>(
    old: &'a BTreeMap<String, crate::Fingerprints>,
    new: &BTreeMap<String, crate::Fingerprints>,
    counterpart: impl Fn(&'a String) -> Option<&'a String>,
) -> Vec<Change> {
    let mut matched = BTreeSet::new();
    let mut changes = Vec::new();
    for (id, previous) in old {
        let found = counterpart(id).and_then(|id| new.get_key_value(id));
        let mut facets = Vec::new();
        if let Some((new_id, current)) = found {
            matched.insert(new_id);
            if previous.contract != current.contract {
                facets.push(ChangeFacet::Contract);
            }
            if previous.behavior != current.behavior {
                facets.push(ChangeFacet::Behavior);
            }
            if previous.facts != current.facts {
                facets.push(ChangeFacet::Facts);
            }
            if previous.location != current.location {
                facets.push(ChangeFacet::Location);
            }
            if facets.is_empty() && id == new_id {
                continue;
            }
        }
        changes.push(Change {
            old: Some(id.clone()),
            new: found.map(|(id, _)| id.clone()),
            kind: if found.is_none() {
                ChangeKind::Removed
            } else if found.is_some_and(|(new, _)| id != new) {
                ChangeKind::Renamed
            } else {
                ChangeKind::Changed
            },
            facets,
            impacts: Vec::new(),
        });
    }
    for id in new.keys() {
        if !matched.contains(id) {
            changes.push(Change {
                old: None,
                new: Some(id.clone()),
                kind: ChangeKind::Added,
                facets: Vec::new(),
                impacts: Vec::new(),
            });
        }
    }
    changes.sort_by(|a, b| (&a.old, &a.new).cmp(&(&b.old, &b.new)));
    changes
}

/// Build traversal indexes only when a delta exists, then reuse across changes.
fn annotate_impacts(changes: &mut [Change], before: &Snapshot, after: &Snapshot) {
    if changes.is_empty() {
        return;
    }
    let before_index = before.graph.index();
    let after_index = after.graph.index();
    for change in changes {
        let mut impacts = Vec::new();
        if let Some(id) = &change.old {
            impacts.extend(before_index.impact(id));
        }
        if let Some(id) = &change.new {
            impacts.extend(after_index.impact(id));
        }
        impacts.sort_by(|a, b| (&a.subject, &a.path).cmp(&(&b.subject, &b.path)));
        impacts.dedup();
        change.impacts = impacts;
    }
}
