use dctx0_core::{Source, WorkspaceIr};
use dctx0_graph::{Impact, SemanticGraph};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprints {
    pub contract: String,
    pub behavior: String,
    pub facts: String,
    pub location: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub id: String,
    pub source_hash: String,
    pub semantic_hash: String,
    pub sources: Vec<Source>,
    pub ir: WorkspaceIr,
    pub graph: SemanticGraph,
    pub fingerprints: BTreeMap<String, Fingerprints>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameMapping {
    pub old: String,
    pub new: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
    Renamed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub old: Option<String>,
    pub new: Option<String>,
    pub kind: ChangeKind,
    pub facets: Vec<ChangeFacet>,
    pub impacts: Vec<Impact>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub before: Option<String>,
    pub after: String,
    pub source_changed: bool,
    pub changes: Vec<Change>,
}

/// Independent aspects of a semantic change; serialized names stay stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeFacet {
    Contract,
    Behavior,
    Facts,
    Location,
}
