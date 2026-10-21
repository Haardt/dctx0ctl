use dctx0_engine::SiteProgress;
use std::time::{Duration, Instant};

pub(super) struct Progress {
    enabled: bool,
    started: Instant,
    last: Option<(&'static str, Instant)>,
}
impl Progress {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            started: Instant::now(),
            last: None,
        }
    }
    pub(super) fn report(&mut self, event: SiteProgress) {
        let (phase, count) = match event {
            SiteProgress::LoadingRecords => ("Loading published sources, tasks and evidence", None),
            SiteProgress::PackingHistory { completed, total } => {
                ("Packing older snapshot sources", Some((completed, total)))
            }
            SiteProgress::PackingRuns { completed, total } => {
                ("Packing verification runs", Some((completed, total)))
            }
            SiteProgress::PackingTasks { completed, total } => {
                ("Packing task evidence", Some((completed, total)))
            }
            SiteProgress::EncodingJson => ("Encoding browser JSON", None),
            SiteProgress::RenderingHtml => ("Rendering self-contained HTML", None),
            SiteProgress::WritingExport => ("Checking and writing export files", None),
        };
        let complete = count.is_some_and(|(done, total)| done == total);
        if !complete
            && self.last.is_some_and(|(name, time)| {
                name == phase && time.elapsed() < Duration::from_secs(1)
            })
        {
            return;
        }
        self.last = Some((phase, Instant::now()));
        let count = count
            .map(|(done, total)| format!(" {done}/{total}"))
            .unwrap_or_default();
        self.message(&format!("{phase}{count}"));
    }
    pub(super) fn message(&self, message: &str) {
        if self.enabled {
            eprintln!(
                "site [{:.1}s]: {message}",
                self.started.elapsed().as_secs_f64()
            );
        }
    }
}
