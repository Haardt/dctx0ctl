//! Actual projection milestones; presentation and throttling belong to callers.
#[derive(Debug, Clone, Copy)]
pub enum SiteProgress {
    LoadingRecords,
    PackingHistory { completed: usize, total: usize },
    PackingRuns { completed: usize, total: usize },
    PackingTasks { completed: usize, total: usize },
    EncodingJson,
    RenderingHtml,
    WritingExport,
}
