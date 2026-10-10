//! Concrete before/after facts, separate from task identity and presentation.
use crate::*;
use dctx0_changes::{Snapshot, hash, semantic_value};
use std::collections::{BTreeMap, BTreeSet};

pub struct DeltaIndex<'a> {
    snapshot: &'a Snapshot,
    index: SpecIndex<'a>,
}
impl<'a> DeltaIndex<'a> {
    pub fn new(snapshot: &'a Snapshot) -> Self {
        Self {
            snapshot,
            index: SpecIndex::new(&snapshot.ir),
        }
    }
    fn facts(&self, c: &Candidate) -> BTreeMap<String, (String, String, String)> {
        let mut ids = c.facts.iter().map(String::as_str).collect::<BTreeSet<_>>();
        for ty in &c.types {
            ids.extend(
                self.index
                    .owned
                    .get(ty.as_str())
                    .into_iter()
                    .flatten()
                    .filter(|n| n.id != *ty)
                    .map(|n| n.id.as_str()),
            );
        }
        ids.into_iter().filter_map(|id|self.index.nodes.get(id)).map(|n|{
   let normalized=semantic_value(serde_json::json!({"kind":n.kind,"terms":n.terms.iter().map(|t|(&t.kind,&t.text)).collect::<Vec<_>>(),"expressions":n.expressions}));
   let text=self.snapshot.sources.iter().find(|s|s.file==n.span.file).and_then(|s|s.text.get(n.span.start.offset..n.span.end.offset)).map(str::to_owned).unwrap_or_else(||normalized.to_string());
   (hash(&normalized),(n.owner.clone(),n.kind.as_str().into(),text))
  }).collect()
    }
    pub fn compare(
        &self,
        after: &DeltaIndex<'_>,
        old: &Candidate,
        new: &Candidate,
    ) -> Vec<RequirementChange> {
        let mut changes = vec![];
        let mut subjects = old
            .subjects
            .iter()
            .chain(&old.types)
            .chain(&new.subjects)
            .chain(&new.types)
            .collect::<BTreeSet<_>>();
        for (index, candidate) in [(&self.index, old), (&after.index, new)] {
            for reference in &index.ir.references {
                if candidate.subjects.contains(&reference.from)
                    && index.ir.operations.contains_key(&reference.target)
                {
                    subjects.insert(&reference.target);
                }
            }
        }
        for id in subjects {
            let before = self
                .snapshot
                .ir
                .symbols
                .contains_key(id)
                .then(|| contract_text(&self.snapshot.ir, id));
            let target = after
                .snapshot
                .ir
                .symbols
                .contains_key(id)
                .then(|| contract_text(&after.snapshot.ir, id));
            if before != target {
                changes.push(RequirementChange {
                    subject: id.clone(),
                    facet: "contract".into(),
                    before,
                    after: target,
                });
            }
        }
        let before = self.facts(old);
        let target = after.facts(new);
        for (key, (subject, facet, text)) in &before {
            if !target.contains_key(key) {
                changes.push(RequirementChange {
                    subject: subject.clone(),
                    facet: facet.clone(),
                    before: Some(text.clone()),
                    after: None,
                });
            }
        }
        for (key, (subject, facet, text)) in &target {
            if !before.contains_key(key) {
                changes.push(RequirementChange {
                    subject: subject.clone(),
                    facet: facet.clone(),
                    before: None,
                    after: Some(text.clone()),
                });
            }
        }
        changes
    }
}
