//! Versioned semantic snapshots and explicit, conservative change tracking.
mod diff;
mod hashing;
mod history;
mod model;
mod snapshot;
pub use diff::diff;
pub use hashing::{hash, semantic_value, source_hash};
pub use history::{
    HistoryEntry, HistoryReader, HistoryWriter, LineagePath, RenameEvent, RenameLineage,
};
pub use model::*;
pub use snapshot::snapshot;
