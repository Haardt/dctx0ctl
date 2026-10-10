use dctx0_core::Diagnostic;

#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    #[error("Pending rename journal; run check to recover before previewing")]
    PendingJournal,
    #[error("New name must be one non-keyword identifier")]
    InvalidName,
    #[error("Cannot rename an invalid workspace: {}", diagnostic_json(.diagnostics))]
    InvalidWorkspace { diagnostics: Vec<Diagnostic> },
    #[error("Unknown qualified symbol {symbol}")]
    MissingSymbol { symbol: String },
    #[error("New name is unchanged")]
    UnchangedName,
    #[error("Symbol {symbol} already exists")]
    NameConflict { symbol: String },
    #[error("Rename would invalidate the workspace: {}", diagnostic_json(.diagnostics))]
    InvalidResult { diagnostics: Vec<Diagnostic> },
    #[error("Reference range does not match source")]
    InvalidReference,
    #[error("Overlapping rename edits")]
    OverlappingEdits,
    #[error("Sources changed during rename; inspect any retained journal and run check")]
    SourcesChanged,
    #[error("Concurrent source edit; rename journal retained for recovery")]
    ConcurrentEdit,
    #[error("Unsafe source path in rename")]
    UnsafeSource,
    #[error("Rename source escaped project or is a symlink")]
    EscapedSource,
    #[error("Unsupported rename journal or different database; recovery stopped")]
    UnsupportedJournal,
    #[error("Duplicate journal source")]
    DuplicateSource,
    #[error("Recovery stopped: {file} has intervening edits; journal retained")]
    InterveningEdit { file: String },
}
fn diagnostic_json(diagnostics: &[Diagnostic]) -> String {
    serde_json::to_string(diagnostics).expect("diagnostics contain only JSON-compatible values")
}
