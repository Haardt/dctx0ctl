//! Read-only application inventory/parse cost; compiler publication is setup.
use dctx0_engine::Project;
use std::io::Write;
#[cfg(feature = "typescript")]
pub(crate) fn benchmark(
    project: &Project,
    iterations: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    if project.database == project.root.join(".dctx0ctl/state.sqlite3") {
        return Err("Discovery benchmarks require --db pointing to a temporary database".into());
    }
    project.check().map_err(crate::error_message)?;
    let application = project
        .root
        .parent()
        .ok_or("Spec root has no application parent")?;
    let mut hash = None;
    let mut files = 0;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let report = project
            .discover_verification(application, &dctx0_target_typescript::TypeScriptInspector)
            .map_err(crate::error_message)?;
        files = report.source_hashes.len();
        let fingerprint = dctx0_store::VerificationArtifact::new(
            "application/json",
            serde_json::to_vec(&report.source_hashes).map_err(crate::error_message)?,
        )
        .reference
        .hash;
        if hash.as_ref().is_some_and(|before| before != &fingerprint) {
            return Err("Discovery benchmark sources changed between iterations".into());
        }
        hash = Some(fingerprint);
        std::hint::black_box(report);
    }
    crate::render::json(
        output,
        &serde_json::json!({"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"iterations":iterations,"files":files,"valid":true,"peak_rss_bytes":crate::metrics::peak_rss(&std::fs::read_to_string("/proc/self/status").unwrap_or_default()),"application_source_hash":hash}),
    )?;
    Ok(true)
}
#[cfg(not(feature = "typescript"))]
pub(crate) fn benchmark(
    _project: &Project,
    _iterations: usize,
    _output: &mut dyn Write,
) -> Result<bool, String> {
    Err("TypeScript structural inspection is unavailable in this build".into())
}
