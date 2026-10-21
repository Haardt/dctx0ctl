use clap::{Args, ValueEnum};
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Verb {
    /// Export complete dependency-closed context with shared content references.
    Package,
    /// Compare saved --file package with current package for this anchor.
    PackageDelta,
    /// Compare task state at explicit start/end event pins.
    Progress,
    Preview,
    Plan,
    List,
    Waves,
    Next,
    Show,
    Context,
    History,
    Dependencies,
    Diff,
    Start,
    ClaimNext,
    Release,
    Complete,
    Slice,
    ClaimSlice,
    ReleaseSlice,
    RecordCheck,
    RecordReview,
    RenewReview,
    AddNote,
    FollowUp,
    ProfileChanges,
    ProfileUpdate,
}
#[derive(Args)]
pub(crate) struct TaskArgs {
    #[arg(value_enum)]
    pub(super) action: Verb,
    pub(super) input: PathBuf,
    #[arg(long)]
    pub(super) project: Option<PathBuf>,
    #[arg(long)]
    pub(super) target: Option<String>,
    #[arg(long = "task")]
    pub(super) tasks: Vec<String>,
    #[arg(long)]
    pub(super) semantic: Option<String>,
    #[arg(long)]
    pub(super) all_affected: bool,
    pub(super) id: Option<String>,
    pub(super) other: Option<String>,
    #[arg(long)]
    pub(super) plan: Option<String>,
    #[arg(long, default_value = "latest")]
    pub(super) snapshot: String,
    #[arg(long)]
    pub(super) dry_run: bool,
    #[arg(long)]
    pub(super) actor: Option<String>,
    #[arg(long)]
    pub(super) wave: Option<usize>,
    #[arg(long)]
    pub(super) state: Option<String>,
    #[arg(long)]
    pub(super) check: Option<String>,
    #[arg(long)]
    pub(super) kind: Option<String>,
    #[arg(long)]
    pub(super) subject: Option<String>,
    #[arg(long)]
    pub(super) domain: Option<String>,
    #[arg(long)]
    pub(super) claim: Option<i64>,
    #[arg(long)]
    pub(super) version: Option<i64>,
    #[arg(long)]
    pub(super) reservation: Option<i64>,
    #[arg(long)]
    pub(super) reason: Option<String>,
    #[arg(long)]
    pub(super) summary: Option<String>,
    #[arg(long)]
    pub(super) implementation: Option<String>,
    #[arg(long)]
    pub(super) artifact: Vec<String>,
    #[arg(long)]
    pub(super) file: Option<PathBuf>,
    #[arg(long)]
    pub(super) finding: Option<String>,
    #[arg(long, default_value = "core")]
    pub(super) section: String,
    #[arg(long, default_value = "both")]
    pub(super) direction: String,
    #[arg(long)]
    pub(super) transitive: bool,
    #[arg(long, default_value = "50")]
    pub(super) limit: usize,
    #[arg(long, default_value = "24576")]
    pub(super) budget: usize,
    #[arg(long)]
    pub(super) cursor: Option<String>,
    #[arg(long)]
    pub(super) as_of: Option<i64>,
    #[arg(long)]
    pub(super) from_plan: Option<i64>,
    #[arg(long)]
    pub(super) from_event: Option<i64>,
    #[arg(long)]
    pub(super) format: Option<String>,
    #[arg(long)]
    pub(super) all_plans: bool,
}
