//! Fresh testing setup filesystem cost; no production or adapter execution.
use dctx0_core::Source;
use std::io::Write;
#[cfg(feature = "typescript")]
pub(crate) fn benchmark(
    sources: &[Source],
    iterations: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        let root = std::env::temp_dir().join(format!(
            "dctx0-verification-init-bench-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).map_err(crate::error_message)?;
        let result: Result<(), String> = (|| {
            let spec = root.join("spec");
            let app = root.join("app");
            fs::create_dir(&spec).map_err(crate::error_message)?;
            fs::create_dir(&app).map_err(crate::error_message)?;
            // The exact representative source workspace is preserved; init only
            // writes testing configuration and artifacts alongside it.
            for source in sources {
                let path = spec.join(&source.file);
                fs::create_dir_all(
                    path.parent()
                        .expect("source paths are rooted in owned benchmark spec"),
                )
                .map_err(crate::error_message)?;
                fs::write(path, &source.text).map_err(crate::error_message)?;
            }
            let project = dctx0_engine::Project::new(&spec, None).map_err(crate::error_message)?;
            std::hint::black_box(
                project
                    .initialize_verification(
                        &app,
                        &dctx0_target_typescript::BunHarness,
                        Default::default(),
                    )
                    .map_err(crate::error_message)?,
            );
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(root).map_err(crate::error_message);
        result?;
        cleanup?;
    }
    crate::benchmark::benchmark_report(output, started, iterations, sources.len())
}
#[cfg(not(feature = "typescript"))]
pub(crate) fn benchmark(
    _sources: &[Source],
    _iterations: usize,
    _output: &mut dyn Write,
) -> Result<bool, String> {
    Err("TypeScript/Bun setup is unavailable in this build".into())
}
