//! Check publication uses a history port; transaction semantics belong to its adapter.
use crate::{CheckReport, Error, Result};
use dctx0_changes::{HistoryWriter, snapshot, source_hash};
use dctx0_core::{Compilation, Source};

pub(crate) fn record_check<H: HistoryWriter>(
    project: &str,
    sources: &[Source],
    compilation: Compilation,
    history: &mut H,
) -> Result<CheckReport>
where
    Error: From<H::Error>,
{
    let source_hash = source_hash(sources);
    if let Some(ir) = compilation.ir {
        let symbols = ir.symbols.len();
        let operations = ir.operations.len();
        let graph = dctx0_graph::build(&ir);
        let snapshot = snapshot(sources, ir, graph);
        let changes = history.publish(project, &snapshot, None)?;
        Ok(CheckReport {
            ok: true,
            source_hash,
            snapshot_id: Some(snapshot.id),
            changes: Some(changes),
            diagnostics: compilation.diagnostics,
            symbols,
            operations,
        })
    } else {
        history.failure(project, &source_hash, &compilation.diagnostics)?;
        Ok(CheckReport {
            ok: false,
            source_hash,
            snapshot_id: None,
            changes: None,
            diagnostics: compilation.diagnostics,
            symbols: 0,
            operations: 0,
        })
    }
}
