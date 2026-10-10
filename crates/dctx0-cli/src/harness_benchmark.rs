//! Artifact generation cost, separately from native process costs.
use dctx0_core::WorkspaceIr;
use std::io::Write;
#[cfg(feature = "typescript")]
pub(crate) fn benchmark(
    ir: &WorkspaceIr,
    iterations: usize,
    files: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    use dctx0_target_sdk::{HarnessGenerator, HarnessRequest};
    let request = HarnessRequest {
        adapter_import: "./adapter.ts".into(),
        subjects: ir.symbols.keys().cloned().collect(),
    };
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(
            dctx0_target_typescript::BunHarness
                .generate(&request)
                .map_err(crate::error_message)?,
        );
    }
    crate::benchmark::benchmark_report(output, start, iterations, files)
}
#[cfg(not(feature = "typescript"))]
pub(crate) fn benchmark(
    _ir: &WorkspaceIr,
    _iterations: usize,
    _files: usize,
    _output: &mut dyn Write,
) -> Result<bool, String> {
    Err("TypeScript/Bun harness is unavailable in this build".into())
}
