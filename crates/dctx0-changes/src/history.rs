//! History contracts and rename continuity rules, independent of SQLite.
use crate::{Changes, Fingerprints, RenameMapping, Snapshot};
use dctx0_core::Diagnostic;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub run_id: i64,
    pub snapshot_id: Option<String>,
    pub status: String,
    pub source_hash: String,
    pub timestamp: String,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameEvent {
    pub id: String,
    pub before: String,
    pub after: String,
    pub mappings: Vec<RenameMapping>,
}

/// Queries do not require publishing permissions.
pub trait HistoryReader {
    type Error;
    fn latest(&self, project: &str) -> Result<Option<Snapshot>, Self::Error>;
    fn load(&self, project: &str, id: &str) -> Result<Snapshot, Self::Error>;
    fn history(&self, project: &str) -> Result<Vec<HistoryEntry>, Self::Error>;
    fn rename_committed(&self, id: &str) -> Result<bool, Self::Error>;
    fn rename_mappings(
        &self,
        project: &str,
        before: &str,
        after: &str,
    ) -> Result<Vec<RenameMapping>, Self::Error>;
}
/// Publication must read the valid head and commit the new head/event atomically.
pub trait HistoryWriter: HistoryReader {
    fn failure(
        &mut self,
        project: &str,
        source_hash: &str,
        diagnostics: &[Diagnostic],
    ) -> Result<(), Self::Error>;
    fn publish(
        &mut self,
        project: &str,
        snapshot: &Snapshot,
        event: Option<&RenameEvent>,
    ) -> Result<Changes, Self::Error>;
}

pub struct LineagePath {
    pub snapshots: Vec<String>,
    pub reverse: bool,
}
impl LineagePath {
    /// Select the latest applicable occurrences from a newest-first run history.
    pub fn between(history: &[HistoryEntry], before: &str, after: &str) -> Result<Self, String> {
        let timeline = history
            .iter()
            .rev()
            .filter_map(|run| run.snapshot_id.as_ref())
            .collect::<Vec<_>>();
        let end = timeline
            .iter()
            .rposition(|id| id.as_str() == after)
            .ok_or_else(|| after.to_owned())?;
        let (start, end, reverse) = if let Some(start) = timeline[..=end]
            .iter()
            .rposition(|id| id.as_str() == before)
        {
            (start, end, false)
        } else {
            let later = timeline
                .iter()
                .rposition(|id| id.as_str() == before)
                .ok_or_else(|| before.to_owned())?;
            (end, later, true)
        };
        Ok(Self {
            snapshots: timeline[start..=end]
                .iter()
                .map(|id| (*id).clone())
                .collect(),
            reverse,
        })
    }
}

/// Explicit mappings compose; deletion permanently ends a declaration's lineage.
pub struct RenameLineage {
    origin_to_current: BTreeMap<String, String>,
}
impl RenameLineage {
    pub fn new(symbols: &BTreeMap<String, Fingerprints>) -> Self {
        Self {
            origin_to_current: symbols.keys().map(|id| (id.clone(), id.clone())).collect(),
        }
    }
    pub fn advance(
        &mut self,
        mappings: &[RenameMapping],
        surviving: &BTreeMap<String, Fingerprints>,
    ) {
        let renames = mappings
            .iter()
            .map(|m| (m.old.as_str(), m.new.as_str()))
            .collect::<BTreeMap<_, _>>();
        for id in self.origin_to_current.values_mut() {
            if let Some(new_name) = renames.get(id.as_str()) {
                *id = (*new_name).to_owned();
            }
        }
        self.origin_to_current
            .retain(|_, id| surviving.contains_key(id));
    }
    pub fn finish(self, reverse: bool) -> Vec<RenameMapping> {
        self.origin_to_current
            .into_iter()
            .filter(|(old, new)| old != new)
            .map(|(old, new)| {
                if reverse {
                    RenameMapping { old: new, new: old }
                } else {
                    RenameMapping { old, new }
                }
            })
            .collect()
    }
}
